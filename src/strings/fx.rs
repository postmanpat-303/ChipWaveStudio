//! Global effects: string ensemble chorus, stereo echo, lush FDN reverb.
// ported DSP: index loops kept as in the stand-alone build (same output)
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]
use super::dsp::*;

pub struct Chorus {
    dl: Delay,
    dr: Delay,
    phase: f32,
    phase_fast: f32,
    sr: f32,
}
impl Chorus {
    pub fn new(sr: f32) -> Self {
        let n = (sr * 0.06) as usize;
        Self {
            dl: Delay::new(n),
            dr: Delay::new(n),
            phase: 0.0,
            phase_fast: 0.0,
            sr,
        }
    }
    pub fn reset(&mut self) {
        self.dl.clear();
        self.dr.clear();
    }
    /// Three-phase ensemble chorus (slow + fast LFO per tap).
    #[inline]
    pub fn process(&mut self, l: f32, r: f32, rate: f32, depth: f32, mix: f32) -> (f32, f32) {
        self.dl.write(l);
        self.dr.write(r);
        self.phase += rate / self.sr;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        self.phase_fast += (rate * 9.7) / self.sr;
        if self.phase_fast >= 1.0 {
            self.phase_fast -= 1.0;
        }
        let base = 0.011 * self.sr;
        let d_slow = 0.0055 * self.sr * depth;
        let d_fast = 0.0006 * self.sr * depth;
        let mut wl = 0.0;
        let mut wr = 0.0;
        for k in 0..3 {
            let off = k as f32 / 3.0;
            let s = (TAU * (self.phase + off)).sin();
            let f = (TAU * (self.phase_fast + off)).sin();
            let dl = base + d_slow * (0.5 + 0.5 * s) + d_fast * f;
            let s2 = (TAU * (self.phase + off + 0.1667)).sin();
            let dr = base + d_slow * (0.5 + 0.5 * s2) - d_fast * f;
            wl += self.dl.read(dl);
            wr += self.dr.read(dr);
        }
        wl *= 1.0 / 3.0;
        wr *= 1.0 / 3.0;
        // cross-blend for width
        let (wl, wr) = (wl * 0.8 + wr * 0.2, wr * 0.8 + wl * 0.2);
        (l + (wl - l) * mix, r + (wr - r) * mix)
    }
}

pub struct Echo {
    dl: Delay,
    dr: Delay,
    time_smooth: f32,
    lp_l: OnePole,
    lp_r: OnePole,
    hp_l: OnePole,
    hp_r: OnePole,
    sr: f32,
}
impl Echo {
    pub fn new(sr: f32) -> Self {
        let n = (sr * 1.6) as usize + 8;
        Self {
            dl: Delay::new(n),
            dr: Delay::new(n),
            time_smooth: 0.38 * sr,
            lp_l: OnePole::default(),
            lp_r: OnePole::default(),
            hp_l: OnePole::default(),
            hp_r: OnePole::default(),
            sr,
        }
    }
    pub fn reset(&mut self) {
        self.dl.clear();
        self.dr.clear();
        self.lp_l = OnePole::default();
        self.lp_r = OnePole::default();
        self.hp_l = OnePole::default();
        self.hp_r = OnePole::default();
    }
    /// Ping-pong-ish stereo echo. `lp_a` / `hp_a` are one-pole coefficients for the feedback tone.
    #[inline]
    pub fn process(
        &mut self,
        l: f32,
        r: f32,
        time_s: f32,
        fb: f32,
        lp_a: f32,
        hp_a: f32,
        mix: f32,
    ) -> (f32, f32) {
        let target = (time_s * self.sr).clamp(4.0, self.dl.len() as f32 - 8.0);
        self.time_smooth += (target - self.time_smooth) * 0.0005;
        let t = self.time_smooth;
        let el = self.dl.read(t);
        let er = self.dr.read(t * 1.0 + 0.0);
        // tone filters sit at the delay input, so every repeat (the first one too) is coloured
        let fl = self.hp_l.hp(er, hp_a);
        let fr = self.hp_r.hp(el, hp_a);
        let inl = l * 0.7 + r * 0.3;
        self.dl.write(fast_tanh(self.lp_l.lp(inl + fl * fb, lp_a)));
        self.dr
            .write(fast_tanh(self.lp_r.lp(fr * fb + (r - l) * 0.15, lp_a)));
        (l + el * mix, r + er * mix)
    }
}

const FDN_N: usize = 8;
const FDN_MS: [f32; FDN_N] = [37.1, 41.9, 47.3, 53.3, 59.9, 67.1, 73.7, 83.3];
const AP_MS: [f32; 4] = [4.77, 3.59, 12.73, 9.31];

pub struct Reverb {
    lines: Vec<Delay>,
    lens: [f32; FDN_N],
    damp: [OnePole; FDN_N],
    ap: Vec<Delay>,
    ap_len: [usize; 4],
    predelay: Delay,
    mod_phase: f32,
    sr: f32,
    hp_in: OnePole,
}
impl Reverb {
    pub fn new(sr: f32) -> Self {
        let mut lens = [0.0; FDN_N];
        let lines = (0..FDN_N)
            .map(|i| {
                lens[i] = FDN_MS[i] * 0.001 * sr * 1.6;
                Delay::new(lens[i] as usize + 64)
            })
            .collect();
        let mut ap_len = [0; 4];
        let ap = (0..4)
            .map(|i| {
                ap_len[i] = (AP_MS[i] * 0.001 * sr) as usize;
                Delay::new(ap_len[i] + 4)
            })
            .collect();
        Self {
            lines,
            lens,
            damp: [OnePole::default(); FDN_N],
            ap,
            ap_len,
            predelay: Delay::new((0.03 * sr) as usize + 4),
            mod_phase: 0.0,
            sr,
            hp_in: OnePole::default(),
        }
    }
    pub fn reset(&mut self) {
        self.lines.iter_mut().for_each(|d| d.clear());
        self.ap.iter_mut().for_each(|d| d.clear());
        self.predelay.clear();
        self.damp = [OnePole::default(); FDN_N];
        self.hp_in = OnePole::default();
    }

    /// `gains` per line come from `decay_gains()` (control rate).
    pub fn decay_gains(&self, size: f32) -> [f32; FDN_N] {
        let t60 = 0.6 + size * size * 11.0;
        let mut g = [0.0; FDN_N];
        for i in 0..FDN_N {
            g[i] = 10f32.powf(-3.0 * self.lens[i] / (t60 * self.sr));
        }
        g
    }

    #[inline]
    pub fn process(
        &mut self,
        l: f32,
        r: f32,
        gains: &[f32; FDN_N],
        damp_a: f32,
        hp_a: f32,
        mix: f32,
    ) -> (f32, f32) {
        // input: mono-ish sum, high-passed, pre-delayed, diffused
        let x = self.hp_in.hp((l + r) * 0.5, hp_a);
        self.predelay.write(x);
        let mut x = self.predelay.read_int((0.022 * self.sr) as usize);
        for i in 0..4 {
            let d = self.ap_len[i];
            let delayed = self.ap[i].read_int(d - 1);
            let g = 0.62;
            let v = x + delayed * g;
            self.ap[i].write(v);
            x = delayed - v * g;
        }
        self.mod_phase += 0.31 / self.sr;
        if self.mod_phase >= 1.0 {
            self.mod_phase -= 1.0;
        }
        let mut o = [0.0f32; FDN_N];
        let md = 0.0012 * self.sr;
        for i in 0..FDN_N {
            let m = (TAU * (self.mod_phase + i as f32 * 0.125)).sin() * md;
            let v = self.lines[i].read(self.lens[i] + m);
            o[i] = self.damp[i].lp(v, damp_a);
        }
        // 8x8 Hadamard (fast Walsh-Hadamard), normalized
        let mut h = o;
        let mut len = 1;
        while len < FDN_N {
            let mut i = 0;
            while i < FDN_N {
                for j in i..i + len {
                    let a = h[j];
                    let b = h[j + len];
                    h[j] = a + b;
                    h[j + len] = a - b;
                }
                i += len * 2;
            }
            len *= 2;
        }
        let norm = 1.0 / (FDN_N as f32).sqrt();
        for i in 0..FDN_N {
            let inject = if i % 2 == 0 { x } else { -x };
            self.lines[i].write(h[i] * norm * gains[i] + inject * 0.35);
        }
        let wl = (o[0] - o[2] + o[4] - o[6] + o[1] * 0.5) * 0.45;
        let wr = (o[1] - o[3] + o[5] - o[7] + o[2] * 0.5) * 0.45;
        (
            l * (1.0 - mix * 0.5) + wl * mix,
            r * (1.0 - mix * 0.5) + wr * mix,
        )
    }
}
