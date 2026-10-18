//! Chip FX: an sfxr-style game sound generator (0.2.3.13).
//!
//! A port of sfxr by Tomas Pettersson (DrPetter, 2007, MIT,
//! https://www.drpetter.se/project_sfxr.html) by way of its Rust
//! reimplementation sfxr-rs by Teemu Erkkola (2018, MIT,
//! https://github.com/bzar/sfxr-rs): the same oscillator (square / saw /
//! sine / noise / triangle with frequency slide, slide change, duty sweep,
//! vibrato, one arpeggio jump), attack-sustain-decay envelope with punch,
//! resonant low-pass + high-pass with sweeps, phaser, repeat, 8x
//! supersampling at 44.1 kHz, and the category recipes (Pickup, Laser,
//! Explosion, Power-up, Hit, Jump, Blip) and Randomize / Mutate. Changes for
//! ChipWave: the 44.1 kHz stream is resampled to the engine rate, notes
//! transpose relative to C-4, no allocation (fixed buffers, `Copy` state),
//! seeded PCG dice instead of `rand`, per-parameter locks, and guards
//! against the 0/0 of zero-length envelope stages. MIT notices are in
//! THIRD_PARTY_LICENSES.txt.
use crate::gen::{MacroDef, Rng};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Wave {
    #[default]
    Square,
    Saw,
    Sine,
    Noise,
    Triangle,
}

impl Wave {
    pub const ALL: [Wave; 5] = [
        Wave::Square,
        Wave::Saw,
        Wave::Sine,
        Wave::Noise,
        Wave::Triangle,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Wave::Square => "Square",
            Wave::Saw => "Saw",
            Wave::Sine => "Sine",
            Wave::Noise => "Noise",
            Wave::Triangle => "Triangle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Pickup,
    Laser,
    Explosion,
    PowerUp,
    Hit,
    Jump,
    Blip,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Pickup,
        Category::Laser,
        Category::Explosion,
        Category::PowerUp,
        Category::Hit,
        Category::Jump,
        Category::Blip,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Category::Pickup => "Pickup",
            Category::Laser => "Laser",
            Category::Explosion => "Explosion",
            Category::PowerUp => "Power-up",
            Category::Hit => "Hit",
            Category::Jump => "Jump",
            Category::Blip => "Blip",
        }
    }
}

/// number of sfxr parameters
pub const N: usize = 23;
pub const BASE_FREQ: usize = 0;
pub const FREQ_LIMIT: usize = 1;
pub const FREQ_RAMP: usize = 2;
pub const FREQ_DRAMP: usize = 3;
pub const DUTY: usize = 4;
pub const DUTY_RAMP: usize = 5;
pub const VIB_STRENGTH: usize = 6;
pub const VIB_SPEED: usize = 7;
pub const VIB_DELAY: usize = 8;
pub const ENV_ATTACK: usize = 9;
pub const ENV_SUSTAIN: usize = 10;
pub const ENV_DECAY: usize = 11;
pub const ENV_PUNCH: usize = 12;
pub const LPF_RES: usize = 13;
pub const LPF_FREQ: usize = 14;
pub const LPF_RAMP: usize = 15;
pub const HPF_FREQ: usize = 16;
pub const HPF_RAMP: usize = 17;
pub const PHA_OFFSET: usize = 18;
pub const PHA_RAMP: usize = 19;
pub const REPEAT: usize = 20;
pub const ARP_SPEED: usize = 21;
pub const ARP_MOD: usize = 22;
/// lock bit of the waveform
pub const WAVE_LOCK: u32 = 1 << 31;

const fn p(name: &'static str, lo: f32, tip: &'static str) -> MacroDef {
    MacroDef {
        name,
        lo,
        hi: 1.0,
        exp: false,
        unit: "",
        dice: (0.0, 1.0),
        tip,
    }
}

/// the sfxr parameters in sfxr's order and ranges
pub const PARAMS: [MacroDef; N] = [
    p("Freq", 0.0, "start frequency"),
    p("Min freq", 0.0, "the slide stops (sound cuts) below this"),
    p("Slide", -1.0, "frequency slide down / up"),
    p("Delta slide", -1.0, "change of the slide over time"),
    p("Duty", 0.0, "square duty / saw shape"),
    p("Duty sweep", -1.0, "duty change over time"),
    p("Vib depth", 0.0, "vibrato depth"),
    p("Vib speed", 0.0, "vibrato speed"),
    p(
        "Vib delay",
        0.0,
        "vibrato delay (kept for sfxr compatibility; unused, as in sfxr)",
    ),
    p("Attack", 0.0, "envelope attack"),
    p("Sustain", 0.0, "envelope sustain time"),
    p("Decay", 0.0, "envelope decay"),
    p("Punch", -1.0, "sustain punch (louder start of the sustain)"),
    p("LP res", 0.0, "low-pass resonance"),
    p("LP cutoff", 0.0, "low-pass cutoff (1 = open)"),
    p("LP sweep", -1.0, "low-pass cutoff sweep"),
    p("HP cutoff", 0.0, "high-pass cutoff"),
    p("HP sweep", -1.0, "high-pass cutoff sweep"),
    p("Phaser", -1.0, "phaser offset"),
    p("Phaser sweep", -1.0, "phaser offset sweep"),
    p(
        "Repeat",
        0.0,
        "restart the pitch / filter at this rate (0 = off)",
    ),
    p(
        "Arp speed",
        0.0,
        "when the arpeggio jump happens (0 = late, 1 = off)",
    ),
    p("Arp jump", -1.0, "arpeggio jump size (up > 0, down < 0)"),
];

/// the eight macro knobs (43xy targets) -> parameter index
pub const MACROS: [usize; 8] = [
    BASE_FREQ,
    FREQ_RAMP,
    DUTY,
    ENV_SUSTAIN,
    ENV_DECAY,
    LPF_FREQ,
    PHA_OFFSET,
    REPEAT,
];

fn defaults() -> [f32; N] {
    let mut v = [0.0; N];
    v[BASE_FREQ] = 0.3;
    v[ENV_ATTACK] = 0.4;
    v[ENV_SUSTAIN] = 0.1;
    v[ENV_DECAY] = 0.5;
    v[LPF_FREQ] = 1.0;
    v
}

/// a Chip FX sound (sfxr's parameter set)
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SfxP {
    pub wave: Wave,
    pub p: [f32; N],
    /// bit i locks parameter i; bit 31 locks the wave
    pub locks: u32,
    pub level: f32,
    /// the note transposes the sound (C-4 = as designed)
    pub follow_note: bool,
    /// the last category button (index into `Category::ALL`; 255 = Randomize):
    /// Dice rolls again in it
    pub category: u8,
}

impl Default for SfxP {
    fn default() -> Self {
        SfxP {
            wave: Wave::Square,
            p: defaults(),
            locks: 0,
            level: 1.0,
            follow_note: true,
            category: 0,
        }
    }
}

struct R(Rng);
impl R {
    fn f(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.0.f()
    }
    /// `t`:`f` odds of true (sfxr's rnd(n) == 0)
    fn b(&mut self, t: u32, f: u32) -> bool {
        self.0.next() % (t + f) < t
    }
    fn pick<T: Copy>(&mut self, s: &[T]) -> T {
        s[self.0.next() as usize % s.len()]
    }
}

impl SfxP {
    pub fn fix(&mut self) {
        for (v, d) in self.p.iter_mut().zip(PARAMS.iter()) {
            *v = if v.is_finite() {
                v.clamp(d.lo, d.hi)
            } else {
                0.0
            };
        }
        self.level = if self.level.is_finite() {
            self.level.clamp(0.0, 2.0)
        } else {
            1.0
        };
    }

    /// keep the locked parameters (and wave) of `old`
    fn keep_locked(&mut self, old: &SfxP) {
        for i in 0..N {
            if old.locks & (1 << i) != 0 {
                self.p[i] = old.p[i];
            }
        }
        if old.locks & WAVE_LOCK != 0 {
            self.wave = old.wave;
        }
        self.locks = old.locks;
        self.level = old.level;
        self.follow_note = old.follow_note;
        self.category = old.category;
        self.fix();
    }

    /// a new sound of category `c` (sfxr's generator buttons), unlocked
    /// parameters only
    pub fn category(&mut self, c: Category, seed: u32) {
        let old = *self;
        *self = recipe(c, seed);
        self.keep_locked(&old);
        self.category = Category::ALL.iter().position(|&x| x == c).unwrap_or(0) as u8;
    }

    /// Dice: roll again in the last category (or Randomize)
    pub fn dice(&mut self, seed: u32) {
        match Category::ALL.get(self.category as usize) {
            Some(&c) => self.category(c, seed),
            None => self.randomize(seed),
        }
    }

    /// sfxr's Randomize: anything goes
    pub fn randomize(&mut self, seed: u32) {
        let old = *self;
        let r = &mut R(Rng::new(seed));
        let mut s = SfxP {
            p: [0.0; N],
            ..Default::default()
        };
        let q = |r: &mut R, k: i32| (r.f(0.0, 2.0) - 1.0).powi(k);
        let v = &mut s.p;
        v[BASE_FREQ] = q(r, 2);
        if r.b(1, 1) {
            v[BASE_FREQ] = q(r, 3) + 0.5;
        }
        v[FREQ_RAMP] = q(r, 5);
        if v[BASE_FREQ] > 0.7 && v[FREQ_RAMP] > 0.2 {
            v[FREQ_RAMP] = -v[FREQ_RAMP];
        }
        if v[BASE_FREQ] < 0.2 && v[FREQ_RAMP] < -0.05 {
            v[FREQ_RAMP] = -v[FREQ_RAMP];
        }
        v[FREQ_DRAMP] = q(r, 3);
        v[DUTY] = q(r, 1);
        v[DUTY_RAMP] = q(r, 3);
        v[VIB_STRENGTH] = q(r, 3);
        v[VIB_SPEED] = q(r, 1);
        v[VIB_DELAY] = q(r, 1);
        v[ENV_ATTACK] = q(r, 3);
        v[ENV_SUSTAIN] = q(r, 2);
        v[ENV_DECAY] = q(r, 1);
        v[ENV_PUNCH] = r.f(0.0, 0.8).powi(2);
        if v[ENV_ATTACK] + v[ENV_SUSTAIN] + v[ENV_DECAY] < 0.2 {
            v[ENV_SUSTAIN] += 0.2 + r.f(0.0, 0.3);
            v[ENV_DECAY] += 0.2 + r.f(0.0, 0.3);
        }
        v[LPF_RES] = q(r, 1);
        v[LPF_FREQ] = 1.0 - r.f(0.0, 1.0).powi(3);
        v[LPF_RAMP] = q(r, 3);
        if v[LPF_FREQ] < 0.1 && v[LPF_RAMP] < -0.05 {
            v[LPF_RAMP] = -v[LPF_RAMP];
        }
        v[HPF_FREQ] = r.f(0.0, 1.0).powi(5);
        v[HPF_RAMP] = q(r, 5);
        v[PHA_OFFSET] = q(r, 3);
        v[PHA_RAMP] = q(r, 3);
        v[REPEAT] = q(r, 1);
        v[ARP_SPEED] = q(r, 1);
        v[ARP_MOD] = q(r, 1);
        // sfxr leaves the wave alone on Randomize; here it is rolled too
        // (lock it to keep it)
        s.wave = r.pick(&Wave::ALL);
        // the long attack sfxr's Randomize often rolls makes a drum-machine
        // hit inaudible; keep the cubic shape but cap it
        s.p[ENV_ATTACK] = s.p[ENV_ATTACK].abs().min(0.6);
        *self = s;
        self.keep_locked(&old);
        self.category = 255;
    }

    /// sfxr's Mutate: each unlocked parameter has a 50 % chance to move by up
    /// to ±`amount` of its half range (0.1 = sfxr's ±0.05 on a 0..1 slider)
    pub fn mutate(&mut self, amount: f32, seed: u32) {
        let r = &mut R(Rng::new(seed));
        #[allow(clippy::needless_range_loop)]
        for i in 0..N {
            let (go, x) = (r.b(1, 1), r.f(-1.0, 1.0));
            if i == FREQ_LIMIT || self.locks & (1 << i) != 0 || !go {
                continue;
            }
            let d = &PARAMS[i];
            self.p[i] += x * amount * (d.hi - d.lo) * 0.5;
        }
        self.fix();
    }
}

/// sfxr / sfxr-rs category recipes
pub fn recipe(c: Category, seed: u32) -> SfxP {
    let r = &mut R(Rng::new(seed));
    let mut s = SfxP::default();
    let v = &mut s.p;
    match c {
        Category::Pickup => {
            v[BASE_FREQ] = r.f(0.4, 0.9);
            v[ENV_ATTACK] = 0.0;
            v[ENV_SUSTAIN] = r.f(0.0, 0.1);
            v[ENV_DECAY] = r.f(0.1, 0.5);
            v[ENV_PUNCH] = r.f(0.3, 0.6);
            if r.b(1, 1) {
                v[ARP_SPEED] = r.f(0.5, 0.7);
                v[ARP_MOD] = r.f(0.2, 0.6);
            }
        }
        Category::Laser => {
            s.wave = r.pick(&[
                Wave::Square,
                Wave::Square,
                Wave::Sine,
                Wave::Sine,
                Wave::Triangle,
            ]);
            if r.b(1, 2) {
                v[BASE_FREQ] = r.f(0.3, 0.9);
                v[FREQ_LIMIT] = r.f(0.0, 0.1);
                v[FREQ_RAMP] = r.f(-0.35, -0.65);
            } else {
                v[BASE_FREQ] = r.f(0.5, 1.0);
                v[FREQ_LIMIT] = (v[BASE_FREQ] - r.f(0.2, 0.8)).max(0.2);
                v[FREQ_RAMP] = r.f(-0.15, -0.35);
            }
            if r.b(1, 1) {
                v[DUTY] = r.f(0.0, 0.5);
                v[DUTY_RAMP] = r.f(0.0, 0.2);
            } else {
                v[DUTY] = r.f(0.4, 0.9);
                v[DUTY_RAMP] = r.f(0.0, -0.7);
            }
            v[ENV_ATTACK] = 0.0;
            v[ENV_SUSTAIN] = r.f(0.1, 0.3);
            v[ENV_DECAY] = r.f(0.0, 0.4);
            if r.b(1, 1) {
                v[ENV_PUNCH] = r.f(0.0, 0.3);
            }
            if r.b(1, 2) {
                v[PHA_OFFSET] = r.f(0.0, 0.2);
                v[PHA_RAMP] = -r.f(0.0, 0.2);
            }
            if r.b(1, 1) {
                v[HPF_FREQ] = r.f(0.0, 0.3);
            }
        }
        Category::Explosion => {
            s.wave = Wave::Noise;
            if r.b(1, 1) {
                v[BASE_FREQ] = r.f(0.1, 0.5);
                v[FREQ_RAMP] = r.f(-0.1, 0.3);
            } else {
                v[BASE_FREQ] = r.f(0.2, 0.9);
                v[FREQ_RAMP] = r.f(-0.2, -0.4);
            }
            v[BASE_FREQ] = v[BASE_FREQ].powi(2);
            if r.b(1, 4) {
                v[FREQ_RAMP] = 0.0;
            }
            if r.b(1, 2) {
                v[REPEAT] = r.f(0.3, 0.8);
            }
            v[ENV_ATTACK] = 0.0;
            v[ENV_SUSTAIN] = r.f(0.1, 0.4);
            v[ENV_DECAY] = r.f(0.0, 0.5);
            if r.b(1, 1) {
                v[PHA_OFFSET] = r.f(-0.3, 0.6);
                v[PHA_RAMP] = r.f(-0.3, 0.0);
            }
            v[ENV_PUNCH] = r.f(0.2, 0.8);
            if r.b(1, 1) {
                v[VIB_STRENGTH] = r.f(0.0, 0.7);
                v[VIB_SPEED] = r.f(0.0, 0.6);
            }
            if r.b(1, 2) {
                v[ARP_SPEED] = r.f(0.6, 0.9);
                v[ARP_MOD] = r.f(-0.8, 0.8);
            }
        }
        Category::PowerUp => {
            if r.b(1, 1) {
                s.wave = Wave::Sine;
            } else {
                v[DUTY] = r.f(0.0, 0.6);
            }
            v[BASE_FREQ] = r.f(0.2, 0.5);
            if r.b(1, 1) {
                v[FREQ_RAMP] = r.f(0.1, 0.5);
                v[REPEAT] = r.f(0.4, 0.8);
            } else {
                v[FREQ_RAMP] = r.f(0.05, 0.25);
                if r.b(1, 1) {
                    v[VIB_STRENGTH] = r.f(0.0, 0.7);
                    v[VIB_SPEED] = r.f(0.0, 0.6);
                }
            }
            v[ENV_ATTACK] = 0.0;
            v[ENV_SUSTAIN] = r.f(0.0, 0.4);
            v[ENV_DECAY] = r.f(0.1, 0.5);
        }
        Category::Hit => {
            s.wave = r.pick(&[Wave::Square, Wave::Sine, Wave::Noise]);
            if s.wave == Wave::Square {
                v[DUTY] = r.f(0.0, 0.6);
            }
            v[BASE_FREQ] = r.f(0.2, 0.8);
            v[FREQ_RAMP] = r.f(-0.3, -0.7);
            v[ENV_ATTACK] = 0.0;
            v[ENV_SUSTAIN] = r.f(0.0, 0.1);
            v[ENV_DECAY] = r.f(0.1, 0.3);
            if r.b(1, 1) {
                v[HPF_FREQ] = r.f(0.0, 0.3);
            }
        }
        Category::Jump => {
            v[DUTY] = r.f(0.0, 0.6);
            v[BASE_FREQ] = r.f(0.3, 0.6);
            v[FREQ_RAMP] = r.f(0.1, 0.3);
            v[ENV_ATTACK] = 0.0;
            v[ENV_SUSTAIN] = r.f(0.1, 0.4);
            v[ENV_DECAY] = r.f(0.1, 0.3);
            if r.b(1, 1) {
                v[HPF_FREQ] = r.f(0.0, 0.3);
            }
            if r.b(1, 1) {
                v[LPF_FREQ] = r.f(0.4, 1.0);
            }
        }
        Category::Blip => {
            s.wave = r.pick(&[Wave::Square, Wave::Sine]);
            if s.wave == Wave::Square {
                v[DUTY] = r.f(0.0, 0.6);
            }
            v[BASE_FREQ] = r.f(0.2, 0.6);
            v[ENV_ATTACK] = 0.0;
            v[ENV_SUSTAIN] = r.f(0.1, 0.2);
            v[ENV_DECAY] = r.f(0.0, 0.2);
            v[HPF_FREQ] = 0.1;
        }
    }
    s.fix();
    s
}

/// sfxr's synthesis rate
const SFXR_SR: f32 = 44_100.0;
const C4: f64 = 261.625_58;

/// per-voice Chip FX state (fixed size, `Copy`, allocation-free)
#[derive(Clone, Copy)]
pub struct SfxState {
    started: bool,
    pub done: bool,
    v: [f32; N],
    wave: Wave,
    ratio: f64,
    // oscillator
    phase: u32,
    period: u32,
    fperiod: f64,
    fmaxperiod: f64,
    fslide: f64,
    fdslide: f64,
    duty: f32,
    duty_slide: f32,
    vib_phase: f64,
    vib_speed: f64,
    vib_amp: f64,
    arp_time: i32,
    arp_limit: i32,
    arp_mod: f64,
    noise: [f32; 32],
    nz: u32,
    // envelope
    stage: u8,
    left: u32,
    len: [u32; 3],
    punch: f32,
    // filters
    fltp: f32,
    fltdp: f32,
    fltw: f32,
    fltw_d: f32,
    fltdmp: f32,
    fltphp: f32,
    flthp: f32,
    flthp_d: f32,
    // phaser
    ipp: usize,
    fphase: f32,
    fdphase: f32,
    buf: [f32; 1024],
    // repeat
    rep_time: i32,
    rep_limit: i32,
    // 44.1 kHz -> engine rate
    acc: f32,
    prev: f32,
    cur: f32,
}

impl Default for SfxState {
    fn default() -> Self {
        SfxState {
            started: false,
            done: false,
            v: defaults(),
            wave: Wave::Square,
            ratio: 1.0,
            phase: 0,
            period: 8,
            fperiod: 0.0,
            fmaxperiod: 0.0,
            fslide: 0.0,
            fdslide: 0.0,
            duty: 0.5,
            duty_slide: 0.0,
            vib_phase: 0.0,
            vib_speed: 0.0,
            vib_amp: 0.0,
            arp_time: 0,
            arp_limit: 0,
            arp_mod: 0.0,
            noise: [0.0; 32],
            nz: 0x1234_5679,
            stage: 0,
            left: 0,
            len: [0; 3],
            punch: 0.0,
            fltp: 0.0,
            fltdp: 0.0,
            fltw: 0.0,
            fltw_d: 0.0,
            fltdmp: 0.0,
            fltphp: 0.0,
            flthp: 0.0,
            flthp_d: 0.0,
            ipp: 0,
            fphase: 0.0,
            fdphase: 0.0,
            buf: [0.0; 1024],
            rep_time: 0,
            rep_limit: 0,
            acc: 0.0,
            prev: 0.0,
            cur: 0.0,
        }
    }
}

impl SfxState {
    fn reset_noise(&mut self) {
        for x in self.noise.iter_mut() {
            self.nz ^= self.nz << 13;
            self.nz ^= self.nz >> 17;
            self.nz ^= self.nz << 5;
            *x = self.nz as f32 / u32::MAX as f32 * 2.0 - 1.0;
        }
    }

    /// sfxr's ResetSample(restart = true): pitch, duty, arpeggio and filter
    fn restart(&mut self) {
        let v = self.v;
        let b = v[BASE_FREQ] as f64;
        let lim = v[FREQ_LIMIT] as f64;
        self.fperiod = 100.0 / (b * b + 0.001) / self.ratio;
        self.fmaxperiod = 100.0 / (lim * lim + 0.001) / self.ratio;
        self.fslide = 1.0 - (v[FREQ_RAMP] as f64).powi(3) * 0.01;
        self.fdslide = -(v[FREQ_DRAMP] as f64).powi(3) * 0.000_001;
        self.duty = 0.5 - v[DUTY] * 0.5;
        self.duty_slide = -v[DUTY_RAMP] * 0.000_05;
        let am = v[ARP_MOD] as f64;
        self.arp_mod = if am >= 0.0 {
            1.0 - am * am * 0.9
        } else {
            1.0 + am * am * 10.0
        };
        self.arp_time = 0;
        self.arp_limit = ((1.0 - v[ARP_SPEED]).powi(2) * 20_000.0 + 32.0) as i32;
        if v[ARP_SPEED] >= 1.0 {
            self.arp_limit = 0;
        }
        self.fltp = 0.0;
        self.fltdp = 0.0;
        self.fltw = v[LPF_FREQ].powi(3) * 0.1;
        self.fltw_d = 1.0 + v[LPF_RAMP] * 0.0001;
        self.fltdmp = (5.0 / (1.0 + v[LPF_RES].powi(2) * 20.0) * (0.01 + self.fltw)).min(0.8);
        self.fltphp = 0.0;
        self.flthp = v[HPF_FREQ].powi(2) * 0.1;
        self.flthp_d = 1.0 + v[HPF_RAMP] * 0.0003;
    }

    fn start(&mut self, p: &SfxP, ov: &[f32; 8], freq: f32) {
        self.v = p.p;
        for (k, &o) in ov.iter().enumerate() {
            if o >= 0.0 {
                let i = MACROS[k];
                self.v[i] = PARAMS[i].map(o);
            }
        }
        self.wave = p.wave;
        self.ratio = if p.follow_note && freq > 0.0 {
            (freq as f64 / C4).clamp(1.0 / 64.0, 64.0)
        } else {
            1.0
        };
        self.restart();
        let v = self.v;
        self.len = [
            (v[ENV_ATTACK].powi(2) * 100_000.0) as u32,
            (v[ENV_SUSTAIN].powi(2) * 100_000.0) as u32,
            (v[ENV_DECAY].powi(2) * 100_000.0) as u32,
        ];
        self.punch = v[ENV_PUNCH];
        self.stage = 0;
        self.left = self.len[0];
        self.fphase = v[PHA_OFFSET].powi(2) * 1020.0 * v[PHA_OFFSET].signum();
        self.fdphase = v[PHA_RAMP].powi(2) * v[PHA_RAMP].signum();
        self.phase = 0;
        self.vib_phase = 0.0;
        self.vib_speed = (v[VIB_SPEED] as f64).powi(2) * 0.01;
        self.vib_amp = v[VIB_STRENGTH] as f64 * 0.5;
        self.reset_noise();
        self.rep_time = 0;
        self.rep_limit = if v[REPEAT] == 0.0 {
            0
        } else {
            ((1.0 - v[REPEAT]).powi(2) * 20_000.0 + 32.0) as i32
        };
        self.started = true;
    }

    fn env_vol(&self) -> f32 {
        if self.stage > 2 {
            return 0.0;
        }
        let n = self.len[self.stage as usize];
        let dt = if n == 0 {
            0.0
        } else {
            self.left as f32 / n as f32
        };
        match self.stage {
            0 => 1.0 - dt,
            1 => 1.0 + dt * 2.0 * self.punch,
            _ => dt,
        }
    }

    /// one 44.1 kHz output sample (8 supersamples), as sfxr's SynthSample
    fn step(&mut self) -> f32 {
        self.rep_time += 1;
        if self.rep_limit != 0 && self.rep_time >= self.rep_limit {
            self.rep_time = 0;
            self.restart();
        }
        // oscillator
        self.arp_time += 1;
        if self.arp_limit != 0 && self.arp_time >= self.arp_limit {
            self.arp_limit = 0;
            self.fperiod *= self.arp_mod;
        }
        self.fslide += self.fdslide;
        self.fperiod *= self.fslide;
        if self.fperiod > self.fmaxperiod {
            self.fperiod = self.fmaxperiod;
            // sfxr: a slide below the minimum frequency ends the sound
            if self.v[FREQ_LIMIT] > 0.0 {
                self.done = true;
            }
        }
        self.vib_phase += self.vib_speed;
        let vib = 1.0 + self.vib_phase.sin() * self.vib_amp;
        self.period = ((vib * self.fperiod) as u32).max(8);
        self.duty = (self.duty + self.duty_slide).clamp(0.0, 0.5);
        // envelope
        if self.left > 1 {
            self.left -= 1;
        } else if self.stage <= 2 {
            self.stage += 1;
            self.left = if self.stage <= 2 {
                self.len[self.stage as usize]
            } else {
                0
            };
        }
        if self.stage > 2 {
            self.done = true;
        }
        let env = self.env_vol();
        // phaser
        self.fphase += self.fdphase;
        let iphase = (self.fphase.abs() as usize).min(1023);
        let mut sum = 0.0;
        for _ in 0..8 {
            self.phase += 1;
            if self.phase >= self.period {
                self.phase %= self.period;
                if self.wave == Wave::Noise {
                    self.reset_noise();
                }
            }
            let fp = self.phase as f32 / self.period as f32;
            let x = match self.wave {
                Wave::Square => {
                    if fp < self.duty {
                        0.5
                    } else {
                        -0.5
                    }
                }
                Wave::Saw => 1.0 - fp * 2.0,
                Wave::Sine => (fp * std::f32::consts::TAU).sin(),
                Wave::Noise => self.noise[((fp * 32.0) as usize).min(31)],
                Wave::Triangle => {
                    if fp < 0.5 {
                        4.0 * fp - 1.0
                    } else {
                        3.0 - 4.0 * fp
                    }
                }
            } * env;
            // low-pass
            let pp = self.fltp;
            self.fltw = (self.fltw * self.fltw_d).clamp(0.0, 0.1);
            if self.v[LPF_FREQ] < 1.0 {
                self.fltdp += (x - self.fltp) * self.fltw;
                self.fltdp -= self.fltdp * self.fltdmp;
            } else {
                self.fltp = x;
                self.fltdp = 0.0;
            }
            self.fltp += self.fltdp;
            // high-pass
            if self.flthp_d != 1.0 {
                self.flthp = (self.flthp * self.flthp_d).clamp(0.000_01, 0.1);
            }
            self.fltphp += self.fltp - pp;
            self.fltphp -= self.fltphp * self.flthp;
            let y = self.fltphp;
            // phaser
            self.buf[self.ipp & 1023] = y;
            let out = y + self.buf[(self.ipp + 1024 - iphase) & 1023];
            self.ipp = (self.ipp + 1) & 1023;
            sum += out;
        }
        sum / 8.0
    }

    /// next engine-rate sample; `false` once the sound has ended
    pub fn render(&mut self, p: &SfxP, ov: &[f32; 8], freq: f32, sr: f32) -> (f32, bool) {
        if self.done {
            return (0.0, false);
        }
        if !self.started {
            self.start(p, ov, freq);
            self.cur = self.step();
        }
        self.acc += SFXR_SR / sr.max(1000.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.prev = self.cur;
            self.cur = self.step();
        }
        let y = (self.prev + (self.cur - self.prev) * self.acc) * 1.6;
        let y = y.clamp(-1.0, 1.0) * p.level;
        if !y.is_finite() {
            self.done = true;
            return (0.0, false);
        }
        (y, !self.done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(p: &SfxP, freq: f32, sr: f32, max_s: f32) -> Vec<f32> {
        let mut st = Box::<SfxState>::default();
        let mut v = vec![];
        for _ in 0..(max_s * sr) as usize {
            let (y, alive) = st.render(p, &[-1.0; 8], freq, sr);
            v.push(y);
            if !alive {
                break;
            }
        }
        v
    }

    fn peak(v: &[f32]) -> f32 {
        v.iter().fold(0.0f32, |a, x| a.max(x.abs()))
    }

    /// zero crossings per second of the first `n` samples
    fn zc_hz(v: &[f32], sr: f32, n: usize) -> f32 {
        let s = &v[..n.min(v.len())];
        let c = s.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        c as f32 / (s.len() as f32 / sr)
    }

    #[test]
    fn every_category_makes_a_finite_audible_sound_that_ends() {
        for c in Category::ALL {
            for seed in 1..40 {
                let p = recipe(c, seed);
                for (v, d) in p.p.iter().zip(PARAMS.iter()) {
                    assert!(*v >= d.lo && *v <= d.hi, "{c:?} {seed} {} {v}", d.name);
                }
                let y = render(&p, 261.6256, 48000.0, 8.0);
                assert!(y.iter().all(|x| x.is_finite()));
                let pk = peak(&y);
                assert!(pk > 0.02 && pk <= 1.0, "{c:?} seed {seed}: peak {pk}");
                assert!(y.len() < (7.9 * 48000.0) as usize, "{c:?} {seed} ends");
            }
        }
    }

    #[test]
    fn pitch_matches_sfxr_and_follows_the_note() {
        // a plain square: base 0.3 -> period 100/(0.09+0.001) supersamples
        let mut p = SfxP::default();
        p.p[ENV_ATTACK] = 0.0;
        p.p[ENV_SUSTAIN] = 0.5;
        p.p[ENV_DECAY] = 0.1;
        let expect = 8.0 * 44100.0 / (100.0 / 0.091f32).floor();
        let y = render(&p, 261.6256, 48000.0, 1.0);
        let f = zc_hz(&y, 48000.0, 20000);
        assert!((f / expect - 1.0).abs() < 0.03, "{f} vs {expect}");
        let y = render(&p, 523.2511, 48000.0, 1.0);
        let f2 = zc_hz(&y, 48000.0, 20000);
        assert!((f2 / (2.0 * expect) - 1.0).abs() < 0.03, "octave {f2}");
        p.follow_note = false;
        let y = render(&p, 523.2511, 48000.0, 1.0);
        assert!((zc_hz(&y, 48000.0, 20000) / expect - 1.0).abs() < 0.03);
        // same sound at another engine rate
        let y = render(&p, 0.0, 96000.0, 1.0);
        assert!((zc_hz(&y, 96000.0, 40000) / expect - 1.0).abs() < 0.03);
    }

    #[test]
    fn envelope_length_follows_sfxr() {
        let mut p = SfxP::default();
        p.p[ENV_ATTACK] = 0.0;
        p.p[ENV_SUSTAIN] = 0.3;
        p.p[ENV_DECAY] = 0.4;
        // (0.09 + 0.16) * 100000 samples at 44.1 kHz
        let secs = (0.09 + 0.16) * 100_000.0 / 44_100.0;
        let y = render(&p, 0.0, 48000.0, 3.0);
        let got = y.len() as f32 / 48000.0;
        assert!((got - secs).abs() < 0.01, "{got} vs {secs}");
    }

    #[test]
    fn randomize_mutate_and_locks() {
        let mut a = SfxP::default();
        a.randomize(7);
        let mut b = SfxP::default();
        b.randomize(7);
        assert_eq!(a, b, "seeded");
        b.randomize(8);
        assert_ne!(a, b);
        // locks keep parameters and the wave through every roll
        let mut l = recipe(Category::Laser, 3);
        l.locks = (1 << BASE_FREQ) | (1 << ENV_DECAY) | WAVE_LOCK;
        let keep = (l.p[BASE_FREQ], l.p[ENV_DECAY], l.wave);
        for s in 0..30 {
            l.randomize(s);
            l.category(Category::Explosion, s);
            l.mutate(0.5, s);
            assert_eq!((l.p[BASE_FREQ], l.p[ENV_DECAY], l.wave), keep);
            for (v, d) in l.p.iter().zip(PARAMS.iter()) {
                assert!(*v >= d.lo && *v <= d.hi);
            }
        }
        // Mutate stays close
        let o = recipe(Category::Pickup, 5);
        let mut m = o;
        m.mutate(0.1, 9);
        let moved = o.p.iter().zip(m.p.iter()).filter(|(x, y)| x != y).count();
        assert!(moved > 3);
        #[allow(clippy::needless_range_loop)]
        for i in 0..N {
            let d = &PARAMS[i];
            assert!((o.p[i] - m.p[i]).abs() <= 0.1 * (d.hi - d.lo) * 0.5 + 1e-6);
        }
        assert_eq!(o.p[FREQ_LIMIT], m.p[FREQ_LIMIT], "sfxr never mutates it");
        // randomized sounds are finite and end
        for s in 0..60 {
            let mut r = SfxP::default();
            r.randomize(s);
            let y = render(&r, 261.6256, 48000.0, 10.0);
            assert!(y.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{s}");
        }
    }

    #[test]
    fn extremes_never_blow_up() {
        for k in 0..64u32 {
            let mut p = SfxP::default();
            for (i, d) in PARAMS.iter().enumerate() {
                p.p[i] = if (k >> (i % 6)) & 1 == 1 { d.hi } else { d.lo };
            }
            p.p[ENV_SUSTAIN] = 0.3;
            p.wave = Wave::ALL[k as usize % 5];
            for f in [0.0, 8.0, 261.6, 8000.0, 20000.0] {
                let y = render(&p, f, 44100.0, 3.0);
                assert!(y.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
            }
        }
        let mut p = SfxP::default();
        p.p[0] = f32::NAN;
        p.level = f32::INFINITY;
        p.fix();
        assert!(p.p[0] == 0.0 && p.level == 1.0);
    }

    #[test]
    fn macros_override_their_parameters() {
        let p = recipe(Category::Blip, 2);
        let mut st = Box::<SfxState>::default();
        let mut ov = [-1.0; 8];
        ov[0] = 1.0; // Freq knob to the top
        st.render(&p, &ov, 0.0, 48000.0);
        assert_eq!(st.v[BASE_FREQ], 1.0);
        assert_eq!(st.v[ENV_DECAY], p.p[ENV_DECAY]);
    }
}
