//! Small subtractive synth: 2 osc (saw/square/pulse/tri/sine) + sub + noise,
//! TPT state-variable filter (LP/HP/BP), amp/filter/pitch envelopes,
//! stereo unison spread and one LFO (cutoff / amp gate / pitch).
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Wave {
    Saw,
    Square,
    Pulse,
    Triangle,
    Sine,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum FilterMode {
    Lp,
    Hp,
    Bp,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Adsr {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
}

/// One LFO per patch. `sync` > 0 = beats per cycle (tempo-synced), else `rate` Hz.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lfo {
    pub shape: Wave,
    pub rate: f32,
    pub sync: f32,
    /// filter cutoff depth in octaves (+/-)
    pub cutoff: f32,
    /// amplitude depth 0..1 (1 with a square = full trance gate)
    pub amp: f32,
    /// pitch depth in semitones (vibrato)
    pub pitch: f32,
}

impl Default for Lfo {
    fn default() -> Self {
        Self {
            shape: Wave::Sine,
            rate: 1.0,
            sync: 0.0,
            cutoff: 0.0,
            amp: 0.0,
            pitch: 0.0,
        }
    }
}

impl Lfo {
    pub fn active(&self) -> bool {
        self.cutoff != 0.0 || self.amp != 0.0 || self.pitch != 0.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub name: String,
    pub osc1: Wave,
    pub osc1_oct: i32,
    pub osc1_level: f32,
    pub pulse_width: f32,
    pub unison: u32,
    pub detune_cents: f32,
    pub osc2: Option<(Wave, i32, f32)>, // wave, semitones, level
    pub sub_level: f32,
    pub noise_level: f32,
    pub filter: FilterMode,
    pub cutoff: f32,
    pub resonance: f32,
    pub key_track: f32,
    pub env_amount: f32, // octaves
    pub amp: Adsr,
    pub fenv: Adsr,
    /// pitch envelope: semitones at note-on, decay seconds
    pub pitch_env: (f32, f32),
    pub drive: f32,
    pub gain: f32,
    /// stereo spread of the unison voices, 0 (mono) ..= 1 (hard L/R)
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub lfo: Lfo,
    /// metallic source level: six detuned squares at 808 hi-hat ratios
    /// (tuned so C-5 gives the classic 205/304/370/523/540/800 Hz set)
    #[serde(default)]
    pub metal: f32,
    /// velocity -> brightness: cutoff octaves added at full velocity vs silence
    /// (soft notes are darker), 0 = off
    #[serde(default)]
    pub vel_cutoff: f32,
    /// start delay in seconds (0..0.05): plays the patch slightly behind the
    /// grid, e.g. laid-back hats; sample-accurate
    #[serde(default)]
    pub start_delay: f32,
    /// 303-style glide time in seconds (pattern command 3C, or `legato`)
    #[serde(default = "default_glide")]
    pub glide: f32,
    /// legato: a new note while the previous one still sounds glides to
    /// it without retriggering the envelopes (303 slide)
    #[serde(default)]
    pub legato: bool,
    /// accent strength 0..1 (louder, brighter, shorter decay)
    #[serde(default = "default_accent")]
    pub accent: f32,
    /// notes at or above this velocity (volume column, 0..1) are accented;
    /// 0 = only by pattern command 3D
    #[serde(default)]
    pub accent_vel: f32,
    /// filter model: the 12 dB SVF (old songs) or the Moog-style ladder
    #[serde(default)]
    pub filter_model: crate::ladder::FilterModel,
    /// ladder slope: 12 dB/oct instead of 24 (the SVF is always 12 dB)
    #[serde(default)]
    pub ladder_12db: bool,
}

pub fn default_glide() -> f32 {
    0.06
}

pub fn default_accent() -> f32 {
    0.6
}

/// Per-note filter overrides from the pattern (commands 38-3B).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiltMod {
    /// absolute cutoff in Hz replacing the patch cutoff (0 = patch)
    pub cut_abs: f32,
    /// cutoff offset in octaves (slides)
    pub cut_oct: f32,
    /// resonance 0..1 replacing the patch resonance (< 0 = patch)
    pub res: f32,
}

impl Default for FiltMod {
    fn default() -> Self {
        Self {
            cut_abs: 0.0,
            cut_oct: 0.0,
            res: -1.0,
        }
    }
}

impl Patch {
    /// Copy every sound parameter but not the name (allocation-free, used on
    /// the audio path). Exhaustive destructure: a new field must be added here.
    pub fn copy_sound_from(&mut self, o: &Patch) {
        let Patch {
            name: _,
            osc1,
            osc1_oct,
            osc1_level,
            pulse_width,
            unison,
            detune_cents,
            osc2,
            sub_level,
            noise_level,
            filter,
            cutoff,
            resonance,
            key_track,
            env_amount,
            amp,
            fenv,
            pitch_env,
            drive,
            gain,
            width,
            lfo,
            metal,
            vel_cutoff,
            start_delay,
            glide,
            legato,
            accent,
            accent_vel,
            filter_model,
            ladder_12db,
        } = o;
        *self = Patch {
            name: std::mem::take(&mut self.name),
            osc1: *osc1,
            osc1_oct: *osc1_oct,
            osc1_level: *osc1_level,
            pulse_width: *pulse_width,
            unison: *unison,
            detune_cents: *detune_cents,
            osc2: *osc2,
            sub_level: *sub_level,
            noise_level: *noise_level,
            filter: *filter,
            cutoff: *cutoff,
            resonance: *resonance,
            key_track: *key_track,
            env_amount: *env_amount,
            amp: *amp,
            fenv: *fenv,
            pitch_env: *pitch_env,
            drive: *drive,
            gain: *gain,
            width: *width,
            lfo: *lfo,
            metal: *metal,
            vel_cutoff: *vel_cutoff,
            start_delay: *start_delay,
            glide: *glide,
            legato: *legato,
            accent: *accent,
            accent_vel: *accent_vel,
            filter_model: *filter_model,
            ladder_12db: *ladder_12db,
        };
    }
}

impl Default for Patch {
    fn default() -> Self {
        Self {
            name: "Init".into(),
            osc1: Wave::Saw,
            osc1_oct: 0,
            osc1_level: 0.8,
            pulse_width: 0.5,
            unison: 1,
            detune_cents: 0.0,
            osc2: None,
            sub_level: 0.0,
            noise_level: 0.0,
            filter: FilterMode::Lp,
            cutoff: 4000.0,
            resonance: 0.2,
            key_track: 0.0,
            env_amount: 0.0,
            amp: Adsr {
                a: 0.002,
                d: 0.3,
                s: 0.7,
                r: 0.15,
            },
            fenv: Adsr {
                a: 0.0,
                d: 0.3,
                s: 0.0,
                r: 0.1,
            },
            pitch_env: (0.0, 0.05),
            drive: 0.0,
            gain: 0.8,
            width: 0.0,
            lfo: Lfo::default(),
            metal: 0.0,
            vel_cutoff: 0.0,
            start_delay: 0.0,
            glide: default_glide(),
            legato: false,
            accent: default_accent(),
            accent_vel: 0.0,
            filter_model: Default::default(),
            ladder_12db: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

#[derive(Clone, Copy)]
pub struct Env {
    stage: Stage,
    level: f32,
}

impl Env {
    pub fn new() -> Self {
        Self {
            stage: Stage::Attack,
            level: 0.0,
        }
    }
    pub fn release(&mut self) {
        if self.stage != Stage::Off {
            self.stage = Stage::Release;
        }
    }
    pub fn done(&self) -> bool {
        self.stage == Stage::Off
    }
    /// still held (not released)
    pub fn gate(&self) -> bool {
        !matches!(self.stage, Stage::Release | Stage::Off)
    }
    pub fn next(&mut self, e: &Adsr, dt: f32) -> f32 {
        match self.stage {
            Stage::Attack => {
                // at least 1.5 ms attack so note starts never click
                self.level += dt / e.a.max(0.0015);
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                // exponential approach to sustain
                let k = 1.0 - (-dt * 5.0 / e.d.max(0.005)).exp();
                self.level += (e.s - self.level) * k;
                if (self.level - e.s).abs() < 1e-4 {
                    self.level = e.s;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                self.level = e.s;
                if e.s <= 1e-4 {
                    self.stage = Stage::Off;
                }
            }
            Stage::Release => {
                // at least ~5 ms release ramp (declick on note-off / stop)
                let k = 1.0 - (-dt * 5.0 / e.r.max(0.008)).exp();
                self.level -= self.level * k;
                if self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = Stage::Off;
                }
            }
            Stage::Off => self.level = 0.0,
        }
        self.level
    }
}

/// Zavalishin TPT SVF
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}
impl Svf {
    pub fn process(&mut self, x: f32, cutoff: f32, res: f32, sr: f32, mode: FilterMode) -> f32 {
        let fc = cutoff.clamp(20.0, sr * 0.45);
        let g = (std::f32::consts::PI * fc / sr).tan();
        let k = 2.0 - 1.98 * res.clamp(0.0, 1.0);
        let (lp, bp, hp) = self.process_gk(x, g, k);
        match mode {
            FilterMode::Lp => lp,
            FilterMode::Bp => bp,
            FilterMode::Hp => hp,
        }
    }

    /// One step with precomputed coefficients: `g` = tan(pi fc / sr),
    /// `k` = 1 / Q. Returns (low-pass, band-pass, high-pass); the band-pass
    /// peaks at 1 / k (multiply by k for unity peak gain).
    #[inline]
    pub fn process_gk(&mut self, x: f32, g: f32, k: f32) -> (f32, f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - k * v1 - v2)
    }
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

pub fn osc(w: Wave, phase: f32, dt: f32, pw: f32) -> f32 {
    match w {
        Wave::Sine => (phase * std::f32::consts::TAU).sin(),
        Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
        Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Wave::Square | Wave::Pulse => {
            let pw = if w == Wave::Square {
                0.5
            } else {
                pw.clamp(0.05, 0.95)
            };
            let mut v = if phase < pw { 1.0 } else { -1.0 };
            v += poly_blep(phase, dt);
            v -= poly_blep((phase - pw + 1.0) % 1.0, dt);
            v
        }
    }
}

#[derive(Clone)]
pub struct SynthVoice {
    pub phases: [f32; 8],
    pub phase2: f32,
    pub sub_phase: f32,
    pub amp: Env,
    pub fenv: Env,
    pub filt: Svf,
    /// right-channel filter (stereo patches)
    pub filt_r: Svf,
    pub t: f32,
    pub noise: u32,
    /// second noise generator for the right channel (stereo noise)
    pub noise_r: u32,
    pub lfo_phase: f32,
    pub metal_ph: [f32; 6],
    /// note velocity 0..1 (for `vel_cutoff`)
    pub vel: f32,
    /// per-note amp decay override in seconds (effect 08), 0 = patch decay
    pub decay: f32,
    /// time waited so far for `start_delay`
    pub waited: f32,
    /// vowel generator state (Voice instruments only)
    pub vox: crate::voice::VoxState,
    /// accent of this note 0..1 (pattern 3D or velocity threshold)
    pub accent: f32,
    /// pattern filter overrides (38-3B), set by the engine every frame
    pub fmod: FiltMod,
    /// Voice Transform state and its pattern overrides (3F-42)
    pub tf: crate::transform::TransformState,
    pub tfo: crate::transform::Overrides,
    /// ladder filter state (left / right)
    pub lad: crate::ladder::Ladder,
    pub lad_r: crate::ladder::Ladder,
    /// Generator state and its pattern macro overrides (43xy)
    pub gen: crate::gen::GenState,
    pub gmo: [f32; 8],
}

/// 808 hi-hat oscillator ratios (205.3, 304.4, 369.6, 522.7, 540, 800 Hz)
const METAL: [f32; 6] = [1.0, 1.4827, 1.8003, 2.5461, 2.6303, 3.8967];

fn xorshift(n: &mut u32) -> f32 {
    *n ^= *n << 13;
    *n ^= *n >> 17;
    *n ^= *n << 5;
    *n as f32 / u32::MAX as f32 * 2.0 - 1.0
}

impl SynthVoice {
    pub fn new() -> Self {
        Self {
            phases: [0.0; 8],
            phase2: 0.0,
            sub_phase: 0.0,
            amp: Env::new(),
            fenv: Env::new(),
            filt: Svf::default(),
            filt_r: Svf::default(),
            t: 0.0,
            noise: 0x1234_5678,
            noise_r: 0x9E37_79B9,
            lfo_phase: 0.0,
            metal_ph: [0.0; 6],
            vel: 1.0,
            decay: 0.0,
            waited: 0.0,
            vox: crate::voice::VoxState::new(),
            accent: 0.0,
            fmod: FiltMod::default(),
            tf: crate::transform::TransformState::default(),
            tfo: crate::transform::NO_OVERRIDES,
            lad: Default::default(),
            lad_r: Default::default(),
            gen: Default::default(),
            gmo: [-1.0; 8],
        }
    }

    /// a fresh voice whose noise differs from the last hit (no "machine gun"
    /// repetition of the exact same noise burst on every drum hit)
    pub fn seeded(seed: u32) -> Self {
        let mut v = Self::new();
        v.noise = seed | 1;
        v.noise_r = seed.rotate_left(16) ^ 0x9E37_79B9 | 1;
        v.vox.seed(seed.rotate_left(7) ^ 0x3C6E_F372);
        v.gen.seed(seed.rotate_left(3) ^ 0xBB67_AE85);
        v
    }

    /// Generator instrument: the generator makes the sound (with its own
    /// envelopes); `p` (normally `gp.base`, or its automated copy) adds the
    /// optional filter (SVF / ladder, pattern 38-3B work), the amp envelope
    /// for note-offs and the gain. Allocation-free.
    pub fn render_gen(
        &mut self,
        p: &Patch,
        gp: &crate::gen::GenPatch,
        freq: f32,
        sr: f32,
        _bpm: f32,
    ) -> (f32, f32) {
        let dt_s = 1.0 / sr;
        if self.waited < p.start_delay {
            self.waited += dt_s;
            return (0.0, 0.0);
        }
        let gmo = self.gmo;
        let own = gp.engine.own_env();
        self.gen.gate = self.amp.gate();
        let (l, r, stereo) = self.gen.render(gp, &gmo, freq, sr);
        let fe = self.fenv.next(&p.fenv, dt_s);
        let (yl, yr) = if gp.filter_on {
            let fm = self.fmod;
            let base = if fm.cut_abs > 0.0 {
                fm.cut_abs
            } else {
                p.cutoff
            };
            let res = if fm.res >= 0.0 { fm.res } else { p.resonance };
            let kt = (freq.max(1.0) / 261.63).powf(p.key_track);
            let cutoff = base
                * kt
                * 2f32.powf(p.env_amount * fe * 6.0 + p.vel_cutoff * (self.vel - 1.0) + fm.cut_oct);
            self.filter(p, l, r, stereo, cutoff, res, sr)
        } else {
            (l, r)
        };
        let acc = self.accent.clamp(0.0, 1.0);
        // own-envelope generators: the amp envelope only carries the gate
        let env = if own {
            1.0
        } else {
            self.amp.next(&p.amp, dt_s)
        };
        let g = env * p.gain * (1.0 + 0.5 * acc);
        (yl * g, yr * g)
    }

    /// stereo render; `bpm` drives tempo-synced LFOs. Allocation-free.
    pub fn render_st(&mut self, p: &Patch, freq: f32, sr: f32, bpm: f32) -> (f32, f32) {
        self.render_with(p, None, freq, sr, bpm)
    }

    /// Voice (glitch) instrument: the vowel generator replaces the
    /// oscillators; `p` (normally `vp.base`, or its automated copy) gives
    /// the envelopes, filter, drive, pitch env and LFO. Allocation-free.
    pub fn render_voice(
        &mut self,
        p: &Patch,
        vp: &crate::voice::VoicePatch,
        freq: f32,
        sr: f32,
        bpm: f32,
    ) -> (f32, f32) {
        self.render_with(p, Some(vp), freq, sr, bpm)
    }

    /// the patch filter (SVF or ladder) on one or two channels
    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn filter(
        &mut self,
        p: &Patch,
        l: f32,
        r: f32,
        stereo: bool,
        cutoff: f32,
        res: f32,
        sr: f32,
    ) -> (f32, f32) {
        use crate::ladder::FilterModel;
        match p.filter_model {
            FilterModel::Svf => {
                let yl = self.filt.process(l, cutoff, res, sr, p.filter);
                let yr = if stereo {
                    self.filt_r.process(r, cutoff, res, sr, p.filter)
                } else {
                    yl
                };
                (yl, yr)
            }
            m => {
                let v = m == FilterModel::Vintage;
                let p4 = !p.ladder_12db;
                let yl = self.lad.process(l, cutoff, res, sr, p.filter, p4, v);
                let yr = if stereo {
                    self.lad_r.process(r, cutoff, res, sr, p.filter, p4, v)
                } else {
                    yl
                };
                (yl, yr)
            }
        }
    }

    fn render_with(
        &mut self,
        p: &Patch,
        vp: Option<&crate::voice::VoicePatch>,
        freq: f32,
        sr: f32,
        bpm: f32,
    ) -> (f32, f32) {
        let dt_s = 1.0 / sr;
        if self.waited < p.start_delay {
            self.waited += dt_s;
            return (0.0, 0.0);
        }
        // LFO (-1..1)
        let lv = if p.lfo.active() || vp.is_some_and(|v| v.lfo_used()) {
            let ph = self.lfo_phase;
            let v = match p.lfo.shape {
                Wave::Sine => (ph * std::f32::consts::TAU).sin(),
                Wave::Triangle => 1.0 - 4.0 * (ph - 0.5).abs(),
                Wave::Saw => 2.0 * ph - 1.0,
                Wave::Square | Wave::Pulse => {
                    if ph < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
            };
            let hz = if p.lfo.sync > 0.0 {
                bpm / 60.0 / p.lfo.sync
            } else {
                p.lfo.rate
            };
            self.lfo_phase = (ph + hz.max(0.0) * dt_s) % 1.0;
            v
        } else {
            0.0
        };
        let pitch_semi = if p.pitch_env.1 > 0.0 {
            p.pitch_env.0 * (-self.t / p.pitch_env.1).exp()
        } else {
            0.0
        };
        self.t += dt_s;
        let f = freq * 2f32.powf(p.osc1_oct as f32 + (pitch_semi + lv * p.lfo.pitch) / 12.0);
        let mut sub = 0.0;
        let (sl, s, sr_, s_r, stereo) = if let Some(vp) = vp {
            let t = &vp.transform;
            if t.active(&self.tfo) || self.tf.running() {
                // formant drop follows the blend target (the formants glide)
                let b = if self.tfo[0] >= 0.0 {
                    self.tfo[0]
                } else {
                    t.blend
                };
                self.vox.drop = 2f32.powf(-t.sub.formant_drop * b / 12.0);
                let v = self.vox.render(vp, f, sr, bpm, lv);
                let (l, r, sb) = self.tf.process(t, &self.tfo, v, f, sr);
                sub = sb;
                (l, 0.0, r, 0.0, true)
            } else {
                let v = self.vox.render(vp, f, sr, bpm, lv);
                (v, 0.0, v, 0.0, false)
            }
        } else {
            let n = p.unison.clamp(1, 8) as usize;
            // stereo: spread unison voices, or decorrelated noise for drums
            let st_noise = p.width > 0.0 && p.noise_level > 0.0;
            let stereo = p.width > 0.0 && (n > 1 || st_noise);
            let (mut sl, mut sr_) = (0.0, 0.0);
            for i in 0..n {
                let spread = if n > 1 {
                    i as f32 / (n - 1) as f32 * 2.0 - 1.0
                } else {
                    0.0
                };
                let fi = f * 2f32.powf(spread * p.detune_cents / 1200.0);
                let dt = (fi / sr).min(0.5);
                let v = osc(p.osc1, self.phases[i], dt, p.pulse_width);
                if stereo {
                    // alternate sides so detune and pan are not correlated
                    let pan = if i % 2 == 0 { spread } else { -spread } * p.width.min(1.0);
                    sl += v * (1.0 - pan).sqrt();
                    sr_ += v * (1.0 + pan).sqrt();
                } else {
                    sl += v;
                }
                self.phases[i] = (self.phases[i] + dt) % 1.0;
            }
            let norm = p.osc1_level / (n as f32).sqrt();
            sl *= norm;
            sr_ = if stereo { sr_ * norm } else { sl };
            let mut s = 0.0;
            let mut s_r = 0.0;
            if let Some((w, semi, lvl)) = p.osc2 {
                let f2 = f * 2f32.powf(semi as f32 / 12.0);
                let dt = (f2 / sr).min(0.5);
                s += osc(w, self.phase2, dt, p.pulse_width) * lvl;
                self.phase2 = (self.phase2 + dt) % 1.0;
            }
            if p.sub_level > 0.0 {
                s += (self.sub_phase * std::f32::consts::TAU).sin() * p.sub_level;
                self.sub_phase = (self.sub_phase + f * 0.5 / sr) % 1.0;
            }
            if p.metal > 0.0 {
                let base = f * (205.3 / 523.25);
                let mut m = 0.0;
                for (ph, r) in self.metal_ph.iter_mut().zip(METAL) {
                    let dt = (base * r / sr).min(0.5);
                    m += osc(Wave::Square, *ph, dt, 0.5);
                    *ph = (*ph + dt) % 1.0;
                }
                s += m * (p.metal / 6.0);
            }
            s_r += s;
            if p.noise_level > 0.0 {
                let nl = xorshift(&mut self.noise);
                s += nl * p.noise_level;
                if st_noise {
                    // partly independent right channel, equal power
                    let w = p.width.min(1.0) * 0.7;
                    let nr = xorshift(&mut self.noise_r);
                    let k = 1.0 / ((1.0 - w) * (1.0 - w) + w * w).sqrt();
                    s_r += ((1.0 - w) * nl + w * nr) * k * p.noise_level;
                } else {
                    s_r += nl * p.noise_level;
                }
            }
            (sl, s, sr_, s_r, stereo)
        };
        // accent (303): shorter filter + amp decay, more open filter, louder
        let acc = self.accent.clamp(0.0, 1.0);
        let fe = if acc > 0.0 {
            let fenv = Adsr {
                d: p.fenv.d * (1.0 - 0.5 * acc),
                ..p.fenv
            };
            self.fenv.next(&fenv, dt_s)
        } else {
            self.fenv.next(&p.fenv, dt_s)
        };
        let kt = (f / 261.63).powf(p.key_track);
        let fm = self.fmod;
        let base = if fm.cut_abs > 0.0 {
            fm.cut_abs
        } else {
            p.cutoff
        };
        let res = if fm.res >= 0.0 { fm.res } else { p.resonance };
        let cutoff = base
            * kt
            * 2f32.powf(
                p.env_amount * fe * 6.0
                    + lv * p.lfo.cutoff
                    + p.vel_cutoff * (self.vel - 1.0)
                    + fm.cut_oct
                    + acc * (0.6 + 1.6 * fe),
            );
        let (mut yl, mut yr) = self.filter(p, sl + s, sr_ + s_r, stereo, cutoff, res, sr);
        if p.drive > 0.0 {
            let g = 1.0 + p.drive * 4.0;
            yl = (yl * g).tanh();
            yr = if stereo { (yr * g).tanh() } else { yl };
        }
        // the Transform's sub joins after the filter and drive (clean, mono)
        yl += sub;
        yr += sub;
        // amp LFO: v = 1 -> full level, v = -1 -> 1 - depth
        let gate = 1.0 - p.lfo.amp.clamp(0.0, 1.0) * (1.0 - (lv + 1.0) * 0.5);
        let amp = if self.decay > 0.0 {
            Adsr {
                d: self.decay,
                ..p.amp
            }
        } else if acc > 0.0 {
            Adsr {
                d: p.amp.d * (1.0 - 0.4 * acc),
                ..p.amp
            }
        } else {
            p.amp
        };
        let g = self.amp.next(&amp, dt_s) * p.gain * gate * (1.0 + 0.7 * acc);
        (yl * g, yr * g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(p: &Patch, secs: f32, bpm: f32) -> Vec<(f32, f32)> {
        let mut v = SynthVoice::new();
        (0..(secs * 48000.0) as usize)
            .map(|_| v.render_st(p, 220.0, 48000.0, bpm))
            .collect()
    }

    #[test]
    fn mono_patch_is_identical_on_both_sides() {
        let p = Patch {
            unison: 5,
            detune_cents: 20.0,
            ..Default::default()
        };
        assert!(run(&p, 0.2, 120.0).iter().all(|(l, r)| l == r));
    }

    #[test]
    fn width_spreads_unison_voices_in_stereo() {
        let p = Patch {
            unison: 7,
            detune_cents: 25.0,
            width: 1.0,
            amp: Adsr {
                a: 0.0,
                d: 0.1,
                s: 1.0,
                r: 0.1,
            },
            ..Default::default()
        };
        let o = run(&p, 0.5, 120.0);
        let (mut m, mut s) = (0.0f64, 0.0f64);
        for (l, r) in &o[4800..] {
            m += ((l + r) as f64 / 2.0).powi(2);
            s += ((l - r) as f64 / 2.0).powi(2);
        }
        let side_db = 10.0 * (s / m).log10();
        assert!(side_db > -12.0, "side/mid {side_db:.1} dB");
        // same loudness as the mono version (within 3 dB)
        let mono = run(
            &Patch {
                width: 0.0,
                ..p.clone()
            },
            0.5,
            120.0,
        );
        let e = |o: &[(f32, f32)]| o.iter().map(|(l, r)| (l * l + r * r) as f64).sum::<f64>();
        let d = 10.0 * (e(&o) / e(&mono)).log10();
        assert!(d.abs() < 3.0, "{d:.1} dB");
    }

    #[test]
    fn tempo_synced_square_lfo_gates_the_amp() {
        let p = Patch {
            amp: Adsr {
                a: 0.0,
                d: 0.1,
                s: 1.0,
                r: 0.1,
            },
            lfo: Lfo {
                shape: Wave::Square,
                sync: 0.25, // a 16th at any tempo
                amp: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // 120 BPM: 16th = 125 ms, open for the first 62.5 ms of each
        let o = run(&p, 0.5, 120.0);
        let rms = |a: usize, b: usize| {
            (o[a..b].iter().map(|(l, _)| l * l).sum::<f32>() / (b - a) as f32).sqrt()
        };
        let open = rms(6000 + 300, 6000 + 2700); // 125..181 ms
        let shut = rms(6000 + 3300, 6000 + 5700); // 194..244 ms
        assert!(open > 0.05 && shut < 1e-4, "open {open} shut {shut}");
    }

    #[test]
    fn lfo_moves_the_filter() {
        let base = Patch {
            cutoff: 400.0,
            amp: Adsr {
                a: 0.0,
                d: 0.1,
                s: 1.0,
                r: 0.1,
            },
            ..Default::default()
        };
        let wob = Patch {
            lfo: Lfo {
                rate: 2.0,
                cutoff: 3.0,
                ..Default::default()
            },
            ..base.clone()
        };
        let a = run(&base, 0.3, 120.0);
        let b = run(&wob, 0.3, 120.0);
        // at the LFO peak (125 ms) the cutoff is 3 octaves up: much brighter
        let hf = |o: &[(f32, f32)]| {
            o[5800..6200]
                .windows(2)
                .map(|w| (w[1].0 - w[0].0).powi(2))
                .sum::<f32>()
        };
        assert!(hf(&b) > 2.0 * hf(&a), "{} vs {}", hf(&b), hf(&a));
    }

    fn hat_patch() -> Patch {
        Patch {
            osc1_level: 0.0,
            noise_level: 0.8,
            metal: 0.6,
            filter: FilterMode::Hp,
            cutoff: 6000.0,
            amp: Adsr {
                a: 0.0,
                d: 0.1,
                s: 0.0,
                r: 0.02,
            },
            ..Default::default()
        }
    }

    #[test]
    fn metal_source_has_the_808_partials() {
        // no noise, no filter: the 6 squares at C-5 (523.25 Hz) -> 205.3 Hz base
        let p = Patch {
            noise_level: 0.0,
            filter: FilterMode::Lp,
            cutoff: 18000.0,
            amp: Adsr {
                a: 0.0,
                d: 1.0,
                s: 1.0,
                r: 0.1,
            },
            ..hat_patch()
        };
        let mut v = SynthVoice::new();
        let o: Vec<f32> = (0..48000)
            .map(|_| v.render_st(&p, 523.25, 48000.0, 120.0).0)
            .collect();
        // Goertzel-style correlation at a partial vs a frequency between partials
        let mag = |hz: f32| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, x) in o.iter().enumerate() {
                let w = std::f64::consts::TAU * hz as f64 * i as f64 / 48000.0;
                re += *x as f64 * w.cos();
                im += *x as f64 * w.sin();
            }
            (re * re + im * im).sqrt()
        };
        for hz in [205.3, 304.4, 540.0, 800.0] {
            assert!(mag(hz) > 20.0 * mag(250.0), "{hz} Hz");
        }
    }

    #[test]
    fn noise_width_makes_hats_stereo_and_mono_stays_mono() {
        let side = |p: &Patch| {
            let o = run(p, 0.1, 120.0);
            let (mut m, mut s) = (0.0f64, 0.0f64);
            for (l, r) in &o {
                m += ((l + r) as f64 / 2.0).powi(2);
                s += ((l - r) as f64 / 2.0).powi(2);
            }
            10.0 * (s.max(1e-30) / m).log10()
        };
        assert!(side(&hat_patch()) < -100.0, "width 0 = mono");
        let w = side(&Patch {
            width: 1.0,
            ..hat_patch()
        });
        assert!((-12.0..-2.0).contains(&w), "side/mid {w:.1} dB");
    }

    #[test]
    fn seeded_voices_give_different_noise_bursts() {
        let p = hat_patch();
        let burst = |seed: u32| {
            let mut v = SynthVoice::seeded(seed);
            (0..2000)
                .map(|_| v.render_st(&p, 523.25, 48000.0, 120.0).0)
                .collect::<Vec<f32>>()
        };
        assert_ne!(burst(1), burst(2));
        assert_eq!(burst(7), burst(7));
    }

    #[test]
    fn soft_notes_are_darker_with_velocity_to_cutoff() {
        let p = Patch {
            filter: FilterMode::Lp,
            cutoff: 8000.0,
            vel_cutoff: 3.0,
            metal: 0.0,
            ..hat_patch()
        };
        let hf = |vel: f32| {
            let mut v = SynthVoice::new();
            v.vel = vel;
            let o: Vec<f32> = (0..2400)
                .map(|_| v.render_st(&p, 523.25, 48000.0, 120.0).0)
                .collect();
            let e: f32 = o.iter().map(|x| x * x).sum();
            let d: f32 = o.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
            d / e // brightness: derivative energy per energy
        };
        assert!(hf(1.0) > 1.5 * hf(0.3), "{} vs {}", hf(1.0), hf(0.3));
    }

    #[test]
    fn per_note_decay_override_shortens_the_note() {
        let p = Patch {
            amp: Adsr {
                a: 0.0,
                d: 0.5,
                s: 0.0,
                r: 0.02,
            },
            ..hat_patch()
        };
        let tail = |decay: f32| {
            let mut v = SynthVoice::new();
            v.decay = decay;
            (0..4800)
                .map(|_| v.render_st(&p, 523.25, 48000.0, 120.0).0)
                .skip(3600)
                .map(|x| x * x)
                .sum::<f32>()
        };
        assert!(tail(0.03) < 0.01 * tail(0.0));
    }

    #[test]
    fn start_delay_plays_the_note_late_by_exactly_that_time() {
        let p = Patch {
            start_delay: 0.02,
            ..hat_patch()
        };
        let mut v = SynthVoice::new();
        let o: Vec<f32> = (0..4800)
            .map(|_| v.render_st(&p, 523.25, 48000.0, 120.0).0)
            .collect();
        let first = o.iter().position(|x| x.abs() > 1e-6).unwrap();
        assert!((959..=962).contains(&first), "{first}");
    }
}
