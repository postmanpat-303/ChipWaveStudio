//! Voice **Transform** (0.2.3.11): three blendable stages after the formant
//! bank, plus the same stages as FX devices for any instrument.
//!
//! 1. **Metallic resonance**: six two-pole modal resonators ("constant peak
//!    gain" form after Smith & Angell 1982 / J. O. Smith, *Introduction to
//!    Digital Filters*), tuned to the note or a fixed pitch, with harmonic,
//!    808-metal, church-bell (Rossing), free-bar (Rossing) or odd ratios.
//!    The poles always sit inside the unit circle (radius clamped), and the
//!    output is soft-saturated, so no setting can blow up.
//! 2. **Digital distortion**: pre high-pass, drive into a hard clip or an
//!    in-phase wavefold, bit-depth reduction, sample-rate reduction (clean
//!    with an anti-alias filter, or raw sample-and-hold that aliases), a ring
//!    modulator tuned to the note, post low-pass. Every step keeps the
//!    polarity of its input, so a dry/wet blend never cancels.
//! 3. **Sub-bass**: a sine one or two octaves under the note and/or an
//!    octave divider (flip-flop on the zero crossings, after the classic
//!    analog octave pedals), shaped by the voice's own amplitude envelope,
//!    saturated for growl, and split with a Linkwitz-Riley crossover so the
//!    sub below the crossover is a clean mono signal. The voice's own low
//!    band is folded to mono too. Formant drop lowers the formants for a
//!    "monster" voice.
//!
//! Everything here is fixed-size state: `process` never allocates.
use serde::{Deserialize, Serialize};

use crate::synth::Svf;

/// resonator mode frequency ratios
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ratios {
    /// 1 2 3 4 5 6: a comb-like, robotic buzz
    Harmonic,
    /// the TR-808 cymbal / hat oscillator ratios: clangy metal
    #[default]
    Metal,
    /// church-bell partials (hum, prime, tierce, quint, nominal, deciem)
    Bell,
    /// free-free bar modes: glassy, very inharmonic
    Bar,
    /// odd harmonics 1 3 5 7 9 11: hollow, square-ish robot
    Odd,
}

impl Ratios {
    pub const ALL: [Ratios; 5] = [
        Ratios::Harmonic,
        Ratios::Metal,
        Ratios::Bell,
        Ratios::Bar,
        Ratios::Odd,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Ratios::Harmonic => "harmonic",
            Ratios::Metal => "metal",
            Ratios::Bell => "bell",
            Ratios::Bar => "bar",
            Ratios::Odd => "odd",
        }
    }
    pub fn ratios(self) -> [f32; 6] {
        match self {
            Ratios::Harmonic => [1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            Ratios::Metal => [1.0, 1.4827, 1.8003, 2.5461, 2.6303, 3.8967],
            Ratios::Bell => [0.5, 1.0, 1.183, 1.506, 2.0, 2.514],
            Ratios::Bar => [1.0, 2.756, 5.404, 8.933, 13.344, 18.638],
            Ratios::Odd => [1.0, 3.0, 5.0, 7.0, 9.0, 11.0],
        }
    }
    fn from_index(i: usize) -> Ratios {
        Ratios::ALL[i.min(4)]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubMode {
    /// a sine following the note pitch
    #[default]
    Sine,
    /// octave divider (follows any input, no note needed)
    Divider,
    /// both together
    Both,
}

impl SubMode {
    pub const ALL: [SubMode; 3] = [SubMode::Sine, SubMode::Divider, SubMode::Both];
    pub fn name(self) -> &'static str {
        match self {
            SubMode::Sine => "sine",
            SubMode::Divider => "divider",
            SubMode::Both => "sine + divider",
        }
    }
}

/// order of the two serial stages (the sub is always added in parallel)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    /// distortion, then resonance (the resonators ring on the grit)
    #[default]
    DistRes,
    /// resonance, then distortion (the ringing gets crushed)
    ResDist,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResParams {
    /// wet mix 0..1
    pub amount: f32,
    pub ratios: Ratios,
    /// tuned to the note (else `hz`)
    pub keyed: bool,
    /// semitones from the note (-24..24)
    pub tune: f32,
    /// fixed pitch 30..4000 Hz
    pub hz: f32,
    /// ring time (T60) of the lowest mode, 20..4000 ms
    pub decay_ms: f32,
    /// 0..1: higher modes die faster and the ring gets darker
    pub damp: f32,
    /// 0..1: modes detuned apart, left vs right (stereo)
    pub spread: f32,
}

impl Default for ResParams {
    fn default() -> Self {
        Self {
            amount: 0.5,
            ratios: Ratios::Metal,
            keyed: true,
            tune: 0.0,
            hz: 220.0,
            decay_ms: 400.0,
            damp: 0.4,
            spread: 0.3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigParams {
    pub amount: f32,
    /// 0..36 dB into the clipper / folder
    pub drive_db: f32,
    /// wavefold instead of a hard clip
    pub fold: bool,
    /// 1..16 bits
    pub bits: f32,
    /// sample-and-hold rate 300..48000 Hz
    pub rate: f32,
    /// raw sample-and-hold (aliasing) instead of filtered decimation
    pub alias: bool,
    /// ring modulator depth 0..1
    pub ring: f32,
    /// ring frequency as a multiple of the note (0.25..8)
    pub ring_ratio: f32,
    /// pre high-pass 20..2000 Hz
    pub hp: f32,
    /// post low-pass 1000..20000 Hz
    pub lp: f32,
}

impl Default for DigParams {
    fn default() -> Self {
        Self {
            amount: 0.4,
            drive_db: 12.0,
            fold: false,
            bits: 8.0,
            rate: 12000.0,
            alias: true,
            ring: 0.0,
            ring_ratio: 1.5,
            hp: 80.0,
            lp: 12000.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubParams {
    /// sub level 0..1
    pub amount: f32,
    pub mode: SubMode,
    /// 1 or 2 octaves down
    pub octave: u8,
    /// 0..1 saturation of the sub
    pub drive: f32,
    /// 0..1 level of the saturation harmonics above the crossover
    pub growl: f32,
    /// crossover 60..250 Hz: below it everything is mono
    pub xover: f32,
    /// formant drop 0..12 semitones (monster voice; Voice only)
    pub formant_drop: f32,
}

impl Default for SubParams {
    fn default() -> Self {
        Self {
            amount: 0.5,
            mode: SubMode::Sine,
            octave: 1,
            drive: 0.3,
            growl: 0.3,
            xover: 120.0,
            formant_drop: 0.0,
        }
    }
}

/// The Voice instrument's Transform section (saved in songs).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    /// master dry (0) .. transformed (1); 0 = off (the default)
    pub blend: f32,
    pub order: Order,
    pub res: ResParams,
    pub dig: DigParams,
    pub sub: SubParams,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            blend: 0.0,
            order: Order::DistRes,
            res: ResParams::default(),
            dig: DigParams::default(),
            sub: SubParams::default(),
        }
    }
}

fn fin(v: f32, d: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        d
    }
}

impl Transform {
    /// clamp loaded / edited values into range (NaN -> default)
    pub fn fix(&mut self) {
        let d = Transform::default();
        self.blend = fin(self.blend, 0.0, 0.0, 1.0);
        let r = &mut self.res;
        r.amount = fin(r.amount, d.res.amount, 0.0, 1.0);
        r.tune = fin(r.tune, 0.0, -24.0, 24.0);
        r.hz = fin(r.hz, d.res.hz, 30.0, 4000.0);
        r.decay_ms = fin(r.decay_ms, d.res.decay_ms, 20.0, 4000.0);
        r.damp = fin(r.damp, d.res.damp, 0.0, 1.0);
        r.spread = fin(r.spread, d.res.spread, 0.0, 1.0);
        let g = &mut self.dig;
        g.amount = fin(g.amount, d.dig.amount, 0.0, 1.0);
        g.drive_db = fin(g.drive_db, d.dig.drive_db, 0.0, 36.0);
        g.bits = fin(g.bits, 16.0, 1.0, 16.0);
        g.rate = fin(g.rate, 48000.0, 300.0, 48000.0);
        g.ring = fin(g.ring, 0.0, 0.0, 1.0);
        g.ring_ratio = fin(g.ring_ratio, 1.0, 0.25, 8.0);
        g.hp = fin(g.hp, 20.0, 20.0, 2000.0);
        g.lp = fin(g.lp, 20000.0, 1000.0, 20000.0);
        let s = &mut self.sub;
        s.amount = fin(s.amount, d.sub.amount, 0.0, 1.0);
        s.octave = s.octave.clamp(1, 2);
        s.drive = fin(s.drive, 0.0, 0.0, 1.0);
        s.growl = fin(s.growl, 0.0, 0.0, 1.0);
        s.xover = fin(s.xover, 120.0, 60.0, 250.0);
        s.formant_drop = fin(s.formant_drop, 0.0, 0.0, 12.0);
    }
}

// ------------------------------------------------------------------ DSP

/// a Butterworth (k = sqrt 2) 2-pole section that caches its coefficient
#[derive(Clone, Copy, Default)]
struct Bw {
    s: Svf,
    fc: f32,
    g: f32,
}

impl Bw {
    #[inline]
    fn run(&mut self, x: f32, fc: f32, sr: f32) -> (f32, f32, f32) {
        if fc != self.fc {
            self.fc = fc;
            self.g = (std::f32::consts::PI * fc.clamp(10.0, sr * 0.45) / sr).tan();
        }
        self.s.process_gk(x, self.g, std::f32::consts::SQRT_2)
    }
    fn lp(&mut self, x: f32, fc: f32, sr: f32) -> f32 {
        self.run(x, fc, sr).0
    }
    fn hp(&mut self, x: f32, fc: f32, sr: f32) -> f32 {
        self.run(x, fc, sr).2
    }
}

/// Linkwitz-Riley 4th order low / high split (LP + HP sum to an all-pass)
#[derive(Clone, Copy, Default)]
struct Lr4 {
    lp: [Bw; 2],
    hp: [Bw; 2],
}

impl Lr4 {
    fn split(&mut self, x: f32, fc: f32, sr: f32) -> (f32, f32) {
        let l0 = self.lp[0].lp(x, fc, sr);
        let l = self.lp[1].lp(l0, fc, sr);
        // LR4 high band = HP^2 with the sign kept so lo + hi is all-pass
        let h0 = self.hp[0].hp(x, fc, sr);
        let h = self.hp[1].hp(h0, fc, sr);
        (l, h)
    }
}

/// "Mono maker": high-passes only the side signal (8th order Butterworth,
/// 48 dB/oct, corner one octave above the crossover), so below the
/// crossover the side is at least ~50 dB down while the mid is untouched
/// (the elliptical EQ of vinyl mastering).
#[derive(Clone, Copy, Default)]
struct MonoLow {
    hp: [Bw; 4],
}

impl MonoLow {
    fn run(&mut self, l: f32, r: f32, fc: f32, sr: f32) -> (f32, f32) {
        let m = (l + r) * 0.5;
        let mut s = (l - r) * 0.5;
        for h in self.hp.iter_mut() {
            s = h.hp(s, fc * 2.0, sr);
        }
        (m + s, m - s)
    }
}

#[inline]
fn clean(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(-16.0, 16.0)
    } else {
        0.0
    }
}

/// smooth soft limit: linear below ~1, approaches +-2 (never blows up)
#[inline]
fn soft(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    let a = x.abs();
    if a < 1.0 {
        x
    } else {
        x.signum() * (2.0 - 1.0 / a)
    }
}

/// in-phase triangle fold (same shape as the Drive device's fold)
#[inline]
fn fold(x: f32) -> f32 {
    let y = x * 0.5;
    1.0 - ((y + 1.0).rem_euclid(4.0) - 2.0).abs()
}

#[derive(Clone, Copy, Default)]
struct Mode {
    y1: f32,
    y2: f32,
    x1: f32,
    x2: f32,
    a1: f32,
    a2: f32,
    b: f32,
}

/// six modal resonators per side
#[derive(Clone, Copy, Default)]
pub struct ResBank {
    m: [[Mode; 6]; 2],
    tone: [Bw; 2],
    cnt: u8,
    last: (f32, f32),
}

/// modal resonator output trim (narrow bands carry little energy)
const RES_GAIN: f32 = 4.5;

impl ResBank {
    fn coefs(&mut self, p: &ResParams, note: f32, sr: f32) {
        let base = if p.keyed && note > 0.0 {
            note * 2f32.powf(p.tune / 12.0)
        } else {
            p.hz
        };
        let rat = p.ratios.ratios();
        let nyq = sr * 0.45;
        for (side, sgn) in [(0usize, 1.0f32), (1, -1.0)] {
            for (k, m) in self.m[side].iter_mut().enumerate() {
                let cents = sgn * p.spread * 9.0 * (k as f32 + 1.0);
                let f = base * rat[k] * 2f32.powf(cents / 1200.0);
                if !(20.0..nyq).contains(&f) {
                    m.b = 0.0;
                    continue;
                }
                let t60 = p.decay_ms * 0.001 / (1.0 + p.damp * 1.5 * k as f32);
                let r = (-6.9078 / (t60.max(0.005) * sr)).exp().min(0.99995);
                let w = std::f32::consts::TAU * f / sr;
                m.a1 = 2.0 * r * w.cos();
                m.a2 = -r * r;
                // constant peak gain: zeros at DC and Nyquist
                m.b = (1.0 - r * r) * 0.5;
            }
        }
    }

    fn process(&mut self, l: f32, r: f32, p: &ResParams, note: f32, sr: f32) -> (f32, f32) {
        if self.cnt == 0 || self.last != (note, p.decay_ms) {
            self.coefs(p, note, sr);
            self.last = (note, p.decay_ms);
        }
        self.cnt = (self.cnt + 1) % 32;
        let tone = 900.0 + 17000.0 * (1.0 - p.damp) * (1.0 - p.damp);
        let mut out = [0.0f32; 2];
        for (side, x) in [(0usize, l), (1, r)] {
            let mut y = 0.0;
            for m in self.m[side].iter_mut() {
                let v = m.b * (x - m.x2) + m.a1 * m.y1 + m.a2 * m.y2;
                let v = if v.is_finite() {
                    v.clamp(-64.0, 64.0)
                } else {
                    0.0
                };
                m.x2 = m.x1;
                m.x1 = x;
                m.y2 = m.y1;
                m.y1 = v;
                y += v;
            }
            out[side] = soft(self.tone[side].lp(y * RES_GAIN, tone, sr));
        }
        let a = p.amount;
        (l + (out[0] - l) * a, r + (out[1] - r) * a)
    }
}

/// digital distortion for one channel
#[derive(Clone, Copy, Default)]
pub struct Digital {
    hp: Bw,
    aa: [Bw; 2],
    post_aa: Bw,
    lp: Bw,
    ph: f32,
    hold: f32,
    ring_ph: f32,
}

impl Digital {
    fn process(&mut self, x: f32, p: &DigParams, note: f32, sr: f32) -> f32 {
        let mut y = self.hp.hp(x, p.hp, sr);
        // drive: hard clip or fold, then trim back so the level stays close
        let g = 10f32.powf(p.drive_db / 20.0);
        y = if p.fold {
            fold(y * g)
        } else {
            (y * g).clamp(-1.0, 1.0)
        } / (1.0 + p.drive_db / 18.0);
        // bit depth
        if p.bits < 15.99 {
            let lv = 2f32.powf(p.bits - 1.0).max(1.0);
            y = (y * lv).round() / lv;
        }
        // sample-rate reduction
        if p.rate < sr * 0.98 {
            let src = if p.alias {
                y
            } else {
                let fc = p.rate * 0.45;
                let a0 = self.aa[0].lp(y, fc, sr);
                self.aa[1].lp(a0, fc, sr)
            };
            self.ph += p.rate / sr;
            if self.ph >= 1.0 {
                self.ph -= self.ph.floor();
                self.hold = src;
            }
            y = if p.alias {
                self.hold
            } else {
                self.post_aa.lp(self.hold, p.rate * 0.45, sr)
            };
        }
        // ring modulator at the note x ratio (440 Hz x ratio without a note)
        if p.ring > 0.0 {
            let f = if note > 0.0 { note } else { 440.0 } * p.ring_ratio;
            self.ring_ph = (self.ring_ph + f / sr).fract();
            let c = (self.ring_ph * std::f32::consts::TAU).sin();
            y *= 1.0 - p.ring + p.ring * c;
        }
        y = self.lp.lp(y, p.lp, sr);
        let y = if y.is_finite() {
            y.clamp(-2.0, 2.0)
        } else {
            0.0
        };
        x + (y - x) * p.amount
    }
}

/// sub-bass generator (mono)
#[derive(Clone, Copy, Default)]
pub struct SubGen {
    env: f32,
    ph: f32,
    det: Bw,
    pos: bool,
    ff: [bool; 2],
    lo: Lr4,
}

/// sub level trim relative to the voice's amplitude envelope
const SUB_GAIN: f32 = 0.8;

impl SubGen {
    /// returns (clean sub below the crossover, growl above it)
    fn process(&mut self, x: f32, p: &SubParams, note: f32, sr: f32) -> (f32, f32) {
        // envelope follower: 2 ms attack, 60 ms release
        let a = x.abs();
        let k = if a > self.env {
            1.0 - (-1.0 / (0.002 * sr)).exp()
        } else {
            1.0 - (-1.0 / (0.06 * sr)).exp()
        };
        self.env += (a - self.env) * k;
        let div_f = 1.0 / (1 << p.octave.clamp(1, 2)) as f32;
        let sine = if note > 0.0 {
            self.ph = (self.ph + note * div_f / sr).fract();
            (self.ph * std::f32::consts::TAU).sin()
        } else {
            0.0
        };
        let use_div = p.mode != SubMode::Sine || note <= 0.0;
        let div = if use_div {
            // flip-flop on positive zero crossings of the low-passed input
            let fc = if note > 0.0 { note * 1.5 } else { 300.0 };
            let d = self.det.lp(x, fc.clamp(40.0, 1200.0), sr);
            let up = if self.pos { d > -1e-4 } else { d > 1e-4 };
            if up && !self.pos {
                self.ff[0] = !self.ff[0];
                if self.ff[0] {
                    self.ff[1] = !self.ff[1];
                }
            }
            self.pos = up;
            let b = if p.octave >= 2 {
                self.ff[1]
            } else {
                self.ff[0]
            };
            if b {
                0.8
            } else {
                -0.8
            }
        } else {
            0.0
        };
        let s = match p.mode {
            SubMode::Sine if note > 0.0 => sine,
            SubMode::Both if note > 0.0 => (sine + div) * 0.6,
            _ => div,
        };
        let raw = (s * self.env * SUB_GAIN).clamp(-1.5, 1.5);
        let y = if p.drive > 0.0 {
            // saturate, keeping the peak at the envelope's level (adds
            // harmonics, not loudness)
            let g = 1.0 + p.drive * 8.0;
            let pk = (self.env * SUB_GAIN).clamp(1e-4, 1.5);
            (raw / pk * g).tanh() * pk / g.tanh()
        } else {
            raw
        };
        let (lo, hi) = self.lo.split(y, p.xover, sr);
        (lo * p.amount, hi * p.amount * p.growl * 1.5)
    }
}

/// Per-note state of the Voice Transform (fixed size, allocation-free).
#[derive(Clone, Copy, Default)]
pub struct TransformState {
    res: ResBank,
    dig: Digital,
    sub: SubGen,
    mono: MonoLow,
    /// smoothed blend and stage amounts (pattern commands 3F-42)
    cur: [f32; 4],
    init: bool,
}

/// pattern overrides for blend, resonance, digital, sub (`< 0` = none)
pub type Overrides = [f32; 4];
pub const NO_OVERRIDES: Overrides = [-1.0; 4];

impl Transform {
    /// true when the transform changes the sound (with these overrides)
    pub fn active(&self, ov: &Overrides) -> bool {
        (if ov[0] >= 0.0 { ov[0] } else { self.blend }) > 0.0
    }
}

impl TransformState {
    /// current smoothed blend (after `process`); the Voice applies the
    /// formant drop with it
    #[allow(dead_code)]
    pub fn blend(&self) -> f32 {
        self.cur[0]
    }

    /// processed at least one sample of this note (keeps running when a
    /// pattern command fades the blend to 0, so nothing clicks)
    pub fn running(&self) -> bool {
        self.init
    }

    /// One sample of the voice `x` (mono) at note `note` Hz through the
    /// transform: returns the stereo main signal and the mono sub (added
    /// after the patch filter so it stays clean). Allocation-free.
    pub fn process(
        &mut self,
        t: &Transform,
        ov: &Overrides,
        x: f32,
        note: f32,
        sr: f32,
    ) -> (f32, f32, f32) {
        let x = clean(x);
        let note = if note.is_finite() {
            note.clamp(0.0, 24000.0)
        } else {
            0.0
        };
        let tg = [
            if ov[0] >= 0.0 { ov[0] } else { t.blend },
            if ov[1] >= 0.0 { ov[1] } else { t.res.amount },
            if ov[2] >= 0.0 { ov[2] } else { t.dig.amount },
            if ov[3] >= 0.0 { ov[3] } else { t.sub.amount },
        ];
        if !self.init {
            self.cur = tg;
            self.init = true;
        }
        let k = 1.0 - (-1.0 / (0.01 * sr)).exp();
        for (c, t) in self.cur.iter_mut().zip(tg) {
            *c += (t - *c) * k;
        }
        let [blend, ra, da, sa] = self.cur;
        let rp = ResParams {
            amount: ra,
            ..t.res
        };
        let dp = DigParams {
            amount: da,
            ..t.dig
        };
        let sp = SubParams {
            amount: sa,
            ..t.sub
        };
        let (wl, wr) = match t.order {
            Order::DistRes => {
                let d = if da > 0.0 {
                    self.dig.process(x, &dp, note, sr)
                } else {
                    x
                };
                if ra > 0.0 {
                    self.res.process(d, d, &rp, note, sr)
                } else {
                    (d, d)
                }
            }
            Order::ResDist => {
                let (l, r) = if ra > 0.0 {
                    self.res.process(x, x, &rp, note, sr)
                } else {
                    (x, x)
                };
                if da > 0.0 {
                    // one digital state: run it on the mid, keep the side
                    let m = (l + r) * 0.5;
                    let s = (l - r) * 0.5;
                    let dm = self.dig.process(m, &dp, note, sr);
                    (dm + s, dm - s)
                } else {
                    (l, r)
                }
            }
        };
        let mut l = x + (wl - x) * blend;
        let mut r = x + (wr - x) * blend;
        let mut sub = 0.0;
        if t.sub.amount > 0.0 || ov[3] > 0.0 {
            let (lo, hi) = self.sub.process(x, &sp, note, sr);
            sub = (lo + hi) * blend;
            // everything below the crossover is mono
            (l, r) = self.mono.run(l, r, t.sub.xover, sr);
        }
        (l, r, sub)
    }
}

// ------------------------------------------------- the stages as FX devices

/// realtime state of a Resonator / Digital / Sub FX device (stereo)
#[derive(Clone, Copy, Default)]
pub struct FxStage {
    res: ResBank,
    dig: [Digital; 2],
    sub: SubGen,
    mono: MonoLow,
}

impl FxStage {
    /// Resonator device: p = mix, ratios, pitch (0 note, 1 fixed), tune,
    /// hz, decay, damp, spread
    pub fn resonator(&mut self, l: f32, r: f32, p: &[f32], note: f32, sr: f32) -> (f32, f32) {
        let rp = ResParams {
            amount: p[0],
            ratios: Ratios::from_index(p[1] as usize),
            keyed: p[2] < 0.5,
            tune: p[3],
            hz: p[4],
            decay_ms: p[5],
            damp: p[6],
            spread: p[7],
        };
        self.res.process(clean(l), clean(r), &rp, note, sr)
    }

    /// Digital device: p = mix, drive, shape (0 clip, 1 fold), bits, rate,
    /// alias, ring, ring ratio, hp, lp
    pub fn digital(&mut self, l: f32, r: f32, p: &[f32], note: f32, sr: f32) -> (f32, f32) {
        let dp = DigParams {
            amount: p[0],
            drive_db: p[1],
            fold: p[2] >= 0.5,
            bits: p[3],
            rate: p[4],
            alias: p[5] >= 0.5,
            ring: p[6],
            ring_ratio: p[7],
            hp: p[8],
            lp: p[9],
        };
        (
            self.dig[0].process(clean(l), &dp, note, sr),
            self.dig[1].process(clean(r), &dp, note, sr),
        )
    }

    /// Sub device: p = level, mode, octave (0 = -1, 1 = -2), drive, growl,
    /// crossover. The sub is generated from the mid; below the crossover
    /// the output is mono.
    pub fn sub(&mut self, l: f32, r: f32, p: &[f32], note: f32, sr: f32) -> (f32, f32) {
        let sp = SubParams {
            amount: p[0],
            mode: SubMode::ALL[(p[1] as usize).min(2)],
            octave: if p[2] >= 0.5 { 2 } else { 1 },
            drive: p[3],
            growl: p[4],
            xover: p[5],
            formant_drop: 0.0,
        };
        let (l, r) = (clean(l), clean(r));
        let (lo, hi) = self.sub.process((l + r) * 0.5, &sp, note, sr);
        let (l, r) = self.mono.run(l, r, sp.xover, sr);
        let s = lo + hi;
        (l + s, r + s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;

    /// a band-limited-ish buzzy test voice (sum of 20 harmonics)
    fn buzz(f: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / SR;
                (1..=20)
                    .filter(|h| *h as f32 * f < SR * 0.45)
                    .map(|h| (std::f32::consts::TAU * f * h as f32 * t).sin() / h as f32)
                    .sum::<f32>()
                    * 0.3
            })
            .collect()
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    fn extreme() -> Vec<Transform> {
        let mut out = vec![];
        for &decay in &[20.0, 4000.0] {
            for &fold in &[false, true] {
                for &alias in &[false, true] {
                    for order in [Order::DistRes, Order::ResDist] {
                        for ratios in [Ratios::Bell, Ratios::Bar] {
                            out.push(Transform {
                                blend: 1.0,
                                order,
                                res: ResParams {
                                    amount: 1.0,
                                    ratios,
                                    keyed: decay > 100.0,
                                    tune: if fold { 24.0 } else { -24.0 },
                                    hz: 4000.0,
                                    decay_ms: decay,
                                    damp: 0.0,
                                    spread: 1.0,
                                },
                                dig: DigParams {
                                    amount: 1.0,
                                    drive_db: 36.0,
                                    fold,
                                    bits: 1.0,
                                    rate: 300.0,
                                    alias,
                                    ring: 1.0,
                                    ring_ratio: 8.0,
                                    hp: 2000.0,
                                    lp: 1000.0,
                                },
                                sub: SubParams {
                                    amount: 1.0,
                                    mode: SubMode::ALL[out.len() % 3],
                                    octave: 1 + (out.len() % 2) as u8,
                                    drive: 1.0,
                                    growl: 1.0,
                                    xover: 250.0,
                                    formant_drop: 12.0,
                                },
                            });
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn extreme_settings_never_blow_up_or_go_nan() {
        // loud noise, a DC step, huge spikes, NaN / inf input samples
        let mut n = 0x1234_5678u32;
        let mut input: Vec<f32> = (0..24000)
            .map(|i| {
                n ^= n << 13;
                n ^= n >> 17;
                n ^= n << 5;
                let r = n as f32 / u32::MAX as f32 * 2.0 - 1.0;
                match i {
                    0..=4799 => r * 10.0,
                    4800..=9599 => 4.0,
                    _ if i % 997 == 0 => 1e6,
                    _ => r,
                }
            })
            .collect();
        input[12000] = f32::NAN;
        input[12001] = f32::INFINITY;
        for (k, t) in extreme().iter().enumerate() {
            for note in [0.0, 20.0, 18000.0] {
                let mut s = TransformState::default();
                let mut peak = 0.0f32;
                for &x in &input {
                    let (l, r, sub) = s.process(t, &NO_OVERRIDES, x, note, SR);
                    assert!(
                        l.is_finite() && r.is_finite() && sub.is_finite(),
                        "{k} {note}"
                    );
                    peak = peak.max(l.abs()).max(r.abs()).max(sub.abs());
                }
                // and it recovers: silence in -> silence out
                let mut tail = 0.0f32;
                for _ in 0..(SR as usize * 5) {
                    let (l, r, sub) = s.process(t, &NO_OVERRIDES, 0.0, note, SR);
                    tail = l.abs().max(r.abs()).max(sub.abs());
                }
                assert!(peak < 1e3, "{k} at {note} Hz: peak {peak}");
                assert!(tail < 1e-3, "{k} at {note} Hz: still ringing {tail}");
            }
        }
    }

    #[test]
    fn fx_stages_survive_extremes_too() {
        let mut st = FxStage::default();
        let res = [1.0, 4.0, 1.0, 24.0, 4000.0, 4000.0, 0.0, 1.0];
        let dig = [1.0, 36.0, 1.0, 1.0, 300.0, 1.0, 1.0, 8.0, 2000.0, 1000.0];
        let sub = [1.0, 2.0, 1.0, 1.0, 1.0, 250.0];
        for i in 0..48000 {
            let x = if i % 500 == 0 {
                50.0
            } else {
                (i as f32 * 0.01).sin() * 4.0
            };
            let x = if i == 777 { f32::NAN } else { x };
            let (a, b) = st.resonator(x, -x, &res, 55.0, SR);
            let (c, d) = st.digital(a, b, &dig, 55.0, SR);
            let (e, f) = st.sub(c, d, &sub, 0.0, SR);
            for v in [a, b, c, d, e, f] {
                assert!(v.is_finite() && v.abs() < 100.0, "{i}: {v}");
            }
        }
    }

    #[test]
    fn sub_and_low_band_stay_mono_below_the_crossover() {
        // wide resonance (spread 1) plus the sub: below 120 Hz L == R
        let t = Transform {
            blend: 1.0,
            res: ResParams {
                amount: 0.8,
                spread: 1.0,
                ratios: Ratios::Bell,
                ..Default::default()
            },
            sub: SubParams {
                amount: 1.0,
                mode: SubMode::Both,
                xover: 120.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let x = buzz(110.0, 96000);
        let mut s = TransformState::default();
        let (mut lo_m, mut lo_s) = ([Lr4::default(); 2], [Lr4::default(); 2]);
        let (mut hi_s_e, mut m_e, mut s_e) = (0.0f64, 0.0f64, 0.0f64);
        let mut hs = Lr4::default();
        for (i, &v) in x.iter().enumerate() {
            let (l, r, sub) = s.process(&t, &NO_OVERRIDES, v, 110.0, SR);
            let (l, r) = (l + sub, r + sub);
            let (m, sd) = ((l + r) * 0.5, (l - r) * 0.5);
            // the band below 120 Hz (8th order low-pass at 100 Hz)
            let lm = lo_m[0].split(m, 100.0, SR).0;
            let lm = lo_m[1].split(lm, 100.0, SR).0;
            let ls = lo_s[0].split(sd, 100.0, SR).0;
            let ls = lo_s[1].split(ls, 100.0, SR).0;
            let hsd = hs.split(sd, 1000.0, SR).1;
            if i > 9600 {
                m_e += (lm * lm) as f64;
                s_e += (ls * ls) as f64;
                hi_s_e += (hsd * hsd) as f64;
            }
        }
        let side_db = 10.0 * (s_e / m_e).log10();
        assert!(m_e > 1.0, "there is a sub ({m_e})");
        assert!(
            side_db < -40.0,
            "side below 120 Hz at {side_db:.1} dB re mid"
        );
        assert!(hi_s_e > 0.1 * m_e.min(1.0), "the highs are still wide");
    }

    #[test]
    fn digital_blend_never_cancels_the_dry_voice() {
        let x = buzz(220.0, 48000);
        let d = rms(&x);
        for fold in [false, true] {
            for drive in [0.0, 12.0, 30.0] {
                for (bits, rate) in [(16.0, 48000.0), (6.0, 8000.0)] {
                    let p = DigParams {
                        amount: 0.5,
                        drive_db: drive,
                        fold,
                        bits,
                        rate,
                        ..Default::default()
                    };
                    let mut g = Digital::default();
                    let y: Vec<f32> = x.iter().map(|&v| g.process(v, &p, 220.0, SR)).collect();
                    let corr: f32 = x.iter().zip(&y).map(|(a, b)| a * b).sum::<f32>()
                        / (x.len() as f32 * d * rms(&y));
                    assert!(
                        corr > 0.3 && rms(&y) > d * 0.5,
                        "fold {fold} drive {drive} bits {bits}: corr {corr:.2}, {:.2} vs {d:.2}",
                        rms(&y)
                    );
                }
            }
        }
    }

    #[test]
    fn resonator_rings_at_the_note_and_follows_pitch() {
        let sine = |f: f32, n: usize| -> Vec<f32> {
            (0..n)
                .map(|i| (std::f32::consts::TAU * f * i as f32 / SR).sin() * 0.3)
                .collect()
        };
        let run = |p: &ResParams, x: &[f32], note: f32| -> Vec<f32> {
            let mut r = ResBank::default();
            x.iter().map(|&v| r.process(v, v, p, note, SR).0).collect()
        };
        // keyed, harmonic: a 220 Hz note rings for a 220 Hz input, not 300 Hz
        let p = ResParams {
            amount: 1.0,
            ratios: Ratios::Harmonic,
            keyed: true,
            decay_ms: 800.0,
            damp: 0.0,
            spread: 0.0,
            ..Default::default()
        };
        let on = rms(&run(&p, &sine(220.0, 24000), 220.0)[12000..]);
        let off = rms(&run(&p, &sine(300.0, 24000), 220.0)[12000..]);
        assert!(on > off * 4.0, "on-pitch {on:.4} vs off-pitch {off:.4}");
        // fixed 1 kHz metal: a 1 kHz burst keeps ringing after it stops,
        // then dies away
        let q = ResParams {
            keyed: false,
            hz: 1000.0,
            ratios: Ratios::Metal,
            ..p
        };
        let mut x = sine(1000.0, 4800);
        x.resize(48000, 0.0);
        let y = run(&q, &x, 0.0);
        let ring = rms(&y[4800..9600]);
        assert!(ring > 0.01, "rings after the burst ({ring})");
        assert!(rms(&y[43200..]) < ring * 0.1, "and decays");
    }

    #[test]
    fn old_voice_patches_load_without_a_transform() {
        let mut v: crate::voice::VoicePatch =
            toml::from_str("register = \"Demon\"\nthroat = true\n").unwrap();
        v.fix();
        assert_eq!(v.transform, Transform::default());
        assert!(!v.transform.active(&NO_OVERRIDES));
        // and a full round trip keeps it
        let mut w = crate::voice::preset("Metal Demon").unwrap();
        w.transform.res.ratios = Ratios::Bar;
        let s = toml::to_string(&w).unwrap();
        let back: crate::voice::VoicePatch = toml::from_str(&s).unwrap();
        assert_eq!(back.transform, w.transform);
        // partial tables fill in defaults
        let p: Transform = toml::from_str("blend = 0.5\n[sub]\namount = 0.9\n").unwrap();
        assert_eq!(p.sub.xover, 120.0);
        assert_eq!(p.res, ResParams::default());
    }

    #[test]
    fn fix_clamps_nan_and_out_of_range() {
        let mut t = Transform {
            blend: f32::NAN,
            ..Default::default()
        };
        t.res.decay_ms = 1e9;
        t.dig.bits = -3.0;
        t.sub.octave = 9;
        t.sub.xover = f32::INFINITY;
        t.fix();
        assert_eq!(t.blend, 0.0);
        assert_eq!(t.res.decay_ms, 4000.0);
        assert_eq!(t.dig.bits, 1.0);
        assert_eq!(t.sub.octave, 2);
        assert_eq!(t.sub.xover, 120.0);
    }
}
