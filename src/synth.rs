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
    /// right-channel filter (stereo patches)
    pub filt_r: Svf,
    pub t: f32,
    pub noise: u32,
    pub lfo_phase: f32,
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
            lfo_phase: 0.0,
        }
    }

    /// stereo render; `bpm` drives tempo-synced LFOs. Allocation-free.
    pub fn render_st(&mut self, p: &Patch, freq: f32, sr: f32, bpm: f32) -> (f32, f32) {
        let dt_s = 1.0 / sr;
        // LFO (-1..1)
        let lv = if p.lfo.active() {
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
        let n = p.unison.clamp(1, 8) as usize;
        let stereo = p.width > 0.0 && n > 1;
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
        let cutoff = p.cutoff * kt * 2f32.powf(p.env_amount * fe * 6.0 + lv * p.lfo.cutoff);
        let mut yl = self.filt.process(sl + s, cutoff, p.resonance, sr, p.filter);
        let mut yr = if stereo {
            self.filt_r
                .process(sr_ + s, cutoff, p.resonance, sr, p.filter)
        } else {
            yl
        };
        if p.drive > 0.0 {
            let g = 1.0 + p.drive * 4.0;
            yl = (yl * g).tanh();
            yr = if stereo { (yr * g).tanh() } else { yl };
        }
        // amp LFO: v = 1 -> full level, v = -1 -> 1 - depth
        let gate = 1.0 - p.lfo.amp.clamp(0.0, 1.0) * (1.0 - (lv + 1.0) * 0.5);
        let g = self.amp.next(&p.amp, dt_s) * p.gain * gate;
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
}
