//! The synth engine: voices, modulation, effects. Independent of the plugin wrapper so it can be
//! rendered offline in tests.
use super::dsp::*;
use super::fx::*;
use super::{Dest, LfoWave, SubOct, Wave};

pub const NUM_VOICES: usize = 16;
const CTRL: usize = 16; // control-rate block size
const ENS_OFFS: [f32; 7] = [-1.0, -0.62, -0.27, 0.0, 0.27, 0.62, 1.0];
const ENS_GAIN: [f32; 7] = [0.75, 0.85, 0.95, 1.0, 0.95, 0.85, 0.75];
const VOICE_GAIN: f32 = 0.74;
/// Time constant of the parameter smoothers (seconds).
const SMOOTH_S: f32 = 0.02;

#[derive(Clone, Copy)]
pub struct LayerSnap {
    /// Audible: power on, not muted, and soloed if anything is soloed.
    pub on: bool,
    pub wave: Wave,
    pub oct: i32,
    pub det: f32,
    pub tone: f32,
    pub lvl: f32,
    pub pan: f32,
}

/// Plain parameter values for one block (CC overrides already applied).
#[derive(Clone, Copy)]
pub struct Snap {
    pub layers: [LayerSnap; 3],
    /// Undertones audible (power, mute, solo).
    pub und: bool,
    pub sub_oct: SubOct,
    pub sub_lvl: f32,
    pub drone_lvl: f32,
    pub drone_tone: f32,
    pub cutoff: f32,
    pub reso: f32,
    pub f_env: f32,
    pub f_key: f32,
    pub f_24: bool,
    pub f_adsr: [f32; 4],
    pub a_adsr: [f32; 4],
    pub vel: f32,
    pub lfo: [(LfoWave, f32, f32, Dest); 2],
    pub me: (f32, f32, f32, Dest),
    pub wheel_vib: f32,
    pub bend_range: f32,
    pub cho: (bool, f32, f32, f32),
    pub echo: (bool, f32, f32, f32, f32),
    pub rev: (bool, f32, f32, f32),
    pub master: f32,
    /// polyphony limit (4..=16)
    pub voices: usize,
}

#[inline]
fn glide(cur: &mut f32, target: f32, a: f32) {
    *cur += (target - *cur) * a;
    if (target - *cur).abs() < 1e-6 {
        *cur = target;
    }
}

impl Snap {
    /// Move the smoothed values `self` one control tick toward `t` (coefficient `a`). Discrete
    /// choices switch at once; continuous values glide so live edits never zipper.
    fn track(&mut self, t: &Snap, a: f32) {
        for (c, tl) in self.layers.iter_mut().zip(t.layers.iter()) {
            c.on = tl.on;
            c.wave = tl.wave;
            c.oct = tl.oct;
            glide(&mut c.det, tl.det, a);
            glide(&mut c.tone, tl.tone, a);
            glide(&mut c.lvl, if tl.on { tl.lvl } else { 0.0 }, a);
            glide(&mut c.pan, tl.pan, a);
        }
        self.und = t.und;
        self.sub_oct = t.sub_oct;
        glide(&mut self.sub_lvl, if t.und { t.sub_lvl } else { 0.0 }, a);
        glide(
            &mut self.drone_lvl,
            if t.und { t.drone_lvl } else { 0.0 },
            a,
        );
        glide(&mut self.drone_tone, t.drone_tone, a);
        // cutoff glides in octaves
        let lc = self.cutoff.max(1.0).log2();
        let lt = t.cutoff.max(1.0).log2();
        self.cutoff = (lc + (lt - lc) * a).exp2();
        if (lt - lc).abs() < 1e-5 {
            self.cutoff = t.cutoff;
        }
        glide(&mut self.reso, t.reso, a);
        glide(&mut self.f_env, t.f_env, a);
        glide(&mut self.f_key, t.f_key, a);
        self.f_24 = t.f_24;
        self.f_adsr = t.f_adsr;
        self.a_adsr = t.a_adsr;
        self.vel = t.vel;
        for (c, tl) in self.lfo.iter_mut().zip(t.lfo.iter()) {
            c.0 = tl.0;
            glide(&mut c.1, tl.1, a);
            glide(&mut c.2, tl.2, a);
            c.3 = tl.3;
        }
        self.me.0 = t.me.0;
        self.me.1 = t.me.1;
        glide(&mut self.me.2, t.me.2, a);
        self.me.3 = t.me.3;
        glide(&mut self.wheel_vib, t.wheel_vib, a);
        self.bend_range = t.bend_range;
        self.cho.0 = t.cho.0;
        glide(&mut self.cho.1, t.cho.1, a);
        glide(&mut self.cho.2, t.cho.2, a);
        self.cho.3 = t.cho.3; // mix is smoothed per sample later
        self.echo.0 = t.echo.0;
        self.echo.1 = t.echo.1; // the echo smooths its own delay time
        glide(&mut self.echo.2, t.echo.2, a);
        glide(&mut self.echo.3, t.echo.3, a);
        self.echo.4 = t.echo.4;
        self.rev.0 = t.rev.0;
        glide(&mut self.rev.1, t.rev.1, a);
        glide(&mut self.rev.2, t.rev.2, a);
        self.rev.3 = t.rev.3;
        self.master = t.master; // smoothed per sample later
        self.voices = t.voices;
    }
}

#[derive(Default, Clone, Copy)]
struct Mods {
    pitch: f32,
    cutoff: f32,
    mix: f32,
    amp: f32,
    cho: f32,
    echo: f32,
    rev: f32,
    reso: f32,
    tone: f32,
    det: f32,
    pan: f32,
    und: f32,
}
impl Mods {
    fn add(&mut self, d: Dest, v: f32, pitch_scale: f32, cut_scale: f32) {
        match d {
            Dest::Pitch => self.pitch += v * pitch_scale,
            Dest::Cutoff => self.cutoff += v * cut_scale,
            Dest::LayerMix => self.mix += v,
            Dest::Amp => self.amp += v,
            Dest::Chorus => self.cho += v * 0.5,
            Dest::Echo => self.echo += v * 0.5,
            Dest::Reverb => self.rev += v * 0.5,
            Dest::Resonance => self.reso += v * 0.5,
            Dest::Tone => self.tone += v * 0.5,
            Dest::Detune => self.det += v * 0.5,
            Dest::Pan => self.pan += v,
            Dest::Undertones => self.und += v,
        }
    }
}

/// Control-rate values for one layer of one voice.
#[derive(Clone, Copy, Default)]
struct LayerCtl {
    ens_dt: [f32; 7],
    str_d: [f32; 2],
    str_g: f32,
    str_s: f32,
    str_dt: [f32; 2],
    exc_a: f32,
    comb_d: f32,
    comb_fb: f32,
    body_dt: [f32; 2],
    /// Ensemble brightness (one-pole low-pass coefficient)
    ens_a: f32,
    gl: f32,
    gr: f32,
}

struct Layer {
    saws: [Saw; 7],
    strings: [Delay; 2],
    prev: [f32; 2],
    excite: [usize; 2],
    exc_lp: OnePole,
    comb_lp: OnePole,
    tone_lp: OnePole,
    body: [Saw; 2],
    bow: [Saw; 2],
    ctl: LayerCtl,
    wave: Wave,
    dc: [OnePole; 1],
}

impl Layer {
    fn new(max_delay: usize) -> Self {
        Self {
            saws: [Saw::default(); 7],
            strings: [Delay::new(max_delay), Delay::new(max_delay)],
            prev: [0.0; 2],
            excite: [0; 2],
            exc_lp: OnePole::default(),
            comb_lp: OnePole::default(),
            tone_lp: OnePole::default(),
            body: [Saw::default(); 2],
            bow: [Saw::default(); 2],
            ctl: LayerCtl::default(),
            wave: Wave::Ensemble,
            dc: [OnePole::default()],
        }
    }
    fn start(&mut self, rng: &mut Rng, wave: Wave, freq: f32, sr: f32) {
        for s in self.saws.iter_mut() {
            s.phase = rng.uni();
        }
        for s in self.body.iter_mut().chain(self.bow.iter_mut()) {
            s.phase = rng.uni();
        }
        self.wave = wave;
        if matches!(wave, Wave::Plucked | Wave::Bowed | Wave::Body) {
            self.strings[0].clear();
            self.strings[1].clear();
        }
        self.prev = [0.0; 2];
        let period = (sr / freq.max(20.0)) as usize + 1;
        self.excite = [period, period];
        self.exc_lp = OnePole::default();
        self.comb_lp = OnePole::default();
        self.dc = [OnePole::default()];
    }

    #[inline]
    fn tick(&mut self, rng: &mut Rng, vel: f32) -> f32 {
        let c = &self.ctl;
        let out = match self.wave {
            Wave::Ensemble => {
                let mut acc = 0.0;
                for k in 0..7 {
                    acc += self.saws[k].tick(c.ens_dt[k]) * ENS_GAIN[k];
                }
                self.tone_lp.lp(acc * 0.24, c.ens_a)
            }
            Wave::Plucked | Wave::Bowed => {
                let bowed = self.wave == Wave::Bowed;
                let mut acc = 0.0;
                for j in 0..2 {
                    let y = self.strings[j].read(c.str_d[j]);
                    let f = (1.0 - c.str_s) * y + c.str_s * self.prev[j];
                    self.prev[j] = y;
                    let mut x = 0.0;
                    if bowed {
                        let saw = self.bow[j].tick(c.str_dt[j]);
                        x = (saw * 0.6 + rng.bi() * 0.18) * (1.0 - c.str_g) * 2.2;
                    } else if self.excite[j] > 0 {
                        self.excite[j] -= 1;
                        x = self.exc_lp.lp(rng.bi(), c.exc_a) * (0.35 + 0.65 * vel) * 0.9;
                    }
                    let v = fast_tanh(c.str_g * f + x);
                    self.strings[j].write(v);
                    acc += v;
                }
                // remove DC that can build up in the loop
                let a = acc * 0.5;
                let lp = self.dc[0].lp(a, 0.0015);
                (a - lp) * if bowed { 1.3 } else { 2.8 }
            }
            Wave::Body => {
                let ex = (self.body[0].tick(c.body_dt[0]) + self.body[1].tick(c.body_dt[1])) * 0.3
                    + rng.bi() * 0.12;
                let fbv = self.comb_lp.lp(self.strings[0].read(c.comb_d), 0.5);
                let y = fast_tanh(ex + c.comb_fb * fbv);
                self.strings[0].write(y);
                let o = y * (1.0 - c.comb_fb) * 1.6 + ex * 0.35;
                let lp = self.dc[0].lp(o, 0.0015);
                o - lp
            }
        };
        out
    }
}

struct Voice {
    active: bool,
    note: u8,
    channel: u16,
    voice_id: Option<i32>,
    vel: f32,
    age: u64,
    released: bool,
    sustained: bool,
    amp: Adsr,
    fenv: Adsr,
    menv: Adsr,
    layers: [Layer; 3],
    sub_phase: f32,
    drone: [Saw; 2],
    drone_lp: OnePole,
    svf: [Svf; 4],
    coefs: [SvfCoefs; 2],
    // steal crossfade
    fade: f32,
    pending: Option<(u8, f32, Option<i32>, u16)>,
    // control-rate cached
    sub_dt: f32,
    drone_dt: [f32; 2],
    drone_a: f32,
    gl_mix: [f32; 3],
    amp_mod: f32,
    und_mod: f32,
}

impl Voice {
    fn new(max_delay: usize) -> Self {
        Self {
            active: false,
            note: 0,
            channel: 0,
            voice_id: None,
            vel: 1.0,
            age: 0,
            released: false,
            sustained: false,
            amp: Adsr::default(),
            fenv: Adsr::default(),
            menv: Adsr::default(),
            layers: [
                Layer::new(max_delay),
                Layer::new(max_delay),
                Layer::new(max_delay),
            ],
            sub_phase: 0.0,
            drone: [Saw::default(); 2],
            drone_lp: OnePole::default(),
            svf: [Svf::default(); 4],
            coefs: [SvfCoefs::default(); 2],
            fade: 0.0,
            pending: None,
            sub_dt: 0.0,
            drone_dt: [0.0; 2],
            drone_a: 0.0,
            gl_mix: [1.0; 3],
            amp_mod: 1.0,
            und_mod: 1.0,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Lfo {
    phase: f32,
    sh: f32,
    out: f32,
}

pub struct Engine {
    sr: f32,
    voices: Vec<Voice>,
    chorus: Chorus,
    echo: Echo,
    reverb: Reverb,
    lfos: [Lfo; 2],
    vib_phase: f32,
    rng: Rng,
    age: u64,
    pub wheel: f32,
    pub expression: f32,
    expr_sm: f32,
    pub bend: f32,
    sustain: bool,
    last_menv: f32,
    ctl_pos: usize,
    sm_cho: f32,
    sm_echo: f32,
    sm_rev: f32,
    sm_master: f32,
    rev_gains: [f32; 8],
    rev_size_cached: f32,
    fx_tail: [bool; 3],
    g_mods: Mods,
    /// output peak and deepest limiter reduction (dB) of the last block
    pub meter_peak: f32,
    pub meter_gr: f32,
    /// tracker pitch (portamento, vibrato), semitones, added to `bend`
    pub tune: f32,
    /// Smoothed parameter values the voices actually use.
    cur: Snap,
    has_cur: bool,
    smooth_a: f32,
    limiter: Limiter,
}

impl Engine {
    pub fn new(sr: f32) -> Self {
        let max_delay = (sr / 18.0) as usize + 8;
        let reverb = Reverb::new(sr);
        let rev_gains = reverb.decay_gains(0.75);
        Self {
            sr,
            voices: (0..NUM_VOICES).map(|_| Voice::new(max_delay)).collect(),
            chorus: Chorus::new(sr),
            echo: Echo::new(sr),
            reverb,
            lfos: [Lfo::default(); 2],
            vib_phase: 0.0,
            rng: Rng::new(0x1234_5678),
            age: 0,
            wheel: 0.0,
            expression: 1.0,
            expr_sm: 1.0,
            bend: 0.0,
            sustain: false,
            last_menv: 0.0,
            ctl_pos: 0,
            sm_cho: 0.0,
            sm_echo: 0.0,
            sm_rev: 0.0,
            sm_master: 0.5,
            rev_gains,
            rev_size_cached: 0.75,
            fx_tail: [true; 3],
            g_mods: Mods::default(),
            meter_peak: 0.0,
            meter_gr: 0.0,
            tune: 0.0,
            // placeholder; replaced by the first real snapshot before use
            cur: Snap::new(&super::StringsP::default(), &|_| None),
            has_cur: false,
            smooth_a: 1.0 - (-(CTRL as f32) / (SMOOTH_S * sr)).exp(),
            limiter: Limiter::new(sr),
        }
    }

    pub fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.active = false;
            v.pending = None;
            v.amp = Adsr::default();
        }
        self.chorus.reset();
        self.echo.reset();
        self.reverb.reset();
        self.sustain = false;
        self.ctl_pos = 0;
        self.has_cur = false;
        self.limiter.reset();
    }

    /// The smoothed values currently in use (for tests).
    pub fn current(&self) -> &Snap {
        &self.cur
    }

    fn sync_cur(&mut self, snap: &Snap) {
        if !self.has_cur {
            self.cur = *snap;
            for l in self.cur.layers.iter_mut() {
                if !l.on {
                    l.lvl = 0.0;
                }
            }
            if !self.cur.und {
                self.cur.sub_lvl = 0.0;
                self.cur.drone_lvl = 0.0;
            }
            // first note after a reset: start the master at its target instead of ramping
            self.sm_master = snap.master;
            self.has_cur = true;
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    pub fn note_on(
        &mut self,
        snap: &Snap,
        note: u8,
        vel: f32,
        voice_id: Option<i32>,
        channel: u16,
    ) {
        self.sync_cur(snap);
        // free voice?
        // the Voices knob limits how many voices new notes may use
        let nv = snap.voices.clamp(1, NUM_VOICES);
        let idx = if let Some(i) = self.voices[..nv].iter().position(|v| !v.active) {
            i
        } else {
            // steal: prefer released voices with lowest level, else the oldest
            let mut best = 0;
            let mut best_score = f32::MAX;
            for (i, v) in self.voices.iter().enumerate().take(nv) {
                if v.pending.is_some() {
                    continue;
                }
                let score = if v.released {
                    v.amp.level
                } else {
                    10.0 + (v.age as f32 / 1e7)
                };
                if score < best_score {
                    best_score = score;
                    best = i;
                }
            }
            let v = &mut self.voices[best];
            v.pending = Some((note, vel, voice_id, channel));
            v.fade = 1.0;
            return;
        };
        self.start_voice(idx, note, vel, voice_id, channel);
    }

    fn start_voice(&mut self, idx: usize, note: u8, vel: f32, voice_id: Option<i32>, channel: u16) {
        let snap = &self.cur.clone();
        self.age += 1;
        let sr = self.sr;
        let gm = self.g_mods;
        let bend = self.bend + self.tune;
        let vib = self.vib_value(snap);
        let v = &mut self.voices[idx];
        v.active = true;
        v.note = note;
        v.channel = channel;
        v.voice_id = voice_id;
        v.vel = vel;
        v.age = self.age;
        v.released = false;
        v.sustained = false;
        v.pending = None;
        v.fade = 0.0;
        v.amp = Adsr::default();
        v.fenv = Adsr::default();
        v.menv = Adsr::default();
        v.amp.gate_on();
        v.fenv.gate_on();
        v.menv.gate_on();
        v.svf = [Svf::default(); 4];
        v.sub_phase = 0.0;
        v.drone[0].phase = self.rng.uni();
        v.drone[1].phase = self.rng.uni();
        v.drone_lp = OnePole::default();
        let base = note_to_hz(note as f32);
        for (l, ls) in v.layers.iter_mut().zip(snap.layers.iter()) {
            l.start(
                &mut self.rng,
                ls.wave,
                base * semis_to_ratio(ls.oct as f32 * 12.0),
                sr,
            );
        }
        // compute control values immediately so the first samples are right
        Self::voice_control(v, snap, &gm, bend, vib, sr, &mut self.last_menv, true);
    }

    pub fn note_off(&mut self, note: u8, channel: u16) {
        for v in self.voices.iter_mut() {
            if let Some((n, _, _, c)) = v.pending {
                if n == note && c == channel {
                    // the note was released before the steal fade finished
                    v.pending = None;
                }
            }
            if v.active
                && !v.released
                && v.note == note
                && v.channel == channel
                && v.pending.is_none()
            {
                if self.sustain {
                    v.sustained = true;
                } else {
                    v.released = true;
                    v.amp.gate_off();
                    v.fenv.gate_off();
                    v.menv.gate_off();
                }
            }
        }
    }

    pub fn set_sustain(&mut self, on: bool) {
        self.sustain = on;
        if !on {
            for v in self.voices.iter_mut() {
                if v.active && v.sustained && !v.released {
                    v.sustained = false;
                    v.released = true;
                    v.amp.gate_off();
                    v.fenv.gate_off();
                    v.menv.gate_off();
                }
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        self.sustain = false;
        for v in self.voices.iter_mut() {
            v.pending = None;
            if v.active {
                v.released = true;
                v.amp.gate_off();
                v.fenv.gate_off();
                v.menv.gate_off();
            }
        }
    }

    pub fn choke(&mut self, note: u8, channel: u16) {
        for v in self.voices.iter_mut() {
            if v.active && v.note == note && v.channel == channel {
                v.pending = None;
                v.fade = 1.0; // fades out quickly then goes idle
                v.released = true;
            }
        }
    }

    fn vib_value(&self, snap: &Snap) -> f32 {
        (TAU * self.vib_phase).sin() * self.wheel * snap.wheel_vib * 0.6
    }

    fn update_globals(&mut self, snap: &Snap, n: usize) {
        let dt = n as f32 / self.sr;
        let mut gm = Mods::default();
        for i in 0..2 {
            let (wave, rate, depth, dest) = snap.lfo[i];
            let l = &mut self.lfos[i];
            l.phase += rate * dt;
            if l.phase >= 1.0 {
                l.phase -= l.phase.floor();
                l.sh = self.rng.bi();
            }
            let p = l.phase;
            let raw = match wave {
                LfoWave::Sine => (TAU * p).sin(),
                LfoWave::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
                LfoWave::Saw => 2.0 * p - 1.0,
                LfoWave::Square => {
                    if p < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                LfoWave::SampleHold => l.sh,
            };
            // de-click steps
            let a = if matches!(wave, LfoWave::Square | LfoWave::SampleHold) {
                0.25
            } else {
                1.0
            };
            l.out += (raw - l.out) * a;
            gm.add(dest, l.out * depth, 1.0, 3.0);
        }
        // mod env for global destinations: follow the newest voice
        let (_, _, amt, dest) = snap.me;
        if matches!(dest, Dest::Chorus | Dest::Echo | Dest::Reverb) {
            gm.add(dest, self.last_menv * amt, 0.0, 0.0);
        }
        self.vib_phase += 5.5 * dt;
        if self.vib_phase >= 1.0 {
            self.vib_phase -= 1.0;
        }
        self.g_mods = gm;
        if (snap.rev.1 - self.rev_size_cached).abs() > 1e-4 {
            self.rev_size_cached = snap.rev.1;
            self.rev_gains = self.reverb.decay_gains(snap.rev.1);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn voice_control(
        v: &mut Voice,
        snap: &Snap,
        gm: &Mods,
        bend: f32,
        vib: f32,
        sr: f32,
        last_menv: &mut f32,
        newest: bool,
    ) {
        const PAN_DIR: [f32; 3] = [-1.0, 1.0, -0.5];
        let (_, _, me_amt, me_dest) = snap.me;
        let menv = v.menv.level * me_amt;
        if newest {
            *last_menv = v.menv.level;
        }
        let mut m = *gm;
        if !matches!(me_dest, Dest::Chorus | Dest::Echo | Dest::Reverb) {
            m.add(me_dest, menv, 12.0, 5.0);
        }
        let pitch = v.note as f32 + bend * snap.bend_range + m.pitch + vib;
        let base = note_to_hz(pitch);
        let mixm = m.mix.clamp(-1.0, 1.0);
        v.gl_mix = [1.0 - mixm.max(0.0), 1.0, 1.0 + mixm.min(0.0)];
        v.amp_mod = (1.0 + 0.6 * m.amp).max(0.0);

        v.und_mod = (1.0 + m.und).max(0.0);
        for (li, (l, ls)) in v.layers.iter_mut().zip(snap.layers.iter()).enumerate() {
            if !ls.on && ls.lvl < 1e-4 {
                continue;
            }
            let ls = LayerSnap {
                det: (ls.det + m.det).clamp(0.0, 1.0),
                tone: (ls.tone + m.tone).clamp(0.0, 1.0),
                ..*ls
            };
            let ls = &ls;
            let f = base * semis_to_ratio(ls.oct as f32 * 12.0);
            if l.wave != ls.wave {
                // wave switched while the note plays: restart that oscillator cleanly
                l.wave = ls.wave;
                l.strings[0].clear();
                l.strings[1].clear();
                l.prev = [0.0; 2];
                l.excite = [(sr / f.max(20.0)) as usize; 2];
            }
            let c = &mut l.ctl;
            match ls.wave {
                Wave::Ensemble => {
                    let spread = ls.det * 0.45; // semitones at the edge
                    for k in 0..7 {
                        c.ens_dt[k] = f * semis_to_ratio(ENS_OFFS[k] * spread) / sr;
                    }
                    // Tone: 900 Hz (dark) .. fully open
                    c.ens_a =
                        OnePole::coef((900.0 * (ls.tone * 4.6).exp2()).min(sr * 0.45), sr).min(1.0);
                }
                Wave::Plucked | Wave::Bowed => {
                    let bowed = ls.wave == Wave::Bowed;
                    let dets = [-ls.det * 0.12, ls.det * 0.12];
                    let s = 0.5 - ls.tone * 0.44;
                    c.str_s = s;
                    let t60 = if bowed {
                        0.35 + ls.tone * 0.5
                    } else {
                        1.2 + ls.tone * 9.0
                    };
                    let fmin = f.max(20.0).min(sr * 0.2);
                    c.str_g = 10f32.powf(-3.0 / (t60 * fmin));
                    for j in 0..2 {
                        let fj = (fmin * semis_to_ratio(dets[j])).min(sr * 0.2);
                        c.str_dt[j] = fj / sr;
                        // total loop delay = read delay + 1 (write) + s (loop filter)
                        c.str_d[j] = (sr / fj - 1.0 - s).max(1.0);
                    }
                    c.exc_a = OnePole::coef(800.0 + 9000.0 * ls.tone * (0.4 + 0.6 * v.vel), sr);
                }
                Wave::Body => {
                    let fmin = f.max(20.0).min(sr * 0.2);
                    c.comb_d = (sr / fmin - 1.0 - 1.0).max(1.0);
                    c.comb_fb = 0.6 + ls.tone * 0.37;
                    let d = ls.det * 0.2;
                    c.body_dt = [f * semis_to_ratio(-d) / sr, f * semis_to_ratio(d) / sr];
                }
            }
            let pan = (ls.pan + m.pan * PAN_DIR[li]).clamp(-1.0, 1.0);
            let ang = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
            c.gl = ang.cos() * ls.lvl * std::f32::consts::SQRT_2;
            c.gr = ang.sin() * ls.lvl * std::f32::consts::SQRT_2;
        }
        let sub_div = match snap.sub_oct {
            SubOct::Down1 => 0.5,
            SubOct::Down2 => 0.25,
        };
        v.sub_dt = base * sub_div / sr;
        v.drone_dt = [
            base * 0.5 * semis_to_ratio(-0.07) / sr,
            base * 0.5 * semis_to_ratio(0.07) / sr,
        ];
        v.drone_a = OnePole::coef(
            (base * 0.5 * (1.5 + snap.drone_tone * 12.0)).min(sr * 0.4),
            sr,
        );

        // filter
        let key = (v.note as f32 - 60.0) / 12.0 * snap.f_key;
        let oct = key + v.fenv.level * snap.f_env * 7.0 + m.cutoff;
        let cut = (snap.cutoff * oct.exp2()).clamp(20.0, sr * 0.45);
        let reso = (snap.reso + m.reso).clamp(0.0, 1.0);
        if snap.f_24 {
            v.coefs[0] = SvfCoefs::lowpass(cut, sr, 1.414 - reso * 0.4);
            v.coefs[1] = SvfCoefs::lowpass(cut, sr, 2.0 - 1.85 * reso);
        } else {
            v.coefs[0] = SvfCoefs::lowpass(cut, sr, 2.0 - 1.9 * reso);
        }
    }

    /// Render `out_l.len()` samples (overwrites the buffers).
    pub fn render(&mut self, target: &Snap, out_l: &mut [f32], out_r: &mut [f32]) {
        self.sync_cur(target);
        let n = out_l.len().min(out_r.len());
        let sr = self.sr;
        let snap = &target.clone();
        let amp_r = AdsrRates::new(
            snap.a_adsr[0],
            snap.a_adsr[1],
            snap.a_adsr[2],
            snap.a_adsr[3],
            sr,
        );
        let f_r = AdsrRates::new(
            snap.f_adsr[0],
            snap.f_adsr[1],
            snap.f_adsr[2],
            snap.f_adsr[3],
            sr,
        );
        let m_r = AdsrRates::new(snap.me.0, snap.me.1, 0.0, snap.me.1, sr);
        let fade_step = 1.0 / (0.004 * sr);
        let vs = snap.vel;

        let mut i = 0;
        while i < n {
            if self.ctl_pos == 0 {
                let a = self.smooth_a;
                self.cur.track(target, a);
                let c = self.cur;
                self.update_globals(&c, CTRL);
                let gm = self.g_mods;
                let vib = self.vib_value(&c);
                let newest = self
                    .voices
                    .iter()
                    .filter(|v| v.active)
                    .map(|v| v.age)
                    .max()
                    .unwrap_or(0);
                let bend = self.bend + self.tune;
                for v in self.voices.iter_mut().filter(|v| v.active) {
                    let is_new = v.age == newest;
                    Self::voice_control(v, &c, &gm, bend, vib, sr, &mut self.last_menv, is_new);
                }
            }
            let c = &self.cur;
            let layer_on = [
                c.layers[0].on || c.layers[0].lvl >= 1e-4,
                c.layers[1].on || c.layers[1].lvl >= 1e-4,
                c.layers[2].on || c.layers[2].lvl >= 1e-4,
            ];
            let f_24 = c.f_24;
            // drone/sub levels are linear-scaled here
            let sub_lvl = c.sub_lvl * 0.9;
            let drone_lvl = c.drone_lvl * 0.8;
            let chunk = (CTRL - self.ctl_pos).min(n - i);
            for k in 0..chunk {
                out_l[i + k] = 0.0;
                out_r[i + k] = 0.0;
            }
            for vi in 0..NUM_VOICES {
                if !self.voices[vi].active {
                    continue;
                }
                let mut finished_fade = false;
                {
                    let v = &mut self.voices[vi];
                    let velg = 1.0 - vs + vs * v.vel;
                    let gain = VOICE_GAIN * velg * v.amp_mod;
                    let rng = &mut self.rng;
                    for k in 0..chunk {
                        let a = v.amp.tick(&amp_r);
                        v.fenv.tick(&f_r);
                        v.menv.tick(&m_r);
                        let mut l = 0.0;
                        let mut r = 0.0;
                        for li in 0..3 {
                            if !layer_on[li] {
                                continue;
                            }
                            let s = v.layers[li].tick(rng, v.vel) * v.gl_mix[li];
                            l += s * v.layers[li].ctl.gl;
                            r += s * v.layers[li].ctl.gr;
                        }
                        // undertones
                        v.sub_phase += v.sub_dt;
                        if v.sub_phase >= 1.0 {
                            v.sub_phase -= 1.0;
                        }
                        let sp = TAU * v.sub_phase;
                        let sub = (sp.sin() + 0.12 * (2.0 * sp).sin()) * sub_lvl * v.und_mod;
                        let dr = v.drone[0].tick(v.drone_dt[0]) + v.drone[1].tick(v.drone_dt[1]);
                        let drone = v.drone_lp.lp(dr * 0.5, v.drone_a) * drone_lvl * v.und_mod;
                        l += sub + drone;
                        r += sub + drone;
                        // filter
                        let (mut fl, mut fr);
                        fl = v.svf[0].lp(l, &v.coefs[0]);
                        fr = v.svf[1].lp(r, &v.coefs[0]);
                        if f_24 {
                            fl = v.svf[2].lp(fl, &v.coefs[1]);
                            fr = v.svf[3].lp(fr, &v.coefs[1]);
                        }
                        let mut g = a * gain;
                        if v.fade > 0.0 {
                            g *= v.fade;
                            v.fade -= fade_step;
                            if v.fade <= 0.0 {
                                v.fade = 0.0;
                                finished_fade = true;
                            }
                        }
                        out_l[i + k] += fl * g;
                        out_r[i + k] += fr * g;
                        if finished_fade {
                            break;
                        }
                    }
                    if v.amp.stage == Stage::Idle && v.pending.is_none() {
                        v.active = false;
                    }
                }
                if finished_fade {
                    let v = &mut self.voices[vi];
                    if let Some((note, vel, id, ch)) = v.pending.take() {
                        self.start_voice(vi, note, vel, id, ch);
                    } else {
                        v.active = false;
                    }
                }
            }
            self.ctl_pos = (self.ctl_pos + chunk) % CTRL;
            i += chunk;
        }

        // ---- global effects ----
        let gm = self.g_mods;
        let snap = &self.cur.clone();
        let ctgt = if snap.cho.0 {
            (snap.cho.3 + gm.cho).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let etgt = if snap.echo.0 {
            (snap.echo.4 + gm.echo).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let rtgt = if snap.rev.0 {
            (snap.rev.3 + gm.rev).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let sm_a = 1.0 - (-1.0 / (0.02 * sr)).exp();
        let echo_lp = OnePole::coef(600.0 * (20.0f32).powf(snap.echo.3), sr);
        let echo_hp = OnePole::coef(90.0, sr);
        let rev_damp = OnePole::coef(16000.0 * (0.04f32).powf(snap.rev.2), sr).min(1.0);
        let rev_hp = OnePole::coef(70.0, sr);
        let master = snap.master;
        // restart fx buffers cleanly after being bypassed
        let run = [
            ctgt > 0.0 || self.sm_cho > 1e-4,
            etgt > 0.0 || self.sm_echo > 1e-4,
            rtgt > 0.0 || self.sm_rev > 1e-4,
        ];
        if run[0] && !self.fx_tail[0] {
            self.chorus.reset();
        }
        if run[1] && !self.fx_tail[1] {
            self.echo.reset();
        }
        if run[2] && !self.fx_tail[2] {
            self.reverb.reset();
        }
        self.fx_tail = run;
        let mut peak = 0.0f32;
        for k in 0..n {
            let mut l = out_l[k];
            let mut r = out_r[k];
            self.sm_cho += (ctgt - self.sm_cho) * sm_a;
            self.sm_echo += (etgt - self.sm_echo) * sm_a;
            self.sm_rev += (rtgt - self.sm_rev) * sm_a;
            self.sm_master += (master - self.sm_master) * sm_a;
            self.expr_sm += (self.expression - self.expr_sm) * sm_a;
            if run[0] {
                (l, r) = self
                    .chorus
                    .process(l, r, snap.cho.1, snap.cho.2, self.sm_cho);
            }
            if run[1] {
                (l, r) = self.echo.process(
                    l,
                    r,
                    snap.echo.1,
                    snap.echo.2,
                    echo_lp,
                    echo_hp,
                    self.sm_echo,
                );
            }
            if run[2] {
                (l, r) = self
                    .reverb
                    .process(l, r, &self.rev_gains, rev_damp, rev_hp, self.sm_rev);
            }
            let g = self.sm_master * self.expr_sm;
            (l, r) = self.limiter.process(l * g, r * g);
            if !l.is_finite() || !r.is_finite() {
                l = 0.0;
                r = 0.0;
            }
            out_l[k] = l;
            out_r[k] = r;
            peak = peak.max(l.abs()).max(r.abs());
        }
        self.meter_peak = peak;
        self.meter_gr = self.limiter.take_reduction_db();
    }
}
