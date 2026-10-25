//! Drag-to-modulate (Unreleased, after 0.2.4.0): modulators, as in other DAWs.
//!
//! An instrument owns **modulators** (LFO, envelope, random, velocity,
//! keytrack, mod wheel, macro, or its own synth LFO). A **route** connects a
//! modulator to any automatable parameter (an `automation::Target`: synth
//! knobs through the patch registry below, generator knobs, built-in and
//! plugin FX parameters, mixer faders) with a bipolar amount in the target's
//! normalised 0..1 range. The engine evaluates modulators every 32 frames
//! (the automation grid) and adds `amount × modulator` on top of the
//! parameter's value (or its automation lane), through the same override
//! path automation uses. Nothing allocates in the audio thread.
//!
//! Modulators are **global per instrument** (one value shared by all its
//! voices): envelopes and retriggered LFOs restart on every note of the
//! owner, velocity and keytrack follow its latest note.
use crate::synth::Patch;
use serde::{Deserialize, Serialize};

/// most modulators per song (engine state is a fixed array)
pub const MAX_MODS: usize = 64;
/// most routes per song
pub const MAX_ROUTES: usize = 256;

// ------------------------------------------------------------------ registry

/// A continuous synth-patch parameter that automation and modulation can
/// drive (`Target::Patch`). Ranges match the instrument editor's knobs, so a
/// route's amount is the same fraction of the knob's travel.
pub struct PatchParam {
    /// the knob key in the instrument editor (`app::params`)
    pub key: &'static str,
    pub name: &'static str,
    pub lo: f32,
    pub hi: f32,
    pub log: bool,
    pub get: fn(&Patch) -> f32,
    pub set: fn(&mut Patch, f32),
}

macro_rules! pp {
    ($key:expr, $name:expr, $lo:expr, $hi:expr, $log:expr, $k:expr, $($f:tt)+) => {
        PatchParam {
            key: $key,
            name: $name,
            lo: $lo,
            hi: $hi,
            log: $log,
            get: |p| p.$($f)+ * $k,
            set: |p, v| p.$($f)+ = v / $k,
        }
    };
}

/// Patch knobs without a dedicated target (cutoff, resonance, drive and
/// level have their own `Target`s).
pub static PATCH_PARAMS: &[PatchParam] = &[
    pp!(
        "pulse_width",
        "Pulse width",
        0.05,
        0.95,
        false,
        1.0,
        pulse_width
    ),
    pp!(
        "detune_cents",
        "Detune",
        0.0,
        50.0,
        false,
        1.0,
        detune_cents
    ),
    pp!("env_amount", "Env amt", -1.0, 1.0, false, 1.0, env_amount),
    pp!("fenv.d", "F.Decay", 0.001, 3.0, true, 1.0, fenv.d),
    pp!("amp.a", "Attack", 0.0, 2.0, true, 1.0, amp.a),
    pp!("amp.d", "Decay", 0.001, 3.0, true, 1.0, amp.d),
    pp!("amp.s", "Sustain", 0.0, 1.0, false, 1.0, amp.s),
    pp!("amp.r", "Release", 0.001, 4.0, true, 1.0, amp.r),
    pp!("width", "Stereo", 0.0, 1.0, false, 1.0, width),
    pp!("glide", "Glide", 1.0, 1000.0, true, 1000.0, glide),
    pp!("lfo.rate", "LFO rate", 0.01, 30.0, true, 1.0, lfo.rate),
    pp!(
        "lfo.cutoff",
        "LFO > Cutoff",
        -4.0,
        4.0,
        false,
        1.0,
        lfo.cutoff
    ),
    pp!("lfo.amp", "LFO > Amp", 0.0, 1.0, false, 1.0, lfo.amp),
    pp!("lfo.pitch", "LFO > Pitch", 0.0, 12.0, false, 1.0, lfo.pitch),
    pp!("vel_cutoff", "Vel>Cut", 0.0, 4.0, false, 1.0, vel_cutoff),
    pp!("accent", "Accent", 0.0, 1.0, false, 1.0, accent),
    pp!(
        "start_delay",
        "Delay",
        0.0,
        50.0,
        false,
        1000.0,
        start_delay
    ),
    pp!("metal", "Metal 808", 0.0, 1.0, false, 1.0, metal),
    pp!("gen.release", "Release", 5.0, 2000.0, true, 1000.0, amp.r),
    pp!("gen.glide", "Glide", 0.0, 1000.0, false, 1000.0, glide),
];

/// registry index of an editor knob key
pub fn patch_param(key: &str) -> Option<usize> {
    PATCH_PARAMS.iter().position(|p| p.key == key)
}

/// registry index from the id form of a key (`amp-a` for `amp.a`)
pub fn patch_param_id(id: &str) -> Option<usize> {
    PATCH_PARAMS.iter().position(|p| {
        p.key.len() == id.len()
            && p.key
                .bytes()
                .zip(id.bytes())
                .all(|(a, b)| a == b || (a == b'.' && b == b'-'))
    })
}

/// knob position 0..1 of a value (the editor's mapping)
pub fn to_norm(v: f32, lo: f32, hi: f32, log: bool) -> f32 {
    if hi <= lo || !v.is_finite() {
        return 0.0;
    }
    let v = v.clamp(lo, hi);
    if log {
        let e = if lo <= 0.0 { hi * 1e-4 } else { 0.0 };
        ((v + e) / (lo + e)).ln() / ((hi + e) / (lo + e)).ln()
    } else {
        (v - lo) / (hi - lo)
    }
}

/// value of a knob position 0..1 (the editor's mapping)
pub fn from_norm(n: f32, lo: f32, hi: f32, log: bool) -> f32 {
    let n = n.clamp(0.0, 1.0);
    if log {
        if n <= 0.0 {
            return lo;
        }
        let e = if lo <= 0.0 { hi * 1e-4 } else { 0.0 };
        ((lo + e) * ((hi + e) / (lo + e)).powf(n) - e).clamp(lo, hi)
    } else {
        lo + (hi - lo) * n
    }
}

// --------------------------------------------------------------------- model

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModKind {
    /// free or tempo-synced LFO
    #[default]
    Lfo,
    /// ADSR, restarted by every note of the owner instrument
    Envelope,
    /// sample & hold random steps (with glide)
    Random,
    /// velocity of the owner's latest note (0..1)
    Velocity,
    /// pitch of the owner's latest note (C-4 = 0, ±1 = ±5 octaves)
    Keytrack,
    /// MIDI mod wheel (CC 1)
    ModWheel,
    /// a hand-set knob (one knob moves many parameters)
    Macro,
    /// the instrument's own synth LFO (its shape, rate and sync)
    InstrumentLfo,
    /// MIDI key pressure (aftertouch) of the owner's held notes (0..1);
    /// recorded pressure curves play it back from note clips
    Pressure,
}

impl ModKind {
    pub const ALL: [ModKind; 9] = [
        ModKind::Lfo,
        ModKind::Envelope,
        ModKind::Random,
        ModKind::Velocity,
        ModKind::Keytrack,
        ModKind::ModWheel,
        ModKind::Pressure,
        ModKind::Macro,
        ModKind::InstrumentLfo,
    ];
    pub fn name(self) -> &'static str {
        match self {
            ModKind::Lfo => "LFO",
            ModKind::Envelope => "Envelope",
            ModKind::Random => "Random",
            ModKind::Velocity => "Velocity",
            ModKind::Keytrack => "Keytrack",
            ModKind::ModWheel => "Mod wheel",
            ModKind::Macro => "Macro",
            ModKind::InstrumentLfo => "Instrument LFO",
            ModKind::Pressure => "Pressure",
        }
    }
    /// output range is -1..1 (else 0..1)
    pub fn bipolar(self, unipolar: bool) -> bool {
        match self {
            ModKind::Lfo | ModKind::Random | ModKind::InstrumentLfo => !unipolar,
            ModKind::Keytrack => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    SawUp,
    SawDown,
    Square,
}

impl LfoShape {
    pub const ALL: [LfoShape; 5] = [
        LfoShape::Sine,
        LfoShape::Triangle,
        LfoShape::SawUp,
        LfoShape::SawDown,
        LfoShape::Square,
    ];
    pub fn name(self) -> &'static str {
        match self {
            LfoShape::Sine => "Sine",
            LfoShape::Triangle => "Triangle",
            LfoShape::SawUp => "Saw up",
            LfoShape::SawDown => "Saw down",
            LfoShape::Square => "Square",
        }
    }
    /// -1..1 at phase 0..1
    pub fn at(self, ph: f32) -> f32 {
        let ph = ph - ph.floor();
        match self {
            LfoShape::Sine => (ph * std::f32::consts::TAU).sin(),
            LfoShape::Triangle => 1.0 - 4.0 * ((ph + 0.25).fract() - 0.5).abs(),
            LfoShape::SawUp => 2.0 * ph - 1.0,
            LfoShape::SawDown => 1.0 - 2.0 * ph,
            LfoShape::Square => {
                if ph < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }
}

fn one() -> f32 {
    1.0
}
fn env_a() -> f32 {
    0.01
}
fn env_d() -> f32 {
    0.3
}
fn env_s() -> f32 {
    0.5
}
fn env_r() -> f32 {
    0.3
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Modulator {
    /// stable id (routes point here); unique within the song
    pub id: u32,
    /// owner instrument (0-based): its notes trigger envelopes / retrigger
    pub owner: usize,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: ModKind,
    #[serde(default)]
    pub shape: LfoShape,
    /// free rate in Hz (LFO, Random)
    #[serde(default = "one")]
    pub rate: f32,
    /// beats per cycle, 0 = free rate (LFO, Random)
    #[serde(default)]
    pub sync: f32,
    /// start phase 0..1 (LFO)
    #[serde(default)]
    pub phase: f32,
    /// restart on every note of the owner (LFO, Random)
    #[serde(default)]
    pub retrig: bool,
    /// output 0..1 instead of -1..1 (LFO, Random, Instrument LFO)
    #[serde(default)]
    pub unipolar: bool,
    /// envelope attack / decay / release seconds, sustain 0..1
    #[serde(default = "env_a")]
    pub a: f32,
    #[serde(default = "env_d")]
    pub d: f32,
    #[serde(default = "env_s")]
    pub s: f32,
    #[serde(default = "env_r")]
    pub r: f32,
    /// glide between random steps, 0..1 of a step (Random); smoothing
    /// for Velocity / Keytrack / Mod wheel
    #[serde(default)]
    pub smooth: f32,
    /// the Macro knob 0..1
    #[serde(default)]
    pub value: f32,
}

impl Modulator {
    pub fn new(id: u32, owner: usize, kind: ModKind) -> Modulator {
        Modulator {
            id,
            owner,
            name: String::new(),
            kind,
            shape: LfoShape::Sine,
            rate: 1.0,
            sync: if kind == ModKind::Lfo { 1.0 } else { 0.0 },
            phase: 0.0,
            retrig: false,
            unipolar: false,
            a: env_a(),
            d: env_d(),
            s: env_s(),
            r: env_r(),
            smooth: 0.0,
            value: 0.5,
        }
    }
    /// shown name: own name or "LFO 1"
    pub fn title(&self, n: usize) -> String {
        if self.name.is_empty() {
            format!("{} {}", self.kind.name(), n + 1)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModRoute {
    /// `Modulator::id`
    pub src: u32,
    /// `automation::Target` id (`instrument.2.cutoff`, `instrument.1.fx.1.p3`…)
    pub target: String,
    /// bipolar depth in the target's normalised range (-1..1)
    pub amount: f32,
}

/// Every modulator and route of a song (saved as `modulation.toml`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Modulation {
    #[serde(default, rename = "modulator")]
    pub mods: Vec<Modulator>,
    #[serde(default, rename = "route")]
    pub routes: Vec<ModRoute>,
}

impl Modulation {
    pub fn is_empty(&self) -> bool {
        self.mods.is_empty() && self.routes.is_empty()
    }
    pub fn find(&self, id: u32) -> Option<usize> {
        self.mods.iter().position(|m| m.id == id)
    }
    /// a fresh modulator id
    pub fn next_id(&self) -> u32 {
        self.mods.iter().map(|m| m.id).max().unwrap_or(0) + 1
    }
    /// number of the modulator among its owner's (for "LFO 2")
    pub fn ordinal(&self, k: usize) -> usize {
        let o = self.mods[k].owner;
        let kind = self.mods[k].kind;
        self.mods[..k]
            .iter()
            .filter(|m| m.owner == o && m.kind == kind)
            .count()
    }
    pub fn title(&self, k: usize) -> String {
        self.mods
            .get(k)
            .map(|m| m.title(self.ordinal(k)))
            .unwrap_or_default()
    }
    /// add a modulator to instrument `owner`; its index
    pub fn add(&mut self, owner: usize, kind: ModKind) -> Option<usize> {
        if self.mods.len() >= MAX_MODS {
            return None;
        }
        let id = self.next_id();
        self.mods.push(Modulator::new(id, owner, kind));
        Some(self.mods.len() - 1)
    }
    /// remove modulator `k` and its routes; which routes survive
    /// (`Song::routes_changed` fixes the amount lanes)
    pub fn remove(&mut self, k: usize) -> Vec<bool> {
        if k >= self.mods.len() {
            return vec![true; self.routes.len()];
        }
        let id = self.mods.remove(k).id;
        let keep: Vec<bool> = self.routes.iter().map(|r| r.src != id).collect();
        self.retain_routes(&keep);
        keep
    }
    /// remove route `k`; which routes survive
    pub fn remove_route(&mut self, k: usize) -> Vec<bool> {
        let keep: Vec<bool> = (0..self.routes.len()).map(|i| i != k).collect();
        self.retain_routes(&keep);
        keep
    }
    fn retain_routes(&mut self, keep: &[bool]) {
        let mut i = 0;
        self.routes.retain(|_| {
            i += 1;
            keep.get(i - 1).copied().unwrap_or(true)
        });
    }
    /// route (src → target): its index; a new one starts at `amount`
    pub fn connect(&mut self, src: u32, target: &str, amount: f32) -> Option<usize> {
        if let Some(k) = self
            .routes
            .iter()
            .position(|r| r.src == src && r.target == target)
        {
            return Some(k);
        }
        if self.routes.len() >= MAX_ROUTES || self.find(src).is_none() {
            return None;
        }
        self.routes.push(ModRoute {
            src,
            target: target.to_string(),
            amount: amount.clamp(-1.0, 1.0),
        });
        Some(self.routes.len() - 1)
    }
    /// routes onto `target`: (route index, modulator index)
    pub fn routes_to<'a>(&'a self, target: &'a str) -> impl Iterator<Item = (usize, usize)> + 'a {
        self.routes
            .iter()
            .enumerate()
            .filter(move |(_, r)| r.target == target)
            .filter_map(|(k, r)| self.find(r.src).map(|m| (k, m)))
    }
    /// instrument `idx` was deleted (its modulators and routes onto it go,
    /// later ones shift); which routes survive
    pub fn delete_instrument(&mut self, idx: usize) -> Vec<bool> {
        let gone: Vec<u32> = self
            .mods
            .iter()
            .filter(|m| m.owner == idx)
            .map(|m| m.id)
            .collect();
        self.mods.retain(|m| m.owner != idx);
        for m in &mut self.mods {
            if m.owner > idx {
                m.owner -= 1;
            }
        }
        let keep: Vec<bool> = self
            .routes
            .iter_mut()
            .map(|r| {
                if gone.contains(&r.src) {
                    return false;
                }
                match crate::model::lane_instrument(&r.target) {
                    Some(i) if i == idx => false,
                    Some(i) if i > idx => {
                        r.target = crate::model::with_lane_instrument(&r.target, i - 1);
                        true
                    }
                    _ => true,
                }
            })
            .collect();
        self.retain_routes(&keep);
        keep
    }
    /// instruments were renumbered (map[old] = new)
    pub fn remap(&mut self, map: &[usize]) {
        for m in &mut self.mods {
            if let Some(&n) = map.get(m.owner) {
                m.owner = n;
            }
        }
        for r in &mut self.routes {
            if let Some(i) = crate::model::lane_instrument(&r.target) {
                if let Some(&n) = map.get(i) {
                    r.target = crate::model::with_lane_instrument(&r.target, n);
                }
            }
        }
    }
}

/// colour index of modulator `k` (UI palette)
pub fn colour_ix(k: usize) -> usize {
    k % 6
}

// -------------------------------------------------------------------- engine

/// per-modulator runtime state (audio thread)
#[derive(Clone, Copy, Default)]
struct Rt {
    phase: f64,
    /// envelope level and stage (0 idle, 1 attack, 2 decay/sustain, 3 release)
    env: f32,
    stage: u8,
    trig_seen: u32,
    rnd: f32,
    rnd_from: f32,
    rnd_to: f32,
    rng: u32,
    smooth: f32,
    id: u32,
    started: bool,
}

/// note information per instrument the engine feeds in
pub struct NoteFeed {
    /// note-on counter per instrument (wraps)
    pub trig: [u32; 256],
    pub vel: [f32; 256],
    pub key: [f32; 256],
    /// a note of the instrument is held (computed per evaluation)
    pub held: [bool; 256],
    /// MIDI mod wheel 0..1
    pub wheel: f32,
    /// key pressure 0..1 per instrument: the latest live MIDI pressure
    /// of its held keys, or a note clip's recorded pressure curve
    pub press: [f32; 256],
}

impl Default for NoteFeed {
    fn default() -> Self {
        NoteFeed {
            trig: [0; 256],
            vel: [0.0; 256],
            key: [0.0; 256],
            held: [false; 256],
            wheel: 0.0,
            press: [0.0; 256],
        }
    }
}

impl NoteFeed {
    pub fn note_on(&mut self, instr: usize, note: u8, vel: f32) {
        if instr < 256 {
            self.trig[instr] = self.trig[instr].wrapping_add(1);
            self.vel[instr] = vel.clamp(0.0, 1.0);
            // ChipWave notes: 1 = C-0 ... C-4 = 49
            self.key[instr] = ((note as f32 - 49.0) / 60.0).clamp(-1.0, 1.0);
        }
    }
}

/// The modulators' runtime: outputs per modulator, evaluated every 32
/// frames. Fixed arrays, no allocation.
pub struct ModEngine {
    rt: [Rt; MAX_MODS],
    /// current output per modulator (index = position in `Modulation::mods`)
    pub out: [f32; MAX_MODS],
    pub feed: NoteFeed,
}

impl Default for ModEngine {
    fn default() -> Self {
        ModEngine {
            rt: [Rt::default(); MAX_MODS],
            out: [0.0; MAX_MODS],
            feed: NoteFeed::default(),
        }
    }
}

fn rand01(s: &mut u32) -> f32 {
    // xorshift32
    let mut x = if *s == 0 { 0x9E37_79B9 } else { *s };
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = x;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

impl ModEngine {
    /// back to the start (song start / stop): phases, envelopes, random seeds
    pub fn reset(&mut self) {
        self.rt = [Rt::default(); MAX_MODS];
        self.out = [0.0; MAX_MODS];
        self.feed.trig = [0; 256];
    }

    /// advance every modulator by `dt` seconds and compute its output
    pub fn step(&mut self, m: &Modulation, song: &crate::model::Song, dt: f32, bpm: f32) {
        for (k, md) in m.mods.iter().enumerate().take(MAX_MODS) {
            let rt = &mut self.rt[k];
            if !rt.started || rt.id != md.id {
                *rt = Rt {
                    phase: md.phase as f64,
                    rng: md.id.wrapping_mul(2_654_435_761) | 1,
                    id: md.id,
                    started: true,
                    ..Rt::default()
                };
                rt.rnd_to = rand01(&mut rt.rng) * 2.0 - 1.0;
                rt.rnd_from = rt.rnd_to;
            }
            let o = md.owner.min(255);
            let trig = self.feed.trig[o];
            let new_note = trig != rt.trig_seen;
            rt.trig_seen = trig;
            let held = self.feed.held[o];
            // cycle rate in Hz
            let hz = |rate: f32, sync: f32| {
                if sync > 0.0 {
                    bpm / 60.0 / sync
                } else {
                    rate.max(0.0)
                }
            };
            let v = match md.kind {
                ModKind::Lfo | ModKind::InstrumentLfo => {
                    let (shape, f) = if md.kind == ModKind::InstrumentLfo {
                        match song.instruments.get(md.owner).and_then(|i| i.kind.patch()) {
                            Some(p) => {
                                let sh = match p.lfo.shape {
                                    crate::synth::Wave::Triangle => LfoShape::Triangle,
                                    crate::synth::Wave::Square | crate::synth::Wave::Pulse => {
                                        LfoShape::Square
                                    }
                                    crate::synth::Wave::Saw => LfoShape::SawDown,
                                    crate::synth::Wave::Sine => LfoShape::Sine,
                                };
                                (sh, hz(p.lfo.rate, p.lfo.sync))
                            }
                            None => (LfoShape::Sine, 0.0),
                        }
                    } else {
                        (md.shape, hz(md.rate, md.sync))
                    };
                    if md.retrig && new_note {
                        rt.phase = md.phase as f64;
                    }
                    let y = shape.at(rt.phase as f32);
                    rt.phase = (rt.phase + (f * dt) as f64).fract();
                    if md.unipolar {
                        0.5 + 0.5 * y
                    } else {
                        y
                    }
                }
                ModKind::Random => {
                    let f = hz(md.rate, md.sync);
                    if md.retrig && new_note {
                        rt.phase = 0.999_999;
                    }
                    rt.phase += (f * dt) as f64;
                    if rt.phase >= 1.0 {
                        rt.phase = rt.phase.fract();
                        rt.rnd_from = rt.rnd;
                        rt.rnd_to = rand01(&mut rt.rng) * 2.0 - 1.0;
                    }
                    let g = md.smooth.clamp(0.0, 1.0);
                    let t = if g <= 0.0 {
                        1.0
                    } else {
                        (rt.phase as f32 / g).min(1.0)
                    };
                    rt.rnd = rt.rnd_from + (rt.rnd_to - rt.rnd_from) * t;
                    if md.unipolar {
                        0.5 + 0.5 * rt.rnd
                    } else {
                        rt.rnd
                    }
                }
                ModKind::Envelope => {
                    if new_note {
                        rt.stage = 1;
                    } else if !held && (rt.stage == 1 || rt.stage == 2) {
                        rt.stage = 3;
                    }
                    let s = md.s.clamp(0.0, 1.0);
                    match rt.stage {
                        1 => {
                            rt.env += dt / md.a.max(0.0005);
                            if rt.env >= 1.0 {
                                rt.env = 1.0;
                                rt.stage = 2;
                            }
                        }
                        2 => {
                            let k = 1.0 - (-dt / (md.d.max(0.0005) / 4.0)).exp();
                            rt.env += (s - rt.env) * k;
                        }
                        3 => {
                            let k = 1.0 - (-dt / (md.r.max(0.0005) / 4.0)).exp();
                            rt.env -= rt.env * k;
                            if rt.env < 1e-4 {
                                rt.env = 0.0;
                                rt.stage = 0;
                            }
                        }
                        _ => {}
                    }
                    rt.env
                }
                ModKind::Velocity | ModKind::Keytrack | ModKind::ModWheel | ModKind::Pressure => {
                    let x = match md.kind {
                        ModKind::Velocity => self.feed.vel[o],
                        ModKind::Keytrack => self.feed.key[o],
                        ModKind::Pressure => self.feed.press[o],
                        _ => self.feed.wheel,
                    };
                    let g = md.smooth.clamp(0.0, 1.0);
                    if g <= 0.0 {
                        rt.smooth = x;
                    } else {
                        let k = 1.0 - (-dt / (g * 0.5)).exp();
                        rt.smooth += (x - rt.smooth) * k;
                    }
                    rt.smooth
                }
                ModKind::Macro => md.value.clamp(0.0, 1.0),
            };
            self.out[k] = if v.is_finite() { v } else { 0.0 };
        }
    }
}

#[cfg(test)]
mod tests;
