//! Moog-style transistor ladder filter, zero-delay-feedback (TPT) form.
//!
//! Four topology-preserving-transform one-pole low-passes with global
//! feedback from the last stage, solved without a unit delay in the loop
//! (V. Zavalishin, *The Art of VA Filter Design*, Native Instruments 2012/2018,
//! chapter 5 "Ladder filter"). The five taps (input + four stages) are mixed
//! with binomial weights for LP/BP/HP at 12 or 24 dB/oct (the Oberheim
//! Xpander trick, as described by Välimäki & Huovilainen, "Oscillator and
//! filter algorithms for virtual analog synthesis", CMJ 30(2) 2006, and as
//! used by DaisySP's `LadderFilter`, MIT, R. van Hoesel / Infrasonic Audio).
//! Passband-gain compensation (feedback minus half the input) follows the
//! same DaisySP filter.
//!
//! * **Clean**: one tanh on the solved loop input (keeps self-oscillation
//!   bounded), linear stages.
//! * **Vintage**: tanh at the loop input and before every stage
//!   (Huovilainen, "Non-linear digital implementation of the Moog ladder
//!   filter", DAFx-04), run at 2x the sample rate through a 47-tap
//!   half-band FIR (Kaiser β = 8: −0.01 dB to 0.4·fs, −56 dB stop band).
//!
//! The response was cross-checked in the tests against ports of fundsp's
//! `moog` (S. Perttu, MIT/Apache-2.0; the Stilson/musicdsp model) and
//! DaisySP's ladder, and against the analytic 4-pole response
//! (ddiakopoulos/MoogLadders collects these models side by side).
//! All state is fixed-size: no allocation on the audio thread.
use crate::synth::FilterMode;
use serde::{Deserialize, Serialize};

/// which filter a patch uses
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterModel {
    /// the original 12 dB TPT state-variable filter
    #[default]
    Svf,
    /// ladder, linear stages
    Ladder,
    /// ladder, saturating stages, 2x oversampled
    Vintage,
}

impl FilterModel {
    pub const ALL: [FilterModel; 3] = [FilterModel::Svf, FilterModel::Ladder, FilterModel::Vintage];
    pub fn name(self) -> &'static str {
        match self {
            FilterModel::Svf => "SVF 12 dB",
            FilterModel::Ladder => "Ladder (clean)",
            FilterModel::Vintage => "Ladder (vintage)",
        }
    }
}

/// odd taps of the half-band FIR (centre tap 0.5, even taps 0)
const HB: [f32; 12] = [
    0.316_062_94,
    -0.099_534_58,
    0.053_239_6,
    -0.031_906_21,
    0.019_511_68,
    -0.011_685_38,
    0.006_670_848,
    -0.003_539_468,
    0.001_690_651,
    -0.000_690_003_6,
    0.000_214_604_3,
    -3.236_809e-5,
];

/// fast odd-symmetric tanh (Padé, exact sign, |x| > 3 clips to ±1)
#[inline]
fn tanh_p(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// 2x up/down sampler state (base-rate input history, 2x-rate output history)
#[derive(Clone, Copy)]
struct Os2 {
    up: [f32; 24],
    a: [f32; 13],
    b: [f32; 25],
}

impl Default for Os2 {
    fn default() -> Self {
        Self {
            up: [0.0; 24],
            a: [0.0; 13],
            b: [0.0; 25],
        }
    }
}

impl Os2 {
    /// two 2x-rate samples for input `x` (12 base samples of delay)
    #[inline]
    fn up(&mut self, x: f32) -> (f32, f32) {
        self.up.copy_within(0..23, 1);
        self.up[0] = x;
        // grid sample x[n-12] and the half-way sample between x[n-12] and x[n-11]
        let mut mid = 0.0;
        for (k, c) in HB.iter().enumerate() {
            mid += c * (self.up[12 + k] + self.up[11 - k]);
        }
        (self.up[12], 2.0 * mid)
    }
    /// one base-rate sample from two 2x-rate samples (12 more samples of delay)
    #[inline]
    fn down(&mut self, a: f32, b: f32) -> f32 {
        self.a.copy_within(0..12, 1);
        self.a[0] = a;
        self.b.copy_within(0..24, 1);
        self.b[0] = b;
        // centre a[n-12]; odd neighbours are the b samples around it
        let mut y = 0.5 * self.a[12];
        for (k, c) in HB.iter().enumerate() {
            y += c * (self.b[12 - k] + self.b[13 + k]);
        }
        y
    }
}

/// one ladder channel
#[derive(Clone, Copy, Default)]
pub struct Ladder {
    s: [f32; 4],
    os: Os2,
}

/// feedback at resonance 1 (k = 4 is the oscillation threshold)
pub const K_MAX: f32 = 4.2;

/// cached coefficients for one cutoff / resonance / sample rate
#[derive(Clone, Copy, Debug)]
pub struct Coefs {
    g_cap: f32,
    beta: f32,
    k: f32,
}

impl Coefs {
    /// `res` 0..1 (1 = self-oscillation); `rate` = the rate the stages run at
    pub fn new(cutoff: f32, res: f32, rate: f32, base_sr: f32) -> Self {
        let fc = if cutoff.is_finite() { cutoff } else { 1000.0 }.clamp(20.0, base_sr * 0.45);
        let g = (std::f32::consts::PI * fc / rate).tan();
        let r = if res.is_finite() { res } else { 0.0 };
        Self {
            g_cap: g / (1.0 + g),
            beta: 1.0 / (1.0 + g),
            // a little past 4 so the top of the knob self-oscillates at any pitch
            k: K_MAX * r.clamp(0.0, 1.0),
        }
    }
}

impl Ladder {
    /// one sample at the stage rate; returns the five taps (u, y1..y4)
    #[inline]
    fn step(&mut self, x: f32, c: &Coefs, vintage: bool) -> [f32; 5] {
        let (gc, k) = (c.g_cap, c.k);
        let s = &mut self.s;
        // y4 = G^4 u + S  (zero-delay solve of the feedback loop)
        let big_s = c.beta * (gc * gc * gc * s[0] + gc * gc * s[1] + gc * s[2] + s[3]);
        let g4 = gc * gc * gc * gc;
        let u_lin = (x * (1.0 + 0.5 * k) - k * big_s) / (1.0 + k * g4);
        // the loop saturates: bounded self-oscillation in both modes
        let u = if vintage {
            tanh_p(u_lin)
        } else {
            2.0 * tanh_p(u_lin * 0.5)
        };
        let mut taps = [u, 0.0, 0.0, 0.0, 0.0];
        let mut inp = u;
        for i in 0..4 {
            let xi = if vintage { tanh_p(inp) } else { inp };
            let v = (xi - s[i]) * gc;
            let y = v + s[i];
            s[i] = y + v;
            taps[i + 1] = y;
            inp = y;
        }
        taps
    }

    /// Filter one sample. `pole4` = 24 dB/oct (else 12 dB).
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        x: f32,
        cutoff: f32,
        res: f32,
        sr: f32,
        mode: FilterMode,
        pole4: bool,
        vintage: bool,
    ) -> f32 {
        let x = if x.is_finite() { x } else { 0.0 };
        let y = if vintage {
            let c = Coefs::new(cutoff, res, sr * 2.0, sr);
            let (a, b) = self.os.up(x);
            let ya = mix(&self.step(a, &c, true), mode, pole4);
            let yb = mix(&self.step(b, &c, true), mode, pole4);
            self.os.down(ya, yb)
        } else {
            let c = Coefs::new(cutoff, res, sr, sr);
            mix(&self.step(x, &c, false), mode, pole4)
        };
        if y.is_finite() {
            y
        } else {
            *self = Self::default();
            0.0
        }
    }
}

/// binomial tap weights (see the module docs); band-passes normalised to 0 dB peak
#[inline]
fn mix(t: &[f32; 5], mode: FilterMode, pole4: bool) -> f32 {
    match (mode, pole4) {
        (FilterMode::Lp, true) => t[4],
        (FilterMode::Lp, false) => t[2],
        (FilterMode::Bp, true) => 4.0 * (t[2] + t[4]) - 8.0 * t[3],
        (FilterMode::Bp, false) => 2.0 * (t[1] - t[2]),
        (FilterMode::Hp, true) => t[0] - 4.0 * t[1] + 6.0 * t[2] - 4.0 * t[3] + t[4],
        (FilterMode::Hp, false) => t[0] - 2.0 * t[1] + t[2],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 48000.0;

    /// steady-state gain (dB) of a sine at `f` through `filt`
    fn gain_db(f: f32, amp: f32, mut filt: impl FnMut(f32) -> f32) -> f32 {
        let n = (SR * 0.4) as usize;
        let (mut a, mut b) = (0.0f64, 0.0f64);
        for i in 0..n {
            let x = amp * (std::f32::consts::TAU * f * i as f32 / SR).sin();
            let y = filt(x);
            if i > n / 2 {
                a += (x * x) as f64;
                b += (y * y) as f64;
            }
        }
        (10.0 * (b / a).log10()) as f32
    }

    fn lad(fc: f32, res: f32, mode: FilterMode, p4: bool, v: bool) -> impl FnMut(f32) -> f32 {
        let mut l = Ladder::default();
        move |x| l.process(x, fc, res, SR, mode, p4, v)
    }

    #[test]
    fn lp24_matches_the_analytic_four_pole_response() {
        // linear 4-pole at res 0: |H(f)| = 1 / (1 + (f/fc)^2)^2
        for v in [false, true] {
            for f in [100.0, 500.0, 1000.0, 2000.0, 4000.0] {
                let want = -40.0 * (1.0 + (f / 1000.0f32).powi(2)).log10();
                let got = gain_db(f, 0.05, lad(1000.0, 0.0, FilterMode::Lp, true, v));
                assert!(
                    (got - want).abs() < 1.0,
                    "vintage {v} f {f}: {got:.2} vs {want:.2}"
                );
            }
        }
    }

    #[test]
    fn slopes_modes_and_resonance_peak() {
        let g = |f, m, p4| gain_db(f, 0.05, lad(1000.0, 0.0, m, p4, false));
        // two octaves up: 24 dB ~ -49 dB, 12 dB ~ -24.6 dB
        assert!(g(4000.0, FilterMode::Lp, true) < -45.0);
        let l12 = g(4000.0, FilterMode::Lp, false);
        assert!((-27.0..-22.0).contains(&l12), "lp12 {l12}");
        // high-pass mirrors it
        assert!(g(250.0, FilterMode::Hp, true) < -45.0);
        assert!(g(8000.0, FilterMode::Hp, true) > -1.0);
        let h12 = g(250.0, FilterMode::Hp, false);
        assert!((-27.0..-22.0).contains(&h12), "hp12 {h12}");
        // band-passes peak at the cutoff, about 0 dB
        for p4 in [true, false] {
            let pk = g(1000.0, FilterMode::Bp, p4);
            assert!(pk > -7.0 && pk < 1.0, "bp {p4} {pk}");
            assert!(g(150.0, FilterMode::Bp, p4) < pk - 9.0);
            assert!(g(6000.0, FilterMode::Bp, p4) < pk - 9.0);
        }
        // resonance: a peak at the cutoff, dc dips by (1 + k/2) / (1 + k)
        let r = |f| gain_db(f, 0.02, lad(1000.0, 0.85, FilterMode::Lp, true, false));
        assert!(
            r(1000.0) > r(100.0) + 6.0,
            "peak {} dc {}",
            r(1000.0),
            r(100.0)
        );
        let dc = r(50.0);
        let k = K_MAX * 0.85;
        let want = 20.0 * ((1.0 + 0.5 * k) / (1.0 + k)).log10();
        assert!((dc - want).abs() < 1.0, "dc {dc} want {want}");
    }

    #[test]
    fn self_oscillates_at_the_cutoff_and_stays_bounded() {
        for v in [false, true] {
            for fc in [220.0, 1000.0, 5000.0] {
                let mut l = Ladder::default();
                let mut out = vec![];
                for i in 0..(SR as usize) {
                    let x = if i == 0 { 1.0 } else { 0.0 };
                    out.push(l.process(x, fc, 1.0, SR, FilterMode::Lp, true, v));
                }
                let tail = &out[(SR as usize) / 2..];
                let peak = tail.iter().fold(0.0f32, |m, x| m.max(x.abs()));
                assert!(
                    peak > 0.05 && peak < 2.5,
                    "vintage {v} fc {fc}: peak {peak}"
                );
                let zc = tail
                    .windows(2)
                    .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                    .count();
                let f = zc as f32 * 2.0;
                assert!(
                    (f / fc - 1.0).abs() < 0.06,
                    "vintage {v} fc {fc}: rings at {f}"
                );
            }
        }
    }

    #[test]
    fn extremes_never_go_nan_or_blow_up() {
        let mut seed = 1u32;
        for v in [false, true] {
            for m in [FilterMode::Lp, FilterMode::Bp, FilterMode::Hp] {
                for p4 in [false, true] {
                    let mut l = Ladder::default();
                    for i in 0..20000 {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let x = (seed as f32 / u32::MAX as f32 - 0.5) * 40.0;
                        let fc = [5.0, 20.0, 23999.0, f32::NAN, 1e9][i % 5];
                        let res = [0.0, 1.0, 7.0, f32::NAN, -3.0][i / 7 % 5];
                        let x = if i == 777 { f32::NAN } else { x };
                        let y = l.process(x, fc, res, SR, m, p4, v);
                        assert!(y.is_finite() && y.abs() < 80.0, "{v} {m:?} {p4}: {y}");
                    }
                }
            }
        }
    }

    #[test]
    fn vintage_saturates_and_its_oversampler_is_transparent() {
        // a loud sine: the vintage ladder adds harmonics, the clean one hardly
        let thd = |v: bool| {
            let mut l = Ladder::default();
            let f = 200.0;
            let (mut fund, mut tot) = (0.0f64, 0.0f64);
            let (mut c, mut s) = (0.0f64, 0.0f64);
            let n = 48000;
            let mut ys = vec![];
            for i in 0..n {
                let x = 1.5 * (std::f32::consts::TAU * f * i as f32 / SR).sin();
                ys.push(l.process(x, 8000.0, 0.0, SR, FilterMode::Lp, true, v));
            }
            for (i, y) in ys.iter().enumerate().skip(n / 2) {
                let ph = std::f64::consts::TAU * f as f64 * i as f64 / SR as f64;
                c += *y as f64 * ph.cos();
                s += *y as f64 * ph.sin();
                tot += (*y as f64).powi(2);
            }
            let m = (n / 2) as f64;
            fund += 2.0 * (c * c + s * s) / (m * m);
            ((tot / m - fund).max(0.0) / fund).sqrt()
        };
        let (tc, tv) = (thd(false), thd(true));
        assert!(
            tv > 0.03 && tv > 3.0 * tc,
            "thd clean {tc:.4} vintage {tv:.4}"
        );
        // high-pass at 30 Hz: the 2x path passes 15 kHz flat (no top-end loss)
        let g = gain_db(15000.0, 0.05, lad(30.0, 0.0, FilterMode::Hp, true, true));
        assert!(g.abs() < 0.5, "vintage hp 15 kHz {g:.2} dB");
        let g = gain_db(1000.0, 0.05, lad(30.0, 0.0, FilterMode::Hp, true, true));
        assert!(g.abs() < 0.5, "vintage hp 1 kHz {g:.2} dB");
    }

    // ---- cross-checks against two independent open-source ladders ----

    /// fundsp `Moog` (Stilson / musicdsp model), MIT OR Apache-2.0, S. Perttu
    struct FundspMoog {
        p: f32,
        k: f32,
        rez: f32,
        s: [f32; 4],
        px: f32,
        ps: [f32; 3],
    }
    impl FundspMoog {
        fn new(cutoff: f32, q: f32) -> Self {
            let c = 2.0 * cutoff / SR;
            let p = c * (1.8 - 0.8 * c);
            let k = 2.0 * (c * std::f32::consts::PI * 0.5).sin() - 1.0;
            let t1 = (1.0 - p) * 1.386249;
            let t2 = 12.0 + t1 * t1;
            let rez = q * (t2 + 6.0 * t1) / (t2 - 6.0 * t1);
            Self {
                p,
                k,
                rez,
                s: [0.0; 4],
                px: 0.0,
                ps: [0.0; 3],
            }
        }
        fn tick(&mut self, x: f32) -> f32 {
            let x = -self.rez * self.s[3] + x;
            let (p, k) = (self.p, self.k);
            self.s[0] = (x + self.px) * p - k * self.s[0];
            self.s[1] = (self.s[0] + self.ps[0]) * p - k * self.s[1];
            self.s[2] = (self.s[1] + self.ps[1]) * p - k * self.s[2];
            self.s[3] = ((self.s[2] + self.ps[2]) * p - k * self.s[3]).tanh();
            self.px = x;
            self.ps = [self.s[0], self.s[1], self.s[2]];
            self.s[3]
        }
    }

    /// DaisySP `LadderFilter` (Huovilainen model by R. van Hoesel, MIT),
    /// LP24, 4x linear-interpolated oversampling as in the original
    struct Daisy {
        alpha: f32,
        k: f32,
        qadj: f32,
        z0: [f32; 4],
        z1: [f32; 4],
        old: f32,
    }
    impl Daisy {
        fn new(freq: f32, res: f32) -> Self {
            let wc = freq * std::f32::consts::TAU / (SR * 4.0);
            let wc2 = wc * wc;
            Self {
                alpha: 0.9892 * wc - 0.4324 * wc2 + 0.1381 * wc * wc2 - 0.0202 * wc2 * wc2,
                k: 4.0 * res,
                qadj: 1.006 + 0.0536 * wc - 0.095 * wc2 - 0.05 * wc2 * wc2,
                z0: [0.0; 4],
                z1: [0.0; 4],
                old: 0.0,
            }
        }
        fn lpf(&mut self, s: f32, i: usize) -> f32 {
            let mut ft = s * 0.769_230_8 + 0.230_769_23 * self.z0[i] - self.z1[i];
            ft = ft * self.alpha + self.z1[i];
            self.z1[i] = ft;
            self.z0[i] = s;
            ft
        }
        fn tick(&mut self, x: f32) -> f32 {
            let input = x * 0.5; // drive 0.5 as DaisySP's default
            let mut tot = 0.0;
            let mut interp = 0.0;
            for _ in 0..4 {
                let xi = interp * self.old + (1.0 - interp) * input;
                let u = xi - (self.z1[3] - 0.5 * xi) * self.k * self.qadj;
                let u = tanh_p(u);
                let a = self.lpf(u, 0);
                let b = self.lpf(a, 1);
                let c = self.lpf(b, 2);
                tot += self.lpf(c, 3) * 0.25;
                interp += 0.25;
            }
            self.old = input;
            tot
        }
    }

    #[test]
    fn response_agrees_with_fundsp_and_daisysp_ladders() {
        // normalise each filter to its own 50 Hz gain, then compare shapes
        let ours = |f, r| gain_db(f, 0.02, lad(1000.0, r, FilterMode::Lp, true, false));
        let fund = |f, r| {
            let mut m = FundspMoog::new(1000.0, r);
            gain_db(f, 0.02, move |x| m.tick(x))
        };
        let daisy = |f, r| {
            let mut d = Daisy::new(1000.0, r);
            gain_db(f, 0.02, move |x| d.tick(x))
        };
        for (name, other) in [
            ("fundsp", &fund as &dyn Fn(f32, f32) -> f32),
            ("daisysp", &daisy),
        ] {
            let (o0, t0) = (ours(50.0, 0.0), other(50.0, 0.0));
            for f in [250.0, 500.0, 1000.0, 2000.0] {
                let d = (ours(f, 0.0) - o0) - (other(f, 0.0) - t0);
                assert!(d.abs() < 3.0, "{name} res 0 at {f} Hz: {d:.2} dB apart");
            }
            // with resonance all three peak near the cutoff
            let peak = |g: &dyn Fn(f32, f32) -> f32| {
                [700.0, 800.0, 900.0, 1000.0, 1100.0, 1250.0]
                    .into_iter()
                    .map(|f| (f, g(f, 0.8)))
                    .fold((0.0, -1e9), |a, b| if b.1 > a.1 { b } else { a })
                    .0
            };
            let (po, pt) = (peak(&ours), peak(other));
            assert!(
                (po / pt - 1.0f32).abs() < 0.3,
                "{name}: resonant peak ours {po} vs {pt}"
            );
        }
    }
}
