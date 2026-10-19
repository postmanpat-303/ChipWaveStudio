//! The **Wave** generator (Unreleased): a wavetable synth.
//!
//! Two wavetable oscillators (built-in or imported tables from
//! `src/wavetable.rs`, band-limited mipmaps) with position scanning, up to 8
//! unison voices with detune and stereo spread, a sub sine and noise, into
//! ChipWave's ladder filter (clean or vintage, LP / BP / HP, 12 or 24 dB) with
//! its own envelope, then the amp envelope. The filter envelope can also
//! sweep both table positions. Every knob is a generator knob, so automation
//! lanes, 43xy (knobs 1-8), MIDI learn and drag-to-modulate reach all of them.
//!
//! Written for ChipWave; the wavetable design is inspired by Vital by Matt
//! Tytel (GPL-3, https://github.com/mtytel/vital) and Surge XT's wavetable
//! oscillator (GPL-3); ideas only, no code copied. Everything at play time is
//! fixed-size (`WaveState`), so rendering never allocates.
use crate::gen::{MacroDef, Ov, Rng};
use crate::synth::{Adsr, Env, FilterMode};
use crate::wavetable::{builtins, Table, UserTable};
use serde::{Deserialize, Serialize};

const C4: f32 = 261.625_58;
/// `WaveP::tables` value meaning "this oscillator's imported table"
pub const USER: u8 = 255;
/// most unison voices per oscillator
pub const MAX_UNISON: usize = 8;

const fn m(
    name: &'static str,
    lo: f32,
    hi: f32,
    exp: bool,
    unit: &'static str,
    dice: (f32, f32),
    tip: &'static str,
) -> MacroDef {
    MacroDef {
        name,
        lo,
        hi,
        exp,
        unit,
        dice,
        tip,
    }
}

pub const WN: usize = 28;
/// knob definitions; 0-7 are also the 43xy macros
pub const WAVE: [MacroDef; WN] = [
    m("Pos A", 0.0, 1.0, false, "", (0.0, 1.0), "oscillator A table position (scans the frames)"),
    m("Pos B", 0.0, 1.0, false, "", (0.0, 1.0), "oscillator B table position"),
    m("Cutoff", 30.0, 18000.0, true, "Hz", (0.3, 0.85), "filter cutoff"),
    m("Res", 0.0, 1.0, false, "", (0.0, 0.6), "filter resonance (self-oscillates near the top)"),
    m("Env amt", 0.0, 1.0, false, "", (0.0, 0.6), "filter envelope amount (up to 6 octaves)"),
    m("Detune", 0.0, 1.0, false, "", (0.0, 0.4), "unison detune, up to +-50 cents across the voices"),
    m("Spread", 0.0, 1.0, false, "", (0.2, 1.0), "unison stereo spread"),
    m("Osc B", 0.0, 1.0, false, "", (0.0, 0.8), "oscillator B level"),
    m("Osc A", 0.0, 1.0, false, "", (0.6, 1.0), "oscillator A level"),
    m("Semi A", -24.0, 24.0, false, "st", (0.5, 0.5), "oscillator A transpose"),
    m("Fine A", -50.0, 50.0, false, "ct", (0.45, 0.55), "oscillator A fine tune"),
    m("Semi B", -24.0, 24.0, false, "st", (0.25, 0.75), "oscillator B transpose"),
    m("Fine B", -50.0, 50.0, false, "ct", (0.4, 0.6), "oscillator B fine tune"),
    m("Unison", 1.0, 8.0, false, "#", (0.0, 0.6), "voices per oscillator"),
    m("Sub", 0.0, 1.0, false, "", (0.0, 0.4), "sine one octave below oscillator A"),
    m("Noise", 0.0, 1.0, false, "", (0.0, 0.1), "white noise level"),
    m("Key trk", 0.0, 1.0, false, "", (0.3, 1.0), "cutoff follows the note"),
    m("F.Attack", 0.001, 2.0, true, "s", (0.0, 0.4), "filter envelope attack"),
    m("F.Decay", 0.01, 3.0, true, "s", (0.1, 0.7), "filter envelope decay"),
    m("F.Sustain", 0.0, 1.0, false, "", (0.0, 0.6), "filter envelope sustain"),
    m("F.Release", 0.005, 3.0, true, "s", (0.2, 0.6), "filter envelope release"),
    m("A.Attack", 0.001, 2.0, true, "s", (0.0, 0.3), "amp attack"),
    m("A.Decay", 0.01, 3.0, true, "s", (0.2, 0.8), "amp decay"),
    m("A.Sustain", 0.0, 1.0, false, "", (0.3, 1.0), "amp sustain"),
    m("A.Release", 0.005, 3.0, true, "s", (0.1, 0.5), "amp release"),
    m("Drive", 0.0, 1.0, false, "", (0.0, 0.4), "drive into the filter, soft-clipped"),
    m("Env>Pos A", -1.0, 1.0, false, "", (0.4, 0.6), "filter envelope -> oscillator A position"),
    m("Env>Pos B", -1.0, 1.0, false, "", (0.4, 0.6), "filter envelope -> oscillator B position"),
];

/// The Wave generator's patch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WaveP {
    /// built-in table index per oscillator (`wavetable::BUILTIN`), or `USER`
    pub tables: [u8; 2],
    /// imported tables (used when `tables[o] == USER`), saved in the song
    #[serde(with = "user_ser", skip_serializing_if = "no_user")]
    pub user: [Option<UserTable>; 2],
    pub filter_on: bool,
    pub mode: FilterMode,
    /// vintage (saturated, 2x) ladder, else clean
    pub vintage: bool,
    pub pole4: bool,
    pub p: [f32; WN],
    /// bit i locks knob i; `choice_lock(0..2)` the tables
    pub locks: u64,
    pub level: f32,
}

impl Default for WaveP {
    fn default() -> Self {
        let mut w = WaveP {
            tables: [0, 2],
            user: [None, None],
            filter_on: true,
            mode: FilterMode::Lp,
            vintage: false,
            pole4: true,
            p: [0.5; WN],
            locks: 0,
            level: 1.0,
        };
        w.set(&[
            0.66, 0.3, 6000.0, 0.15, 0.25, 0.2, 0.6, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
            0.5, 0.002, 0.4, 0.3, 0.3, 0.002, 0.5, 0.8, 0.25, 0.0, 0.0, 0.0,
        ]);
        w
    }
}

impl WaveP {
    /// set knobs from real values (as many as given)
    pub fn set(&mut self, v: &[f32]) -> &mut Self {
        for (i, x) in v.iter().enumerate().take(WN) {
            self.p[i] = WAVE[i].unmap(*x);
        }
        self
    }
    pub fn fix(&mut self) {
        for v in self.p.iter_mut() {
            *v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.5 };
        }
        for o in 0..2 {
            let ok = match self.tables[o] {
                USER => self.user[o].is_some(),
                t => (t as usize) < crate::wavetable::BUILTIN.len(),
            };
            if !ok {
                self.tables[o] = 0;
            }
        }
        self.level = if self.level.is_finite() {
            self.level.clamp(0.0, 4.0)
        } else {
            1.0
        };
    }
    pub fn dice(&mut self, seed: u32) {
        let r = &mut Rng::new(seed);
        for (i, d) in WAVE.iter().enumerate() {
            let x = r.f();
            if self.locks & (1 << i) == 0 {
                self.p[i] = d.dice.0 + (d.dice.1 - d.dice.0) * x;
            }
        }
        let n = crate::wavetable::BUILTIN.len() as u32;
        for o in 0..2 {
            let t = (r.next() % n) as u8;
            if self.locks & crate::gen2::choice_lock(o) == 0 {
                self.tables[o as usize] = t;
            }
        }
        if self.locks & 1 << 8 == 0 {
            self.p[8] = self.p[8].max(0.6);
        }
    }
    pub fn mutate(&mut self, a: f32, seed: u32) {
        let mut r = Rng::new(seed);
        for (i, v) in self.p.iter_mut().enumerate() {
            let x = r.f() * 2.0 - 1.0;
            if self.locks & (1 << i) == 0 {
                *v = (*v + x * a).clamp(0.0, 1.0);
            }
        }
    }
    /// the table oscillator `o` plays (None: built-ins not ready / missing)
    #[inline]
    pub fn table(&self, o: usize) -> Option<&Table> {
        match self.tables[o] {
            USER => self.user[o].as_ref().map(|u| &*u.table),
            t => builtins()?.get(t as usize),
        }
    }
    /// display name of oscillator `o`'s table
    pub fn table_name(&self, o: usize) -> String {
        match self.tables[o] {
            USER => self.user[o]
                .as_ref()
                .map(|u| format!("{} (imported)", u.name()))
                .unwrap_or_else(|| "(missing)".into()),
            t => crate::wavetable::BUILTIN
                .get(t as usize)
                .copied()
                .unwrap_or("?")
                .into(),
        }
    }
}

fn no_user(u: &[Option<UserTable>; 2]) -> bool {
    u.iter().all(|x| x.is_none())
}

/// `user` as a list of { osc, table } (TOML has no `None` in arrays)
mod user_ser {
    use super::UserTable;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct Slot {
        osc: u8,
        table: UserTable,
    }

    pub fn serialize<S: Serializer>(u: &[Option<UserTable>; 2], s: S) -> Result<S::Ok, S::Error> {
        let v: Vec<Slot> = u
            .iter()
            .enumerate()
            .filter_map(|(o, t)| {
                t.clone().map(|table| Slot {
                    osc: o as u8,
                    table,
                })
            })
            .collect();
        v.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[Option<UserTable>; 2], D::Error> {
        let v = Vec::<Slot>::deserialize(d)?;
        let mut u: [Option<UserTable>; 2] = [None, None];
        for s in v {
            if let Some(x) = u.get_mut(s.osc as usize) {
                *x = Some(s.table);
            }
        }
        Ok(u)
    }
}

#[derive(Clone, Copy)]
struct Env0(Env);
impl Default for Env0 {
    fn default() -> Self {
        Env0(Env::new())
    }
}

#[derive(Clone, Copy, Default)]
pub struct WaveState {
    started: bool,
    ph: [[f32; MAX_UNISON]; 2],
    sub: f32,
    nz: u32,
    pos: [f32; 2],
    lad: [crate::ladder::Ladder; 2],
    fenv: Env0,
    aenv: Env0,
    gate_was: bool,
    pub done: bool,
}

#[inline]
fn xs(n: &mut u32) -> f32 {
    *n ^= *n << 13;
    *n ^= *n >> 17;
    *n ^= *n << 5;
    *n as f32 / u32::MAX as f32 * 2.0 - 1.0
}

impl WaveState {
    /// one stereo sample (l, r, stereo). Allocation-free.
    pub fn render(
        &mut self,
        w: &WaveP,
        ov: Ov,
        freq: f32,
        sr: f32,
        gate: bool,
    ) -> (f32, f32, bool) {
        if self.done {
            return (0.0, 0.0, true);
        }
        let v = |i: usize| WAVE[i].map(ov.get(i).unwrap_or(w.p[i]));
        let dt = 1.0 / sr;
        if self.gate_was && !gate {
            self.fenv.0.release();
            self.aenv.0.release();
        }
        self.gate_was = gate;
        let fe = self.fenv.0.next(
            &Adsr {
                a: v(17),
                d: v(18),
                s: v(19),
                r: v(20),
            },
            dt,
        );
        let ae = self.aenv.0.next(
            &Adsr {
                a: v(21),
                d: v(22),
                s: v(23),
                r: v(24),
            },
            dt,
        );
        if self.aenv.0.done() {
            self.done = true;
            return (0.0, 0.0, true);
        }
        let target = [
            (v(0) + v(26) * fe).clamp(0.0, 1.0),
            (v(1) + v(27) * fe).clamp(0.0, 1.0),
        ];
        if !self.started {
            self.started = true;
            self.nz = 0x6A09_E667 ^ freq.to_bits().wrapping_mul(2_654_435_761) | 1;
            self.gate_was = gate;
            self.pos = target;
            // unison voices start spread around the cycle (deterministic)
            for o in 0..2 {
                for i in 1..MAX_UNISON {
                    self.ph[o][i] = (i as f32 * 0.618_034 + o as f32 * 0.37).fract();
                }
            }
        }
        // position smoothing (~5 ms): automation steps don't click
        let a = (dt * 200.0).min(1.0);
        for o in 0..2 {
            self.pos[o] += (target[o] - self.pos[o]) * a;
        }
        let f0 = freq.max(1.0);
        let n = (v(13).round() as usize).clamp(1, MAX_UNISON);
        let det = v(5) * 50.0;
        let spread = v(6);
        let ug = 1.0 / (n as f32).sqrt();
        let lv = [v(8), v(7)];
        let tune = [v(9).round() + v(10) * 0.01, v(11).round() + v(12) * 0.01];
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for o in 0..2 {
            let fo = f0 * 2f32.powf(tune[o] / 12.0);
            let tab = w.table(o);
            for i in 0..n {
                let s = if n == 1 {
                    0.0
                } else {
                    2.0 * i as f32 / (n - 1) as f32 - 1.0
                };
                let f = fo * 2f32.powf(s * det / 1200.0);
                let ph = self.ph[o][i] + f * dt;
                self.ph[o][i] = ph - ph.floor();
                if lv[o] <= 0.0 {
                    continue;
                }
                let (Some(t), Some(k)) = (tab, Table::level(f, sr)) else {
                    continue;
                };
                let y = t.sample(self.pos[o], self.ph[o][i], k) * lv[o] * ug;
                let pan = s * spread;
                l += y * (1.0 - pan).min(1.0);
                r += y * (1.0 + pan).min(1.0);
            }
        }
        // sub (sine an octave below A) and noise, centred
        let fs = f0 * 2f32.powf(v(9).round() / 12.0) * 0.5;
        self.sub = (self.sub + fs * dt).fract();
        let c = (self.sub * std::f32::consts::TAU).sin() * v(14) * 0.8 + xs(&mut self.nz) * v(15) * 0.5;
        l += c;
        r += c;
        let drive = v(25);
        let g = 1.0 + 3.0 * drive;
        let (mut yl, mut yr) = (l * g, r * g);
        if w.filter_on {
            let kt = (f0 / C4).powf(v(16));
            let cut = (v(2) * kt * 2f32.powf(v(4) * 6.0 * fe)).clamp(20.0, sr * 0.45);
            yl = self.lad[0].process(yl, cut, v(3), sr, w.mode, w.pole4, w.vintage);
            yr = self.lad[1].process(yr, cut, v(3), sr, w.mode, w.pole4, w.vintage);
        }
        if drive > 0.001 {
            yl = yl.tanh();
            yr = yr.tanh();
        }
        let out = ae * w.level * 0.5;
        let (yl, yr) = (yl * out, yr * out);
        if yl.is_finite() && yr.is_finite() {
            (yl, yr, true)
        } else {
            self.done = true;
            (0.0, 0.0, true)
        }
    }
}

/// factory presets: (name, tip, patch, glide seconds, legato)
pub fn presets() -> Vec<(&'static str, &'static str, WaveP, f32, bool)> {
    let wv = |t: [u8; 2], vals: &[f32]| {
        let mut w = WaveP {
            tables: t,
            ..Default::default()
        };
        w.set(vals);
        w
    };
    vec![
        (
            "Wave Supersaw",
            "Wave: 7 detuned saws (Basic Shapes) per side, wide, bright lead / pad",
            wv(
                [0, 0],
                &[
                    0.67, 0.67, 7000.0, 0.1, 0.2, 0.35, 0.9, 0.0, 1.0, 0.0, 0.0, 12.0, 0.0, 7.0,
                    0.25, 0.0, 0.5, 0.002, 0.6, 0.4, 0.4, 0.004, 0.6, 0.9, 0.3, 0.0, 0.0, 0.0,
                ],
            ),
            0.0,
            false,
        ),
        (
            "Wave Vowel Bass",
            "Wave: the Formant Vowels table swept by the filter envelope, mono, with sub",
            wv(
                [3, 1],
                &[
                    0.05, 0.2, 900.0, 0.35, 0.45, 0.05, 0.1, 0.25, 1.0, -12.0, 0.0, -12.0, 4.0,
                    1.0, 0.55, 0.0, 0.4, 0.002, 0.35, 0.2, 0.2, 0.001, 0.4, 0.75, 0.08, 0.2, 0.8,
                    0.0,
                ],
            ),
            0.03,
            true,
        ),
        (
            "Wave Glass Keys",
            "Wave: FM Sweep + Odd Harmonics, a bright attack that settles",
            wv(
                [4, 8],
                &[
                    0.15, 0.5, 9000.0, 0.05, 0.2, 0.08, 0.5, 0.45, 1.0, 0.0, 0.0, 12.0, 3.0, 2.0,
                    0.0, 0.0, 0.6, 0.001, 0.8, 0.0, 0.6, 0.001, 1.6, 0.0, 0.7, 0.0, 0.55, -0.4,
                ],
            ),
            0.0,
            false,
        ),
        (
            "Wave Evolving Pad",
            "Wave: Harmonic Sweep and Digital Noise, slow envelopes open the tables",
            wv(
                [2, 9],
                &[
                    0.1, 0.2, 2600.0, 0.2, 0.3, 0.3, 1.0, 0.6, 0.9, 0.0, 0.0, 7.0, 0.0, 5.0, 0.0,
                    0.03, 0.4, 1.8, 2.5, 0.6, 2.5, 1.2, 2.0, 0.9, 2.5, 0.0, 0.7, 0.6,
                ],
            ),
            0.0,
            false,
        ),
        (
            "Wave Chip Pluck",
            "Wave: Pulse Width + Bit Crush, short and square-ish (chiptune arps)",
            wv(
                [1, 7],
                &[
                    0.0, 0.85, 12000.0, 0.0, 0.0, 0.0, 0.3, 0.35, 1.0, 0.0, 0.0, 12.0, 0.0, 1.0,
                    0.0, 0.0, 0.5, 0.001, 0.2, 0.0, 0.1, 0.001, 0.25, 0.0, 0.08, 0.0, 0.6, 0.0,
                ],
            ),
            0.0,
            false,
        ),
        (
            "Wave Growl",
            "Wave: Wavefold and Sync Sweep with drive, for modulating Pos A / Pos B",
            wv(
                [6, 5],
                &[
                    0.4, 0.5, 1800.0, 0.4, 0.4, 0.1, 0.4, 0.6, 1.0, -12.0, 0.0, -12.0, -7.0, 2.0,
                    0.4, 0.0, 0.3, 0.002, 0.5, 0.4, 0.2, 0.002, 0.5, 0.85, 0.12, 0.5, 0.3, 0.3,
                ],
            ),
            0.02,
            true,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{GenEngine, GenPatch, NO_KNOBS};

    fn note(w: &WaveP, freq: f32, sr: f32, n: usize) -> (Vec<f32>, Vec<f32>) {
        crate::wavetable::init();
        let mut s = WaveState::default();
        let mac = [-1.0; 8];
        let ov = Ov {
            mac: &mac,
            knob: &NO_KNOBS,
        };
        (0..n).map(|_| {
            let (l, r, _) = s.render(w, ov, freq, sr, true);
            (l, r)
        })
        .unzip()
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    /// magnitude of the DFT bin at `hz`
    fn bin(v: &[f32], hz: f32, sr: f32) -> f32 {
        let (mut a, mut b) = (0.0f64, 0.0f64);
        for (i, x) in v.iter().enumerate() {
            let p = std::f64::consts::TAU * hz as f64 * i as f64 / sr as f64;
            a += *x as f64 * p.cos();
            b += *x as f64 * p.sin();
        }
        ((a * a + b * b).sqrt() / v.len() as f64) as f32
    }

    fn plain() -> WaveP {
        let mut w = WaveP::default();
        w.filter_on = false;
        w.set(&[0.67, 0.0, 6000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
        w.p[23] = 1.0; // sustain
        w
    }

    #[test]
    fn a_high_saw_has_no_aliases() {
        // saw position of Basic Shapes at 3.1 kHz, 48 kHz: harmonics 1..7 sit
        // below Nyquist; aliases of 8, 9, ... would land at 23.2k mirrored
        let w = plain();
        let sr = 48000.0;
        let f = 3100.0;
        let (l, _) = note(&w, f, sr, 48000);
        let tail = &l[4800..];
        assert!(bin(tail, f, sr) > 0.05);
        // the first alias images: 48000 - 8 f, 9 f - 48000, ...
        for h in 8..15 {
            let a = (h as f32 * f) % sr;
            let im = if a > sr / 2.0 { sr - a } else { a };
            let m = bin(tail, im, sr);
            assert!(m < 2e-4, "alias of harmonic {h} at {im} Hz: {m}");
        }
    }

    #[test]
    fn position_scans_the_frames() {
        let mut w = plain();
        w.p[0] = 0.0; // sine
        let (a, _) = note(&w, 220.0, 48000.0, 9600);
        w.p[0] = 1.0; // square
        let (b, _) = note(&w, 220.0, 48000.0, 9600);
        let (ha, hb) = (bin(&a[2400..], 660.0, 48000.0), bin(&b[2400..], 660.0, 48000.0));
        assert!(ha < 0.01 && hb > 0.05, "3rd harmonic: sine {ha}, square {hb}");
    }

    #[test]
    fn unison_spreads_into_stereo_and_mono_stays_centred() {
        let mut w = plain();
        let (l, r) = note(&w, 220.0, 48000.0, 9600);
        assert_eq!(l, r, "one voice is mono");
        w.set(&[0.67, 0.0, 6000.0, 0.0, 0.0, 0.5, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 7.0]);
        let (l, r) = note(&w, 220.0, 48000.0, 9600);
        let d: Vec<f32> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
        assert!(rms(&d) > 0.05 * rms(&l), "spread: {} vs {}", rms(&d), rms(&l));
        assert!(rms(&l) > 0.05);
    }

    #[test]
    fn sub_noise_filter_and_envelopes_behave() {
        let mut w = plain();
        w.p[8] = 0.0; // osc A off
        w.p[14] = 1.0; // sub only
        let (l, _) = note(&w, 440.0, 48000.0, 9600);
        assert!(bin(&l[2400..], 220.0, 48000.0) > 0.1, "sub an octave down");
        // a closed low-pass darkens the saw
        let mut open = plain();
        open.filter_on = true;
        open.set(&[0.67, 0.0, 18000.0]);
        let mut shut = open.clone();
        shut.set(&[0.67, 0.0, 300.0]);
        let (a, _) = note(&open, 220.0, 48000.0, 9600);
        let (b, _) = note(&shut, 220.0, 48000.0, 9600);
        assert!(bin(&b[2400..], 2200.0, 48000.0) < 0.2 * bin(&a[2400..], 2200.0, 48000.0));
        // release ends the voice
        let mut s = WaveState::default();
        let mac = [-1.0; 8];
        let ov = Ov {
            mac: &mac,
            knob: &NO_KNOBS,
        };
        for _ in 0..4800 {
            s.render(&open, ov, 220.0, 48000.0, true);
        }
        for _ in 0..48000 {
            s.render(&open, ov, 220.0, 48000.0, false);
        }
        assert!(s.done);
    }

    #[test]
    fn knob_overrides_reach_the_position() {
        crate::wavetable::init();
        let w = plain();
        let mac = [-1.0; 8];
        let mut knob = NO_KNOBS;
        knob[0] = 0.0; // automation: Pos A -> sine
        let mut s = WaveState::default();
        let v: Vec<f32> = (0..9600)
            .map(|_| {
                s.render(
                    &w,
                    Ov {
                        mac: &mac,
                        knob: &knob,
                    },
                    220.0,
                    48000.0,
                    true,
                )
                .0
            })
            .collect();
        assert!(bin(&v[2400..], 660.0, 48000.0) < 0.01);
    }

    #[test]
    fn presets_render_finite_and_audible_and_serde_round_trips() {
        for (name, _, p, _, _) in presets() {
            let g = GenPatch {
                engine: GenEngine::Wave(p.clone()),
                ..Default::default()
            };
            crate::wavetable::init();
            let out = crate::gen::bake(&g, 220.0, 48000.0, 2.0);
            assert!(out.iter().all(|x| x.is_finite()), "{name}");
            let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.05 && peak < 2.0, "{name}: peak {peak}");
            let t = toml::to_string(&g).unwrap();
            let back: GenPatch = toml::from_str(&t).unwrap();
            assert_eq!(back, g, "{name}");
        }
        // an imported table on B only survives a TOML round trip
        let cyc: Vec<i16> = (0..600)
            .map(|i| ((i as f32 / 600.0 * std::f32::consts::TAU).sin() * 20000.0) as i16)
            .collect();
        let mut w = WaveP::default();
        w.user[1] = Some(UserTable::new("mine", 600, cyc));
        w.tables[1] = USER;
        let g = GenPatch {
            engine: GenEngine::Wave(w),
            ..Default::default()
        };
        let t = toml::to_string(&g).unwrap();
        assert!(t.contains("mine"));
        let back: GenPatch = toml::from_str(&t).unwrap();
        assert_eq!(back, g);
        let GenEngine::Wave(b) = &back.engine else {
            unreachable!()
        };
        assert!(b.user[0].is_none() && b.table(1).is_some());
    }
}
