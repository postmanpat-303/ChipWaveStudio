//! The **Grid** (Unreleased): a modular, patchable generator (and FX device).
//!
//! A patch (`GridP`) is a list of modules (oscillators incl. wavetable frames,
//! noise, filters, envelopes, LFOs, math / mix / logic, delay, shaper, note
//! and audio inputs, outputs) and the cables between their ports. It is
//! polyphonic: every voice runs its own copy of the graph (`GridState`, fixed
//! size, inside the voice's `GenState`).
//!
//! **Audio thread:** at the first sample (and whenever the patch structure
//! changes, checked every 64 samples by a fingerprint) the graph is compiled
//! into a fixed `Plan`: modules in topological order with their input sources
//! resolved to indices. Compiling and running use only fixed-size arrays, so
//! nothing is allocated. A cable that closes a loop reads the previous
//! sample's value (a one-sample feedback delay).
//!
//! **Knobs:** module parameters live in the 64 generator knob slots
//! (`GridP::p`, 0..1). Slots 0-7 are the Grid's eight macros (43xy, read by
//! Macro modules); each module takes free slots from 8 up when added, and
//! keeps them, so automation lanes (`instrument.N.gen.K`), MIDI learn and
//! drag-to-modulate routes stay attached when other modules come and go.
//!
//! Written for ChipWave; the idea follows other DAWs' modular environments (no code or
//! assets).
use crate::gen::{MacroDef, Ov, GEN_KNOBS};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

pub const MAX_MODULES: usize = 32;
pub const MAX_CABLES: usize = 128;
pub const MAX_IN: usize = 4;
pub const MAX_OUT: usize = 3;
pub const MAX_P: usize = 4;
/// knob slots 0..MACROS are the Grid macros
pub const MACROS: usize = 8;
/// samples per Delay module line
pub const DELAY_LEN: usize = 4096;
pub const MAX_DELAYS: usize = 2;
pub const MAX_LADDERS: usize = 4;
pub const NONE: u8 = 255;
pub const C4: f32 = 261.625_58;
/// knob values are re-read every CR samples (control rate)
const CR: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Note,
    Macro,
    AudioIn,
    Osc,
    Wavetable,
    Noise,
    Filter,
    Ladder,
    Adsr,
    Lfo,
    Mix,
    Vca,
    Math,
    Logic,
    Slew,
    SampleHold,
    Shaper,
    Delay,
    Knob,
    Out,
}

/// how an unconnected input reads
#[derive(Clone, Copy, PartialEq)]
pub enum Def {
    Zero,
    One,
    /// the note's pitch (octaves from C-4)
    Pitch,
    /// the note gate (1 held, 0 released)
    Gate,
    /// white noise
    Noise,
}

pub struct Spec {
    pub name: &'static str,
    pub cat: &'static str,
    pub tip: &'static str,
    pub ins: &'static [(&'static str, Def)],
    pub outs: &'static [&'static str],
    pub params: &'static [MacroDef],
    /// default real values of `params`
    pub defaults: &'static [f32],
    /// discrete modes (a menu on the module; not automatable)
    pub modes: &'static [&'static str],
}

const fn m(
    name: &'static str,
    lo: f32,
    hi: f32,
    exp: bool,
    unit: &'static str,
    tip: &'static str,
) -> MacroDef {
    MacroDef {
        name,
        lo,
        hi,
        exp,
        unit,
        dice: (0.0, 1.0),
        tip,
    }
}

use Def::*;
const TUNE: MacroDef = m("Tune", -24.0, 24.0, false, "st", "transpose in semitones");
const FINE: MacroDef = m("Fine", -100.0, 100.0, false, "ct", "fine tune in cents");
const CUT: MacroDef = m("Cutoff", 20.0, 20000.0, true, "Hz", "cutoff frequency");
const RES: MacroDef = m("Res", 0.0, 1.0, false, "", "resonance");
const FMOD: MacroDef = m(
    "Mod",
    -8.0,
    8.0,
    false,
    "oct",
    "octaves of cutoff change per unit on the Cutoff input",
);
const KEY: MacroDef = m("Key", 0.0, 1.0, false, "", "cutoff follows the note");

pub const KINDS: [Kind; 20] = [
    Kind::Note,
    Kind::Macro,
    Kind::AudioIn,
    Kind::Osc,
    Kind::Wavetable,
    Kind::Noise,
    Kind::Filter,
    Kind::Ladder,
    Kind::Adsr,
    Kind::Lfo,
    Kind::Mix,
    Kind::Vca,
    Kind::Math,
    Kind::Logic,
    Kind::Slew,
    Kind::SampleHold,
    Kind::Shaper,
    Kind::Delay,
    Kind::Knob,
    Kind::Out,
];

pub fn spec(k: Kind) -> &'static Spec {
    match k {
        Kind::Note => {
            const S: Spec = Spec {
                name: "Note",
                cat: "In / Out",
                tip: "the voice's note: pitch (octaves from C-4), gate and velocity",
                ins: &[],
                outs: &["Pitch", "Gate", "Vel"],
                params: &[],
                defaults: &[],
                modes: &[],
            };
            &S
        }
        Kind::Macro => {
            const S: Spec = Spec {
                name: "Macro",
                cat: "In / Out",
                tip: "one of the Grid's 8 macro knobs (43xy, automation, modulation), 0..1",
                ins: &[],
                outs: &["Out"],
                params: &[],
                defaults: &[],
                modes: &["M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8"],
            };
            &S
        }
        Kind::AudioIn => {
            const S: Spec = Spec {
                name: "Audio In",
                cat: "In / Out",
                tip: "the audio coming into a Grid FX device (silent on an instrument)",
                ins: &[],
                outs: &["L", "R"],
                params: &[],
                defaults: &[],
                modes: &[],
            };
            &S
        }
        Kind::Out => {
            const S: Spec = Spec {
                name: "Out",
                cat: "In / Out",
                tip: "the Grid's output (R follows L when unconnected)",
                ins: &[("L", Zero), ("R", Zero)],
                outs: &[],
                params: &[m("Level", 0.0, 2.0, false, "", "output level")],
                defaults: &[1.0],
                modes: &[],
            };
            &S
        }
        Kind::Osc => {
            const S: Spec = Spec {
                name: "Osc",
                cat: "Sources",
                tip: "band-limited oscillator (polyBLEP); PM = phase modulation in cycles",
                ins: &[("Pitch", Pitch), ("PM", Zero), ("PW", Zero)],
                outs: &["Out"],
                params: &[
                    TUNE,
                    FINE,
                    m("PW", 0.05, 0.95, false, "", "pulse width (Pulse)"),
                    m("PM", 0.0, 4.0, false, "x", "phase-modulation depth"),
                ],
                defaults: &[0.0, 0.0, 0.5, 1.0],
                modes: &["Sine", "Triangle", "Saw", "Square", "Pulse"],
            };
            &S
        }
        Kind::Wavetable => {
            const S: Spec = Spec {
                name: "Wavetable",
                cat: "Sources",
                tip: "plays a built-in wavetable (band-limited mipmaps); Pos scans the frames",
                ins: &[("Pitch", Pitch), ("Pos", Zero)],
                outs: &["Out"],
                params: &[
                    TUNE,
                    FINE,
                    m("Pos", 0.0, 1.0, false, "", "table position"),
                    m("Pos mod", -1.0, 1.0, false, "", "Pos input depth"),
                ],
                defaults: &[0.0, 0.0, 0.0, 1.0],
                modes: &crate::wavetable::BUILTIN,
            };
            &S
        }
        Kind::Noise => {
            const S: Spec = Spec {
                name: "Noise",
                cat: "Sources",
                tip: "white noise, darker with Colour",
                ins: &[],
                outs: &["Out"],
                params: &[m(
                    "Colour",
                    0.0,
                    1.0,
                    false,
                    "",
                    "0 white, 1 dark (low-passed)",
                )],
                defaults: &[0.0],
                modes: &[],
            };
            &S
        }
        Kind::Filter => {
            const S: Spec = Spec {
                name: "Filter",
                cat: "Filters",
                tip: "state-variable filter (TPT); the Cutoff input adds octaves times Mod",
                ins: &[("In", Zero), ("Cutoff", Zero)],
                outs: &["Out"],
                params: &[CUT, RES, FMOD, KEY],
                defaults: &[2000.0, 0.2, 2.0, 0.0],
                modes: &["Low-pass", "Band-pass", "High-pass", "Notch"],
            };
            &S
        }
        Kind::Ladder => {
            const S: Spec = Spec {
                name: "Ladder",
                cat: "Filters",
                tip:
                    "ChipWave's Moog-style ladder (clean); the Cutoff input adds octaves times Mod",
                ins: &[("In", Zero), ("Cutoff", Zero)],
                outs: &["Out"],
                params: &[CUT, RES, FMOD, KEY],
                defaults: &[1200.0, 0.3, 3.0, 0.0],
                modes: &["LP 24", "LP 12", "Band-pass", "High-pass"],
            };
            &S
        }
        Kind::Adsr => {
            const S: Spec = Spec {
                name: "ADSR",
                cat: "Modulators",
                tip: "envelope 0..1, triggered by its Gate input (the note gate when unconnected)",
                ins: &[("Gate", Gate)],
                outs: &["Env"],
                params: &[
                    m("Attack", 0.001, 4.0, true, "s", "attack time"),
                    m("Decay", 0.005, 4.0, true, "s", "decay time"),
                    m("Sustain", 0.0, 1.0, false, "", "sustain level"),
                    m("Release", 0.005, 6.0, true, "s", "release time"),
                ],
                defaults: &[0.003, 0.3, 0.6, 0.25],
                modes: &[],
            };
            &S
        }
        Kind::Lfo => {
            const S: Spec = Spec {
            name: "LFO",
            cat: "Modulators",
            tip: "low-frequency oscillator, -1..1, restarts with every note; Rate input in octaves",
            ins: &[("Rate", Zero)],
            outs: &["Out"],
            params: &[
                m("Rate", 0.02, 40.0, true, "Hz", "speed"),
                m("Phase", 0.0, 1.0, false, "", "start phase"),
            ],
            defaults: &[2.0, 0.0],
            modes: &["Sine", "Triangle", "Saw", "Square", "Random"],
            };
            &S
        }
        Kind::Mix => {
            const S: Spec = Spec {
                name: "Mix",
                cat: "Math",
                tip: "sums three inputs with their levels",
                ins: &[("A", Zero), ("B", Zero), ("C", Zero)],
                outs: &["Out"],
                params: &[
                    m("A", -1.0, 1.0, false, "", "level of A"),
                    m("B", -1.0, 1.0, false, "", "level of B"),
                    m("C", -1.0, 1.0, false, "", "level of C"),
                ],
                defaults: &[1.0, 1.0, 1.0],
                modes: &[],
            };
            &S
        }
        Kind::Vca => {
            const S: Spec = Spec {
                name: "VCA",
                cat: "Math",
                tip: "In × CV × Gain (CV is 1 when unconnected)",
                ins: &[("In", Zero), ("CV", One)],
                outs: &["Out"],
                params: &[m("Gain", 0.0, 2.0, false, "", "gain")],
                defaults: &[1.0],
                modes: &[],
            };
            &S
        }
        Kind::Math => {
            const S: Spec = Spec {
                name: "Math",
                cat: "Math",
                tip: "combines A and B, then × Scale + Offset",
                ins: &[("A", Zero), ("B", Zero)],
                outs: &["Out"],
                params: &[
                    m("Scale", -4.0, 4.0, false, "x", "multiplier"),
                    m("Offset", -2.0, 2.0, false, "v", "added afterwards"),
                ],
                defaults: &[1.0, 0.0],
                modes: &["A", "A + B", "A - B", "A × B", "|A|", "min", "max"],
            };
            &S
        }
        Kind::Logic => {
            const S: Spec = Spec {
            name: "Logic",
            cat: "Math",
            tip: "gates: 1 or 0. Compare A with B (B = Threshold when unconnected), or AND/OR/XOR of A and B above Threshold",
            ins: &[("A", Zero), ("B", Zero)],
            outs: &["Out"],
            params: &[m("Threshold", -1.0, 1.0, false, "v", "the level counted as on")],
            defaults: &[0.5],
            modes: &["A > B", "A < B", "AND", "OR", "XOR", "NOT A"],
            };
            &S
        }
        Kind::Slew => {
            const S: Spec = Spec {
                name: "Slew",
                cat: "Math",
                tip: "lag / glide: smooths its input with separate rise and fall times",
                ins: &[("In", Zero)],
                outs: &["Out"],
                params: &[
                    m("Rise", 0.0005, 4.0, true, "s", "rise time"),
                    m("Fall", 0.0005, 4.0, true, "s", "fall time"),
                ],
                defaults: &[0.05, 0.05],
                modes: &[],
            };
            &S
        }
        Kind::SampleHold => {
            const S: Spec = Spec {
            name: "S&H",
            cat: "Math",
            tip: "samples In (noise when unconnected) on each rising Trig (the note gate when unconnected)",
            ins: &[("In", Noise), ("Trig", Gate)],
            outs: &["Out"],
            params: &[],
            defaults: &[],
            modes: &[],
            };
            &S
        }
        Kind::Shaper => {
            const S: Spec = Spec {
                name: "Shaper",
                cat: "FX",
                tip: "distortion: soft (tanh), fold or hard clip",
                ins: &[("In", Zero)],
                outs: &["Out"],
                params: &[
                    m(
                        "Drive",
                        0.0,
                        1.0,
                        false,
                        "",
                        "gain into the shaper (1x-20x)",
                    ),
                    m("Mix", 0.0, 1.0, false, "", "dry / wet"),
                ],
                defaults: &[0.3, 1.0],
                modes: &["Soft", "Fold", "Hard"],
            };
            &S
        }
        Kind::Delay => {
            const S: Spec = Spec {
            name: "Delay",
            cat: "FX",
            tip: "short delay / comb (Karplus-Strong with Damp); the Time input in octaves shortens it (connect Pitch to tune it)",
            ins: &[("In", Zero), ("Time", Zero)],
            outs: &["Out"],
            params: &[
                m("Time", 0.1, 80.0, true, "ms", "delay time (up to 4 096 samples)"),
                m("Feedback", -0.99, 0.99, false, "", "feedback"),
                m("Mix", 0.0, 1.0, false, "", "dry / wet"),
                m("Damp", 0.0, 1.0, false, "", "low-pass in the feedback"),
            ],
            defaults: &[10.0, 0.5, 0.5, 0.2],
            modes: &[],
            };
            &S
        }
        Kind::Knob => {
            const S: Spec = Spec {
                name: "Knob",
                cat: "Math",
                tip: "a constant set by its knob (automate or modulate it)",
                ins: &[],
                outs: &["Out"],
                params: &[m("Value", -1.0, 1.0, false, "v", "the output value")],
                defaults: &[0.0],
                modes: &[],
            };
            &S
        }
    }
}

/// the macro knobs (slots 0-7)
pub const MACRO_DEFS: [MacroDef; MACROS] = [
    m(
        "Macro 1",
        0.0,
        1.0,
        false,
        "",
        "Grid macro 1 (read by Macro modules)",
    ),
    m("Macro 2", 0.0, 1.0, false, "", "Grid macro 2"),
    m("Macro 3", 0.0, 1.0, false, "", "Grid macro 3"),
    m("Macro 4", 0.0, 1.0, false, "", "Grid macro 4"),
    m("Macro 5", 0.0, 1.0, false, "", "Grid macro 5"),
    m("Macro 6", 0.0, 1.0, false, "", "Grid macro 6"),
    m("Macro 7", 0.0, 1.0, false, "", "Grid macro 7"),
    m("Macro 8", 0.0, 1.0, false, "", "Grid macro 8"),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub id: u16,
    pub kind: Kind,
    #[serde(default)]
    pub mode: u8,
    /// canvas position
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// knob slot of each parameter (`NONE` = unused)
    #[serde(default = "no_slots")]
    pub slots: [u8; MAX_P],
}

fn no_slots() -> [u8; MAX_P] {
    [NONE; MAX_P]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cable {
    pub from: u16,
    pub out: u8,
    pub to: u16,
    pub inp: u8,
}

/// A Grid patch (instrument engine or FX device).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridP {
    pub modules: Vec<Module>,
    pub cables: Vec<Cable>,
    /// the 64 knob slots, normalised 0..1
    pub p: Vec<f32>,
    pub locks: u64,
    pub level: f32,
}

impl Default for GridP {
    fn default() -> Self {
        let mut g = GridP::empty();
        // Note -> Osc (saw) -> Filter -> VCA (ADSR) -> Out
        let n = g.add(Kind::Note, 20.0, 40.0).unwrap();
        let o = g.add(Kind::Osc, 200.0, 20.0).unwrap();
        g.set_mode(o, 2);
        let f = g.add(Kind::Filter, 380.0, 20.0).unwrap();
        let e = g.add(Kind::Adsr, 380.0, 230.0).unwrap();
        let v = g.add(Kind::Vca, 560.0, 60.0).unwrap();
        let out = g.add(Kind::Out, 740.0, 60.0).unwrap();
        g.connect(n, 0, o, 0);
        g.connect(o, 0, f, 0);
        g.connect(f, 0, v, 0);
        g.connect(e, 0, v, 1);
        g.connect(v, 0, out, 0);
        g
    }
}

impl GridP {
    pub fn empty() -> Self {
        GridP {
            modules: vec![],
            cables: vec![],
            p: vec![0.0; GEN_KNOBS],
            locks: 0,
            level: 1.0,
        }
    }
    pub fn module(&self, id: u16) -> Option<&Module> {
        self.modules.iter().find(|m| m.id == id)
    }
    fn used_slots(&self) -> u64 {
        let mut u = (1u64 << MACROS) - 1;
        for m in &self.modules {
            for s in m.slots {
                if (s as usize) < GEN_KNOBS {
                    u |= 1 << s;
                }
            }
        }
        u
    }
    /// add a module (None: too many modules or no free knob slots)
    pub fn add(&mut self, kind: Kind, x: f32, y: f32) -> Option<u16> {
        if self.modules.len() >= MAX_MODULES {
            return None;
        }
        let sp = spec(kind);
        let mut used = self.used_slots();
        let mut slots = [NONE; MAX_P];
        for (j, s) in slots.iter_mut().enumerate().take(sp.params.len()) {
            let free = (0..GEN_KNOBS).find(|k| used & (1 << k) == 0)?;
            used |= 1 << free;
            *s = free as u8;
            self.p[free] = sp.params[j].unmap(sp.defaults[j]);
            self.locks &= !(1 << free);
        }
        let id = self.modules.iter().map(|m| m.id + 1).max().unwrap_or(1);
        self.modules.push(Module {
            id,
            kind,
            mode: 0,
            x,
            y,
            slots,
        });
        Some(id)
    }
    pub fn remove(&mut self, id: u16) {
        self.modules.retain(|m| m.id != id);
        self.cables.retain(|c| c.from != id && c.to != id);
    }
    pub fn set_mode(&mut self, id: u16, mode: u8) {
        if let Some(m) = self.modules.iter_mut().find(|m| m.id == id) {
            m.mode = mode.min(spec(m.kind).modes.len().saturating_sub(1) as u8);
        }
    }
    /// set parameter `j` of module `id` to a real value
    pub fn set(&mut self, id: u16, j: usize, real: f32) {
        if let Some(m) = self.module(id) {
            let s = m.slots[j.min(MAX_P - 1)];
            if let (Some(d), Some(v)) = (spec(m.kind).params.get(j), self.p.get_mut(s as usize)) {
                *v = d.unmap(real);
            }
        }
    }
    /// cable `from:out` -> `to:inp` (replaces the input's cable); false = invalid
    pub fn connect(&mut self, from: u16, out: u8, to: u16, inp: u8) -> bool {
        let (Some(a), Some(b)) = (self.module(from), self.module(to)) else {
            return false;
        };
        if out as usize >= spec(a.kind).outs.len() || inp as usize >= spec(b.kind).ins.len() {
            return false;
        }
        self.cables.retain(|c| !(c.to == to && c.inp == inp));
        if self.cables.len() >= MAX_CABLES {
            return false;
        }
        self.cables.push(Cable { from, out, to, inp });
        true
    }
    pub fn disconnect(&mut self, to: u16, inp: u8) {
        self.cables.retain(|c| !(c.to == to && c.inp == inp));
    }
    /// keep a loaded patch within the engine's limits
    pub fn fix(&mut self) {
        self.p.resize(GEN_KNOBS, 0.5);
        for v in self.p.iter_mut() {
            *v = if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.5
            };
        }
        self.modules.truncate(MAX_MODULES);
        // unique ids, valid modes and slots (a slot used twice is dropped)
        let mut seen: Vec<u16> = vec![];
        let mut used = (1u64 << MACROS) - 1;
        let mut next = self.modules.iter().map(|m| m.id).max().unwrap_or(0);
        for m in self.modules.iter_mut() {
            if m.id == 0 || seen.contains(&m.id) {
                next += 1;
                m.id = next;
            }
            seen.push(m.id);
            let sp = spec(m.kind);
            m.mode = m.mode.min(sp.modes.len().saturating_sub(1) as u8);
            if !m.x.is_finite() {
                m.x = 0.0;
            }
            if !m.y.is_finite() {
                m.y = 0.0;
            }
            for j in 0..MAX_P {
                let s = m.slots[j] as usize;
                let ok = j < sp.params.len() && s < GEN_KNOBS && used & (1 << s) == 0;
                if ok {
                    used |= 1 << s;
                } else {
                    m.slots[j] = NONE;
                }
            }
        }
        // params that lost their slot get a free one back (at the default)
        for m in self.modules.iter_mut() {
            let sp = spec(m.kind);
            for j in 0..sp.params.len() {
                if m.slots[j] == NONE {
                    if let Some(f) = (MACROS..GEN_KNOBS).find(|k| used & (1 << k) == 0) {
                        used |= 1 << f;
                        m.slots[j] = f as u8;
                        self.p[f] = sp.params[j].unmap(sp.defaults[j]);
                    }
                }
            }
        }
        let mods = &self.modules;
        let valid = |c: &Cable| {
            let a = mods.iter().find(|m| m.id == c.from);
            let b = mods.iter().find(|m| m.id == c.to);
            matches!((a, b), (Some(a), Some(b))
                if (c.out as usize) < spec(a.kind).outs.len()
                    && (c.inp as usize) < spec(b.kind).ins.len())
        };
        let mut cs: Vec<Cable> = vec![];
        for c in self.cables.iter().filter(|c| valid(c)) {
            cs.retain(|x| !(x.to == c.to && x.inp == c.inp));
            cs.push(*c);
        }
        cs.truncate(MAX_CABLES);
        self.cables = cs;
        self.level = if self.level.is_finite() {
            self.level.clamp(0.0, 4.0)
        } else {
            1.0
        };
    }
    /// the module and parameter on knob slot `k`
    pub fn slot_owner(&self, k: usize) -> Option<(&Module, usize)> {
        self.modules.iter().find_map(|m| {
            m.slots
                .iter()
                .position(|s| *s as usize == k)
                .filter(|j| *j < spec(m.kind).params.len())
                .map(|j| (m, j))
        })
    }
    pub fn knob_def(&self, k: usize) -> Option<&'static MacroDef> {
        if k < MACROS {
            return MACRO_DEFS.get(k);
        }
        self.slot_owner(k).map(|(m, j)| &spec(m.kind).params[j])
    }
    /// "Osc 2" (the module's kind and its number among that kind)
    pub fn label(&self, id: u16) -> String {
        let Some(m) = self.module(id) else {
            return "?".into();
        };
        let n = self
            .modules
            .iter()
            .filter(|x| x.kind == m.kind && x.id <= m.id)
            .count();
        format!("{} {n}", spec(m.kind).name)
    }
    /// every knob: (slot, "Osc 1: Tune"); the 8 macros first
    pub fn knobs(&self) -> Vec<(usize, String)> {
        let mut v: Vec<(usize, String)> = (0..MACROS)
            .map(|k| (k, MACRO_DEFS[k].name.to_string()))
            .collect();
        for m in &self.modules {
            let sp = spec(m.kind);
            for (j, s) in m.slots.iter().enumerate().take(sp.params.len()) {
                if (*s as usize) < GEN_KNOBS {
                    v.push((
                        *s as usize,
                        format!("{}: {}", self.label(m.id), sp.params[j].name),
                    ));
                }
            }
        }
        v
    }
    pub fn dice(&mut self, seed: u32) {
        let mut r = crate::gen::Rng::new(seed);
        for (k, _) in self.knobs() {
            let x = r.f();
            if self.locks & (1 << k) == 0 && k >= MACROS {
                // a gentle roll: within ±25 % of the current value
                self.p[k] = (self.p[k] + (x - 0.5) * 0.5).clamp(0.0, 1.0);
            }
        }
    }
    pub fn mutate(&mut self, a: f32, seed: u32) {
        let mut r = crate::gen::Rng::new(seed);
        for (k, _) in self.knobs() {
            let x = r.f() * 2.0 - 1.0;
            if self.locks & (1 << k) == 0 {
                self.p[k] = (self.p[k] + x * a).clamp(0.0, 1.0);
            }
        }
    }
    /// structure fingerprint (modules, modes, cables; not positions or knobs)
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x0100_0000_01b3);
        };
        mix(self.modules.len() as u64);
        for m in &self.modules {
            mix(m.id as u64 | (m.kind as u64) << 16 | (m.mode as u64) << 24);
            for s in m.slots {
                mix(s as u64);
            }
        }
        mix(self.cables.len() as u64 | 1 << 40);
        for c in &self.cables {
            mix(c.from as u64 | (c.out as u64) << 16 | (c.to as u64) << 24 | (c.inp as u64) << 40);
        }
        h
    }
}

// ------------------------------------------------------------------ presets

/// a little patch builder for the presets
struct B(GridP);
impl B {
    fn m(&mut self, k: Kind, col: f32, row: f32, mode: u8, vals: &[(usize, f32)]) -> u16 {
        let id = self
            .0
            .add(k, 20.0 + col * 185.0, 20.0 + row * 120.0)
            .unwrap();
        self.0.set_mode(id, mode);
        for (j, v) in vals {
            self.0.set(id, *j, *v);
        }
        id
    }
    fn c(&mut self, a: u16, o: u8, b: u16, i: u8) {
        assert!(self.0.connect(a, o, b, i));
    }
}

/// factory patches: (name, tip, patch, glide, legato)
pub fn presets() -> Vec<(&'static str, &'static str, GridP, f32, bool)> {
    use Kind::*;
    let mut v = vec![];
    // ---- Saw Bass: 2 oscillators -> ladder swept by an envelope (+ Macro 1)
    let mut b = B(GridP::empty());
    let n = b.m(Note, 0.0, 0.0, 0, &[]);
    let o1 = b.m(Osc, 1.0, 0.0, 2, &[(0, -12.0)]);
    let o2 = b.m(Osc, 1.0, 2.0, 3, &[(0, -12.0), (1, 7.0)]);
    let mx = b.m(Mix, 2.0, 0.0, 0, &[(0, 0.6), (1, 0.4)]);
    let fe = b.m(
        Adsr,
        2.0,
        2.0,
        0,
        &[(0, 0.001), (1, 0.25), (2, 0.15), (3, 0.15)],
    );
    let mc = b.m(Macro, 2.0, 4.5, 0, &[]);
    let sum = b.m(Math, 3.0, 2.5, 1, &[]);
    let lad = b.m(
        Ladder,
        3.0,
        0.0,
        0,
        &[(0, 180.0), (1, 0.45), (2, 4.5), (3, 0.5)],
    );
    let ae = b.m(
        Adsr,
        4.0,
        2.0,
        0,
        &[(0, 0.002), (1, 0.4), (2, 0.7), (3, 0.08)],
    );
    let vca = b.m(Vca, 4.0, 0.0, 0, &[]);
    let out = b.m(Out, 5.0, 0.0, 0, &[(0, 0.9)]);
    b.c(n, 0, o1, 0);
    b.c(n, 0, o2, 0);
    b.c(o1, 0, mx, 0);
    b.c(o2, 0, mx, 1);
    b.c(mx, 0, lad, 0);
    b.c(fe, 0, sum, 0);
    b.c(mc, 0, sum, 1);
    b.c(sum, 0, lad, 1);
    b.c(lad, 0, vca, 0);
    b.c(ae, 0, vca, 1);
    b.c(vca, 0, out, 0);
    b.0.p[0] = 0.25; // Macro 1 opens the filter
    v.push((
        "Grid Saw Bass",
        "Grid: saw + square through the ladder, an envelope and Macro 1 on the cutoff",
        b.0,
        0.02,
        true,
    ));
    // ---- Wavetable Pad: an LFO scans the table, a short delay widens it
    let mut b = B(GridP::empty());
    let n = b.m(Note, 0.0, 0.0, 0, &[]);
    let lfo = b.m(Lfo, 0.0, 2.0, 0, &[(0, 0.15)]);
    let w1 = b.m(Wavetable, 1.0, 0.0, 2, &[(2, 0.25), (3, 0.3)]);
    let w2 = b.m(Wavetable, 1.0, 2.5, 3, &[(1, 9.0), (2, 0.4), (3, -0.3)]);
    let mx = b.m(Mix, 2.0, 0.0, 0, &[(0, 0.5), (1, 0.5)]);
    let f = b.m(
        Filter,
        3.0,
        0.0,
        0,
        &[(0, 2600.0), (1, 0.15), (2, 1.0), (3, 0.3)],
    );
    let ae = b.m(Adsr, 3.0, 2.5, 0, &[(0, 1.2), (1, 1.0), (2, 0.8), (3, 2.0)]);
    let vca = b.m(Vca, 4.0, 0.0, 0, &[]);
    let d = b.m(
        Delay,
        4.0,
        2.5,
        0,
        &[(0, 17.0), (1, 0.35), (2, 0.6), (3, 0.3)],
    );
    let out = b.m(Out, 5.0, 0.0, 0, &[(0, 0.8)]);
    b.c(n, 0, w1, 0);
    b.c(n, 0, w2, 0);
    b.c(lfo, 0, w1, 1);
    b.c(lfo, 0, w2, 1);
    b.c(lfo, 0, f, 1);
    b.c(w1, 0, mx, 0);
    b.c(w2, 0, mx, 1);
    b.c(mx, 0, f, 0);
    b.c(f, 0, vca, 0);
    b.c(ae, 0, vca, 1);
    b.c(vca, 0, out, 0);
    b.c(vca, 0, d, 0);
    b.c(d, 0, out, 1);
    v.push((
        "Grid Wavetable Pad",
        "Grid: two wavetables scanned by an LFO, filter, slow envelope, a delay on the right",
        b.0,
        0.0,
        false,
    ));
    // ---- FM Bell: a modulator through a decaying VCA into the carrier's PM
    let mut b = B(GridP::empty());
    let n = b.m(Note, 0.0, 0.0, 0, &[]);
    let md = b.m(Osc, 1.0, 0.0, 0, &[(0, 19.0), (1, 2.0)]);
    let me = b.m(
        Adsr,
        1.0,
        2.0,
        0,
        &[(0, 0.001), (1, 1.2), (2, 0.0), (3, 0.8)],
    );
    let mv = b.m(Vca, 2.0, 0.0, 0, &[(0, 1.0)]);
    let car = b.m(Osc, 3.0, 0.0, 0, &[(3, 1.2)]);
    let ae = b.m(
        Adsr,
        3.0,
        2.5,
        0,
        &[(0, 0.001), (1, 2.5), (2, 0.0), (3, 1.5)],
    );
    let vca = b.m(Vca, 4.0, 0.0, 0, &[]);
    let out = b.m(Out, 5.0, 0.0, 0, &[(0, 0.8)]);
    b.c(n, 0, md, 0);
    b.c(n, 0, car, 0);
    b.c(md, 0, mv, 0);
    b.c(me, 0, mv, 1);
    b.c(mv, 0, car, 1);
    b.c(car, 0, vca, 0);
    b.c(ae, 0, vca, 1);
    b.c(vca, 0, out, 0);
    v.push((
        "Grid FM Bell",
        "Grid: 2-operator FM, the modulator's depth decays (ratio 3)",
        b.0,
        0.0,
        false,
    ));
    // ---- Karplus Pluck: a noise burst into a delay tuned by the note
    let mut b = B(GridP::empty());
    let n = b.m(Note, 0.0, 0.0, 0, &[]);
    let nz = b.m(Noise, 0.0, 2.0, 0, &[(0, 0.3)]);
    let be = b.m(
        Adsr,
        1.0,
        2.0,
        0,
        &[(0, 0.001), (1, 0.015), (2, 0.0), (3, 0.01)],
    );
    let bv = b.m(Vca, 1.0, 0.0, 0, &[]);
    let d = b.m(
        Delay,
        2.0,
        0.0,
        0,
        &[(0, 1000.0 / C4), (1, 0.985), (2, 1.0), (3, 0.35)],
    );
    let f = b.m(Filter, 3.0, 0.0, 2, &[(0, 60.0), (1, 0.0)]);
    let out = b.m(Out, 4.0, 0.0, 0, &[(0, 1.0)]);
    b.c(nz, 0, bv, 0);
    b.c(be, 0, bv, 1);
    b.c(bv, 0, d, 0);
    b.c(n, 0, d, 1);
    b.c(d, 0, f, 0);
    b.c(f, 0, out, 0);
    v.push((
        "Grid Karplus Pluck",
        "Grid: Karplus-Strong string: a noise burst rings in a delay tuned by the note's pitch",
        b.0,
        0.0,
        false,
    ));
    // ---- S&H Lead: random cutoff steps from a sample & hold on an LFO
    let mut b = B(GridP::empty());
    let n = b.m(Note, 0.0, 0.0, 0, &[]);
    let o1 = b.m(Osc, 1.0, 0.0, 3, &[]);
    let o2 = b.m(Osc, 1.0, 2.0, 2, &[(0, 7.0), (1, 6.0)]);
    let mx = b.m(Mix, 2.0, 0.0, 0, &[(0, 0.5), (1, 0.4)]);
    let lfo = b.m(Lfo, 0.0, 2.5, 3, &[(0, 7.0)]);
    let sh = b.m(SampleHold, 1.0, 4.0, 0, &[]);
    let sl = b.m(Slew, 2.0, 2.5, 0, &[(0, 0.01), (1, 0.03)]);
    let f = b.m(
        Filter,
        3.0,
        0.0,
        0,
        &[(0, 1400.0), (1, 0.55), (2, 1.8), (3, 0.5)],
    );
    let ae = b.m(
        Adsr,
        3.0,
        2.5,
        0,
        &[(0, 0.005), (1, 0.3), (2, 0.8), (3, 0.2)],
    );
    let vca = b.m(Vca, 4.0, 0.0, 0, &[]);
    let out = b.m(Out, 5.0, 0.0, 0, &[(0, 0.7)]);
    b.c(n, 0, o1, 0);
    b.c(n, 0, o2, 0);
    b.c(o1, 0, mx, 0);
    b.c(o2, 0, mx, 1);
    b.c(lfo, 0, sh, 1);
    b.c(sh, 0, sl, 0);
    b.c(sl, 0, f, 1);
    b.c(mx, 0, f, 0);
    b.c(f, 0, vca, 0);
    b.c(ae, 0, vca, 1);
    b.c(vca, 0, out, 0);
    v.push((
        "Grid S&H Lead",
        "Grid: square + saw, a sample & hold of noise steps the filter 7 times a second",
        b.0,
        0.03,
        false,
    ));
    // ---- Chip Gate: logic chops a pulse into a trance gate while the note is held
    let mut b = B(GridP::empty());
    let n = b.m(Note, 0.0, 0.0, 0, &[]);
    let lfo = b.m(Lfo, 0.0, 2.0, 3, &[(0, 12.0)]);
    let lg = b.m(Logic, 1.0, 2.0, 2, &[(0, 0.5)]);
    let ae = b.m(
        Adsr,
        2.0,
        2.0,
        0,
        &[(0, 0.001), (1, 0.06), (2, 0.0), (3, 0.03)],
    );
    let o = b.m(Osc, 1.0, 0.0, 4, &[(2, 0.25)]);
    let sh = b.m(Shaper, 2.0, 0.0, 0, &[(0, 0.15), (1, 0.6)]);
    let vca = b.m(Vca, 3.0, 0.0, 0, &[]);
    let out = b.m(Out, 4.0, 0.0, 0, &[(0, 0.6)]);
    b.c(n, 0, o, 0);
    b.c(lfo, 0, lg, 0);
    b.c(n, 1, lg, 1);
    b.c(lg, 0, ae, 0);
    b.c(o, 0, sh, 0);
    b.c(sh, 0, vca, 0);
    b.c(ae, 0, vca, 1);
    b.c(vca, 0, out, 0);
    v.push((
        "Grid Chip Gate",
        "Grid: a pulse chopped at 12 Hz by an LFO AND the note gate into a short envelope",
        b.0,
        0.0,
        false,
    ));
    v
}

/// Grid FX patches: (name, tip, patch); the first is a new device's
pub fn fx_presets() -> Vec<(&'static str, &'static str, GridP)> {
    use Kind::*;
    let mut v = vec![];
    // ---- Auto-Filter: an LFO (+ Macro 1) sweeps two low-passes
    let mut b = B(GridP::empty());
    let a = b.m(AudioIn, 0.0, 0.0, 0, &[]);
    let lfo = b.m(Lfo, 0.0, 1.5, 1, &[(0, 0.25)]);
    let mc = b.m(Macro, 0.0, 3.0, 0, &[]);
    let sum = b.m(Math, 1.0, 1.5, 1, &[]);
    let fl = b.m(
        Filter,
        2.0,
        0.0,
        0,
        &[(0, 700.0), (1, 0.35), (2, 2.0), (3, 0.0)],
    );
    let fr = b.m(
        Filter,
        2.0,
        2.5,
        0,
        &[(0, 700.0), (1, 0.35), (2, 2.0), (3, 0.0)],
    );
    let out = b.m(Out, 3.0, 0.0, 0, &[(0, 1.0)]);
    b.c(lfo, 0, sum, 0);
    b.c(mc, 0, sum, 1);
    b.c(a, 0, fl, 0);
    b.c(a, 1, fr, 0);
    b.c(sum, 0, fl, 1);
    b.c(sum, 0, fr, 1);
    b.c(fl, 0, out, 0);
    b.c(fr, 0, out, 1);
    v.push((
        "Auto-Filter",
        "an LFO plus Macro 1 sweep a resonant low-pass on each side",
        b.0,
    ));
    // ---- Ring Mod: the input times a sine tuned to the channel's note
    let mut b = B(GridP::empty());
    let a = b.m(AudioIn, 0.0, 0.0, 0, &[]);
    let n = b.m(Note, 0.0, 1.5, 0, &[]);
    let o = b.m(Osc, 1.0, 1.5, 0, &[(0, 7.0)]);
    let vl = b.m(Vca, 2.0, 0.0, 0, &[]);
    let vr = b.m(Vca, 2.0, 1.5, 0, &[]);
    let out = b.m(Out, 3.0, 0.0, 0, &[(0, 1.0)]);
    b.c(n, 0, o, 0);
    b.c(a, 0, vl, 0);
    b.c(a, 1, vr, 0);
    b.c(o, 0, vl, 1);
    b.c(o, 0, vr, 1);
    b.c(vl, 0, out, 0);
    b.c(vr, 0, out, 1);
    v.push((
        "Ring Mod",
        "the input times a sine a fifth above the channel's note",
        b.0,
    ));
    // ---- Comb Resonator: a feedback delay tuned to the channel's note
    let mut b = B(GridP::empty());
    let a = b.m(AudioIn, 0.0, 0.0, 0, &[]);
    let n = b.m(Note, 0.0, 1.5, 0, &[]);
    let d = b.m(
        Delay,
        1.0,
        0.0,
        0,
        &[(0, 1000.0 / C4), (1, 0.9), (2, 0.6), (3, 0.2)],
    );
    let sh = b.m(Shaper, 2.0, 0.0, 0, &[(0, 0.1), (1, 1.0)]);
    let out = b.m(Out, 3.0, 0.0, 0, &[(0, 0.8)]);
    b.c(a, 0, d, 0);
    b.c(n, 0, d, 1);
    b.c(d, 0, sh, 0);
    b.c(sh, 0, out, 0);
    v.push((
        "Comb Resonator",
        "a feedback delay tuned to the channel's note rings with the input (mono)",
        b.0,
    ));
    v
}

/// a new Grid FX device's patch
pub fn fx_default() -> GridP {
    fx_presets().swap_remove(0).2
}

// ------------------------------------------------------------------ engine

#[derive(Clone, Copy)]
struct Plan {
    fp: u64,
    n: usize,
    order: [u8; MAX_MODULES],
    kind: [Kind; MAX_MODULES],
    mode: [u8; MAX_MODULES],
    /// (source module index, output) per input; NONE = unconnected
    src: [[(u8, u8); MAX_IN]; MAX_MODULES],
    slot: [[u8; MAX_P]; MAX_MODULES],
    /// delay line / ladder index per module (NONE = none)
    res: [u8; MAX_MODULES],
    has_adsr: bool,
    has_out: bool,
}

impl Default for Plan {
    fn default() -> Self {
        Plan {
            fp: 0,
            n: 0,
            order: [0; MAX_MODULES],
            kind: [Kind::Knob; MAX_MODULES],
            mode: [0; MAX_MODULES],
            src: [[(NONE, 0); MAX_IN]; MAX_MODULES],
            slot: [[NONE; MAX_P]; MAX_MODULES],
            res: [NONE; MAX_MODULES],
            has_adsr: false,
            has_out: false,
        }
    }
}

/// allocation-free: fixed arrays only
fn compile(g: &GridP, fp: u64) -> Plan {
    let mut p = Plan {
        fp,
        n: g.modules.len().min(MAX_MODULES),
        ..Default::default()
    };
    let (mut nd, mut nl) = (0u8, 0u8);
    for (i, m) in g.modules.iter().take(MAX_MODULES).enumerate() {
        p.kind[i] = m.kind;
        p.mode[i] = m.mode;
        p.slot[i] = m.slots;
        match m.kind {
            Kind::Delay if (nd as usize) < MAX_DELAYS => {
                p.res[i] = nd;
                nd += 1;
            }
            Kind::Ladder if (nl as usize) < MAX_LADDERS => {
                p.res[i] = nl;
                nl += 1;
            }
            Kind::Adsr => p.has_adsr = true,
            Kind::Out => p.has_out = true,
            _ => {}
        }
    }
    for c in g.cables.iter().take(MAX_CABLES) {
        let a = g
            .modules
            .iter()
            .take(MAX_MODULES)
            .position(|m| m.id == c.from);
        let b = g
            .modules
            .iter()
            .take(MAX_MODULES)
            .position(|m| m.id == c.to);
        if let (Some(a), Some(b)) = (a, b) {
            if (c.inp as usize) < MAX_IN && (c.out as usize) < MAX_OUT {
                p.src[b][c.inp as usize] = (a as u8, c.out);
            }
        }
    }
    // depth-first post-order from the outputs (then everything else): every
    // module runs after its sources; a cable that closes a loop is feedback
    // and reads the previous sample
    let n = p.n;
    let mut mark = [0u8; MAX_MODULES]; // 0 new, 1 on the stack, 2 placed
    let mut stack = [(0u8, 0u8); MAX_MODULES];
    let mut k = 0;
    let roots = (0..n)
        .filter(|&i| p.kind[i] == Kind::Out)
        .chain((0..n).filter(|&i| p.kind[i] != Kind::Out));
    for r in roots {
        if mark[r] != 0 {
            continue;
        }
        let mut sp = 1;
        stack[0] = (r as u8, 0);
        mark[r] = 1;
        while sp > 0 {
            let (i, j) = stack[sp - 1];
            let (i, j) = (i as usize, j as usize);
            if j < MAX_IN {
                stack[sp - 1].1 += 1;
                let s = p.src[i][j].0;
                if s != NONE && mark[s as usize] == 0 {
                    mark[s as usize] = 1;
                    stack[sp] = (s, 0);
                    sp += 1;
                }
            } else {
                mark[i] = 2;
                p.order[k] = i as u8;
                k += 1;
                sp -= 1;
            }
        }
    }
    p
}

/// per-voice Grid state (fixed size; lives in `GenState`)
#[derive(Clone, Copy)]
pub struct GridState {
    plan: Plan,
    started: bool,
    outs: [[f32; MAX_OUT]; MAX_MODULES],
    /// module state (phases, filter memories, envelope levels)
    st: [[f32; 4]; MAX_MODULES],
    /// real knob values (control rate)
    pv: [[f32; MAX_P]; MAX_MODULES],
    lad: [crate::ladder::Ladder; MAX_LADDERS],
    dl: [[f32; DELAY_LEN]; MAX_DELAYS],
    dw: [usize; MAX_DELAYS],
    nz: u32,
    tick: u32,
    pitch: f32,
    macros: [f32; MACROS],
    gate_was: bool,
    released: f32,
    quiet: u32,
    fade: f32,
    pub done: bool,
}

impl Default for GridState {
    fn default() -> Self {
        GridState {
            plan: Plan::default(),
            started: false,
            outs: [[0.0; MAX_OUT]; MAX_MODULES],
            st: [[0.0; 4]; MAX_MODULES],
            pv: [[0.0; MAX_P]; MAX_MODULES],
            lad: [Default::default(); MAX_LADDERS],
            dl: [[0.0; DELAY_LEN]; MAX_DELAYS],
            dw: [0; MAX_DELAYS],
            nz: 0x2545_F491,
            tick: 0,
            pitch: 0.0,
            macros: [0.0; MACROS],
            gate_was: true,
            released: 0.0,
            quiet: 0,
            fade: 1.0,
            done: false,
        }
    }
}

#[inline]
fn xs(n: &mut u32) -> f32 {
    *n ^= *n << 13;
    *n ^= *n >> 17;
    *n ^= *n << 5;
    *n as f32 / u32::MAX as f32 * 2.0 - 1.0
}

#[inline]
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

#[inline]
fn fold(x: f32) -> f32 {
    // triangle fold into -1..1
    let y = (x + 1.0) * 0.25;
    let f = y - y.floor();
    if f < 0.5 {
        4.0 * f - 1.0
    } else {
        3.0 - 4.0 * f
    }
}

impl GridState {
    /// re-read the knobs (control rate)
    fn read_knobs(&mut self, g: &GridP, ov: &Ov) {
        let p = &self.plan;
        for i in 0..p.n {
            let sp = spec(p.kind[i]);
            for (j, d) in sp.params.iter().enumerate() {
                let s = p.slot[i][j] as usize;
                let v = if s < GEN_KNOBS {
                    ov.get(s).unwrap_or(g.p.get(s).copied().unwrap_or(0.5))
                } else {
                    d.unmap(sp.defaults[j])
                };
                self.pv[i][j] = d.map(v);
            }
        }
        for k in 0..MACROS {
            self.macros[k] = ov.get(k).unwrap_or(g.p.get(k).copied().unwrap_or(0.0));
        }
    }

    /// One stereo sample. `audio` = the FX input (0 on instruments). Allocation-free.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        g: &GridP,
        ov: Ov,
        freq: f32,
        sr: f32,
        gate: bool,
        vel: f32,
        audio: (f32, f32),
    ) -> (f32, f32, bool) {
        if self.done {
            return (0.0, 0.0, true);
        }
        let dt = 1.0 / sr;
        if !self.started || self.tick.is_multiple_of(64) {
            let fp = g.fingerprint();
            if !self.started || fp != self.plan.fp {
                self.plan = compile(g, fp);
                if !self.started {
                    self.nz = 0x6A09_E667 ^ freq.to_bits().wrapping_mul(2_654_435_761) | 1;
                    // LFO start phases
                    for i in 0..self.plan.n {
                        self.st[i] = [0.0; 4];
                    }
                }
                self.read_knobs(g, &ov);
                if !self.started {
                    for i in 0..self.plan.n {
                        if self.plan.kind[i] == Kind::Lfo {
                            self.st[i][0] = self.pv[i][1];
                        }
                    }
                    self.gate_was = gate;
                }
                self.started = true;
            }
        }
        if self.tick.is_multiple_of(CR) {
            self.read_knobs(g, &ov);
            self.pitch = (freq.max(1.0) / C4).log2();
        }
        self.tick = self.tick.wrapping_add(1);
        let note_gate = if gate { 1.0 } else { 0.0 };
        let white = xs(&mut self.nz);
        let (mut ol, mut or) = (0.0f32, 0.0f32);
        let n = self.plan.n;
        for k in 0..n {
            let i = self.plan.order[k] as usize;
            let kind = self.plan.kind[i];
            let sp = spec(kind);
            let mut x = [0.0f32; MAX_IN];
            let mut con = [false; MAX_IN];
            for (j, (_, d)) in sp.ins.iter().enumerate() {
                let (s, o) = self.plan.src[i][j];
                if s != NONE {
                    x[j] = self.outs[s as usize][o as usize];
                    con[j] = true;
                } else {
                    x[j] = match d {
                        Zero => 0.0,
                        One => 1.0,
                        Pitch => self.pitch,
                        Gate => note_gate,
                        Noise => white,
                    };
                }
            }
            let pv = self.pv[i];
            let mode = self.plan.mode[i];
            let st = &mut self.st[i];
            let mut y = [0.0f32; MAX_OUT];
            match kind {
                Kind::Note => {
                    y[0] = self.pitch;
                    y[1] = note_gate;
                    y[2] = vel;
                }
                Kind::Macro => y[0] = self.macros[(mode as usize).min(MACROS - 1)],
                Kind::AudioIn => {
                    y[0] = audio.0;
                    y[1] = audio.1;
                }
                Kind::Out => {
                    let l = x[0];
                    let r = if con[1] { x[1] } else { x[0] };
                    ol += l * pv[0];
                    or += r * pv[0];
                }
                Kind::Osc => {
                    let f = C4 * (x[0] + pv[0] / 12.0 + pv[1] / 1200.0).exp2();
                    let inc = (f * dt).clamp(0.0, 0.5);
                    let ph = st[0];
                    let t = (ph + x[1] * pv[3]).rem_euclid(1.0);
                    let pw = (pv[2] + x[2] * 0.5).clamp(0.02, 0.98);
                    y[0] = match mode {
                        0 => (TAU * t).sin(),
                        1 => {
                            if t < 0.5 {
                                4.0 * t - 1.0
                            } else {
                                3.0 - 4.0 * t
                            }
                        }
                        2 => 2.0 * t - 1.0 - blep(t, inc.max(1e-6)),
                        _ => {
                            let w = if mode == 3 { 0.5 } else { pw };
                            let d = inc.max(1e-6);
                            let s = if t < w { 1.0 } else { -1.0 };
                            s + blep(t, d) - blep((t - w).rem_euclid(1.0), d)
                        }
                    };
                    let p2 = ph + inc;
                    st[0] = p2 - p2.floor();
                }
                Kind::Wavetable => {
                    let f = C4 * (x[0] + pv[0] / 12.0 + pv[1] / 1200.0).exp2();
                    let ph = st[0] + f * dt;
                    st[0] = ph - ph.floor();
                    let pos = (pv[2] + x[1] * pv[3]).clamp(0.0, 1.0);
                    y[0] = match (
                        crate::wavetable::builtins().and_then(|b| b.get(mode as usize)),
                        crate::wavetable::Table::level(f, sr),
                    ) {
                        (Some(t), Some(lv)) => t.sample(pos, st[0], lv),
                        _ => 0.0,
                    };
                }
                Kind::Noise => {
                    let a = 1.0 - pv[0] * 0.985;
                    st[0] += (white - st[0]) * a;
                    y[0] = st[0] * (1.0 / a.sqrt()).min(6.0);
                }
                Kind::Filter => {
                    let oct = x[1] * pv[2] + pv[3] * self.pitch;
                    let fc = (pv[0] * oct.exp2()).clamp(10.0, sr * 0.49);
                    let gg = (PI * fc * dt).tan();
                    let kk = 2.0 - 1.96 * pv[1];
                    let a1 = 1.0 / (1.0 + gg * (gg + kk));
                    let a2 = gg * a1;
                    let a3 = gg * a2;
                    let v0 = x[0];
                    let v3 = v0 - st[1];
                    let v1 = a1 * st[0] + a2 * v3;
                    let v2 = st[1] + a2 * st[0] + a3 * v3;
                    st[0] = 2.0 * v1 - st[0];
                    st[1] = 2.0 * v2 - st[1];
                    y[0] = match mode {
                        0 => v2,
                        1 => v1,
                        2 => v0 - kk * v1 - v2,
                        _ => v0 - kk * v1,
                    };
                    if !y[0].is_finite() {
                        st[0] = 0.0;
                        st[1] = 0.0;
                        y[0] = 0.0;
                    }
                }
                Kind::Ladder => {
                    let r = self.plan.res[i];
                    if (r as usize) < MAX_LADDERS {
                        let oct = x[1] * pv[2] + pv[3] * self.pitch;
                        let fc = (pv[0] * oct.exp2()).clamp(20.0, sr * 0.45);
                        use crate::synth::FilterMode as F;
                        let (fm, p4) = match mode {
                            0 => (F::Lp, true),
                            1 => (F::Lp, false),
                            2 => (F::Bp, true),
                            _ => (F::Hp, true),
                        };
                        y[0] = self.lad[r as usize].process(x[0], fc, pv[1], sr, fm, p4, false);
                    }
                }
                Kind::Adsr => {
                    let on = x[0] > 0.5;
                    let was = st[2] > 0.5;
                    if on && !was {
                        st[1] = 1.0; // attack
                    } else if !on && was {
                        st[1] = 3.0; // release
                    }
                    st[2] = if on { 1.0 } else { 0.0 };
                    let mut lv = st[0];
                    let lv = &mut lv;
                    match st[1] as u8 {
                        1 => {
                            *lv += dt / pv[0];
                            if *lv >= 1.0 {
                                *lv = 1.0;
                                st[1] = 2.0;
                            }
                        }
                        2 => *lv += (pv[2] - *lv) * (dt * 5.0 / pv[1]).min(1.0),
                        3 => {
                            *lv += (0.0 - *lv) * (dt * 5.0 / pv[3]).min(1.0);
                            if *lv < 1e-5 {
                                *lv = 0.0;
                                st[1] = 0.0;
                            }
                        }
                        _ => {}
                    }
                    st[0] = *lv;
                    y[0] = st[0];
                }
                Kind::Lfo => {
                    let t = st[0];
                    y[0] = match mode {
                        0 => (TAU * t).sin(),
                        1 => {
                            if t < 0.5 {
                                4.0 * t - 1.0
                            } else {
                                3.0 - 4.0 * t
                            }
                        }
                        2 => 2.0 * t - 1.0,
                        3 => {
                            if t < 0.5 {
                                1.0
                            } else {
                                -1.0
                            }
                        }
                        _ => st[1],
                    };
                    let ph = t + pv[0] * x[0].exp2() * dt;
                    if ph >= 1.0 || st[3] == 0.0 {
                        st[1] = white;
                        st[3] = 1.0;
                    }
                    st[0] = ph - ph.floor();
                }
                Kind::Mix => y[0] = x[0] * pv[0] + x[1] * pv[1] + x[2] * pv[2],
                Kind::Vca => y[0] = x[0] * x[1] * pv[0],
                Kind::Math => {
                    let (a, b) = (x[0], x[1]);
                    let r = match mode {
                        0 => a,
                        1 => a + b,
                        2 => a - b,
                        3 => a * b,
                        4 => a.abs(),
                        5 => a.min(b),
                        _ => a.max(b),
                    };
                    y[0] = r * pv[0] + pv[1];
                }
                Kind::Logic => {
                    let th = pv[0];
                    let b = if con[1] { x[1] } else { th };
                    let (pa, pb) = (x[0] > th, x[1] > th);
                    let on = match mode {
                        0 => x[0] > b,
                        1 => x[0] < b,
                        2 => pa && pb,
                        3 => pa || pb,
                        4 => pa != pb,
                        _ => !pa,
                    };
                    y[0] = if on { 1.0 } else { 0.0 };
                }
                Kind::Slew => {
                    let tau = if x[0] > st[0] { pv[0] } else { pv[1] };
                    st[0] += (x[0] - st[0]) * (dt / tau).min(1.0);
                    y[0] = st[0];
                }
                Kind::SampleHold => {
                    let on = x[1] > 0.5;
                    if (on && st[1] < 0.5) || st[2] == 0.0 {
                        st[0] = x[0];
                        st[2] = 1.0;
                    }
                    st[1] = if on { 1.0 } else { 0.0 };
                    y[0] = st[0];
                }
                Kind::Shaper => {
                    let gg = 1.0 + pv[0] * 19.0;
                    let w = match mode {
                        0 => (x[0] * gg).tanh(),
                        1 => fold(x[0] * gg),
                        _ => (x[0] * gg).clamp(-1.0, 1.0),
                    };
                    y[0] = x[0] + (w - x[0]) * pv[1];
                }
                Kind::Delay => {
                    let r = self.plan.res[i] as usize;
                    if r < MAX_DELAYS {
                        let d = (pv[0] * 0.001 * sr * (-x[1]).exp2())
                            .clamp(1.0, (DELAY_LEN - 2) as f32);
                        let line = &mut self.dl[r];
                        let w = self.dw[r];
                        let rp = w as f32 + DELAY_LEN as f32 - d;
                        let i0 = rp.floor() as usize;
                        let fr = rp - rp.floor();
                        let a = line[i0 % DELAY_LEN];
                        let b = line[(i0 + 1) % DELAY_LEN];
                        let tap = a + (b - a) * fr;
                        // damping: one-pole low-pass in the loop
                        st[0] += (tap - st[0]) * (1.0 - pv[3] * 0.95);
                        let fb = st[0] * pv[1];
                        let wv = x[0] + fb;
                        line[w] = if wv.is_finite() {
                            wv.clamp(-4.0, 4.0)
                        } else {
                            0.0
                        };
                        self.dw[r] = (w + 1) % DELAY_LEN;
                        y[0] = x[0] * (1.0 - pv[2]) + st[0] * pv[2];
                    } else {
                        y[0] = x[0];
                    }
                }
                Kind::Knob => y[0] = pv[0],
            }
            for v in y.iter_mut() {
                if !v.is_finite() {
                    *v = 0.0;
                }
            }
            self.outs[i] = y;
        }
        if !self.plan.has_out {
            self.done = true;
            return (0.0, 0.0, true);
        }
        // the voice ends: a short fade without an ADSR, else when silent
        if self.gate_was && !gate {
            self.released = 0.0;
        }
        self.gate_was = gate;
        if !gate {
            self.released += dt;
            if !self.plan.has_adsr {
                self.fade -= dt / 0.01;
                if self.fade <= 0.0 {
                    self.done = true;
                }
            } else if ol.abs() + or.abs() < 1e-4 {
                self.quiet += 1;
                if self.quiet > (sr * 0.05) as u32 {
                    self.done = true;
                }
            } else {
                self.quiet = 0;
            }
            if self.released > 30.0 {
                self.done = true;
            }
        }
        let gl = g.level * self.fade.max(0.0);
        let (l, r) = (ol * gl, or * gl);
        if l.is_finite() && r.is_finite() {
            (l.clamp(-8.0, 8.0), r.clamp(-8.0, 8.0), true)
        } else {
            (0.0, 0.0, true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{GenEngine, GenPatch, NO_KNOBS};

    fn run(g: &GridP, freq: f32, on: usize, off: usize) -> (Vec<f32>, GridState) {
        crate::wavetable::init();
        let mut s = GridState::default();
        let mac = [-1.0; 8];
        let ov = Ov {
            mac: &mac,
            knob: &NO_KNOBS,
        };
        let mut v = Vec::with_capacity(on + off);
        for i in 0..on + off {
            v.push(s.render(g, ov, freq, 48000.0, i < on, 1.0, (0.0, 0.0)).0);
        }
        (v, s)
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    #[test]
    fn the_plan_runs_sources_before_their_listeners_and_cuts_loops() {
        let mut g = GridP::empty();
        // added in reverse order on purpose
        let out = g.add(Kind::Out, 0.0, 0.0).unwrap();
        let vca = g.add(Kind::Vca, 0.0, 0.0).unwrap();
        let osc = g.add(Kind::Osc, 0.0, 0.0).unwrap();
        let n = g.add(Kind::Note, 0.0, 0.0).unwrap();
        assert!(g.connect(n, 0, osc, 0));
        assert!(g.connect(osc, 0, vca, 0));
        assert!(g.connect(vca, 0, out, 0));
        // feedback: the VCA's output phase-modulates the oscillator
        assert!(g.connect(vca, 0, osc, 1));
        let p = compile(&g, g.fingerprint());
        let pos = |id: u16| {
            let ix = g.modules.iter().position(|m| m.id == id).unwrap();
            p.order[..p.n]
                .iter()
                .position(|&o| o as usize == ix)
                .unwrap()
        };
        assert!(pos(n) < pos(osc) && pos(osc) < pos(vca) && pos(vca) < pos(out));
        let (v, _) = run(&g, 220.0, 4800, 0);
        assert!(v.iter().all(|x| x.is_finite()) && rms(&v) > 0.05);
        // an invalid cable is refused; one input holds one cable
        assert!(!g.connect(n, 7, osc, 0));
        assert!(g.connect(n, 0, osc, 0));
        assert_eq!(
            g.cables
                .iter()
                .filter(|c| c.to == osc && c.inp == 0)
                .count(),
            1
        );
    }

    #[test]
    fn fix_clamps_a_hostile_patch() {
        let mut g = GridP::default();
        for _ in 0..40 {
            g.add(Kind::Osc, 0.0, 0.0);
        }
        assert!(g.modules.len() <= MAX_MODULES && g.add(Kind::Osc, 0.0, 0.0).is_none());
        let mut h = g.clone();
        h.modules[1].id = h.modules[0].id;
        h.modules[2].mode = 99;
        h.modules[3].slots = [70, 70, 70, 70];
        h.cables.push(Cable {
            from: 999,
            out: 0,
            to: 1,
            inp: 0,
        });
        h.p.truncate(3);
        h.fix();
        assert_eq!(h.p.len(), GEN_KNOBS);
        let mut ids: Vec<u16> = h.modules.iter().map(|m| m.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), h.modules.len());
        assert!(h
            .modules
            .iter()
            .all(|m| (m.mode as usize) < spec(m.kind).modes.len().max(1)));
        assert!(h.cables.iter().all(|c| h.module(c.from).is_some()));
        let (v, _) = run(&h, 220.0, 2000, 0);
        assert!(v.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn note_off_ends_the_voice() {
        // with an envelope: after its release has gone quiet
        let (_, s) = run(&GridP::default(), 220.0, 4800, 48000);
        assert!(s.done);
        // without one: a short fade
        let mut g = GridP::empty();
        let o = g.add(Kind::Osc, 0.0, 0.0).unwrap();
        let out = g.add(Kind::Out, 0.0, 0.0).unwrap();
        g.connect(o, 0, out, 0);
        let (v, s) = run(&g, 220.0, 4800, 4800);
        assert!(rms(&v[..4800]) > 0.1 && s.done);
    }

    #[test]
    fn velocity_pitch_and_macros_reach_the_patch() {
        let mut g = GridP::empty();
        let n = g.add(Kind::Note, 0.0, 0.0).unwrap();
        let o = g.add(Kind::Osc, 0.0, 0.0).unwrap();
        let v = g.add(Kind::Vca, 0.0, 0.0).unwrap();
        let mc = g.add(Kind::Macro, 0.0, 0.0).unwrap();
        let mx = g.add(Kind::Math, 0.0, 0.0).unwrap();
        let out = g.add(Kind::Out, 0.0, 0.0).unwrap();
        g.set_mode(mx, 3); // A x B
        g.connect(n, 0, o, 0);
        g.connect(o, 0, v, 0);
        g.connect(n, 2, mx, 0);
        g.connect(mc, 0, mx, 1);
        g.connect(mx, 0, v, 1);
        g.connect(v, 0, out, 0);
        g.p[0] = 1.0;
        let level = |vel: f32, mac: f32| {
            let mut s = GridState::default();
            let m = [mac; 8];
            let ov = Ov {
                mac: &m,
                knob: &NO_KNOBS,
            };
            let x: Vec<f32> = (0..4800)
                .map(|_| s.render(&g, ov, 220.0, 48000.0, true, vel, (0.0, 0.0)).0)
                .collect();
            rms(&x[480..])
        };
        let full = level(1.0, -1.0);
        assert!(full > 0.1);
        assert!((level(0.5, -1.0) / full - 0.5).abs() < 0.05, "velocity");
        assert!(level(1.0, 0.0) < 1e-4, "macro override 0 mutes");
    }

    #[test]
    fn presets_render_finite_and_audible_and_round_trip() {
        let ps = presets();
        assert!(ps.len() >= 5);
        for (name, _, p, _, _) in ps {
            let mut q = p.clone();
            q.fix();
            assert_eq!(q, p, "{name} is already fixed");
            let g = GenPatch {
                engine: GenEngine::Grid(p.clone()),
                ..Default::default()
            };
            crate::wavetable::init();
            let out = crate::gen::bake(&g, 220.0, 48000.0, 1.5);
            assert!(out.iter().all(|x| x.is_finite()), "{name}");
            let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.05 && peak < 2.0, "{name}: peak {peak}");
            let t = toml::to_string(&g).unwrap();
            let back: GenPatch = toml::from_str(&t).unwrap();
            assert_eq!(back, g, "{name}");
            // every knob has a definition and a label
            for (k, label) in p.knobs() {
                assert!(p.knob_def(k).is_some() && !label.is_empty(), "{name} {k}");
            }
        }
    }

    #[test]
    fn the_karplus_preset_is_in_tune() {
        let (_, _, p, _, _) = presets()
            .into_iter()
            .find(|x| x.0 == "Grid Karplus Pluck")
            .unwrap();
        let (v, _) = run(&p, 220.0, 24000, 0);
        // the autocorrelation peak after the attack
        let tail = &v[4800..14400];
        let ac = |lag: usize| -> f32 { tail.iter().zip(&tail[lag..]).map(|(a, b)| a * b).sum() };
        let lag = (100..400).max_by(|&a, &b| ac(a).total_cmp(&ac(b))).unwrap();
        let hz = 48000.0 / lag as f32;
        assert!((hz - 220.0).abs() < 220.0 * 0.03, "{hz} Hz");
    }
}
