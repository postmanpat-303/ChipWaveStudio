//! Small subtractive synth: 2 osc (saw/square/pulse/tri/sine) + sub + noise,
//! TPT state-variable filter (LP/HP/BP), amp/filter/pitch envelopes.
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Adsr {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub fn next(&mut self, e: &Adsr, dt: f32) -> f32 {
        match self.stage {
            Stage::Attack => {
                if e.a <= 0.0005 {
                    self.level = 1.0;
                } else {
                    self.level += dt / e.a;
                }
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                // exponential approach to sustain
                let k = if e.d <= 0.0005 {
                    1.0
                } else {
                    1.0 - (-dt * 5.0 / e.d).exp()
                };
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
                let k = if e.r <= 0.0005 {
                    1.0
                } else {
                    1.0 - (-dt * 5.0 / e.r).exp()
                };
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
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        match mode {
            FilterMode::Lp => v2,
            FilterMode::Bp => v1,
            FilterMode::Hp => x - k * v1 - v2,
        }
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
    pub t: f32,
    pub noise: u32,
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
            t: 0.0,
            noise: 0x1234_5678,
        }
    }

    pub fn render(&mut self, p: &Patch, freq: f32, sr: f32) -> f32 {
        let dt_s = 1.0 / sr;
        let pitch_semi = if p.pitch_env.1 > 0.0 {
            p.pitch_env.0 * (-self.t / p.pitch_env.1).exp()
        } else {
            0.0
        };
        self.t += dt_s;
        let f = freq * 2f32.powf(p.osc1_oct as f32 + pitch_semi / 12.0);
        let n = p.unison.clamp(1, 8) as usize;
        let mut s = 0.0;
        for i in 0..n {
            let spread = if n > 1 {
                i as f32 / (n - 1) as f32 * 2.0 - 1.0
            } else {
                0.0
            };
            let fi = f * 2f32.powf(spread * p.detune_cents / 1200.0);
            let dt = (fi / sr).min(0.5);
            s += osc(p.osc1, self.phases[i], dt, p.pulse_width);
            self.phases[i] = (self.phases[i] + dt) % 1.0;
        }
        s *= p.osc1_level / (n as f32).sqrt();
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
        if p.noise_level > 0.0 {
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            s += (self.noise as f32 / u32::MAX as f32 * 2.0 - 1.0) * p.noise_level;
        }
        let fe = self.fenv.next(&p.fenv, dt_s);
        let kt = (f / 261.63).powf(p.key_track);
        let cutoff = p.cutoff * kt * 2f32.powf(p.env_amount * fe * 6.0);
        let mut y = self.filt.process(s, cutoff, p.resonance, sr, p.filter);
        if p.drive > 0.0 {
            y = (y * (1.0 + p.drive * 4.0)).tanh();
        }
        y * self.amp.next(&p.amp, dt_s) * p.gain
    }
}
