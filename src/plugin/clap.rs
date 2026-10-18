//! CLAP host: loads a `.clap` binary, creates and activates one plugin
//! instance, feeds it notes and parameter changes, renders its stereo output
//! (allocation-free on the audio thread), saves/loads its state and opens its
//! editor. Written against the CLAP C ABI through `clap-sys`
//! (MIT OR Apache-2.0, https://github.com/micahrj/clap-sys); CLAP itself is by
//! the free-audio project (MIT, https://github.com/free-audio/clap). Hosting
//! choices follow the CLAP reference host `clap-host` (free-audio, MIT) and
//! `clack` by Prokopyl (MIT OR Apache-2.0).
//!
//! Threading: ChipWave keeps every instance inside the engine, which sits
//! behind the audio mutex, so main-thread and audio-thread calls never run at
//! the same time. The audio callback marks its thread (`AUDIO_THREAD`) so the
//! thread-check extension answers correctly.
use super::{Hosted, ParamInfo, MAX_BLOCK};
use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::*;
use clap_sys::ext::audio_ports::*;
use clap_sys::ext::gui::*;
use clap_sys::ext::log::*;
use clap_sys::ext::note_ports::*;
use clap_sys::ext::params::*;
use clap_sys::ext::state::*;
use clap_sys::ext::thread_check::*;
use clap_sys::factory::plugin_factory::*;
use clap_sys::host::clap_host;
use clap_sys::plugin::*;
use clap_sys::process::*;
use clap_sys::stream::*;
use clap_sys::version::CLAP_VERSION;
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};

thread_local! {
    /// set by the audio callback / offline render loop while it runs plugins
    pub static AUDIO_THREAD: Cell<bool> = const { Cell::new(false) };
}

/// one loaded `.clap` binary (entry initialised once, shared by instances)
pub struct ClapLib {
    _lib: libloading::Library,
    entry: *const clap_plugin_entry,
    factory: *const clap_plugin_factory,
}
unsafe impl Send for ClapLib {}
unsafe impl Sync for ClapLib {}

impl Drop for ClapLib {
    fn drop(&mut self) {
        unsafe {
            if let Some(d) = (*self.entry).deinit {
                d();
            }
        }
    }
}

fn libs() -> &'static Mutex<HashMap<PathBuf, Weak<ClapLib>>> {
    static L: std::sync::OnceLock<Mutex<HashMap<PathBuf, Weak<ClapLib>>>> =
        std::sync::OnceLock::new();
    L.get_or_init(Default::default)
}

/// a plugin described by a factory
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Desc {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub features: Vec<String>,
}

impl Desc {
    pub fn is_instrument(&self) -> bool {
        self.features.iter().any(|f| f == "instrument")
    }
}

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

fn cbuf(b: &[c_char]) -> String {
    let bytes: Vec<u8> = b
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The loadable binary of a plugin: the file itself, or inside a bundle
/// folder (macOS `.clap` / `.vst3`: `Contents/MacOS/<CFBundleExecutable>`;
/// VST3 on Windows / Linux: `Contents/<arch>/<name>.vst3|.so`).
pub fn binary_path(path: &Path) -> PathBuf {
    if !path.is_dir() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let contents = path.join("Contents");
    // macOS: Info.plist names the executable
    let mac = contents.join("MacOS");
    if mac.is_dir() {
        let exe = std::fs::read_to_string(contents.join("Info.plist"))
            .ok()
            .and_then(|t| plist_string(&t, "CFBundleExecutable"));
        for name in exe.iter().map(String::as_str).chain([stem.as_str()]) {
            let p = mac.join(name);
            if p.is_file() {
                return p;
            }
        }
        if let Some(p) = std::fs::read_dir(&mac)
            .ok()
            .and_then(|rd| rd.flatten().map(|e| e.path()).find(|p| p.is_file()))
        {
            return p;
        }
    }
    let arch = if cfg!(windows) {
        if cfg!(target_arch = "aarch64") {
            "arm64-win"
        } else {
            "x86_64-win"
        }
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-linux"
    } else {
        "x86_64-linux"
    };
    let ext = if cfg!(windows) { "vst3" } else { "so" };
    let p = contents.join(arch).join(format!("{stem}.{ext}"));
    if p.is_file() {
        return p;
    }
    path.to_path_buf()
}

/// `<key>K</key><string>V</string>` in an Info.plist (XML form)
fn plist_string(t: &str, key: &str) -> Option<String> {
    let at = t.find(&format!("<key>{key}</key>"))?;
    let rest = &t[at..];
    let a = rest.find("<string>")? + "<string>".len();
    let b = rest[a..].find("</string>")?;
    Some(rest[a..a + b].trim().to_string())
}

impl ClapLib {
    /// Load (or reuse) the binary at `path` and initialise its entry.
    pub fn load(path: &Path) -> Result<Arc<ClapLib>, String> {
        let mut map = libs().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(l) = map.get(path).and_then(|w| w.upgrade()) {
            return Ok(l);
        }
        let lib = unsafe { libloading::Library::new(binary_path(path)) }
            .map_err(|e| format!("load: {e}"))?;
        let entry = unsafe {
            let s: libloading::Symbol<*const clap_plugin_entry> = lib
                .get(c"clap_entry")
                .map_err(|_| "not a CLAP plugin (no clap_entry)".to_string())?;
            *s
        };
        if entry.is_null() {
            return Err("clap_entry is null".into());
        }
        let cpath = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        unsafe {
            let e = &*entry;
            if e.clap_version.major < 1 {
                return Err("CLAP version 0.x is not supported".into());
            }
            let ok = e.init.map(|f| f(cpath.as_ptr())).unwrap_or(false);
            if !ok {
                return Err("plugin entry init failed".into());
            }
            let f = e
                .get_factory
                .map(|g| g(CLAP_PLUGIN_FACTORY_ID.as_ptr()))
                .unwrap_or(std::ptr::null());
            if f.is_null() {
                if let Some(d) = e.deinit {
                    d();
                }
                return Err("no plugin factory".into());
            }
            let l = Arc::new(ClapLib {
                _lib: lib,
                entry,
                factory: f as *const clap_plugin_factory,
            });
            map.insert(path.to_path_buf(), Arc::downgrade(&l));
            Ok(l)
        }
    }

    pub fn descriptors(&self) -> Vec<Desc> {
        let mut v = vec![];
        unsafe {
            let f = &*self.factory;
            let n = f.get_plugin_count.map(|c| c(self.factory)).unwrap_or(0);
            for i in 0..n {
                let Some(gd) = f.get_plugin_descriptor else {
                    break;
                };
                let d = gd(self.factory, i);
                if d.is_null() {
                    continue;
                }
                let d = &*d;
                let mut features = vec![];
                if !d.features.is_null() {
                    let mut p = d.features;
                    while !(*p).is_null() {
                        features.push(cstr(*p));
                        p = p.add(1);
                    }
                }
                v.push(Desc {
                    id: cstr(d.id),
                    name: cstr(d.name),
                    vendor: cstr(d.vendor),
                    version: cstr(d.version),
                    features,
                });
            }
        }
        v
    }
}

// ------------------------------------------------------------------ host side

/// flags the plugin raises from any thread; the UI polls them
#[derive(Default)]
pub struct HostFlags {
    pub callback: AtomicBool,
    pub restart: AtomicBool,
    pub dirty: AtomicBool,
    pub gui_closed: AtomicBool,
    pub resize: AtomicU32,
    pub rescan: AtomicBool,
}

#[repr(C)]
struct HostCtx {
    host: clap_host,
    flags: Arc<HostFlags>,
}

unsafe fn flags<'a>(h: *const clap_host) -> &'a HostFlags {
    &(*((*h).host_data as *const HostCtx)).flags
}

unsafe extern "C" fn h_get_ext(_h: *const clap_host, id: *const c_char) -> *const c_void {
    let id = CStr::from_ptr(id);
    if id == CLAP_EXT_THREAD_CHECK {
        &HOST_THREAD_CHECK as *const _ as *const c_void
    } else if id == CLAP_EXT_LOG {
        &HOST_LOG as *const _ as *const c_void
    } else if id == CLAP_EXT_PARAMS {
        &HOST_PARAMS as *const _ as *const c_void
    } else if id == CLAP_EXT_STATE {
        &HOST_STATE as *const _ as *const c_void
    } else if id == CLAP_EXT_GUI {
        &HOST_GUI as *const _ as *const c_void
    } else if id == CLAP_EXT_NOTE_PORTS {
        &HOST_NOTE_PORTS as *const _ as *const c_void
    } else if id == CLAP_EXT_AUDIO_PORTS {
        &HOST_AUDIO_PORTS as *const _ as *const c_void
    } else {
        std::ptr::null()
    }
}
unsafe extern "C" fn h_restart(h: *const clap_host) {
    flags(h).restart.store(true, Ordering::Release);
}
unsafe extern "C" fn h_process(_h: *const clap_host) {}
unsafe extern "C" fn h_callback(h: *const clap_host) {
    flags(h).callback.store(true, Ordering::Release);
}

unsafe extern "C" fn tc_main(_h: *const clap_host) -> bool {
    !AUDIO_THREAD.with(|a| a.get())
}
unsafe extern "C" fn tc_audio(_h: *const clap_host) -> bool {
    AUDIO_THREAD.with(|a| a.get())
}
static HOST_THREAD_CHECK: clap_host_thread_check = clap_host_thread_check {
    is_main_thread: Some(tc_main),
    is_audio_thread: Some(tc_audio),
};

unsafe extern "C" fn h_log(_h: *const clap_host, sev: clap_log_severity, msg: *const c_char) {
    // never print from the audio thread (allocation / blocking)
    if sev >= CLAP_LOG_ERROR && !AUDIO_THREAD.with(|a| a.get()) {
        eprintln!("plugin: {}", cstr(msg));
    }
}
static HOST_LOG: clap_host_log = clap_host_log { log: Some(h_log) };

unsafe extern "C" fn hp_rescan(h: *const clap_host, _f: clap_param_rescan_flags) {
    flags(h).rescan.store(true, Ordering::Release);
}
unsafe extern "C" fn hp_clear(_h: *const clap_host, _id: u32, _f: clap_param_clear_flags) {}
unsafe extern "C" fn hp_flush(_h: *const clap_host) {}
static HOST_PARAMS: clap_host_params = clap_host_params {
    rescan: Some(hp_rescan),
    clear: Some(hp_clear),
    request_flush: Some(hp_flush),
};

unsafe extern "C" fn hs_dirty(h: *const clap_host) {
    flags(h).dirty.store(true, Ordering::Release);
}
static HOST_STATE: clap_host_state = clap_host_state {
    mark_dirty: Some(hs_dirty),
};

unsafe extern "C" fn hg_hints(_h: *const clap_host) {}
unsafe extern "C" fn hg_resize(h: *const clap_host, w: u32, hh: u32) -> bool {
    let v = (w.min(0xFFFF) << 16) | hh.min(0xFFFF);
    flags(h).resize.store(v, Ordering::Release);
    true
}
unsafe extern "C" fn hg_show(_h: *const clap_host) -> bool {
    false
}
unsafe extern "C" fn hg_hide(_h: *const clap_host) -> bool {
    false
}
unsafe extern "C" fn hg_closed(h: *const clap_host, _destroyed: bool) {
    flags(h).gui_closed.store(true, Ordering::Release);
}
static HOST_GUI: clap_host_gui = clap_host_gui {
    resize_hints_changed: Some(hg_hints),
    request_resize: Some(hg_resize),
    request_show: Some(hg_show),
    request_hide: Some(hg_hide),
    closed: Some(hg_closed),
};

unsafe extern "C" fn hn_dialects(_h: *const clap_host) -> clap_note_dialect {
    CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI
}
unsafe extern "C" fn hn_rescan(_h: *const clap_host, _f: u32) {}
static HOST_NOTE_PORTS: clap_host_note_ports = clap_host_note_ports {
    supported_dialects: Some(hn_dialects),
    rescan: Some(hn_rescan),
};
unsafe extern "C" fn ha_flag(_h: *const clap_host, _f: u32) -> bool {
    false
}
unsafe extern "C" fn ha_rescan(_h: *const clap_host, _f: u32) {}
static HOST_AUDIO_PORTS: clap_host_audio_ports = clap_host_audio_ports {
    is_rescan_flag_supported: Some(ha_flag),
    rescan: Some(ha_rescan),
};

// ------------------------------------------------------------------ events

#[repr(C)]
#[derive(Clone, Copy)]
union Ev {
    h: clap_event_header,
    note: clap_event_note,
    expr: clap_event_note_expression,
    param: clap_event_param_value,
    midi: clap_event_midi,
}

const MAX_EVENTS: usize = 512;

struct EvList {
    v: Vec<Ev>,
}

unsafe extern "C" fn ev_size(l: *const clap_input_events) -> u32 {
    (*((*l).ctx as *const EvList)).v.len() as u32
}
unsafe extern "C" fn ev_get(l: *const clap_input_events, i: u32) -> *const clap_event_header {
    let list = &*((*l).ctx as *const EvList);
    match list.v.get(i as usize) {
        Some(e) => &e.h,
        None => std::ptr::null(),
    }
}
unsafe extern "C" fn ev_push(_l: *const clap_output_events, _e: *const clap_event_header) -> bool {
    true
}

fn header(size: usize, ty: u16) -> clap_event_header {
    clap_event_header {
        size: size as u32,
        time: 0,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_: ty,
        flags: 0,
    }
}

// ------------------------------------------------------------------ instance

pub struct ClapInst {
    _lib: Arc<ClapLib>,
    _host: Box<HostCtx>,
    pub flags: Arc<HostFlags>,
    plugin: *const clap_plugin,
    params: *const clap_plugin_params,
    state: *const clap_plugin_state,
    gui: *const clap_plugin_gui,
    midi_notes: bool,
    sr: f32,
    active: bool,
    processing: bool,
    sleeping: bool,
    steady: i64,
    ev: Box<EvList>,
    in_list: clap_input_events,
    out_list: clap_output_events,
    /// scratch audio: [port][channel][frame]; outputs[0] is the main output
    in_data: Vec<Vec<Vec<f32>>>,
    out_data: Vec<Vec<Vec<f32>>>,
    in_ptrs: Vec<Vec<*mut f32>>,
    out_ptrs: Vec<Vec<*mut f32>>,
    in_bufs: Vec<clap_audio_buffer>,
    out_bufs: Vec<clap_audio_buffer>,
    next_note_id: i32,
    pub gui_win: Option<super::gui::Window>,
    pub desc: Desc,
}
unsafe impl Send for ClapInst {}

impl ClapInst {
    /// Create, initialise and activate `id` from `path` at `sr`.
    pub fn new(path: &Path, id: &str, sr: f32) -> Result<ClapInst, String> {
        let lib = ClapLib::load(path)?;
        let desc = lib
            .descriptors()
            .into_iter()
            .find(|d| d.id == id)
            .ok_or_else(|| format!("{} does not contain plugin {id}", path.display()))?;
        let flags = Arc::new(HostFlags::default());
        let mut host = Box::new(HostCtx {
            host: clap_host {
                clap_version: CLAP_VERSION,
                host_data: std::ptr::null_mut(),
                name: c"ChipWave Studio".as_ptr(),
                vendor: c"ChipWave".as_ptr(),
                url: c"https://github.com/postmanpat-303/ChipWaveStudio".as_ptr(),
                version: c"0.2.3.16".as_ptr(),
                get_extension: Some(h_get_ext),
                request_restart: Some(h_restart),
                request_process: Some(h_process),
                request_callback: Some(h_callback),
            },
            flags: flags.clone(),
        });
        host.host.host_data = &mut *host as *mut HostCtx as *mut c_void;
        let cid = CString::new(id).map_err(|e| e.to_string())?;
        let plugin = unsafe {
            let f = &*lib.factory;
            let create = f.create_plugin.ok_or("factory cannot create plugins")?;
            create(lib.factory, &host.host, cid.as_ptr())
        };
        if plugin.is_null() {
            return Err(format!("could not create {id}"));
        }
        let ext = |name: &CStr| unsafe {
            (*plugin)
                .get_extension
                .map(|g| g(plugin, name.as_ptr()))
                .unwrap_or(std::ptr::null())
        };
        unsafe {
            if !(*plugin).init.map(|f| f(plugin)).unwrap_or(false) {
                if let Some(d) = (*plugin).destroy {
                    d(plugin);
                }
                return Err(format!("{id}: init failed"));
            }
        }
        let params = ext(CLAP_EXT_PARAMS) as *const clap_plugin_params;
        let state = ext(CLAP_EXT_STATE) as *const clap_plugin_state;
        let gui = ext(CLAP_EXT_GUI) as *const clap_plugin_gui;
        // note dialect: CLAP notes if the first input port speaks them, else MIDI
        let np = ext(CLAP_EXT_NOTE_PORTS) as *const clap_plugin_note_ports;
        let midi_notes = unsafe {
            if np.is_null() {
                false
            } else {
                let mut info: clap_note_port_info = std::mem::zeroed();
                let n = (*np).count.map(|c| c(plugin, true)).unwrap_or(0);
                n > 0
                    && (*np)
                        .get
                        .map(|g| g(plugin, 0, true, &mut info))
                        .unwrap_or(false)
                    && info.supported_dialects & CLAP_NOTE_DIALECT_CLAP == 0
                    && info.supported_dialects & CLAP_NOTE_DIALECT_MIDI != 0
            }
        };
        // audio ports (default: no inputs, one stereo output)
        let ap = ext(CLAP_EXT_AUDIO_PORTS) as *const clap_plugin_audio_ports;
        let ports = |input: bool| -> Vec<u32> {
            if ap.is_null() {
                return if input { vec![] } else { vec![2] };
            }
            unsafe {
                let n = (*ap).count.map(|c| c(plugin, input)).unwrap_or(0);
                (0..n)
                    .map(|i| {
                        let mut info: clap_audio_port_info = std::mem::zeroed();
                        if (*ap)
                            .get
                            .map(|g| g(plugin, i, input, &mut info))
                            .unwrap_or(false)
                        {
                            info.channel_count.clamp(1, 8)
                        } else {
                            2
                        }
                    })
                    .collect()
            }
        };
        let (ins, outs) = (ports(true), ports(false));
        if outs.is_empty() {
            unsafe {
                if let Some(d) = (*plugin).destroy {
                    d(plugin);
                }
            }
            return Err(format!("{id} has no audio output"));
        }
        let mk = |p: &[u32]| -> Vec<Vec<Vec<f32>>> {
            p.iter()
                .map(|&c| (0..c).map(|_| vec![0.0f32; MAX_BLOCK]).collect())
                .collect()
        };
        let mut ev = Box::new(EvList {
            v: Vec::with_capacity(MAX_EVENTS),
        });
        let ctx = &mut *ev as *mut EvList as *mut c_void;
        let mut me = ClapInst {
            _lib: lib,
            _host: host,
            flags,
            plugin,
            params,
            state,
            gui,
            midi_notes,
            sr,
            active: false,
            processing: false,
            sleeping: false,
            steady: 0,
            ev,
            in_list: clap_input_events {
                ctx,
                size: Some(ev_size),
                get: Some(ev_get),
            },
            out_list: clap_output_events {
                ctx: std::ptr::null_mut(),
                try_push: Some(ev_push),
            },
            in_data: mk(&ins),
            out_data: mk(&outs),
            in_ptrs: vec![],
            out_ptrs: vec![],
            in_bufs: vec![],
            out_bufs: vec![],
            next_note_id: 0,
            gui_win: None,
            desc,
        };
        // pointer tables (the Vecs above never move or grow after this)
        me.in_ptrs = me
            .in_data
            .iter_mut()
            .map(|p| p.iter_mut().map(|c| c.as_mut_ptr()).collect())
            .collect();
        me.out_ptrs = me
            .out_data
            .iter_mut()
            .map(|p| p.iter_mut().map(|c| c.as_mut_ptr()).collect())
            .collect();
        let buf = |ptrs: &mut Vec<*mut f32>| clap_audio_buffer {
            data32: ptrs.as_mut_ptr(),
            data64: std::ptr::null_mut(),
            channel_count: ptrs.len() as u32,
            latency: 0,
            constant_mask: 0,
        };
        me.in_bufs = me.in_ptrs.iter_mut().map(buf).collect();
        me.out_bufs = me.out_ptrs.iter_mut().map(buf).collect();
        me.activate()?;
        Ok(me)
    }

    fn activate(&mut self) -> Result<(), String> {
        let ok = unsafe {
            (*self.plugin)
                .activate
                .map(|a| a(self.plugin, self.sr as f64, 1, MAX_BLOCK as u32))
                .unwrap_or(false)
        };
        if !ok {
            return Err(format!("{}: activate failed", self.desc.name));
        }
        self.active = true;
        self.sleeping = false;
        Ok(())
    }

    fn deactivate(&mut self) {
        unsafe {
            if self.processing {
                // stop_processing is [audio-thread]: no audio callback runs
                // now (the engine lock is held), so act as the audio thread
                if let Some(s) = (*self.plugin).stop_processing {
                    let was = AUDIO_THREAD.with(|a| a.replace(true));
                    s(self.plugin);
                    AUDIO_THREAD.with(|a| a.set(was));
                }
                self.processing = false;
            }
            if self.active {
                if let Some(d) = (*self.plugin).deactivate {
                    d(self.plugin);
                }
                self.active = false;
            }
        }
    }

    fn push(&mut self, e: Ev) {
        if self.ev.v.len() < MAX_EVENTS {
            self.ev.v.push(e);
        }
    }
}

impl Drop for ClapInst {
    fn drop(&mut self) {
        self.close_editor();
        self.deactivate();
        unsafe {
            if let Some(d) = (*self.plugin).destroy {
                d(self.plugin);
            }
        }
    }
}

impl Hosted for ClapInst {
    fn note_on(&mut self, key: u8, vel: f32) {
        let key = key.min(127);
        let e = if self.midi_notes {
            Ev {
                midi: clap_event_midi {
                    header: header(std::mem::size_of::<clap_event_midi>(), CLAP_EVENT_MIDI),
                    port_index: 0,
                    data: [
                        0x90,
                        key,
                        ((vel.clamp(0.0, 1.0) * 127.0).round() as u8).max(1),
                    ],
                },
            }
        } else {
            self.next_note_id = self.next_note_id.wrapping_add(1) & 0x7FFF_FFFF;
            Ev {
                note: clap_event_note {
                    header: header(std::mem::size_of::<clap_event_note>(), CLAP_EVENT_NOTE_ON),
                    note_id: self.next_note_id,
                    port_index: 0,
                    channel: 0,
                    key: key as i16,
                    velocity: vel.clamp(0.0, 1.0) as f64,
                },
            }
        };
        self.push(e);
    }

    fn note_off(&mut self, key: u8) {
        let key = key.min(127);
        let e = if self.midi_notes {
            Ev {
                midi: clap_event_midi {
                    header: header(std::mem::size_of::<clap_event_midi>(), CLAP_EVENT_MIDI),
                    port_index: 0,
                    data: [0x80, key, 0],
                },
            }
        } else {
            Ev {
                note: clap_event_note {
                    header: header(std::mem::size_of::<clap_event_note>(), CLAP_EVENT_NOTE_OFF),
                    note_id: -1,
                    port_index: 0,
                    channel: 0,
                    key: key as i16,
                    velocity: 0.0,
                },
            }
        };
        self.push(e);
    }

    fn tuning(&mut self, key: u8, semis: f32) {
        if self.midi_notes {
            return;
        }
        self.push(Ev {
            expr: clap_event_note_expression {
                header: header(
                    std::mem::size_of::<clap_event_note_expression>(),
                    CLAP_EVENT_NOTE_EXPRESSION,
                ),
                expression_id: CLAP_NOTE_EXPRESSION_TUNING,
                note_id: -1,
                port_index: 0,
                channel: 0,
                key: key as i16,
                value: semis as f64,
            },
        });
    }

    fn param(&mut self, id: u32, value: f64) {
        self.push(Ev {
            param: clap_event_param_value {
                header: header(
                    std::mem::size_of::<clap_event_param_value>(),
                    CLAP_EVENT_PARAM_VALUE,
                ),
                param_id: id,
                cookie: std::ptr::null_mut(),
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value,
            },
        });
    }

    fn panic(&mut self) {
        // drop queued events, then all sound off: CC 120 + 123 (MIDI dialect)
        // or a wildcard note choke (CLAP dialect), and reset the plugin
        self.ev.v.clear();
        if self.midi_notes {
            for cc in [120u8, 123, 64] {
                self.push(Ev {
                    midi: clap_event_midi {
                        header: header(std::mem::size_of::<clap_event_midi>(), CLAP_EVENT_MIDI),
                        port_index: 0,
                        data: [0xB0, cc, 0],
                    },
                });
            }
        } else {
            self.push(Ev {
                note: clap_event_note {
                    header: header(
                        std::mem::size_of::<clap_event_note>(),
                        CLAP_EVENT_NOTE_CHOKE,
                    ),
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    velocity: 0.0,
                },
            });
        }
        if self.active && self.processing {
            // clap_plugin.reset is [audio-thread]: clears tails and voices
            unsafe {
                if let Some(r) = (*self.plugin).reset {
                    r(self.plugin);
                }
            }
        }
        self.sleeping = false;
    }

    fn process(&mut self, n: usize, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = n.min(MAX_BLOCK).min(out_l.len()).min(out_r.len());
        if !self.active {
            out_l[..n].fill(0.0);
            out_r[..n].fill(0.0);
            self.ev.v.clear();
            return;
        }
        if self.sleeping && self.ev.v.is_empty() {
            out_l[..n].fill(0.0);
            out_r[..n].fill(0.0);
            return;
        }
        unsafe {
            if !self.processing {
                self.processing = (*self.plugin)
                    .start_processing
                    .map(|s| s(self.plugin))
                    .unwrap_or(true);
                if !self.processing {
                    out_l[..n].fill(0.0);
                    out_r[..n].fill(0.0);
                    return;
                }
            }
            let pr = clap_process {
                steady_time: self.steady,
                frames_count: n as u32,
                transport: std::ptr::null(),
                audio_inputs: self.in_bufs.as_ptr(),
                audio_outputs: self.out_bufs.as_mut_ptr(),
                audio_inputs_count: self.in_bufs.len() as u32,
                audio_outputs_count: self.out_bufs.len() as u32,
                in_events: &self.in_list,
                out_events: &self.out_list,
            };
            let st = (*self.plugin)
                .process
                .map(|p| p(self.plugin, &pr))
                .unwrap_or(CLAP_PROCESS_ERROR);
            self.steady += n as i64;
            self.ev.v.clear();
            self.sleeping = st == CLAP_PROCESS_SLEEP;
            let o = &self.out_data[0];
            let r = if o.len() > 1 { 1 } else { 0 };
            for i in 0..n {
                let (a, b) = (o[0][i], o[r][i]);
                out_l[i] = if a.is_finite() {
                    a.clamp(-8.0, 8.0)
                } else {
                    0.0
                };
                out_r[i] = if b.is_finite() {
                    b.clamp(-8.0, 8.0)
                } else {
                    0.0
                };
            }
            if st == CLAP_PROCESS_ERROR {
                out_l[..n].fill(0.0);
                out_r[..n].fill(0.0);
            }
        }
    }

    fn params(&self) -> Vec<ParamInfo> {
        let mut v = vec![];
        if self.params.is_null() {
            return v;
        }
        unsafe {
            let p = &*self.params;
            let n = p.count.map(|c| c(self.plugin)).unwrap_or(0);
            for i in 0..n {
                let mut info: clap_param_info = std::mem::zeroed();
                if !p
                    .get_info
                    .map(|g| g(self.plugin, i, &mut info))
                    .unwrap_or(false)
                {
                    continue;
                }
                if info.flags & CLAP_PARAM_IS_HIDDEN != 0 {
                    continue;
                }
                let module = cbuf(&info.module);
                v.push(ParamInfo {
                    id: info.id,
                    name: cbuf(&info.name),
                    module: module.trim_matches('/').replace("//", "/"),
                    min: info.min_value,
                    max: info.max_value,
                    default: info.default_value,
                    stepped: info.flags & CLAP_PARAM_IS_STEPPED != 0,
                    readonly: info.flags & CLAP_PARAM_IS_READONLY != 0,
                });
            }
        }
        v
    }

    fn value(&self, id: u32) -> Option<f64> {
        if self.params.is_null() {
            return None;
        }
        let mut out = 0.0;
        unsafe {
            (*self.params)
                .get_value
                .map(|g| g(self.plugin, id, &mut out))
                .unwrap_or(false)
                .then_some(out)
        }
    }

    fn value_text(&self, id: u32, value: f64) -> Option<String> {
        if self.params.is_null() {
            return None;
        }
        let mut buf = [0 as c_char; 128];
        unsafe {
            (*self.params)
                .value_to_text
                .map(|f| f(self.plugin, id, value, buf.as_mut_ptr(), buf.len() as u32))
                .unwrap_or(false)
                .then(|| cbuf(&buf))
        }
    }

    fn save_state(&mut self) -> Option<Vec<u8>> {
        if self.state.is_null() {
            return None;
        }
        unsafe extern "C" fn w(s: *const clap_ostream, b: *const c_void, n: u64) -> i64 {
            let v = &mut *((*s).ctx as *mut Vec<u8>);
            v.extend_from_slice(std::slice::from_raw_parts(b as *const u8, n as usize));
            n as i64
        }
        let mut data: Vec<u8> = vec![];
        let os = clap_ostream {
            ctx: &mut data as *mut Vec<u8> as *mut c_void,
            write: Some(w),
        };
        let ok = unsafe {
            (*self.state)
                .save
                .map(|s| s(self.plugin, &os))
                .unwrap_or(false)
        };
        ok.then_some(data)
    }

    fn load_state(&mut self, data: &[u8]) -> bool {
        if self.state.is_null() {
            return false;
        }
        struct Rd<'a> {
            d: &'a [u8],
            pos: usize,
        }
        unsafe extern "C" fn r(s: *const clap_istream, b: *mut c_void, n: u64) -> i64 {
            let rd = &mut *((*s).ctx as *mut Rd);
            let k = (n as usize).min(rd.d.len() - rd.pos);
            std::ptr::copy_nonoverlapping(rd.d.as_ptr().add(rd.pos), b as *mut u8, k);
            rd.pos += k;
            k as i64
        }
        let mut rd = Rd { d: data, pos: 0 };
        let is = clap_istream {
            ctx: &mut rd as *mut Rd as *mut c_void,
            read: Some(r),
        };
        unsafe {
            (*self.state)
                .load
                .map(|l| l(self.plugin, &is))
                .unwrap_or(false)
        }
    }

    fn set_param_main(&mut self, id: u32, value: f64) {
        // inactive / not processing: the params flush applies it at once
        if self.processing || self.params.is_null() {
            self.param(id, value);
            return;
        }
        self.param(id, value);
        unsafe {
            if let Some(f) = (*self.params).flush {
                // params.flush is [active ? audio-thread : main-thread]; the
                // engine lock keeps the real audio thread out meanwhile
                let was = AUDIO_THREAD.with(|a| a.replace(self.active));
                f(self.plugin, &self.in_list, &self.out_list);
                AUDIO_THREAD.with(|a| a.set(was));
            }
        }
        self.ev.v.clear();
    }

    fn has_editor(&self) -> bool {
        !self.gui.is_null() && super::gui::supported(self.plugin, self.gui)
    }

    fn open_editor(&mut self, title: &str) -> Result<(), String> {
        if self.gui_win.is_some() {
            if let Some(w) = &self.gui_win {
                w.raise();
            }
            return Ok(());
        }
        if self.gui.is_null() {
            return Err("this plugin has no editor; use the parameter list".into());
        }
        self.flags.gui_closed.store(false, Ordering::Release);
        let w = super::gui::open(self.plugin, self.gui, title)?;
        self.gui_win = Some(w);
        Ok(())
    }

    fn close_editor(&mut self) {
        if let Some(w) = self.gui_win.take() {
            super::gui::close(self.plugin, self.gui, w);
        }
    }

    fn editor_open(&self) -> bool {
        self.gui_win.is_some()
    }

    fn poll_main(&mut self) {
        if self.flags.callback.swap(false, Ordering::AcqRel) {
            unsafe {
                if let Some(f) = (*self.plugin).on_main_thread {
                    f(self.plugin);
                }
            }
        }
        if self.flags.restart.swap(false, Ordering::AcqRel) {
            self.deactivate();
            let _ = self.activate();
        }
        let rs = self.flags.resize.swap(0, Ordering::AcqRel);
        if rs != 0 {
            if let Some(w) = &self.gui_win {
                w.resize(rs >> 16, rs & 0xFFFF);
            }
        }
        let closed_by_user = self.gui_win.as_ref().is_some_and(|w| w.closed());
        if self.flags.gui_closed.swap(false, Ordering::AcqRel) || closed_by_user {
            self.close_editor();
        }
    }

    fn take_dirty(&self) -> bool {
        self.flags.dirty.swap(false, Ordering::AcqRel)
    }

    fn set_sample_rate(&mut self, sr: f32) {
        if (sr - self.sr).abs() > 0.5 {
            self.deactivate();
            self.sr = sr;
            let _ = self.activate();
        }
    }
}
