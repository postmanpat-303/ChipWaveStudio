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
