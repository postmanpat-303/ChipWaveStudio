//! Modulation effects added after 0.2.4.0: the **Phaser** and the
//! **Frequency shifter** FX devices (see crate::fx for the device / param side
//! and docs/FX.md). Independent implementations of published, textbook
//! methods; no code copied:
//!
//! - Phaser: a cascade of first-order all-pass filters swept by an LFO and
//!   mixed with the dry signal (the classic analog phaser structure, e.g. the
//!   MXR Phase 90 / Small Stone family). Each all-pass is the zero-delay-
//!   feedback ("TPT") one-pole of Vadim Zavalishin, "The Art of VA Filter
//!   Design" (Native Instruments, 2012-2018), so the sweep stays exact up to
//!   Nyquist.
//! - Frequency shifter: the single-sideband ("Bode") frequency shifter of
//!   Harald Bode's 1960s analog designs: a Hilbert transformer splits the
//!   signal into two copies 90° apart, which are ring-modulated by a
//!   quadrature (cos / sin) oscillator and summed or subtracted for the upper
//!   or lower sideband. The Hilbert transformer is a pair of all-pass chains
//!   with the published coefficients of Olli Niemitalo, "Hilbert transform
//!   by IIR all-pass filters" (yehar.com, about 2001): 90° ± about 0.7°
//!   phase difference over almost the whole band (numbers only, no code).
//!
//! Everything here is allocation-free: fixed-size arrays, built once.
use std::f32::consts::{PI, TAU};

/// most all-pass stages a phaser can have
pub const MAX_STAGES: usize = 12;
/// the Stages choice (index -> number of all-pass stages)
pub const STAGES: [usize; 4] = [4, 6, 8, 12];
/// sweep range at 100 % depth: the centre ± this many octaves
pub const PHASER_OCTAVES: f32 = 2.5;

/// one channel of the all-pass cascade
#[derive(Clone, Copy, Default)]
struct ApChain {
    s: [f32; MAX_STAGES],
    /// last wet output (one-sample feedback path)
    fb: f32,
}

impl ApChain {
    /// `g` = tan(pi fc / sr) of every stage, `n` stages, feedback `k`
    #[inline]
    fn run(&mut self, x: f32, g: f32, n: usize, k: f32) -> f32 {
        let a = g / (1.0 + g);
        let mut y = (x + k * self.fb).clamp(-4.0, 4.0);
        for s in self.s.iter_mut().take(n) {
            // TPT one-pole: lp = v + s, all-pass = 2 lp - x
            let v = (y - *s) * a;
            let lp = v + *s;
            *s = lp + v;
            y = 2.0 * lp - y;
        }
        self.fb = y;
        y
    }
}

/// realtime state of a Phaser device (both channels)
#[derive(Clone, Copy, Default)]
pub struct Phaser {
    ch: [ApChain; 2],
    /// LFO phase 0..1
    ph: f32,
}

/// the phaser's real parameter values, in `crate::fx::PHASER` order
pub struct PhaserParams {
    pub stages: usize,
    /// LFO rate in Hz (already tempo-synced if asked)
    pub rate: f32,
    pub depth: f32,
    pub center: f32,
    pub feedback: f32,
    /// right-channel LFO offset in degrees
    pub stereo: f32,
    pub mix: f32,
}

impl Phaser {
    /// the sweep frequency for an LFO value -1..1
    #[inline]
    pub fn sweep_hz(center: f32, depth: f32, lfo: f32, sr: f32) -> f32 {
        (center * (depth * PHASER_OCTAVES * lfo).exp2()).clamp(10.0, sr * 0.49)
    }

    pub fn process(&mut self, l: f32, r: f32, p: &PhaserParams, sr: f32) -> (f32, f32) {
        self.ph = (self.ph + p.rate / sr).fract();
        let n = p.stages.clamp(1, MAX_STAGES);
        let off = p.stereo / 360.0;
        let mut out = [0.0f32; 2];
        for (c, (x, o)) in [l, r].into_iter().zip(out.iter_mut()).enumerate() {
            let lfo = ((self.ph + off * c as f32) * TAU).sin();
            let fc = Self::sweep_hz(p.center, p.depth, lfo, sr);
            let g = (PI * fc / sr).tan();
            let y = self.ch[c].run(x, g, n, p.feedback);
            *o = x * (1.0 - p.mix) + y * p.mix;
        }
        (out[0], out[1])
    }
}

/// Olli Niemitalo's all-pass coefficients for the two Hilbert paths; each
/// section is y[n] = c²·(x[n] + y[n-2]) − x[n-2]
const HILBERT_A: [f32; 4] = [0.692_387_8, 0.936_065_43, 0.988_229_5, 0.998_748_8];
const HILBERT_B: [f32; 4] = [0.402_192_12, 0.856_171_1, 0.972_290_95, 0.995_288_5];

#[derive(Clone, Copy, Default)]
struct Ap2 {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Ap2 {
    #[inline]
    fn run(&mut self, x: f32, c2: f32) -> f32 {
        let y = c2 * (x + self.y2) - self.x2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// one channel of the Hilbert transformer: (in-phase, quadrature)
#[derive(Clone, Copy, Default)]
struct Hilbert {
    a: [Ap2; 4],
    b: [Ap2; 4],
    /// path A's one-sample delay
    bz: f32,
}

impl Hilbert {
    #[inline]
    fn run(&mut self, x: f32) -> (f32, f32) {
        let mut i = x;
        for (s, c) in self.a.iter_mut().zip(HILBERT_A) {
            i = s.run(i, c * c);
        }
        let mut q = x;
        for (s, c) in self.b.iter_mut().zip(HILBERT_B) {
            q = s.run(q, c * c);
        }
        // Niemitalo: path A's output is the one delayed by a sample
        let id = self.bz;
        self.bz = i;
        (id, q)
    }
}

/// which sideband(s) the frequency shifter outputs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShiftOut {
    Up,
    Down,
    /// left = up, right = down (Bode's two outputs)
    Both,
}

/// realtime state of a Frequency shifter device (both channels)
#[derive(Clone, Copy, Default)]
pub struct FreqShifter {
    h: [Hilbert; 2],
    /// oscillator phase 0..1 (f64: no drift over long notes)
    ph: f64,
    fb: [f32; 2],
}

pub struct ShiftParams {
    /// total shift in Hz (coarse + fine), may be negative
    pub hz: f32,
    pub out: ShiftOut,
    pub feedback: f32,
    pub mix: f32,
}

impl FreqShifter {
    pub fn process(&mut self, l: f32, r: f32, p: &ShiftParams, sr: f32) -> (f32, f32) {
        self.ph = (self.ph + p.hz as f64 / sr as f64).rem_euclid(1.0);
        let w = self.ph * std::f64::consts::TAU;
        let (sn, cs) = (w.sin() as f32, w.cos() as f32);
        let mut out = [0.0f32; 2];
        for (c, (x, o)) in [l, r].into_iter().zip(out.iter_mut()).enumerate() {
            let xin = (x + p.feedback * self.fb[c]).clamp(-4.0, 4.0);
            let (i, q) = self.h[c].run(xin);
            // path B lags path A by 90°, so A·cos + B·sin is the upper sideband
            let up = i * cs + q * sn;
            let down = i * cs - q * sn;
            let y = match (p.out, c) {
                (ShiftOut::Up, _) | (ShiftOut::Both, 0) => up,
                _ => down,
            };
            self.fb[c] = y;
            *o = x * (1.0 - p.mix) + y * p.mix;
        }
        (out[0], out[1])
    }
}

/// Where the notches of an `n`-stage phaser with every stage at `fc` fall
/// (50 % mix, no feedback): the frequencies where the cascade's phase is an
/// odd multiple of 180°. In analog terms fc·tan((2k+1)·90°/n), k = 0..n/2;
/// here through the bilinear (TPT) frequency warping at sample rate `sr`.
#[cfg(test)]
pub fn notch_freqs(n: usize, fc: f32, sr: f32) -> impl Iterator<Item = f32> {
    let g = (PI * fc / sr).tan();
    (0..n / 2).map(move |k| {
        let t = ((2 * k + 1) as f32 * PI / (2 * n) as f32).tan();
        sr / PI * (g * t).atan()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;

    /// steady-state gain of a static phaser (depth 0) at `hz`
    fn gain(stages: usize, center: f32, mix: f32, fb: f32, hz: f32) -> f32 {
        let p = PhaserParams {
            stages,
            rate: 0.0,
            depth: 0.0,
            center,
            feedback: fb,
            stereo: 0.0,
            mix,
        };
        let mut ph = Phaser::default();
        let n = (SR * 0.5) as usize;
        let mut e = 0.0f32;
        let mut ein = 0.0f32;
        for i in 0..n {
            let x = (i as f32 * hz / SR * TAU).sin();
            let (y, _) = ph.process(x, x, &p, SR);
            if i > n / 2 {
                e += y * y;
                ein += x * x;
            }
        }
        (e / ein).sqrt()
    }

    #[test]
    fn notches_sit_where_the_all_pass_phase_is_180_degrees() {
        for (stages, center) in [(4, 1000.0), (6, 800.0), (8, 600.0), (12, 500.0)] {
            let notches: Vec<f32> = notch_freqs(stages, center, SR).collect();
            assert_eq!(notches.len(), stages / 2);
            for &f in notches.iter().filter(|f| **f < 15000.0) {
                let g = gain(stages, center, 0.5, 0.0, f);
                assert!(g < 0.03, "{stages} stages: notch at {f:.0} Hz, gain {g}");
            }
            // half-way (in log frequency) between the first two notches, the
            // wet and dry are in phase: no cut
            let mid = (notches[0] * notches[1]).sqrt();
            let g = gain(stages, center, 0.5, 0.0, mid);
            assert!(g > 0.9, "{stages} stages: peak at {mid:.0} Hz, gain {g}");
        }
    }

    #[test]
    fn all_pass_cascade_is_flat_and_feedback_sharpens() {
        // 100 % wet: an all-pass, so every frequency comes out at unity
        for hz in [60.0, 440.0, 3000.0, 12000.0] {
            let g = gain(8, 700.0, 1.0, 0.0, hz);
            assert!((g - 1.0).abs() < 0.02, "{hz}: {g}");
        }
        // feedback (wet = H / (1 - k H)) makes the peaks between the notches
        // resonant, and the notches stay deep dips
        let f0 = notch_freqs(4, 1000.0, SR).next().unwrap();
        let mid = (f0 * notch_freqs(4, 1000.0, SR).nth(1).unwrap()).sqrt();
        let peak = gain(4, 1000.0, 0.5, 0.7, mid);
        assert!(
            peak > gain(4, 1000.0, 0.5, 0.0, mid) * 1.1,
            "resonant peak {peak}"
        );
        let dip = gain(4, 1000.0, 0.5, 0.7, f0);
        assert!(peak / dip > 5.0, "notch {dip} vs peak {peak}");
        // negative feedback: the peaks fall and the notches widen
        assert!(gain(4, 1000.0, 0.5, -0.7, mid) < gain(4, 1000.0, 0.5, 0.0, mid));
    }

    /// amplitude of `hz` in `x` (single-bin DFT, Hann window against leakage)
    fn bin(x: &[f32], hz: f32) -> f32 {
        let (mut re, mut im, mut ws) = (0.0f64, 0.0f64, 0.0f64);
        let n = x.len() as f64;
        for (i, v) in x.iter().enumerate() {
            let win = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n).cos();
            let w = std::f64::consts::TAU * hz as f64 * i as f64 / SR as f64;
            re += *v as f64 * win * w.cos();
            im += *v as f64 * win * w.sin();
            ws += win;
        }
        ((re * re + im * im).sqrt() * 2.0 / ws) as f32
    }

    fn shift_sine(input: f32, hz: f32, out: ShiftOut) -> (Vec<f32>, Vec<f32>) {
        let mut fs = FreqShifter::default();
        let p = ShiftParams {
            hz,
            out,
            feedback: 0.0,
            mix: 1.0,
        };
        let n = SR as usize;
        let (mut l, mut r) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let x = (i as f32 * input / SR * TAU).sin() * 0.5;
            let (a, b) = fs.process(x, x, &p, SR);
            if i >= n / 4 {
                l.push(a);
                r.push(b);
            }
        }
        (l, r)
    }

    #[test]
    fn frequency_shifter_moves_a_sine_by_n_hz() {
        for (input, hz) in [
            (1000.0, 200.0),
            (440.0, -150.0),
            (3000.0, 2000.0),
            (2500.0, -2000.0),
            (220.0, 7.5),
            (8000.0, 1234.0),
        ] {
            let (l, _) = shift_sine(input, hz, ShiftOut::Up);
            let want = bin(&l, input + hz);
            let mirror = bin(&l, input - hz);
            let orig = bin(&l, input);
            assert!(
                (want - 0.5).abs() < 0.03,
                "{input} + {hz}: {want} at {} Hz",
                input + hz
            );
            // the other sideband and the carrier are at least 34 dB down
            assert!(mirror < want * 0.02, "{input} {hz}: mirror {mirror}");
            assert!(orig < want * 0.02, "{input} {hz}: carrier {orig}");
            // the measured peak sits within 1 Hz of input + shift
            let best = (-20..=20)
                .map(|k| input + hz + k as f32 * 0.25)
                .max_by(|a, b| bin(&l, *a).total_cmp(&bin(&l, *b)))
                .unwrap();
            assert!(
                (best - (input + hz)).abs() <= 1.0,
                "{input} {hz}: peak {best}"
            );
        }
    }

    #[test]
    fn frequency_shifter_down_and_both_outputs() {
        let (l, _) = shift_sine(1000.0, 300.0, ShiftOut::Down);
        assert!(bin(&l, 700.0) > 0.47 && bin(&l, 1300.0) < 0.01);
        let (l, r) = shift_sine(1000.0, 300.0, ShiftOut::Both);
        assert!(bin(&l, 1300.0) > 0.47 && bin(&l, 700.0) < 0.01, "left = up");
        assert!(
            bin(&r, 700.0) > 0.47 && bin(&r, 1300.0) < 0.01,
            "right = down"
        );
        // 0 Hz shift is (up to the all-pass phase) the input itself
        let (l, _) = shift_sine(1000.0, 0.0, ShiftOut::Up);
        assert!((bin(&l, 1000.0) - 0.5).abs() < 0.02);
    }

    #[test]
    fn frequency_shifter_feedback_adds_more_shifted_copies_and_stays_bounded() {
        let mut fs = FreqShifter::default();
        let p = ShiftParams {
            hz: 100.0,
            out: ShiftOut::Up,
            feedback: 0.9,
            mix: 1.0,
        };
        let mut l = vec![];
        for i in 0..(SR as usize) {
            let x = (i as f32 * 500.0 / SR * TAU).sin() * 0.5;
            let (a, _) = fs.process(x, x, &p, SR);
            assert!(a.is_finite() && a.abs() < 8.0);
            if i > 12000 {
                l.push(a);
            }
        }
        // 600, 700, 800 Hz: the spiral of re-shifted copies
        for hz in [600.0, 700.0, 800.0] {
            assert!(bin(&l, hz) > 0.1, "{hz}: {}", bin(&l, hz));
        }
    }

    #[test]
    fn the_lfo_sweeps_the_notch_and_stereo_offsets_the_right_channel() {
        let p = PhaserParams {
            stages: 4,
            rate: 1.0,
            depth: 1.0,
            center: 1000.0,
            feedback: 0.0,
            stereo: 180.0,
            mix: 0.5,
        };
        let mut ph = Phaser::default();
        // a fixed tone at the centre notch is cut only when the sweep passes
        let f0 = notch_freqs(4, 1000.0, SR).next().unwrap();
        let (mut lo, mut hi) = (f32::MAX, 0.0f32);
        let mut lr_diff = 0.0f32;
        let win = (SR * 0.02) as usize;
        let (mut el, mut er) = (0.0f32, 0.0f32);
        for i in 0..(SR as usize) {
            let x = (i as f32 * f0 / SR * TAU).sin();
            let (l, r) = ph.process(x, x, &p, SR);
            el += l * l;
            er += r * r;
            if i % win == win - 1 {
                let (a, b) = ((el / win as f32).sqrt(), (er / win as f32).sqrt());
                lo = lo.min(a);
                hi = hi.max(a);
                lr_diff = lr_diff.max((a - b).abs());
                el = 0.0;
                er = 0.0;
            }
        }
        assert!(lo < 0.15 && hi > 0.5, "swept: {lo} .. {hi}");
        assert!(lr_diff > 0.3, "the right LFO is 180° away: {lr_diff}");
    }
}
