//! Master bus: lookahead true-peak brickwall limiter and an ITU-R BS.1770
//! loudness meter (momentary / short-term / integrated LUFS + true peak).
//!
//! Design references (ideas and published formulas only, no code copied):
//! - ITU-R BS.1770-4 (2015), "Algorithms to measure audio programme loudness
//!   and true-peak audio level": K-weighting, 400 ms / 3 s windows, the
//!   -70 LUFS absolute and -10 LU relative gates, 4x oversampled true peak.
//!   https://www.itu.int/rec/R-REC-BS.1770
//! - EBU R 128 / Tech 3341 (momentary, short-term, integrated definitions).
//!   https://tech.ebu.ch/publications/r128
//! - K-weighting coefficients for any sample rate: the analog-matched
//!   biquad formulas popularised by libebur128 (Jan Kokemüller, MIT)
//!   https://github.com/jiixyj/libebur128 and pyloudnorm (Christian
//!   Steinmetz, MIT) https://github.com/csteinmetz1/pyloudnorm
//! - Lookahead limiter with a peak-hold + moving-average gain smoother:
//!   Geraint Luff, "Designing a straightforward limiter" (Signalsmith Audio)
//!   https://signalsmith-audio.co.uk/writing/2022/limiter/
//!
//! Everything is allocation-free after `new` (buffers sized from the sample
//! rate once, on the engine thread).
use serde::{Deserialize, Serialize};

/// Master limiter settings (saved in the song's mixer).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LimiterSettings {
    /// false = bypass: the classic tanh soft clipper of ChipWave <= 0.2.3.6
    pub enabled: bool,
    /// input gain in dB (0..24): drives the mix into the limiter
    pub gain_db: f32,
    /// output ceiling in dBTP (-12..0)
    pub ceiling_db: f32,
    /// release time in ms (10..1000)
    pub release_ms: f32,
    /// loudness target in LUFS for the guidance text (0 = off)
    pub target_lufs: f32,
}

impl Default for LimiterSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            gain_db: 0.0,
            ceiling_db: -1.0,
            release_ms: 80.0,
            target_lufs: -14.0,
        }
    }
}

impl LimiterSettings {
    pub fn fix(&mut self) {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.gain_db = f(self.gain_db, 0.0, 24.0, 0.0);
        self.ceiling_db = f(self.ceiling_db, -12.0, 0.0, -1.0);
        self.release_ms = f(self.release_ms, 10.0, 1000.0, 80.0);
        self.target_lufs = f(self.target_lufs, -30.0, 0.0, -14.0);
    }
}

/// Loudness targets offered in the mixer (LUFS, label).
pub const TARGETS: [(f32, &str); 5] = [
    (0.0, "off"),
    (-14.0, "streaming (−14)"),
    (-16.0, "podcast / Apple (−16)"),
    (-9.0, "club / loud (−9)"),
    (-23.0, "broadcast EBU R128 (−23)"),
];

/// Guidance text for an integrated loudness against a target.
pub fn guidance(integrated: f32, target: f32) -> String {
    if target == 0.0 || !integrated.is_finite() {
        return String::new();
    }
    let d = integrated - target;
    if d.abs() < 1.0 {
        format!("on target ({target:.0} LUFS)")
    } else if d > 0.0 {
        format!("{d:.1} LU above {target:.0}: services will turn it down")
    } else {
        format!("{:.1} LU below {target:.0}: raise the limiter gain", -d)
    }
}

pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn lin_to_db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

// ---------------------------------------------------------------- true peak

const TP_HALF: usize = 8; // input samples per side of the kernel
const TP_HIST: usize = 2 * TP_HALF; // taps per phase
const TP_TAPS: usize = 4 * TP_HIST + 1; // 4x Kaiser-windowed sinc

fn bessel_i0(x: f64) -> f64 {
    let (mut s, mut t) = (1.0, 1.0);
    for k in 1..32 {
        t *= (x / 2.0) / k as f64;
        s += t * t;
    }
    s
}

/// 4x polyphase interpolation kernel: phase k uses h[k + 4j].
fn tp_kernel() -> [[f32; TP_HIST]; 4] {
    let mut h = [0.0f64; TP_TAPS];
    let mid = (TP_TAPS / 2) as f64;
    let beta = 6.0;
    for (n, v) in h.iter_mut().enumerate() {
        let x = (n as f64 - mid) / 4.0;
        let sinc = if x == 0.0 {
            1.0
        } else {
            (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
        };
        let r = (n as f64 - mid) / mid;
        let win = bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(beta);
        *v = sinc * win;
    }
    let mut out = [[0.0f32; TP_HIST]; 4];
    for (k, ph) in out.iter_mut().enumerate() {
        let sum: f64 = (0..TP_HIST).filter_map(|j| h.get(k + 4 * j)).sum();
        for (j, c) in ph.iter_mut().enumerate() {
            *c = (h.get(k + 4 * j).copied().unwrap_or(0.0) / sum) as f32;
        }
    }
    out
}

/// Per-channel 4x oversampled peak detector (BS.1770-4 Annex 2 style).
/// `push` returns the largest absolute value of the 4 interpolated points
/// for positions t-8 .. t-7.25 (`DELAY` samples of delay).
#[derive(Clone)]
pub struct TruePeak {
    k: [[f32; TP_HIST]; 4],
    hist: [f32; TP_HIST],
    w: usize,
}

impl Default for TruePeak {
    fn default() -> Self {
        Self {
            k: tp_kernel(),
            hist: [0.0; TP_HIST],
            w: 0,
        }
    }
}

impl TruePeak {
    pub const DELAY: usize = TP_HALF;

    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        self.w = (self.w + 1) % TP_HIST;
        self.hist[self.w] = x;
        let mut m = 0.0f32;
        for ph in &self.k {
            let mut y = 0.0;
            for (j, c) in ph.iter().enumerate() {
                y += c * self.hist[(self.w + TP_HIST - j) % TP_HIST];
            }
            m = m.max(y.abs());
        }
        m
    }
}

/// True peak (linear) of interleaved stereo, 4x oversampled.
pub fn true_peak(d: &[f32]) -> f32 {
    let mut tp = [TruePeak::default(), TruePeak::default()];
    let mut m = 0.0f32;
    for f in d.chunks(2) {
        for (c, x) in f.iter().enumerate() {
            m = m.max(tp[c].push(*x));
        }
    }
    // flush the filter delay
    for _ in 0..TP_HIST {
        m = m.max(tp[0].push(0.0)).max(tp[1].push(0.0));
    }
    m
}

// ---------------------------------------------------------------- limiter

/// Stereo-linked lookahead true-peak limiter. Latency: `latency()` frames.
pub struct Limiter {
    sr: f32,
    tp: [TruePeak; 2],
    /// previous true-peak estimate (the peak span of a sample reaches into
    /// the neighbouring interpolation intervals)
    prev_tp: f32,
    /// audio delay line (interleaved L/R)
    dl: Vec<f32>,
    dw: usize,
    delay: usize,
    /// required-gain history for the peak hold
    hold: Vec<f32>,
    hw: usize,
    /// moving-average of the hold output
    boxb: Vec<f32>,
    bw: usize,
    box_sum: f64,
    box_n: usize,
    renorm: u32,
    env: f32,
    /// last applied gain (for the gain-reduction meter)
    pub gain: f32,
    /// smallest gain since `take_min_gain`
    min_gain: f32,
}

impl Limiter {
    /// lookahead (= attack) time in seconds
    pub const LOOKAHEAD: f32 = 0.0015;

    pub fn new(sr: f32) -> Self {
        let n = ((Self::LOOKAHEAD * sr) as usize).max(4);
        let delay = n + TruePeak::DELAY;
        Self {
            sr,
            tp: Default::default(),
            prev_tp: 0.0,
            dl: vec![0.0; (delay + 1) * 2],
            dw: 0,
            delay,
            hold: vec![1.0; n + 2],
            hw: 0,
            boxb: vec![1.0; n],
            bw: 0,
            box_sum: n as f64,
            box_n: n,
            renorm: 0,
            env: 1.0,
            gain: 1.0,
            min_gain: 1.0,
        }
    }

    /// frames of delay the limiter adds
    #[cfg(test)]
    pub fn latency(&self) -> usize {
        self.delay
    }

    pub fn take_min_gain(&mut self) -> f32 {
        std::mem::replace(&mut self.min_gain, 1.0)
    }

    /// One stereo frame. `ceiling` linear (true peak), `release_s` seconds.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32, ceiling: f32, release_s: f32) -> (f32, f32) {
        let (l, r) = (
            if l.is_finite() { l } else { 0.0 },
            if r.is_finite() { r } else { 0.0 },
        );
        // detection: true peak of the input (0.15 dB internal margin)
        let t = self.tp[0].push(l).max(self.tp[1].push(r));
        let peak = t.max(self.prev_tp);
        self.prev_tp = t;
        let target = ceiling * 0.983;
        let gr = if peak > target { target / peak } else { 1.0 };
        // instant attack, exponential release
        let k = 1.0 - (-1.0 / (release_s.max(0.001) * self.sr)).exp();
        self.env = (self.env + (1.0 - self.env) * k).min(gr);
        // peak hold over lookahead + 2 frames
        self.hw = (self.hw + 1) % self.hold.len();
        self.hold[self.hw] = self.env;
        let mut h = 1.0f32;
        for v in &self.hold {
            h = h.min(*v);
        }
        // moving average over the lookahead: a smooth ramp that reaches the
        // held gain exactly when the peak leaves the delay line
        self.box_sum += h as f64 - self.boxb[self.bw] as f64;
        self.boxb[self.bw] = h;
        self.bw = (self.bw + 1) % self.box_n;
        self.renorm += 1;
        if self.renorm >= 8192 {
            self.renorm = 0;
            self.box_sum = self.boxb.iter().map(|&x| x as f64).sum();
        }
        let g = ((self.box_sum / self.box_n as f64) as f32).min(1.0);
        // delayed audio
        let n = self.dl.len() / 2;
        self.dl[self.dw * 2] = l;
        self.dl[self.dw * 2 + 1] = r;
        let rd = (self.dw + n - self.delay) % n;
        self.dw = (self.dw + 1) % n;
        let (dl, dr) = (self.dl[rd * 2], self.dl[rd * 2 + 1]);
        self.gain = g;
        self.min_gain = self.min_gain.min(g);
        // sample-peak safety net (never active on band-limited material)
        (
            (dl * g).clamp(-ceiling, ceiling),
            (dr * g).clamp(-ceiling, ceiling),
        )
    }
}

// ---------------------------------------------------------------- loudness

#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}

impl Biquad {
    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        // transposed direct form II
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// BS.1770 K-weighting (pre-filter shelf + RLB high-pass) for `sr`.
fn k_weighting(sr: f64) -> [Biquad; 2] {
    use std::f64::consts::PI;
    // stage 1: high shelf, +4 dB above ~1.7 kHz
    let (f0, g, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (PI * f0 / sr).tan();
    let vh = 10f64.powf(g / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad {
        b: [
            (vh + vb * k / q + k * k) / a0,
            2.0 * (k * k - vh) / a0,
            (vh - vb * k / q + k * k) / a0,
        ],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        z: [0.0; 2],
    };
    // stage 2: RLB high-pass at ~38 Hz
    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (PI * f0 / sr).tan();
    let a0 = 1.0 + k / q + k * k;
    let hp = Biquad {
        b: [1.0, -2.0, 1.0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        z: [0.0; 2],
    };
    [shelf, hp]
}

const HIST_MIN: f32 = -70.0;
const HIST_BINS: usize = 800; // 0.1 LU bins, -70 .. +10 LUFS
const SUB: usize = 30; // 100 ms sub-blocks kept (3 s)

/// Readout for the UI (LUFS values are -inf until measured).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Readout {
    pub momentary: f32,
    pub short_term: f32,
    pub integrated: f32,
    /// max true peak since reset, dBTP
    pub true_peak: f32,
    /// current limiter gain reduction in dB (<= 0)
    pub gain_red: f32,
}

impl Default for Readout {
    fn default() -> Self {
        Self {
            momentary: f32::NEG_INFINITY,
            short_term: f32::NEG_INFINITY,
            integrated: f32::NEG_INFINITY,
            true_peak: f32::NEG_INFINITY,
            gain_red: 0.0,
        }
    }
}

fn lufs(ms: f64) -> f32 {
    if ms <= 1e-12 {
        f32::NEG_INFINITY
    } else {
        (-0.691 + 10.0 * ms.log10()) as f32
    }
}

/// Streaming BS.1770 meter. Integrated loudness uses a 0.1 LU histogram
/// of the 400 ms gating blocks (75 % overlap), so it never allocates.
pub struct Loudness {
    kw: [[Biquad; 2]; 2],
    tp: [TruePeak; 2],
    sub_len: usize,
    acc: f64,
    acc_n: usize,
    subs: [f64; SUB],
    sub_w: usize,
    sub_count: usize,
    hist_n: Box<[u32; HIST_BINS]>,
    hist_e: Box<[f64; HIST_BINS]>,
    pub out: Readout,
    tp_max: f32,
}

impl Loudness {
    pub fn new(sr: f32) -> Self {
        let kw = k_weighting(sr as f64);
        Self {
            kw: [kw, kw],
            tp: Default::default(),
            sub_len: ((sr * 0.1) as usize).max(1),
            acc: 0.0,
            acc_n: 0,
            subs: [0.0; SUB],
            sub_w: 0,
            sub_count: 0,
            hist_n: Box::new([0; HIST_BINS]),
            hist_e: Box::new([0.0; HIST_BINS]),
            out: Readout::default(),
            tp_max: 0.0,
        }
    }

    /// Clear the integrated measurement and the true-peak hold.
    pub fn reset(&mut self) {
        self.hist_n.fill(0);
        self.hist_e.fill(0.0);
        self.tp_max = 0.0;
        self.out.integrated = f32::NEG_INFINITY;
        self.out.true_peak = f32::NEG_INFINITY;
    }

    /// Feed one output frame. `gate` = count it in the integrated value.
    #[inline]
    pub fn push(&mut self, l: f32, r: f32, gate: bool) {
        self.tp_max = self.tp_max.max(self.tp[0].push(l)).max(self.tp[1].push(r));
        let mut e = 0.0;
        for (c, x) in [l, r].into_iter().enumerate() {
            let s1 = self.kw[c][0].run(x as f64);
            let y = self.kw[c][1].run(s1);
            e += y * y;
        }
        self.acc += e;
        self.acc_n += 1;
        if self.acc_n >= self.sub_len {
            self.subs[self.sub_w] = self.acc / self.acc_n as f64;
            self.sub_w = (self.sub_w + 1) % SUB;
            self.sub_count = (self.sub_count + 1).min(SUB);
            self.acc = 0.0;
            self.acc_n = 0;
            self.update(gate);
        }
    }

    fn mean_last(&self, n: usize) -> f64 {
        let n = n.min(self.sub_count).max(1);
        let mut s = 0.0;
        for i in 0..n {
            s += self.subs[(self.sub_w + SUB - 1 - i) % SUB];
        }
        s / n as f64
    }

    fn update(&mut self, gate: bool) {
        let m = self.mean_last(4);
        self.out.momentary = lufs(m);
        self.out.short_term = lufs(self.mean_last(30));
        self.out.true_peak = lin_to_db(self.tp_max);
        if gate && self.sub_count >= 4 {
            let l = self.out.momentary;
            if l > HIST_MIN {
                let b = (((l - HIST_MIN) * 10.0) as usize).min(HIST_BINS - 1);
                self.hist_n[b] += 1;
                self.hist_e[b] += m;
            }
            self.out.integrated = self.integrated();
        }
    }

    /// gated integrated loudness from the histogram
    pub fn integrated(&self) -> f32 {
        let (n, e): (u64, f64) = self
            .hist_n
            .iter()
            .zip(self.hist_e.iter())
            .fold((0, 0.0), |(n, e), (a, b)| (n + *a as u64, e + b));
        if n == 0 {
            return f32::NEG_INFINITY;
        }
        let rel = lufs(e / n as f64) - 10.0;
        let first = (((rel - HIST_MIN) * 10.0).max(0.0) as usize).min(HIST_BINS);
        let (n, e) = (first..HIST_BINS).fold((0u64, 0.0f64), |(n, e), b| {
            (n + self.hist_n[b] as u64, e + self.hist_e[b])
        });
        if n == 0 {
            f32::NEG_INFINITY
        } else {
            lufs(e / n as f64)
        }
    }
}

/// Integrated loudness of interleaved stereo (offline helper).
pub fn integrated_lufs(d: &[f32], sr: f32) -> f32 {
    let mut m = Loudness::new(sr);
    for f in d.chunks(2) {
        m.push(f[0], *f.get(1).unwrap_or(&f[0]), true);
    }
    m.integrated()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, amp: f32, secs: f32, sr: f32) -> Vec<f32> {
        (0..(secs * sr) as usize)
            .flat_map(|i| {
                let v = (i as f32 * hz / sr * std::f32::consts::TAU).sin() * amp;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn true_peak_sees_intersample_overs() {
        // fs/4 sine at 45 degrees: samples peak at 0.707, the wave at 1.0
        let d: Vec<f32> = (0..4800)
            .flat_map(|i| {
                let v =
                    (i as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4).sin();
                [v, v]
            })
            .collect();
        let sp = d.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let tp = true_peak(&d);
        assert!(sp < 0.71, "{sp}");
        assert!((tp - 1.0).abs() < 0.02, "true peak {tp}");
    }

    #[test]
    fn k_weighted_1k_sine_reads_minus_3_lufs_per_channel_pair() {
        // BS.1770: a 0 dBFS 997 Hz sine on L+R reads about -0.69+3+0.69 =
        // +0.0 ... (each channel -3.01 dB mean square, summed: 0 dB) -> ~ -0.0
        for sr in [44100.0, 48000.0, 96000.0] {
            let d = sine(997.0, 1.0, 5.0, sr);
            let l = integrated_lufs(&d, sr);
            assert!((l - 0.0).abs() < 0.25, "{sr}: {l}");
            let d = sine(997.0, 0.1, 5.0, sr);
            let l = integrated_lufs(&d, sr);
            assert!((l + 20.0).abs() < 0.25, "{sr}: {l}");
        }
    }

    #[test]
    fn gating_ignores_silence() {
        let sr = 48000.0;
        let mut d = sine(997.0, 0.1, 4.0, sr);
        d.extend(std::iter::repeat_n(0.0, (sr * 8.0) as usize * 2));
        let l = integrated_lufs(&d, sr);
        assert!((l + 20.0).abs() < 0.3, "{l}");
    }

    fn hot_material(sr: f32, secs: f32) -> Vec<f32> {
        // saw chords + noise bursts + square kick, peaks around +12 dBFS
        let mut n = 0x1234_5678u32;
        (0..(secs * sr) as usize)
            .flat_map(|i| {
                let t = i as f32 / sr;
                let saw = |f: f32| 2.0 * (t * f).fract() - 1.0;
                n ^= n << 13;
                n ^= n >> 17;
                n ^= n << 5;
                let noise = n as f32 / u32::MAX as f32 * 2.0 - 1.0;
                let beat = (t * 2.3).fract();
                let kick = if beat < 0.15 {
                    (t * 55.0 * std::f32::consts::TAU).sin() * 3.0
                } else {
                    0.0
                };
                let burst = if (t * 7.0).fract() < 0.05 {
                    noise * 2.5
                } else {
                    0.0
                };
                // 10 ms fade-in: a buffer that starts with a step has a
                // true over at the step that no lookahead can see coming
                let fade = (t / 0.01).min(1.0);
                let l = saw(110.0) + saw(164.8) * 0.8 + kick + burst;
                let r = saw(110.5) + saw(220.0) * 0.7 + kick - burst;
                [l * fade, r * fade]
            })
            .collect()
    }

    #[test]
    fn limiter_true_peak_never_exceeds_the_ceiling() {
        for sr in [44100.0f32, 48000.0] {
            for (ceil_db, gain_db) in [(-1.0, 0.0), (-1.0, 12.0), (-0.1, 6.0), (-6.0, 18.0)] {
                let mut lim = Limiter::new(sr);
                let ceiling = db_to_lin(ceil_db);
                let g = db_to_lin(gain_db);
                let out: Vec<f32> = hot_material(sr, 3.0)
                    .chunks(2)
                    .flat_map(|f| {
                        let (a, b) = lim.process(f[0] * g, f[1] * g, ceiling, 0.05);
                        [a, b]
                    })
                    .collect();
                let tp = lin_to_db(true_peak(&out));
                assert!(
                    tp <= ceil_db + 1e-3,
                    "{sr} {ceil_db} {gain_db}: {tp:.3} dBTP"
                );
                // and it is still loud (not just silenced)
                assert!(tp > ceil_db - 1.0, "{tp}");
            }
        }
    }

    /// independent reference: 16x oversampling with a long Kaiser sinc,
    /// interior only (a buffer that starts or stops abruptly rings at its
    /// own edges, which is not the signal's true peak)
    fn true_peak_ref(d: &[f32]) -> f32 {
        let ch: Vec<Vec<f64>> = (0..2)
            .map(|c| d.iter().skip(c).step_by(2).map(|&x| x as f64).collect())
            .collect();
        let half = 32i64;
        let bessel = |x: f64| {
            let (mut s, mut t) = (1.0, 1.0);
            for k in 1..30 {
                t *= (x / 2.0) / k as f64;
                s += t * t;
            }
            s
        };
        let beta = 9.0;
        let mut m = 0.0f64;
        for x in &ch {
            for i in half..x.len() as i64 - half {
                for ph in 0..16 {
                    let pos = i as f64 + ph as f64 / 16.0;
                    let mut y = 0.0;
                    for j in (i - half + 1)..=(i + half) {
                        if j < 0 || j >= x.len() as i64 {
                            continue;
                        }
                        let t = pos - j as f64;
                        let sinc = if t.abs() < 1e-12 {
                            1.0
                        } else {
                            (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t)
                        };
                        let r = t / half as f64;
                        let w = if r.abs() >= 1.0 {
                            0.0
                        } else {
                            bessel(beta * (1.0 - r * r).sqrt()) / bessel(beta)
                        };
                        y += x[j as usize] * sinc * w;
                    }
                    m = m.max(y.abs());
                }
            }
        }
        m as f32
    }

    /// band-limited hot material: additive saws (harmonics below 0.42 fs),
    /// a sine kick and chord stabs, peaks around +12 dBFS
    fn hot_bandlimited(sr: f32, secs: f32) -> Vec<f32> {
        let saw = |t: f32, f: f32| {
            let mut v = 0.0;
            let mut k = 1.0;
            while k * f < sr * 0.42 {
                v += (t * f * k * std::f32::consts::TAU).sin() / k;
                k += 1.0;
            }
            v * 0.6
        };
        (0..(secs * sr) as usize)
            .flat_map(|i| {
                let t = i as f32 / sr;
                let fade = (t / 0.01).min(1.0);
                let beat = (t * 2.3).fract();
                let kick = (t * 55.0 * std::f32::consts::TAU).sin() * 3.0 * (-beat * 12.0).exp();
                let stab = if (t * 4.0).fract() < 0.3 { 1.5 } else { 0.3 };
                let l = (saw(t, 110.0) + saw(t, 164.8)) * stab + kick;
                let r = (saw(t, 110.5) + saw(t, 220.0)) * stab + kick;
                [l * fade, r * fade]
            })
            .collect()
    }

    #[test]
    fn limiter_output_passes_an_independent_16x_check() {
        // the 4x detector is the BS.1770 definition; on band-limited material
        // an independent 16x / 64-tap reconstruction must agree with it
        let sr = 48000.0;
        for gain_db in [6.0, 12.0, 18.0] {
            let mut lim = Limiter::new(sr);
            let g = db_to_lin(gain_db);
            let out: Vec<f32> = hot_bandlimited(sr, 0.6)
                .chunks(2)
                .flat_map(|f| {
                    let (a, b) = lim.process(f[0] * g, f[1] * g, db_to_lin(-1.0), 0.05);
                    [a, b]
                })
                .collect();
            let tp = lin_to_db(true_peak_ref(&out));
            assert!(tp <= -1.0 + 0.02, "{gain_db} dB in: {tp:.3} dBTP");
            assert!(tp > -2.0, "{tp:.3}");
        }
    }

    #[test]
    fn limiter_is_transparent_below_the_ceiling() {
        let sr = 48000.0;
        let mut lim = Limiter::new(sr);
        let d = sine(440.0, 0.5, 1.0, sr);
        let lat = lim.latency();
        let out: Vec<f32> = d
            .chunks(2)
            .flat_map(|f| {
                let (a, b) = lim.process(f[0], f[1], db_to_lin(-1.0), 0.08);
                [a, b]
            })
            .collect();
        for i in 1000..d.len() / 2 - lat {
            assert!((out[(i + lat) * 2] - d[i * 2]).abs() < 1e-6);
        }
    }

    #[test]
    fn limiter_and_meter_do_not_allocate() {
        let mut lim = Limiter::new(48000.0);
        let mut m = Loudness::new(48000.0);
        let d = hot_material(48000.0, 0.5);
        let n = crate::engine::alloc_count::count(|| {
            for f in d.chunks(2) {
                let (a, b) = lim.process(f[0], f[1], 0.89, 0.08);
                m.push(a, b, true);
            }
        });
        assert_eq!(n, 0);
    }

    #[test]
    fn settings_default_and_fix() {
        let s: LimiterSettings = toml::from_str("").unwrap();
        assert_eq!(s, LimiterSettings::default());
        let mut s = LimiterSettings {
            ceiling_db: 5.0,
            release_ms: f32::NAN,
            ..Default::default()
        };
        s.fix();
        assert_eq!(s.ceiling_db, 0.0);
        assert_eq!(s.release_ms, 80.0);
        assert!(guidance(-10.0, -14.0).contains("above"));
        assert!(guidance(-14.3, -14.0).contains("on target"));
    }
}
