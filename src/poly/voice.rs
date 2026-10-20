//! One poly synth note (`PolyState`, inside `gen::GenState`). Knobs and the
//! modulation routes are evaluated every `BLOCK` samples; oscillators,
//! envelopes and filters run per sample. `Copy`, fixed size, allocation-free.
use super::dsp::{blend, damping, exp2, shape, sin2pi, Env, Filt, Osc, OscSet};
use super::*;
use crate::gen::Ov;
use crate::ladder::tanh_p;
use crate::synth::Svf;

const C4: f32 = 261.625_58;
/// control-rate block (samples)
pub const BLOCK: u32 = 16;
/// highest filter coefficient (cutoff 0.45 sr)
const G_MAX: f32 = 6.313_752;

/// control-rate values of one note
#[derive(Clone, Copy, Default)]
struct Ctl {
    k: (f32, f32),
    hp_on: bool,
    hp_g: f32,
    hp_k: f32,
    drive_on: bool,
    dg: f32,
    fb: f32,
    noise: f32,
    colour: f32,
    nz_a: f32,
    fm: f32,
    fm_inc: f32,
    shp: f32,
    shp_mix: f32,
    m: f32,
    amp: f32,
    pl: f32,
    pr: f32,
    fc: (f32, f32, f32),
    ac: (f32, f32, f32),
    fs: f32,
    asus: f32,
    glide_k: f32,
    /// cutoff in Hz before the filter envelope, and the envelope amount (oct)
    cut: f32,
    env_amt: f32,
}

#[derive(Clone, Copy)]
pub struct PolyState {
    osc: [Osc; 2],
    set: [OscSet; 2],
    started: bool,
    seed: u32,
    nz: u32,
    nlp: f32,
    hp: [Svf; 2],
    filt: [Filt; 2],
    fbk: [f32; 2],
    pub feg: Env,
    pub aeg: Env,
    gate_was: bool,
    ctl_n: u32,
    c: Ctl,
    g: f32,
    g_step: f32,
    fm_ph: f32,
    /// glide offset now (semitones, decays to 0)
    glide: f32,
    /// set by the engine: start this note gliding from here (Hz, 0 = none)
    pub glide_from: f32,
    /// set by the engine: restart the envelopes (Mono mode re-use)
    pub retrigger: bool,
    /// the note frequency of the last block
    pub freq: f32,
    pub done: bool,
}

impl Default for PolyState {
    fn default() -> Self {
        PolyState {
            osc: [Osc::default(); 2],
            set: [OscSet::default(); 2],
            started: false,
            seed: 0x51ED_270B,
            nz: 0x2545_F491,
            nlp: 0.0,
            hp: [Svf::default(); 2],
            filt: [Filt::default(); 2],
            fbk: [0.0; 2],
            feg: Env::default(),
            aeg: Env::default(),
            gate_was: true,
            ctl_n: 0,
            c: Ctl::default(),
            g: 0.0,
            g_step: 0.0,
            fm_ph: 0.0,
            glide: 0.0,
            glide_from: 0.0,
            retrigger: false,
            freq: 0.0,
            done: false,
        }
    }
}

#[inline]
fn pan_gains(p: f32) -> (f32, f32) {
    let p = p.clamp(-1.0, 1.0);
    ((1.0 - p).min(1.0), (1.0 + p).min(1.0))
}

impl PolyState {
    pub fn seed(&mut self, s: u32) {
        self.seed = s;
        self.nz = s.rotate_left(5) ^ 0x9E37_79B9 | 1;
    }

    /// the pitch sounding now (Hz), for legato glides
    #[allow(dead_code)] // WIP: wired up in the next commits
    pub fn sounding(&self) -> f32 {
        self.freq * exp2(self.glide / 12.0)
    }

    /// the knob values (0..1) after overrides and the modulation routes
    fn knobs(&self, pp: &PolyP, ov: &Ov, vel: f32, freq: f32, out: &mut [f32; PN]) {
        for (k, o) in out.iter_mut().enumerate() {
            *o = ov.get(k).unwrap_or(pp.p[k]);
        }
        let key = ((freq.max(1.0) / C4).log2() / 5.0).clamp(-1.0, 1.0);
        for (i, r) in pp.routes.iter().enumerate() {
            let src = match r.src {
                ModSrc::Off => continue,
                ModSrc::Feg => self.feg.level,
                ModSrc::Aeg => self.aeg.level,
                ModSrc::Vel => vel,
                ModSrc::Key => key,
            };
            let a = POLY[MOD_AMT + i].map(out[MOD_AMT + i]);
            let d = r.dst as usize;
            if d < MOD_AMT {
                out[d] = (out[d] + a * src).clamp(0.0, 1.0);
            }
        }
    }

    fn control(&mut self, pp: &PolyP, ov: &Ov, vel: f32, freq: f32, sr: f32) {
        let mut kn = [0.0; PN];
        self.knobs(pp, ov, vel, freq, &mut kn);
        let v = |k: usize| POLY[k].map(kn[k]);
        let dt = 1.0 / sr;
        let c = &mut self.c;
        c.k = damping(pp.filter, kn[RES]);
        let hp = v(HP_CUT);
        c.hp_on = hp > 10.5 || kn[HP_RES] > 0.0;
        c.hp_g = (std::f32::consts::PI * hp.min(sr * 0.45) * dt).tan();
        c.hp_k = 2.0 - 1.98 * kn[HP_RES];
        c.dg = 10f32.powf(kn[DRIVE] * 24.0 / 20.0);
        c.fb = kn[FEEDBACK] * 0.95;
        c.drive_on = kn[DRIVE] > 0.0 || c.fb > 0.0;
        c.noise = kn[NOISE];
        c.colour = v(COLOUR);
        c.nz_a = 1.0 - (-std::f32::consts::TAU * 1500.0 * dt).exp();
        c.fm = kn[FM_AMT] * 4.0;
        c.fm_inc = v(FM_FREQ) * dt;
        c.shp = kn[SHAPER];
        c.shp_mix = kn[SHAPER_MIX];
        c.m = kn[BLEND];
        let va = kn[VEL_AMP];
        c.amp = (1.0 - va + va * vel) * v(GAIN) * 0.5;
        (c.pl, c.pr) = pan_gains(v(PAN));
        c.fc = Env::coefs(v(FEG), v(FEG + 1), v(FEG + 3), dt, 0.0005);
        c.fs = kn[FEG + 2];
        c.ac = Env::coefs(v(AEG), v(AEG + 1), v(AEG + 3), dt, 0.0015);
        c.asus = kn[AEG + 2];
        let gt = v(GLIDE);
        c.glide_k = if gt > 0.0015 {
            (-(BLOCK as f32) * dt * 5.0 / gt).exp()
        } else {
            0.0
        };
        // cutoff: knob × key tracking × velocity (the envelope per sample)
        let kt = kn[KEYTRK] * (freq.max(1.0) / C4).log2();
        let vo = v(VEL_CUT) * (vel - 1.0);
        c.cut = v(CUTOFF) * exp2(kt + vo);
        c.env_amt = v(ENV_AMT);
        // oscillators
        let f = freq * exp2(self.glide / 12.0);
        for (o, s) in self.set.iter_mut().enumerate() {
            let b = OSC[o];
            let ov = |k: usize| POLY[b + k].map(kn[b + k]);
            let semis = ov(PITCH) + 12.0 * ov(OCT).round();
            s.dt = f * exp2(semis / 12.0) * dt;
            s.sync = exp2(ov(SYNC) / 12.0);
            s.pw = ov(PW);
            s.w = super::dsp::weights(ov(SHAPE));
            s.sub = kn[b + SUB];
            s.sub_pw = ov(SUB_PW);
            let n = ov(VOICES).round().clamp(1.0, MAX_UNISON as f32) as usize;
            s.n = n;
            let det = ov(DETUNE);
            let width = kn[b + WIDTH];
            let opan = ov(OPAN);
            let norm = 1.0 / (n as f32).sqrt();
            for i in 0..n {
                let sp = if n == 1 {
                    0.0
                } else {
                    -1.0 + 2.0 * i as f32 / (n - 1) as f32
                };
                s.ratio[i] = exp2(sp * det / 1200.0);
                let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                let (l, r) = pan_gains(opan + side * sp.abs() * width);
                s.gl[i] = l * norm;
                s.gr[i] = r * norm;
            }
            (s.sl, s.sr) = pan_gains(opan);
        }
        // the filter coefficient ramps to this block's target
        let fc = (self.c.cut * exp2(self.feg.level * self.c.env_amt)).clamp(20.0, sr * 0.45);
        let gt = (std::f32::consts::PI * fc * dt).tan().min(G_MAX);
        if !self.started {
            self.g = gt;
        }
        self.g_step = (gt - self.g) / BLOCK as f32;
    }

    /// one stereo sample
    pub fn render(
        &mut self,
        pp: &PolyP,
        ov: Ov,
        freq: f32,
        sr: f32,
        gate: bool,
        vel: f32,
    ) -> (f32, f32, bool) {
        if self.done {
            return (0.0, 0.0, true);
        }
        if !self.started || self.retrigger {
            if self.glide_from > 0.0 && freq > 0.0 {
                self.glide = 12.0 * (self.glide_from / freq).log2();
                if !self.glide.is_finite() {
                    self.glide = 0.0;
                }
            }
            self.glide_from = 0.0;
            if !self.started {
                for (o, x) in self.osc.iter_mut().enumerate() {
                    x.start(pp.reset[o], self.seed.rotate_left(o as u32 * 13));
                }
            } else {
                self.feg.retrigger();
                self.aeg.retrigger();
            }
            self.retrigger = false;
            self.gate_was = true;
            self.ctl_n = 0;
        } else if self.glide_from > 0.0 && freq > 0.0 {
            // legato: glide without restarting the envelopes
            self.glide = 12.0 * (self.glide_from / freq).log2();
            if !self.glide.is_finite() {
                self.glide = 0.0;
            }
            self.glide_from = 0.0;
            self.ctl_n = 0;
        }
        if self.gate_was && !gate {
            self.feg.release();
            self.aeg.release();
        }
        self.gate_was = gate;
        if self.ctl_n == 0 {
            self.freq = freq;
            self.control(pp, &ov, vel, freq, sr);
            self.started = true;
            self.glide *= self.c.glide_k;
            self.ctl_n = BLOCK;
        }
        self.ctl_n -= 1;
        let c = self.c;
        self.feg.next(c.fc, c.fs);
        let ae = self.aeg.next(c.ac, c.asus);
        if self.aeg.done() {
            self.done = true;
            return (0.0, 0.0, true);
        }
        let (a_l, a_r) = self.osc[0].next(&self.set[0]);
        let (mut l, mut r) = if c.m > 0.0 {
            let (b_l, b_r) = self.osc[1].next(&self.set[1]);
            (
                blend(pp.blend, a_l, b_l, c.m),
                blend(pp.blend, a_r, b_r, c.m),
            )
        } else {
            // oscillator 2 silent: keep its phase moving cheaply
            (a_l, a_r)
        };
        if c.noise > 0.0 {
            let mut x = self.nz;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.nz = x;
            let w = (x as i32) as f32 * (1.0 / 2_147_483_648.0);
            self.nlp += (w - self.nlp) * c.nz_a;
            let n = if c.colour < 0.0 {
                w + (2.0 * self.nlp - w) * -c.colour
            } else {
                w - self.nlp * c.colour
            };
            l += n * c.noise;
            r += n * c.noise;
        }
        let mut g = self.g + self.g_step;
        self.g = g;
        if c.fm > 0.0 {
            self.fm_ph += c.fm_inc;
            if self.fm_ph >= 1.0 {
                self.fm_ph -= 1.0;
            }
            g = (g * exp2(c.fm * sin2pi(self.fm_ph))).min(G_MAX);
        }
        let mut out = [l, r];
        for (ch, x) in out.iter_mut().enumerate() {
            let mut y = *x;
            if c.hp_on {
                y = self.hp[ch].process_gk(y, c.hp_g, c.hp_k).2;
            }
            if c.drive_on {
                y = tanh_p(c.dg * (y + c.fb * self.fbk[ch]));
            }
            y = self.filt[ch].run(pp.filter, y, g, c.k);
            self.fbk[ch] = y;
            if c.shp > 0.0 {
                y += (shape(pp.shaper, y, c.shp) - y) * c.shp_mix;
            }
            *x = y * ae * c.amp;
        }
        let (l, r) = (out[0] * c.pl, out[1] * c.pr);
        if !(l.is_finite() && r.is_finite()) {
            self.done = true;
            return (0.0, 0.0, true);
        }
        (l, r, true)
    }
}
