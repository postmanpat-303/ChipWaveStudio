//! Poly synth building blocks: band-limited oscillator stacks, blend
//! operators, filters, shaper and envelopes. All `Copy`, fixed size, no
//! allocation.
use super::{BlendOp, PolyFilter, ShaperMode, MAX_UNISON};
use crate::ladder::tanh_p;
use crate::synth::Svf;

/// oscillator shape weights (saw, pulse, octave saw) for Shape -1..1:
/// pulse at -1, saw at 0, octave-up saw at +1, crossfading in between
#[inline]
pub fn weights(shape: f32) -> (f32, f32, f32) {
    let s = shape.clamp(-1.0, 1.0);
    if s < 0.0 {
        (1.0 + s, -s, 0.0)
    } else {
        (1.0 - s, 0.0, s)
    }
}

/// the naive (aliasing) waveform at phase `s` 0..1
#[inline]
pub fn wave(s: f32, pw: f32, w: (f32, f32, f32)) -> f32 {
    let (ws, wp, wo) = w;
    let saw = 2.0 * s - 1.0;
    if s < pw {
        ws * saw + wp + wo * (2.0 * s / pw - 1.0)
    } else {
        ws * saw - wp + wo * (2.0 * (s - pw) / (1.0 - pw) - 1.0)
    }
}

/// step heights at the pulse-width edge and at the wrap
#[inline]
fn jumps(w: (f32, f32, f32)) -> (f32, f32) {
    let (ws, wp, wo) = w;
    (-2.0 * wp - 2.0 * wo, -2.0 * ws + 2.0 * wp - 2.0 * wo)
}

/// 2-point polyBLEP corrections of a step `j` that happened `f` samples
/// (0..1) before the current sample: (to the previous sample, to this one)
#[inline]
pub fn blep(j: f32, f: f32) -> (f32, f32) {
    let h = 0.5 * j;
    (h * f * f, -h * (1.0 - f) * (1.0 - f))
}

/// settings of one oscillator for one block (control rate)
#[derive(Clone, Copy, Default)]
pub struct OscSet {
    /// phase increment of the oscillator pitch, per sample
    pub dt: f32,
    /// slave/master ratio for hard sync (1 = off)
    pub sync: f32,
    pub pw: f32,
    pub w: (f32, f32, f32),
    pub sub: f32,
    pub sub_pw: f32,
    pub n: usize,
    /// per unison voice: pitch ratio, left and right gain
    pub ratio: [f32; MAX_UNISON],
    pub gl: [f32; MAX_UNISON],
    pub gr: [f32; MAX_UNISON],
    /// sub oscillator pan gains
    pub sl: f32,
    pub sr: f32,
}

/// one oscillator: up to 16 unison saws/pulses (each with a hidden master
/// for hard sync) and a sub pulse, band-limited with one sample of delay
#[derive(Clone, Copy)]
pub struct Osc {
    ph: [f32; MAX_UNISON],
    mph: [f32; MAX_UNISON],
    sub: f32,
    /// the delayed output (previous sample with its corrections so far)
    dl: f32,
    dr: f32,
}

impl Default for Osc {
    fn default() -> Self {
        Osc {
            ph: [0.0; MAX_UNISON],
            mph: [0.0; MAX_UNISON],
            sub: 0.0,
            dl: 0.0,
            dr: 0.0,
        }
    }
}

impl Osc {
    /// start phases: all zero (reset) or spread by the golden ratio from `seed`
    pub fn start(&mut self, reset: bool, seed: u32) {
        let off = (seed >> 8) as f32 / (1u32 << 24) as f32;
        for i in 0..MAX_UNISON {
            let p = if reset {
                0.0
            } else {
                (off + i as f32 * 0.618_034).fract()
            };
            self.ph[i] = p;
            self.mph[i] = p;
        }
        self.sub = if reset { 0.0 } else { off };
    }

    /// one sample (stereo), delayed by one sample
    #[inline]
    pub fn next(&mut self, o: &OscSet) -> (f32, f32) {
        let (jpw, jwrap) = jumps(o.w);
        let pw = o.pw;
        let mut nl = 0.0;
        let mut nr = 0.0;
        let mut cl = 0.0; // corrections to the delayed sample
        let mut cr = 0.0;
        let sync = o.sync > 1.0001;
        for i in 0..o.n {
            let dm = (o.dt * o.ratio[i]).min(0.5);
            let ds = (dm * o.sync).min(0.5);
            let mut s = self.ph[i];
            let mut cn = 0.0; // correction to this sample (gain 1)
            let mut cp = 0.0;
            // time (0..1 of this sample) of the master wrap, if any
            let mut tm = 2.0;
            if sync {
                let m = self.mph[i] + dm;
                if m >= 1.0 {
                    tm = (1.0 - self.mph[i]) / dm;
                    self.mph[i] = m - 1.0;
                } else {
                    self.mph[i] = m;
                }
            }
            let mut t = 0.0;
            if ds <= 0.0 {
                tm = 2.0;
            }
            // at most a few edges per sample (ds <= 0.5)
            for _ in 0..6 {
                let end = if tm <= 1.0 { tm } else { 1.0 };
                let (edge, j) = if s < pw { (pw, jpw) } else { (1.0, jwrap) };
                let te = if ds > 0.0 { t + (edge - s) / ds } else { 9.0 };
                if te <= end {
                    let (a, b) = blep(j, 1.0 - te);
                    cp += a;
                    cn += b;
                    t = te;
                    s = if edge >= 1.0 { 0.0 } else { pw };
                    continue;
                }
                s += ds * (end - t);
                t = end;
                if tm <= 1.0 {
                    // hard sync: restart at the master wrap
                    let j = wave(0.0, pw, o.w) - wave(s, pw, o.w);
                    let (a, b) = blep(j, 1.0 - tm);
                    cp += a;
                    cn += b;
                    s = 0.0;
                    tm = 2.0;
                    continue;
                }
                break;
            }
            self.ph[i] = s;
            let v = wave(s, pw, o.w) + cn;
            nl += o.gl[i] * v;
            nr += o.gr[i] * v;
            cl += o.gl[i] * cp;
            cr += o.gr[i] * cp;
        }
        // the sub: a pulse one octave down, not synced
        if o.sub > 0.0 {
            let d = (o.dt * 0.5).min(0.5);
            let mut s = self.sub + d;
            let mut v_c = 0.0;
            let mut v_p = 0.0;
            let was = self.sub;
            if was < o.sub_pw && s >= o.sub_pw {
                let (a, b) = blep(-2.0, (s - o.sub_pw) / d);
                v_p += a;
                v_c += b;
            }
            if s >= 1.0 {
                s -= 1.0;
                let (a, b) = blep(2.0, s / d);
                v_p += a;
                v_c += b;
                if s >= o.sub_pw {
                    let (a, b) = blep(-2.0, (s - o.sub_pw) / d);
                    v_p += a;
                    v_c += b;
                }
            }
            self.sub = s;
            let v = (if s < o.sub_pw { 1.0 } else { -1.0 }) + v_c;
            nl += o.sub * o.sl * v;
            nr += o.sub * o.sr * v;
            cl += o.sub * o.sl * v_p;
            cr += o.sub * o.sr * v_p;
        } else {
            self.sub = (self.sub + o.dt * 0.5).fract();
        }
        let out = (self.dl + cl, self.dr + cr);
        self.dl = nl;
        self.dr = nr;
        out
    }
}

/// blend oscillator 1 (`a`) with oscillator 2 (`b`), amount `m` 0..1
#[inline]
pub fn blend(op: BlendOp, a: f32, b: f32, m: f32) -> f32 {
    match op {
        BlendOp::Mix => a * (1.0 - m) + b * m,
        BlendOp::Neg => a * (1.0 - m) - b * m,
        BlendOp::Wipe => {
            let w = m * m * (3.0 - 2.0 * m);
            a * (1.0 - w) + b * w
        }
        BlendOp::Am => a * (1.0 - m + m * b),
        BlendOp::Sign => {
            let s = if a < 0.0 { -1.0 } else { 1.0 };
            a * (1.0 - m) + m * b.abs() * s
        }
        BlendOp::Max => a * (1.0 - m) + m * a.max(b),
    }
}

/// the main filter of one channel: two TPT SVF stages
#[derive(Clone, Copy, Default)]
pub struct Filt {
    s1: Svf,
    s2: Svf,
}

/// filter damping (1/Q) of the stages for resonance 0..1
#[inline]
pub fn damping(mode: PolyFilter, res: f32) -> (f32, f32) {
    let r = res.clamp(0.0, 1.0);
    if mode.pole4() {
        // Butterworth first stage, resonant second stage
        (1.848, (0.765 - 0.745 * r).max(0.02))
    } else {
        ((2.0 - 1.98 * r).max(0.02), 0.0)
    }
}

impl Filt {
    #[inline]
    pub fn run(&mut self, mode: PolyFilter, x: f32, g: f32, k: (f32, f32)) -> f32 {
        let (lp, bp, hp) = self.s1.process_gk(x, g, k.0);
        match mode {
            PolyFilter::Lp2 => lp,
            PolyFilter::Bp2 => bp * k.0,
            PolyFilter::Hp2 => hp,
            PolyFilter::Br => lp + hp,
            PolyFilter::Lp4 => self.s2.process_gk(lp, g, k.1).0,
            PolyFilter::Bp4 => self.s2.process_gk(bp * k.0, g, k.1).1 * k.1,
            PolyFilter::Hp4 => self.s2.process_gk(hp, g, k.1).2,
        }
    }
}

/// the shaper, amount `a` 0..1 (0 = clean)
#[inline]
pub fn shape(mode: ShaperMode, x: f32, a: f32) -> f32 {
    let g = 1.0 + 15.0 * a;
    match mode {
        ShaperMode::Soft => tanh_p(g * x) / tanh_p(g),
        ShaperMode::Hard => (g * x).clamp(-1.0, 1.0),
        ShaperMode::Fold => crate::transform::fold(g * x),
        ShaperMode::Asym => {
            let b = 0.5 * a;
            tanh_p(g * x + b) - tanh_p(b)
        }
    }
}

/// ADSR: linear attack, exponential decay and release. `retrigger` restarts
/// the attack from the current level (Mono mode, no click).
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Env {
    pub stage: u8,
    pub level: f32,
}

pub const ATTACK: u8 = 0;
pub const DECAY: u8 = 1;
pub const SUSTAIN: u8 = 2;
pub const RELEASE: u8 = 3;
pub const OFF: u8 = 4;

impl Env {
    pub fn retrigger(&mut self) {
        self.stage = ATTACK;
    }
    pub fn release(&mut self) {
        if self.stage != OFF {
            self.stage = RELEASE;
        }
    }
    pub fn done(&self) -> bool {
        self.stage == OFF
    }
    /// per-sample rate factors: attack step, decay and release coefficients
    /// (precomputed at control rate)
    pub fn coefs(a: f32, d: f32, r: f32, dt: f32, min_a: f32) -> (f32, f32, f32) {
        (
            dt / a.max(min_a),
            1.0 - (-dt * 5.0 / d.max(0.005)).exp(),
            1.0 - (-dt * 5.0 / r.max(0.005)).exp(),
        )
    }
    #[inline]
    pub fn next(&mut self, c: (f32, f32, f32), s: f32) -> f32 {
        match self.stage {
            ATTACK => {
                self.level += c.0;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = DECAY;
                }
            }
            DECAY => {
                self.level += (s - self.level) * c.1;
                if (self.level - s).abs() < 1e-4 {
                    self.level = s;
                    // a decay to silence ends the note (plucks free their voice)
                    self.stage = if s <= 1e-4 { OFF } else { SUSTAIN };
                }
            }
            SUSTAIN => {
                self.level += (s - self.level) * c.1;
            }
            RELEASE => {
                self.level -= self.level * c.2;
                if self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = OFF;
                }
            }
            _ => self.level = 0.0,
        }
        self.level
    }
}

/// fast 2^x (for per-sample filter FM), within 0.1 % over ±8
#[inline]
pub fn exp2(x: f32) -> f32 {
    let x = x.clamp(-30.0, 30.0);
    let i = x.floor();
    let f = x - i;
    let p = 1.0
        + f * (std::f32::consts::LN_2 + f * (0.240_226_5 + f * (0.055_504_1 + f * 0.009_618_1)));
    p * 2f32.powi(i as i32)
}

/// fast sine of `2 pi p`, p 0..1 (parabolic, ~0.1 % error)
#[inline]
pub fn sin2pi(p: f32) -> f32 {
    let x = p - p.floor();
    let q = if x < 0.5 { x } else { x - 1.0 };
    let y = 8.0 * q - 16.0 * q * q.abs();
    0.225 * (y * y.abs() - y) + y
}
