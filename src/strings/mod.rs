//! StringWave (shown as [`STRINGS_DISPLAY_NAME`]): a lush string-ensemble
//! synth built into ChipWave as a generator engine (`GenEngine::Strings`).
//! Three layers (ensemble saws, plucked and bowed waveguide strings, a body
//! resonator) plus undertones (sub octave and drone) → a 12/24 dB low-pass
//! with its own envelope → amp envelope, two LFOs and a mod envelope with
//! twelve destinations, then string chorus, stereo echo, an 8-line FDN
//! reverb, master gain and a peak limiter.
//!
//! Unlike the other generators it is one polyphonic instance per
//! (instrument, track) (`rack.rs`), like a plugin: its effects and limiter
//! work on the whole chord. Every knob is live (re-read every block of at
//! most 64 samples, smoothed), automatable (`instrument.N.gen.K`), MIDI
//! learnable, and knobs 1-8 are the 43xy macros.
//!
//! StringWave by Del (postmanpat), MIT. Nothing allocates while rendering.
use crate::gen::MacroDef;
use serde::{Deserialize, Serialize};

/// The engine's display name, as a literal (for `concat!` in preset names).
/// Rename it here only. Files never store it (the file tag is `Strings`).
#[macro_export]
macro_rules! strings_name {
    () => {
        "StringWave"
    };
}
/// The engine's display name (see [`strings_name!`]).
pub const STRINGS_DISPLAY_NAME: &str = strings_name!();

pub mod dsp;
pub mod engine;
pub mod fx;
pub mod presets;
pub mod rack;

#[cfg(test)]
mod tests;

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

// knob indices (0-7 are also the 43xy macros)
pub const CUTOFF: usize = 0;
pub const RESO: usize = 1;
pub const F_ENV: usize = 2;
pub const AMP_A: usize = 3;
pub const AMP_R: usize = 4;
pub const REV_MIX: usize = 5;
pub const CHO_MIX: usize = 6;
pub const MASTER: usize = 7;
/// first knob of each layer: octave, detune, tone, level, pan
pub const LAYER: [usize; 3] = [8, 13, 18];
pub const L_OCT: usize = 0;
pub const L_DET: usize = 1;
pub const L_TONE: usize = 2;
pub const L_LVL: usize = 3;
pub const L_PAN: usize = 4;
pub const SUB_LVL: usize = 23;
pub const DRONE_LVL: usize = 24;
pub const DRONE_TONE: usize = 25;
pub const F_KEY: usize = 26;
pub const F_A: usize = 27;
pub const F_D: usize = 28;
pub const F_S: usize = 29;
pub const F_R: usize = 30;
pub const AMP_D: usize = 31;
pub const AMP_S: usize = 32;
pub const VEL: usize = 33;
/// LFO n: rate at `LFO[n]`, depth at `LFO[n] + 1`
pub const LFO: [usize; 2] = [34, 36];
pub const ME_A: usize = 38;
pub const ME_D: usize = 39;
pub const ME_AMT: usize = 40;
pub const WHEEL: usize = 41;
pub const CHO_RATE: usize = 42;
pub const CHO_DEPTH: usize = 43;
pub const ECHO_TIME: usize = 44;
pub const ECHO_FB: usize = 45;
pub const ECHO_TONE: usize = 46;
pub const ECHO_MIX: usize = 47;
pub const REV_SIZE: usize = 48;
pub const REV_DAMP: usize = 49;
pub const VOICES: usize = 50;
/// knob count
pub const SN: usize = 51;

const LAYER_DEFS: [MacroDef; 5] = [
    m(
        "Octave",
        -2.0,
        2.0,
        false,
        "#",
        (0.25, 0.75),
        "the layer's octave, -2..+2",
    ),
    m(
        "Detune",
        0.0,
        1.0,
        false,
        "",
        (0.1, 0.7),
        "spread of the layer's voices: wider is lusher",
    ),
    m(
        "Tone",
        0.0,
        1.0,
        false,
        "",
        (0.3, 0.8),
        "the layer's brightness",
    ),
    m(
        "Level",
        0.0,
        1.0,
        false,
        "",
        (0.2, 0.8),
        "the layer's volume",
    ),
    m(
        "Pan",
        -1.0,
        1.0,
        false,
        "v",
        (0.2, 0.8),
        "the layer's place in the stereo field",
    ),
];

/// a layer knob named after its layer ("Layer 2 Level"), so automation
/// lanes, MIDI learn and search tell the three layers apart
const fn lay(name: &'static str, k: usize) -> MacroDef {
    MacroDef {
        name,
        ..LAYER_DEFS[k]
    }
}

/// The knobs (real ranges; `p` holds them 0..1).
pub static STRINGS: [MacroDef; SN] = [
    m(
        "Cutoff",
        30.0,
        18000.0,
        true,
        "Hz",
        (0.45, 0.9),
        "low-pass cutoff: lower is darker",
    ),
    m(
        "Resonance",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.4),
        "low-pass resonance",
    ),
    m(
        "Filter Env",
        -1.0,
        1.0,
        false,
        "v",
        (0.4, 0.8),
        "how far the filter envelope opens (or closes) the cutoff",
    ),
    m(
        "Attack",
        0.001,
        10.0,
        true,
        "s",
        (0.3, 0.75),
        "amp attack: slow for a swell, short for a bite",
    ),
    m(
        "Release",
        0.005,
        15.0,
        true,
        "s",
        (0.5, 0.85),
        "amp release: how long notes ring after key-up",
    ),
    m("Reverb", 0.0, 1.0, false, "", (0.1, 0.6), "reverb mix"),
    m(
        "Chorus",
        0.0,
        1.0,
        false,
        "",
        (0.2, 0.8),
        "ensemble chorus mix",
    ),
    m(
        "Master",
        -48.0,
        6.0,
        false,
        "dB",
        (0.85, 0.85),
        "output level (the limiter keeps peaks near -4 dBFS)",
    ),
    lay("Layer 1 Octave", 0),
    lay("Layer 1 Detune", 1),
    lay("Layer 1 Tone", 2),
    lay("Layer 1 Level", 3),
    lay("Layer 1 Pan", 4),
    lay("Layer 2 Octave", 0),
    lay("Layer 2 Detune", 1),
    lay("Layer 2 Tone", 2),
    lay("Layer 2 Level", 3),
    lay("Layer 2 Pan", 4),
    lay("Layer 3 Octave", 0),
    lay("Layer 3 Detune", 1),
    lay("Layer 3 Tone", 2),
    lay("Layer 3 Level", 3),
    lay("Layer 3 Pan", 4),
    m(
        "Sub",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.4),
        "sub-octave sine under every note",
    ),
    m(
        "Drone",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.3),
        "a low drone under every note",
    ),
    m(
        "Drone Tone",
        0.0,
        1.0,
        false,
        "",
        (0.2, 0.6),
        "the drone's brightness",
    ),
    m(
        "Key Track",
        0.0,
        1.0,
        false,
        "",
        (0.3, 0.7),
        "higher notes open the filter",
    ),
    m(
        "F Attack",
        0.001,
        10.0,
        true,
        "s",
        (0.2, 0.7),
        "filter envelope attack",
    ),
    m(
        "F Decay",
        0.005,
        10.0,
        true,
        "s",
        (0.3, 0.7),
        "filter envelope decay",
    ),
    m(
        "F Sustain",
        0.0,
        1.0,
        false,
        "",
        (0.3, 0.8),
        "filter envelope sustain",
    ),
    m(
        "F Release",
        0.005,
        15.0,
        true,
        "s",
        (0.4, 0.8),
        "filter envelope release",
    ),
    m("Decay", 0.005, 10.0, true, "s", (0.4, 0.8), "amp decay"),
    m("Sustain", 0.0, 1.0, false, "", (0.5, 1.0), "amp sustain"),
    m(
        "Velocity",
        0.0,
        1.0,
        false,
        "",
        (0.2, 0.8),
        "how much velocity changes the level",
    ),
    m(
        "LFO 1 Rate",
        0.02,
        20.0,
        true,
        "Hz",
        (0.3, 0.8),
        "LFO 1 speed",
    ),
    m(
        "LFO 1 Depth",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.3),
        "LFO 1 amount",
    ),
    m(
        "LFO 2 Rate",
        0.02,
        20.0,
        true,
        "Hz",
        (0.2, 0.6),
        "LFO 2 speed",
    ),
    m(
        "LFO 2 Depth",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.4),
        "LFO 2 amount",
    ),
    m(
        "Mod Attack",
        0.001,
        10.0,
        true,
        "s",
        (0.1, 0.6),
        "mod envelope attack",
    ),
    m(
        "Mod Decay",
        0.005,
        10.0,
        true,
        "s",
        (0.3, 0.7),
        "mod envelope decay",
    ),
    m(
        "Mod Amount",
        -1.0,
        1.0,
        false,
        "v",
        (0.4, 0.6),
        "mod envelope amount (to its destination)",
    ),
    m(
        "Wheel Vibrato",
        0.0,
        1.0,
        false,
        "",
        (0.3, 0.7),
        "vibrato the mod wheel adds",
    ),
    m(
        "Chorus Rate",
        0.05,
        6.0,
        true,
        "Hz",
        (0.3, 0.7),
        "chorus speed",
    ),
    m(
        "Chorus Depth",
        0.0,
        1.0,
        false,
        "",
        (0.3, 0.8),
        "chorus depth",
    ),
    m(
        "Echo Time",
        0.02,
        1.5,
        true,
        "s",
        (0.5, 0.9),
        "echo delay time",
    ),
    m(
        "Echo Feedback",
        0.0,
        0.95,
        false,
        "",
        (0.1, 0.5),
        "echo repeats",
    ),
    m(
        "Echo Tone",
        0.0,
        1.0,
        false,
        "",
        (0.3, 0.7),
        "echo brightness",
    ),
    m("Echo Mix", 0.0, 1.0, false, "", (0.0, 0.3), "echo mix"),
    m(
        "Reverb Size",
        0.0,
        1.0,
        false,
        "",
        (0.5, 0.95),
        "reverb length",
    ),
    m(
        "Reverb Damp",
        0.0,
        1.0,
        false,
        "",
        (0.2, 0.7),
        "reverb darkness",
    ),
    m(
        "Voices",
        4.0,
        16.0,
        false,
        "#",
        (1.0, 1.0),
        "most notes at once (fewer saves CPU)",
    ),
];

/// Stable file keys, one per knob (never rename).
pub const KEYS: [&str; SN] = [
    "cutoff",
    "reso",
    "f_env",
    "aa",
    "ar",
    "rev_mix",
    "cho_mix",
    "master",
    "l1_oct",
    "l1_det",
    "l1_tone",
    "l1_lvl",
    "l1_pan",
    "l2_oct",
    "l2_det",
    "l2_tone",
    "l2_lvl",
    "l2_pan",
    "l3_oct",
    "l3_det",
    "l3_tone",
    "l3_lvl",
    "l3_pan",
    "sub_lvl",
    "drone_lvl",
    "drone_tone",
    "f_key",
    "fa",
    "fd",
    "fs",
    "fr",
    "ad",
    "as",
    "vel",
    "lfo1_rate",
    "lfo1_depth",
    "lfo2_rate",
    "lfo2_depth",
    "me_a",
    "me_d",
    "me_amt",
    "wheel",
    "cho_rate",
    "cho_depth",
    "echo_time",
    "echo_fb",
    "echo_tone",
    "echo_mix",
    "rev_size",
    "rev_damp",
    "voices",
];

/// a knob's full name ("Layer 2: Tone"): automation lanes, learn
pub fn knob_label(k: usize) -> String {
    let n = STRINGS.get(k).map_or("?", |d| d.name);
    match LAYER.iter().position(|&b| (b..b + 5).contains(&k)) {
        Some(l) => format!("Layer {}: {n}", l + 1),
        None => n.to_string(),
    }
}

/// A layer's sound source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wave {
    #[default]
    Ensemble,
    Plucked,
    Bowed,
    Body,
}
impl Wave {
    pub const ALL: [Wave; 4] = [Wave::Ensemble, Wave::Plucked, Wave::Bowed, Wave::Body];
    pub fn name(self) -> &'static str {
        match self {
            Wave::Ensemble => "Ensemble",
            Wave::Plucked => "Plucked",
            Wave::Bowed => "Bowed",
            Wave::Body => "Body",
        }
    }
}

/// The undertones' sub octave.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubOct {
    #[default]
    Down1,
    Down2,
}
impl SubOct {
    pub const ALL: [SubOct; 2] = [SubOct::Down1, SubOct::Down2];
    pub fn name(self) -> &'static str {
        match self {
            SubOct::Down1 => "-1 oct",
            SubOct::Down2 => "-2 oct",
        }
    }
}

/// An LFO's shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LfoWave {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
    SampleHold,
}
impl LfoWave {
    pub const ALL: [LfoWave; 5] = [
        LfoWave::Sine,
        LfoWave::Triangle,
        LfoWave::Saw,
        LfoWave::Square,
        LfoWave::SampleHold,
    ];
    pub fn name(self) -> &'static str {
        match self {
            LfoWave::Sine => "Sine",
            LfoWave::Triangle => "Triangle",
            LfoWave::Saw => "Saw",
            LfoWave::Square => "Square",
            LfoWave::SampleHold => "S&H",
        }
    }
}

/// Where an LFO or the mod envelope goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dest {
    Pitch,
    #[default]
    Cutoff,
    LayerMix,
    Amp,
    Chorus,
    Echo,
    Reverb,
    Resonance,
    Tone,
    Detune,
    Pan,
    Undertones,
}
impl Dest {
    pub const ALL: [Dest; 12] = [
        Dest::Pitch,
        Dest::Cutoff,
        Dest::LayerMix,
        Dest::Amp,
        Dest::Chorus,
        Dest::Echo,
        Dest::Reverb,
        Dest::Resonance,
        Dest::Tone,
        Dest::Detune,
        Dest::Pan,
        Dest::Undertones,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Dest::Pitch => "Pitch",
            Dest::Cutoff => "Cutoff",
            Dest::LayerMix => "Layer Mix",
            Dest::Amp => "Amp",
            Dest::Chorus => "Chorus",
            Dest::Echo => "Echo",
            Dest::Reverb => "Reverb",
            Dest::Resonance => "Resonance",
            Dest::Tone => "Tone",
            Dest::Detune => "Detune",
            Dest::Pan => "Pan",
            Dest::Undertones => "Undertones",
        }
    }
}

/// The StringWave patch. `p` holds the knobs 0..1 (see `STRINGS`); in
/// files they are written by name (`KEYS`).
#[derive(Clone, Debug, PartialEq)]
pub struct StringsP {
    pub p: [f32; SN],
    pub layer_on: [bool; 3],
    pub layer_mute: [bool; 3],
    pub layer_solo: [bool; 3],
    pub wave: [Wave; 3],
    pub und_on: bool,
    pub und_mute: bool,
    pub und_solo: bool,
    pub sub_oct: SubOct,
    pub filter_on: bool,
    /// 24 dB/oct instead of 12
    pub f24: bool,
    pub lfo_on: [bool; 2],
    pub lfo_wave: [LfoWave; 2],
    pub lfo_dest: [Dest; 2],
    pub me_on: bool,
    pub me_dest: Dest,
    pub cho_on: bool,
    pub echo_on: bool,
    pub rev_on: bool,
    /// bit i keeps knob i on Dice
    pub locks: u64,
}

impl Default for StringsP {
    /// the "Velvet Ensemble" patch
    fn default() -> Self {
        let mut p = StringsP {
            p: [0.0; SN],
            layer_on: [true; 3],
            layer_mute: [false; 3],
            layer_solo: [false; 3],
            wave: [Wave::Ensemble, Wave::Bowed, Wave::Ensemble],
            und_on: true,
            und_mute: false,
            und_solo: false,
            sub_oct: SubOct::Down1,
            filter_on: true,
            f24: false,
            lfo_on: [true; 2],
            lfo_wave: [LfoWave::Sine, LfoWave::Triangle],
            lfo_dest: [Dest::Pitch, Dest::Cutoff],
            me_on: true,
            me_dest: Dest::Cutoff,
            cho_on: true,
            echo_on: true,
            rev_on: true,
            locks: 0,
        };
        p.layer(0, 0, 0.45, 0.6, 0.7, -0.35);
        p.layer(1, 0, 0.3, 0.55, 0.55, 0.0);
        p.layer(2, 1, 0.35, 0.5, 0.28, 0.35);
        p.set(&[
            (CUTOFF, 3200.0),
            (RESO, 0.15),
            (F_ENV, 0.25),
            (AMP_A, 0.35),
            (AMP_R, 1.6),
            (REV_MIX, 0.3),
            (CHO_MIX, 0.5),
            (MASTER, -3.0),
            (SUB_LVL, 0.22),
            (DRONE_LVL, 0.15),
            (DRONE_TONE, 0.35),
            (F_KEY, 0.5),
            (F_A, 0.8),
            (F_D, 1.5),
            (F_S, 0.6),
            (F_R, 1.5),
            (AMP_D, 1.0),
            (AMP_S, 0.85),
            (VEL, 0.5),
            (LFO[0], 5.2),
            (LFO[0] + 1, 0.06),
            (LFO[1], 0.15),
            (LFO[1] + 1, 0.25),
            (ME_A, 0.01),
            (ME_D, 0.6),
            (ME_AMT, 0.0),
            (WHEEL, 0.5),
            (CHO_RATE, 0.5),
            (CHO_DEPTH, 0.6),
            (ECHO_TIME, 0.38),
            (ECHO_FB, 0.3),
            (ECHO_TONE, 0.5),
            (ECHO_MIX, 0.15),
            (REV_SIZE, 0.75),
            (REV_DAMP, 0.45),
            (VOICES, 16.0),
        ]);
        p
    }
}

impl StringsP {
    /// set knobs from real values
    pub fn set(&mut self, v: &[(usize, f32)]) -> &mut Self {
        for &(k, x) in v {
            if k < SN {
                self.p[k] = STRINGS[k].unmap(x);
            }
        }
        self
    }
    /// set layer `l`'s knobs from real values
    pub fn layer(&mut self, l: usize, oct: i32, det: f32, tone: f32, lvl: f32, pan: f32) {
        let b = LAYER[l];
        self.set(&[
            (b + L_OCT, oct as f32),
            (b + L_DET, det),
            (b + L_TONE, tone),
            (b + L_LVL, lvl),
            (b + L_PAN, pan),
        ]);
    }
    /// the real value of knob `k`
    pub fn get(&self, k: usize) -> f32 {
        STRINGS[k].map(self.p[k])
    }
    pub fn fix(&mut self) {
        for v in self.p.iter_mut() {
            *v = if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.5
            };
        }
    }
    pub fn dice(&mut self, seed: u32) {
        let r = &mut crate::gen::Rng::new(seed);
        for (i, d) in STRINGS.iter().enumerate() {
            let x = r.f();
            if self.locks & (1 << i) == 0 {
                self.p[i] = d.dice.0 + (d.dice.1 - d.dice.0) * x;
            }
        }
        let b = r.next() as usize;
        if self.locks & (1 << LAYER[0]) == 0 {
            self.wave[0] = Wave::ALL[b % 4];
        }
        if self.locks & (1 << LAYER[1]) == 0 {
            self.wave[1] = Wave::ALL[(b / 4) % 4];
        }
    }
    pub fn mutate(&mut self, a: f32, seed: u32) {
        let mut r = crate::gen::Rng::new(seed);
        for (i, v) in self.p.iter_mut().enumerate() {
            let x = r.f() * 2.0 - 1.0;
            if self.locks & (1 << i) == 0 && i != MASTER && i != VOICES {
                *v = (*v + x * a).clamp(0.0, 1.0);
            }
        }
    }
}

impl engine::Snap {
    /// The engine's values from patch `p`; `ov(k)` overrides knob `k`
    /// (0..1: automation, 43xy, modulation). Audibility: power on, not
    /// muted, and soloed when anything is soloed.
    pub fn new(p: &StringsP, ov: &dyn Fn(usize) -> Option<f32>) -> Self {
        let k = |i: usize| STRINGS[i].map(ov(i).unwrap_or(p.p[i]));
        let any_solo = p.layer_solo.iter().any(|&x| x) || p.und_solo;
        let audible = |on: bool, mute: bool, solo: bool| on && !mute && (!any_solo || solo);
        let layer = |l: usize| {
            let b = LAYER[l];
            engine::LayerSnap {
                on: audible(p.layer_on[l], p.layer_mute[l], p.layer_solo[l]),
                wave: p.wave[l],
                oct: k(b + L_OCT).round() as i32,
                det: k(b + L_DET),
                tone: k(b + L_TONE),
                lvl: k(b + L_LVL),
                pan: k(b + L_PAN),
            }
        };
        let lfo = |n: usize| {
            let depth = if p.lfo_on[n] { k(LFO[n] + 1) } else { 0.0 };
            (p.lfo_wave[n], k(LFO[n]), depth, p.lfo_dest[n])
        };
        let f_on = p.filter_on;
        engine::Snap {
            layers: [layer(0), layer(1), layer(2)],
            und: audible(p.und_on, p.und_mute, p.und_solo),
            sub_oct: p.sub_oct,
            sub_lvl: k(SUB_LVL),
            drone_lvl: k(DRONE_LVL),
            drone_tone: k(DRONE_TONE),
            // filter power off = wide open, flat
            cutoff: if f_on { k(CUTOFF) } else { 20000.0 },
            reso: if f_on { k(RESO) } else { 0.0 },
            f_env: if f_on { k(F_ENV) } else { 0.0 },
            f_key: if f_on { k(F_KEY) } else { 0.0 },
            f_24: p.f24,
            f_adsr: [k(F_A), k(F_D), k(F_S), k(F_R)],
            a_adsr: [k(AMP_A), k(AMP_D), k(AMP_S), k(AMP_R)],
            vel: k(VEL),
            lfo: [lfo(0), lfo(1)],
            me: (
                k(ME_A),
                k(ME_D),
                if p.me_on { k(ME_AMT) } else { 0.0 },
                p.me_dest,
            ),
            wheel_vib: k(WHEEL),
            // the engine's bend is in semitones (the app's MIDI bend range)
            bend_range: 1.0,
            cho: (p.cho_on, k(CHO_RATE), k(CHO_DEPTH), k(CHO_MIX)),
            echo: (
                p.echo_on,
                k(ECHO_TIME),
                k(ECHO_FB),
                k(ECHO_TONE),
                k(ECHO_MIX),
            ),
            rev: (p.rev_on, k(REV_SIZE), k(REV_DAMP), k(REV_MIX)),
            master: db_to_gain(k(MASTER)),
            voices: k(VOICES).round().clamp(1.0, engine::NUM_VOICES as f32) as usize,
        }
    }
}

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// The default MIDI CC map (a learned CC wins): (CC, knob). CC 1 (mod
/// wheel) and 11 (expression) play live instead (`LiveEv::Cc`).
pub const DEFAULT_CC: [(u8, usize); 22] = [
    (7, MASTER),
    (74, CUTOFF),
    (71, RESO),
    (73, AMP_A),
    (72, AMP_R),
    (75, F_ENV),
    (76, LFO[0]),
    (77, LFO[0] + 1),
    (78, LFO[1]),
    (79, LFO[1] + 1),
    (91, REV_MIX),
    (93, CHO_MIX),
    (94, ECHO_MIX),
    (95, ECHO_FB),
    (20, LAYER[0] + L_LVL),
    (21, LAYER[1] + L_LVL),
    (22, LAYER[2] + L_LVL),
    (23, SUB_LVL),
    (24, DRONE_LVL),
    (25, LAYER[0] + L_TONE),
    (26, LAYER[1] + L_TONE),
    (27, LAYER[2] + L_TONE),
];

/// the knob CC `cc` moves by default (no learned mapping)
pub fn default_cc(cc: u8) -> Option<usize> {
    DEFAULT_CC.iter().find(|x| x.0 == cc).map(|x| x.1)
}

// ---- files: knobs by name (0..1, exact), unknown keys ignored ----

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct StringsFile {
    knobs: std::collections::BTreeMap<String, f32>,
    layer_on: [bool; 3],
    layer_mute: [bool; 3],
    layer_solo: [bool; 3],
    wave: [Wave; 3],
    und_on: bool,
    und_mute: bool,
    und_solo: bool,
    sub_oct: SubOct,
    filter_on: bool,
    f24: bool,
    lfo_on: [bool; 2],
    lfo_wave: [LfoWave; 2],
    lfo_dest: [Dest; 2],
    me_on: bool,
    me_dest: Dest,
    cho_on: bool,
    echo_on: bool,
    rev_on: bool,
    #[serde(skip_serializing_if = "is_zero")]
    locks: u64,
}

impl Default for StringsFile {
    fn default() -> Self {
        let p = StringsP::default();
        StringsFile {
            knobs: Default::default(),
            layer_on: p.layer_on,
            layer_mute: p.layer_mute,
            layer_solo: p.layer_solo,
            wave: p.wave,
            und_on: p.und_on,
            und_mute: p.und_mute,
            und_solo: p.und_solo,
            sub_oct: p.sub_oct,
            filter_on: p.filter_on,
            f24: p.f24,
            lfo_on: p.lfo_on,
            lfo_wave: p.lfo_wave,
            lfo_dest: p.lfo_dest,
            me_on: p.me_on,
            me_dest: p.me_dest,
            cho_on: p.cho_on,
            echo_on: p.echo_on,
            rev_on: p.rev_on,
            locks: 0,
        }
    }
}

fn is_zero(x: &u64) -> bool {
    *x == 0
}

impl Serialize for StringsP {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let f = StringsFile {
            knobs: KEYS
                .iter()
                .enumerate()
                .map(|(k, n)| (n.to_string(), self.p[k]))
                .collect(),
            layer_on: self.layer_on,
            layer_mute: self.layer_mute,
            layer_solo: self.layer_solo,
            wave: self.wave,
            und_on: self.und_on,
            und_mute: self.und_mute,
            und_solo: self.und_solo,
            sub_oct: self.sub_oct,
            filter_on: self.filter_on,
            f24: self.f24,
            lfo_on: self.lfo_on,
            lfo_wave: self.lfo_wave,
            lfo_dest: self.lfo_dest,
            me_on: self.me_on,
            me_dest: self.me_dest,
            cho_on: self.cho_on,
            echo_on: self.echo_on,
            rev_on: self.rev_on,
            locks: self.locks,
        };
        f.serialize(s)
    }
}

impl<'de> Deserialize<'de> for StringsP {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let f = StringsFile::deserialize(d)?;
        let mut p = StringsP::default();
        for (k, name) in KEYS.iter().enumerate() {
            if let Some(v) = f.knobs.get(*name) {
                if v.is_finite() {
                    p.p[k] = *v;
                }
            }
        }
        p.layer_on = f.layer_on;
        p.layer_mute = f.layer_mute;
        p.layer_solo = f.layer_solo;
        p.wave = f.wave;
        p.und_on = f.und_on;
        p.und_mute = f.und_mute;
        p.und_solo = f.und_solo;
        p.sub_oct = f.sub_oct;
        p.filter_on = f.filter_on;
        p.f24 = f.f24;
        p.lfo_on = f.lfo_on;
        p.lfo_wave = f.lfo_wave;
        p.lfo_dest = f.lfo_dest;
        p.me_on = f.me_on;
        p.me_dest = f.me_dest;
        p.cho_on = f.cho_on;
        p.echo_on = f.echo_on;
        p.rev_on = f.rev_on;
        p.locks = f.locks;
        p.fix();
        Ok(p)
    }
}
