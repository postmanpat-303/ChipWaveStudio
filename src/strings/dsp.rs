//! Small DSP building blocks.
use std::f32::consts::PI;

pub const TAU: f32 = 2.0 * PI;

/// Tiny xorshift PRNG (realtime safe).
#[derive(Clone)]
pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// uniform in [0, 1)
    #[inline]
    pub fn uni(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
    /// uniform in [-1, 1)
    #[inline]
    pub fn bi(&mut self) -> f32 {
        self.uni() * 2.0 - 1.0
    }
}

#[inline]
pub fn poly_blep(t: f32, dt: f32) -> f32 {
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

/// Band-limited saw (PolyBLEP), phase in [0,1).
#[derive(Clone, Copy, Default)]
pub struct Saw {
    pub phase: f32,
}
impl Saw {
    #[inline]
    pub fn tick(&mut self, dt: f32) -> f32 {
        let dt = dt.min(0.49);
        let v = 2.0 * self.phase - 1.0 - poly_blep(self.phase, dt);
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        v
    }
}

#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// Transparent below 0.85, smoothly saturates so |y| <= 0.985 (about -0.13 dBFS).
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.85 {
        x
    } else {
        let y = 0.85 + 0.135 * fast_tanh((a - 0.85) / 0.135);
        y.copysign(x)
    }
}

/// Master peak limiter (stereo-linked, fast attack, slow release) followed by `soft_clip` as a
/// safety saturator. Leaves signals below the threshold untouched.
pub struct Limiter {
    gain: f32,
    att: f32,
    rel: f32,
    /// lowest gain since the last `take_reduction_db`
    min_gain: f32,
}

impl Limiter {
    pub const THRESHOLD: f32 = 0.72;
    pub fn new(sr: f32) -> Self {
        Self {
            gain: 1.0,
            att: 1.0 - (-1.0 / (0.0008 * sr)).exp(),
            rel: 1.0 - (-1.0 / (0.15 * sr)).exp(),
            min_gain: 1.0,
        }
    }
    pub fn reset(&mut self) {
        self.gain = 1.0;
        self.min_gain = 1.0;
    }
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs());
        let target = if peak > Self::THRESHOLD {
            Self::THRESHOLD / peak
        } else {
            1.0
        };
        let c = if target < self.gain {
            self.att
        } else {
            self.rel
        };
        self.gain += (target - self.gain) * c;
        self.min_gain = self.min_gain.min(self.gain);
        (soft_clip(l * self.gain), soft_clip(r * self.gain))
    }
    /// Deepest gain reduction (dB, 0 = none) since the previous call, for metering.
    pub fn take_reduction_db(&mut self) -> f32 {
        let g = std::mem::replace(&mut self.min_gain, self.gain);
        20.0 * g.max(1e-4).log10()
    }
    /// Current gain reduction in dB (0 = none), for metering.
    pub fn reduction_db(&self) -> f32 {
        20.0 * self.gain.max(1e-4).log10()
    }
}

#[inline]
pub fn semis_to_ratio(s: f32) -> f32 {
    (s * (std::f32::consts::LN_2 / 12.0)).exp()
}

#[inline]
pub fn note_to_hz(n: f32) -> f32 {
    440.0 * semis_to_ratio(n - 69.0)
}

/// Linear-interpolated delay line with power-of-two buffer.
#[derive(Clone)]
pub struct Delay {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
}
impl Delay {
    pub fn new(min_len: usize) -> Self {
        let len = min_len.max(4).next_power_of_two();
        Self {
            buf: vec![0.0; len],
            mask: len - 1,
            pos: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.pos = 0;
    }
    #[inline]
    pub fn write(&mut self, x: f32) {
        self.pos = (self.pos + 1) & self.mask;
        self.buf[self.pos] = x;
    }
    /// Read `d` samples behind the last written sample (d >= 0, fractional).
    #[inline]
    pub fn read(&self, d: f32) -> f32 {
        let d = d.clamp(0.0, (self.mask - 2) as f32);
        let di = d as usize;
        let f = d - di as f32;
        let a = self.buf[(self.pos.wrapping_sub(di)) & self.mask];
        let b = self.buf[(self.pos.wrapping_sub(di + 1)) & self.mask];
        a + (b - a) * f
    }
    #[inline]
    pub fn read_int(&self, d: usize) -> f32 {
        self.buf[(self.pos.wrapping_sub(d)) & self.mask]
    }
}

/// One-pole lowpass.
#[derive(Clone, Copy, Default)]
pub struct OnePole {
    pub z: f32,
}
impl OnePole {
    #[inline]
    pub fn coef(hz: f32, sr: f32) -> f32 {
        1.0 - (-TAU * hz / sr).exp()
    }
    #[inline]
    pub fn lp(&mut self, x: f32, a: f32) -> f32 {
        self.z += a * (x - self.z);
        self.z
    }
    #[inline]
    pub fn hp(&mut self, x: f32, a: f32) -> f32 {
        x - self.lp(x, a)
    }
}

/// Cytomic/Simper TPT state variable filter (lowpass output).
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}
#[derive(Clone, Copy, Default)]
pub struct SvfCoefs {
    a1: f32,
    a2: f32,
    a3: f32,
}
impl SvfCoefs {
    pub fn lowpass(hz: f32, sr: f32, k: f32) -> Self {
        let g = (PI * (hz / sr).clamp(0.0001, 0.45)).tan();
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        Self { a1, a2, a3 }
    }
}
impl Svf {
    #[inline]
    pub fn lp(&mut self, x: f32, c: &SvfCoefs) -> f32 {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        v2
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// ADSR envelope. Linear attack, exponential decay/release (times are to ~-60 dB).
#[derive(Clone, Copy)]
pub struct Adsr {
    pub stage: Stage,
    pub level: f32,
}
impl Default for Adsr {
    fn default() -> Self {
        Self {
            stage: Stage::Idle,
            level: 0.0,
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct AdsrRates {
    pub attack_inc: f32,
    pub decay_coef: f32,
    pub sustain: f32,
    pub release_coef: f32,
}
impl AdsrRates {
    pub fn new(a: f32, d: f32, s: f32, r: f32, sr: f32) -> Self {
        // exp coefficient reaching -60 dB (factor 1/1000) after `t` seconds
        let c = |t: f32| (-6.907_755 / (t.max(0.001) * sr)).exp();
        Self {
            attack_inc: 1.0 / (a.max(0.001) * sr),
            decay_coef: c(d),
            sustain: s,
            release_coef: c(r),
        }
    }
}
impl Adsr {
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }
    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }
    #[inline]
    pub fn tick(&mut self, r: &AdsrRates) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += r.attack_inc;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level = r.sustain + (self.level - r.sustain) * r.decay_coef;
                if (self.level - r.sustain).abs() < 1e-4 {
                    self.level = r.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                // follow sustain changes smoothly
                self.level += (r.sustain - self.level) * 0.001;
            }
            Stage::Release => {
                self.level *= r.release_coef;
                if self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}
