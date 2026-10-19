//! VST 3 host: loads a `.vst3` module, creates one plugin instance (the
//! processor `IComponent` + `IAudioProcessor` and its `IEditController`,
//! connected through `IConnectionPoint`), feeds it notes (`IEventList`) and
//! parameter changes (`IParameterChanges`), renders its stereo output
//! (allocation-free on the audio thread), saves and loads its state
//! (component + controller streams) and opens its editor (`IPlugView` in a
//! Win32 window on Windows). It implements the same [`Hosted`] trait as the
//! CLAP host, so instruments, FX chains, macros, automation and modulation
//! treat both formats alike.
//!
//! Written against the VST 3 API by Steinberg Media Technologies GmbH (MIT
//! licence since VST 3 SDK 3.8.0, https://github.com/steinbergmedia/vst3sdk)
//! through the `vst3` crate (Rust bindings of the SDK headers by Micah
//! Johnston, MIT OR Apache-2.0, https://github.com/coupler-rs/vst3-rs). No
//! SDK source is copied; the hosting order follows the SDK's documentation
//! ("VST 3 API documentation > Hosting") and its `EditorHost` /
//! `AudioHost` examples. VST is a registered trademark of Steinberg Media
//! Technologies GmbH.
//!
//! Parameters: VST3 parameters are always normalised (0..1), so
//! [`ParamInfo`] reports min 0, max 1 and `value` / `param` take normalised
//! values. Threading: like CLAP, every instance lives inside the engine
//! behind the audio mutex, so the UI thread and the audio thread never call
//! into one instance at the same time; edits made in the plugin's own editor
//! arrive through `IComponentHandler` on the UI thread and are handed to the
//! audio thread through a small preallocated queue (`try_lock`, no waiting).
#![allow(non_upper_case_globals, non_snake_case)]

use super::clap::{binary_path, Desc, HostFlags, AUDIO_THREAD};
use super::{Hosted, ParamInfo, MAX_BLOCK};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, Weak};
use vst3::Steinberg::Vst::*;
use vst3::Steinberg::*;
use vst3::{Class, ComPtr, ComRef, ComWrapper, Interface};

const MAX_EVENTS: usize = 512;
/// parameter queues per block (one per parameter that changes) and points
const MAX_QUEUES: usize = 256;
const MAX_POINTS: usize = 16;
/// parameter values waiting to be mirrored into the controller
const MAX_MIRROR: usize = 512;

// ------------------------------------------------------------------ ids

/// The canonical text form of a VST3 class id: 32 hex digits, the same on
/// every OS (the SDK's `FUID::toString`; Windows keeps TUIDs in COM GUID
/// byte order, other systems in plain order).
pub fn cid_string(t: &TUID) -> String {
    let b: [u8; 16] = t.map(|c| c as u8);
    let tail: String = b[8..].iter().map(|x| format!("{x:02X}")).collect();
    if cfg!(windows) {
        let d1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let d2 = u16::from_le_bytes([b[4], b[5]]);
        let d3 = u16::from_le_bytes([b[6], b[7]]);
        format!("{d1:08X}{d2:04X}{d3:04X}{tail}")
    } else {
        let head: String = b[..8].iter().map(|x| format!("{x:02X}")).collect();
        format!("{head}{tail}")
    }
}

/// inverse of [`cid_string`]
#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_cid(s: &str) -> Option<TUID> {
    let s = s.trim();
    if s.len() != 32 || !s.is_ascii() {
        return None;
    }
    let mut b = [0u8; 16];
    for (k, x) in b.iter_mut().enumerate() {
        *x = u8::from_str_radix(&s[2 * k..2 * k + 2], 16).ok()?;
    }
    if cfg!(windows) {
        b[..4].reverse();
        b[4..6].reverse();
        b[6..8].reverse();
    }
    Some(b.map(|x| x as c_char))
}

fn guid_eq(t: &TUID, g: &[u8; 16]) -> bool {
    t.iter().zip(g).all(|(a, b)| *a as u8 == *b)
}

fn cbuf(b: &[c_char]) -> String {
    let bytes: Vec<u8> = b
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn wbuf(b: &[TChar]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf16_lossy(&b[..n])
}

fn wcopy(src: &str, dst: &mut [TChar]) {
    let mut n = 0;
    for (d, s) in dst.iter_mut().zip(src.encode_utf16()) {
        *d = s;
        n += 1;
    }
    let n = n.min(dst.len().saturating_sub(1));
    if let Some(d) = dst.get_mut(n) {
        *d = 0;
    }
}

// ------------------------------------------------------------------ module

type EntryFn = unsafe extern "system" fn() -> bool;

/// one loaded `.vst3` module (entry called once, shared by instances)
pub struct Vst3Lib {
    factory: Option<ComPtr<IPluginFactory>>,
    exit: Option<EntryFn>,
    #[cfg(target_os = "macos")]
    bundle: *mut c_void,
    _lib: libloading::Library,
}
unsafe impl Send for Vst3Lib {}
unsafe impl Sync for Vst3Lib {}

impl Drop for Vst3Lib {
    fn drop(&mut self) {
        // the factory goes before the module's exit function, the library last
        self.factory = None;
        unsafe {
            if let Some(f) = self.exit {
                f();
            }
            #[cfg(target_os = "macos")]
            if !self.bundle.is_null() {
                mac::CFRelease(self.bundle);
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFURLCreateFromFileSystemRepresentation(
            alloc: *const c_void,
            buf: *const u8,
            len: isize,
            is_dir: u8,
        ) -> *mut c_void;
        pub fn CFBundleCreate(alloc: *const c_void, url: *mut c_void) -> *mut c_void;
        pub fn CFRelease(cf: *mut c_void);
    }
    /// a CFBundleRef for the bundle folder (null if it can't be made)
    pub fn bundle(path: &std::path::Path) -> *mut c_void {
        use std::os::unix::ffi::OsStrExt;
        let b = path.as_os_str().as_bytes();
        unsafe {
            let url = CFURLCreateFromFileSystemRepresentation(
                std::ptr::null(),
                b.as_ptr(),
                b.len() as isize,
                1,
            );
            if url.is_null() {
                return std::ptr::null_mut();
            }
            let bun = CFBundleCreate(std::ptr::null(), url);
            CFRelease(url);
            bun
        }
    }
}

fn libs() -> &'static Mutex<HashMap<PathBuf, Weak<Vst3Lib>>> {
    static L: std::sync::OnceLock<Mutex<HashMap<PathBuf, Weak<Vst3Lib>>>> =
        std::sync::OnceLock::new();
    L.get_or_init(Default::default)
}

/// one processor class of a module: its id and description
#[derive(Clone, Debug)]
pub struct ClassDesc {
    pub cid: TUID,
    pub desc: Desc,
}

/// "Instrument|Synth" -> CLAP-style features ("instrument", "synth"),
/// "Fx|Delay" -> ("audio-effect", "delay")
pub fn features(sub: &str) -> Vec<String> {
    let parts: Vec<String> = sub
        .split('|')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let mut f = vec![];
    match parts.first().map(String::as_str) {
        Some("instrument") => f.push("instrument".to_string()),
        Some("fx") | Some("spatial") | Some("analyzer") => f.push("audio-effect".to_string()),
        _ if parts.iter().any(|p| p == "fx") => f.push("audio-effect".to_string()),
        _ => {}
    }
    for p in parts {
        if p != "instrument" && p != "fx" && !f.contains(&p) {
            f.push(p);
        }
    }
    f
}

impl Vst3Lib {
    /// Load (or reuse) the module at `path` and call its entry function.
    pub fn load(path: &Path) -> Result<Arc<Vst3Lib>, String> {
        let mut map = libs().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(l) = map.get(path).and_then(|w| w.upgrade()) {
            return Ok(l);
        }
        let bin = binary_path(path);
        #[cfg(all(unix, not(target_os = "macos")))]
        let (lib, handle) = {
            use libloading::os::unix::{Library, RTLD_LAZY, RTLD_LOCAL};
            let l = unsafe { Library::open(Some(&bin), RTLD_LAZY | RTLD_LOCAL) }
                .map_err(|e| format!("load: {e}"))?;
            let h = l.into_raw();
            (
                libloading::Library::from(unsafe { Library::from_raw(h) }),
                h,
            )
        };
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        let lib = unsafe { libloading::Library::new(&bin) }.map_err(|e| format!("load: {e}"))?;
        let sym = |name: &[u8]| unsafe { lib.get::<EntryFn>(name).ok().map(|s| *s) };
        let get_factory = unsafe {
            lib.get::<unsafe extern "system" fn() -> *mut IPluginFactory>(b"GetPluginFactory\0")
                .map(|s| *s)
                .map_err(|_| "not a VST3 module (no GetPluginFactory)".to_string())?
        };
        #[cfg(target_os = "macos")]
        let bundle = mac::bundle(path);
        // the module entry: ModuleEntry (Linux), InitDll (Windows),
        // BundleEntry (macOS)
        let exit;
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let entry = unsafe {
                lib.get::<unsafe extern "system" fn(*mut c_void) -> bool>(b"ModuleEntry\0")
                    .map(|s| *s)
                    .map_err(|_| "not a VST3 module (no ModuleEntry)".to_string())?
            };
            if !unsafe { entry(handle) } {
                return Err("the module's ModuleEntry failed".into());
            }
            exit = sym(b"ModuleExit\0");
        }
        #[cfg(windows)]
        {
            if let Some(init) = sym(b"InitDll\0") {
                if !unsafe { init() } {
                    return Err("the module's InitDll failed".into());
                }
            }
            exit = sym(b"ExitDll\0");
        }
        #[cfg(target_os = "macos")]
        {
            let entry = unsafe {
                lib.get::<unsafe extern "system" fn(*mut c_void) -> bool>(b"BundleEntry\0")
                    .map(|s| *s)
                    .map_err(|_| "not a VST3 module (no BundleEntry)".to_string())?
            };
            if !unsafe { entry(bundle) } {
                return Err("the module's BundleEntry failed".into());
            }
            exit = sym(b"BundleExit\0");
        }
        let factory = unsafe { ComPtr::from_raw(get_factory()) };
        let Some(factory) = factory else {
            unsafe {
                if let Some(f) = exit {
                    f();
                }
            }
            return Err("the module has no plugin factory".into());
        };
        let l = Arc::new(Vst3Lib {
            factory: Some(factory),
            exit,
            #[cfg(target_os = "macos")]
            bundle,
            _lib: lib,
        });
        // the factory may want a host context (IPluginFactory3)
        if let Some(f3) = l.factory().cast::<IPluginFactory3>() {
            let host = ComWrapper::new(HostApp);
            if let Some(h) = host.to_com_ptr::<FUnknown>() {
                unsafe { f3.setHostContext(h.as_ptr()) };
            }
        }
        map.insert(path.to_path_buf(), Arc::downgrade(&l));
        Ok(l)
    }

    fn factory(&self) -> &ComPtr<IPluginFactory> {
        self.factory.as_ref().expect("factory")
    }

    /// the processor classes ("Audio Module Class") the module offers
    pub fn classes(&self) -> Vec<ClassDesc> {
        let f = self.factory();
        let f2 = f.cast::<IPluginFactory2>();
        let mut fi: PFactoryInfo = unsafe { std::mem::zeroed() };
        let fvendor = if unsafe { f.getFactoryInfo(&mut fi) } == kResultOk {
            cbuf(&fi.vendor)
        } else {
            String::new()
        };
        let n = unsafe { f.countClasses() }.max(0);
        let mut v = vec![];
        for i in 0..n {
            let mut ci: PClassInfo = unsafe { std::mem::zeroed() };
            if unsafe { f.getClassInfo(i, &mut ci) } != kResultOk {
                continue;
            }
            if cbuf(&ci.category) != "Audio Module Class" {
                continue;
            }
            let (mut vendor, mut version, mut sub) =
                (fvendor.clone(), String::new(), String::new());
            if let Some(f2) = &f2 {
                let mut c2: PClassInfo2 = unsafe { std::mem::zeroed() };
                if unsafe { f2.getClassInfo2(i, &mut c2) } == kResultOk {
                    if !cbuf(&c2.vendor).is_empty() {
                        vendor = cbuf(&c2.vendor);
                    }
                    version = cbuf(&c2.version);
                    sub = cbuf(&c2.subCategories);
                }
            }
            v.push(ClassDesc {
                cid: ci.cid,
                desc: Desc {
                    id: cid_string(&ci.cid),
                    name: cbuf(&ci.name),
                    vendor,
                    version,
                    features: features(&sub),
                },
            });
        }
        v
    }

    pub fn descriptors(&self) -> Vec<Desc> {
        self.classes().into_iter().map(|c| c.desc).collect()
    }
}

// ------------------------------------------------------------------ host objects

/// `IHostApplication`: the host context handed to every plugin object; it
/// also allocates the `IMessage` / `IAttributeList` objects plugins use to
/// talk between their processor and controller.
struct HostApp;

impl Class for HostApp {
    type Interfaces = (IHostApplication,);
}

impl IHostApplicationTrait for HostApp {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kInvalidArgument;
        }
        wcopy("ChipWave Studio", &mut *name);
        kResultOk
    }
    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        obj: *mut *mut c_void,
    ) -> tresult {
        if cid.is_null() || iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let unk = if guid_eq(&*cid, &IMessage::IID) {
            ComWrapper::new(Message::default()).to_com_ptr::<FUnknown>()
        } else if guid_eq(&*cid, &IAttributeList::IID) {
            ComWrapper::new(Attrs::default()).to_com_ptr::<FUnknown>()
        } else {
            None
        };
        match unk {
            Some(u) => {
                let p = u.as_ptr();
                ((*(*p).vtbl).queryInterface)(p, iid as *const TUID, obj)
            }
            None => {
                *obj = std::ptr::null_mut();
                kNoInterface
            }
        }
    }
}

enum Attr {
    Int(i64),
    Float(f64),
    Str(Vec<u16>),
    Bin(Vec<u8>),
}

#[derive(Default)]
struct Attrs(RefCell<HashMap<Vec<u8>, Attr>>);

impl Class for Attrs {
    type Interfaces = (IAttributeList,);
}

unsafe fn key(id: *const c_char) -> Option<Vec<u8>> {
    (!id.is_null()).then(|| CStr::from_ptr(id).to_bytes().to_vec())
}

impl IAttributeListTrait for Attrs {
    unsafe fn setInt(&self, id: *const c_char, v: i64) -> tresult {
        let Some(k) = key(id) else {
            return kInvalidArgument;
        };
        self.0.borrow_mut().insert(k, Attr::Int(v));
        kResultOk
    }
    unsafe fn getInt(&self, id: *const c_char, v: *mut i64) -> tresult {
        match key(id).and_then(|k| match self.0.borrow().get(&k) {
            Some(Attr::Int(x)) => Some(*x),
            _ => None,
        }) {
            Some(x) if !v.is_null() => {
                *v = x;
                kResultOk
            }
            _ => kResultFalse,
        }
    }
    unsafe fn setFloat(&self, id: *const c_char, v: f64) -> tresult {
        let Some(k) = key(id) else {
            return kInvalidArgument;
        };
        self.0.borrow_mut().insert(k, Attr::Float(v));
        kResultOk
    }
    unsafe fn getFloat(&self, id: *const c_char, v: *mut f64) -> tresult {
        match key(id).and_then(|k| match self.0.borrow().get(&k) {
            Some(Attr::Float(x)) => Some(*x),
            _ => None,
        }) {
            Some(x) if !v.is_null() => {
                *v = x;
                kResultOk
            }
            _ => kResultFalse,
        }
    }
    unsafe fn setString(&self, id: *const c_char, s: *const TChar) -> tresult {
        let Some(k) = key(id) else {
            return kInvalidArgument;
        };
        let mut v = vec![];
        if !s.is_null() {
            let mut p = s;
            while *p != 0 && v.len() < 1 << 20 {
                v.push(*p);
                p = p.add(1);
            }
        }
        self.0.borrow_mut().insert(k, Attr::Str(v));
        kResultOk
    }
    unsafe fn getString(&self, id: *const c_char, s: *mut TChar, size: u32) -> tresult {
        let m = self.0.borrow();
        let Some(Attr::Str(v)) = key(id).and_then(|k| m.get(&k)) else {
            return kResultFalse;
        };
        let cap = size as usize / 2;
        if s.is_null() || cap == 0 {
            return kInvalidArgument;
        }
        let n = v.len().min(cap - 1);
        std::ptr::copy_nonoverlapping(v.as_ptr(), s, n);
        *s.add(n) = 0;
        kResultOk
    }
    unsafe fn setBinary(&self, id: *const c_char, data: *const c_void, size: u32) -> tresult {
        let Some(k) = key(id) else {
            return kInvalidArgument;
        };
        let b = if data.is_null() {
            vec![]
        } else {
            std::slice::from_raw_parts(data as *const u8, size as usize).to_vec()
        };
        self.0.borrow_mut().insert(k, Attr::Bin(b));
        kResultOk
    }
    unsafe fn getBinary(
        &self,
        id: *const c_char,
        data: *mut *const c_void,
        size: *mut u32,
    ) -> tresult {
        let m = self.0.borrow();
        let Some(Attr::Bin(b)) = key(id).and_then(|k| m.get(&k)) else {
            return kResultFalse;
        };
        if data.is_null() || size.is_null() {
            return kInvalidArgument;
        }
        // the bytes stay where they are until the attribute is set again
        *data = b.as_ptr() as *const c_void;
        *size = b.len() as u32;
        kResultOk
    }
}

#[derive(Default)]
struct Message {
    id: RefCell<Option<CString>>,
    attrs: RefCell<Option<ComWrapper<Attrs>>>,
}

impl Class for Message {
    type Interfaces = (IMessage,);
}

impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> FIDString {
        self.id
            .borrow()
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null())
    }
    unsafe fn setMessageID(&self, id: FIDString) {
        *self.id.borrow_mut() = (!id.is_null()).then(|| CStr::from_ptr(id).to_owned());
    }
    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        let mut a = self.attrs.borrow_mut();
        let a = a.get_or_insert_with(|| ComWrapper::new(Attrs::default()));
        // not add-ref'd: the list lives as long as the message
        a.as_com_ref::<IAttributeList>()
            .map(|r| r.as_ptr())
            .unwrap_or(std::ptr::null_mut())
    }
}

/// `IBStream` over a byte vector (state save / load)
#[derive(Default)]
struct MemStream {
    data: RefCell<Vec<u8>>,
    pos: Cell<usize>,
}

impl MemStream {
    fn from(b: &[u8]) -> ComWrapper<MemStream> {
        ComWrapper::new(MemStream {
            data: RefCell::new(b.to_vec()),
            pos: Cell::new(0),
        })
    }
}

impl Class for MemStream {
    type Interfaces = (IBStream, ISizeableStream);
}

impl IBStreamTrait for MemStream {
    unsafe fn read(&self, buf: *mut c_void, n: i32, got: *mut i32) -> tresult {
        let d = self.data.borrow();
        let pos = self.pos.get().min(d.len());
        let k = (n.max(0) as usize).min(d.len() - pos);
        if k > 0 && !buf.is_null() {
            std::ptr::copy_nonoverlapping(d.as_ptr().add(pos), buf as *mut u8, k);
        }
        self.pos.set(pos + k);
        if !got.is_null() {
            *got = k as i32;
        }
        kResultOk
    }
    unsafe fn write(&self, buf: *mut c_void, n: i32, done: *mut i32) -> tresult {
        if buf.is_null() || n < 0 {
            return kInvalidArgument;
        }
        let mut d = self.data.borrow_mut();
        let pos = self.pos.get();
        let n = n as usize;
        if d.len() < pos + n {
            d.resize(pos + n, 0);
        }
        std::ptr::copy_nonoverlapping(buf as *const u8, d.as_mut_ptr().add(pos), n);
        self.pos.set(pos + n);
        if !done.is_null() {
            *done = n as i32;
        }
        kResultOk
    }
    unsafe fn seek(&self, pos: i64, mode: i32, result: *mut i64) -> tresult {
        let len = self.data.borrow().len() as i64;
        let base = match mode {
            m if m == IBStream_::IStreamSeekMode_::kIBSeekSet as i32 => 0,
            m if m == IBStream_::IStreamSeekMode_::kIBSeekCur as i32 => self.pos.get() as i64,
            m if m == IBStream_::IStreamSeekMode_::kIBSeekEnd as i32 => len,
            _ => return kInvalidArgument,
        };
        let p = (base + pos).max(0);
        self.pos.set(p as usize);
        if !result.is_null() {
            *result = p;
        }
        kResultOk
    }
    unsafe fn tell(&self, pos: *mut i64) -> tresult {
        if pos.is_null() {
            return kInvalidArgument;
        }
        *pos = self.pos.get() as i64;
        kResultOk
    }
}

impl ISizeableStreamTrait for MemStream {
    unsafe fn getStreamSize(&self, size: *mut i64) -> tresult {
        if size.is_null() {
            return kInvalidArgument;
        }
        *size = self.data.borrow().len() as i64;
        kResultOk
    }
    unsafe fn setStreamSize(&self, size: i64) -> tresult {
        self.data.borrow_mut().resize(size.max(0) as usize, 0);
        kResultOk
    }
}

/// what the component handler (the plugin's editor) hands the host
pub struct HandlerShared {
    /// edits waiting for the next process call: (id, normalised value)
    edits: Mutex<Vec<(u32, f64)>>,
    /// `RestartFlags` raised since the last poll
    restart: AtomicI32,
    pub flags: Arc<HostFlags>,
}

impl HandlerShared {
    /// an edit from the plugin's editor (performEdit)
    pub fn edit(&self, id: u32, v: f64) {
        if let Ok(mut e) = self.edits.lock() {
            if let Some(x) = e.iter_mut().find(|x| x.0 == id) {
                x.1 = v;
            } else if e.len() < e.capacity() {
                e.push((id, v));
            }
        }
        self.flags.dirty.store(true, Ordering::Release);
    }
    /// the plugin asked for a restart (`IComponentHandler::restartComponent`)
    pub fn restart(&self, flags: i32) {
        self.restart.fetch_or(flags, Ordering::AcqRel);
        self.flags.dirty.store(true, Ordering::Release);
    }
}

struct Handler(Arc<HandlerShared>);

impl Class for Handler {
    type Interfaces = (IComponentHandler, IComponentHandler2);
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _id: u32) -> tresult {
        kResultOk
    }
    unsafe fn performEdit(&self, id: u32, v: f64) -> tresult {
        self.0.edit(id, v.clamp(0.0, 1.0));
        kResultOk
    }
    unsafe fn endEdit(&self, _id: u32) -> tresult {
        self.0.flags.dirty.store(true, Ordering::Release);
        kResultOk
    }
    unsafe fn restartComponent(&self, flags: i32) -> tresult {
        self.0.restart(flags);
        kResultOk
    }
}

impl IComponentHandler2Trait for Handler {
    unsafe fn setDirty(&self, state: TBool) -> tresult {
        if state != 0 {
            self.0.flags.dirty.store(true, Ordering::Release);
        }
        kResultOk
    }
    unsafe fn requestOpenEditor(&self, _name: FIDString) -> tresult {
        kNotImplemented
    }
    unsafe fn startGroupEdit(&self) -> tresult {
        kResultOk
    }
    unsafe fn finishGroupEdit(&self) -> tresult {
        kResultOk
    }
}

/// `IEventList` with a fixed capacity (no allocation while playing)
struct EvList(UnsafeCell<Vec<Event>>);

impl EvList {
    fn new() -> EvList {
        EvList(UnsafeCell::new(Vec::with_capacity(MAX_EVENTS)))
    }
    #[allow(clippy::mut_from_ref)]
    fn v(&self) -> &mut Vec<Event> {
        unsafe { &mut *self.0.get() }
    }
    fn push(&self, e: Event) {
        let v = self.v();
        if v.len() < v.capacity() {
            v.push(e);
        }
    }
}

impl Class for EvList {
    type Interfaces = (IEventList,);
}

impl IEventListTrait for EvList {
    unsafe fn getEventCount(&self) -> i32 {
        self.v().len() as i32
    }
    unsafe fn getEvent(&self, index: i32, e: *mut Event) -> tresult {
        match self.v().get(index.max(0) as usize) {
            Some(x) if !e.is_null() && index >= 0 => {
                *e = *x;
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }
    unsafe fn addEvent(&self, e: *mut Event) -> tresult {
        if e.is_null() {
            return kInvalidArgument;
        }
        let v = self.v();
        if v.len() < v.capacity() {
            v.push(*e);
            kResultOk
        } else {
            kOutOfMemory
        }
    }
}

/// one parameter's points in a block
struct Queue {
    id: Cell<u32>,
    n: Cell<usize>,
    pts: UnsafeCell<[(i32, f64); MAX_POINTS]>,
}

impl Queue {
    fn add(&self, off: i32, v: f64) -> i32 {
        let pts = unsafe { &mut *self.pts.get() };
        let n = self.n.get();
        if n > 0 && pts[n - 1].0 >= off {
            pts[n - 1].1 = v;
            return n as i32 - 1;
        }
        if n < MAX_POINTS {
            pts[n] = (off, v);
            self.n.set(n + 1);
            n as i32
        } else {
            pts[n - 1] = (off, v);
            n as i32 - 1
        }
    }
    fn last(&self) -> Option<f64> {
        let n = self.n.get();
        (n > 0).then(|| unsafe { (*self.pts.get())[n - 1].1 })
    }
}

impl Class for Queue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for Queue {
    unsafe fn getParameterId(&self) -> u32 {
        self.id.get()
    }
    unsafe fn getPointCount(&self) -> i32 {
        self.n.get() as i32
    }
    unsafe fn getPoint(&self, index: i32, off: *mut i32, v: *mut f64) -> tresult {
        if index < 0 || index as usize >= self.n.get() || off.is_null() || v.is_null() {
            return kInvalidArgument;
        }
        let p = (*self.pts.get())[index as usize];
        *off = p.0;
        *v = p.1;
        kResultOk
    }
    unsafe fn addPoint(&self, off: i32, v: f64, index: *mut i32) -> tresult {
        let k = self.add(off, v);
        if !index.is_null() {
            *index = k;
        }
        kResultOk
    }
}

/// `IParameterChanges` over preallocated queues
struct Changes {
    queues: Vec<ComWrapper<Queue>>,
    ptrs: Vec<*mut IParamValueQueue>,
    used: Cell<usize>,
}

impl Changes {
    fn new() -> Changes {
        let queues: Vec<ComWrapper<Queue>> = (0..MAX_QUEUES)
            .map(|_| {
                ComWrapper::new(Queue {
                    id: Cell::new(0),
                    n: Cell::new(0),
                    pts: UnsafeCell::new([(0, 0.0); MAX_POINTS]),
                })
            })
            .collect();
        let ptrs = queues
            .iter()
            .map(|q| {
                q.as_com_ref::<IParamValueQueue>()
                    .map(|r| r.as_ptr())
                    .unwrap_or(std::ptr::null_mut())
            })
            .collect();
        Changes {
            queues,
            ptrs,
            used: Cell::new(0),
        }
    }
    fn queue(&self, id: u32) -> Option<usize> {
        let n = self.used.get();
        if let Some(k) = (0..n).find(|&k| self.queues[k].id.get() == id) {
            return Some(k);
        }
        if n < self.queues.len() {
            self.queues[n].id.set(id);
            self.queues[n].n.set(0);
            self.used.set(n + 1);
            return Some(n);
        }
        None
    }
    fn push(&self, id: u32, off: i32, v: f64) {
        if let Some(k) = self.queue(id) {
            self.queues[k].add(off, v);
        }
    }
    fn clear(&self) {
        self.used.set(0);
    }
    fn is_empty(&self) -> bool {
        self.used.get() == 0
    }
}

impl Class for Changes {
    type Interfaces = (IParameterChanges,);
}

impl IParameterChangesTrait for Changes {
    unsafe fn getParameterCount(&self) -> i32 {
        self.used.get() as i32
    }
    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        if index < 0 || index as usize >= self.used.get() {
            return std::ptr::null_mut();
        }
        self.ptrs[index as usize]
    }
    unsafe fn addParameterData(&self, id: *const u32, index: *mut i32) -> *mut IParamValueQueue {
        if id.is_null() {
            return std::ptr::null_mut();
        }
        match self.queue(*id) {
            Some(k) => {
                if !index.is_null() {
                    *index = k as i32;
                }
                self.ptrs[k]
            }
            None => std::ptr::null_mut(),
        }
    }
}

// ------------------------------------------------------------------ instance

fn channels(a: SpeakerArrangement) -> usize {
    (a.count_ones() as usize).min(32)
}

pub struct Vst3Inst {
    comp: ComPtr<IComponent>,
    proc_: ComPtr<IAudioProcessor>,
    ctrl: Option<ComPtr<IEditController>>,
    /// the controller is a separate object (initialised / terminated by us)
    separate: bool,
    /// (component side, controller side) when both are connection points
    link: Option<(ComPtr<IConnectionPoint>, ComPtr<IConnectionPoint>)>,
    _host: ComWrapper<HostApp>,
    _handler: ComWrapper<Handler>,
    shared: Arc<HandlerShared>,
    pub flags: Arc<HostFlags>,
    sr: f32,
    active: bool,
    processing: bool,
    lat: u32,
    has_events: bool,
    ev: ComWrapper<EvList>,
    ev_out: ComWrapper<EvList>,
    pin: ComWrapper<Changes>,
    pout: ComWrapper<Changes>,
    ctx: Box<ProcessContext>,
    /// scratch audio: [bus][channel][frame]; outputs[0] is the main output
    in_data: Vec<Vec<Vec<f32>>>,
    out_data: Vec<Vec<Vec<f32>>>,
    in_ptrs: Vec<Vec<*mut f32>>,
    out_ptrs: Vec<Vec<*mut f32>>,
    in_bufs: Vec<AudioBusBuffers>,
    out_bufs: Vec<AudioBusBuffers>,
    /// note id per key (tuning expressions name the note)
    note_ids: [i32; 128],
    held: [bool; 128],
    next_note_id: i32,
    /// values the audio thread sent that the controller should show
    mirror: Vec<(u32, f64)>,
    infos: Vec<ParamInfo>,
    units: HashMap<u32, String>,
    /// editor support: 0 = not checked yet, 1 = no, 2 = yes
    editor: Cell<u8>,
    view: Option<ComPtr<IPlugView>>,
    #[cfg(windows)]
    frame: Option<ComWrapper<Frame>>,
    pub gui_win: Option<super::gui::Window>,
    pub desc: Desc,
    _lib: Arc<Vst3Lib>,
}
unsafe impl Send for Vst3Inst {}

/// undo what `Vst3Inst::new` set up so far when it fails half-way
fn teardown(
    comp: &ComPtr<IComponent>,
    ctrl: Option<&ComPtr<IEditController>>,
    separate: bool,
    link: Option<(ComPtr<IConnectionPoint>, ComPtr<IConnectionPoint>)>,
) {
    unsafe {
        if let Some((a, b)) = link {
            a.disconnect(b.as_ptr());
            b.disconnect(a.as_ptr());
        }
        if let Some(c) = ctrl {
            c.setComponentHandler(std::ptr::null_mut());
            if separate {
                c.terminate();
            }
        }
        comp.terminate();
    }
}

impl Vst3Inst {
    /// Create, initialise, connect and activate class `id` from `path` at `sr`.
    pub fn new(path: &Path, id: &str, sr: f32) -> Result<Vst3Inst, String> {
        let lib = Vst3Lib::load(path)?;
        let class = lib
            .classes()
            .into_iter()
            .find(|c| c.desc.id.eq_ignore_ascii_case(id))
            .ok_or_else(|| format!("{} does not contain plugin {id}", path.display()))?;
        let name = class.desc.name.clone();
        let host = ComWrapper::new(HostApp);
        let host_unk = host.to_com_ptr::<FUnknown>().ok_or("host context")?;
        let create = |cid: &TUID, iid: &[u8; 16]| -> Option<*mut c_void> {
            let mut obj: *mut c_void = std::ptr::null_mut();
            let r = unsafe {
                lib.factory()
                    .createInstance(cid.as_ptr(), iid.as_ptr() as *const c_char, &mut obj)
            };
            (r == kResultOk && !obj.is_null()).then_some(obj)
        };
        let comp = create(&class.cid, &IComponent::IID)
            .and_then(|p| unsafe { ComPtr::from_raw(p as *mut IComponent) })
            .ok_or_else(|| format!("could not create {name}"))?;
        if unsafe { comp.initialize(host_unk.as_ptr()) } != kResultOk {
            return Err(format!("{name}: initialize failed"));
        }
        let Some(proc_) = comp.cast::<IAudioProcessor>() else {
            teardown(&comp, None, false, None);
            return Err(format!("{name} is not an audio processor"));
        };
        if unsafe { proc_.canProcessSampleSize(SymbolicSampleSizes_::kSample32 as i32) }
            != kResultOk
        {
            teardown(&comp, None, false, None);
            return Err(format!("{name} can't process 32-bit audio"));
        }
        // the edit controller: the component itself, or a separate class
        let (ctrl, separate) = match comp.cast::<IEditController>() {
            Some(c) => (Some(c), false),
            None => {
                let mut cid: TUID = [0; 16];
                let c = (unsafe { comp.getControllerClassId(&mut cid) } == kResultOk)
                    .then(|| create(&cid, &IEditController::IID))
                    .flatten()
                    .and_then(|p| unsafe { ComPtr::from_raw(p as *mut IEditController) })
                    .filter(|c| unsafe { c.initialize(host_unk.as_ptr()) } == kResultOk);
                (c, true)
            }
        };
        let link = if separate {
            ctrl.as_ref().and_then(|c| {
                Some((
                    comp.cast::<IConnectionPoint>()?,
                    c.cast::<IConnectionPoint>()?,
                ))
            })
        } else {
            None
        };
        if let Some((a, b)) = &link {
            unsafe {
                a.connect(b.as_ptr());
                b.connect(a.as_ptr());
            }
        }
        let flags = Arc::new(HostFlags::default());
        let shared = Arc::new(HandlerShared {
            edits: Mutex::new(Vec::with_capacity(256)),
            restart: AtomicI32::new(0),
            flags: flags.clone(),
        });
        let handler = ComWrapper::new(Handler(shared.clone()));
        if let (Some(c), Some(h)) = (&ctrl, handler.to_com_ptr::<IComponentHandler>()) {
            unsafe { c.setComponentHandler(h.as_ptr()) };
        }
        // the controller starts from the processor's state
        if let (Some(c), true) = (&ctrl, separate) {
            let st = ComWrapper::new(MemStream::default());
            if let Some(sp) = st.to_com_ptr::<IBStream>() {
                if unsafe { comp.getState(sp.as_ptr()) } == kResultOk {
                    st.pos.set(0);
                    unsafe { c.setComponentState(sp.as_ptr()) };
                }
            }
        }
        // buses: the main input and output in stereo, the rest as the
        // plugin likes them
        let nb = |media: MediaTypes, dir: BusDirections| unsafe {
            comp.getBusCount(media as MediaType, dir as BusDirection)
                .max(0) as usize
        };
        let (ai, ao) = (
            nb(MediaTypes_::kAudio, BusDirections_::kInput),
            nb(MediaTypes_::kAudio, BusDirections_::kOutput),
        );
        let ei = nb(MediaTypes_::kEvent, BusDirections_::kInput);
        if ao == 0 {
            teardown(&comp, ctrl.as_ref(), separate, link);
            return Err(format!("{name} has no audio output"));
        }
        let arr = |dir: BusDirections, n: usize| -> Vec<SpeakerArrangement> {
            (0..n)
                .map(|i| {
                    let mut a: SpeakerArrangement = 0;
                    unsafe { proc_.getBusArrangement(dir as BusDirection, i as i32, &mut a) };
                    a
                })
                .collect()
        };
        let (mut ins, mut outs) = (
            arr(BusDirections_::kInput, ai),
            arr(BusDirections_::kOutput, ao),
        );
        if let Some(a) = ins.first_mut() {
            *a = SpeakerArr::kStereo;
        }
        outs[0] = SpeakerArr::kStereo;
        let ok = unsafe {
            proc_.setBusArrangements(
                if ai > 0 {
                    ins.as_mut_ptr()
                } else {
                    std::ptr::null_mut()
                },
                ai as i32,
                outs.as_mut_ptr(),
                ao as i32,
            )
        } == kResultOk;
        if !ok {
            // the plugin keeps its own layout (mono and so on)
            ins = arr(BusDirections_::kInput, ai);
            outs = arr(BusDirections_::kOutput, ao);
        }
        unsafe {
            let (audio, event) = (
                MediaTypes_::kAudio as MediaType,
                MediaTypes_::kEvent as MediaType,
            );
            let (input, output) = (
                BusDirections_::kInput as BusDirection,
                BusDirections_::kOutput as BusDirection,
            );
            if ai > 0 {
                comp.activateBus(audio, input, 0, 1);
            }
            comp.activateBus(audio, output, 0, 1);
            if ei > 0 {
                comp.activateBus(event, input, 0, 1);
            }
        }
        let mk = |a: &[SpeakerArrangement], main_min: usize| -> Vec<Vec<Vec<f32>>> {
            a.iter()
                .enumerate()
                .map(|(i, &x)| {
                    let c = if i == 0 {
                        channels(x).max(main_min)
                    } else {
                        channels(x)
                    };
                    (0..c).map(|_| vec![0.0f32; MAX_BLOCK]).collect()
                })
                .collect()
        };
        let mut ctx: Box<ProcessContext> = Box::new(unsafe { std::mem::zeroed() });
        ctx.sampleRate = sr as f64;
        ctx.tempo = 120.0;
        ctx.timeSigNumerator = 4;
        ctx.timeSigDenominator = 4;
        let mut me = Vst3Inst {
            comp,
            proc_,
            ctrl,
            separate,
            link,
            _host: host,
            _handler: handler,
            shared,
            flags,
            sr,
            active: false,
            processing: false,
            lat: 0,
            has_events: ei > 0,
            ev: ComWrapper::new(EvList::new()),
            ev_out: ComWrapper::new(EvList::new()),
            pin: ComWrapper::new(Changes::new()),
            pout: ComWrapper::new(Changes::new()),
            ctx,
            in_data: mk(&ins, 0),
            out_data: mk(&outs, 1),
            in_ptrs: vec![],
            out_ptrs: vec![],
            in_bufs: vec![],
            out_bufs: vec![],
            note_ids: [-1; 128],
            held: [false; 128],
            next_note_id: 0,
            mirror: Vec::with_capacity(MAX_MIRROR),
            infos: vec![],
            units: HashMap::new(),
            editor: Cell::new(0),
            view: None,
            #[cfg(windows)]
            frame: None,
            gui_win: None,
            desc: class.desc,
            _lib: lib,
        };
        // pointer tables (the Vecs above never move or grow after this)
        me.in_ptrs = me
            .in_data
            .iter_mut()
            .map(|b| b.iter_mut().map(|c| c.as_mut_ptr()).collect())
            .collect();
        me.out_ptrs = me
            .out_data
            .iter_mut()
            .map(|b| b.iter_mut().map(|c| c.as_mut_ptr()).collect())
            .collect();
        let buf = |p: &mut Vec<*mut f32>| AudioBusBuffers {
            numChannels: p.len() as i32,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: p.as_mut_ptr(),
            },
        };
        me.in_bufs = me.in_ptrs.iter_mut().map(buf).collect();
        me.out_bufs = me.out_ptrs.iter_mut().map(buf).collect();
        me.refresh_params();
        // on failure `me` drops here, which tears everything down
        me.activate()?;
        Ok(me)
    }

    fn activate(&mut self) -> Result<(), String> {
        let mut setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime as i32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            maxSamplesPerBlock: MAX_BLOCK as i32,
            sampleRate: self.sr as f64,
        };
        if unsafe { self.proc_.setupProcessing(&mut setup) } != kResultOk {
            return Err(format!("{}: setupProcessing failed", self.desc.name));
        }
        if unsafe { self.comp.setActive(1) } != kResultOk {
            return Err(format!("{}: activate failed", self.desc.name));
        }
        self.active = true;
        self.ctx.sampleRate = self.sr as f64;
        self.lat = unsafe { self.proc_.getLatencySamples() }.min(1 << 20);
        Ok(())
    }

    fn deactivate(&mut self) {
        unsafe {
            if self.processing {
                // setProcessing(false) belongs to the audio thread: no audio
                // callback runs now (the engine lock is held)
                let was = AUDIO_THREAD.with(|a| a.replace(true));
                self.proc_.setProcessing(0);
                AUDIO_THREAD.with(|a| a.set(was));
                self.processing = false;
            }
            if self.active {
                self.comp.setActive(0);
                self.active = false;
            }
        }
    }

    /// parameter list (titles, units, groups) from the controller
    fn refresh_params(&mut self) {
        self.infos.clear();
        self.units.clear();
        let Some(c) = &self.ctrl else { return };
        // unit (group) paths: "Osc 1", "Filter/Env"
        let mut unit_names: HashMap<i32, (i32, String)> = HashMap::new();
        if let Some(u) = c.cast::<IUnitInfo>() {
            for i in 0..unsafe { u.getUnitCount() }.clamp(0, 4096) {
                let mut ui: UnitInfo = unsafe { std::mem::zeroed() };
                if unsafe { u.getUnitInfo(i, &mut ui) } == kResultOk {
                    unit_names.insert(ui.id, (ui.parentUnitId, wbuf(&ui.name)));
                }
            }
        }
        let path = |mut id: i32| -> String {
            let mut parts = vec![];
            for _ in 0..8 {
                if id == 0 {
                    break; // the root unit
                }
                match unit_names.get(&id) {
                    Some((parent, n)) => {
                        parts.push(n.clone());
                        id = *parent;
                    }
                    None => break,
                }
            }
            parts.reverse();
            parts.join("/")
        };
        let n = unsafe { c.getParameterCount() }.clamp(0, 1 << 16);
        for i in 0..n {
            let mut pi: ParameterInfo = unsafe { std::mem::zeroed() };
            if unsafe { c.getParameterInfo(i, &mut pi) } != kResultOk {
                continue;
            }
            use ParameterInfo_::ParameterFlags_::*;
            if pi.flags & kIsHidden as i32 != 0 {
                continue;
            }
            let title = wbuf(&pi.title);
            let units = wbuf(&pi.units);
            if !units.is_empty() {
                self.units.insert(pi.id, units);
            }
            self.infos.push(ParamInfo {
                id: pi.id,
                name: if title.is_empty() {
                    wbuf(&pi.shortTitle)
                } else {
                    title
                },
                module: path(pi.unitId),
                min: 0.0,
                max: 1.0,
                default: pi.defaultNormalizedValue.clamp(0.0, 1.0),
                stepped: pi.stepCount > 0,
                readonly: pi.flags & kIsReadOnly as i32 != 0,
            });
        }
    }

    /// one process call; `loud` = the audio input is not silent
    fn run(&mut self, n: usize, out_l: &mut [f32], out_r: &mut [f32], loud: bool) {
        let n = n.min(MAX_BLOCK).min(out_l.len()).min(out_r.len());
        if !self.active {
            out_l[..n].fill(0.0);
            out_r[..n].fill(0.0);
            self.ev.v().clear();
            self.pin.clear();
            return;
        }
        unsafe {
            if !self.processing {
                // kNotImplemented is fine too
                self.proc_.setProcessing(1);
                self.processing = true;
            }
            // edits from the plugin's editor (never wait for the UI thread)
            if let Ok(mut e) = self.shared.edits.try_lock() {
                for (id, v) in e.drain(..) {
                    self.pin.push(id, 0, v);
                }
            }
            for b in self.out_data.iter_mut() {
                for c in b.iter_mut() {
                    c[..n].fill(0.0);
                }
            }
            for b in self.out_bufs.iter_mut() {
                b.silenceFlags = 0;
            }
            for (b, d) in self.in_bufs.iter_mut().zip(&self.in_data) {
                b.silenceFlags = if loud || d.is_empty() {
                    0
                } else {
                    (1u64 << d.len().min(63)) - 1
                };
            }
            let raw = |p: Option<ComRef<'_, IParameterChanges>>| {
                p.map(|r| r.as_ptr()).unwrap_or(std::ptr::null_mut())
            };
            let evp = |p: Option<ComRef<'_, IEventList>>| {
                p.map(|r| r.as_ptr()).unwrap_or(std::ptr::null_mut())
            };
            let mut data = ProcessData {
                processMode: ProcessModes_::kRealtime as i32,
                symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
                numSamples: n as i32,
                numInputs: self.in_bufs.len() as i32,
                numOutputs: self.out_bufs.len() as i32,
                inputs: if self.in_bufs.is_empty() {
                    std::ptr::null_mut()
                } else {
                    self.in_bufs.as_mut_ptr()
                },
                outputs: self.out_bufs.as_mut_ptr(),
                inputParameterChanges: raw(self.pin.as_com_ref()),
                outputParameterChanges: raw(self.pout.as_com_ref()),
                inputEvents: if self.has_events {
                    evp(self.ev.as_com_ref())
                } else {
                    std::ptr::null_mut()
                },
                outputEvents: evp(self.ev_out.as_com_ref()),
                processContext: &mut *self.ctx,
            };
            self.proc_.process(&mut data);
            self.ctx.projectTimeSamples += n as i64;
            self.ctx.continousTimeSamples += n as i64;
            self.ev.v().clear();
            self.ev_out.v().clear();
            self.pin.clear();
            // parameter changes the processor reports go to the controller
            for k in 0..self.pout.used.get() {
                let q = &self.pout.queues[k];
                if let Some(v) = q.last() {
                    Self::mirror_push(&mut self.mirror, q.id.get(), v);
                }
            }
            self.pout.clear();
            let o = &self.out_data[0];
            let silent = self.out_bufs[0].silenceFlags;
            let r = if o.len() > 1 { 1 } else { 0 };
            let clean = |x: f32| {
                if x.is_finite() {
                    x.clamp(-8.0, 8.0)
                } else {
                    0.0
                }
            };
            for i in 0..n {
                out_l[i] = if silent & 1 != 0 { 0.0 } else { clean(o[0][i]) };
                out_r[i] = if silent & (1 << r) != 0 {
                    0.0
                } else {
                    clean(o[r][i])
                };
            }
        }
    }

    fn mirror_push(m: &mut Vec<(u32, f64)>, id: u32, v: f64) {
        if let Some(x) = m.iter_mut().find(|x| x.0 == id) {
            x.1 = v;
        } else if m.len() < m.capacity() {
            m.push((id, v));
        }
    }

    /// apply queued parameter changes now: a process call without audio
    /// (the engine lock keeps the real audio thread out meanwhile)
    fn flush(&mut self) {
        if !self.active {
            return;
        }
        let pending = !self.pin.is_empty()
            || self
                .shared
                .edits
                .lock()
                .map(|e| !e.is_empty())
                .unwrap_or(false);
        if pending {
            let was = AUDIO_THREAD.with(|a| a.replace(true));
            self.run(0, &mut [], &mut [], false);
            AUDIO_THREAD.with(|a| a.set(was));
        }
    }

    fn event(&mut self, ty: u32, body: Event__type0) {
        if !self.has_events {
            return;
        }
        self.ev.push(Event {
            busIndex: 0,
            sampleOffset: 0,
            ppqPosition: 0.0,
            flags: 0,
            r#type: ty as u16,
            __field0: body,
        });
    }

    /// the component handler (tests drive it like an editor would)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn handler(&self) -> Arc<HandlerShared> {
        self.shared.clone()
    }

    fn mirror_to_controller(&mut self) {
        if let Some(c) = &self.ctrl {
            for &(id, v) in &self.mirror {
                unsafe { c.setParamNormalized(id, v) };
            }
        }
        self.mirror.clear();
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn create_view(&self) -> Option<ComPtr<IPlugView>> {
        let c = self.ctrl.as_ref()?;
        unsafe { ComPtr::from_raw(c.createView(ViewType::kEditor)) }
    }
}

impl Drop for Vst3Inst {
    fn drop(&mut self) {
        self.close_editor();
        self.deactivate();
        teardown(
            &self.comp,
            self.ctrl.as_ref(),
            self.separate,
            self.link.take(),
        );
        self.ctrl = None;
    }
}

/// "CWV3" <u32 len> component state <u32 len> controller state
fn split_state(d: &[u8]) -> Option<(&[u8], &[u8])> {
    if d.len() < 12 || &d[..4] != b"CWV3" {
        return None;
    }
    let a = u32::from_le_bytes(d[4..8].try_into().ok()?) as usize;
    let comp = d.get(8..8 + a)?;
    let rest = &d[8 + a..];
    let b = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
    let ctrl = rest.get(4..4 + b)?;
    Some((comp, ctrl))
}

impl Hosted for Vst3Inst {
    fn note_on(&mut self, key: u8, vel: f32) {
        let key = key.min(127);
        self.next_note_id = self.next_note_id.wrapping_add(1) & 0x7FFF_FFFF;
        self.note_ids[key as usize] = self.next_note_id;
        self.held[key as usize] = true;
        self.event(
            Event_::EventTypes_::kNoteOnEvent as u32,
            Event__type0 {
                noteOn: NoteOnEvent {
                    channel: 0,
                    pitch: key as i16,
                    tuning: 0.0,
                    velocity: vel.clamp(1.0 / 127.0, 1.0),
                    length: 0,
                    noteId: self.next_note_id,
                },
            },
        );
    }

    fn note_off(&mut self, key: u8) {
        let key = key.min(127);
        self.held[key as usize] = false;
        self.event(
            Event_::EventTypes_::kNoteOffEvent as u32,
            Event__type0 {
                noteOff: NoteOffEvent {
                    channel: 0,
                    pitch: key as i16,
                    velocity: 0.0,
                    noteId: self.note_ids[key as usize],
                    tuning: 0.0,
                },
            },
        );
    }

    fn tuning(&mut self, key: u8, semis: f32) {
        // the tuning note expression: 0.5 = none, +-120 semitones
        let id = self.note_ids[key.min(127) as usize];
        self.event(
            Event_::EventTypes_::kNoteExpressionValueEvent as u32,
            Event__type0 {
                noteExpressionValue: NoteExpressionValueEvent {
                    typeId: NoteExpressionTypeIDs_::kTuningTypeID as u32,
                    noteId: id,
                    value: (0.5 + semis as f64 / 240.0).clamp(0.0, 1.0),
                },
            },
        );
    }

    fn param(&mut self, id: u32, value: f64) {
        let v = value.clamp(0.0, 1.0);
        self.pin.push(id, 0, v);
        Self::mirror_push(&mut self.mirror, id, v);
    }

    fn panic(&mut self) {
        // drop queued notes, release every held key, then reset the plugin
        // (setProcessing off / on clears voices and tails)
        self.ev.v().clear();
        for k in 0..128u8 {
            if self.held[k as usize] {
                self.note_off(k);
            }
        }
        if self.active && self.processing {
            unsafe {
                self.proc_.setProcessing(0);
                self.proc_.setProcessing(1);
            }
        }
    }

    fn process(&mut self, n: usize, out_l: &mut [f32], out_r: &mut [f32]) {
        let m = n.min(MAX_BLOCK);
        for b in self.in_data.iter_mut() {
            for c in b.iter_mut() {
                c[..m].fill(0.0);
            }
        }
        self.run(n, out_l, out_r, false);
    }

    fn process_fx(
        &mut self,
        n: usize,
        in_l: &[f32],
        in_r: &[f32],
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) {
        let n = n.min(MAX_BLOCK).min(in_l.len()).min(in_r.len());
        let mut loud = false;
        for (b, bus) in self.in_data.iter_mut().enumerate() {
            for (c, ch) in bus.iter_mut().enumerate() {
                if b == 0 {
                    let src = if c % 2 == 0 { in_l } else { in_r };
                    ch[..n].copy_from_slice(&src[..n]);
                    loud = loud || src[..n].iter().any(|x| *x != 0.0);
                } else {
                    ch[..n].fill(0.0);
                }
            }
        }
        self.run(n, out_l, out_r, loud);
    }

    fn latency(&self) -> u32 {
        self.lat
    }

    fn params(&self) -> Vec<ParamInfo> {
        self.infos.clone()
    }

    fn value(&self, id: u32) -> Option<f64> {
        let c = self.ctrl.as_ref()?;
        self.infos
            .iter()
            .any(|i| i.id == id)
            .then(|| unsafe { c.getParamNormalized(id) })
    }

    fn value_text(&self, id: u32, value: f64) -> Option<String> {
        let c = self.ctrl.as_ref()?;
        let mut s: String128 = [0; 128];
        if unsafe { c.getParamStringByValue(id, value.clamp(0.0, 1.0), &mut s) } != kResultOk {
            return None;
        }
        let t = wbuf(&s);
        Some(match self.units.get(&id) {
            Some(u) if !t.is_empty() => format!("{t} {u}"),
            _ => t,
        })
    }

    fn save_state(&mut self) -> Option<Vec<u8>> {
        self.flush();
        let cs = ComWrapper::new(MemStream::default());
        let cp = cs.to_com_ptr::<IBStream>()?;
        if unsafe { self.comp.getState(cp.as_ptr()) } != kResultOk {
            return None;
        }
        let comp = cs.data.borrow().clone();
        let mut ctrl = vec![];
        if let (Some(c), true) = (&self.ctrl, self.separate) {
            let ks = ComWrapper::new(MemStream::default());
            if let Some(kp) = ks.to_com_ptr::<IBStream>() {
                if unsafe { c.getState(kp.as_ptr()) } == kResultOk {
                    ctrl = ks.data.borrow().clone();
                }
            }
        }
        let mut b = Vec::with_capacity(12 + comp.len() + ctrl.len());
        b.extend_from_slice(b"CWV3");
        b.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        b.extend_from_slice(&comp);
        b.extend_from_slice(&(ctrl.len() as u32).to_le_bytes());
        b.extend_from_slice(&ctrl);
        Some(b)
    }

    fn load_state(&mut self, data: &[u8]) -> bool {
        let Some((comp, ctrl)) = split_state(data) else {
            return false;
        };
        let cs = MemStream::from(comp);
        let Some(cp) = cs.to_com_ptr::<IBStream>() else {
            return false;
        };
        let ok = unsafe { self.comp.setState(cp.as_ptr()) } == kResultOk;
        if let Some(c) = &self.ctrl {
            if self.separate {
                let cs = MemStream::from(comp);
                if let Some(cp) = cs.to_com_ptr::<IBStream>() {
                    unsafe { c.setComponentState(cp.as_ptr()) };
                }
            }
            if !ctrl.is_empty() {
                let ks = MemStream::from(ctrl);
                if let Some(kp) = ks.to_com_ptr::<IBStream>() {
                    unsafe { c.setState(kp.as_ptr()) };
                }
            }
        }
        self.mirror.clear();
        ok
    }

    fn set_param_main(&mut self, id: u32, value: f64) {
        let v = value.clamp(0.0, 1.0);
        if let Some(c) = &self.ctrl {
            unsafe { c.setParamNormalized(id, v) };
        }
        self.pin.push(id, 0, v);
        self.flush();
    }

    fn has_editor(&self) -> bool {
        if !cfg!(windows) || self.ctrl.is_none() {
            return false;
        }
        if self.editor.get() == 0 {
            let ok = self.view.is_some()
                || self.create_view().is_some_and(|v| unsafe {
                    v.isPlatformTypeSupported(kPlatformTypeHWND) == kResultOk
                });
            self.editor.set(if ok { 2 } else { 1 });
        }
        self.editor.get() == 2
    }

    fn open_editor(&mut self, title: &str) -> Result<(), String> {
        if let Some(w) = &self.gui_win {
            w.raise();
            return Ok(());
        }
        #[cfg(windows)]
        {
            let view = self
                .create_view()
                .ok_or("this plugin has no editor; use the parameter list")?;
            if unsafe { view.isPlatformTypeSupported(kPlatformTypeHWND) } != kResultOk {
                return Err("this plugin has no Windows editor; use the parameter list".into());
            }
            self.flags.gui_closed.store(false, Ordering::Release);
            let (w, frame) = editor::open(&view, title)?;
            self.view = Some(view);
            self.frame = Some(frame);
            self.gui_win = Some(w);
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = title;
            Err("VST3 editors open on Windows for now; use the parameter list here".into())
        }
    }

    fn close_editor(&mut self) {
        #[cfg(windows)]
        if let Some(w) = self.gui_win.take() {
            if let Some(v) = self.view.take() {
                editor::close(&v, w);
            }
            self.frame = None;
        }
        self.view = None;
        self.gui_win = None;
    }

    fn editor_open(&self) -> bool {
        self.gui_win.is_some()
    }

    fn poll_main(&mut self) {
        let rs = self.shared.restart.swap(0, Ordering::AcqRel);
        use RestartFlags_::*;
        if rs & (kReloadComponent | kLatencyChanged | kIoChanged) as i32 != 0 {
            self.deactivate();
            let _ = self.activate();
        }
        if rs & (kParamTitlesChanged | kParamValuesChanged | kReloadComponent) as i32 != 0 {
            self.refresh_params();
            self.flags.rescan.store(true, Ordering::Release);
        }
        self.mirror_to_controller();
        // editor edits while nothing plays still reach the processor
        if !self.processing {
            self.flush();
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

// ------------------------------------------------------------------ editor

/// `IPlugFrame`: the plugin asks for a new editor size
#[cfg(windows)]
struct Frame {
    hwnd: Cell<isize>,
}

#[cfg(windows)]
impl Class for Frame {
    type Interfaces = (IPlugFrame,);
}

#[cfg(windows)]
impl IPlugFrameTrait for Frame {
    unsafe fn resizeView(&self, view: *mut IPlugView, r: *mut ViewRect) -> tresult {
        let Some(v) = ComRef::from_raw(view) else {
            return kInvalidArgument;
        };
        if r.is_null() || self.hwnd.get() == 0 {
            return kInvalidArgument;
        }
        let (w, h) = ((*r).right - (*r).left, (*r).bottom - (*r).top);
        super::gui::win::resize_quiet(self.hwnd.get(), w.max(1) as u32, h.max(1) as u32);
        v.onSize(r);
        kResultTrue
    }
}

#[cfg(windows)]
mod editor {
    use super::super::gui::{self, win};
    use super::*;

    pub fn open(
        view: &ComPtr<IPlugView>,
        title: &str,
    ) -> Result<(gui::Window, ComWrapper<Frame>), String> {
        let frame = ComWrapper::new(Frame { hwnd: Cell::new(0) });
        let fp = frame.to_com_ptr::<IPlugFrame>().ok_or("frame")?;
        unsafe {
            view.setFrame(fp.as_ptr());
            let mut r: ViewRect = std::mem::zeroed();
            let (w, h) = if view.getSize(&mut r) == kResultOk {
                (
                    (r.right - r.left).max(64) as u32,
                    (r.bottom - r.top).max(48) as u32,
                )
            } else {
                (800, 500)
            };
            let resizable = view.canResize() == kResultTrue;
            let hwnd = win::create(
                title,
                w,
                h,
                resizable,
                win::Owner::Vst3 {
                    view: view.as_ptr() as usize,
                },
            )?;
            frame.hwnd.set(hwnd);
            if view.attached(hwnd as *mut c_void, kPlatformTypeHWND) != kResultOk {
                view.setFrame(std::ptr::null_mut());
                win::destroy(hwnd);
                return Err("the plugin refused the editor window".into());
            }
            win::raise(hwnd);
            Ok((gui::Window::embedded(hwnd), frame))
        }
    }

    pub fn close(view: &ComPtr<IPlugView>, w: gui::Window) {
        unsafe {
            view.removed();
            view.setFrame(std::ptr::null_mut());
        }
        gui::destroy_window(w);
    }
}
