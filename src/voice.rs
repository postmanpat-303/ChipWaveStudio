//! Voice (glitch) instrument: synthetic "ahh / eee / ooh / err" vowel chops.
//! No recorded voice and no samples: a glottal pulse (plus breath noise and
//! jitter) drives a bank of 5 parallel formant band-passes built from the
//! synth's TPT state-variable filter. The vowel morphs smoothly through a
//! per-instrument sequence (tempo-synced steps, random "babble" jumps,
//! LFO), and pattern effects 30-37 change vowel / shift / stutter per row.
//!
//! Credits (see docs/research/voice-glitch.md and THIRD_PARTY_LICENSES):
//! - Design inspired by the "naive speech synth" of Mutable Instruments
//!   Plaits by Emilie Gillet (MIT): a pulse source into zero-delay SVF
//!   formants with a vowel x register morph table.
//!   https://github.com/pichenettes/eurorack/tree/master/plaits/dsp/speech
//!   No Plaits code or table data is copied; this is an independent
//!   implementation, the MIT notice is reproduced in THIRD_PARTY_LICENSES.
//! - Formant data: Csound Manual, Appendix "Formant values" (bass, tenor,
//!   alto, soprano: a e i o u; https://csound.com/docs/manual/MiscFormants.html)
//!   and G. E. Peterson & H. L. Barney (1952), "Control methods used in a
//!   study of the vowels", JASA 24(2) (ER and AE).
//! - Glottal pulse: A. E. Rosenberg (1971), "Effect of glottal pulse shape on
//!   the quality of natural vowels", JASA 49(2); the "tension" control follows
//!   the open-quotient idea of the LF model (Fant, Liljencrants & Lin 1985).
//! - Parallel formant structure: D. H. Klatt (1980), "Software for a
//!   cascade/parallel formant synthesizer", JASA 67(3).
//! - Formant filters: the TPT state-variable filter after Vadim Zavalishin,
//!   "The Art of VA Filter Design" (crate::synth::Svf).
use serde::{Deserialize, Serialize};

use crate::synth::{Adsr, FilterMode, Lfo, Patch, Svf, Wave};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vowel {
    A,
    E,
    I,
    O,
    U,
    Er,
    Ae,
}

impl Vowel {
    pub const ALL: [Vowel; 7] = [
        Vowel::A,
        Vowel::E,
        Vowel::I,
        Vowel::O,
        Vowel::U,
        Vowel::Er,
        Vowel::Ae,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Vowel::A => "A",
            Vowel::E => "E",
            Vowel::I => "I",
            Vowel::O => "O",
            Vowel::U => "U",
            Vowel::Er => "ER",
            Vowel::Ae => "AE",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Vowel::A => "\"ahh\" (father)",
            Vowel::E => "\"eh\" (bed)",
            Vowel::I => "\"eee\" (see)",
            Vowel::O => "\"ohh\" (law)",
            Vowel::U => "\"ooh\" (boot)",
            Vowel::Er => "\"err\" (bird)",
            Vowel::Ae => "\"aah\" (cat)",
        }
    }
    /// index for effect 30x_ (0 = A .. 6 = AE)
    pub fn from_index(i: u8) -> Option<Vowel> {
        Self::ALL.get(i as usize).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Register {
    Bass,
    Tenor,
    Alto,
    Soprano,
    /// bass formants scaled down (bigger, darker "throat")
    Demon,
}

impl Register {
    pub const ALL: [Register; 5] = [
        Register::Bass,
        Register::Tenor,
        Register::Alto,
        Register::Soprano,
        Register::Demon,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceSource {
    /// Rosenberg glottal pulse (most voice-like)
    Glottal,
    /// band-limited saw (brighter, synthier)
    Saw,
    /// pulse at the patch pulse width (chip / robot)
    Pulse,
    /// noise only (whisper)
    Whisper,
}

impl VoiceSource {
    pub const ALL: [VoiceSource; 4] = [
        VoiceSource::Glottal,
        VoiceSource::Saw,
        VoiceSource::Pulse,
        VoiceSource::Whisper,
    ];
}

/// One vowel: formant centre (Hz), level (dB re F1) and bandwidth (Hz).
#[derive(Clone, Copy, Debug)]
pub struct Formants {
    pub f: [f32; 5],
    pub db: [f32; 5],
    pub bw: [f32; 5],
}

const fn fm(f: [f32; 5], db: [f32; 5], bw: [f32; 5]) -> Formants {
    Formants { f, db, bw }
}

/// [register][vowel A E I O U ER AE]. A..U from the Csound formant table,
/// ER / AE F1-F3 from the Peterson & Barney averages (men for bass, women
/// for soprano; tenor and alto are those values scaled about 3 % up / down),
/// their F4/F5 from the same register's E; levels and bandwidths for ER / AE
/// are ChipWave's own estimates.
const TABLE: [[Formants; 7]; 4] = [
    // bass
    [
        fm(
            [600., 1040., 2250., 2450., 2750.],
            [0., -7., -9., -9., -20.],
            [60., 70., 110., 120., 130.],
        ),
        fm(
            [400., 1620., 2400., 2800., 3100.],
            [0., -12., -9., -12., -18.],
            [40., 80., 100., 120., 120.],
        ),
        fm(
            [250., 1750., 2600., 3050., 3340.],
            [0., -30., -16., -22., -28.],
            [60., 90., 100., 120., 120.],
        ),
        fm(
            [400., 750., 2400., 2600., 2900.],
            [0., -11., -21., -20., -40.],
            [40., 80., 100., 120., 120.],
        ),
        fm(
            [350., 600., 2400., 2675., 2950.],
            [0., -20., -32., -28., -36.],
            [40., 80., 100., 120., 120.],
        ),
        fm(
            [490., 1350., 1690., 2800., 3100.],
            [0., -8., -12., -18., -26.],
            [60., 80., 100., 120., 120.],
        ),
        fm(
            [660., 1720., 2410., 2800., 3100.],
            [0., -6., -14., -16., -24.],
            [70., 80., 110., 120., 120.],
        ),
    ],
    // tenor
    [
        fm(
            [650., 1080., 2650., 2900., 3250.],
            [0., -6., -7., -8., -22.],
            [80., 90., 120., 130., 140.],
        ),
        fm(
            [400., 1700., 2600., 3200., 3580.],
            [0., -14., -12., -14., -20.],
            [70., 80., 100., 120., 120.],
        ),
        fm(
            [290., 1870., 2800., 3250., 3540.],
            [0., -15., -18., -20., -30.],
            [40., 90., 100., 120., 120.],
        ),
        fm(
            [400., 800., 2600., 2800., 3000.],
            [0., -10., -12., -12., -26.],
            [70., 80., 100., 130., 135.],
        ),
        fm(
            [350., 600., 2700., 2900., 3300.],
            [0., -20., -17., -14., -26.],
            [40., 60., 100., 120., 120.],
        ),
        fm(
            [500., 1400., 1750., 3200., 3580.],
            [0., -8., -12., -18., -26.],
            [60., 80., 100., 120., 120.],
        ),
        fm(
            [690., 1780., 2500., 3200., 3580.],
            [0., -6., -14., -16., -24.],
            [80., 90., 110., 120., 120.],
        ),
    ],
    // alto
    [
        fm(
            [800., 1150., 2800., 3500., 4950.],
            [0., -4., -20., -36., -60.],
            [80., 90., 120., 130., 140.],
        ),
        fm(
            [400., 1600., 2700., 3300., 4950.],
            [0., -24., -30., -35., -60.],
            [60., 80., 120., 150., 200.],
        ),
        fm(
            [350., 1700., 2700., 3700., 4950.],
            [0., -20., -30., -36., -60.],
            [50., 100., 120., 150., 200.],
        ),
        fm(
            [450., 800., 2830., 3500., 4950.],
            [0., -9., -16., -28., -55.],
            [70., 80., 100., 130., 135.],
        ),
        fm(
            [325., 700., 2530., 3500., 4950.],
            [0., -12., -30., -40., -64.],
            [50., 60., 170., 180., 200.],
        ),
        fm(
            [485., 1590., 1900., 3300., 4950.],
            [0., -12., -16., -35., -60.],
            [60., 90., 110., 150., 200.],
        ),
        fm(
            [835., 1990., 2765., 3500., 4950.],
            [0., -8., -20., -34., -60.],
            [80., 100., 120., 150., 200.],
        ),
    ],
    // soprano
    [
        fm(
            [800., 1150., 2900., 3900., 4950.],
            [0., -6., -32., -20., -50.],
            [80., 90., 120., 130., 140.],
        ),
        fm(
            [350., 2000., 2800., 3600., 4950.],
            [0., -20., -15., -40., -56.],
            [60., 100., 120., 150., 200.],
        ),
        fm(
            [270., 2140., 2950., 3900., 4950.],
            [0., -12., -26., -26., -44.],
            [60., 90., 100., 120., 120.],
        ),
        fm(
            [450., 800., 2830., 3800., 4950.],
            [0., -11., -22., -22., -50.],
            [40., 80., 100., 120., 120.],
        ),
        fm(
            [325., 700., 2700., 3800., 4950.],
            [0., -16., -35., -40., -60.],
            [50., 60., 170., 180., 200.],
        ),
        fm(
            [500., 1640., 1960., 3600., 4950.],
            [0., -14., -18., -38., -56.],
            [60., 90., 110., 150., 200.],
        ),
        fm(
            [860., 2050., 2850., 3900., 4950.],
            [0., -8., -24., -24., -50.],
            [80., 100., 120., 150., 200.],
        ),
    ],
];

/// formant scale of the Demon register (applied to the bass table)
pub const DEMON_SCALE: f32 = 0.78;

/// Table formants of `v` in register `r` (Demon = scaled bass).
pub fn formants(r: Register, v: Vowel) -> Formants {
    let vi = v as usize;
    match r {
        Register::Bass => TABLE[0][vi],
        Register::Tenor => TABLE[1][vi],
        Register::Alto => TABLE[2][vi],
        Register::Soprano => TABLE[3][vi],
        Register::Demon => {
            let mut t = TABLE[0][vi];
            for k in 0..5 {
                t.f[k] *= DEMON_SCALE;
                t.bw[k] *= 0.9;
            }
            t
        }
    }
}

/// beats for the 1-digit length codes of effects 33 / 36 / 37
pub const LEN_BEATS: [f32; 7] = [0.0, 1.0 / 16.0, 0.125, 0.25, 0.5, 1.0, 2.0];

pub fn len_code_beats(code: u8) -> f32 {
    LEN_BEATS[(code as usize).min(LEN_BEATS.len() - 1)]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoicePatch {
    /// envelopes, post filter, drive, pitch env, LFO and gain (the
    /// oscillator fields are not used by the voice)
    pub base: Patch,
    pub register: Register,
    pub source: VoiceSource,
    /// vowel sequence (1..=8), stepped every `step` beats
    pub vowels: Vec<Vowel>,
    /// glide time between vowels in ms
    pub morph_ms: f32,
    /// beats per vowel step, 0 = stay on the first vowel
    pub step: f32,
    /// chance 0..1 that a step jumps to a random vowel instead
    pub babble: f32,
    /// formant shift ratio 0.5..2 (independent of pitch)
    pub shift: f32,
    /// formant sharpness (bandwidth divisor) 0.5..3
    pub sharp: f32,
    /// breath / aspiration noise 0..1
    pub breath: f32,
    /// 0 = soft, breathy pulse .. 1 = pressed, buzzy
    pub tension: f32,
    /// random pitch / level wobble per glottal period 0..1
    pub jitter: f32,
    /// patch LFO -> vowel position (in sequence steps)
    pub lfo_vowel: f32,
    /// patch LFO -> formant shift (semitones)
    pub lfo_shift: f32,
    /// stutter on every note: captured length in beats (0 = off)
    pub stutter_len: f32,
    /// stutter repeats after the captured slice
    pub stutter_repeats: u8,
    /// pitch step per repeat in semitones (tape-style)
    pub stutter_pitch: f32,
    /// chop gate cycle in beats (0 = off)
    pub chop_rate: f32,
    /// chop gate open fraction 0.05..1
    pub chop_duty: f32,
}

impl Default for VoicePatch {
    fn default() -> Self {
        Self {
            base: Patch {
                name: "Voice".into(),
                filter: FilterMode::Lp,
                cutoff: 9000.0,
                resonance: 0.1,
                amp: Adsr {
                    a: 0.01,
                    d: 0.3,
                    s: 0.8,
                    r: 0.12,
                },
                gain: 0.8,
                ..Patch::default()
            },
            register: Register::Bass,
            source: VoiceSource::Glottal,
            vowels: vec![Vowel::A],
            morph_ms: 60.0,
            step: 0.0,
            babble: 0.0,
            shift: 1.0,
            sharp: 1.0,
            breath: 0.15,
            tension: 0.5,
            jitter: 0.2,
            lfo_vowel: 0.0,
            lfo_shift: 0.0,
            stutter_len: 0.0,
            stutter_repeats: 0,
            stutter_pitch: 0.0,
            chop_rate: 0.0,
            chop_duty: 0.5,
        }
    }
}

impl VoicePatch {
    /// clamp loaded / edited values into range
    pub fn fix(&mut self) {
        if self.vowels.is_empty() {
            self.vowels.push(Vowel::A);
        }
        self.vowels.truncate(8);
        self.morph_ms = self.morph_ms.clamp(0.0, 2000.0);
        self.step = self.step.clamp(0.0, 16.0);
        self.babble = self.babble.clamp(0.0, 1.0);
        self.shift = self.shift.clamp(0.5, 2.0);
        self.sharp = self.sharp.clamp(0.5, 3.0);
        self.breath = self.breath.clamp(0.0, 1.0);
        self.tension = self.tension.clamp(0.0, 1.0);
        self.jitter = self.jitter.clamp(0.0, 1.0);
        self.lfo_vowel = self.lfo_vowel.clamp(-8.0, 8.0);
        self.lfo_shift = self.lfo_shift.clamp(-24.0, 24.0);
        self.stutter_len = self.stutter_len.clamp(0.0, 2.0);
        self.stutter_repeats = self.stutter_repeats.min(16);
        self.stutter_pitch = self.stutter_pitch.clamp(-12.0, 12.0);
        self.chop_rate = self.chop_rate.clamp(0.0, 4.0);
        self.chop_duty = self.chop_duty.clamp(0.05, 1.0);
    }

    /// any modulation that needs the patch LFO to run
    pub fn lfo_used(&self) -> bool {
        self.lfo_vowel != 0.0 || self.lfo_shift != 0.0
    }
}

/// Per-note overrides set by pattern effects 30-37 (reset on each note).
#[derive(Clone, Copy, Debug, Default)]
pub struct VoxFx {
    /// 30xy: forced vowel and its glide time in ms
    pub vowel: Option<(Vowel, f32)>,
    /// 32xx: formant shift ratio
    pub shift: Option<f32>,
    /// 35xx: breath 0..1
    pub breath: Option<f32>,
    /// 36xy: babble (step beats, amount)
    pub babble: Option<(f32, f32)>,
    /// 37xy: chop gate (cycle beats, duty; 0 cycle = off)
    pub chop: Option<(f32, f32)>,
}

/// Voice oscillator + formant bank state (fixed size, allocation-free).
#[derive(Clone, Copy)]
pub struct VoxState {
    ph: f32,
    /// jitter / shimmer of the current glottal period
    per_pitch: f32,
    per_amp: f32,
    form: [Svf; 5],
    g: [f32; 5],
    k: [f32; 5],
    /// smoothed formants: f, amplitude (linear), bandwidth
    cur_f: [f32; 5],
    cur_a: [f32; 5],
    cur_bw: [f32; 5],
    init: bool,
    coef_cnt: u8,
    /// sequence step and time inside it (beats)
    idx: usize,
    step_t: f32,
    /// beats since the note started (chop gate)
    beats: f32,
    chop_g: f32,
    /// random vowel of a babble jump (until the next step)
    babble_v: Option<Vowel>,
    rng: u32,
    pub fx: VoxFx,
}

impl Default for VoxState {
    fn default() -> Self {
        Self::new()
    }
}

fn rnd(n: &mut u32) -> f32 {
    *n ^= *n << 13;
    *n ^= *n >> 17;
    *n ^= *n << 5;
    *n as f32 / u32::MAX as f32
}

fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// output level trim so a sustained vowel sits near a synth saw's level
const OUT_GAIN: f32 = 2.2;

impl VoxState {
    pub fn new() -> Self {
        Self {
            ph: 0.0,
            per_pitch: 1.0,
            per_amp: 1.0,
            form: [Svf::default(); 5],
            g: [0.0; 5],
            k: [1.0; 5],
            cur_f: [500.0; 5],
            cur_a: [0.0; 5],
            cur_bw: [100.0; 5],
            init: true,
            coef_cnt: 0,
            idx: 0,
            step_t: 0.0,
            beats: 0.0,
            chop_g: 1.0,
            babble_v: None,
            rng: 0x6A09_E667,
            fx: VoxFx::default(),
        }
    }

    pub fn seed(&mut self, s: u32) {
        self.rng = s | 1;
    }

    /// 31xx: jump to sequence step `i` (and leave a forced 30 vowel)
    pub fn set_step(&mut self, i: usize) {
        self.fx.vowel = None;
        self.babble_v = None;
        self.idx = i;
        self.step_t = 0.0;
    }

    /// current sequence step (tests)
    #[cfg(test)]
    pub fn step_index(&self) -> usize {
        self.idx
    }

    /// Target formants for the current position (vowel sequence + LFO).
    fn target(&self, p: &VoicePatch, lfo: f32) -> Formants {
        if let Some((v, _)) = self.fx.vowel {
            return formants(p.register, v);
        }
        if let Some(v) = self.babble_v {
            return formants(p.register, v);
        }
        let n = p.vowels.len().max(1);
        let pos = self.idx as f32 + lfo * p.lfo_vowel;
        let pos = pos.rem_euclid(n as f32);
        let i0 = pos.floor() as usize % n;
        let i1 = (i0 + 1) % n;
        let t = pos - pos.floor();
        let v0 = p.vowels.get(i0).copied().unwrap_or(Vowel::A);
        let v1 = p.vowels.get(i1).copied().unwrap_or(Vowel::A);
        let (a, b) = (formants(p.register, v0), formants(p.register, v1));
        if t <= 0.0 || v0 == v1 {
            return a;
        }
        let mut o = a;
        for k in 0..5 {
            o.f[k] = a.f[k] + (b.f[k] - a.f[k]) * t;
            o.db[k] = a.db[k] + (b.db[k] - a.db[k]) * t;
            o.bw[k] = a.bw[k] + (b.bw[k] - a.bw[k]) * t;
        }
        o
    }

    /// One mono sample of the voice at fundamental `f` Hz. `lfo` is the
    /// patch LFO value (-1..1). Allocation-free.
    pub fn render(&mut self, p: &VoicePatch, f: f32, sr: f32, bpm: f32, lfo: f32) -> f32 {
        let dt_s = 1.0 / sr;
        let beat_dt = bpm.max(1.0) / 60.0 * dt_s;
        // --- vowel sequence / babble
        let (step, babble) = match self.fx.babble {
            Some((s, a)) => (s, a),
            None => (p.step, p.babble),
        };
        if step > 0.0 && self.fx.vowel.is_none() {
            self.step_t += beat_dt;
            if self.step_t >= step {
                self.step_t -= step;
                self.babble_v = None;
                let n = p.vowels.len().max(1);
                self.idx = (self.idx + 1) % n;
                if babble > 0.0 && rnd(&mut self.rng) < babble {
                    if n < 3 {
                        // short sequence: jump to any of the 7 vowels
                        self.babble_v = Some(Vowel::ALL[(rnd(&mut self.rng) * 6.999) as usize]);
                    } else {
                        self.idx = (rnd(&mut self.rng) * (n as f32 - 0.001)) as usize;
                    }
                }
            }
        }
        // --- smoothed formant targets
        let tg = self.target(p, lfo);
        let shift = self.fx.shift.unwrap_or(p.shift) * 2f32.powf(lfo * p.lfo_shift / 12.0);
        let morph_ms = match (self.fx.vowel, self.babble_v) {
            (Some((_, ms)), _) => ms,
            (None, Some(_)) => p.morph_ms.min(20.0),
            _ => p.morph_ms,
        };
        let a = if self.init {
            1.0
        } else {
            1.0 - (-dt_s / (morph_ms.max(0.3) * 0.001)).exp()
        };
        self.init = false;
        let nyq = sr * 0.45;
        for k in 0..5 {
            let ft = (tg.f[k] * shift).min(nyq);
            let at = 10f32.powf(tg.db[k] / 20.0);
            let bt = tg.bw[k] * shift.sqrt() / p.sharp.max(0.1);
            self.cur_f[k] += (ft - self.cur_f[k]) * a;
            self.cur_a[k] += (at - self.cur_a[k]) * a;
            self.cur_bw[k] += (bt - self.cur_bw[k]) * a;
        }
        // filter coefficients every 4 samples (tan is the costly part)
        if self.coef_cnt == 0 {
            for k in 0..5 {
                let fc = self.cur_f[k].clamp(40.0, nyq);
                self.g[k] = (std::f32::consts::PI * fc / sr).tan();
                self.k[k] = (self.cur_bw[k].max(10.0) / fc).clamp(0.01, 2.0);
            }
        }
        self.coef_cnt = (self.coef_cnt + 1) % 4;
        // --- excitation
        let f = f * self.per_pitch;
        let dt = (f / sr).clamp(0.0, 0.5);
        let breath = self.fx.breath.unwrap_or(p.breath);
        let noise = rnd(&mut self.rng) * 2.0 - 1.0;
        let oq = 0.8 - 0.45 * p.tension.clamp(0.0, 1.0);
        let tp = oq * 0.7;
        let tn = oq * 0.3;
        let ph = self.ph;
        let (mut e, flow) = match p.source {
            VoiceSource::Glottal => {
                // Rosenberg flow derivative, normalised so closure = -1
                let norm = 2.0 * tn / std::f32::consts::PI;
                let (d, g) = if ph < tp {
                    let x = std::f32::consts::PI * ph / tp;
                    (
                        0.5 * std::f32::consts::PI / tp * x.sin(),
                        0.5 * (1.0 - x.cos()),
                    )
                } else if ph < tp + tn {
                    let x = std::f32::consts::FRAC_PI_2 * (ph - tp) / tn;
                    (-std::f32::consts::FRAC_PI_2 / tn * x.sin(), x.cos())
                } else {
                    (0.0, 0.0)
                };
                // the derivative steps from -1 back to 0 at closure: BLEP it
                let tc = (ph - (tp + tn)).rem_euclid(1.0);
                (d * norm + 0.5 * poly_blep(tc, dt), g)
            }
            VoiceSource::Saw => ((2.0 * ph - 1.0 - poly_blep(ph, dt)) * 0.6, 0.5),
            VoiceSource::Pulse => (
                crate::synth::osc(Wave::Pulse, ph, dt, p.base.pulse_width) * 0.5,
                0.5,
            ),
            VoiceSource::Whisper => (0.0, 0.5),
        };
        e *= self.per_amp;
        let nb = if p.source == VoiceSource::Whisper {
            0.6 + breath * 0.6
        } else {
            breath * 0.8 * (0.25 + 0.75 * flow)
        };
        e += noise * nb;
        self.ph += dt;
        if self.ph >= 1.0 {
            self.ph -= 1.0;
            let j = p.jitter.clamp(0.0, 1.0);
            self.per_pitch = 1.0 + j * 0.025 * (rnd(&mut self.rng) * 2.0 - 1.0);
            self.per_amp = 1.0 + j * 0.15 * (rnd(&mut self.rng) * 2.0 - 1.0);
        }
        // --- parallel formant bank (constant-peak band-passes)
        let mut y = 0.0;
        for k in 0..5 {
            let (_, bp, _) = self.form[k].process_gk(e, self.g[k], self.k[k]);
            y += bp * self.k[k] * self.cur_a[k];
        }
        // --- chop gate
        self.beats += beat_dt;
        let (rate, duty) = match self.fx.chop {
            Some(c) => c,
            None => (p.chop_rate, p.chop_duty),
        };
        let gate = if rate > 0.0 {
            if (self.beats / rate).fract() < duty.clamp(0.05, 1.0) {
                1.0
            } else {
                0.0
            }
        } else {
            1.0
        };
        // ~1.5 ms declick
        self.chop_g += (gate - self.chop_g) * (1.0 - (-dt_s / 0.0015).exp());
        y * OUT_GAIN * self.chop_g
    }
}

/// Tempo-synced beat-repeat for one channel (effect 33, or the Voice
/// patch's stutter on every note): records `len` frames as they play,
/// then repeats them `repeats` times, each repeat `step` semitones up or
/// down (tape-style rate change). The buffer is allocated once up front;
/// `process` never allocates.
#[derive(Clone)]
pub struct Stutter {
    buf: Vec<[f32; 2]>,
    len: usize,
    w: usize,
    /// 0 = pass-through, 1 = recording, 2 = repeating
    mode: u8,
    read: f32,
    rate: f32,
    step: f32,
    left: u8,
    /// pass-through gain (fades back in after the repeats)
    live: f32,
    fade: f32,
}

impl Stutter {
    /// `max_frames` = longest slice (e.g. 1 s), `fade` = edge ramp in frames
    pub fn new(max_frames: usize, fade: usize) -> Self {
        Self {
            buf: vec![[0.0; 2]; max_frames.max(16)],
            len: 0,
            w: 0,
            mode: 0,
            read: 0.0,
            rate: 1.0,
            step: 1.0,
            left: 0,
            live: 1.0,
            fade: fade.max(1) as f32,
        }
    }

    #[cfg(test)]
    pub fn active(&self) -> bool {
        self.mode != 0
    }

    /// slice length in frames (clamped to the buffer)
    pub fn start(&mut self, frames: usize, repeats: u8, semis: f32) {
        if frames < 8 || repeats == 0 {
            self.stop();
            return;
        }
        self.len = frames.min(self.buf.len());
        self.w = 0;
        self.mode = 1;
        self.left = repeats;
        self.set_pitch(semis);
        self.rate = 1.0;
    }

    pub fn set_pitch(&mut self, semis: f32) {
        self.step = 2f32.powf(semis.clamp(-24.0, 24.0) / 12.0);
    }

    /// back to pass-through (with a short fade-in)
    pub fn stop(&mut self) {
        if self.mode != 0 {
            self.live = 0.0;
        }
        self.mode = 0;
    }

    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        match self.mode {
            1 => {
                self.buf[self.w] = [l, r];
                self.w += 1;
                // fade the live slice out just before the first repeat
                let g = ((self.len - self.w) as f32 / self.fade).min(1.0);
                if self.w >= self.len {
                    self.mode = 2;
                    self.read = 0.0;
                    self.rate = self.step;
                }
                (l * g, r * g)
            }
            2 => {
                let n = self.len as f32;
                let i = self.read as usize;
                let t = self.read - i as f32;
                let a = self.buf[i.min(self.len - 1)];
                let b = self.buf[(i + 1).min(self.len - 1)];
                let win = (self.read / self.fade)
                    .min((n - self.read) / self.fade)
                    .clamp(0.0, 1.0);
                let o = (
                    (a[0] + (b[0] - a[0]) * t) * win,
                    (a[1] + (b[1] - a[1]) * t) * win,
                );
                self.read += self.rate;
                if self.read >= n {
                    self.left = self.left.saturating_sub(1);
                    if self.left == 0 {
                        self.mode = 0;
                        self.live = 0.0;
                    } else {
                        self.read = 0.0;
                        self.rate = (self.rate * self.step).clamp(0.125, 8.0);
                    }
                }
                o
            }
            _ => {
                if self.live < 1.0 {
                    self.live = (self.live + 1.0 / self.fade).min(1.0);
                }
                (l * self.live, r * self.live)
            }
        }
    }
}

/// Built-in Voice presets (name, tooltip, patch).
pub fn presets() -> Vec<(&'static str, &'static str, VoicePatch)> {
    use Vowel::*;
    let d = VoicePatch::default;
    let base = |name: &str, cutoff: f32, a: f32, dcy: f32, s: f32, r: f32| Patch {
        name: name.into(),
        cutoff,
        amp: Adsr { a, d: dcy, s, r },
        ..d().base
    };
    vec![
        (
            "Demon Ahh",
            "huge dark throat: Demon register, slow A > O > U morph, breathy",
            VoicePatch {
                base: Patch {
                    lfo: Lfo {
                        shape: Wave::Triangle,
                        sync: 4.0,
                        ..Lfo::default()
                    },
                    drive: 0.2,
                    ..base("Demon Ahh", 5000.0, 0.03, 0.6, 0.85, 0.4)
                },
                register: Register::Demon,
                vowels: vec![A, O, U],
                morph_ms: 220.0,
                lfo_vowel: 1.0,
                breath: 0.35,
                tension: 0.35,
                jitter: 0.35,
                ..d()
            },
        ),
        (
            "Choir Eee",
            "airy soprano E > I > E pad voice; add reverb and chorus",
            VoicePatch {
                base: Patch {
                    lfo: Lfo {
                        shape: Wave::Sine,
                        rate: 5.0,
                        pitch: 0.15,
                        ..Lfo::default()
                    },
                    ..base("Choir Eee", 10000.0, 0.25, 0.8, 0.9, 1.2)
                },
                register: Register::Soprano,
                vowels: vec![E, I, E, Ae],
                step: 2.0,
                morph_ms: 400.0,
                breath: 0.3,
                tension: 0.3,
                jitter: 0.25,
                ..d()
            },
        ),
        (
            "Glitch Babble",
            "random vowel jumps every 1/16 with a 1/32 chop: alien chatter",
            VoicePatch {
                base: base("Glitch Babble", 8000.0, 0.003, 0.3, 0.7, 0.08),
                register: Register::Tenor,
                vowels: vec![A, E, I, O, U, Er],
                step: 0.25,
                babble: 0.8,
                morph_ms: 12.0,
                chop_rate: 0.125,
                chop_duty: 0.6,
                tension: 0.6,
                breath: 0.1,
                ..d()
            },
        ),
        (
            "Robot Err",
            "pulse source, narrow formants, pressed: chip-speech ER > A",
            VoicePatch {
                base: Patch {
                    pulse_width: 0.3,
                    ..base("Robot Err", 7000.0, 0.002, 0.2, 0.8, 0.06)
                },
                register: Register::Bass,
                source: VoiceSource::Pulse,
                vowels: vec![Er, Er, A, Er],
                step: 0.5,
                morph_ms: 25.0,
                sharp: 2.2,
                breath: 0.0,
                tension: 0.9,
                jitter: 0.0,
                ..d()
            },
        ),
        (
            "Stutter Ohh",
            "every note repeats its first 1/32 beat-slice 3 times, pitching down",
            VoicePatch {
                base: base("Stutter Ohh", 6000.0, 0.002, 0.25, 0.6, 0.1),
                register: Register::Tenor,
                vowels: vec![O, A],
                step: 0.5,
                morph_ms: 40.0,
                stutter_len: 0.125,
                stutter_repeats: 3,
                stutter_pitch: -2.0,
                tension: 0.55,
                ..d()
            },
        ),
        (
            "Whisper Ooh",
            "noise-only vowel U > O with an LFO formant shift: ghost voice",
            VoicePatch {
                base: Patch {
                    lfo: Lfo {
                        shape: Wave::Sine,
                        sync: 2.0,
                        ..Lfo::default()
                    },
                    ..base("Whisper Ooh", 9000.0, 0.08, 0.5, 0.8, 0.5)
                },
                register: Register::Alto,
                source: VoiceSource::Whisper,
                vowels: vec![U, O],
                step: 1.0,
                morph_ms: 300.0,
                lfo_shift: 3.0,
                breath: 0.6,
                ..d()
            },
        ),
    ]
}

pub fn preset(name: &str) -> Option<VoicePatch> {
    presets()
        .into_iter()
        .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, _, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;

    fn steady(p: &VoicePatch, f: f32, secs: f32) -> Vec<f32> {
        let mut v = VoxState::new();
        (0..(secs * SR) as usize)
            .map(|_| v.render(p, f, SR, 120.0, 0.0))
            .collect()
    }

    /// amplitude of harmonic `h` of `f0` (Goertzel-style correlation)
    fn harm(o: &[f32], f0: f32, h: usize) -> f64 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let w = std::f64::consts::TAU * f0 as f64 * h as f64 / SR as f64;
        for (i, x) in o.iter().enumerate() {
            re += *x as f64 * (w * i as f64).cos();
            im += *x as f64 * (w * i as f64).sin();
        }
        (re * re + im * im).sqrt() / o.len() as f64
    }

    #[test]
    fn all_presets_and_sources_are_finite_and_bounded() {
        for (name, _, p) in presets() {
            for src in VoiceSource::ALL {
                let p = VoicePatch {
                    source: src,
                    ..p.clone()
                };
                let o = steady(&p, 110.0, 1.0);
                let peak = o.iter().fold(0.0f32, |m, x| m.max(x.abs()));
                assert!(o.iter().all(|x| x.is_finite()), "{name} {src:?}");
                assert!(peak < 4.0, "{name} {src:?} peak {peak}");
                let rms = (o.iter().map(|x| x * x).sum::<f32>() / o.len() as f32).sqrt();
                assert!(rms > 0.01, "{name} {src:?} silent ({rms})");
            }
        }
    }

    #[test]
    fn extreme_settings_stay_finite() {
        let p = VoicePatch {
            shift: 2.0,
            sharp: 3.0,
            register: Register::Soprano,
            tension: 1.0,
            jitter: 1.0,
            breath: 1.0,
            ..VoicePatch::default()
        };
        for f in [20.0, 55.0, 880.0, 4000.0, 12000.0] {
            let o = steady(&p, f, 0.3);
            assert!(o.iter().all(|x| x.is_finite() && x.abs() < 8.0), "{f} Hz");
        }
    }

    /// The spectral envelope of a sustained vowel peaks at the table's
    /// formants: the strongest harmonic near each formant lies within one
    /// harmonic spacing of it, and it beats the valley between them.
    #[test]
    fn vowel_formants_land_near_the_table() {
        let f0 = 50.0; // exact 50 Hz harmonics, 1 s = 50 whole periods
        for reg in [Register::Bass, Register::Tenor, Register::Soprano] {
            for v in [Vowel::A, Vowel::I, Vowel::U, Vowel::Er] {
                let p = VoicePatch {
                    register: reg,
                    vowels: vec![v],
                    jitter: 0.0,
                    breath: 0.0,
                    ..VoicePatch::default()
                };
                let o = steady(&p, f0, 1.5);
                let o = &o[24000..]; // skip the start
                let t = formants(reg, v);
                let env: Vec<f64> = (1..=80).map(|h| harm(o, f0, h)).collect();
                // every formant that is within 20 dB of F1 (the table's
                // -30 dB F2 of a bass "I" hides under the F1 skirt)
                for k in (0..3).filter(|&k| t.db[k] >= -20.0) {
                    let fk = t.f[k];
                    // a local maximum of the harmonic envelope within one
                    // harmonic spacing (+4 %) of the table frequency
                    let tol = f0 * 1.01 + fk * 0.04;
                    let peak = (2..80).find(|&h| {
                        env[h - 1] > env[h - 2]
                            && env[h - 1] >= env[h]
                            && (h as f32 * f0 - fk).abs() <= tol
                    });
                    assert!(
                        peak.is_some(),
                        "{reg:?} {v:?} F{} table {fk}: no envelope peak near it",
                        k + 1
                    );
                }
                // valley between F1 and F2 is lower than both peaks
                if t.f[1] - t.f[0] > 300.0 {
                    let mid = ((t.f[0] + t.f[1]) / 2.0 / f0).round() as usize;
                    let p1 = env[(t.f[0] / f0).round() as usize - 1];
                    assert!(env[mid - 1] < p1, "{reg:?} {v:?} valley");
                }
            }
        }
    }

    #[test]
    fn demon_register_is_the_bass_table_scaled_down() {
        let b = formants(Register::Bass, Vowel::A);
        let d = formants(Register::Demon, Vowel::A);
        for k in 0..5 {
            assert!((d.f[k] - b.f[k] * DEMON_SCALE).abs() < 0.01);
        }
    }

    #[test]
    fn vowel_morph_glides_instead_of_jumping() {
        // A (F2 1040) -> I (F2 1750) every quarter beat with a 50 ms glide
        let p = VoicePatch {
            vowels: vec![Vowel::A, Vowel::I],
            step: 0.5, // 250 ms at 120 BPM
            morph_ms: 50.0,
            ..VoicePatch::default()
        };
        let mut v = VoxState::new();
        let mut f2 = vec![];
        for i in 0..(0.6 * SR) as usize {
            v.render(&p, 100.0, SR, 120.0, 0.0);
            if i % 480 == 0 {
                f2.push(v.cur_f[1]);
            }
        }
        // before the step: at A; 10 ms after it: between; 200 ms after: at I
        assert!((f2[24] - 1040.0).abs() < 5.0, "{}", f2[24]);
        assert!(f2[26] > 1100.0 && f2[26] < 1700.0, "{}", f2[26]);
        assert!((f2[45] - 1750.0).abs() < 20.0, "{}", f2[45]);
    }

    #[test]
    fn formant_shift_moves_formants() {
        let p = VoicePatch {
            shift: 1.5,
            ..VoicePatch::default()
        };
        let mut v = VoxState::new();
        for _ in 0..4800 {
            v.render(&p, 100.0, SR, 120.0, 0.0);
        }
        assert!((v.cur_f[0] - 900.0).abs() < 2.0, "{}", v.cur_f[0]);
    }

    #[test]
    fn babble_jumps_between_vowels_and_is_deterministic() {
        let p = VoicePatch {
            vowels: vec![Vowel::A, Vowel::E, Vowel::I, Vowel::O, Vowel::U],
            step: 0.25,
            babble: 1.0,
            ..VoicePatch::default()
        };
        let run = |seed: u32| {
            let mut v = VoxState::new();
            v.seed(seed);
            let mut idx = vec![];
            for i in 0..SR as usize {
                v.render(&p, 100.0, SR, 120.0, 0.0);
                if i % 6000 == 3000 {
                    idx.push(v.step_index());
                }
            }
            idx
        };
        let a = run(5);
        assert_eq!(a, run(5));
        let distinct = {
            let mut d = a.clone();
            d.sort();
            d.dedup();
            d.len()
        };
        assert!(distinct >= 3, "{a:?}");
    }

    #[test]
    fn chop_gate_opens_and_closes_on_the_beat_grid() {
        let p = VoicePatch {
            chop_rate: 0.25, // 1/16 at 120 BPM = 125 ms
            chop_duty: 0.5,
            ..VoicePatch::default()
        };
        let o = steady(&p, 110.0, 0.5);
        let rms = |a: usize, b: usize| {
            (o[a..b].iter().map(|x| x * x).sum::<f32>() / (b - a) as f32).sqrt()
        };
        let open = rms(6000 + 300, 6000 + 2700);
        let shut = rms(6000 + 3300, 6000 + 5700);
        assert!(open > 0.02 && shut < open * 0.01, "open {open} shut {shut}");
    }

    #[test]
    fn stutter_repeats_the_slice_with_exact_timing() {
        // a ramp in, 100-frame slice, 3 repeats at the same pitch
        let mut s = Stutter::new(48000, 4);
        s.start(100, 3, 0.0);
        let out: Vec<f32> = (0..600).map(|i| s.process(i as f32, 0.0).0).collect();
        // recording plays live
        assert_eq!(out[50], 50.0);
        // each repeat replays frame 50 of the slice at 100 + 50, 200 + 50, 300 + 50
        for r in 1..=3 {
            assert!((out[r * 100 + 50] - 50.0).abs() < 1e-3, "repeat {r}");
        }
        // then live again (after the fade-in)
        assert!((out[450] - 450.0).abs() < 1e-3);
        assert!(!s.active());
        // pitch step: +12 st = double speed, the repeat lasts half as long
        let mut s = Stutter::new(48000, 4);
        s.start(100, 1, 12.0);
        let mut n = 0;
        for i in 0..400 {
            s.process(i as f32, 0.0);
            if !s.active() {
                break;
            }
            n += 1;
        }
        assert_eq!(n, 149, "100 recorded + 50 repeated frames");
    }

    #[test]
    fn serde_round_trip_and_defaults() {
        let p = preset("Glitch Babble").unwrap();
        let t = toml::to_string(&p).unwrap();
        let q: VoicePatch = toml::from_str(&t).unwrap();
        assert_eq!(p, q);
        // missing fields fall back to defaults
        let q: VoicePatch =
            toml::from_str("register = \"Demon\"\nvowels = [\"Er\", \"A\"]").unwrap();
        assert_eq!(q.register, Register::Demon);
        assert_eq!(q.vowels, vec![Vowel::Er, Vowel::A]);
        assert_eq!(q.shift, 1.0);
    }
}
