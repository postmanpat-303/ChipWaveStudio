//! The poly synth (Unreleased; shown as [`POLY_DISPLAY_NAME`]): a configurable
//! polyphonic subtractive synth, inspired by classic subtractive synths. A generator engine (`GenEngine::Poly`), so the
//! voice pools, insert FX, mixer, automation lanes (`instrument.N.gen.K`),
//! 43xy (knobs 1-8), MIDI learn, drag-to-modulate, presets and `.cwi` files
//! all work as for the other generators.
//!
//! Signal flow per voice (`voice.rs`):
//! Osc 1 + Osc 2 (each 1-16 unison voices: pulse ▸ saw ▸ octave saw shape,
//! sub pulse, hard sync, detune, width, pan) → Blend (MIX / NEG / WIPE / AM /
//! SIGN / MAX) → + noise (colour) → high-pass → drive with feedback → main
//! filter (LP / BP / HP 2- or 4-pole, band-reject; key tracking, filter
//! envelope, velocity, filter FM) → shaper (soft / hard / fold / asym) → amp
//! envelope × velocity → gain, pan. Four per-voice modulation routes move
//! any knob from the filter envelope, amp envelope, velocity or key.
//!
//! Original ChipWave code. The oscillators use 2-point polyBLEP step
//! correction (Välimäki & Huovilainen, "Antialiasing oscillators in
//! subtractive synthesis", IEEE SPM 2007), applied with one sample of delay
//! so both sides of every edge (wraps, pulse edges, hard-sync resets) are
//! corrected. Filters are Zavalishin TPT state-variable filters
//! (`crate::synth::Svf`). Nothing allocates while rendering.
use crate::gen::{MacroDef, Rng};
use serde::{Deserialize, Serialize};

/// The engine's display name, as a literal (for `concat!` in preset tips).
/// Rename it here only.
/// Files never store it (the engine's file tag is `Poly`).
#[macro_export]
macro_rules! poly_name {
    () => {
        "WaveForge"
    };
}
/// The engine's display name (see [`poly_name!`]).
pub const POLY_DISPLAY_NAME: &str = poly_name!();

pub mod dsp;
pub mod presets;
pub mod voice;
pub use voice::PolyState;

#[cfg(test)]
mod tests;

/// most unison voices per oscillator
pub const MAX_UNISON: usize = 16;
/// per-voice modulation routes
pub const MOD_SLOTS: usize = 8;
/// most notes per track (the note-clip pool, `engine::NOTE_VOICES`)
pub const MAX_POLY: u8 = 8;

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
pub const RES: usize = 1;
pub const ENV_AMT: usize = 2;
pub const BLEND: usize = 3;
pub const DRIVE: usize = 4;
pub const SHAPER: usize = 5;
pub const GLIDE: usize = 6;
pub const GAIN: usize = 7;
pub const KEYTRK: usize = 8;
pub const VEL_CUT: usize = 9;
pub const HP_CUT: usize = 10;
pub const HP_RES: usize = 11;
pub const FEEDBACK: usize = 12;
pub const NOISE: usize = 13;
pub const COLOUR: usize = 14;
pub const FM_AMT: usize = 15;
pub const FM_FREQ: usize = 16;
pub const SHAPER_MIX: usize = 17;
pub const FEG: usize = 18; // A D S R = 18..22
pub const AEG: usize = 22; // A D S R = 22..26
pub const VEL_AMP: usize = 26;
pub const PAN: usize = 27;
/// first knob of oscillator `o` (`OSC[o] + SHAPE` etc.)
pub const OSC: [usize; 2] = [28, 39];
pub const SHAPE: usize = 0;
pub const PW: usize = 1;
pub const PITCH: usize = 2;
pub const OCT: usize = 3;
pub const SUB: usize = 4;
pub const SUB_PW: usize = 5;
pub const SYNC: usize = 6;
pub const VOICES: usize = 7;
pub const DETUNE: usize = 8;
pub const WIDTH: usize = 9;
pub const OPAN: usize = 10;
/// knobs per oscillator
pub const OSC_N: usize = 11;
/// first modulation-route amount knob
pub const MOD_AMT: usize = 50;
/// number of knobs
pub const PN: usize = 58;

macro_rules! osc_knobs {
    ($n:literal) => {
        [
            m("Shape", -1.0, 1.0, false, "v", (0.2, 0.8),
                "pulse (left), saw (centre), octave-up saw (right)"),
            m("PW", 0.05, 0.95, false, "", (0.2, 0.8),
                "pulse width; also moves the octave-up saw's second ramp"),
            m("Pitch", -7.0, 7.0, false, "st.", (0.5, 0.5),
                "pitch, -7 to +7 semitones"),
            m("Octave", -2.0, 3.0, false, "ft", (0.2, 0.8),
                "octave in organ footage: 32' 16' 8' 4' 2' 1'"),
            m("Sub", 0.0, 1.0, false, "", (0.0, 0.5),
                "sub oscillator level: a pulse one octave down"),
            m("Sub PW", 0.05, 0.95, false, "", (0.3, 0.7),
                "sub oscillator pulse width"),
            m("Sync", 0.0, 60.0, false, "st.", (0.0, 0.3),
                "hard sync: the oscillator runs this far above its pitch and restarts every cycle (0 = off)"),
            m("Voices", 1.0, 16.0, false, "#", (0.0, 0.4),
                "unison voices per note"),
            m("Detune", 0.0, 100.0, false, "ct", (0.0, 0.4),
                "unison detune (the outer voices, in cents)"),
            m("Width", 0.0, 1.0, false, "", (0.3, 1.0),
                "stereo spread of the unison voices"),
            m("Pan", -1.0, 1.0, false, "v", (0.4, 0.6),
                "oscillator pan"),
        ]
    };
}

const fn cat(
    a: [MacroDef; 28],
    o1: [MacroDef; 11],
    o2: [MacroDef; 11],
    md: [MacroDef; 8],
) -> [MacroDef; PN] {
    let mut out = [a[0]; PN];
    let mut i = 0;
    while i < 28 {
        out[i] = a[i];
        i += 1;
    }
    let mut j = 0;
    while j < OSC_N {
        out[28 + j] = o1[j];
        out[39 + j] = o2[j];
        j += 1;
    }
    let mut k = 0;
    while k < MOD_SLOTS {
        out[MOD_AMT + k] = md[k];
        k += 1;
    }
    out
}

/// knob definitions (`gen::MacroDef`: range, unit, Dice window, tip)
pub const POLY: [MacroDef; PN] = cat(
    [
        m(
            "Cutoff",
            20.0,
            20000.0,
            true,
            "Hz",
            (0.35, 0.9),
            "main filter cutoff",
        ),
        m(
            "Res",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.6),
            "main filter resonance (self-oscillates at the top)",
        ),
        m(
            "Env amt",
            -6.0,
            6.0,
            false,
            "oct",
            (0.5, 0.8),
            "filter envelope to cutoff, in octaves",
        ),
        m(
            "Blend",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.7),
            "from oscillator 1 to oscillator 2 (AM: the depth)",
        ),
        m(
            "Drive",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.4),
            "drive into the filter (up to +24 dB, soft-clipped)",
        ),
        m(
            "Shaper",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.4),
            "waveshaper amount after the filter",
        ),
        m(
            "Glide",
            0.001,
            2.0,
            true,
            "s",
            (0.0, 0.3),
            "glide time from the previous note (Mono / Legato, and Poly)",
        ),
        m("Gain", 0.0, 2.0, false, "x", (0.4, 0.6), "output gain"),
        m(
            "Key trk",
            0.0,
            1.0,
            false,
            "",
            (0.0, 1.0),
            "cutoff follows the note (100 % = one octave per octave)",
        ),
        m(
            "Vel > cut",
            0.0,
            4.0,
            false,
            "oct",
            (0.0, 0.5),
            "velocity opens the filter (octaves at full velocity)",
        ),
        m(
            "HP cut",
            10.0,
            4000.0,
            true,
            "Hz",
            (0.0, 0.3),
            "high-pass before the drive",
        ),
        m(
            "HP res",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.4),
            "high-pass resonance",
        ),
        m(
            "Feedback",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.3),
            "filter output fed back into the drive",
        ),
        m(
            "Noise",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.15),
            "white noise level",
        ),
        m(
            "Colour",
            -1.0,
            1.0,
            false,
            "v",
            (0.3, 0.7),
            "noise colour: dark (left) to bright (right)",
        ),
        m(
            "Filter FM",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.2),
            "a sine at FM freq moves the cutoff (up to ±4 octaves)",
        ),
        m(
            "FM freq",
            20.0,
            8000.0,
            true,
            "Hz",
            (0.2, 0.8),
            "filter FM oscillator frequency",
        ),
        m(
            "Shp mix",
            0.0,
            1.0,
            false,
            "",
            (0.6, 1.0),
            "shaper: dry to wet",
        ),
        m(
            "F.Attack",
            0.001,
            10.0,
            true,
            "s",
            (0.0, 0.35),
            "filter envelope attack",
        ),
        m(
            "F.Decay",
            0.005,
            10.0,
            true,
            "s",
            (0.2, 0.6),
            "filter envelope decay",
        ),
        m(
            "F.Sustain",
            0.0,
            1.0,
            false,
            "",
            (0.0, 0.6),
            "filter envelope sustain",
        ),
        m(
            "F.Release",
            0.005,
            10.0,
            true,
            "s",
            (0.2, 0.5),
            "filter envelope release",
        ),
        m("A.Attack", 0.001, 10.0, true, "s", (0.0, 0.3), "amp attack"),
        m("A.Decay", 0.005, 10.0, true, "s", (0.2, 0.6), "amp decay"),
        m("A.Sustain", 0.0, 1.0, false, "", (0.4, 1.0), "amp sustain"),
        m(
            "A.Release",
            0.005,
            10.0,
            true,
            "s",
            (0.2, 0.5),
            "amp release",
        ),
        m(
            "Vel > amp",
            0.0,
            1.0,
            false,
            "",
            (0.2, 0.6),
            "how much velocity sets the loudness",
        ),
        m("Pan", -1.0, 1.0, false, "v", (0.5, 0.5), "voice pan"),
    ],
    osc_knobs!("1"),
    osc_knobs!("2"),
    [
        m("Mod 1", -1.0, 1.0, false, "v", (0.5, 0.5), "route 1 amount"),
        m("Mod 2", -1.0, 1.0, false, "v", (0.5, 0.5), "route 2 amount"),
        m("Mod 3", -1.0, 1.0, false, "v", (0.5, 0.5), "route 3 amount"),
        m("Mod 4", -1.0, 1.0, false, "v", (0.5, 0.5), "route 4 amount"),
        m("Mod 5", -1.0, 1.0, false, "v", (0.5, 0.5), "route 5 amount"),
        m("Mod 6", -1.0, 1.0, false, "v", (0.5, 0.5), "route 6 amount"),
        m("Mod 7", -1.0, 1.0, false, "v", (0.5, 0.5), "route 7 amount"),
        m("Mod 8", -1.0, 1.0, false, "v", (0.5, 0.5), "route 8 amount"),
    ],
);

/// stable file keys of the knobs (presets and songs store the 0..1 knob
/// values by name, bit-exact, so knobs can be added later without
/// breaking files)
pub const KEYS: [&str; PN] = [
    "cutoff",
    "res",
    "env_amt",
    "blend",
    "drive",
    "shaper",
    "glide",
    "gain",
    "key_track",
    "vel_cut",
    "hp_cut",
    "hp_res",
    "feedback",
    "noise",
    "colour",
    "fm_amt",
    "fm_freq",
    "shaper_mix",
    "f_attack",
    "f_decay",
    "f_sustain",
    "f_release",
    "a_attack",
    "a_decay",
    "a_sustain",
    "a_release",
    "vel_amp",
    "pan",
    "o1_shape",
    "o1_pw",
    "o1_pitch",
    "o1_oct",
    "o1_sub",
    "o1_sub_pw",
    "o1_sync",
    "o1_voices",
    "o1_detune",
    "o1_width",
    "o1_pan",
    "o2_shape",
    "o2_pw",
    "o2_pitch",
    "o2_oct",
    "o2_sub",
    "o2_sub_pw",
    "o2_sync",
    "o2_voices",
    "o2_detune",
    "o2_width",
    "o2_pan",
    "mod1",
    "mod2",
    "mod3",
    "mod4",
    "mod5",
    "mod6",
    "mod7",
    "mod8",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendOp {
    #[default]
    Mix,
    Neg,
    Wipe,
    Am,
    Sign,
    Max,
}
impl BlendOp {
    pub const ALL: [BlendOp; 6] = [
        BlendOp::Mix,
        BlendOp::Neg,
        BlendOp::Wipe,
        BlendOp::Am,
        BlendOp::Sign,
        BlendOp::Max,
    ];
    pub fn name(self) -> &'static str {
        ["MIX", "NEG", "WIPE", "AM", "SIGN", "MAX"][self as usize]
    }
    pub fn tip(self) -> &'static str {
        [
            "a linear mix of oscillators 1 and 2",
            "oscillator 1 mixed with an inverted oscillator 2 (can cancel)",
            "a mix with a curved ramp: each side stays strong for longer",
            "oscillator 2 modulates oscillator 1's amplitude (Blend = depth)",
            "oscillator 2 takes oscillator 1's polarity",
            "oscillator 1 mixed with the larger of the two",
        ][self as usize]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolyFilter {
    /// low-pass, 2-pole (12 dB/oct, gentle)
    Lp2,
    /// low-pass, 4-pole (24 dB/oct)
    #[default]
    Lp4,
    Bp2,
    Bp4,
    Hp2,
    Hp4,
    /// band-reject (notch)
    Br,
}
impl PolyFilter {
    pub const ALL: [PolyFilter; 7] = [
        PolyFilter::Lp2,
        PolyFilter::Lp4,
        PolyFilter::Bp2,
        PolyFilter::Bp4,
        PolyFilter::Hp2,
        PolyFilter::Hp4,
        PolyFilter::Br,
    ];
    pub fn name(self) -> &'static str {
        ["LP 12", "LP 24", "BP 12", "BP 24", "HP 12", "HP 24", "BR"][self as usize]
    }
    pub fn pole4(self) -> bool {
        matches!(self, PolyFilter::Lp4 | PolyFilter::Bp4 | PolyFilter::Hp4)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaperMode {
    #[default]
    Soft,
    Hard,
    Fold,
    Asym,
}
impl ShaperMode {
    pub const ALL: [ShaperMode; 4] = [
        ShaperMode::Soft,
        ShaperMode::Hard,
        ShaperMode::Fold,
        ShaperMode::Asym,
    ];
    pub fn name(self) -> &'static str {
        ["Soft", "Hard", "Fold", "Asym"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceMode {
    #[default]
    Poly,
    /// one voice per track; every note retriggers the envelopes
    Mono,
    /// one voice; a note while one is held glides without retriggering
    Legato,
}
impl VoiceMode {
    pub const ALL: [VoiceMode; 3] = [VoiceMode::Poly, VoiceMode::Mono, VoiceMode::Legato];
    pub fn name(self) -> &'static str {
        ["Poly", "Mono", "Legato"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModSrc {
    #[default]
    Off,
    /// filter envelope 0..1
    Feg,
    /// amp envelope 0..1
    Aeg,
    /// note velocity 0..1
    Vel,
    /// key: C-4 = 0, ±1 = ±5 octaves
    Key,
}
impl ModSrc {
    pub const ALL: [ModSrc; 5] = [
        ModSrc::Off,
        ModSrc::Feg,
        ModSrc::Aeg,
        ModSrc::Vel,
        ModSrc::Key,
    ];
    pub fn name(self) -> &'static str {
        ["Off", "Filter env", "Amp env", "Velocity", "Keytrack"][self as usize]
    }
}

/// one per-voice modulation route; its amount is knob `MOD_AMT + slot`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub src: ModSrc,
    /// destination knob index (< MOD_AMT)
    pub dst: u8,
}

/// a knob's full name ("Osc 1: Shape"): automation lanes and route targets
pub fn knob_label(k: usize) -> String {
    let n = POLY.get(k).map_or("?", |d| d.name);
    if (OSC[0]..OSC[0] + OSC_N).contains(&k) {
        format!("Osc 1: {n}")
    } else if (OSC[1]..OSC[1] + OSC_N).contains(&k) {
        format!("Osc 2: {n}")
    } else {
        n.to_string()
    }
}

/// knobs a route may target (everything but the route amounts)
pub fn route_targets() -> impl Iterator<Item = usize> {
    0..MOD_AMT
}

/// The poly synth patch. `p` holds the knobs 0..1 (see `POLY`); in files they
/// are written by name (`KEYS`).
#[derive(Clone, Debug, PartialEq)]
pub struct PolyP {
    pub p: [f32; PN],
    pub blend: BlendOp,
    pub filter: PolyFilter,
    pub shaper: ShaperMode,
    pub mode: VoiceMode,
    /// most notes at once per track, 1..=8 (Poly mode)
    pub polyphony: u8,
    /// restart each oscillator's phase on every note
    pub reset: [bool; 2],
    pub routes: [Route; MOD_SLOTS],
    /// bit i keeps knob i on Dice
    pub locks: u64,
}

impl Default for PolyP {
    /// the Init patch: one plain saw through an open filter
    fn default() -> Self {
        let mut p = PolyP {
            p: [0.0; PN],
            blend: BlendOp::Mix,
            filter: PolyFilter::Lp4,
            shaper: ShaperMode::Soft,
            mode: VoiceMode::Poly,
            polyphony: MAX_POLY,
            reset: [true, true],
            routes: [Route::default(); MOD_SLOTS],
            locks: 0,
        };
        p.set(&[
            (CUTOFF, 18000.0),
            (RES, 0.0),
            (ENV_AMT, 0.0),
            (BLEND, 0.0),
            (DRIVE, 0.0),
            (SHAPER, 0.0),
            (GLIDE, 0.001),
            (GAIN, 1.0),
            (KEYTRK, 0.0),
            (VEL_CUT, 0.0),
            (HP_CUT, 10.0),
            (HP_RES, 0.0),
            (FEEDBACK, 0.0),
            (NOISE, 0.0),
            (COLOUR, 0.0),
            (FM_AMT, 0.0),
            (FM_FREQ, 440.0),
            (SHAPER_MIX, 1.0),
            (FEG, 0.001),
            (FEG + 1, 0.4),
            (FEG + 2, 0.0),
            (FEG + 3, 0.3),
            (AEG, 0.002),
            (AEG + 1, 0.3),
            (AEG + 2, 1.0),
            (AEG + 3, 0.2),
            (VEL_AMP, 0.5),
            (PAN, 0.0),
        ]);
        for (o, b) in OSC.into_iter().enumerate() {
            p.set_osc(o, 0.0, 0.5, 0.0, 0, 0.0, 1, 0.0, 0.5);
            p.p[b + SUB_PW] = 0.5;
            p.p[b + OPAN] = 0.5;
        }
        for k in 0..MOD_SLOTS {
            p.p[MOD_AMT + k] = 0.5;
        }
        p
    }
}

impl PolyP {
    /// set knobs from real values
    pub fn set(&mut self, v: &[(usize, f32)]) -> &mut Self {
        for &(k, x) in v {
            if k < PN {
                self.p[k] = POLY[k].unmap(x);
            }
        }
        self
    }
    /// set the common oscillator knobs from real values
    #[allow(clippy::too_many_arguments)]
    pub fn set_osc(
        &mut self,
        o: usize,
        shape: f32,
        pw: f32,
        pitch: f32,
        oct: i32,
        sub: f32,
        voices: u32,
        detune: f32,
        width: f32,
    ) -> &mut Self {
        let b = OSC[o];
        self.set(&[
            (b + SHAPE, shape),
            (b + PW, pw),
            (b + PITCH, pitch),
            (b + OCT, oct as f32),
            (b + SUB, sub),
            (b + VOICES, voices as f32),
            (b + DETUNE, detune),
            (b + WIDTH, width),
        ])
    }
    /// the real value of knob `k`
    pub fn get(&self, k: usize) -> f32 {
        POLY[k].map(self.p[k])
    }
    /// route `slot`: source, destination knob, amount -1..1
    pub fn route(&mut self, slot: usize, src: ModSrc, dst: usize, amt: f32) -> &mut Self {
        if slot < MOD_SLOTS && dst < MOD_AMT {
            self.routes[slot] = Route {
                src,
                dst: dst as u8,
            };
            self.p[MOD_AMT + slot] = POLY[MOD_AMT + slot].unmap(amt);
        }
        self
    }
    pub fn fix(&mut self) {
        for v in self.p.iter_mut() {
            *v = if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.5
            };
        }
        self.polyphony = self.polyphony.clamp(1, MAX_POLY);
        for r in &mut self.routes {
            if r.dst as usize >= MOD_AMT {
                *r = Route::default();
            }
        }
    }
    pub fn dice(&mut self, seed: u32) {
        let r = &mut Rng::new(seed);
        for (i, d) in POLY.iter().enumerate().take(MOD_AMT) {
            let x = r.f();
            if self.locks & (1 << i) == 0 {
                self.p[i] = d.dice.0 + (d.dice.1 - d.dice.0) * x;
            }
        }
        // keep it audible: some amp sustain or decay, gain near unity
        if self.locks & (1 << GAIN) == 0 {
            self.set(&[(GAIN, 1.0)]);
        }
        let b = r.next() as usize;
        self.blend = BlendOp::ALL[b % 6];
    }
    pub fn mutate(&mut self, a: f32, seed: u32) {
        let mut r = Rng::new(seed);
        for (i, v) in self.p.iter_mut().enumerate().take(MOD_AMT) {
            let x = r.f() * 2.0 - 1.0;
            if self.locks & (1 << i) == 0 {
                *v = (*v + x * a).clamp(0.0, 1.0);
            }
        }
    }
    /// voices sounding per note (unison of both oscillators; oscillator 2
    /// counts only when the blend lets it through)
    pub fn osc_voices(&self) -> usize {
        let n = |o: usize| self.get(OSC[o] + VOICES).round().clamp(1.0, 16.0) as usize;
        n(0) + if self.p[BLEND] > 0.0 { n(1) } else { 0 }
    }
}

// ---- files: knobs by name (0..1, exact), unknown keys ignored ----

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct PolyFile {
    knobs: std::collections::BTreeMap<String, f32>,
    blend: BlendOp,
    filter: PolyFilter,
    shaper: ShaperMode,
    mode: VoiceMode,
    polyphony: u8,
    reset: [bool; 2],
    routes: Vec<Route>,
    #[serde(skip_serializing_if = "is_zero")]
    locks: u64,
}

fn is_zero(x: &u64) -> bool {
    *x == 0
}

impl Serialize for PolyP {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let f = PolyFile {
            knobs: KEYS
                .iter()
                .enumerate()
                .map(|(k, n)| (n.to_string(), self.p[k]))
                .collect(),
            blend: self.blend,
            filter: self.filter,
            shaper: self.shaper,
            mode: self.mode,
            polyphony: self.polyphony,
            reset: self.reset,
            routes: self.routes.to_vec(),
            locks: self.locks,
        };
        f.serialize(s)
    }
}

impl<'de> Deserialize<'de> for PolyP {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let f = PolyFile::deserialize(d)?;
        let mut p = PolyP::default();
        for (k, name) in KEYS.iter().enumerate() {
            if let Some(v) = f.knobs.get(*name) {
                if v.is_finite() {
                    p.p[k] = *v;
                }
            }
        }
        p.blend = f.blend;
        p.filter = f.filter;
        p.shaper = f.shaper;
        p.mode = f.mode;
        p.polyphony = if f.polyphony == 0 {
            MAX_POLY
        } else {
            f.polyphony
        };
        p.reset = f.reset;
        for (i, r) in f.routes.into_iter().take(MOD_SLOTS).enumerate() {
            p.routes[i] = r;
        }
        p.locks = f.locks;
        p.fix();
        Ok(p)
    }
}
