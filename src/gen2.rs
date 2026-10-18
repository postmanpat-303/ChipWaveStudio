//! Generators stage 2 (0.2.3.14): Analog, FM-4 and Noise.
//!
//! All three are written from scratch for ChipWave; the designs are informed
//! by (and credited to) these open-source projects and papers:
//! - Analog: the Moog Minimoog Model D signal flow (3 VCOs + noise into a
//!   ladder VCF with its own contour, then the VCA contour). Band-limited
//!   saw / pulse by polyBLEP (V. Välimäki & A. Huovilainen, "Antialiasing
//!   oscillators in subtractive synthesis", IEEE SPM 2007; M. Finke's polyBLEP
//!   write-up); oscillator drift and hard sync as in Surge XT (GPL-3,
//!   https://github.com/surge-synthesizer/surge) and Vital by Matt Tytel
//!   (GPL-3, https://github.com/mtytel/vital); the filter is ChipWave's
//!   Zavalishin ladder (src/ladder.rs).
//! - FM-4: phase-modulation operators with the eight 4-operator algorithms
//!   of the Yamaha DX21 / DX100 / TX81Z family (the DX7's 4-op cousins),
//!   feedback on operator 4 averaged over two samples as in Dexed / MSFA
//!   (Dexed by Pascal Gauthier, GPL-3, https://github.com/asb2m10/dexed;
//!   MSFA by Raph Levien / Google, Apache-2.0,
//!   https://github.com/google/music-synthesizer-for-android); macro layout
//!   informed by Vital's FM and Plaits' 2-op FM (Emilie Gillet, MIT).
//! - Noise: white (xorshift), pink (Paul Kellet's filter, public domain,
//!   musicdsp.org), brown (leaky integrator), crackle (Poisson clicks, the idea
//!   of SuperCollider's Dust / DaisySP's Dust, GPL-3 / MIT), through the SVF or
//!   the ladder with a swept cutoff; impact thump after the sc808 kick.
//!
//! No code from those projects is copied.
use crate::gen::{MacroDef, Rng};
use crate::synth::{Adsr, Env, FilterMode, Svf};
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

const C4: f32 = 261.625_58;

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

#[inline]
fn xs(n: &mut u32) -> f32 {
    *n ^= *n << 13;
    *n ^= *n >> 17;
    *n ^= *n << 5;
    *n as f32 / u32::MAX as f32 * 2.0 - 1.0
}

/// map knob `i` (with a 43xy override) through its definition
#[inline]
fn val(defs: &[MacroDef], p: &[f32], ov: &[f32; 8], i: usize) -> f32 {
    let v = if i < 8 && ov[i] >= 0.0 { ov[i] } else { p[i] };
    defs[i].map(v)
}

fn fix_knobs(p: &mut [f32]) {
    for v in p.iter_mut() {
        *v = if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.5
        };
    }
}

/// Dice: every unlocked knob inside its dice window
fn roll_knobs(p: &mut [f32], defs: &[MacroDef], locks: u64, r: &mut Rng) {
    for (i, d) in defs.iter().enumerate() {
        let x = r.f();
        if locks & (1 << i) == 0 {
            p[i] = d.dice.0 + (d.dice.1 - d.dice.0) * x;
        }
    }
}

/// Mutate: every unlocked knob by up to ±`a`
fn mutate_knobs(p: &mut [f32], locks: u64, a: f32, r: &mut Rng) {
    for (i, v) in p.iter_mut().enumerate() {
        let x = r.f() * 2.0 - 1.0;
        if locks & (1 << i) == 0 {
            *v = (*v + x * a).clamp(0.0, 1.0);
        }
    }
}

/// lock bit of discrete choice `k` (waveforms, algorithm, colour ...)
pub const fn choice_lock(k: u32) -> u64 {
    1 << (48 + k)
}

fn fix_level(l: &mut f32) {
    *l = if l.is_finite() {
        l.clamp(0.0, 4.0)
    } else {
        1.0
    };
}

// ================================================================ Analog

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AWave {
    #[default]
    Saw,
    Triangle,
    Square,
    Pulse,
}

impl AWave {
    pub const ALL: [AWave; 4] = [AWave::Saw, AWave::Triangle, AWave::Square, AWave::Pulse];
    pub fn name(self) -> &'static str {
        match self {
            AWave::Saw => "Saw",
            AWave::Triangle => "Tri",
            AWave::Square => "Square",
            AWave::Pulse => "Pulse",
        }
    }
}

pub const AN: usize = 25;
pub const ANALOG: [MacroDef; AN] = [
    m(
        "Cutoff",
        30.0,
        16000.0,
        true,
        "Hz",
        (0.2, 0.7),
        "ladder cutoff",
    ),
    m(
        "Res",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.75),
        "ladder resonance (self-oscillates near the top)",
    ),
    m(
        "Env amt",
        0.0,
        1.0,
        false,
        "",
        (0.1, 0.8),
        "filter contour amount (up to 6 octaves)",
    ),
    m(
        "F.Decay",
        0.01,
        3.0,
        true,
        "s",
        (0.1, 0.7),
        "filter contour decay",
    ),
    m(
        "Detune",
        0.0,
        40.0,
        false,
        "",
        (0.0, 0.4),
        "osc 2 up / osc 3 down, in cents (shown as %)",
    ),
    m(
        "Drift",
        0.0,
        1.0,
        false,
        "",
        (0.1, 0.6),
        "analogue pitch and phase drift",
    ),
    m(
        "Drive",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.6),
        "overdrive into the ladder",
    ),
    m(
        "PWM",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.6),
        "pulse-width modulation depth (Pulse waves)",
    ),
    m(
        "Osc 1",
        0.0,
        1.0,
        false,
        "",
        (0.5, 1.0),
        "oscillator 1 level",
    ),
    m(
        "Osc 2",
        0.0,
        1.0,
        false,
        "",
        (0.0, 1.0),
        "oscillator 2 level",
    ),
    m(
        "Osc 3",
        0.0,
        1.0,
        false,
        "",
        (0.0, 1.0),
        "oscillator 3 level",
    ),
    m("Noise", 0.0, 1.0, false, "", (0.0, 0.2), "noise level"),
    m(
        "Oct 2",
        -2.0,
        2.0,
        false,
        "#",
        (0.25, 0.75),
        "oscillator 2 octave",
    ),
    m(
        "Oct 3",
        -2.0,
        2.0,
        false,
        "#",
        (0.0, 0.5),
        "oscillator 3 octave",
    ),
    m(
        "Semi 2",
        -12.0,
        12.0,
        false,
        "st",
        (0.5, 0.5),
        "oscillator 2 semitones",
    ),
    m(
        "F.Attack",
        0.001,
        2.0,
        true,
        "s",
        (0.0, 0.4),
        "filter contour attack",
    ),
    m(
        "F.Sustain",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.6),
        "filter contour sustain",
    ),
    m(
        "F.Release",
        0.005,
        3.0,
        true,
        "s",
        (0.2, 0.6),
        "filter contour release",
    ),
    m("A.Attack", 0.001, 2.0, true, "s", (0.0, 0.3), "amp attack"),
    m("A.Decay", 0.01, 3.0, true, "s", (0.2, 0.8), "amp decay"),
    m("A.Sustain", 0.0, 1.0, false, "", (0.3, 1.0), "amp sustain"),
    m(
        "A.Release",
        0.005,
        3.0,
        true,
        "s",
        (0.1, 0.5),
        "amp release",
    ),
    m(
        "Key trk",
        0.0,
        1.0,
        false,
        "",
        (0.3, 1.0),
        "cutoff follows the note",
    ),
    m(
        "PWM rate",
        0.05,
        8.0,
        true,
        "Hz",
        (0.2, 0.7),
        "PWM LFO rate",
    ),
    m(
        "Width",
        0.05,
        0.5,
        false,
        "",
        (0.3, 1.0),
        "pulse width (0.5 = square)",
    ),
];

/// Analog: three drifting oscillators + noise -> ladder -> amp
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalogP {
    pub waves: [AWave; 3],
    /// hard-sync oscillator 2 to oscillator 1
    pub sync: bool,
    pub mode: FilterMode,
    /// vintage (saturated, 2x) ladder, else clean
    pub vintage: bool,
    pub pole4: bool,
    pub p: [f32; AN],
    /// bit i locks knob i; `choice_lock(0..3)` the waves, 3 sync, 4 filter
    pub locks: u64,
    pub level: f32,
}

impl Default for AnalogP {
    fn default() -> Self {
        let mut a = AnalogP {
            waves: [AWave::Saw; 3],
            sync: false,
            mode: FilterMode::Lp,
            vintage: true,
            pole4: true,
            p: [0.5; AN],
            locks: 0,
            level: 1.0,
        };
        a.set(&[
            1200.0, 0.3, 0.45, 0.35, 8.0, 0.3, 0.15, 0.0, 1.0, 0.8, 0.0, 0.0, 0.0, -1.0, 0.0,
            0.001, 0.2, 0.3, 0.002, 0.6, 0.8, 0.15, 0.5, 0.6, 0.5,
        ]);
        a
    }
}

impl AnalogP {
    /// set knobs from real values (as many as given)
    pub fn set(&mut self, v: &[f32]) -> &mut Self {
        for (i, x) in v.iter().enumerate().take(AN) {
            self.p[i] = ANALOG[i].unmap(*x);
        }
        self
    }
    pub fn fix(&mut self) {
        fix_knobs(&mut self.p);
        fix_level(&mut self.level);
    }
    pub fn dice(&mut self, seed: u32) {
        let r = &mut Rng::new(seed);
        roll_knobs(&mut self.p, &ANALOG, self.locks, r);
        for k in 0..3 {
            let w = AWave::ALL[(r.next() % 4) as usize];
            if self.locks & choice_lock(k as u32) == 0 {
                self.waves[k] = w;
            }
        }
        let s = r.next().is_multiple_of(5);
        if self.locks & choice_lock(3) == 0 {
            self.sync = s;
        }
        // keep the sound audible: osc 1 at least half up
        if self.locks & 1 << 8 == 0 {
            self.p[8] = self.p[8].max(0.5);
        }
    }
    pub fn mutate(&mut self, a: f32, seed: u32) {
        mutate_knobs(&mut self.p, self.locks, a, &mut Rng::new(seed));
    }
}

/// polyBLEP residual (Välimäki & Huovilainen 2007)
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

#[derive(Clone, Copy, Default)]
pub struct AnalogState {
    started: bool,
    ph: [f32; 3],
    tri: [f32; 3],
    drift: [f32; 3],
    lfo: f32,
    nz: u32,
    lad: crate::ladder::Ladder,
    fenv: Env0,
    aenv: Env0,
    gate_was: bool,
    pub done: bool,
}

/// a synth::Env with a Default
#[derive(Clone, Copy)]
struct Env0(Env);
impl Default for Env0 {
    fn default() -> Self {
        Env0(Env::new())
    }
}

impl AnalogState {
    pub fn render(
        &mut self,
        a: &AnalogP,
        ov: &[f32; 8],
        freq: f32,
        sr: f32,
        gate: bool,
    ) -> (f32, f32, bool) {
        if self.done {
            return (0.0, 0.0, false);
        }
        let v = |i| val(&ANALOG, &a.p, ov, i);
        let drift = v(5);
        if !self.started {
            self.started = true;
            self.nz = 0x6A09_E667 ^ (freq.to_bits().wrapping_mul(2_654_435_761));
            self.gate_was = true;
            for k in 0..3 {
                // free-running phases: random start when drifting
                self.ph[k] = if drift > 0.0 {
                    (xs(&mut self.nz) * 0.5 + 0.5) * drift.min(1.0)
                } else {
                    0.0
                };
            }
        }
        if self.gate_was && !gate {
            self.fenv.0.release();
            self.aenv.0.release();
        }
        self.gate_was = gate;
        let dt = 1.0 / sr;
        let fe = self.fenv.0.next(
            &Adsr {
                a: v(15),
                d: v(3),
                s: v(16),
                r: v(17),
            },
            dt,
        );
        let ae = self.aenv.0.next(
            &Adsr {
                a: v(18),
                d: v(19),
                s: v(20),
                r: v(21),
            },
            dt,
        );
        if self.aenv.0.done() {
            self.done = true;
            return (0.0, 0.0, false);
        }
        let f0 = freq.max(1.0);
        let det = v(4);
        let oct = [0.0, v(12).round(), v(13).round()];
        let semi = [0.0, v(14).round(), 0.0];
        let cents = [0.0, det, -det];
        let lv = [v(8), v(9), v(10)];
        // PWM LFO
        self.lfo = (self.lfo + v(23) * dt).fract();
        let pw = (v(24) + v(7) * 0.42 * (self.lfo * TAU).sin()).clamp(0.04, 0.5);
        let mut mix = 0.0;
        let mut wrapped1 = false;
        let mut fr1 = 0.0;
        for k in 0..3 {
            // drift: a slow random walk (about +-8 cents at full drift)
            let w = xs(&mut self.nz);
            self.drift[k] += (w * 0.5 - self.drift[k]) * (3.0 * dt);
            let dc = self.drift[k] * drift * 30.0;
            let f = f0 * 2f32.powf(oct[k] + (semi[k] + (cents[k] + dc) * 0.01) / 12.0);
            let inc = (f * dt).min(0.45);
            let mut ph = self.ph[k] + inc;
            let mut wrapped = false;
            if ph >= 1.0 {
                ph -= 1.0;
                wrapped = true;
            }
            if k == 0 {
                wrapped1 = wrapped;
                fr1 = ph / inc.max(1e-9);
            }
            if k == 1 && a.sync && wrapped1 {
                // hard sync: restart where oscillator 1 wrapped
                ph = (fr1 * inc).clamp(0.0, 0.999);
            }
            self.ph[k] = ph;
            if lv[k] <= 0.0 {
                continue;
            }
            let y = match a.waves[k] {
                AWave::Saw => 2.0 * ph - 1.0 - blep(ph, inc),
                AWave::Square | AWave::Pulse => {
                    let w = if a.waves[k] == AWave::Square { 0.5 } else { pw };
                    let mut s = if ph < w { 1.0 } else { -1.0 };
                    s += blep(ph, inc);
                    s -= blep((ph - w + 1.0).fract(), inc);
                    s - (2.0 * w - 1.0)
                }
                AWave::Triangle => {
                    // leaky-integrated band-limited square
                    let mut s = if ph < 0.5 { 1.0 } else { -1.0 };
                    s += blep(ph, inc);
                    s -= blep((ph + 0.5).fract(), inc);
                    self.tri[k] = self.tri[k] * (1.0 - 0.0005) + s * 4.0 * inc;
                    self.tri[k]
                }
            };
            mix += y * lv[k];
        }
        mix += xs(&mut self.nz) * v(11) * 0.7;
        mix *= 0.9 * (1.0 + 3.0 * v(6));
        let kt = (f0 / C4).powf(v(22));
        let cut = (v(0) * kt * 2f32.powf(v(2) * 6.0 * fe)).clamp(20.0, sr * 0.45);
        let y = self
            .lad
            .process(mix, cut, v(1), sr, a.mode, a.pole4, a.vintage);
        let y = (y * 2.0).tanh() * ae * a.level * 0.9;
        if y.is_finite() {
            (y, y, false)
        } else {
            self.done = true;
            (0.0, 0.0, false)
        }
    }
}

// ================================================================ FM-4

/// the eight 4-operator algorithms (DX21 / DX100 / TX81Z numbering);
/// `mods[i]` = operators modulating operator i, `car` = carriers
pub const ALGOS: [([u8; 4], u8, &str); 8] = [
    ([0b0010, 0b0100, 0b1000, 0], 0b0001, "1: 4>3>2>1"),
    ([0b0010, 0b1100, 0, 0], 0b0001, "2: (3+4)>2>1"),
    ([0b1010, 0b0100, 0, 0], 0b0001, "3: (3>2 + 4)>1"),
    ([0b0110, 0, 0b1000, 0], 0b0001, "4: (4>3 + 2)>1"),
    ([0b0010, 0, 0b1000, 0], 0b0101, "5: 2>1, 4>3"),
    ([0b1000, 0b1000, 0b1000, 0], 0b0111, "6: 4>(1 2 3)"),
    ([0, 0, 0b1000, 0], 0b0111, "7: 4>3, 1, 2"),
    ([0, 0, 0, 0], 0b1111, "8: 1 2 3 4"),
];

/// frequency ratios the Ratio knobs step through
pub const RATIOS: [f32; 20] = [
    0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0,
    14.0, 16.0,
];

pub const FN: usize = 32;
const fn op(n: u8) -> [MacroDef; 6] {
    let (r, l, a, d, s, rl) = match n {
        1 => ("1 Ratio", "1 Level", "1 Atk", "1 Dec", "1 Sus", "1 Rel"),
        2 => ("2 Ratio", "2 Level", "2 Atk", "2 Dec", "2 Sus", "2 Rel"),
        3 => ("3 Ratio", "3 Level", "3 Atk", "3 Dec", "3 Sus", "3 Rel"),
        _ => ("4 Ratio", "4 Level", "4 Atk", "4 Dec", "4 Sus", "4 Rel"),
    };
    [
        m(
            r,
            0.0,
            19.0,
            false,
            "#",
            (0.0, 0.5),
            "frequency ratio step (0.5 .. 16)",
        ),
        m(
            l,
            0.0,
            1.0,
            false,
            "",
            (0.3, 1.0),
            "output level (modulation index for modulators)",
        ),
        m(a, 0.001, 3.0, true, "s", (0.0, 0.3), "attack"),
        m(d, 0.01, 6.0, true, "s", (0.2, 0.8), "decay"),
        m(s, 0.0, 1.0, false, "", (0.0, 0.8), "sustain"),
        m(rl, 0.01, 6.0, true, "s", (0.2, 0.6), "release"),
    ]
}

const FMACROS: [MacroDef; 8] = [
    m(
        "Bright",
        0.0,
        2.0,
        false,
        "x",
        (0.2, 0.8),
        "scales every modulator level",
    ),
    m(
        "Feedback",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.6),
        "operator 4 self-feedback",
    ),
    m(
        "Mod dec",
        0.25,
        4.0,
        true,
        "x",
        (0.2, 0.8),
        "scales the modulators' decay",
    ),
    m(
        "Ratio 2",
        0.0,
        19.0,
        false,
        "#",
        (0.0, 0.6),
        "operator 2 ratio (overrides 2 Ratio when moved by 43xy)",
    ),
    m(
        "Attack",
        0.25,
        4.0,
        true,
        "x",
        (0.2, 0.6),
        "scales every attack",
    ),
    m(
        "Release",
        0.25,
        4.0,
        true,
        "x",
        (0.2, 0.7),
        "scales every release",
    ),
    m(
        "Detune",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.4),
        "operators 2 / 4 detuned (up to 15 cents)",
    ),
    m(
        "Drive",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.5),
        "soft saturation of the output",
    ),
];

const fn fm_defs() -> [MacroDef; FN] {
    let mut d = [FMACROS[0]; FN];
    let mut i = 0;
    while i < 8 {
        d[i] = FMACROS[i];
        i += 1;
    }
    let mut o = 0;
    while o < 4 {
        let x = op(o as u8 + 1);
        let mut j = 0;
        while j < 6 {
            d[8 + o * 6 + j] = x[j];
            j += 1;
        }
        o += 1;
    }
    d
}

pub const FM: [MacroDef; FN] = fm_defs();

/// index of knob `j` (0 ratio, 1 level, 2 attack, 3 decay, 4 sustain, 5 release) of operator `o` (0..3)
pub const fn fk(o: usize, j: usize) -> usize {
    8 + o * 6 + j
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FmP {
    /// 0..7 (algorithm 1..8)
    pub algo: u8,
    pub p: [f32; FN],
    /// bit i locks knob i; `choice_lock(0)` the algorithm
    pub locks: u64,
    pub level: f32,
}

impl Default for FmP {
    fn default() -> Self {
        let mut f = FmP {
            algo: 0,
            p: [0.5; FN],
            locks: 0,
            level: 1.0,
        };
        f.set(&[1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
        for o in 0..4 {
            f.op(o, 1.0, if o == 0 { 1.0 } else { 0.5 }, 0.002, 1.0, 0.7, 0.3);
        }
        f
    }
}

impl FmP {
    pub fn set(&mut self, v: &[f32]) -> &mut Self {
        for (i, x) in v.iter().enumerate().take(FN) {
            self.p[i] = FM[i].unmap(*x);
        }
        self
    }
    /// set operator `o` (0..3): ratio (real), level, a, d, s, r
    #[allow(clippy::too_many_arguments)]
    pub fn op(
        &mut self,
        o: usize,
        ratio: f32,
        l: f32,
        a: f32,
        d: f32,
        s: f32,
        r: f32,
    ) -> &mut Self {
        let step = RATIOS
            .iter()
            .enumerate()
            .min_by(|x, y| (x.1 - ratio).abs().total_cmp(&(y.1 - ratio).abs()))
            .map(|x| x.0)
            .unwrap_or(1) as f32;
        for (j, x) in [step, l, a, d, s, r].into_iter().enumerate() {
            self.p[fk(o, j)] = FM[fk(o, j)].unmap(x);
        }
        // the Ratio 2 macro mirrors operator 2's ratio
        if o == 1 {
            self.p[3] = self.p[fk(1, 0)];
        }
        self
    }
    pub fn fix(&mut self) {
        fix_knobs(&mut self.p);
        fix_level(&mut self.level);
        self.algo = self.algo.min(7);
    }
    pub fn dice(&mut self, seed: u32) {
        let r = &mut Rng::new(seed);
        roll_knobs(&mut self.p, &FM, self.locks, r);
        let a = (r.next() % 8) as u8;
        if self.locks & choice_lock(0) == 0 {
            self.algo = a;
        }
        // carriers stay audible
        let car = ALGOS[self.algo as usize].1;
        for o in 0..4 {
            let k = fk(o, 1);
            if car & (1 << o) != 0 && self.locks & (1 << k) == 0 {
                self.p[k] = self.p[k].max(0.6);
            }
        }
        if self.locks & (1 << 3) == 0 {
            self.p[3] = self.p[fk(1, 0)];
        }
    }
    pub fn mutate(&mut self, a: f32, seed: u32) {
        mutate_knobs(&mut self.p, self.locks, a, &mut Rng::new(seed));
    }
}

#[derive(Clone, Copy, Default)]
pub struct FmState {
    started: bool,
    ph: [f32; 4],
    out: [f32; 4],
    fb: [f32; 2],
    env: [Env0; 4],
    gate_was: bool,
    pub done: bool,
}

impl FmState {
    pub fn render(
        &mut self,
        f: &FmP,
        ov: &[f32; 8],
        freq: f32,
        sr: f32,
        gate: bool,
    ) -> (f32, f32, bool) {
        if self.done {
            return (0.0, 0.0, false);
        }
        let v = |i| val(&FM, &f.p, ov, i);
        if !self.started {
            self.started = true;
            self.gate_was = true;
        }
        if self.gate_was && !gate {
            for e in self.env.iter_mut() {
                e.0.release();
            }
        }
        self.gate_was = gate;
        let (mods, car, _) = ALGOS[f.algo.min(7) as usize];
        let dt = 1.0 / sr;
        let (bright, fbk, mdec, atk, rel, det, drv) = (v(0), v(1), v(2), v(4), v(5), v(6), v(7));
        let f0 = freq.max(1.0);
        let mut alive = false;
        let mut lv = [0.0; 4];
        #[allow(clippy::needless_range_loop)]
        for o in 0..4 {
            let is_car = car & (1 << o) != 0;
            let dsc = if is_car { 1.0 } else { mdec };
            let e = self.env[o].0.next(
                &Adsr {
                    a: v(fk(o, 2)) * atk,
                    d: v(fk(o, 3)) * dsc,
                    s: v(fk(o, 4)),
                    r: v(fk(o, 5)) * rel,
                },
                dt,
            );
            if is_car && !self.env[o].0.done() {
                alive = true;
            }
            let l = v(fk(o, 1));
            lv[o] = if is_car { l * e } else { l * e * bright };
        }
        if !alive {
            self.done = true;
            return (0.0, 0.0, false);
        }
        let ratio = |o: usize| {
            let k = if o == 1 && ov[3] >= 0.0 { 3 } else { fk(o, 0) };
            RATIOS[(v(k).round() as usize).min(RATIOS.len() - 1)]
        };
        // operators 4 -> 1 (modulation flows down)
        for o in (0..4).rev() {
            let cents = match o {
                1 => det * 15.0,
                3 => -det * 9.0,
                _ => 0.0,
            };
            let fo = f0 * ratio(o) * 2f32.powf(cents / 1200.0);
            self.ph[o] = (self.ph[o] + fo * dt).fract();
            let mut pm = 0.0;
            for s in 0..4 {
                if mods[o] & (1 << s) != 0 {
                    pm += self.out[s];
                }
            }
            if o == 3 && fbk > 0.0 {
                pm += (self.fb[0] + self.fb[1]) * 0.5 * fbk * 1.6;
            }
            let car_o = car & (1 << o) != 0;
            // modulators: index up to ~2 pi at level 1 (x Bright)
            let y = (self.ph[o] * TAU + pm).sin();
            self.out[o] = if car_o { y * lv[o] } else { y * lv[o] * TAU };
            if o == 3 {
                self.fb[1] = self.fb[0];
                self.fb[0] = y * lv[3].min(1.0);
            }
        }
        let n = car.count_ones() as f32;
        let mut y = 0.0;
        for o in 0..4 {
            if car & (1 << o) != 0 {
                y += self.out[o];
            }
        }
        y /= n.sqrt();
        if drv > 0.0 {
            let g = 1.0 + 5.0 * drv;
            y = (y * g).tanh() / g.tanh();
        }
        let y = (y * f.level * 0.7).clamp(-2.0, 2.0);
        if y.is_finite() {
            (y, y, false)
        } else {
            self.done = true;
            (0.0, 0.0, false)
        }
    }
}

// ================================================================ Noise

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Color {
    #[default]
    White,
    Pink,
    Brown,
    Crackle,
}

impl Color {
    pub const ALL: [Color; 4] = [Color::White, Color::Pink, Color::Brown, Color::Crackle];
    pub fn name(self) -> &'static str {
        match self {
            Color::White => "White",
            Color::Pink => "Pink",
            Color::Brown => "Brown",
            Color::Crackle => "Crackle",
        }
    }
}

pub const NN: usize = 15;
pub const NOISE: [MacroDef; NN] = [
    m(
        "Length",
        0.05,
        16.0,
        true,
        "s",
        (0.2, 0.8),
        "sweep / envelope length",
    ),
    m(
        "Cut start",
        20.0,
        18000.0,
        true,
        "Hz",
        (0.2, 0.9),
        "filter cutoff at the start",
    ),
    m(
        "Cut end",
        20.0,
        18000.0,
        true,
        "Hz",
        (0.2, 0.9),
        "filter cutoff at the end of the sweep",
    ),
    m("Res", 0.0, 1.0, false, "", (0.0, 0.7), "filter resonance"),
    m(
        "Curve",
        0.0,
        1.0,
        false,
        "",
        (0.1, 0.9),
        "sweep and envelope curve (0 = fast, 1 = slow start)",
    ),
    m(
        "Rise",
        0.0,
        1.0,
        false,
        "",
        (0.0, 1.0),
        "part of the length spent rising (1 = riser, 0 = impact)",
    ),
    m("Width", 0.0, 1.0, false, "", (0.2, 1.0), "stereo width"),
    m("Drive", 0.0, 1.0, false, "", (0.0, 0.5), "saturation"),
    m(
        "Density",
        1.0,
        3000.0,
        true,
        "",
        (0.2, 0.8),
        "crackle clicks per second",
    ),
    m(
        "Gust",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.6),
        "slow random cutoff movement (wind)",
    ),
    m("Gust rate", 0.05, 4.0, true, "Hz", (0.1, 0.6), "gust speed"),
    m(
        "Release",
        0.005,
        2.0,
        true,
        "s",
        (0.1, 0.5),
        "fade after note-off (Hold sounds)",
    ),
    m(
        "Thump",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.0),
        "impact: a falling sine body",
    ),
    m(
        "Thump Hz",
        30.0,
        120.0,
        true,
        "Hz",
        (0.2, 0.7),
        "impact body pitch",
    ),
    m(
        "Hiss",
        0.0,
        1.0,
        false,
        "",
        (0.0, 0.3),
        "crackle: pink hiss under the clicks",
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoiseP {
    pub color: Color,
    /// ladder (else SVF)
    pub ladder: bool,
    pub mode: FilterMode,
    /// hold at the peak while the note is held (wind, vinyl), else one-shot
    pub hold: bool,
    /// the note transposes the cutoff (C-4 = as set)
    pub key_track: bool,
    pub p: [f32; NN],
    /// bit i locks knob i; `choice_lock(0)` the colour, 1 the filter, 2 hold
    pub locks: u64,
    pub level: f32,
}

impl Default for NoiseP {
    fn default() -> Self {
        let mut n = NoiseP {
            color: Color::White,
            ladder: false,
            mode: FilterMode::Lp,
            hold: false,
            key_track: false,
            p: [0.5; NN],
            locks: 0,
            level: 1.0,
        };
        n.set(&[
            2.0, 400.0, 9000.0, 0.3, 0.5, 1.0, 0.6, 0.0, 300.0, 0.0, 0.3, 0.2, 0.0, 55.0, 0.0,
        ]);
        n
    }
}

impl NoiseP {
    pub fn set(&mut self, v: &[f32]) -> &mut Self {
        for (i, x) in v.iter().enumerate().take(NN) {
            self.p[i] = NOISE[i].unmap(*x);
        }
        self
    }
    pub fn fix(&mut self) {
        fix_knobs(&mut self.p);
        fix_level(&mut self.level);
    }
    pub fn dice(&mut self, seed: u32) {
        let r = &mut Rng::new(seed);
        roll_knobs(&mut self.p, &NOISE, self.locks, r);
        let c = Color::ALL[(r.next() % 4) as usize];
        if self.locks & choice_lock(0) == 0 {
            self.color = c;
        }
        let l = r.next().is_multiple_of(2);
        if self.locks & choice_lock(1) == 0 {
            self.ladder = l;
        }
    }
    pub fn mutate(&mut self, a: f32, seed: u32) {
        mutate_knobs(&mut self.p, self.locks, a, &mut Rng::new(seed));
    }
}

#[derive(Clone, Copy, Default)]
pub struct NoiseState {
    started: bool,
    t: f32,
    nz: [u32; 2],
    pink: [[f32; 7]; 2],
    brown: [f32; 2],
    click: [f32; 2],
    gust: f32,
    gust_t: f32,
    svf: [Svf; 2],
    lad: [crate::ladder::Ladder; 2],
    thump_ph: f32,
    rel: f32,
    gate_was: bool,
    pub done: bool,
}

impl NoiseState {
    fn src(&mut self, c: Color, k: usize, dens: f32, hiss: f32, sr: f32) -> f32 {
        let w = xs(&mut self.nz[k]);
        let pink = |p: &mut [f32; 7], w: f32| {
            // Paul Kellet's refined pink filter (public domain)
            p[0] = 0.99886 * p[0] + w * 0.0555179;
            p[1] = 0.99332 * p[1] + w * 0.0750759;
            p[2] = 0.96900 * p[2] + w * 0.153852;
            p[3] = 0.86650 * p[3] + w * 0.3104856;
            p[4] = 0.55000 * p[4] + w * 0.5329522;
            p[5] = -0.7616 * p[5] - w * 0.0168980;
            let y = p[0] + p[1] + p[2] + p[3] + p[4] + p[5] + p[6] + w * 0.5362;
            p[6] = w * 0.115926;
            y * 0.11
        };
        match c {
            Color::White => w,
            Color::Pink => pink(&mut self.pink[k], w),
            Color::Brown => {
                self.brown[k] = (self.brown[k] + w * 0.02) * 0.998;
                self.brown[k] * 3.5
            }
            Color::Crackle => {
                // Poisson clicks: probability dens / sr per sample
                let u = xs(&mut self.nz[k]) * 0.5 + 0.5;
                if u < dens / sr {
                    let a = xs(&mut self.nz[k]);
                    self.click[k] = a.signum() * (0.3 + 0.7 * a.abs());
                }
                let y = self.click[k];
                self.click[k] *= 0.85;
                y + pink(&mut self.pink[k], w) * hiss * 0.5
            }
        }
    }

    pub fn render(
        &mut self,
        n: &NoiseP,
        ov: &[f32; 8],
        freq: f32,
        sr: f32,
        gate: bool,
    ) -> (f32, f32, bool) {
        if self.done {
            return (0.0, 0.0, false);
        }
        let v = |i| val(&NOISE, &n.p, ov, i);
        if !self.started {
            self.started = true;
            self.nz = [0x9E37_79B9, 0x7F4A_7C15];
            self.gate_was = true;
            self.rel = 1.0;
        }
        let dt = 1.0 / sr;
        let len = v(0);
        let curve = v(4);
        let rise = v(5);
        let t = self.t;
        self.t += dt;
        let x = (t / len).min(1.0);
        // curve exponent: 0 -> 0.3 (fast), 1 -> 3 (slow start)
        let k = 0.3 * 10f32.powf(curve);
        let env = if x < rise {
            (x / rise.max(1e-6)).powf(k)
        } else if n.hold {
            1.0 // held at the peak; the release below fades it after note-off
        } else {
            let d = (x - rise) / (1.0 - rise).max(1e-6);
            (1.0 - d).max(0.0).powf(1.0 / k.max(0.05) * 1.5)
        };
        if self.gate_was && !gate {
            self.gate_was = false;
        }
        if !gate {
            self.rel *= (-dt * 5.0 / v(11)).exp();
        }
        if (!n.hold && x >= 1.0) || self.rel < 1e-4 {
            self.done = true;
            return (0.0, 0.0, false);
        }
        // swept cutoff (exponential), key track, wind gusts
        let sweep = x.powf(k);
        let (c0, c1) = (v(1), v(2));
        let mut cut = c0 * (c1 / c0).powf(sweep);
        if n.key_track && freq > 0.0 {
            cut *= freq / C4;
        }
        let gust = v(9);
        if gust > 0.0 {
            self.gust_t += v(10) * dt;
            if self.gust_t >= 1.0 {
                self.gust_t -= 1.0;
            }
            let target = xs(&mut self.nz[0]);
            self.gust += (target - self.gust) * (v(10) * 4.0 * dt).min(1.0);
            cut *= 2f32.powf(self.gust * gust * 6.0);
        }
        let cut = cut.clamp(20.0, sr * 0.45);
        let res = v(3);
        let (dens, hiss) = (v(8), v(14));
        let a = self.src(n.color, 0, dens, hiss, sr);
        let b = self.src(n.color, 1, dens, hiss, sr);
        let w = v(6);
        let raw = [a, a * (1.0 - w) + b * w];
        let mut out = [0.0; 2];
        for (c, o) in out.iter_mut().enumerate() {
            *o = if n.ladder {
                self.lad[c].process(raw[c], cut, res, sr, n.mode, true, false)
            } else {
                self.svf[c].process(raw[c], cut, res, sr, n.mode)
            };
        }
        // impact body
        let th = v(12);
        let mut body = 0.0;
        if th > 0.0 {
            let fb = v(13) * (1.0 + 3.0 * (-t / 0.03).exp());
            self.thump_ph = (self.thump_ph + fb * dt).fract();
            body = (self.thump_ph * TAU).sin() * th * (-t / 0.35).exp() * 1.2;
        }
        let drv = v(7);
        let g = env * self.rel * n.level * 0.9;
        let shape = |y: f32| {
            if drv > 0.0 {
                let d = 1.0 + 6.0 * drv;
                (y * d).tanh() / d.tanh()
            } else {
                y
            }
        };
        let l = shape(out[0] + body) * g;
        let r = shape(out[1] + body) * g;
        if l.is_finite() && r.is_finite() {
            (l.clamp(-2.0, 2.0), r.clamp(-2.0, 2.0), w > 0.0)
        } else {
            self.done = true;
            (0.0, 0.0, false)
        }
    }
}

// ================================================================ presets

pub fn presets() -> Vec<(&'static str, &'static str, crate::gen::GenEngine, f32, bool)> {
    use crate::gen::GenEngine as E;
    let mut v = vec![];
    // (name, tip, engine, glide seconds, legato)
    let an = |w: [AWave; 3], vals: &[f32]| {
        let mut a = AnalogP {
            waves: w,
            ..Default::default()
        };
        a.set(vals);
        a
    };
    use AWave::*;
    v.push((
        "Fat Moog Bass",
        "analog: three saws an octave apart, vintage ladder, punchy contour",
        E::Analog(an(
            [Saw, Saw, Square],
            &[
                380.0, 0.35, 0.55, 0.28, 7.0, 0.35, 0.35, 0.0, 1.0, 0.8, 0.6, 0.0, 0.0, -1.0, 0.0,
                0.001, 0.15, 0.15, 0.002, 0.5, 0.75, 0.08, 0.4, 0.5, 0.5,
            ],
        )),
        0.04,
        true,
    ));
    v.push((
        "Psy Rolling Bass",
        "analog: dark psy offbeat bass (play E or F#), short plucky contour",
        E::Analog(an(
            [Saw, Square, Saw],
            &[
                240.0, 0.45, 0.6, 0.11, 4.0, 0.15, 0.3, 0.0, 1.0, 0.5, 0.0, 0.0, 0.0, -1.0, 0.0,
                0.001, 0.0, 0.08, 0.001, 0.14, 0.0, 0.03, 0.6, 0.5, 0.5,
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Warm Lead",
        "analog: detuned saw + pulse with PWM, gentle drift, glide",
        E::Analog(an(
            [Saw, Pulse, Triangle],
            &[
                2400.0, 0.25, 0.3, 0.6, 12.0, 0.45, 0.2, 0.35, 1.0, 0.7, 0.4, 0.02, 0.0, 1.0, 0.0,
                0.01, 0.5, 0.4, 0.01, 0.8, 0.85, 0.3, 0.6, 0.8, 0.4,
            ],
        )),
        0.08,
        true,
    ));
    v.push((
        "Brass",
        "analog: slow filter attack on stacked saws",
        E::Analog(an(
            [Saw, Saw, Saw],
            &[
                700.0, 0.15, 0.55, 0.5, 10.0, 0.4, 0.25, 0.0, 1.0, 0.9, 0.5, 0.0, 0.0, 0.0, 7.0,
                0.12, 0.55, 0.3, 0.04, 0.6, 0.85, 0.25, 0.5, 0.5, 0.5,
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Dark Pad",
        "analog: slow, drifting, dark detuned saws and noise",
        E::Analog(an(
            [Saw, Saw, Pulse],
            &[
                450.0, 0.3, 0.25, 2.0, 22.0, 0.7, 0.1, 0.5, 0.9, 0.9, 0.6, 0.06, 0.0, -1.0, 0.0,
                1.5, 0.6, 2.0, 1.2, 2.0, 0.9, 2.5, 0.3, 0.2, 0.35,
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Acid Squelch",
        "analog: one saw, high resonance, snappy contour, glide (acid-ish)",
        E::Analog(an(
            [Saw, Saw, Saw],
            &[
                320.0, 0.82, 0.7, 0.16, 0.0, 0.1, 0.45, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.001, 0.05, 0.08, 0.001, 0.3, 0.7, 0.04, 0.3, 0.5, 0.5,
            ],
        )),
        0.07,
        true,
    ));
    // FM-4
    let fm = |algo: u8, mac: &[f32], ops: [[f32; 6]; 4]| {
        let mut f = FmP {
            algo,
            ..Default::default()
        };
        f.set(mac);
        for (o, x) in ops.iter().enumerate() {
            f.op(o, x[0], x[1], x[2], x[3], x[4], x[5]);
        }
        f
    };
    v.push((
        "E-Piano",
        "FM-4: two stacks (algorithm 5), tine-like 14:1 attack",
        E::Fm(fm(
            4,
            &[0.9, 0.0, 1.0, 1.0, 1.0, 1.0, 0.25, 0.0],
            [
                [1.0, 1.0, 0.002, 2.5, 0.0, 0.5],
                [1.0, 0.35, 0.002, 1.2, 0.0, 0.4],
                [1.0, 0.7, 0.002, 1.8, 0.0, 0.5],
                [14.0, 0.25, 0.001, 0.25, 0.0, 0.2],
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Metallic Bell",
        "FM-4: inharmonic 3.5 ratio, long ring",
        E::Fm(fm(
            4,
            &[1.0, 0.1, 1.0, 1.0, 1.0, 1.0, 0.4, 0.0],
            [
                [1.0, 1.0, 0.001, 4.5, 0.0, 2.5],
                [3.5, 0.6, 0.001, 3.0, 0.0, 2.0],
                [2.0, 0.6, 0.001, 3.5, 0.0, 2.5],
                [7.0, 0.35, 0.001, 1.5, 0.0, 1.0],
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Psy Zap Bass",
        "FM-4: serial stack with a fast-decaying index (zappy psy bass)",
        E::Fm(fm(
            0,
            &[1.0, 0.3, 1.0, 1.0, 1.0, 1.0, 0.0, 0.3],
            [
                [0.5, 1.0, 0.001, 0.25, 0.0, 0.05],
                [1.0, 0.8, 0.001, 0.09, 0.0, 0.05],
                [2.0, 0.5, 0.001, 0.05, 0.0, 0.05],
                [1.0, 0.4, 0.001, 0.04, 0.0, 0.05],
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Growl",
        "FM-4: feedback into a sustained stack (algorithm 2), dirty",
        E::Fm(fm(
            1,
            &[1.1, 0.75, 1.0, 1.0, 1.0, 1.0, 0.3, 0.45],
            [
                [0.5, 1.0, 0.005, 1.0, 0.85, 0.15],
                [1.0, 0.55, 0.005, 0.6, 0.6, 0.15],
                [1.5, 0.35, 0.005, 0.8, 0.5, 0.15],
                [1.0, 0.6, 0.005, 0.5, 0.7, 0.15],
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Dark Pluck",
        "FM-4: soft, short pluck (algorithm 3)",
        E::Fm(fm(
            2,
            &[0.6, 0.15, 1.0, 1.0, 1.0, 1.0, 0.2, 0.0],
            [
                [1.0, 1.0, 0.002, 0.6, 0.0, 0.3],
                [2.0, 0.45, 0.001, 0.2, 0.0, 0.2],
                [1.0, 0.35, 0.001, 0.15, 0.0, 0.2],
                [3.0, 0.2, 0.001, 0.1, 0.0, 0.2],
            ],
        )),
        0.0,
        false,
    ));
    // Noise
    let nz = |c: Color, ladder: bool, mode: FilterMode, hold: bool, vals: &[f32]| {
        let mut n = NoiseP {
            color: c,
            ladder,
            mode,
            hold,
            ..Default::default()
        };
        n.set(vals);
        n
    };
    v.push((
        "White Riser",
        "noise: 4 s white riser, band-pass sweeping up",
        E::Noise(nz(
            Color::White,
            false,
            FilterMode::Bp,
            false,
            &[
                4.0, 300.0, 9000.0, 0.45, 0.6, 0.97, 0.8, 0.0, 300.0, 0.0, 0.3, 0.2, 0.0, 55.0, 0.0,
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Downlifter",
        "noise: 3 s fall, ladder low-pass closing",
        E::Noise(nz(
            Color::White,
            true,
            FilterMode::Lp,
            false,
            &[
                3.0, 12000.0, 150.0, 0.35, 0.35, 0.02, 0.7, 0.0, 300.0, 0.0, 0.3, 0.2, 0.0, 55.0,
                0.0,
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Impact",
        "noise: a hit with a falling sine body and a closing filter",
        E::Noise(nz(
            Color::Pink,
            true,
            FilterMode::Lp,
            false,
            &[
                1.8, 9000.0, 200.0, 0.2, 0.25, 0.0, 0.6, 0.35, 300.0, 0.0, 0.3, 0.2, 0.85, 48.0,
                0.0,
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Wind",
        "noise: held pink noise, resonant band-pass with gusts",
        E::Noise(nz(
            Color::Pink,
            false,
            FilterMode::Bp,
            true,
            &[
                1.5, 700.0, 700.0, 0.6, 0.5, 1.0, 0.9, 0.0, 300.0, 0.6, 0.25, 1.0, 0.0, 55.0, 0.0,
            ],
        )),
        0.0,
        false,
    ));
    v.push((
        "Vinyl Crackle",
        "noise: held crackle with soft hiss",
        E::Noise(nz(
            Color::Crackle,
            false,
            FilterMode::Lp,
            true,
            &[
                0.1, 7000.0, 7000.0, 0.1, 0.5, 1.0, 0.5, 0.0, 25.0, 0.0, 0.3, 0.3, 0.0, 55.0, 0.35,
            ],
        )),
        0.0,
        false,
    ));
    // level calibration (peaks about 0.6-0.9, similar loudness)
    for (name, _, e, _, _) in v.iter_mut() {
        let l = match *name {
            "Psy Rolling Bass" => 1.3,
            "Dark Pad" => 1.1,
            "Growl" => 0.6,
            "Fat Moog Bass" | "Acid Squelch" => 0.75,
            "Warm Lead" | "Brass" => 0.65,
            "White Riser" => 1.1,
            "Wind" => 3.0,
            _ => 1.0,
        };
        match e {
            crate::gen::GenEngine::Analog(a) => a.level = l,
            crate::gen::GenEngine::Fm(f) => f.level = l,
            crate::gen::GenEngine::Noise(n) => n.level = l,
            _ => {}
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 48000.0;

    fn run<F: FnMut(bool) -> (f32, f32, bool)>(
        mut f: F,
        hold_s: f32,
        max_s: f32,
    ) -> Vec<(f32, f32)> {
        let mut v = vec![];
        for n in 0..(max_s * SR) as usize {
            let gate = (n as f32) < hold_s * SR;
            let (l, r, _) = f(gate);
            v.push((l, r));
        }
        v
    }

    fn peak(v: &[(f32, f32)]) -> f32 {
        v.iter()
            .fold(0.0f32, |m, (l, r)| m.max(l.abs()).max(r.abs()))
    }

    fn zc(v: &[(f32, f32)], a: usize, b: usize) -> f32 {
        let s = &v[a..b];
        let c = s
            .windows(2)
            .filter(|w| w[0].0 < 0.0 && w[1].0 >= 0.0)
            .count();
        c as f32 / ((b - a) as f32 / SR)
    }

    #[test]
    fn analog_pitch_detune_drift_and_extremes() {
        // a single clean saw, filter open: the fundamental is the note
        let mut a = AnalogP::default();
        a.set(&[
            16000.0, 0.0, 0.0, 0.3, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
        ]);
        a.vintage = false;
        a.waves[0] = AWave::Triangle;
        let mut s = AnalogState::default();
        let o = run(|g| s.render(&a, &[-1.0; 8], 220.0, SR, g), 1.0, 1.0);
        let f = zc(&o, 12000, 48000);
        assert!((f - 220.0).abs() < 3.0, "{f}");
        // drift moves the pitch only a little
        a.p[5] = 1.0;
        let mut s = AnalogState::default();
        let o = run(|g| s.render(&a, &[-1.0; 8], 220.0, SR, g), 1.0, 1.0);
        let f = zc(&o, 12000, 48000);
        assert!((f - 220.0).abs() < 4.0, "drift {f}");
        // extremes: every knob 0 / 1, every wave, sync, notes 8 Hz .. 20 kHz
        for k in 0..32u32 {
            let mut a = AnalogP::default();
            for i in 0..AN {
                a.p[i] = if (k >> (i % 5)) & 1 == 1 { 1.0 } else { 0.0 };
            }
            a.p[8] = 1.0;
            a.p[20] = 1.0;
            a.waves = [AWave::ALL[k as usize % 4]; 3];
            a.sync = k % 2 == 0;
            a.vintage = k % 3 == 0;
            for f in [0.0, 8.0, 55.0, 2000.0, 20000.0] {
                let mut s = AnalogState::default();
                let o = run(|g| s.render(&a, &[-1.0; 8], f, SR, g), 0.3, 0.6);
                assert!(o.iter().all(|(l, r)| l.is_finite()
                    && r.is_finite()
                    && l.abs() <= 2.0
                    && r.abs() <= 2.0));
            }
        }
        // 43xy: the cutoff macro closes the filter (quieter, darker)
        let a = AnalogP::default();
        let mut s1 = AnalogState::default();
        let open = run(
            |g| {
                s1.render(
                    &a,
                    &[1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0],
                    110.0,
                    SR,
                    g,
                )
            },
            0.5,
            0.5,
        );
        let mut s2 = AnalogState::default();
        let shut = run(
            |g| {
                s2.render(
                    &a,
                    &[0.0, -1.0, 0.0, -1.0, -1.0, -1.0, -1.0, -1.0],
                    110.0,
                    SR,
                    g,
                )
            },
            0.5,
            0.5,
        );
        let rms =
            |v: &[(f32, f32)]| (v.iter().map(|x| x.0 * x.0).sum::<f32>() / v.len() as f32).sqrt();
        assert!(
            rms(&open) > 2.0 * rms(&shut),
            "{} {}",
            rms(&open),
            rms(&shut)
        );
    }

    #[test]
    fn analog_note_off_releases_and_ends() {
        let a = AnalogP::default();
        let mut s = AnalogState::default();
        let mut n = 0;
        while !s.done && n < (SR * 3.0) as usize {
            s.render(&a, &[-1.0; 8], 110.0, SR, n < 4800);
            n += 1;
        }
        assert!(s.done, "ends after note-off");
        assert!(n < (SR * 1.5) as usize);
    }

    #[test]
    fn fm_algorithms_ratios_and_extremes() {
        // algorithm 8, only op 1 sounding, ratio 2 -> octave
        let mut f = FmP {
            algo: 7,
            ..Default::default()
        };
        for o in 1..4 {
            f.p[fk(o, 1)] = 0.0;
        }
        f.op(0, 2.0, 1.0, 0.001, 1.0, 1.0, 0.1);
        let mut s = FmState::default();
        let o = run(|g| s.render(&f, &[-1.0; 8], 220.0, SR, g), 1.0, 1.0);
        let fr = zc(&o, 4800, 48000);
        assert!((fr - 440.0).abs() < 3.0, "{fr}");
        // modulation brightens: more zero crossings with index
        let mut f2 = FmP {
            algo: 0,
            ..Default::default()
        };
        f2.op(1, 3.0, 1.0, 0.001, 1.0, 1.0, 0.1);
        let mut s = FmState::default();
        let o2 = run(|g| f2_render(&mut s, &f2, g), 1.0, 1.0);
        fn f2_render(s: &mut FmState, f: &FmP, g: bool) -> (f32, f32, bool) {
            s.render(f, &[-1.0; 8], 220.0, SR, g)
        }
        assert!(zc(&o2, 4800, 48000) > 300.0);
        for algo in 0..8u8 {
            for k in 0..16u32 {
                let mut f = FmP {
                    algo,
                    ..Default::default()
                };
                for i in 0..FN {
                    f.p[i] = if (k >> (i % 4)) & 1 == 1 { 1.0 } else { 0.0 };
                }
                for fq in [0.0, 30.0, 440.0, 12000.0] {
                    let mut s = FmState::default();
                    let o = run(|g| s.render(&f, &[-1.0; 8], fq, SR, g), 0.2, 0.4);
                    assert!(
                        o.iter().all(|(l, _)| l.is_finite() && l.abs() <= 2.0),
                        "algo {algo}"
                    );
                }
            }
        }
    }

    #[test]
    fn noise_colours_envelopes_and_extremes() {
        // pink and brown tilt towards the lows vs white
        let lowness = |c: Color| {
            let mut n = NoiseP {
                color: c,
                hold: true,
                ..Default::default()
            };
            n.set(&[1.0, 18000.0, 18000.0, 0.0, 0.5, 0.0, 0.0]);
            let mut s = NoiseState::default();
            let o = run(|g| s.render(&n, &[-1.0; 8], 0.0, SR, g), 1.0, 1.0);
            let d: f32 = o.windows(2).map(|w| (w[1].0 - w[0].0).powi(2)).sum();
            let e: f32 = o.iter().map(|x| x.0 * x.0).sum();
            e / d.max(1e-9)
        };
        let (w, p, b) = (
            lowness(Color::White),
            lowness(Color::Pink),
            lowness(Color::Brown),
        );
        assert!(p > 2.0 * w && b > 2.0 * p, "{w} {p} {b}");
        // a riser grows, a downlifter falls, a one-shot ends after Length
        for (name, _, e, _, _) in presets() {
            let crate::gen::GenEngine::Noise(n) = e else {
                continue;
            };
            let mut s = NoiseState::default();
            let o = run(|g| s.render(&n, &[-1.0; 8], C4, SR, g), 2.0, 6.0);
            assert!(o.iter().all(|(l, r)| l.is_finite() && r.is_finite()));
            assert!(peak(&o) > 0.05, "{name} silent");
            assert!(s.done, "{name} ends");
        }
        for k in 0..32u32 {
            let mut n = NoiseP::default();
            for i in 0..NN {
                n.p[i] = if (k >> (i % 5)) & 1 == 1 { 1.0 } else { 0.0 };
            }
            n.color = Color::ALL[k as usize % 4];
            n.ladder = k % 2 == 0;
            let mut s = NoiseState::default();
            let o = run(|g| s.render(&n, &[-1.0; 8], 440.0, SR, g), 0.2, 0.4);
            assert!(o
                .iter()
                .all(|(l, r)| l.is_finite() && r.is_finite() && l.abs() <= 2.0 && r.abs() <= 2.0));
        }
    }

    #[test]
    fn dice_mutate_and_locks_for_all_three() {
        let mut a = AnalogP {
            locks: 1 | choice_lock(0),
            ..Default::default()
        };
        let keep = (a.p[0], a.waves[0]);
        let mut f = FmP {
            locks: (1 << 1) | choice_lock(0),
            ..Default::default()
        };
        let fk1 = (f.p[1], f.algo);
        let mut n = NoiseP {
            locks: (1 << 2) | choice_lock(0),
            ..Default::default()
        };
        let nk = (n.p[2], n.color);
        for s in 0..40 {
            a.dice(s);
            a.mutate(0.3, s);
            f.dice(s);
            f.mutate(0.3, s);
            n.dice(s);
            n.mutate(0.3, s);
            assert_eq!((a.p[0], a.waves[0]), keep);
            assert_eq!((f.p[1], f.algo), fk1);
            assert_eq!((n.p[2], n.color), nk);
            assert!(a
                .p
                .iter()
                .chain(f.p.iter())
                .chain(n.p.iter())
                .all(|x| (0.0..=1.0).contains(x)));
        }
        let mut x = AnalogP::default();
        let mut y = AnalogP::default();
        x.dice(9);
        y.dice(9);
        assert_eq!(x, y, "seeded");
    }

    #[test]
    fn psy_rolling_bass_is_short_and_dark() {
        let e = presets()
            .into_iter()
            .find(|p| p.0 == "Psy Rolling Bass")
            .unwrap()
            .2;
        let crate::gen::GenEngine::Analog(a) = e else {
            panic!()
        };
        let mut s = AnalogState::default();
        // E-1 (41.2 Hz), a 16th at 145 BPM
        let o = run(|g| s.render(&a, &[-1.0; 8], 41.2, SR, g), 0.1, 0.4);
        let first: f32 = o[..4800].iter().map(|x| x.0.abs()).fold(0.0, f32::max);
        let late: f32 = o[12000..].iter().map(|x| x.0.abs()).fold(0.0, f32::max);
        assert!(
            first > 0.2 && late < 0.02 * first.max(1e-6) + 0.01,
            "{first} {late}"
        );
    }
}

#[cfg(test)]
mod levels2 {
    #[test]
    #[ignore]
    fn print_levels2() {
        for (name, _, g) in crate::gen::presets().into_iter().skip(30) {
            for f in [41.2f32, 261.6] {
                let mut v = crate::synth::SynthVoice::seeded(5);
                let mut pk = 0.0f32;
                let mut e = 0.0;
                let n = 48000 * 6;
                for i in 0..n {
                    if i == 48000 * 4 {
                        v.amp.release();
                    }
                    let (l, _) = v.render_gen(&g.base, &g, f, 48000.0, 120.0);
                    pk = pk.max(l.abs());
                    if i < 48000 {
                        e += l * l;
                    }
                }
                println!(
                    "{name:18} {f:6.1} peak {pk:.3} rms1s {:.3}",
                    (e / 48000.0f32).sqrt()
                );
            }
        }
    }
}
