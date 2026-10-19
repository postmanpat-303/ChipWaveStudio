//! ChipWave Test Synth: a tiny CLAP instrument used as a test fixture by
//! ChipWave Studio's plugin host (src/plugin/). Polyphonic sine/square with
//! two parameters (Gain, Tone), state save/load, CLAP and MIDI note dialects
//! and per-note tuning expressions. Built on clap-sys (MIT OR Apache-2.0,
//! https://github.com/micahrj/clap-sys); CLAP is by free-audio (MIT,
//! https://github.com/free-audio/clap).
//!
//! The same library also holds **ChipWave Test FX** (`org.chipwave.test-fx`),
//! a stereo audio effect for the plugin-FX tests: Gain (0-2) and Flip
//! (polarity) parameters, state save/load, and a fixed 32-sample delay that it
//! reports through the CLAP latency extension.
//!
//! Test hooks: if the plugin file's path contains "crash" the entry aborts
//! the process on init (scanner crash guard); "hang" makes init sleep forever
//! (scanner timeout).
#![allow(clippy::missing_safety_doc)]
use clap_sys::entry::*;
use clap_sys::events::*;
use clap_sys::ext::audio_ports::*;
use clap_sys::ext::note_ports::*;
use clap_sys::ext::params::*;
use clap_sys::ext::state::*;
use clap_sys::factory::plugin_factory::*;
use clap_sys::host::*;
use clap_sys::id::*;
use clap_sys::plugin::*;
use clap_sys::process::*;
use clap_sys::stream::*;
use clap_sys::version::*;
use std::ffi::{c_char, c_void, CStr};
use std::ptr::null;

const ID: &CStr = c"org.chipwave.test-synth";
const NAME: &CStr = c"ChipWave Test Synth";

struct Feat([*const c_char; 4]);
unsafe impl Sync for Feat {}
static FEATURES: Feat = Feat([
    c"instrument".as_ptr(),
    c"synthesizer".as_ptr(),
    c"stereo".as_ptr(),
    null(),
]);

struct Desc(clap_plugin_descriptor);
unsafe impl Sync for Desc {}
static DESC: Desc = Desc(clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: ID.as_ptr(),
    name: NAME.as_ptr(),
    vendor: c"ChipWave".as_ptr(),
    url: c"https://github.com/postmanpat-303/ChipWaveStudio".as_ptr(),
    manual_url: c"".as_ptr(),
    support_url: c"".as_ptr(),
    version: c"0.1.0".as_ptr(),
    description: c"Test fixture: polyphonic sine/square".as_ptr(),
    features: &FEATURES.0 as *const _ as *const *const c_char,
});

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Voice {
    on: bool,
    gate: bool,
    key: i16,
    id: i32,
    vel: f32,
    tune: f64,
    ph: f32,
    env: f32,
}

const OFF: Voice = Voice {
    on: false,
    gate: false,
    key: 0,
    id: -1,
    vel: 0.0,
    tune: 0.0,
    ph: 0.0,
    env: 0.0,
};

#[repr(C)]
struct Plug {
    plugin: clap_plugin,
    sr: f32,
    gain: f64,
    tone: f64,
    v: [Voice; 8],
}

unsafe fn me<'a>(p: *const clap_plugin) -> &'a mut Plug {
    &mut *((*p).plugin_data as *mut Plug)
}

impl Plug {
    fn note_on(&mut self, key: i16, id: i32, vel: f64) {
        let slot = self
            .v
            .iter()
            .position(|v| !v.on)
            .unwrap_or_else(|| (key as usize) % 8);
        self.v[slot] = Voice {
            on: true,
            gate: true,
            key,
            id,
            vel: vel as f32,
            tune: 0.0,
            ph: 0.0,
            env: 0.0,
        };
    }
    fn note_off(&mut self, key: i16) {
        for v in self.v.iter_mut().filter(|v| v.on && v.key == key) {
            v.gate = false;
        }
    }
    unsafe fn event(&mut self, h: *const clap_event_header) {
        if (*h).space_id != CLAP_CORE_EVENT_SPACE_ID {
            return;
        }
        match (*h).type_ {
            CLAP_EVENT_NOTE_ON => {
                let e = &*(h as *const clap_event_note);
                self.note_on(e.key, e.note_id, e.velocity);
            }
            CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE => {
                let e = &*(h as *const clap_event_note);
                self.note_off(e.key);
            }
            CLAP_EVENT_NOTE_EXPRESSION => {
                let e = &*(h as *const clap_event_note_expression);
                if e.expression_id == CLAP_NOTE_EXPRESSION_TUNING {
                    for v in self.v.iter_mut().filter(|v| v.on && v.key == e.key) {
                        v.tune = e.value;
                    }
                }
            }
            CLAP_EVENT_PARAM_VALUE => {
                let e = &*(h as *const clap_event_param_value);
                match e.param_id {
                    1 => self.gain = e.value.clamp(0.0, 1.0),
                    2 => self.tone = e.value.clamp(0.0, 1.0),
                    _ => {}
                }
            }
            CLAP_EVENT_MIDI => {
                let e = &*(h as *const clap_event_midi);
                let (st, k, vel) = (e.data[0] & 0xF0, e.data[1] as i16, e.data[2]);
                if st == 0x90 && vel > 0 {
                    self.note_on(k, -1, vel as f64 / 127.0);
                } else if st == 0x80 || st == 0x90 {
                    self.note_off(k);
                }
            }
            _ => {}
        }
    }
    fn sample(&mut self) -> f32 {
        let (att, rel) = (1.0 / (0.005 * self.sr), 1.0 / (0.05 * self.sr));
        let mut s = 0.0;
        for v in self.v.iter_mut().filter(|v| v.on) {
            if v.gate {
                v.env = (v.env + att).min(1.0);
            } else {
                v.env -= rel;
                if v.env <= 0.0 {
                    v.on = false;
                    continue;
                }
            }
            let f = 440.0 * 2f64.powf((v.key as f64 + v.tune - 69.0) / 12.0);
            v.ph = (v.ph + f as f32 / self.sr).fract();
            let sine = (v.ph * std::f32::consts::TAU).sin();
            let sq = if v.ph < 0.5 { 0.6 } else { -0.6 };
            let t = self.tone as f32;
            s += (sine * (1.0 - t) + sq * t) * v.vel * v.env;
        }
        s * self.gain as f32 * 0.5
    }
}

unsafe extern "C" fn p_init(_p: *const clap_plugin) -> bool {
    true
}
unsafe extern "C" fn p_destroy(p: *const clap_plugin) {
    drop(Box::from_raw((*p).plugin_data as *mut Plug));
}
unsafe extern "C" fn p_activate(p: *const clap_plugin, sr: f64, _a: u32, _b: u32) -> bool {
    me(p).sr = sr as f32;
    true
}
unsafe extern "C" fn p_deactivate(_p: *const clap_plugin) {}
unsafe extern "C" fn p_start(_p: *const clap_plugin) -> bool {
    true
}
unsafe extern "C" fn p_stop(_p: *const clap_plugin) {}
unsafe extern "C" fn p_reset(p: *const clap_plugin) {
    me(p).v = [OFF; 8];
}
unsafe extern "C" fn p_process(
    p: *const clap_plugin,
    pr: *const clap_process,
) -> clap_process_status {
    let s = me(p);
    let pr = &*pr;
    let n = pr.frames_count as usize;
    let inev = &*pr.in_events;
    let count = inev.size.map(|f| f(inev)).unwrap_or(0);
    let mut next = 0;
    if pr.audio_outputs_count == 0 {
        return CLAP_PROCESS_ERROR;
    }
    let out = &*pr.audio_outputs;
    let l = *out.data32;
    let r = if out.channel_count > 1 {
        *out.data32.add(1)
    } else {
        l
    };
    for i in 0..n {
        while next < count {
            let h = (inev.get.unwrap())(inev, next);
            if (*h).time as usize > i {
                break;
            }
            s.event(h);
            next += 1;
        }
        let x = s.sample();
        *l.add(i) = x;
        *r.add(i) = x;
    }
    while next < count {
        s.event((inev.get.unwrap())(inev, next));
        next += 1;
    }
    if s.v.iter().any(|v| v.on) {
        CLAP_PROCESS_CONTINUE
    } else {
        CLAP_PROCESS_SLEEP
    }
}

// ---- extensions
unsafe extern "C" fn ap_count(_p: *const clap_plugin, input: bool) -> u32 {
    if input {
        0
    } else {
        1
    }
}
unsafe extern "C" fn ap_get(
    _p: *const clap_plugin,
    i: u32,
    input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if input || i != 0 {
        return false;
    }
    let info = &mut *info;
    info.id = 0;
    write_name(&mut info.name, "Out");
    info.flags = CLAP_AUDIO_PORT_IS_MAIN;
    info.channel_count = 2;
    info.port_type = CLAP_PORT_STEREO.as_ptr();
    info.in_place_pair = CLAP_INVALID_ID;
    true
}
static AUDIO_PORTS: clap_plugin_audio_ports = clap_plugin_audio_ports {
    count: Some(ap_count),
    get: Some(ap_get),
};

unsafe extern "C" fn np_count(_p: *const clap_plugin, input: bool) -> u32 {
    if input {
        1
    } else {
        0
    }
}
unsafe extern "C" fn np_get(
    _p: *const clap_plugin,
    i: u32,
    input: bool,
    info: *mut clap_note_port_info,
) -> bool {
    if !input || i != 0 {
        return false;
    }
    let info = &mut *info;
    info.id = 0;
    info.supported_dialects = CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI;
    info.preferred_dialect = CLAP_NOTE_DIALECT_CLAP;
    write_name(&mut info.name, "Notes");
    true
}
static NOTE_PORTS: clap_plugin_note_ports = clap_plugin_note_ports {
    count: Some(np_count),
    get: Some(np_get),
};

fn write_name(dst: &mut [c_char], s: &str) {
    for (d, b) in dst.iter_mut().zip(s.bytes().chain(std::iter::once(0))) {
        *d = b as c_char;
    }
}

unsafe extern "C" fn pa_count(_p: *const clap_plugin) -> u32 {
    2
}
unsafe extern "C" fn pa_info(_p: *const clap_plugin, i: u32, info: *mut clap_param_info) -> bool {
    let info = &mut *info;
    let (id, name, def) = match i {
        0 => (1, "Gain", 0.5),
        1 => (2, "Tone", 0.0),
        _ => return false,
    };
    info.id = id;
    info.flags = CLAP_PARAM_IS_AUTOMATABLE;
    info.cookie = std::ptr::null_mut();
    write_name(&mut info.name, name);
    write_name(&mut info.module, "");
    info.min_value = 0.0;
    info.max_value = 1.0;
    info.default_value = def;
    true
}
unsafe extern "C" fn pa_value(p: *const clap_plugin, id: clap_id, out: *mut f64) -> bool {
    let s = me(p);
    *out = match id {
        1 => s.gain,
        2 => s.tone,
        _ => return false,
    };
    true
}
unsafe extern "C" fn pa_to_text(
    p: *const clap_plugin,
    id: clap_id,
    v: f64,
    buf: *mut c_char,
    cap: u32,
) -> bool {
    let _ = p;
    if id != 1 && id != 2 {
        return false;
    }
    let s = format!("{:.0} %", v * 100.0);
    let dst = std::slice::from_raw_parts_mut(buf, cap as usize);
    write_name(dst, &s);
    true
}
unsafe extern "C" fn pa_from_text(
    _p: *const clap_plugin,
    _id: clap_id,
    _t: *const c_char,
    _o: *mut f64,
) -> bool {
    false
}
unsafe extern "C" fn pa_flush(
    p: *const clap_plugin,
    inev: *const clap_input_events,
    _o: *const clap_output_events,
) {
    let s = me(p);
    let inev = &*inev;
    for i in 0..inev.size.map(|f| f(inev)).unwrap_or(0) {
        s.event((inev.get.unwrap())(inev, i));
    }
}
static PARAMS: clap_plugin_params = clap_plugin_params {
    count: Some(pa_count),
    get_info: Some(pa_info),
    get_value: Some(pa_value),
    value_to_text: Some(pa_to_text),
    text_to_value: Some(pa_from_text),
    flush: Some(pa_flush),
};

unsafe extern "C" fn st_save(p: *const clap_plugin, os: *const clap_ostream) -> bool {
    let s = me(p);
    let mut b = Vec::with_capacity(20);
    b.extend_from_slice(b"CWTS");
    b.extend_from_slice(&s.gain.to_le_bytes());
    b.extend_from_slice(&s.tone.to_le_bytes());
    let mut off = 0;
    while off < b.len() {
        let w = ((*os).write.unwrap())(
            os,
            b[off..].as_ptr() as *const c_void,
            (b.len() - off) as u64,
        );
        if w <= 0 {
            return false;
        }
        off += w as usize;
    }
    true
}
unsafe extern "C" fn st_load(p: *const clap_plugin, is: *const clap_istream) -> bool {
    let mut b = [0u8; 20];
    let mut off = 0;
    while off < 20 {
        let r = ((*is).read.unwrap())(is, b[off..].as_mut_ptr() as *mut c_void, (20 - off) as u64);
        if r <= 0 {
            return false;
        }
        off += r as usize;
    }
    if &b[..4] != b"CWTS" {
        return false;
    }
    let s = me(p);
    s.gain = f64::from_le_bytes(b[4..12].try_into().unwrap());
    s.tone = f64::from_le_bytes(b[12..20].try_into().unwrap());
    true
}
static STATE: clap_plugin_state = clap_plugin_state {
    save: Some(st_save),
    load: Some(st_load),
};

unsafe extern "C" fn p_ext(_p: *const clap_plugin, id: *const c_char) -> *const c_void {
    let id = CStr::from_ptr(id);
    if id == CLAP_EXT_AUDIO_PORTS {
        &AUDIO_PORTS as *const _ as *const c_void
    } else if id == CLAP_EXT_NOTE_PORTS {
        &NOTE_PORTS as *const _ as *const c_void
    } else if id == CLAP_EXT_PARAMS {
        &PARAMS as *const _ as *const c_void
    } else if id == CLAP_EXT_STATE {
        &STATE as *const _ as *const c_void
    } else {
        null()
    }
}
unsafe extern "C" fn p_main(_p: *const clap_plugin) {}

// ---- factory + entry
unsafe extern "C" fn f_count(_f: *const clap_plugin_factory) -> u32 {
    2
}
unsafe extern "C" fn f_desc(
    _f: *const clap_plugin_factory,
    i: u32,
) -> *const clap_plugin_descriptor {
    match i {
        0 => &DESC.0,
        1 => &fx::DESC.0,
        _ => null(),
    }
}
unsafe extern "C" fn f_create(
    _f: *const clap_plugin_factory,
    _host: *const clap_host,
    id: *const c_char,
) -> *const clap_plugin {
    if CStr::from_ptr(id) == fx::ID {
        return fx::create();
    }
    if CStr::from_ptr(id) != ID {
        return null();
    }
    let b = Box::into_raw(Box::new(Plug {
        plugin: clap_plugin {
            desc: &DESC.0,
            plugin_data: std::ptr::null_mut(),
            init: Some(p_init),
            destroy: Some(p_destroy),
            activate: Some(p_activate),
            deactivate: Some(p_deactivate),
            start_processing: Some(p_start),
            stop_processing: Some(p_stop),
            reset: Some(p_reset),
            process: Some(p_process),
            get_extension: Some(p_ext),
            on_main_thread: Some(p_main),
        },
        sr: 48000.0,
        gain: 0.5,
        tone: 0.0,
        v: [OFF; 8],
    }));
    (*b).plugin.plugin_data = b as *mut c_void;
    &(*b).plugin
}
static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(f_count),
    get_plugin_descriptor: Some(f_desc),
    create_plugin: Some(f_create),
};

unsafe extern "C" fn e_init(path: *const c_char) -> bool {
    let p = CStr::from_ptr(path).to_string_lossy();
    if p.contains("crash") {
        std::process::abort();
    }
    if p.contains("hang") {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    true
}
unsafe extern "C" fn e_deinit() {}
unsafe extern "C" fn e_factory(id: *const c_char) -> *const c_void {
    if CStr::from_ptr(id) == CLAP_PLUGIN_FACTORY_ID {
        &FACTORY as *const _ as *const c_void
    } else {
        null()
    }
}

#[allow(non_upper_case_globals)]
#[no_mangle]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(e_init),
    deinit: Some(e_deinit),
    get_factory: Some(e_factory),
};

/// ChipWave Test FX: stereo gain / polarity with a reported 32-sample delay.
mod fx {
    use super::write_name;
    use clap_sys::audio_buffer::*;
    use clap_sys::events::*;
    use clap_sys::ext::audio_ports::*;
    use clap_sys::ext::latency::*;
    use clap_sys::ext::params::*;
    use clap_sys::ext::state::*;
    use clap_sys::id::*;
    use clap_sys::plugin::*;
    use clap_sys::process::*;
    use clap_sys::stream::*;
    use clap_sys::version::*;
    use std::ffi::{c_char, c_void, CStr};
    use std::ptr::null;

    pub const ID: &CStr = c"org.chipwave.test-fx";
    pub const LAT: usize = 32;

    struct Feat([*const c_char; 3]);
    unsafe impl Sync for Feat {}
    static FEATURES: Feat = Feat([c"audio-effect".as_ptr(), c"stereo".as_ptr(), null()]);

    pub struct Desc(pub clap_plugin_descriptor);
    unsafe impl Sync for Desc {}
    pub static DESC: Desc = Desc(clap_plugin_descriptor {
        clap_version: CLAP_VERSION,
        id: ID.as_ptr(),
        name: c"ChipWave Test FX".as_ptr(),
        vendor: c"ChipWave".as_ptr(),
        url: c"https://github.com/postmanpat-303/ChipWaveStudio".as_ptr(),
        manual_url: c"".as_ptr(),
        support_url: c"".as_ptr(),
        version: c"0.1.0".as_ptr(),
        description: c"Test fixture: gain + polarity, 32-sample latency".as_ptr(),
        features: &FEATURES.0 as *const _ as *const *const c_char,
    });

    #[repr(C)]
    struct Fx {
        plugin: clap_plugin,
        gain: f64,
        flip: f64,
        buf: [[f32; LAT]; 2],
        w: usize,
    }

    unsafe fn me<'a>(p: *const clap_plugin) -> &'a mut Fx {
        &mut *((*p).plugin_data as *mut Fx)
    }

    impl Fx {
        unsafe fn event(&mut self, h: *const clap_event_header) {
            if (*h).space_id != CLAP_CORE_EVENT_SPACE_ID || (*h).type_ != CLAP_EVENT_PARAM_VALUE {
                return;
            }
            let e = &*(h as *const clap_event_param_value);
            match e.param_id {
                10 => self.gain = e.value.clamp(0.0, 2.0),
                11 => self.flip = if e.value >= 0.5 { 1.0 } else { 0.0 },
                _ => {}
            }
        }
    }

    unsafe extern "C" fn p_init(_p: *const clap_plugin) -> bool {
        true
    }
    unsafe extern "C" fn p_destroy(p: *const clap_plugin) {
        drop(Box::from_raw((*p).plugin_data as *mut Fx));
    }
    unsafe extern "C" fn p_activate(_p: *const clap_plugin, _sr: f64, _a: u32, _b: u32) -> bool {
        true
    }
    unsafe extern "C" fn p_deactivate(_p: *const clap_plugin) {}
    unsafe extern "C" fn p_start(_p: *const clap_plugin) -> bool {
        true
    }
    unsafe extern "C" fn p_stop(_p: *const clap_plugin) {}
    unsafe extern "C" fn p_reset(p: *const clap_plugin) {
        me(p).buf = [[0.0; LAT]; 2];
    }
    unsafe extern "C" fn p_process(
        p: *const clap_plugin,
        pr: *const clap_process,
    ) -> clap_process_status {
        let s = me(p);
        let pr = &*pr;
        let n = pr.frames_count as usize;
        let inev = &*pr.in_events;
        let count = inev.size.map(|f| f(inev)).unwrap_or(0);
        for i in 0..count {
            s.event((inev.get.unwrap())(inev, i));
        }
        if pr.audio_outputs_count == 0 || pr.audio_inputs_count == 0 {
            return CLAP_PROCESS_ERROR;
        }
        let (inp, out) = (&*pr.audio_inputs, &*pr.audio_outputs);
        let ch = |b: &clap_audio_buffer, c: u32| *b.data32.add(c.min(b.channel_count - 1) as usize);
        let (il, ir, ol, or) = (ch(inp, 0), ch(inp, 1), ch(out, 0), ch(out, 1));
        let g = (s.gain * if s.flip >= 0.5 { -1.0 } else { 1.0 }) as f32;
        for i in 0..n {
            let (x, y) = (*il.add(i), *ir.add(i));
            let (dl, dr) = (s.buf[0][s.w], s.buf[1][s.w]);
            s.buf[0][s.w] = x;
            s.buf[1][s.w] = y;
            s.w = (s.w + 1) % LAT;
            *ol.add(i) = dl * g;
            *or.add(i) = dr * g;
        }
        CLAP_PROCESS_CONTINUE
    }

    unsafe extern "C" fn ap_count(_p: *const clap_plugin, _input: bool) -> u32 {
        1
    }
    unsafe extern "C" fn ap_get(
        _p: *const clap_plugin,
        i: u32,
        input: bool,
        info: *mut clap_audio_port_info,
    ) -> bool {
        if i != 0 {
            return false;
        }
        let info = &mut *info;
        info.id = 0;
        write_name(&mut info.name, if input { "In" } else { "Out" });
        info.flags = CLAP_AUDIO_PORT_IS_MAIN;
        info.channel_count = 2;
        info.port_type = CLAP_PORT_STEREO.as_ptr();
        info.in_place_pair = CLAP_INVALID_ID;
        true
    }
    static AUDIO_PORTS: clap_plugin_audio_ports = clap_plugin_audio_ports {
        count: Some(ap_count),
        get: Some(ap_get),
    };

    unsafe extern "C" fn pa_count(_p: *const clap_plugin) -> u32 {
        2
    }
    unsafe extern "C" fn pa_info(_p: *const clap_plugin, i: u32, info: *mut clap_param_info) -> bool {
        let info = &mut *info;
        let (id, name, max, def, flags) = match i {
            0 => (10, "Gain", 2.0, 1.0, CLAP_PARAM_IS_AUTOMATABLE),
            1 => (11, "Flip", 1.0, 0.0, CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_STEPPED),
            _ => return false,
        };
        info.id = id;
        info.flags = flags;
        info.cookie = std::ptr::null_mut();
        write_name(&mut info.name, name);
        write_name(&mut info.module, "");
        info.min_value = 0.0;
        info.max_value = max;
        info.default_value = def;
        true
    }
    unsafe extern "C" fn pa_value(p: *const clap_plugin, id: clap_id, out: *mut f64) -> bool {
        let s = me(p);
        *out = match id {
            10 => s.gain,
            11 => s.flip,
            _ => return false,
        };
        true
    }
    unsafe extern "C" fn pa_to_text(
        _p: *const clap_plugin,
        id: clap_id,
        v: f64,
        buf: *mut c_char,
        cap: u32,
    ) -> bool {
        let s = match id {
            10 => format!("x{v:.2}"),
            11 => (if v >= 0.5 { "inverted" } else { "normal" }).to_string(),
            _ => return false,
        };
        write_name(std::slice::from_raw_parts_mut(buf, cap as usize), &s);
        true
    }
    unsafe extern "C" fn pa_from_text(
        _p: *const clap_plugin,
        _id: clap_id,
        _t: *const c_char,
        _o: *mut f64,
    ) -> bool {
        false
    }
    unsafe extern "C" fn pa_flush(
        p: *const clap_plugin,
        inev: *const clap_input_events,
        _o: *const clap_output_events,
    ) {
        let s = me(p);
        let inev = &*inev;
        for i in 0..inev.size.map(|f| f(inev)).unwrap_or(0) {
            s.event((inev.get.unwrap())(inev, i));
        }
    }
    static PARAMS: clap_plugin_params = clap_plugin_params {
        count: Some(pa_count),
        get_info: Some(pa_info),
        get_value: Some(pa_value),
        value_to_text: Some(pa_to_text),
        text_to_value: Some(pa_from_text),
        flush: Some(pa_flush),
    };

    unsafe extern "C" fn st_save(p: *const clap_plugin, os: *const clap_ostream) -> bool {
        let s = me(p);
        let mut b = Vec::with_capacity(20);
        b.extend_from_slice(b"CWTF");
        b.extend_from_slice(&s.gain.to_le_bytes());
        b.extend_from_slice(&s.flip.to_le_bytes());
        ((*os).write.unwrap())(os, b.as_ptr() as *const c_void, b.len() as u64) == b.len() as i64
    }
    unsafe extern "C" fn st_load(p: *const clap_plugin, is: *const clap_istream) -> bool {
        let mut b = [0u8; 20];
        let mut off = 0;
        while off < 20 {
            let r = ((*is).read.unwrap())(is, b[off..].as_mut_ptr() as *mut c_void, (20 - off) as u64);
            if r <= 0 {
                return false;
            }
            off += r as usize;
        }
        if &b[..4] != b"CWTF" {
            return false;
        }
        let s = me(p);
        s.gain = f64::from_le_bytes(b[4..12].try_into().unwrap());
        s.flip = f64::from_le_bytes(b[12..20].try_into().unwrap());
        true
    }
    static STATE: clap_plugin_state = clap_plugin_state {
        save: Some(st_save),
        load: Some(st_load),
    };

    unsafe extern "C" fn lat_get(_p: *const clap_plugin) -> u32 {
        LAT as u32
    }
    static LATENCY: clap_plugin_latency = clap_plugin_latency { get: Some(lat_get) };

    unsafe extern "C" fn p_ext(_p: *const clap_plugin, id: *const c_char) -> *const c_void {
        let id = CStr::from_ptr(id);
        if id == CLAP_EXT_AUDIO_PORTS {
            &AUDIO_PORTS as *const _ as *const c_void
        } else if id == CLAP_EXT_PARAMS {
            &PARAMS as *const _ as *const c_void
        } else if id == CLAP_EXT_STATE {
            &STATE as *const _ as *const c_void
        } else if id == CLAP_EXT_LATENCY {
            &LATENCY as *const _ as *const c_void
        } else {
            null()
        }
    }
    unsafe extern "C" fn p_main(_p: *const clap_plugin) {}

    pub unsafe fn create() -> *const clap_plugin {
        let b = Box::into_raw(Box::new(Fx {
            plugin: clap_plugin {
                desc: &DESC.0,
                plugin_data: std::ptr::null_mut(),
                init: Some(p_init),
                destroy: Some(p_destroy),
                activate: Some(p_activate),
                deactivate: Some(p_deactivate),
                start_processing: Some(p_start),
                stop_processing: Some(p_stop),
                reset: Some(p_reset),
                process: Some(p_process),
                get_extension: Some(p_ext),
                on_main_thread: Some(p_main),
            },
            gain: 1.0,
            flip: 0.0,
            buf: [[0.0; LAT]; 2],
            w: 0,
        }));
        (*b).plugin.plugin_data = b as *mut c_void;
        &(*b).plugin
    }
}
