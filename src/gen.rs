//! Generators: instruments that make their sound from scratch, with a few
//! macro knobs on the front and a seeded Dice.
//!
//! 0.2.3.12 adds the family and its first engine, the **Drum Synth**: Kick
//! (v0 / 808 / 909 / Zap psy), Snare, Hat, Clap, Tom and Perc, eight macro
//! knobs each, plus a **Kit** mode (one instrument, the note picks the drum).
//!
//! Recipes (re-implemented, not copied):
//! * TR-808 kick / snare / clap / toms / cowbell / claves / rim / hats:
//!   Sonic Pi's `sc808` SynthDefs (MIT, Sam Aaron and contributors), based on
//!   Yoshinosuke Horiuchi's free SC-808: pitch envelope note·7 → note·1.35 →
//!   note, two sines a few semitones apart under the snare noise, the six
//!   808 square oscillators, 540 / 800 Hz cowbell, the multi-burst clap.
//! * Ring-modulated square-pair hats and the pinged bass-drum body:
//!   Emilie Gillet's Mutable Instruments Plaits drum models (MIT), also in
//!   DaisySP `Source/Drums` (MIT, Electrosmith).
//! * Macro layout (Tune, Punch, Click, Body, Shape ... per drum) and the Zap
//!   kick idea: Bitwig's v0 drum devices (closed source; ideas only).
//! * Dice with per-category ranges, Mutate and locks: sfxr (DrPetter, MIT)
//!   and bfxr (Stephen Lavelle, Apache-2.0) "lock" toggles.
//!
//! Everything a voice needs at play time is fixed-size (`GenState`), so
//! rendering never allocates.
use crate::synth::{FilterMode, Patch, Svf};
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

/// one macro knob: stored 0..1, shown in real units
#[derive(Clone, Copy, Debug)]
pub struct MacroDef {
    pub name: &'static str,
    pub lo: f32,
    pub hi: f32,
    /// exponential (frequencies, times) or linear
    pub exp: bool,
    pub unit: &'static str,
    /// the Dice rolls inside this 0..1 window ("sensible" sounds)
    pub dice: (f32, f32),
    pub tip: &'static str,
}

impl MacroDef {
    pub fn map(&self, v: f32) -> f32 {
        let v = if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.5
        };
        if self.exp {
            self.lo * (self.hi / self.lo).powf(v)
        } else {
            self.lo + (self.hi - self.lo) * v
        }
    }
    pub fn unmap(&self, x: f32) -> f32 {
        let x = x.clamp(self.lo.min(self.hi), self.lo.max(self.hi));
        if self.exp {
            (x / self.lo).ln() / (self.hi / self.lo).ln()
        } else {
            (x - self.lo) / (self.hi - self.lo)
        }
        .clamp(0.0, 1.0)
    }
    /// value text for the knob ("47 Hz", "220 ms", "35 %")
    pub fn show(&self, v: f32) -> String {
        let x = self.map(v);
        match self.unit {
            "Hz" if x >= 1000.0 => format!("{:.1} kHz", x / 1000.0),
            "Hz" => format!("{x:.0} Hz"),
            "s" if x < 1.0 => format!("{:.0} ms", x * 1000.0),
            "s" => format!("{x:.2} s"),
            "st" => format!("{x:+.0} st"),
            "x" => format!("{x:.2}x"),
            "#" => format!("{}", x.round() as i32),
            _ => format!("{:.0} %", x * 100.0),
        }
    }
}

const fn m(
    name: &'static str,
    lo: f32,
    hi: f32,
    exp: bool,
    unit: &'static str,
    dice: (f32, f32),
    tip: &'static str,
) -> MacroDef {
    MacroDef {
        name,
        lo,
        hi,
        exp,
        unit,
        dice,
        tip,
    }
}

const KICK: [MacroDef; 8] = [
    m(
        "Tune",
        30.0,
        150.0,
        true,
        "Hz",
        (0.12, 0.42),
        "pitch of the tail (C-4 plays this; other notes transpose)",
    ),
    m(
        "Punch",
        0.0,
        60.0,
        false,
        "st",
        (0.3, 0.8),
        "pitch-envelope depth in semitones",
    ),
    m(
        "P.Time",
        0.002,
        0.15,
        true,
        "s",
        (0.15, 0.6),
        "how fast the pitch falls (Zap: the whole sweep)",
    ),
    m(
        "Decay",
        0.05,
        2.5,
        true,
        "s",
        (0.3, 0.75),
        "amp decay to -60 dB",
    ),
    m(
        "Click",
        0.0,
        1.0,
        false,
        "%",
        (0.0, 0.6),
        "attack transient",
    ),
    m(
        "Body",
        0.0,
        1.0,
        false,
        "%",
        (0.0, 0.5),
        "shell knock: a resonance above the fundamental",
    ),
    m("Drive", 0.0, 1.0, false, "%", (0.0, 0.5), "saturation"),
    m(
        "Noise",
        0.0,
        1.0,
        false,
        "%",
        (0.0, 0.25),
        "a low noise layer",
    ),
];
const SNARE: [MacroDef; 8] = [
    m("Tune", 100.0, 400.0, true, "Hz", (0.25, 0.7), "shell pitch"),
    m(
        "Tone",
        0.0,
        1.0,
        false,
        "%",
        (0.35, 0.8),
        "shell (left) vs snares / noise (right)",
    ),
    m("Snappy", 0.03, 0.8, true, "s", (0.3, 0.7), "noise decay"),
    m("Decay", 0.03, 0.6, true, "s", (0.2, 0.6), "shell decay"),
    m(
        "Color",
        1000.0,
        12000.0,
        true,
        "Hz",
        (0.3, 0.8),
        "noise band (brightness)",
    ),
    m("Click", 0.0, 1.0, false, "%", (0.0, 0.6), "stick transient"),
    m(
        "Ring",
        0.0,
        1.0,
        false,
        "%",
        (0.0, 0.5),
        "drum-head modes that ring on",
    ),
    m("Drive", 0.0, 1.0, false, "%", (0.0, 0.4), "saturation"),
];
const HAT: [MacroDef; 8] = [
    m(
        "Tune",
        0.5,
        2.0,
        true,
        "x",
        (0.3, 0.7),
        "metal pitch (1x = the 808 set)",
    ),
    m(
        "Decay",
        0.01,
        1.5,
        true,
        "s",
        (0.05, 0.55),
        "short = closed, long = open",
    ),
    m(
        "Metal",
        0.0,
        1.0,
        false,
        "%",
        (0.25, 0.9),
        "metal oscillators vs plain noise",
    ),
    m(
        "Tone",
        3000.0,
        14000.0,
        true,
        "Hz",
        (0.3, 0.8),
        "high-pass / band centre",
    ),
    m(
        "Color",
        0.0,
        1.0,
        false,
        "%",
        (0.0, 0.7),
        "resonance (808 / ring) or comb feedback",
    ),
    m("Width", 0.0, 1.0, false, "%", (0.0, 0.8), "stereo"),
    m("Click", 0.0, 1.0, false, "%", (0.0, 0.5), "stick transient"),
    m("Drive", 0.0, 1.0, false, "%", (0.0, 0.3), "saturation"),
];
const CLAP: [MacroDef; 8] = [
    m(
        "Tone",
        600.0,
        3000.0,
        true,
        "Hz",
        (0.3, 0.7),
        "band-pass centre",
    ),
    m("Decay", 0.05, 1.5, true, "s", (0.2, 0.6), "tail decay"),
    m(
        "Spread",
        0.003,
        0.025,
        true,
        "s",
        (0.3, 0.7),
        "time between the hand claps",
    ),
    m(
        "Claps",
        1.0,
        6.0,
        false,
        "#",
        (0.35, 0.8),
        "number of bursts before the tail",
    ),
    m("Q", 0.0, 1.0, false, "%", (0.2, 0.6), "band-pass resonance"),
    m(
        "Room",
        0.0,
        1.0,
        false,
        "%",
        (0.0, 0.5),
        "a soft noise room after it",
    ),
    m("Width", 0.0, 1.0, false, "%", (0.2, 0.8), "stereo"),
    m("Drive", 0.0, 1.0, false, "%", (0.0, 0.3), "saturation"),
];
const TOM: [MacroDef; 8] = [
    m("Tune", 50.0, 500.0, true, "Hz", (0.2, 0.6), "pitch"),
    m(
        "Punch",
        0.0,
        24.0,
        false,
        "st",
        (0.15, 0.6),
        "pitch-envelope depth",
    ),
    m(
        "P.Time",
        0.01,
        0.3,
        true,
        "s",
        (0.3, 0.7),
        "pitch fall time",
    ),
    m("Decay", 0.05, 2.0, true, "s", (0.3, 0.7), "amp decay"),
    m("Click", 0.0, 1.0, false, "%", (0.0, 0.5), "stick transient"),
    m("Noise", 0.0, 1.0, false, "%", (0.0, 0.3), "a little noise"),
    m("Body", 0.0, 1.0, false, "%", (0.0, 0.5), "second head mode"),
    m("Drive", 0.0, 1.0, false, "%", (0.0, 0.3), "saturation"),
];
const PERC: [MacroDef; 8] = [
    m(
        "Tune",
        0.25,
        4.0,
        true,
        "x",
        (0.35, 0.65),
        "pitch (1x = the classic tuning)",
    ),
    m("Decay", 0.01, 2.0, true, "s", (0.1, 0.5), "amp decay"),
    m("Tone", 0.0, 1.0, false, "%", (0.3, 0.8), "brightness"),
    m(
        "Color",
        0.0,
        1.0,
        false,
        "%",
        (0.2, 0.8),
        "cowbell detune, rim body, cymbal noise, zap wave",
    ),
    m("Ring", 0.0, 1.0, false, "%", (0.0, 0.6), "resonance"),
    m("Click", 0.0, 1.0, false, "%", (0.0, 0.6), "transient"),
    m("Width", 0.0, 1.0, false, "%", (0.0, 0.6), "stereo"),
    m("Drive", 0.0, 1.0, false, "%", (0.0, 0.3), "saturation"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrumKind {
    #[default]
    Kick,
    Snare,
    Hat,
    Clap,
    Tom,
    Perc,
}

impl DrumKind {
    pub const ALL: [DrumKind; 6] = [
        DrumKind::Kick,
        DrumKind::Snare,
        DrumKind::Hat,
        DrumKind::Clap,
        DrumKind::Tom,
        DrumKind::Perc,
    ];
    pub fn name(self) -> &'static str {
        match self {
            DrumKind::Kick => "Kick",
            DrumKind::Snare => "Snare",
            DrumKind::Hat => "Hat",
            DrumKind::Clap => "Clap",
            DrumKind::Tom => "Tom",
            DrumKind::Perc => "Perc",
        }
    }
    pub fn models(self) -> &'static [&'static str] {
        match self {
            DrumKind::Kick => &["v0", "808", "909", "Zap (psy)"],
            DrumKind::Snare => &["808", "909", "Shell (modal)"],
            DrumKind::Hat => &["808 metal", "Ring mod", "Comb + FM"],
            DrumKind::Clap => &["808", "Tight"],
            DrumKind::Tom => &["808 tom", "Conga"],
            DrumKind::Perc => &["Cowbell", "Claves", "Rimshot", "Cymbal", "Zap"],
        }
    }
    pub fn macros(self) -> &'static [MacroDef; 8] {
        match self {
            DrumKind::Kick => &KICK,
            DrumKind::Snare => &SNARE,
            DrumKind::Hat => &HAT,
            DrumKind::Clap => &CLAP,
            DrumKind::Tom => &TOM,
            DrumKind::Perc => &PERC,
        }
    }
}

/// one drum voice: kind, model and its eight macros (0..1)
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumP {
    pub kind: DrumKind,
    pub model: u8,
    pub macros: [f32; 8],
    /// output level 0..1.5
    pub level: f32,
    /// the pattern note transposes the drum (C-4 = as tuned)
    pub follow_note: bool,
    /// bit i = macro i is locked (Dice and Mutate leave it alone)
    pub locks: u8,
}

impl Default for DrumP {
    fn default() -> Self {
        DrumP::new(DrumKind::Kick, 0, &[])
    }
}

impl DrumP {
    /// a drum with `vals` in real units (missing ones = the middle of the dice range)
    pub fn new(kind: DrumKind, model: u8, vals: &[f32]) -> Self {
        let defs = kind.macros();
        let mut macros = [0.0; 8];
        for (i, d) in defs.iter().enumerate() {
            macros[i] = match vals.get(i) {
                Some(v) => d.unmap(*v),
                None => (d.dice.0 + d.dice.1) * 0.5,
            };
        }
        DrumP {
            kind,
            model,
            macros,
            level: 1.0,
            follow_note: true,
            locks: 0,
        }
    }
    pub fn fix(&mut self) {
        let n = self.kind.models().len() as u8;
        self.model = self.model.min(n - 1);
        for v in &mut self.macros {
            *v = if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.5
            };
        }
        self.level = if self.level.is_finite() {
            self.level.clamp(0.0, 1.5)
        } else {
            1.0
        };
    }
}

/// Kit mode: eight drums on one instrument; the note picks the drum
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KitP {
    pub pieces: [DrumP; 8],
}

/// kit piece names, in `pieces` order
pub const KIT_NAMES: [&str; 8] = [
    "Kick",
    "Snare",
    "Clap",
    "Closed hat",
    "Open hat",
    "Tom",
    "Perc",
    "Perc 2",
];

/// note in the octave (C = 0) -> (piece, tom offset in semitones)
pub const KIT_MAP: [(usize, f32); 12] = [
    (0, 0.0),  // C  kick
    (6, 0.0),  // C# perc
    (1, 0.0),  // D  snare
    (2, 0.0),  // D# clap
    (5, -7.0), // E  low tom
    (5, -4.0), // F  low-mid tom
    (3, 0.0),  // F# closed hat
    (5, 0.0),  // G  tom
    (3, 0.0),  // G# closed hat
    (5, 4.0),  // A  high tom
    (4, 0.0),  // A# open hat
    (7, 0.0),  // B  perc 2
];

/// which keys play each piece (for the UI)
pub const KIT_KEYS: [&str; 8] = ["C", "D", "D#", "F# G#", "A#", "E F G A", "C#", "B"];

impl Default for KitP {
    fn default() -> Self {
        use DrumKind::*;
        KitP {
            pieces: [
                DrumP::new(Kick, 3, &[49.0, 36.0, 0.03, 0.45, 0.35, 0.1, 0.25, 0.0]),
                DrumP::new(Snare, 0, &[190.0, 0.65, 0.22, 0.12, 5200.0, 0.3, 0.1, 0.1]),
                DrumP::new(Clap, 0, &[1300.0, 0.3, 0.009, 4.0, 0.35, 0.2, 0.5, 0.0]),
                DrumP::new(Hat, 0, &[1.0, 0.045, 0.75, 8000.0, 0.3, 0.3, 0.2, 0.0]),
                DrumP::new(Hat, 0, &[1.0, 0.45, 0.75, 7500.0, 0.3, 0.4, 0.1, 0.0]),
                DrumP::new(Tom, 0, &[130.0, 5.0, 0.08, 0.5, 0.2, 0.05, 0.1, 0.0]),
                DrumP::new(Perc, 0, &[1.0, 0.25, 0.6, 0.3, 0.3, 0.2, 0.0, 0.0]),
                DrumP::new(Perc, 4, &[1.0, 0.25, 0.5, 0.6, 0.2, 0.0, 0.3, 0.0]),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)] // a kit is 8 pieces inline; no allocation on the audio thread
pub enum GenEngine {
    Drum(DrumP),
    Kit(KitP),
    /// sfxr-style game sound (0.2.3.13)
    ChipFx(crate::chipfx::SfxP),
    /// Moog-style analog synth (0.2.3.14)
    Analog(crate::gen2::AnalogP),
    /// 4-operator FM (0.2.3.14)
    Fm(crate::gen2::FmP),
    /// noise: risers, downlifters, impacts, wind, crackle (0.2.3.14)
    Noise(crate::gen2::NoiseP),
}

impl GenEngine {
    /// engines with their own envelopes (the patch amp envelope is not used;
    /// the note-off is passed in as a gate)
    pub fn own_env(&self) -> bool {
        matches!(
            self,
            GenEngine::Analog(_) | GenEngine::Fm(_) | GenEngine::Noise(_)
        )
    }
}

impl Default for GenEngine {
    fn default() -> Self {
        GenEngine::Drum(DrumP::default())
    }
}

/// a Generator instrument
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenPatch {
    pub engine: GenEngine,
    /// amp envelope (release), optional filter (SVF or ladder), gain
    pub base: Patch,
    /// run the sound through `base`'s filter
    pub filter_on: bool,
    /// seed of the last Dice roll (rolls are reproducible)
    pub seed: u32,
    /// Mutate amount 0..0.5 (0.1 = ±10 %)
    pub mutate: f32,
}

pub fn base_patch() -> Patch {
    Patch {
        name: "Generator".into(),
        amp: crate::synth::Adsr {
            a: 0.0,
            d: 1.0,
            s: 1.0,
            r: 0.04,
        },
        cutoff: 6000.0,
        resonance: 0.2,
        gain: 1.0,
        filter_model: crate::ladder::FilterModel::Ladder,
        ..Default::default()
    }
}

impl Default for GenPatch {
    fn default() -> Self {
        GenPatch {
            engine: GenEngine::default(),
            base: base_patch(),
            filter_on: false,
            seed: 1,
            mutate: 0.1,
        }
    }
}

impl GenPatch {
    pub fn drum(d: DrumP) -> Self {
        GenPatch {
            engine: GenEngine::Drum(d),
            ..Default::default()
        }
    }
    pub fn fix(&mut self) {
        match &mut self.engine {
            GenEngine::Drum(d) => d.fix(),
            GenEngine::Kit(k) => k.pieces.iter_mut().for_each(|d| d.fix()),
            GenEngine::ChipFx(s) => s.fix(),
            GenEngine::Analog(a) => a.fix(),
            GenEngine::Fm(f) => f.fix(),
            GenEngine::Noise(n) => n.fix(),
        }
        self.mutate = if self.mutate.is_finite() {
            self.mutate.clamp(0.0, 0.5)
        } else {
            0.1
        };
    }
    /// the drum the editor shows: the Drum, or kit piece `sel`
    /// (None for Chip FX)
    pub fn drum_mut(&mut self, sel: usize) -> Option<&mut DrumP> {
        match &mut self.engine {
            GenEngine::Drum(d) => Some(d),
            GenEngine::Kit(k) => Some(&mut k.pieces[sel.min(7)]),
            _ => None,
        }
    }
    pub fn chip_fx(sfx: crate::chipfx::SfxP) -> Self {
        GenPatch {
            engine: GenEngine::ChipFx(sfx),
            ..Default::default()
        }
    }
    /// Dice: re-roll every unlocked macro of the shown drum inside its range
    /// (Chip FX: roll again in the last category)
    pub fn dice(&mut self, sel: usize, seed: u32) {
        self.seed = seed;
        match &mut self.engine {
            GenEngine::ChipFx(s) => s.dice(seed),
            GenEngine::Analog(a) => a.dice(seed),
            GenEngine::Fm(f) => f.dice(seed),
            GenEngine::Noise(n) => n.dice(seed),
            _ => {
                if let Some(d) = self.drum_mut(sel) {
                    roll(d, seed)
                }
            }
        }
    }
    /// Dice every kit piece (or the one drum)
    pub fn dice_all(&mut self, seed: u32) {
        self.seed = seed;
        match &mut self.engine {
            GenEngine::Drum(d) => roll(d, seed),
            GenEngine::ChipFx(s) => s.dice(seed),
            GenEngine::Analog(a) => a.dice(seed),
            GenEngine::Fm(f) => f.dice(seed),
            GenEngine::Noise(n) => n.dice(seed),
            GenEngine::Kit(k) => {
                for (i, d) in k.pieces.iter_mut().enumerate() {
                    roll(d, seed.wrapping_add(i as u32 * 0x9E37_79B9));
                }
            }
        }
    }
    /// Mutate: nudge every unlocked macro by up to ±`mutate`
    pub fn mutate(&mut self, sel: usize, seed: u32) {
        let a = self.mutate;
        match &mut self.engine {
            GenEngine::ChipFx(s) => return s.mutate(a, seed),
            GenEngine::Analog(x) => return x.mutate(a, seed),
            GenEngine::Fm(x) => return x.mutate(a, seed),
            GenEngine::Noise(x) => return x.mutate(a, seed),
            _ => {}
        }
        let Some(d) = self.drum_mut(sel) else {
            return;
        };
        let mut r = Rng::new(seed);
        for i in 0..8 {
            let x = r.f() * 2.0 - 1.0;
            if d.locks & (1 << i) == 0 {
                d.macros[i] = (d.macros[i] + x * a).clamp(0.0, 1.0);
            }
        }
    }
}

/// small PCG-style generator for Dice (reproducible from the seed)
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u32) -> Self {
        let mut r = Rng((seed as u64) ^ 0x853C_49E6_748F_EA9B);
        r.next();
        r
    }
    pub fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let x = (((self.0 >> 18) ^ self.0) >> 27) as u32;
        x.rotate_right((self.0 >> 59) as u32)
    }
    /// 0..1
    pub fn f(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// re-roll the unlocked macros of `d` inside each macro's dice window
pub fn roll(d: &mut DrumP, seed: u32) {
    let mut r = Rng::new(seed);
    for (i, def) in d.kind.macros().iter().enumerate() {
        let x = r.f();
        if d.locks & (1 << i) == 0 {
            d.macros[i] = def.dice.0 + (def.dice.1 - def.dice.0) * x;
        }
    }
}

// ------------------------------------------------------------------ DSP

const C4: f32 = 261.625_58;
/// 808 hat oscillators (Hz)
const HAT808: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

#[inline]
fn noise(n: &mut u32) -> f32 {
    *n ^= *n << 13;
    *n ^= *n >> 17;
    *n ^= *n << 5;
    *n as f32 / u32::MAX as f32 * 2.0 - 1.0
}

#[inline]
fn sq(ph: f32) -> f32 {
    if ph < 0.5 {
        1.0
    } else {
        -1.0
    }
}

/// peak-preserving soft drive
#[inline]
fn drive(x: f32, d: f32) -> f32 {
    if d <= 0.001 {
        return x;
    }
    let g = 1.0 + 7.0 * d;
    (x * g).tanh() / g.tanh()
}

/// −60 dB after `t60` seconds
#[inline]
fn dec(t: f32, t60: f32) -> f32 {
    (-t * 6.907_755 / t60.max(1e-4)).exp()
}

/// per-voice generator state (fixed size, `Copy`: cloned freely, never allocates)
#[derive(Clone, Copy)]
pub struct GenState {
    t: f32,
    ph: [f32; 6],
    ph_r: [f32; 6],
    ph2: f32,
    nz: u32,
    nz_r: u32,
    f: [Svf; 6],
    f_r: [Svf; 3],
    comb: [f32; 1024],
    ci: usize,
    /// mapped macros (cached while the knobs do not move)
    src: [f32; 8],
    val: [f32; 8],
    kind: Option<(DrumKind, u8)>,
    /// kit: piece and tom offset, chosen from the note
    note_f: f32,
    piece: usize,
    off: f32,
    pub done: bool,
    sfx: crate::chipfx::SfxState,
    an: crate::gen2::AnalogState,
    fm: crate::gen2::FmState,
    ns: crate::gen2::NoiseState,
    /// note held (own-envelope engines release on false)
    pub gate: bool,
}

impl Default for GenState {
    fn default() -> Self {
        GenState {
            t: 0.0,
            ph: [0.0; 6],
            ph_r: [0.13, 0.41, 0.07, 0.77, 0.29, 0.53],
            ph2: 0.0,
            nz: 0x2545_F491,
            nz_r: 0x9E37_79B9,
            f: [Svf::default(); 6],
            f_r: [Svf::default(); 3],
            comb: [0.0; 1024],
            ci: 0,
            src: [-1.0; 8],
            val: [0.0; 8],
            kind: None,
            note_f: -1.0,
            piece: 0,
            off: 0.0,
            sfx: crate::chipfx::SfxState::default(),
            an: Default::default(),
            fm: Default::default(),
            ns: Default::default(),
            gate: true,
            done: false,
        }
    }
}

impl GenState {
    pub fn seed(&mut self, s: u32) {
        self.nz = s | 1;
        self.nz_r = s.rotate_left(11) ^ 0x6A09_E667 | 1;
    }

    /// pattern macro overrides (43xy): values >= 0 replace macro i
    pub fn render(&mut self, g: &GenPatch, ov: &[f32; 8], freq: f32, sr: f32) -> (f32, f32, bool) {
        if self.done {
            return (0.0, 0.0, false);
        }
        let gate = self.gate;
        let own = match &g.engine {
            GenEngine::Analog(a) => Some(self.an.render(a, ov, freq, sr, gate)),
            GenEngine::Fm(f) => Some(self.fm.render(f, ov, freq, sr, gate)),
            GenEngine::Noise(n) => Some(self.ns.render(n, ov, freq, sr, gate)),
            _ => None,
        };
        if let Some(o) = own {
            if self.an.done || self.fm.done || self.ns.done {
                self.done = true;
            }
            return o;
        }
        if let GenEngine::ChipFx(p) = &g.engine {
            let (y, alive) = self.sfx.render(p, ov, freq, sr);
            if !alive {
                self.done = true;
            }
            return (y, y, false);
        }
        let (d, ratio) = match &g.engine {
            GenEngine::ChipFx(_)
            | GenEngine::Analog(_)
            | GenEngine::Fm(_)
            | GenEngine::Noise(_) => {
                unreachable!()
            }
            GenEngine::Drum(d) => (
                d,
                if d.follow_note && freq > 0.0 {
                    freq / C4
                } else {
                    1.0
                },
            ),
            GenEngine::Kit(k) => {
                if freq != self.note_f {
                    self.note_f = freq;
                    let n = (12.0 * (freq.max(1.0) / C4).log2()).round() as i32;
                    let (p, o) = KIT_MAP[n.rem_euclid(12) as usize];
                    self.piece = p;
                    self.off = o;
                }
                (&k.pieces[self.piece], 2f32.powf(self.off / 12.0))
            }
        };
        let mut mac = d.macros;
        for (m, o) in mac.iter_mut().zip(ov) {
            if *o >= 0.0 {
                *m = *o;
            }
        }
        if mac != self.src || self.kind != Some((d.kind, d.model)) {
            self.src = mac;
            self.kind = Some((d.kind, d.model));
            for (i, def) in d.kind.macros().iter().enumerate() {
                self.val[i] = def.map(mac[i]);
            }
        }
        let v = self.val;
        let t = self.t;
        self.t += 1.0 / sr;
        let (l, r, st, end) = match d.kind {
            DrumKind::Kick => {
                let (y, e) = self.kick(d.model, &v, ratio, sr);
                (y, y, false, e)
            }
            DrumKind::Snare => {
                let (y, e) = self.snare(d.model, &v, ratio, sr);
                (y, y, false, e)
            }
            DrumKind::Hat => self.hat(d.model, &v, ratio, sr),
            DrumKind::Clap => self.clap(d.model, &v, sr),
            DrumKind::Tom => {
                let (y, e) = self.tom(d.model, &v, ratio, sr);
                (y, y, false, e)
            }
            DrumKind::Perc => self.perc(d.model, &v, ratio, sr),
        };
        if t > end {
            self.done = true;
        }
        let lv = d.level;
        let (l, r) = (l * lv, r * lv);
        if l.is_finite() && r.is_finite() {
            (l.clamp(-4.0, 4.0), r.clamp(-4.0, 4.0), st)
        } else {
            self.done = true;
            (0.0, 0.0, false)
        }
    }

    /// v = tune, punch, ptime, decay, click, body, drive, noise
    fn kick(&mut self, model: u8, v: &[f32; 8], ratio: f32, sr: f32) -> (f32, f32) {
        let t = self.t;
        let f0 = v[0] * ratio;
        let (punch, pt, decay) = (v[1], v[2], v[3]);
        let f = match model {
            // 808 (sc808): note*7 -> note*1.35 fast, then -> note over ~0.6 s
            1 => {
                let top = 2f32.powf(punch / 12.0);
                f0 * (1.0 + 0.35 * (-t / 0.15).exp() + (top - 1.35).max(0.0) * (-t / pt).exp())
            }
            // 909: a quicker fall
            2 => f0 * 2f32.powf(punch / 12.0 * (-t / (pt * 0.6)).exp()),
            // Zap: a long sweep (an octave deeper start) that lands on the note
            3 => {
                let x = (t / (pt * 4.0)).min(1.0);
                f0 * 2f32.powf((punch + 12.0) / 12.0 * (1.0 - x).powi(3))
            }
            _ => f0 * 2f32.powf(punch / 12.0 * (-t / pt).exp()),
        };
        self.ph[0] = (self.ph[0] + f / sr).fract();
        let mut s = (self.ph[0] * TAU).sin();
        if model == 2 {
            // 909: a slightly squarer sine
            s = (1.8 * s).tanh() / 1.8f32.tanh();
        }
        let atk = (t / 0.0006).min(1.0);
        let amp = dec(t, decay) * atk;
        let n = noise(&mut self.nz);
        // click: a short high-passed noise / square burst
        let ce = (-t / if model == 2 { 0.003 } else { 0.0012 }).exp();
        let click = if v[4] > 0.0 {
            let c = self.f[0].process(n, 3000.0, 0.1, sr, FilterMode::Hp);
            let sqb = if model == 0 {
                sq((t * f0 * 8.0).fract()) * 0.4
            } else {
                0.0
            };
            (c + sqb) * ce * v[4] * 1.2
        } else {
            0.0
        };
        // body: a resonance tracking 2.4x the swept pitch
        let body = if v[5] > 0.0 {
            self.f[1].process(s, f * 2.4, 0.9, sr, FilterMode::Bp) * v[5] * 1.2
        } else {
            0.0
        };
        let nl = if v[7] > 0.0 {
            self.f[2].process(n, 2500.0, 0.1, sr, FilterMode::Lp) * v[7] * 0.6 * dec(t, decay * 0.4)
        } else {
            0.0
        };
        let y = (s + body) * amp + click + nl;
        (drive(y, v[6]) * 0.85, decay * 1.1 + 0.02)
    }

    /// v = tune, tone, snappy, decay, color, click, ring, drive
    fn snare(&mut self, model: u8, v: &[f32; 8], ratio: f32, sr: f32) -> (f32, f32) {
        let t = self.t;
        let f0 = v[0] * ratio;
        let (tone, snappy, decay, color, ring) = (v[1], v[2], v[3], v[4], v[5 + 1]);
        let se = dec(t, decay) * (t / 0.0005).min(1.0);
        let shell = match model {
            // 909: two partials with a quick pitch drop
            1 => {
                let f = f0 * (1.0 + 0.5 * (-t / 0.008).exp());
                self.ph[0] = (self.ph[0] + f / sr).fract();
                self.ph[1] = (self.ph[1] + f * 1.83 / sr).fract();
                ((self.ph[0] * TAU).sin() * 0.7 + (self.ph[1] * TAU).sin() * 0.4) / 1.1 * se
            }
            // modal shell: drum-head ratios (Bessel zeros) as decaying sines
            2 => {
                const R: [f32; 5] = [1.0, 1.593, 2.135, 2.295, 2.653];
                let mut s = 0.0;
                for (i, r) in R.iter().enumerate() {
                    self.ph[i] = (self.ph[i] + f0 * r / sr).fract();
                    let a = if i == 0 { 1.0 } else { 0.25 + 0.4 * ring };
                    s += (self.ph[i] * TAU).sin() * a * dec(t, decay * (1.0 - 0.12 * i as f32));
                }
                s / 1.8 * (t / 0.0005).min(1.0)
            }
            // 808 (sc808): two sines 11 semitones apart
            _ => {
                self.ph[0] = (self.ph[0] + f0 / sr).fract();
                self.ph[1] = (self.ph[1] + f0 * 0.529_7 / sr).fract();
                ((self.ph[0] * TAU).sin() * 0.7 + (self.ph[1] * TAU).sin() * 0.6) / 1.3 * se
            }
        };
        // ring: an extra head mode that lasts longer
        let rg = if ring > 0.0 && model != 2 {
            self.ph[5] = (self.ph[5] + f0 * 1.593 / sr).fract();
            (self.ph[5] * TAU).sin() * ring * 0.5 * dec(t, decay * 2.5)
        } else {
            0.0
        };
        let n = noise(&mut self.nz);
        let hp = self.f[0].process(n, color * 0.25, 0.1, sr, FilterMode::Hp);
        let nb = self.f[1].process(hp, color, 0.25, sr, FilterMode::Lp);
        let ne = dec(t, snappy) * (t / 0.0008).min(1.0);
        let click = n * (-t / 0.0007).exp() * v[5] * 0.8;
        let y = (1.0 - tone) * (shell + rg) * 1.2 + tone * nb * ne * 1.6 + click;
        (
            drive(y, v[7]) * 0.8,
            decay.max(snappy) * 1.1 + decay * 1.5 * (ring > 0.0) as u8 as f32 + 0.02,
        )
    }

    /// v = tune, decay, metal, tone, color, width, click, drive
    fn hat(&mut self, model: u8, v: &[f32; 8], ratio: f32, sr: f32) -> (f32, f32, bool, f32) {
        let t = self.t;
        let tune = v[0] * ratio;
        let (decay, metal, tone, color, width) = (v[1], v[2], v[3], v[4], v[5]);
        let stereo = width > 0.01;
        let metal_of = |ph: &mut [f32; 6], det: f32, model: u8| -> f32 {
            let mut s = [0.0f32; 6];
            for i in 0..6 {
                ph[i] = (ph[i] + (HAT808[i] * tune * det / sr).min(0.5)).fract();
                s[i] = sq(ph[i]);
            }
            match model {
                1 => (s[0] * s[1] + s[2] * s[3] + s[4] * s[5]) / 3.0,
                _ => s.iter().sum::<f32>() / 6.0,
            }
        };
        let n = noise(&mut self.nz);
        let src_l = if model == 2 {
            // comb-filtered noise (fixed buffer) + a decaying FM "hit"
            let fc = (3100.0 * tune).clamp(200.0, sr * 0.45);
            let dl = ((sr / fc) as usize).clamp(2, 1023);
            let rd = (self.ci + 1024 - dl) % 1024;
            let y = n + self.comb[rd] * (0.4 + 0.55 * color);
            self.comb[self.ci] = y.clamp(-4.0, 4.0);
            self.ci = (self.ci + 1) % 1024;
            let fm_i = 3.0 * (-t / 0.02).exp();
            self.ph2 = (self.ph2 + 5200.0 * tune / sr).fract();
            let fm = (self.ph2 * TAU + fm_i * (self.ph2 * 1.37 * TAU).sin()).sin();
            metal * (y * 0.4 + fm * 0.5) + (1.0 - metal) * n
        } else {
            let m = metal_of(&mut self.ph, 1.0, model);
            metal * m * 1.6 + (1.0 - metal) * n
        };
        let env = dec(t, decay) * (t / 0.0003).min(1.0);
        let click = n * (-t / 0.0005).exp() * v[6];
        let res = if model == 2 { 0.2 } else { 0.2 + 0.7 * color };
        let shape = |f: &mut [Svf], x: f32| {
            let bp = f[0].process(x, tone * 1.15, res, sr, FilterMode::Bp);
            let hp = f[1].process(x, tone * 0.8, 0.3, sr, FilterMode::Hp);
            f[2].process(bp * 0.8 + hp, tone * 0.6, 0.1, sr, FilterMode::Hp)
        };
        let yl = shape(&mut self.f[0..3], src_l);
        let hg = [2.2, 1.4, 1.0][model.min(2) as usize];
        let l = drive((yl * env + click) * hg, v[7]);
        let end = decay * 1.1 + 0.01;
        if !stereo {
            return (l, l, false, end);
        }
        // the right side: slightly detuned metal and its own noise
        let nr = noise(&mut self.nz_r);
        let src_r = if model == 2 {
            metal * (src_l - (1.0 - metal) * n) + (1.0 - metal) * nr
        } else {
            let m = metal_of(&mut self.ph_r, 1.0 + 0.008 * width, model);
            metal * m * 1.6 + (1.0 - metal) * nr
        };
        let yr = shape(&mut self.f_r[..], src_r);
        let r = drive((yr * env + click) * hg, v[7]);
        // width blends from mono to fully independent sides
        let w = width.min(1.0);
        let mid = 0.5 * (l + r);
        (mid + (l - mid) * w, mid + (r - mid) * w, true, end)
    }

    /// v = tone, decay, spread, claps, q, room, width, drive
    fn clap(&mut self, model: u8, v: &[f32; 8], sr: f32) -> (f32, f32, bool, f32) {
        let t = self.t;
        let (tone, decay, spread, claps, q, room, width) =
            (v[0], v[1], v[2], v[3].round().max(1.0), v[4], v[5], v[6]);
        let (spread, burst) = if model == 1 {
            (spread * 0.6, 0.0015)
        } else {
            (spread, 0.0025)
        };
        let k = (t / spread).floor();
        let be = if k < claps {
            (-(t - k * spread) / burst).exp()
        } else {
            0.0
        };
        let t0 = (claps - 1.0) * spread;
        let te = if t >= t0 { dec(t - t0, decay) } else { 0.0 };
        let e = be.max(te * 0.85);
        let n = noise(&mut self.nz);
        let nr = noise(&mut self.nz_r);
        let w = width.min(1.0) * 0.8;
        let res = 0.3 + 0.6 * q;
        let side = |f: &mut [Svf], x: f32| {
            let h = f[0].process(x, 500.0, 0.1, sr, FilterMode::Hp);
            f[1].process(h, tone, res, sr, FilterMode::Bp) * 2.2
        };
        let xl = n * e;
        let xr = ((1.0 - w) * n + w * nr) * e;
        let mut l = side(&mut self.f[0..2], xl);
        let mut r = if w > 0.0 {
            side(&mut self.f_r[0..2], xr)
        } else {
            l
        };
        if room > 0.0 {
            let re = dec(t, decay * 3.0 + 0.3) * (t / 0.01).min(1.0);
            let rv = self.f[2].process(n, tone * 1.5, 0.1, sr, FilterMode::Lp) * room * 0.25 * re;
            let rr =
                self.f_r[2].process(nr, tone * 1.5, 0.1, sr, FilterMode::Lp) * room * 0.25 * re;
            l += rv;
            r += if w > 0.0 { rr } else { rv };
        }
        let end = t0
            + decay * (1.1 + 3.0 * (room > 0.0) as u8 as f32)
            + 0.3 * (room > 0.0) as u8 as f32
            + 0.02;
        (drive(l, v[7]), drive(r, v[7]), w > 0.0, end)
    }

    /// v = tune, punch, ptime, decay, click, noise, body, drive
    fn tom(&mut self, model: u8, v: &[f32; 8], ratio: f32, sr: f32) -> (f32, f32) {
        let t = self.t;
        let f0 = v[0] * ratio;
        let (punch, pt, decay) = (v[1], v[2], v[3]);
        let (f, decay) = if model == 1 {
            // conga: a quick upward blip of pitch, shorter
            (
                f0 * 2f32.powf(punch / 12.0 * (-t / (pt * 0.3)).exp()),
                decay * 0.6,
            )
        } else {
            // 808 tom (sc808): note*1.33 -> *1.125 -> note
            (
                f0 * (1.0 + 0.125 * (-t / 0.25).exp() + 0.21 * (-t / 0.05).exp())
                    * 2f32.powf(punch / 12.0 * (-t / pt).exp()),
                decay,
            )
        };
        self.ph[0] = (self.ph[0] + f / sr).fract();
        let s = (self.ph[0] * TAU).sin();
        let amp = dec(t, decay) * (t / 0.0006).min(1.0);
        let n = noise(&mut self.nz);
        let body = if v[6] > 0.0 {
            self.ph[1] = (self.ph[1] + f * 1.5 / sr).fract();
            (self.ph[1] * TAU).sin() * v[6] * 0.5 * dec(t, decay * 0.6)
        } else {
            0.0
        };
        let click = self.f[0].process(n, 2500.0, 0.2, sr, FilterMode::Hp)
            * (-t / if model == 1 { 0.002 } else { 0.001 }).exp()
            * v[4];
        let nl =
            self.f[1].process(n, f0 * 4.0, 0.2, sr, FilterMode::Bp) * v[5] * dec(t, decay * 0.3);
        let y = s * amp + body + click + nl;
        (drive(y, v[7]) * 0.85, decay * 1.1 + 0.02)
    }

    /// v = tune, decay, tone, color, ring, click, width, drive
    fn perc(&mut self, model: u8, v: &[f32; 8], ratio: f32, sr: f32) -> (f32, f32, bool, f32) {
        let t = self.t;
        let tune = v[0] * ratio;
        let (decay, tone, color, ring, click, width) = (v[1], v[2], v[3], v[4], v[5], v[6]);
        let n = noise(&mut self.nz);
        let env = dec(t, decay) * (t / 0.0005).min(1.0);
        let ck = n * (-t / 0.0006).exp() * click;
        let y = match model {
            // claves (sc808): a short sine at ~2.5 kHz, ring = a pinged band-pass
            1 => {
                let f = 2500.0 * tune * (1.0 + 0.05 * color);
                self.ph[0] = (self.ph[0] + f / sr).fract();
                let s = (self.ph[0] * TAU).sin() * env;
                let p = self.f[0].process(s + ck, f, 0.9 + 0.09 * ring, sr, FilterMode::Bp);
                (s + p * ring * 0.12) * 0.75 + ck * 0.5
            }
            // rimshot (sc808): triangle + 80 % pulse + noise, a 464 Hz bump
            2 => {
                self.ph[0] = (self.ph[0] + 1661.0 * 1.1 * tune / sr).fract();
                self.ph[1] = (self.ph[1] + 465.0 * 1.1 * tune / sr).fract();
                let tri = 4.0 * (self.ph[0] - 0.5).abs() - 1.0;
                let pu = if self.ph[1] < 0.8 { 1.0 } else { -1.0 };
                let e = dec(t, decay * 0.5);
                let x = (tri + pu * 0.6 + n * 0.46) * e;
                let bump = self.f[0].process(
                    x,
                    464.0 * tune * (1.0 + color),
                    0.5 + 0.4 * ring,
                    sr,
                    FilterMode::Bp,
                );
                let hp = self.f[1].process(x + bump * 2.0, 300.0, 0.1, sr, FilterMode::Hp);
                self.f[2].process(hp, 3000.0 + tone * 12000.0, 0.1, sr, FilterMode::Lp) * 0.6 + ck
            }
            // cymbal: the 808 metal set an octave down, long, plus noise
            3 => {
                let mut s = 0.0;
                #[allow(clippy::needless_range_loop)]
                for i in 0..6 {
                    self.ph[i] = (self.ph[i] + (HAT808[i] * 1.45 * tune / sr).min(0.5)).fract();
                    s += sq(self.ph[i]);
                }
                let x = (s / 6.0) * (1.0 - color) + n * color;
                let bp = self.f[0].process(
                    x,
                    3500.0 + tone * 6000.0,
                    0.3 + 0.6 * ring,
                    sr,
                    FilterMode::Bp,
                );
                let hp = self.f[1].process(x, 2500.0 + tone * 4000.0, 0.2, sr, FilterMode::Hp);
                (bp + hp) * env * 1.2 + ck
            }
            // zap: a fast falling pitch, sine -> square by color
            4 => {
                let x = (t / (decay * 0.5)).min(1.0);
                let f = 180.0 * tune * 2f32.powf(5.0 * (1.0 - x).powi(2));
                self.ph[0] = (self.ph[0] + f / sr).fract();
                let s = (self.ph[0] * TAU).sin() * (1.0 - color) + sq(self.ph[0]) * color * 0.6;
                let p = self.f[0].process(s, f * 2.0, 0.5 + 0.45 * ring, sr, FilterMode::Bp);
                let y = (s + p * ring) * env;
                self.f[1].process(y, 2000.0 + tone * 14000.0, 0.1, sr, FilterMode::Lp) + ck
            }
            // cowbell (sc808): 540 + 800 Hz pulses, a hard first strike, band-pass
            _ => {
                let det = 1.0 + 0.1 * (color - 0.3);
                self.ph[0] = (self.ph[0] + 800.0 * tune / sr).fract();
                self.ph[1] = (self.ph[1] + 540.0 * tune * det / sr).fract();
                let p = (sq(self.ph[0]) + sq(self.ph[1])) * 0.5;
                let atk = (-t / 0.012).exp();
                let x = p * (atk * 1.5 + env);
                let bp = self.f[0].process(
                    x,
                    800.0 * tune * (0.8 + tone * 1.6),
                    0.4 + 0.5 * ring,
                    sr,
                    FilterMode::Bp,
                );
                let hp =
                    self.f[1].process(bp * 1.5 + x * 0.2, 400.0 * tune, 0.1, sr, FilterMode::Hp);
                hp * 0.3 + ck
            }
        };
        let l = drive(y * 0.9, v[7]);
        // width: a 0.4..1.4 ms Haas offset is not allocation-free here, so the
        // right side gets its own transient noise and a touch of detune instead
        let r = if width > 0.01 {
            let nr = noise(&mut self.nz_r);
            l * (1.0 - 0.3 * width) + nr * env * width * 0.08
        } else {
            l
        };
        let end = match model {
            2 => decay * 0.55,
            _ => decay * 1.1,
        } + 0.02;
        (l, r, width > 0.01, end)
    }
}

/// Factory generator presets: (name, tip, patch)
fn presets1() -> Vec<(&'static str, &'static str, GenPatch)> {
    use crate::chipfx::Category;
    use DrumKind::*;
    let d = |k, m, v: &[f32]| GenPatch::drum(DrumP::new(k, m, v));
    vec![
        ("Psy Zap Kick", "long deep zap sweep, tuned to E (C-4 = 41 Hz)", d(Kick, 3, &[41.2, 36.0, 0.035, 0.42, 0.35, 0.1, 0.3, 0.0])),
        ("808 Boom", "sc808 long sine kick", d(Kick, 1, &[45.0, 34.0, 0.03, 1.4, 0.15, 0.0, 0.1, 0.0])),
        ("909 Kick", "short punchy 909", d(Kick, 2, &[52.0, 28.0, 0.025, 0.35, 0.5, 0.15, 0.35, 0.05])),
        ("v0 Punch Kick", "clicky modern kick", d(Kick, 0, &[50.0, 40.0, 0.02, 0.5, 0.6, 0.3, 0.25, 0.08])),
        ("808 Snare", "sc808: two sines + noise", d(Snare, 0, &[190.0, 0.65, 0.22, 0.12, 5200.0, 0.3, 0.1, 0.1])),
        ("909 Snare", "bright 909 crack", d(Snare, 1, &[220.0, 0.6, 0.18, 0.09, 7500.0, 0.5, 0.0, 0.2])),
        ("Shell Snare", "modal drum head that rings", d(Snare, 2, &[170.0, 0.45, 0.25, 0.3, 4500.0, 0.3, 0.6, 0.1])),
        ("Closed Hat", "808 metal, short", d(Hat, 0, &[1.0, 0.045, 0.75, 8000.0, 0.3, 0.3, 0.2, 0.0])),
        ("Open Hat", "808 metal, long", d(Hat, 0, &[1.0, 0.45, 0.75, 7500.0, 0.3, 0.4, 0.1, 0.0])),
        ("Ring Hat", "Plaits-style ring-modulated pairs", d(Hat, 1, &[1.2, 0.08, 0.85, 9000.0, 0.4, 0.5, 0.1, 0.0])),
        ("Comb Hat", "noise comb + FM hit", d(Hat, 2, &[1.0, 0.12, 0.7, 7000.0, 0.6, 0.5, 0.2, 0.0])),
        ("808 Clap", "four bursts and a tail", d(Clap, 0, &[1300.0, 0.3, 0.009, 4.0, 0.35, 0.2, 0.5, 0.0])),
        ("Tight Clap", "short, dry", d(Clap, 1, &[1600.0, 0.15, 0.008, 3.0, 0.4, 0.0, 0.3, 0.1])),
        ("808 Tom", "sc808 mid tom", d(Tom, 0, &[130.0, 5.0, 0.08, 0.5, 0.2, 0.05, 0.1, 0.0])),
        ("Conga", "short, with a blip", d(Tom, 1, &[260.0, 6.0, 0.03, 0.35, 0.3, 0.0, 0.2, 0.0])),
        ("Cowbell", "sc808 540 + 800 Hz", d(Perc, 0, &[1.0, 0.25, 0.6, 0.3, 0.3, 0.2, 0.0, 0.0])),
        ("Claves", "sc808 claves", d(Perc, 1, &[1.0, 0.08, 0.5, 0.3, 0.4, 0.3, 0.0, 0.0])),
        ("Rimshot", "sc808 rim", d(Perc, 2, &[1.0, 0.07, 0.6, 0.0, 0.3, 0.4, 0.0, 0.0])),
        ("Cymbal", "long metal wash", d(Perc, 3, &[1.0, 1.4, 0.6, 0.35, 0.3, 0.1, 0.5, 0.0])),
        ("Zap Perc", "psy laser zap", d(Perc, 4, &[1.0, 0.25, 0.5, 0.6, 0.2, 0.0, 0.3, 0.0])),
        (
            "Psy Kit",
            "kit mode: C kick, D snare, D# clap, F# closed hat, A# open hat, E-A toms, C# cowbell, B zap",
            GenPatch {
                engine: GenEngine::Kit(KitP::default()),
                ..Default::default()
            },
        ),
        ("808 Kit", "kit mode with 808 models", {
            let mut k = KitP::default();
            k.pieces[0] = DrumP::new(Kick, 1, &[45.0, 34.0, 0.03, 1.2, 0.15, 0.0, 0.1, 0.0]);
            k.pieces[6] = DrumP::new(Perc, 2, &[1.0, 0.07, 0.6, 0.0, 0.3, 0.4, 0.0, 0.0]);
            k.pieces[7] = DrumP::new(Perc, 1, &[1.0, 0.08, 0.5, 0.3, 0.4, 0.3, 0.0, 0.0]);
            GenPatch {
                engine: GenEngine::Kit(k),
                ..Default::default()
            }
        }),
        ("Coin", "chip FX: sfxr Pickup", fx(Category::Pickup, 3)),
        ("Laser Shot", "chip FX: sfxr Laser", fx(Category::Laser, 2)),
        ("Explosion", "chip FX: sfxr Explosion", fx(Category::Explosion, 4)),
        ("Power Up", "chip FX: sfxr Power-up", fx(Category::PowerUp, 1)),
        ("Hit", "chip FX: sfxr Hit", fx(Category::Hit, 5)),
        ("Jump", "chip FX: sfxr Jump", fx(Category::Jump, 2)),
        ("Blip", "chip FX: sfxr Blip", fx(Category::Blip, 6)),
        ("Psy Laser", "chip FX: a falling saw laser with phaser, for psy fills", {
            use crate::chipfx::*;
            let mut s = SfxP {
                wave: Wave::Saw,
                category: 1,
                ..Default::default()
            };
            s.p[BASE_FREQ] = 0.62;
            s.p[FREQ_RAMP] = -0.38;
            s.p[FREQ_LIMIT] = 0.1;
            s.p[ENV_ATTACK] = 0.0;
            s.p[ENV_SUSTAIN] = 0.28;
            s.p[ENV_DECAY] = 0.35;
            s.p[ENV_PUNCH] = 0.2;
            s.p[PHA_OFFSET] = 0.18;
            s.p[PHA_RAMP] = -0.15;
            s.p[LPF_FREQ] = 0.8;
            s.p[LPF_RES] = 0.4;
            GenPatch::chip_fx(s)
        }),
    ]
}

/// every preset, with the stage-2 generators (Analog, FM-4, Noise)
pub fn presets() -> Vec<(&'static str, &'static str, GenPatch)> {
    let mut v = presets1();
    for (n, tip, e, glide, legato) in crate::gen2::presets() {
        let mut g = GenPatch {
            engine: e,
            ..Default::default()
        };
        g.base.glide = glide;
        g.base.legato = legato;
        v.push((n, tip, g));
    }
    v
}

fn fx(c: crate::chipfx::Category, seed: u32) -> GenPatch {
    let mut s = crate::chipfx::SfxP::default();
    s.category(c, seed);
    GenPatch::chip_fx(s)
}

pub fn preset(name: &str) -> Option<GenPatch> {
    presets()
        .into_iter()
        .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
        .map(|(n, _, mut p)| {
            p.base.name = n.into();
            p
        })
}

/// Render one note offline (for Bake to sample and tests): mono samples
/// until the generator finishes (at most `max_s` seconds).
pub fn bake(g: &GenPatch, freq: f32, sr: f32, max_s: f32) -> Vec<f32> {
    let mut v = crate::synth::SynthVoice::seeded(g.seed.wrapping_mul(2_654_435_761) | 1);
    let mut out = Vec::with_capacity((sr * max_s.min(2.0)) as usize);
    let hold = (sr * 1.2) as usize;
    for n in 0..(sr * max_s) as usize {
        if n == hold && g.engine.own_env() {
            v.amp.release();
        }
        let (l, r) = v.render_gen(&g.base, g, freq, sr, 120.0);
        out.push(0.5 * (l + r));
        if v.gen.done || v.amp.done() {
            break;
        }
    }
    // trim the silent tail and fade the last 2 ms
    while out.len() > 1 && out.last().is_some_and(|x| x.abs() < 1e-5) {
        out.pop();
    }
    let n = out.len();
    let f = ((sr * 0.002) as usize).min(n);
    for i in 0..f {
        out[n - f + i] *= 1.0 - i as f32 / f as f32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 48000.0;

    fn run(g: &GenPatch, freq: f32, secs: f32) -> Vec<(f32, f32)> {
        let mut v = crate::synth::SynthVoice::seeded(77);
        (0..(SR * secs) as usize)
            .map(|_| v.render_gen(&g.base, g, freq, SR, 120.0))
            .collect()
    }

    #[test]
    fn every_preset_sounds_at_a_sane_level_and_finishes() {
        for (name, _, g) in presets() {
            let freqs: Vec<f32> = match &g.engine {
                GenEngine::Kit(_) => (0..12).map(|k| C4 * 2f32.powf(k as f32 / 12.0)).collect(),
                _ => vec![C4],
            };
            for f in freqs {
                let o = run(&g, f, 3.0);
                let pk = o
                    .iter()
                    .fold(0.0f32, |m, (l, r)| m.max(l.abs()).max(r.abs()));
                assert!(
                    o.iter().all(|(l, r)| l.is_finite() && r.is_finite()),
                    "{name}"
                );
                assert!(
                    (0.15..=1.3).contains(&pk),
                    "{name} at {f:.0} Hz: peak {pk:.3}"
                );
                let mut v = crate::synth::SynthVoice::seeded(3);
                let mut n = 0;
                while !v.gen.done && n < (SR * 8.0) as usize {
                    if n == SR as usize {
                        v.amp.release();
                    }
                    v.render_gen(&g.base, &g, f, SR, 120.0);
                    n += 1;
                }
                assert!(v.gen.done, "{name} never finishes");
            }
        }
    }

    #[test]
    fn dice_is_seeded_respects_ranges_and_locks() {
        for kind in DrumKind::ALL {
            let mut a = GenPatch::drum(DrumP::new(kind, 0, &[]));
            let mut b = a.clone();
            a.dice(0, 1234);
            b.dice(0, 1234);
            assert_eq!(a, b, "same seed, same sound");
            let GenEngine::Drum(d) = &a.engine else {
                panic!()
            };
            for (v, def) in d.macros.iter().zip(kind.macros()) {
                assert!(
                    *v >= def.dice.0 && *v <= def.dice.1,
                    "{kind:?} {}",
                    def.name
                );
            }
            b.dice(0, 99);
            assert_ne!(a, b, "another seed rolls another sound");
            // locks
            let mut c = GenPatch::drum(DrumP::new(kind, 0, &[]));
            c.drum_mut(0).unwrap().locks = 0b0000_0101;
            c.drum_mut(0).unwrap().macros[0] = 0.987;
            c.drum_mut(0).unwrap().macros[2] = 0.012;
            for s in 0..20 {
                c.dice(0, s);
                c.mutate(0, s + 1000);
            }
            assert_eq!(c.drum_mut(0).unwrap().macros[0], 0.987);
            assert_eq!(c.drum_mut(0).unwrap().macros[2], 0.012);
            // mutate stays within ±amount
            let mut e = GenPatch::drum(DrumP::new(kind, 0, &[]));
            let before = e.drum_mut(0).unwrap().macros;
            e.mutate = 0.1;
            e.mutate(0, 5);
            for (x, y) in before.iter().zip(e.drum_mut(0).unwrap().macros) {
                assert!((x - y).abs() <= 0.1 + 1e-6);
            }
        }
    }

    #[test]
    fn extremes_and_random_rolls_never_blow_up() {
        let mut seed = 1;
        for kind in DrumKind::ALL {
            for model in 0..kind.models().len() as u8 {
                for corner in 0..6u32 {
                    let mut d = DrumP::new(kind, model, &[]);
                    match corner {
                        0 => d.macros = [0.0; 8],
                        1 => d.macros = [1.0; 8],
                        2 => d.macros = [f32::NAN, 1.0, 0.0, f32::INFINITY, 0.5, -3.0, 9.0, 1.0],
                        _ => {
                            seed += 1;
                            let mut r = Rng::new(seed);
                            for v in &mut d.macros {
                                *v = r.f();
                            }
                        }
                    }
                    let mut g = GenPatch::drum(d);
                    if corner == 2 {
                        // what load / fix would do with a hand-edited file
                        g.fix();
                    }
                    for f in [8.0, C4, 4000.0] {
                        let o = run(&g, f, 0.6);
                        for (l, r) in o {
                            assert!(l.is_finite() && r.is_finite(), "{kind:?} {model} {corner}");
                            assert!(l.abs() <= 4.0 && r.abs() <= 4.0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn kick_follows_tune_and_the_note_and_zap_lands_on_pitch() {
        // the tail of an 808 boom at C-4 sits on the Tune frequency
        let g = preset("808 Boom").unwrap();
        let o = run(&g, C4, 1.2);
        let zc = |o: &[(f32, f32)]| {
            o.windows(2)
                .filter(|w| w[0].0 <= 0.0 && w[1].0 > 0.0)
                .count()
        };
        let tail = &o[(SR * 0.8) as usize..(SR * 1.2) as usize];
        let f = zc(tail) as f32 / 0.4;
        assert!((f / 45.0 - 1.0).abs() < 0.08, "808 tail {f:.1} Hz");
        // an octave up plays an octave higher
        let o2 = run(&g, C4 * 2.0, 1.2);
        let f2 = zc(&o2[(SR * 0.8) as usize..(SR * 1.2) as usize]) as f32 / 0.4;
        assert!((f2 / 90.0 - 1.0).abs() < 0.08, "808 octave up {f2:.1} Hz");
        // Zap: tuned to E (41.2 Hz) after the sweep
        let z = preset("Psy Zap Kick").unwrap();
        let o = run(&z, C4, 0.4);
        let f = zc(&o[(SR * 0.2) as usize..(SR * 0.4) as usize]) as f32 / 0.2;
        assert!((f / 41.2 - 1.0).abs() < 0.15, "zap tail {f:.1} Hz");
    }

    #[test]
    fn kit_mode_picks_pieces_by_note() {
        let g = preset("Psy Kit").unwrap();
        let mut v = crate::synth::SynthVoice::seeded(1);
        for (semi, piece) in [
            (0, 0),
            (2, 1),
            (3, 2),
            (6, 3),
            (10, 4),
            (7, 5),
            (1, 6),
            (11, 7),
            (14, 1),
        ] {
            v.gen = GenState::default();
            v.render_gen(&g.base, &g, C4 * 2f32.powf(semi as f32 / 12.0), SR, 120.0);
            assert_eq!(v.gen.piece, piece, "semitone {semi}");
        }
    }

    #[test]
    fn hats_and_claps_get_wide_with_width_and_stay_mono_without() {
        for name in ["Closed Hat", "808 Clap", "Ring Hat", "Comb Hat"] {
            let mut g = preset(name).unwrap();
            let side = |g: &GenPatch| {
                let o = run(g, C4, 0.3);
                let (mut m, mut s) = (0.0f64, 0.0f64);
                for (l, r) in o {
                    m += ((l + r) as f64 * 0.5).powi(2);
                    s += ((l - r) as f64 * 0.5).powi(2);
                }
                10.0 * (s / m.max(1e-12)).log10()
            };
            g.drum_mut(0).unwrap().macros[if name.contains("Clap") { 6 } else { 5 }] = 0.9;
            assert!(side(&g) > -20.0, "{name} wide {:.1}", side(&g));
            g.drum_mut(0).unwrap().macros[if name.contains("Clap") { 6 } else { 5 }] = 0.0;
            assert!(side(&g) < -100.0, "{name} mono {:.1}", side(&g));
        }
    }

    #[test]
    fn old_fields_default_and_text_round_trips() {
        let g = preset("Psy Kit").unwrap();
        let t = toml::to_string(&g).unwrap();
        let back: GenPatch = toml::from_str(&t).unwrap();
        assert_eq!(g, back);
        // a sparse table (hand-written / older file) fills in the defaults
        let s: GenPatch = toml::from_str("[engine.Drum]\nkind = \"Snare\"\nmodel = 1\n").unwrap();
        let GenEngine::Drum(d) = &s.engine else {
            panic!()
        };
        assert_eq!(d.kind, DrumKind::Snare);
        assert_eq!(d.level, 1.0);
        assert!(s.base.amp.s > 0.9 && !s.filter_on);
    }

    #[test]
    fn bake_renders_until_the_drum_ends() {
        let g = preset("Closed Hat").unwrap();
        let b = bake(&g, C4, SR, 4.0);
        assert!(
            b.len() > 1000 && b.len() < (SR * 0.2) as usize,
            "{}",
            b.len()
        );
        assert!(b.iter().all(|x| x.is_finite()));
        assert!(b.last().unwrap().abs() < 1e-3);
    }
}
#[cfg(test)]
mod levels_dbg {
    #[test]
    #[ignore]
    fn print_levels() {
        for (name, _, g) in super::presets() {
            let freqs: Vec<f32> = match &g.engine {
                super::GenEngine::Kit(_) => (0..12)
                    .map(|k| super::C4 * 2f32.powf(k as f32 / 12.0))
                    .collect(),
                _ => vec![super::C4],
            };
            for f in freqs {
                let mut v = crate::synth::SynthVoice::seeded(77);
                let o: Vec<(f32, f32)> = (0..48000 * 2)
                    .map(|_| v.render_gen(&g.base, &g, f, 48000.0, 120.0))
                    .collect();
                let pk = o
                    .iter()
                    .fold(0.0f32, |m, (l, r)| m.max(l.abs()).max(r.abs()));
                let n = (48000.0 * 0.1) as usize;
                let rms = (o[..n].iter().map(|(l, _)| l * l).sum::<f32>() / n as f32).sqrt();
                eprintln!(
                    "{name:16} {f:6.0} peak {pk:.3} rms100ms {:.1} dB",
                    20.0 * rms.log10()
                );
            }
        }
    }
}

/// `scripts/make_features_02312.py` builds the 0.2.3.12 Generators demo
#[cfg(test)]
mod features {
    use crate::model::{Cell, Instrument, InstrumentKind, Song};

    fn note(s: &str) -> u8 {
        let names = [
            "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
        ];
        let k = names.iter().position(|n| s.starts_with(n)).unwrap();
        let o: u8 = s[2..].parse().unwrap();
        o * 12 + k as u8 + 1
    }

    fn put(s: &mut Song, b: usize, r: usize, ch: usize, n: &str, ins: u8, fx: (u8, u8)) {
        s.patterns[b].cells[r][ch] = Cell {
            note: note(n),
            instr: ins,
            vol: 0,
            fx: fx.0,
            fxp: fx.1,
        };
    }

    fn fxonly(s: &mut Song, b: usize, r: usize, ch: usize, fx: (u8, u8)) {
        let c = &mut s.patterns[b].cells[r][ch];
        c.fx = fx.0;
        c.fxp = fx.1;
    }

    fn add(s: &mut Song, name: &str, vol: f32, kind: InstrumentKind) -> u8 {
        let i = Instrument {
            name: name.into(),
            volume: vol,
            transpose: 0,
            fx: vec![],
            kind,
        };
        s.add_instrument(i).unwrap() as u8 + 1
    }

    fn gen(s: &mut Song, name: &str, vol: f32) -> u8 {
        add(
            s,
            name,
            vol,
            InstrumentKind::Gen(super::preset(name).unwrap()),
        )
    }

    #[test]
    #[ignore]
    fn features_02312_demo() {
        let inp = std::env::var("CW_F2312_IN").expect("CW_F2312_IN");
        let out = std::env::var("CW_F2312_OUT").expect("CW_F2312_OUT");
        let mut s = crate::fileio::load_text_song(std::path::Path::new(&inp)).unwrap();
        let s = &mut s;
        // block 0: the four kicks, two hits each
        for (k, n) in ["Psy Zap Kick", "808 Boom", "909 Kick", "v0 Punch Kick"]
            .iter()
            .enumerate()
        {
            let i = gen(s, n, 0.85);
            put(s, 0, k * 8, 0, "C-4", i, (0, 0));
            put(s, 0, k * 8 + 4, 0, "C-4", i, (0, 0));
        }
        // block 1: snares and claps
        for (k, n) in [
            "808 Snare",
            "909 Snare",
            "Shell Snare",
            "808 Clap",
            "Tight Clap",
        ]
        .iter()
        .enumerate()
        {
            let i = gen(s, n, 0.8);
            put(s, 1, k * 6, 2, "C-4", i, (0, 0));
            put(s, 1, k * 6 + 3, 2, "C-4", i, (0, 0));
        }
        // block 2: hats
        for (k, (n, step)) in [
            ("Closed Hat", 1),
            ("Open Hat", 4),
            ("Ring Hat", 2),
            ("Comb Hat", 2),
        ]
        .iter()
        .enumerate()
        {
            let i = gen(s, n, 0.8);
            for r in (0..8).step_by(*step) {
                put(s, 2, k * 8 + r, 1, "C-4", i, (0, 0));
            }
        }
        // block 3: toms, then percussion
        let t = gen(s, "808 Tom", 0.8);
        for (r, n) in [(0, "C-4"), (2, "A-3"), (4, "F-3"), (6, "D-3")] {
            put(s, 3, r, 3, n, t, (0, 0));
        }
        let c = gen(s, "Conga", 0.8);
        for (r, n) in [(8, "C-4"), (10, "C-4"), (11, "G-3"), (12, "C-4")] {
            put(s, 3, r, 3, n, c, (0, 0));
        }
        for (k, n) in ["Cowbell", "Claves", "Rimshot", "Cymbal", "Zap Perc"]
            .iter()
            .enumerate()
        {
            let i = gen(s, n, 0.75);
            put(s, 3, 16 + k * 3, 3, "C-4", i, (0, 0));
        }
        // blocks 4-5: Dice. 8 dice rolls of the Psy Zap Kick (block 4), then
        // 8 mutations (25 %) of the original (block 5); hats re-rolled every 2 beats
        let kick = super::preset("Psy Zap Kick").unwrap();
        let hat = super::preset("Closed Hat").unwrap();
        for b in 4..6 {
            for k in 0..8 {
                let mut g = kick.clone();
                if b == 4 {
                    g.dice(0, 101 + k as u32);
                } else {
                    g.mutate = 0.25;
                    g.mutate(0, 201 + k as u32);
                }
                let tag = if b == 4 { "Dice" } else { "Mutate" };
                let i = add(
                    s,
                    &format!("{tag} kick {}", k + 1),
                    0.85,
                    InstrumentKind::Gen(g),
                );
                put(s, b, k * 4, 0, "C-4", i, (0, 0));
                if k % 2 == 0 {
                    let mut h = hat.clone();
                    h.dice(0, 301 + (b * 8 + k) as u32);
                    let j = add(
                        s,
                        &format!("Dice hat {}", b * 8 + k),
                        0.7,
                        InstrumentKind::Gen(h),
                    );
                    for r in [2, 3, 6, 7] {
                        put(s, b, k * 4 + r, 1, "C-4", j, (0, 0));
                    }
                }
            }
        }
        // block 6: 43xy on the Psy Zap Kick: decay (macro 4) grows, punch
        // (macro 2) drops, then the closed hat's metal (macro 3)
        let pk = gen(s, "Psy Zap Kick", 0.85);
        let ch = gen(s, "Closed Hat", 0.75);
        for beat in 0..8 {
            let fx = if beat < 4 {
                (0x43, 0x40 | (2 + beat as u8 * 4))
            } else {
                (0x43, 0x20 | (15 - (beat as u8 - 4) * 4))
            };
            put(s, 6, beat * 4, 0, "C-4", pk, fx);
            put(
                s,
                6,
                beat * 4 + 2,
                1,
                "C-4",
                ch,
                (0x43, 0x30 | (beat as u8 * 2)),
            );
        }
        // blocks 7-9: one acid line through SVF, clean ladder, vintage ladder,
        // with a 38xx cutoff sweep up and back
        let line = [
            "E-2", "E-2", "E-3", "E-2", "G-2", "E-2", "A#2", "E-2", "E-2", "D-3", "E-2", "E-2",
            "G-2", "E-3", "E-2", "B-2",
        ];
        for (b, fm) in (7..10).zip(crate::ladder::FilterModel::ALL) {
            let p = crate::synth::Patch {
                name: fm.name().into(),
                osc1: crate::synth::Wave::Saw,
                cutoff: 300.0,
                resonance: 0.82,
                env_amount: 2.0,
                fenv: crate::synth::Adsr {
                    a: 0.001,
                    d: 0.18,
                    s: 0.0,
                    r: 0.05,
                },
                amp: crate::synth::Adsr {
                    a: 0.002,
                    d: 0.2,
                    s: 0.7,
                    r: 0.04,
                },
                filter_model: fm,
                drive: 0.2,
                ..Default::default()
            };
            let i = add(s, fm.name(), 0.55, InstrumentKind::Synth(p));
            for r in 0..32 {
                let tri = if r < 16 { r } else { 31 - r } as u8;
                put(s, b, r, 4, line[r % 16], i, (0x38, 0x50 + tri * 6));
            }
            for r in (0..32).step_by(4) {
                put(s, b, r, 0, "C-4", pk, (0x43, 0x00));
            }
        }
        // blocks 10-11: a psy groove on the Psy Kit (one kit, the note picks
        // the drum), a vintage-ladder rolling bass; 43xy shortens the kick in 11
        let kit = gen(s, "Psy Kit", 0.85);
        let bp = crate::synth::Patch {
            name: "Rolling bass".into(),
            osc1: crate::synth::Wave::Saw,
            cutoff: 500.0,
            resonance: 0.55,
            env_amount: 1.6,
            fenv: crate::synth::Adsr {
                a: 0.001,
                d: 0.09,
                s: 0.0,
                r: 0.03,
            },
            amp: crate::synth::Adsr {
                a: 0.001,
                d: 0.1,
                s: 0.5,
                r: 0.02,
            },
            filter_model: crate::ladder::FilterModel::Vintage,
            ..Default::default()
        };
        let bass = add(s, "Rolling bass", 0.6, InstrumentKind::Synth(bp));
        for b in 10..12 {
            for beat in 0..8 {
                let r = beat * 4;
                let kfx = if b == 11 {
                    (0x43, 0x40 | ((beat as u8 % 4) * 3 + 3))
                } else {
                    (0, 0)
                };
                put(s, b, r, 0, "C-4", kit, kfx);
                put(s, b, r + 2, 1, "A#4", kit, (0, 0));
                put(s, b, r + 3, 1, "F#4", kit, (0, 0));
                if b == 11 {
                    put(s, b, r + 1, 1, "F#4", kit, (0, 0));
                }
                if beat % 2 == 1 {
                    put(s, b, r, 2, "D#4", kit, (0, 0));
                }
                for k in 1..4 {
                    let n = if beat == 7 && k == 3 { "G-2" } else { "E-2" };
                    put(s, b, r + k, 4, n, bass, (0x38, 0x60 + (beat as u8) * 4));
                }
            }
            put(s, b, 14, 3, "C#4", kit, (0, 0));
            put(s, b, 27, 3, "B-4", kit, (0, 0));
            put(s, b, 30, 3, "C#4", kit, (0, 0));
            fxonly(s, b, 31, 0, (0x43, 0x00));
        }
        crate::fileio::save_cwt(s, std::path::Path::new(&out)).unwrap();
    }

    fn fxi(s: &mut Song, name: &str, sfx: crate::chipfx::SfxP, vol: f32) -> u8 {
        add(
            s,
            name,
            vol,
            InstrumentKind::Gen(super::GenPatch::chip_fx(sfx)),
        )
    }

    #[test]
    #[ignore]
    fn features_02313_demo() {
        use crate::chipfx::{Category, SfxP};
        let inp = std::env::var("CW_F2313_IN").expect("CW_F2313_IN");
        let out = std::env::var("CW_F2313_OUT").expect("CW_F2313_OUT");
        let mut s = crate::fileio::load_text_song(std::path::Path::new(&inp)).unwrap();
        let s = &mut s;
        // blocks 0-6: three rolls of each category button
        for (b, c) in Category::ALL.iter().enumerate() {
            for k in 0..3 {
                let mut x = SfxP::default();
                x.category(*c, 11 + k as u32);
                let i = fxi(s, &format!("{} {}", c.name(), k + 1), x, 0.7);
                put(s, b, k * 10, 5, "C-4", i, (0, 0));
            }
        }
        // block 7: Randomize x4
        for k in 0..4 {
            let mut x = SfxP::default();
            x.randomize(40 + k as u32);
            let i = fxi(s, &format!("Random {}", k + 1), x, 0.6);
            put(s, 7, k * 8, 5, "C-4", i, (0, 0));
        }
        // block 8: Laser Shot, then three 25 % mutations of it
        let super::GenEngine::ChipFx(laser) = super::preset("Laser Shot").unwrap().engine else {
            panic!()
        };
        for k in 0..4 {
            let mut x = laser;
            if k > 0 {
                x.mutate(0.25, 60 + k as u32);
            }
            let i = fxi(s, &format!("Laser mutate {k}"), x, 0.7);
            put(s, 8, k * 8, 5, "C-4", i, (0, 0));
            put(s, 8, k * 8 + 4, 5, "C-4", i, (0, 0));
        }
        // block 9: Psy Laser played as a melody, Coin arpeggio on top
        let pl = gen(s, "Psy Laser", 0.6);
        let coin = gen(s, "Coin", 0.5);
        for (k, n) in ["E-3", "G-3", "A-3", "B-3", "E-4", "D-4", "B-3", "G-3"]
            .iter()
            .enumerate()
        {
            put(s, 9, k * 4, 5, n, pl, (0, 0));
        }
        for (r, n) in [
            (16, "E-4"),
            (18, "G-4"),
            (20, "B-4"),
            (22, "E-5"),
            (28, "E-5"),
        ] {
            put(s, 9, r, 6, n, coin, (0, 0));
        }
        // blocks 10-11: groove: Psy Kit + rolling bass + chip FX accents
        let kit = gen(s, "Psy Kit", 0.85);
        let bp = crate::synth::Patch {
            name: "Rolling bass".into(),
            osc1: crate::synth::Wave::Saw,
            cutoff: 500.0,
            resonance: 0.5,
            env_amount: 1.6,
            fenv: crate::synth::Adsr {
                a: 0.001,
                d: 0.09,
                s: 0.0,
                r: 0.03,
            },
            amp: crate::synth::Adsr {
                a: 0.001,
                d: 0.1,
                s: 0.5,
                r: 0.02,
            },
            filter_model: crate::ladder::FilterModel::Vintage,
            ..Default::default()
        };
        let bass = add(s, "Rolling bass", 0.6, InstrumentKind::Synth(bp));
        let ls = gen(s, "Laser Shot", 0.45);
        let ex = gen(s, "Explosion", 0.6);
        let pu = gen(s, "Power Up", 0.45);
        let bl = gen(s, "Blip", 0.4);
        for b in 10..12 {
            for beat in 0..8 {
                let r = beat * 4;
                put(s, b, r, 0, "C-4", kit, (0, 0));
                put(s, b, r + 2, 1, "A#4", kit, (0, 0));
                put(s, b, r + 3, 1, "F#4", kit, (0, 0));
                if beat % 2 == 1 {
                    put(s, b, r, 2, "D#4", kit, (0, 0));
                }
                for k in 1..4 {
                    put(s, b, r + k, 4, "E-2", bass, (0, 0));
                }
            }
            // lasers on the offbeats, the Freq knob (43 1y) stepping up
            for (k, r) in [6usize, 14, 22].iter().enumerate() {
                put(s, b, *r, 5, "C-4", ls, (0x43, 0x18 + k as u8 * 3));
            }
            put(s, b, 10, 6, "E-4", bl, (0, 0));
            put(s, b, 11, 6, "B-4", bl, (0, 0));
            if b == 10 {
                put(s, b, 24, 6, "C-4", pu, (0, 0));
            } else {
                put(s, b, 28, 5, "C-4", ex, (0, 0));
            }
        }
        crate::fileio::save_cwt(s, std::path::Path::new(&out)).unwrap();
    }

    fn off(s: &mut Song, b: usize, r: usize, ch: usize) {
        s.patterns[b].cells[r][ch].note = crate::model::NOTE_OFF;
    }

    /// a whole-block chord on channels 4.. with note-offs at row `end`
    fn chord(s: &mut Song, b: usize, r: usize, end: usize, ns: &[&str], ins: u8) {
        for (k, n) in ns.iter().enumerate() {
            put(s, b, r, 4 + k, n, ins, (0, 0));
            if end < 32 {
                off(s, b, end, 4 + k);
            }
        }
    }

    #[test]
    #[ignore]
    fn features_02314_demo() {
        let inp = std::env::var("CW_F2314_IN").expect("CW_F2314_IN");
        let out = std::env::var("CW_F2314_OUT").expect("CW_F2314_OUT");
        let mut s = crate::fileio::load_text_song(std::path::Path::new(&inp)).unwrap();
        let s = &mut s;
        let kit = gen(s, "Psy Kit", 0.85);
        // 0 Fat Moog Bass: legato line, the overlaps glide
        let fm = gen(s, "Fat Moog Bass", 1.0);
        for (r, n) in [
            (0, "E-2"),
            (6, "E-2"),
            (8, "G-2"),
            (12, "A-2"),
            (14, "B-2"),
            (16, "E-2"),
            (22, "D-3"),
            (24, "B-2"),
            (28, "G-2"),
        ] {
            put(s, 0, r, 3, n, fm, (0, 0));
        }
        off(s, 0, 4, 3);
        off(s, 0, 31, 3);
        // 1 Psy Rolling Bass with the kick: kick + three bass 16ths, E then F#
        let pb = gen(s, "Psy Rolling Bass", 0.85);
        for beat in 0..8 {
            let r = beat * 4;
            put(s, 1, r, 0, "C-4", kit, (0, 0));
            let n = if beat >= 6 { "F#1" } else { "E-1" };
            for k in 1..4 {
                put(s, 1, r + k, 3, n, pb, (0, 0));
            }
        }
        // 2 Warm Lead: E minor phrase with glide
        let wl = gen(s, "Warm Lead", 0.95);
        for (r, n) in [
            (0, "E-4"),
            (6, "G-4"),
            (8, "B-4"),
            (12, "A-4"),
            (14, "G-4"),
            (16, "F#4"),
            (20, "E-4"),
            (24, "D-4"),
            (26, "E-4"),
        ] {
            put(s, 2, r, 4, n, wl, (0, 0));
        }
        off(s, 2, 31, 4);
        // 3 Brass stabs
        let br = gen(s, "Brass", 0.6);
        for (r, end, ns) in [
            (0, 6, ["E-3", "G-3", "B-3"]),
            (8, 14, ["D-3", "F#3", "A-3"]),
            (16, 30, ["C-3", "E-3", "G-3"]),
        ] {
            chord(s, 3, r, end, &ns, br);
        }
        // 4 Dark Pad: Em(add9) held
        let dp = gen(s, "Dark Pad", 0.4);
        chord(s, 4, 0, 28, &["E-3", "G-3", "B-3", "F#4"], dp);
        // 5 Acid Squelch: 16ths with the cutoff macro (43 1y) climbing
        let ac = gen(s, "Acid Squelch", 0.95);
        let line = ["E-2", "E-2", "E-3", "E-2", "G-2", "E-2", "A#2", "E-2"];
        for r in 0..32 {
            if r % 8 == 7 {
                continue;
            }
            put(s, 5, r, 4, line[r % 8], ac, (0x43, 0x14 + (r / 4) as u8));
        }
        off(s, 5, 31, 4);
        // 6 E-Piano chords
        let ep = gen(s, "E-Piano", 0.6);
        chord(s, 6, 0, 14, &["E-3", "G-3", "B-3", "D-4"], ep);
        chord(s, 6, 16, 30, &["C-3", "E-3", "G-3", "B-3"], ep);
        // 7 Metallic Bell
        let mb = gen(s, "Metallic Bell", 1.0);
        for (r, n) in [(0, "E-4"), (8, "B-4"), (16, "G-4"), (24, "D-5")] {
            put(s, 7, r, 4, n, mb, (0, 0));
        }
        // 8 Psy Zap Bass rolling
        let zb = gen(s, "Psy Zap Bass", 0.6);
        for beat in 0..8 {
            put(s, 8, beat * 4, 0, "C-4", kit, (0, 0));
            for k in 1..4 {
                put(
                    s,
                    8,
                    beat * 4 + k,
                    3,
                    if beat >= 6 { "F#2" } else { "E-2" },
                    zb,
                    (0, 0),
                );
            }
        }
        // 9 Growl
        let gr = gen(s, "Growl", 0.7);
        for (r, n) in [
            (0, "E-2"),
            (6, "E-2"),
            (8, "G-2"),
            (16, "E-2"),
            (22, "A#1"),
            (24, "B-1"),
        ] {
            put(s, 9, r, 3, n, gr, (0, 0));
            off(s, 9, r + 5, 3);
        }
        // 10 Dark Pluck arpeggio
        let pl = gen(s, "Dark Pluck", 0.9);
        let arp = ["E-3", "B-3", "G-4", "B-3", "F#4", "B-3", "G-4", "D-4"];
        for r in 0..32 {
            if r % 2 == 0 {
                put(s, 10, r, 4, arp[(r / 2) % 8], pl, (0, 0));
            }
        }
        // 11-13 noise: riser; downlifter then impact; wind + vinyl
        let ri = gen(s, "White Riser", 1.0);
        let dl = gen(s, "Downlifter", 0.75);
        let im = gen(s, "Impact", 0.85);
        let wi = gen(s, "Wind", 0.9);
        let vc = gen(s, "Vinyl Crackle", 0.7);
        put(s, 11, 0, 7, "C-4", ri, (0, 0));
        off(s, 11, 31, 7);
        put(s, 12, 0, 7, "C-4", dl, (0, 0));
        put(s, 12, 18, 6, "C-4", im, (0, 0));
        put(s, 13, 0, 7, "C-4", wi, (0, 0));
        put(s, 13, 0, 6, "C-4", vc, (0, 0));
        off(s, 13, 28, 7);
        off(s, 13, 28, 6);
        // 14-17 the groove: dark, hypnotic, 140 BPM in E / F#.
        // kick on the beat, the Psy Rolling Bass on the three 16ths after it,
        // open hats on the offbeat only, a clap on 2 and 4 from block 15.
        for b in 14..18 {
            for beat in 0..8 {
                let r = beat * 4;
                put(s, b, r, 0, "C-4", kit, (0, 0));
                put(s, b, r + 2, 1, "A#4", kit, (0, 0));
                if b >= 15 && beat % 2 == 1 {
                    put(s, b, r, 2, "D#4", kit, (0, 0));
                }
                let n = if (b == 15 || b == 17) && beat >= 6 {
                    "F#1"
                } else {
                    "E-1"
                };
                for k in 1..4 {
                    put(s, b, r + k, 3, n, pb, (0, 0));
                }
            }
        }
        // the bass filter opens a little each block (macro 1 = cutoff), 4300 resets
        fxonly(s, 14, 1, 3, (0x43, 0x00));
        fxonly(s, 16, 1, 3, (0x43, 0x13));
        fxonly(s, 17, 1, 3, (0x43, 0x00));
        // wind under it all, a dark pad from 15, a bell call, acid line in 16, riser into 17
        put(s, 14, 0, 6, "C-4", wi, (0, 0));
        off(s, 14, 31, 6);
        chord(s, 15, 0, 31, &["E-3", "B-3"], dp);
        put(s, 15, 14, 6, "B-4", mb, (0, 0));
        for r in 0..32 {
            if r % 4 == 2 || r % 8 == 7 {
                continue;
            }
            put(s, 16, r, 4, line[r % 8], ac, (0x43, 0x12 + (r / 6) as u8));
        }
        off(s, 16, 31, 4);
        put(s, 16, 0, 7, "C-4", ri, (0, 0));
        off(s, 16, 31, 7);
        put(s, 17, 0, 7, "C-4", im, (0, 0));
        chord(s, 17, 0, 31, &["E-3", "G-3"], dp);
        put(s, 17, 6, 6, "E-4", pl, (0, 0));
        put(s, 17, 14, 6, "G-4", pl, (0, 0));
        put(s, 17, 22, 6, "B-3", pl, (0, 0));
        put(s, 17, 30, 6, "F#4", pl, (0, 0));
        crate::fileio::save_cwt(s, std::path::Path::new(&out)).unwrap();
    }
}
