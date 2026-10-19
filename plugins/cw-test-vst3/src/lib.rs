//! ChipWave VST3 Test Synth + Test FX: a tiny VST 3 module used as a test
//! fixture by ChipWave Studio's VST3 host (src/plugin/vst3.rs). It behaves
//! like the CLAP fixture (plugins/cw-test-synth) so the two formats can be
//! compared sample for sample:
//!
//! - **ChipWave VST3 Test Synth** (`Instrument|Synth`): polyphonic
//!   sine/square, Gain (id 1) and Tone (id 2), per-note tuning through the
//!   VST3 tuning note expression, and a read-only Link parameter (id 3) that
//!   the controller sets when the processor's "cw-hello" message (an
//!   `IMessage` allocated through the host's `IHostApplication`) reaches it
//!   over the `IConnectionPoint` link.
//! - **ChipWave VST3 Test FX** (`Fx|Delay`): stereo Gain (id 10, 0-2) and
//!   Flip (id 11, stepped) with a fixed 32-sample delay that it reports as
//!   its latency.
//!
//! Processor and controller are separate classes (state flows through
//! `getState` / `setComponentState`). Built on the `vst3` crate (Rust
//! bindings of the VST 3 API by Micah Johnston, MIT OR Apache-2.0,
//! https://github.com/coupler-rs/vst3-rs); the VST 3 API is by Steinberg Media
//! Technologies GmbH (MIT since SDK 3.8.0).
// binding enum types differ per OS: the casts are needed on one of them
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::missing_safety_doc,
    clippy::unnecessary_cast
)]

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_void, CStr};
use vst3::Steinberg::Vst::*;
use vst3::Steinberg::*;
use vst3::{uid, Class, ComPtr, ComRef, ComWrapper, Interface};

pub const LAT: usize = 32;

const SYNTH_PROC: TUID = uid(0xC3A1_5E01, 0x7B2D_4F10, 0x9E44_0A61, 0x5C2B_7701);
const SYNTH_CTRL: TUID = uid(0xC3A1_5E02, 0x7B2D_4F10, 0x9E44_0A61, 0x5C2B_7702);
const FX_PROC: TUID = uid(0xC3A1_5E03, 0x7B2D_4F10, 0x9E44_0A61, 0x5C2B_7703);
const FX_CTRL: TUID = uid(0xC3A1_5E04, 0x7B2D_4F10, 0x9E44_0A61, 0x5C2B_7704);

fn copy_cstring(src: &str, dst: &mut [c_char]) {
    let n = src.len().min(dst.len() - 1);
    for (d, s) in dst.iter_mut().zip(src.bytes().take(n)) {
        *d = s as c_char;
    }
    dst[n] = 0;
}

fn copy_wstring(src: &str, dst: &mut [TChar]) {
    let mut n = 0;
    for (d, s) in dst.iter_mut().zip(src.encode_utf16()) {
        *d = s as TChar;
        n += 1;
    }
    let n = n.min(dst.len() - 1);
    dst[n] = 0;
}

unsafe fn write_all(st: *mut IBStream, b: &[u8]) -> bool {
    let Some(st) = ComRef::from_raw(st) else {
        return false;
    };
    let mut done = 0;
    st.write(b.as_ptr() as *mut c_void, b.len() as i32, &mut done) == kResultOk
        && done as usize == b.len()
}

unsafe fn read_exact(st: *mut IBStream, b: &mut [u8]) -> bool {
    let Some(st) = ComRef::from_raw(st) else {
        return false;
    };
    let mut off = 0;
    while off < b.len() {
        let mut got = 0;
        if st.read(
            b[off..].as_mut_ptr() as *mut c_void,
            (b.len() - off) as i32,
            &mut got,
        ) != kResultOk
            || got <= 0
        {
            return false;
        }
        off += got as usize;
    }
    true
}

/// "<tag><a f64 LE><b f64 LE>"
unsafe fn read_state(st: *mut IBStream, tag: &[u8; 4]) -> Option<(f64, f64)> {
    let mut b = [0u8; 20];
    if !read_exact(st, &mut b) || &b[..4] != tag {
        return None;
    }
    Some((
        f64::from_le_bytes(b[4..12].try_into().unwrap()),
        f64::from_le_bytes(b[12..20].try_into().unwrap()),
    ))
}

unsafe fn write_state(st: *mut IBStream, tag: &[u8; 4], a: f64, b: f64) -> tresult {
    let mut v = [0u8; 20];
    v[..4].copy_from_slice(tag);
    v[4..12].copy_from_slice(&a.to_le_bytes());
    v[12..20].copy_from_slice(&b.to_le_bytes());
    if write_all(st, &v) {
        kResultOk
    } else {
        kResultFalse
    }
}

fn bus_info(bus: &mut BusInfo, media: MediaType, dir: BusDirection, ch: i32, name: &str) {
    bus.mediaType = media;
    bus.direction = dir;
    bus.channelCount = ch;
    copy_wstring(name, &mut bus.name);
    bus.busType = BusTypes_::kMain as BusType;
    bus.flags = BusInfo_::BusFlags_::kDefaultActive as u32;
}

/// the last value of every parameter queue: (id, value)
unsafe fn param_changes(data: &ProcessData, mut f: impl FnMut(u32, f64)) {
    let Some(pc) = ComRef::from_raw(data.inputParameterChanges) else {
        return;
    };
    for i in 0..pc.getParameterCount() {
        let Some(q) = ComRef::from_raw(pc.getParameterData(i)) else {
            continue;
        };
        let n = q.getPointCount();
        let (mut off, mut v) = (0, 0.0);
        if n > 0 && q.getPoint(n - 1, &mut off, &mut v) == kResultOk {
            f(q.getParameterId(), v);
        }
    }
}

unsafe fn out_channels(data: &ProcessData) -> Option<(*mut f32, *mut f32)> {
    if data.numOutputs < 1 || data.outputs.is_null() {
        return None;
    }
    let b = &mut *data.outputs;
    if b.numChannels < 1 {
        return None;
    }
    b.silenceFlags = 0;
    let ch = b.__field0.channelBuffers32;
    let l = *ch;
    let r = if b.numChannels > 1 { *ch.add(1) } else { l };
    Some((l, r))
}

// ===================================================================== synth

#[derive(Clone, Copy)]
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

struct Synth {
    sr: f32,
    gain: f64,
    tone: f64,
    v: [Voice; 8],
    host: Option<ComPtr<IHostApplication>>,
    peer: Option<ComPtr<IConnectionPoint>>,
}

impl Synth {
    fn note_on(&mut self, key: i16, id: i32, vel: f32) {
        let slot = self
            .v
            .iter()
            .position(|v| !v.on)
            .unwrap_or((key as usize) % 8);
        self.v[slot] = Voice {
            on: true,
            gate: true,
            key,
            id,
            vel,
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
    unsafe fn event(&mut self, e: &Event) {
        match e.r#type as u32 {
            x if x == Event_::EventTypes_::kNoteOnEvent as u32 => {
                let n = e.__field0.noteOn;
                if n.velocity > 0.0 {
                    self.note_on(n.pitch, n.noteId, n.velocity);
                } else {
                    self.note_off(n.pitch);
                }
            }
            x if x == Event_::EventTypes_::kNoteOffEvent as u32 => {
                self.note_off(e.__field0.noteOff.pitch);
            }
            x if x == Event_::EventTypes_::kNoteExpressionValueEvent as u32 => {
                let n = e.__field0.noteExpressionValue;
                if n.typeId == NoteExpressionTypeIDs_::kTuningTypeID as u32 {
                    for v in self.v.iter_mut().filter(|v| v.on && v.id == n.noteId) {
                        v.tune = 240.0 * (n.value - 0.5);
                    }
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

struct SynthProc(UnsafeCell<Synth>);

impl SynthProc {
    #[allow(clippy::mut_from_ref)]
    fn s(&self) -> &mut Synth {
        unsafe { &mut *self.0.get() }
    }
}

impl Class for SynthProc {
    type Interfaces = (IComponent, IAudioProcessor, IConnectionPoint);
}

impl IPluginBaseTrait for SynthProc {
    unsafe fn initialize(&self, context: *mut FUnknown) -> tresult {
        self.s().host = ComRef::from_raw(context).and_then(|c| c.cast());
        kResultOk
    }
    unsafe fn terminate(&self) -> tresult {
        self.s().host = None;
        kResultOk
    }
}

impl IComponentTrait for SynthProc {
    unsafe fn getControllerClassId(&self, class_id: *mut TUID) -> tresult {
        *class_id = SYNTH_CTRL;
        kResultOk
    }
    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }
    unsafe fn getBusCount(&self, media: MediaType, dir: BusDirection) -> i32 {
        let out = dir == BusDirections_::kOutput as BusDirection;
        if (media == MediaTypes_::kAudio as MediaType && out)
            || (media == MediaTypes_::kEvent as MediaType && !out)
        {
            1
        } else {
            0
        }
    }
    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        bus: *mut BusInfo,
    ) -> tresult {
        if index != 0 || self.getBusCount(media, dir) == 0 {
            return kInvalidArgument;
        }
        if media == MediaTypes_::kAudio as MediaType {
            bus_info(&mut *bus, media, dir, 2, "Out");
        } else {
            bus_info(&mut *bus, media, dir, 1, "Notes");
        }
        kResultOk
    }
    unsafe fn getRoutingInfo(&self, _i: *mut RoutingInfo, _o: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }
    unsafe fn activateBus(&self, _m: MediaType, _d: BusDirection, _i: i32, _s: TBool) -> tresult {
        kResultOk
    }
    unsafe fn setActive(&self, state: TBool) -> tresult {
        let s = self.s();
        if state != 0 {
            // say hello to the controller through a host-allocated message
            if let (Some(h), Some(peer)) = (&s.host, &s.peer) {
                let mut cid = IMessage::IID.map(|b| b as c_char);
                let mut iid = cid;
                let mut obj: *mut c_void = std::ptr::null_mut();
                if h.createInstance(&mut cid, &mut iid, &mut obj) == kResultOk {
                    if let Some(m) = ComPtr::<IMessage>::from_raw(obj as *mut IMessage) {
                        m.setMessageID(c"cw-hello".as_ptr());
                        if let Some(a) = ComRef::from_raw(m.getAttributes()) {
                            a.setFloat(c"sr".as_ptr(), s.sr as f64);
                        }
                        peer.notify(m.as_ptr());
                    }
                }
            }
        }
        kResultOk
    }
    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        match read_state(state, b"CWTS") {
            Some((g, t)) => {
                self.s().gain = g.clamp(0.0, 1.0);
                self.s().tone = t.clamp(0.0, 1.0);
                kResultOk
            }
            None => kResultFalse,
        }
    }
    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        write_state(state, b"CWTS", self.s().gain, self.s().tone)
    }
}

impl IConnectionPointTrait for SynthProc {
    unsafe fn connect(&self, other: *mut IConnectionPoint) -> tresult {
        self.s().peer = ComRef::from_raw(other).map(|o| o.to_com_ptr());
        kResultOk
    }
    unsafe fn disconnect(&self, _other: *mut IConnectionPoint) -> tresult {
        self.s().peer = None;
        kResultOk
    }
    unsafe fn notify(&self, _message: *mut IMessage) -> tresult {
        kResultOk
    }
}

impl IAudioProcessorTrait for SynthProc {
    unsafe fn setBusArrangements(
        &self,
        _inputs: *mut SpeakerArrangement,
        num_ins: i32,
        outputs: *mut SpeakerArrangement,
        num_outs: i32,
    ) -> tresult {
        if num_ins == 0 && num_outs == 1 && *outputs == SpeakerArr::kStereo {
            kResultTrue
        } else {
            kResultFalse
        }
    }
    unsafe fn getBusArrangement(
        &self,
        dir: BusDirection,
        index: i32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        if dir == BusDirections_::kOutput as BusDirection && index == 0 {
            *arr = SpeakerArr::kStereo;
            kResultOk
        } else {
            kInvalidArgument
        }
    }
    unsafe fn canProcessSampleSize(&self, size: i32) -> tresult {
        if size == SymbolicSampleSizes_::kSample32 as i32 {
            kResultOk
        } else {
            kNotImplemented
        }
    }
    unsafe fn getLatencySamples(&self) -> u32 {
        0
    }
    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        self.s().sr = (*setup).sampleRate as f32;
        kResultOk
    }
    unsafe fn setProcessing(&self, state: TBool) -> tresult {
        if state == 0 {
            self.s().v = [OFF; 8];
        }
        kResultOk
    }
    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let s = self.s();
        let data = &*data;
        param_changes(data, |id, v| match id {
            1 => s.gain = v.clamp(0.0, 1.0),
            2 => s.tone = v.clamp(0.0, 1.0),
            _ => {}
        });
        let n = data.numSamples.max(0) as usize;
        let events = ComRef::from_raw(data.inputEvents);
        let count = events.as_ref().map(|e| e.getEventCount()).unwrap_or(0);
        let mut next = 0;
        let mut ev: Event = std::mem::zeroed();
        let Some((l, r)) = out_channels(data) else {
            // parameter flush (no buffers): still take the events
            while next < count {
                if events.unwrap().getEvent(next, &mut ev) == kResultOk {
                    s.event(&ev);
                }
                next += 1;
            }
            return kResultOk;
        };
        for i in 0..n {
            while next < count {
                if events.unwrap().getEvent(next, &mut ev) != kResultOk {
                    next += 1;
                    continue;
                }
                if ev.sampleOffset as usize > i {
                    break;
                }
                s.event(&ev);
                next += 1;
            }
            let x = s.sample();
            *l.add(i) = x;
            *r.add(i) = x;
        }
        while next < count {
            if events.unwrap().getEvent(next, &mut ev) == kResultOk {
                s.event(&ev);
            }
            next += 1;
        }
        kResultOk
    }
    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}

// ------------------------------------------------------------- controllers

/// (id, title, units, steps, default, read-only)
type PDef = (u32, &'static str, &'static str, i32, f64, bool);

struct Ctrl {
    tag: [u8; 4],
    defs: &'static [PDef],
    vals: [f64; 3],
    handler: Option<ComPtr<IComponentHandler>>,
    peer: Option<ComPtr<IConnectionPoint>>,
    /// state field order -> parameter index, and plain -> normalised scale
    scale: [f64; 2],
}

struct Controller(UnsafeCell<Ctrl>);

impl Controller {
    #[allow(clippy::mut_from_ref)]
    fn s(&self) -> &mut Ctrl {
        unsafe { &mut *self.0.get() }
    }
    fn synth() -> Controller {
        const D: &[PDef] = &[
            (1, "Gain", "%", 0, 0.5, false),
            (2, "Tone", "%", 0, 0.0, false),
            (3, "Link", "", 1, 0.0, true),
        ];
        Controller(UnsafeCell::new(Ctrl {
            tag: *b"CWTS",
            defs: D,
            vals: [0.5, 0.0, 0.0],
            handler: None,
            peer: None,
            scale: [1.0, 1.0],
        }))
    }
    fn fx() -> Controller {
        const D: &[PDef] = &[
            (10, "Gain", "", 0, 0.5, false),
            (11, "Flip", "", 1, 0.0, false),
        ];
        Controller(UnsafeCell::new(Ctrl {
            tag: *b"CWTF",
            defs: D,
            vals: [0.5, 0.0, 0.0],
            handler: None,
            peer: None,
            scale: [0.5, 1.0],
        }))
    }
    fn index(&self, id: u32) -> Option<usize> {
        self.s().defs.iter().position(|d| d.0 == id)
    }
}

impl Class for Controller {
    type Interfaces = (IEditController, IConnectionPoint);
}

impl IPluginBaseTrait for Controller {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }
    unsafe fn terminate(&self) -> tresult {
        self.s().handler = None;
        self.s().peer = None;
        kResultOk
    }
}

impl IConnectionPointTrait for Controller {
    unsafe fn connect(&self, other: *mut IConnectionPoint) -> tresult {
        self.s().peer = ComRef::from_raw(other).map(|o| o.to_com_ptr());
        kResultOk
    }
    unsafe fn disconnect(&self, _other: *mut IConnectionPoint) -> tresult {
        self.s().peer = None;
        kResultOk
    }
    unsafe fn notify(&self, message: *mut IMessage) -> tresult {
        let Some(m) = ComRef::from_raw(message) else {
            return kInvalidArgument;
        };
        let id = m.getMessageID();
        if !id.is_null() && CStr::from_ptr(id) == c"cw-hello" {
            let mut sr = 0.0;
            let ok = ComRef::from_raw(m.getAttributes())
                .map(|a| a.getFloat(c"sr".as_ptr(), &mut sr) == kResultOk)
                .unwrap_or(false);
            if ok && sr > 0.0 {
                if let Some(k) = self.index(3) {
                    self.s().vals[k] = 1.0;
                }
            }
        }
        kResultOk
    }
}

impl IEditControllerTrait for Controller {
    unsafe fn setComponentState(&self, state: *mut IBStream) -> tresult {
        let s = self.s();
        match read_state(state, &s.tag) {
            Some((a, b)) => {
                s.vals[0] = (a * s.scale[0]).clamp(0.0, 1.0);
                s.vals[1] = (b * s.scale[1]).clamp(0.0, 1.0);
                kResultOk
            }
            None => kResultFalse,
        }
    }
    unsafe fn setState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }
    unsafe fn getState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }
    unsafe fn getParameterCount(&self) -> i32 {
        self.s().defs.len() as i32
    }
    unsafe fn getParameterInfo(&self, index: i32, info: *mut ParameterInfo) -> tresult {
        let Some(d) = self.s().defs.get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.id = d.0;
        copy_wstring(d.1, &mut info.title);
        copy_wstring(d.1, &mut info.shortTitle);
        copy_wstring(d.2, &mut info.units);
        info.stepCount = d.3;
        info.defaultNormalizedValue = d.4;
        info.unitId = 0;
        info.flags = if d.5 {
            ParameterInfo_::ParameterFlags_::kIsReadOnly as i32
        } else {
            ParameterInfo_::ParameterFlags_::kCanAutomate as i32
        };
        kResultOk
    }
    unsafe fn getParamStringByValue(&self, id: u32, v: f64, string: *mut String128) -> tresult {
        let txt = match (self.s().tag, id) {
            (_, 1) | (_, 2) => format!("{:.0}", v * 100.0),
            (_, 3) => (if v >= 0.5 { "linked" } else { "off" }).to_string(),
            (_, 10) => format!("x{:.2}", v * 2.0),
            (_, 11) => (if v >= 0.5 { "inverted" } else { "normal" }).to_string(),
            _ => return kInvalidArgument,
        };
        copy_wstring(&txt, &mut *string);
        kResultOk
    }
    unsafe fn getParamValueByString(&self, _id: u32, _s: *mut TChar, _v: *mut f64) -> tresult {
        kNotImplemented
    }
    unsafe fn normalizedParamToPlain(&self, id: u32, v: f64) -> f64 {
        if id == 10 {
            v * 2.0
        } else {
            v
        }
    }
    unsafe fn plainParamToNormalized(&self, id: u32, v: f64) -> f64 {
        if id == 10 {
            v / 2.0
        } else {
            v
        }
    }
    unsafe fn getParamNormalized(&self, id: u32) -> f64 {
        self.index(id).map(|k| self.s().vals[k]).unwrap_or(0.0)
    }
    unsafe fn setParamNormalized(&self, id: u32, v: f64) -> tresult {
        match self.index(id) {
            Some(k) => {
                self.s().vals[k] = v.clamp(0.0, 1.0);
                kResultOk
            }
            None => kInvalidArgument,
        }
    }
    unsafe fn setComponentHandler(&self, handler: *mut IComponentHandler) -> tresult {
        self.s().handler = ComRef::from_raw(handler).map(|h| h.to_com_ptr());
        kResultOk
    }
    unsafe fn createView(&self, _name: *const c_char) -> *mut IPlugView {
        std::ptr::null_mut()
    }
}

// ======================================================================== fx

struct Fx {
    gain: f64,
    flip: f64,
    buf: [[f32; LAT]; 2],
    w: usize,
}

struct FxProc(UnsafeCell<Fx>);

impl FxProc {
    #[allow(clippy::mut_from_ref)]
    fn s(&self) -> &mut Fx {
        unsafe { &mut *self.0.get() }
    }
}

impl Class for FxProc {
    type Interfaces = (IComponent, IAudioProcessor);
}

impl IPluginBaseTrait for FxProc {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }
    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IComponentTrait for FxProc {
    unsafe fn getControllerClassId(&self, class_id: *mut TUID) -> tresult {
        *class_id = FX_CTRL;
        kResultOk
    }
    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }
    unsafe fn getBusCount(&self, media: MediaType, _dir: BusDirection) -> i32 {
        (media == MediaTypes_::kAudio as MediaType) as i32
    }
    unsafe fn getBusInfo(
        &self,
        media: MediaType,
        dir: BusDirection,
        index: i32,
        bus: *mut BusInfo,
    ) -> tresult {
        if index != 0 || media != MediaTypes_::kAudio as MediaType {
            return kInvalidArgument;
        }
        let name = if dir == BusDirections_::kInput as BusDirection {
            "In"
        } else {
            "Out"
        };
        bus_info(&mut *bus, media, dir, 2, name);
        kResultOk
    }
    unsafe fn getRoutingInfo(&self, _i: *mut RoutingInfo, _o: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }
    unsafe fn activateBus(&self, _m: MediaType, _d: BusDirection, _i: i32, _s: TBool) -> tresult {
        kResultOk
    }
    unsafe fn setActive(&self, _state: TBool) -> tresult {
        kResultOk
    }
    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        match read_state(state, b"CWTF") {
            Some((g, f)) => {
                self.s().gain = g.clamp(0.0, 2.0);
                self.s().flip = if f >= 0.5 { 1.0 } else { 0.0 };
                kResultOk
            }
            None => kResultFalse,
        }
    }
    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        write_state(state, b"CWTF", self.s().gain, self.s().flip)
    }
}

impl IAudioProcessorTrait for FxProc {
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        num_ins: i32,
        outputs: *mut SpeakerArrangement,
        num_outs: i32,
    ) -> tresult {
        if num_ins == 1
            && num_outs == 1
            && *inputs == SpeakerArr::kStereo
            && *outputs == SpeakerArr::kStereo
        {
            kResultTrue
        } else {
            kResultFalse
        }
    }
    unsafe fn getBusArrangement(
        &self,
        _dir: BusDirection,
        index: i32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        if index == 0 {
            *arr = SpeakerArr::kStereo;
            kResultOk
        } else {
            kInvalidArgument
        }
    }
    unsafe fn canProcessSampleSize(&self, size: i32) -> tresult {
        if size == SymbolicSampleSizes_::kSample32 as i32 {
            kResultOk
        } else {
            kNotImplemented
        }
    }
    unsafe fn getLatencySamples(&self) -> u32 {
        LAT as u32
    }
    unsafe fn setupProcessing(&self, _setup: *mut ProcessSetup) -> tresult {
        kResultOk
    }
    unsafe fn setProcessing(&self, state: TBool) -> tresult {
        if state == 0 {
            self.s().buf = [[0.0; LAT]; 2];
        }
        kResultOk
    }
    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let s = self.s();
        let data = &*data;
        param_changes(data, |id, v| match id {
            10 => s.gain = (v * 2.0).clamp(0.0, 2.0),
            11 => s.flip = if v >= 0.5 { 1.0 } else { 0.0 },
            _ => {}
        });
        let Some((ol, or)) = out_channels(data) else {
            return kResultOk;
        };
        if data.numInputs < 1 || data.inputs.is_null() || (*data.inputs).numChannels < 1 {
            return kResultOk;
        }
        let ib = &*data.inputs;
        let ich = ib.__field0.channelBuffers32;
        let il = *ich;
        let ir = if ib.numChannels > 1 { *ich.add(1) } else { il };
        let g = (s.gain * if s.flip >= 0.5 { -1.0 } else { 1.0 }) as f32;
        for i in 0..data.numSamples.max(0) as usize {
            let (x, y) = (*il.add(i), *ir.add(i));
            let (dl, dr) = (s.buf[0][s.w], s.buf[1][s.w]);
            s.buf[0][s.w] = x;
            s.buf[1][s.w] = y;
            s.w = (s.w + 1) % LAT;
            *ol.add(i) = dl * g;
            *or.add(i) = dr * g;
        }
        kResultOk
    }
    unsafe fn getTailSamples(&self) -> u32 {
        LAT as u32
    }
}

// =================================================================== factory

struct Factory;

impl Class for Factory {
    type Interfaces = (IPluginFactory2,);
}

/// (cid, category, name, sub-categories)
const CLASSES: [(TUID, &str, &str, &str); 4] = [
    (
        SYNTH_PROC,
        "Audio Module Class",
        "ChipWave VST3 Test Synth",
        "Instrument|Synth",
    ),
    (
        SYNTH_CTRL,
        "Component Controller Class",
        "ChipWave VST3 Test Synth",
        "",
    ),
    (
        FX_PROC,
        "Audio Module Class",
        "ChipWave VST3 Test FX",
        "Fx|Delay",
    ),
    (
        FX_CTRL,
        "Component Controller Class",
        "ChipWave VST3 Test FX",
        "",
    ),
];

impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        let info = &mut *info;
        copy_cstring("ChipWave", &mut info.vendor);
        copy_cstring(
            "https://github.com/postmanpat-303/ChipWaveStudio",
            &mut info.url,
        );
        copy_cstring("", &mut info.email);
        info.flags = PFactoryInfo_::FactoryFlags_::kUnicode as i32;
        kResultOk
    }
    unsafe fn countClasses(&self) -> i32 {
        CLASSES.len() as i32
    }
    unsafe fn getClassInfo(&self, index: i32, info: *mut PClassInfo) -> tresult {
        let Some(c) = CLASSES.get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = c.0;
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as i32;
        copy_cstring(c.1, &mut info.category);
        copy_cstring(c.2, &mut info.name);
        kResultOk
    }
    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        let cid = *(cid as *const TUID);
        let unknown = if cid == SYNTH_PROC {
            ComWrapper::new(SynthProc(UnsafeCell::new(Synth {
                sr: 48000.0,
                gain: 0.5,
                tone: 0.0,
                v: [OFF; 8],
                host: None,
                peer: None,
            })))
            .to_com_ptr::<FUnknown>()
        } else if cid == SYNTH_CTRL {
            ComWrapper::new(Controller::synth()).to_com_ptr::<FUnknown>()
        } else if cid == FX_PROC {
            ComWrapper::new(FxProc(UnsafeCell::new(Fx {
                gain: 1.0,
                flip: 0.0,
                buf: [[0.0; LAT]; 2],
                w: 0,
            })))
            .to_com_ptr::<FUnknown>()
        } else if cid == FX_CTRL {
            ComWrapper::new(Controller::fx()).to_com_ptr::<FUnknown>()
        } else {
            None
        };
        match unknown {
            Some(u) => {
                let p = u.as_ptr();
                ((*(*p).vtbl).queryInterface)(p, iid as *const TUID, obj)
            }
            None => {
                *obj = std::ptr::null_mut();
                kInvalidArgument
            }
        }
    }
}

impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(&self, index: i32, info: *mut PClassInfo2) -> tresult {
        let Some(c) = CLASSES.get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = c.0;
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as i32;
        copy_cstring(c.1, &mut info.category);
        copy_cstring(c.2, &mut info.name);
        info.classFlags = 0;
        copy_cstring(c.3, &mut info.subCategories);
        copy_cstring("ChipWave", &mut info.vendor);
        copy_cstring("0.1.0", &mut info.version);
        copy_cstring("VST 3.8.0", &mut info.sdkVersion);
        kResultOk
    }
}

// ===================================================================== entry

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn InitDll() -> bool {
    true
}

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn ExitDll() -> bool {
    true
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "system" fn BundleEntry(_bundle_ref: *mut c_void) -> bool {
    true
}

#[cfg(target_os = "macos")]
#[no_mangle]
extern "system" fn BundleExit() -> bool {
    true
}

#[cfg(all(unix, not(target_os = "macos")))]
#[no_mangle]
extern "system" fn ModuleEntry(_library_handle: *mut c_void) -> bool {
    true
}

#[cfg(all(unix, not(target_os = "macos")))]
#[no_mangle]
extern "system" fn ModuleExit() -> bool {
    true
}

#[no_mangle]
extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    ComWrapper::new(Factory)
        .to_com_ptr::<IPluginFactory>()
        .map(|p| p.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
