//! Wavetables (Unreleased): the tables of the **Wave** generator
//! (`GenEngine::Wave`, the engine itself is in `src/gen_wave.rs`).
//!
//! A table is 1..=256 single-cycle frames. Each frame is stored as a set of
//! band-limited **mipmaps**, one per octave: level `k` keeps the harmonics up
//! to `512 >> k`, at four samples per cycle of its highest harmonic (so
//! linear interpolation stays clean). The oscillator picks the level whose
//! highest harmonic stays below Nyquist for the note it plays, so tables never
//! alias. Mipmaps come from an FFT of each frame, built when a table is made
//! or loaded (never on the audio thread).
//!
//! The built-in tables are generated in code here (shapes, pulse widths,
//! additive sweeps, the Voice generator's vowel formants, FM, sync, folding,
//! bit reduction, random spectra), so every one is ChipWave's own and freely
//! redistributable. User tables are imported from WAV files: a single cycle
//! of any length, or several 2048-sample frames (Serum-style; a `clm ` chunk
//! `<!>N` sets another frame size). Design ideas from Vital by Matt Tytel and
//! Surge XT's wavetable oscillator (both GPL-3; ideas only, no code copied).
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::f64::consts::TAU;
use std::sync::{Arc, OnceLock};

/// the frame length of imported multi-frame tables (Serum-style)
pub const FRAME: usize = 2048;
/// most frames in a table
pub const MAX_FRAMES: usize = 256;
/// mipmap levels: harmonics 512, 256, ... 1
pub const LEVELS: usize = 10;
/// highest harmonic kept in a table (level 0)
pub const MAX_HARM: usize = 512;

const fn lens() -> [usize; LEVELS] {
    let mut l = [0; LEVELS];
    let mut k = 0;
    while k < LEVELS {
        let h = MAX_HARM >> k;
        l[k] = if h * 4 > 64 { h * 4 } else { 64 };
        k += 1;
    }
    l
}
/// samples per cycle of each level
pub const LENS: [usize; LEVELS] = lens();
const fn offs() -> ([usize; LEVELS], usize) {
    let mut o = [0; LEVELS];
    let mut acc = 0;
    let mut k = 0;
    while k < LEVELS {
        o[k] = acc;
        acc += LENS[k] + 1; // + a guard sample (= sample 0)
        k += 1;
    }
    (o, acc)
}
const OFFS: [usize; LEVELS] = offs().0;
/// floats per frame (all levels)
pub const STRIDE: usize = offs().1;

// ------------------------------------------------------------------ FFT

/// in-place radix-2 complex FFT (`n` a power of two); `inv` = e^{+i}
fn fft(re: &mut [f64], im: &mut [f64], inv: bool) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = TAU / len as f64 * if inv { 1.0 } else { -1.0 };
        let h = len / 2;
        for i in (0..n).step_by(len) {
            for k in 0..h {
                let (wi, wr) = (ang * k as f64).sin_cos();
                let (xr, xi) = (re[i + k + h], im[i + k + h]);
                let (br, bi) = (xr * wr - xi * wi, xr * wi + xi * wr);
                let (ar, ai) = (re[i + k], im[i + k]);
                re[i + k] = ar + br;
                im[i + k] = ai + bi;
                re[i + k + h] = ar - br;
                im[i + k + h] = ai - bi;
            }
        }
        len <<= 1;
    }
}

/// harmonics 1..=MAX_HARM of one cycle (`x` = any length), as complex
/// amplitudes so that x(t) = sum 2 Re(c_h e^{i h t}) (DC dropped)
fn spectrum(x: &[f32]) -> Vec<(f64, f64)> {
    let n = x.len();
    let hmax = MAX_HARM.min(n.saturating_sub(1) / 2);
    let mut out = vec![(0.0, 0.0); MAX_HARM + 1];
    if n == 0 {
        return out;
    }
    if n.is_power_of_two() {
        let mut re: Vec<f64> = x.iter().map(|v| *v as f64).collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im, false);
        for h in 1..=hmax {
            out[h] = (re[h] / n as f64, im[h] / n as f64);
        }
    } else {
        // any other length (single-cycle WAVs): a direct DFT of the kept bins
        for (h, o) in out.iter_mut().enumerate().take(hmax + 1).skip(1) {
            let (mut sr, mut si) = (0.0, 0.0);
            for (t, v) in x.iter().enumerate() {
                let a = -TAU * (h * t) as f64 / n as f64;
                sr += *v as f64 * a.cos();
                si += *v as f64 * a.sin();
            }
            *o = (sr / n as f64, si / n as f64);
        }
    }
    out
}

// ------------------------------------------------------------------ tables

/// A mipmapped wavetable (immutable once built; shared by `Arc`).
pub struct Table {
    pub name: String,
    pub frames: usize,
    data: Vec<f32>,
}

impl std::fmt::Debug for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Table({:?}, {} frames)", self.name, self.frames)
    }
}

impl Table {
    /// build from frames of one cycle each (any length), peak-normalised
    pub fn from_cycles(name: &str, cycles: &[Vec<f32>]) -> Table {
        let frames = cycles.len().clamp(1, MAX_FRAMES);
        let mut data = vec![0.0f32; frames * STRIDE];
        let specs: Vec<Vec<(f64, f64)>> = (0..frames)
            .map(|f| spectrum(cycles.get(f).map(|c| c.as_slice()).unwrap_or(&[])))
            .collect();
        // one gain for the whole table (frames keep their relative levels),
        // from the loudest level-0 frame
        let mut peak = 0.0f32;
        for (f, sp) in specs.iter().enumerate() {
            for k in 0..LEVELS {
                let len = LENS[k];
                let hmax = (MAX_HARM >> k).min(len / 2 - 1);
                let mut re = vec![0.0; len];
                let mut im = vec![0.0; len];
                for h in 1..=hmax {
                    re[h] = sp[h].0;
                    im[h] = sp[h].1;
                    re[len - h] = sp[h].0;
                    im[len - h] = -sp[h].1;
                }
                fft(&mut re, &mut im, true);
                let base = f * STRIDE + OFFS[k];
                for j in 0..len {
                    data[base + j] = re[j] as f32;
                }
                data[base + len] = re[0] as f32;
                if k == 0 {
                    peak = data[base..base + len]
                        .iter()
                        .fold(peak, |m, v| m.max(v.abs()));
                }
            }
        }
        if peak > 1e-6 {
            let g = 1.0 / peak;
            data.iter_mut().for_each(|v| *v *= g);
        }
        Table {
            name: name.into(),
            frames,
            data,
        }
    }

    /// mip level for a note at `freq` Hz (its harmonics stay below Nyquist);
    /// None = above Nyquist (silent)
    #[inline]
    pub fn level(freq: f32, sr: f32) -> Option<usize> {
        let ratio = freq * MAX_HARM as f32 / (sr * 0.5);
        if ratio <= 1.0 {
            return Some(0);
        }
        let k = ratio.log2().ceil() as usize;
        (k < LEVELS).then_some(k)
    }

    /// one sample: frame position `pos` 0..1 (frames interpolated), phase
    /// `ph` 0..1, mip level `k`. Allocation-free.
    #[inline]
    pub fn sample(&self, pos: f32, ph: f32, k: usize) -> f32 {
        let fp = pos.clamp(0.0, 1.0) * (self.frames - 1) as f32;
        let f0 = (fp as usize).min(self.frames - 1);
        let f1 = (f0 + 1).min(self.frames - 1);
        let t = fp - f0 as f32;
        let len = LENS[k];
        let x = ph.rem_euclid(1.0) * len as f32;
        let j = (x as usize).min(len - 1);
        let fr = x - j as f32;
        let at = |f: usize| {
            let b = f * STRIDE + OFFS[k] + j;
            let (a, c) = (self.data[b], self.data[b + 1]);
            a + (c - a) * fr
        };
        let a = at(f0);
        if t <= 0.0 || f1 == f0 {
            a
        } else {
            a + (at(f1) - a) * t
        }
    }

    /// a frame at level 0 resampled to `n` points (UI drawings)
    pub fn frame_points(&self, pos: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| self.sample(pos, i as f32 / n as f32, 0))
            .collect()
    }
}

// ------------------------------------------------------------------ built-ins

/// built-in table names, in index order (append only: songs store the index)
pub const BUILTIN: [&str; 10] = [
    "Basic Shapes",
    "Pulse Width",
    "Harmonic Sweep",
    "Formant Vowels",
    "FM Sweep",
    "Sync Sweep",
    "Wavefold",
    "Bit Crush",
    "Odd Harmonics",
    "Digital Noise",
];
const BUILTIN_FRAMES: usize = 32;
/// samples per generated cycle (oversampled; the FFT keeps 512 harmonics)
const GEN_N: usize = 4096;

fn tri(x: f64) -> f64 {
    1.0 - 4.0 * (x - 0.5).abs()
}
fn saw(x: f64) -> f64 {
    2.0 * x - 1.0
}
fn square(x: f64, w: f64) -> f64 {
    if x < w {
        1.0
    } else {
        -1.0
    }
}

/// a deterministic pseudo-random value in 0..1 from an integer
fn hash01(n: u64) -> f64 {
    let mut z = n.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

/// additive cycle from harmonic amplitudes (sine phase)
fn additive(amps: &[f64]) -> Vec<f32> {
    (0..GEN_N)
        .map(|i| {
            let x = i as f64 / GEN_N as f64;
            amps.iter()
                .enumerate()
                .map(|(h, a)| a * (TAU * (h + 1) as f64 * x).sin())
                .sum::<f64>() as f32
        })
        .collect()
}

/// one frame (t = 0..1 across the table) of built-in table `i`
fn builtin_cycle(i: usize, t: f64) -> Vec<f32> {
    let td = |f: &dyn Fn(f64) -> f64| -> Vec<f32> {
        (0..GEN_N)
            .map(|s| f(s as f64 / GEN_N as f64) as f32)
            .collect()
    };
    match i {
        // sine -> triangle -> saw -> square
        0 => {
            let seg = (t * 3.0).min(2.999);
            let (k, u) = (seg as usize, seg.fract());
            let shape = |k: usize, x: f64| match k {
                0 => (TAU * x).sin(),
                1 => tri((x + 0.25).fract()),
                2 => saw((x + 0.5).fract()),
                _ => square(x, 0.5),
            };
            td(&|x| shape(k, x) * (1.0 - u) + shape(k + 1, x) * u)
        }
        // pulse 50 % -> 3 %
        1 => {
            let w = 0.5 - 0.47 * t;
            td(&|x| square(x, w) - (2.0 * w - 1.0))
        }
        // harmonics 1/h added one by one (1 .. 48)
        2 => {
            let n = 1.0 + t * 47.0;
            let amps: Vec<f64> = (1..=48)
                .map(|h| {
                    let on = (n - (h as f64 - 1.0)).clamp(0.0, 1.0);
                    on / h as f64
                })
                .collect();
            additive(&amps)
        }
        // a saw through the Voice generator's vowel formants A E I O U
        // (bass register; f0 taken as 110 Hz), morphing between vowels
        3 => {
            use crate::voice::{formants, Register, Vowel};
            let vs = [Vowel::A, Vowel::E, Vowel::I, Vowel::O, Vowel::U];
            let seg = (t * 4.0).min(3.999);
            let (k, u) = (seg as usize, seg.fract());
            let gain = |v: Vowel, hz: f64| -> f64 {
                let f = formants(Register::Bass, v);
                (0..5)
                    .map(|j| {
                        let (fc, bw) = (f.f[j] as f64, f.bw[j].max(30.0) as f64);
                        let a = 10f64.powf(f.db[j] as f64 / 20.0);
                        a / (1.0 + ((hz - fc) / (bw * 0.5)).powi(2))
                    })
                    .sum()
            };
            let amps: Vec<f64> = (1..=200)
                .map(|h| {
                    let hz = 110.0 * h as f64;
                    let g = gain(vs[k], hz) * (1.0 - u) + gain(vs[k + 1], hz) * u;
                    g / (h as f64).sqrt()
                })
                .collect();
            additive(&amps)
        }
        // 1:1 FM, index 0 -> 6
        4 => {
            let ix = t * 6.0;
            td(&|x| (TAU * x + ix * (TAU * x).sin()).sin())
        }
        // hard-synced saw, slave ratio 1 -> 7
        5 => {
            let r = 1.0 + t * 6.0;
            td(&|x| saw((x * r).fract()) * (1.0 - x * 0.3))
        }
        // folded sine, gain 1 -> 8
        6 => {
            let g = 1.0 + t * 7.0;
            td(&|x| {
                let y = (TAU * x).sin() * g;
                // triangle fold into -1..1
                let z = ((y + 1.0) * 0.25).rem_euclid(1.0);
                -tri(z)
            })
        }
        // sine quantised to 2^bits levels, 8 bits -> 1 bit
        7 => {
            let bits = 8.0 - t * 7.0;
            let q = 2f64.powf(bits - 1.0);
            td(&|x| ((TAU * x).sin() * q).round().clamp(-q, q) / q)
        }
        // odd harmonics, getting steeper: sine -> square-ish -> buzzy
        8 => {
            let tilt = 2.0 - t * 1.6;
            let amps: Vec<f64> = (1..=96)
                .map(|h| {
                    if h % 2 == 1 {
                        1.0 / (h as f64).powf(tilt)
                    } else {
                        0.0
                    }
                })
                .collect();
            additive(&amps)
        }
        // four seeded random spectra, crossfaded
        _ => {
            let seg = (t * 3.0).min(2.999);
            let (k, u) = (seg as usize, seg.fract());
            let amps: Vec<f64> = (1..=64u64)
                .map(|h| {
                    let a = |s: u64| hash01(s * 1000 + h).powi(3) / (h as f64).powf(0.6);
                    a(k as u64) * (1.0 - u) + a(k as u64 + 1) * u
                })
                .collect();
            additive(&amps)
        }
    }
}

/// build built-in table `i`
pub fn build_builtin(i: usize) -> Table {
    let cycles: Vec<Vec<f32>> = (0..BUILTIN_FRAMES)
        .map(|f| builtin_cycle(i, f as f64 / (BUILTIN_FRAMES - 1) as f64))
        .collect();
    Table::from_cycles(BUILTIN[i.min(BUILTIN.len() - 1)], &cycles)
}

static BUILTINS: OnceLock<Vec<Table>> = OnceLock::new();

/// Build the built-in tables (once per process). Called off the audio
/// thread (engine creation, the UI); the audio thread only reads them.
pub fn init() -> &'static [Table] {
    BUILTINS.get_or_init(|| {
        std::thread::scope(|s| {
            let hs: Vec<_> = (0..BUILTIN.len())
                .map(|i| s.spawn(move || build_builtin(i)))
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        })
    })
}

/// the built-in tables if built (audio thread: never builds)
#[inline]
pub fn builtins() -> Option<&'static [Table]> {
    BUILTINS.get().map(|v| v.as_slice())
}

// ------------------------------------------------------------------ user tables

/// An imported table, saved in the song: the original frames (16-bit) plus
/// the mipmaps built from them on load.
#[derive(Clone)]
pub struct UserTable {
    /// original cycles, each `frame_len` samples
    pub frame_len: usize,
    pub samples: Arc<Vec<i16>>,
    pub table: Arc<Table>,
}

impl std::fmt::Debug for UserTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "UserTable({:?}, {} frames)",
            self.table.name, self.table.frames
        )
    }
}

impl PartialEq for UserTable {
    fn eq(&self, o: &Self) -> bool {
        self.table.name == o.table.name
            && self.frame_len == o.frame_len
            && (Arc::ptr_eq(&self.samples, &o.samples) || self.samples == o.samples)
    }
}

impl UserTable {
    pub fn new(name: &str, frame_len: usize, samples: Vec<i16>) -> UserTable {
        let fl = frame_len.max(1);
        let cycles: Vec<Vec<f32>> = samples
            .chunks(fl)
            .filter(|c| c.len() == fl)
            .take(MAX_FRAMES)
            .map(|c| c.iter().map(|s| *s as f32 / 32768.0).collect())
            .collect();
        UserTable {
            frame_len: fl,
            table: Arc::new(Table::from_cycles(name, &cycles)),
            samples: Arc::new(samples),
        }
    }
    pub fn name(&self) -> &str {
        &self.table.name
    }
}

#[derive(Serialize, Deserialize)]
struct UserTableSer {
    name: String,
    frame_len: usize,
    /// 16-bit little-endian PCM, base64
    data: String,
}

impl Serialize for UserTable {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use base64::Engine;
        let bytes: Vec<u8> = self.samples.iter().flat_map(|v| v.to_le_bytes()).collect();
        UserTableSer {
            name: self.table.name.clone(),
            frame_len: self.frame_len,
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for UserTable {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use base64::Engine;
        let u = UserTableSer::deserialize(d)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(u.data.as_bytes())
            .map_err(serde::de::Error::custom)?;
        let samples: Vec<i16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect();
        if u.frame_len == 0 || samples.len() < u.frame_len {
            return Err(serde::de::Error::custom("empty wavetable"));
        }
        Ok(UserTable::new(&u.name, u.frame_len, samples))
    }
}

/// the frame size a Serum-style `clm ` chunk names ("<!>2048 ..."), if any
fn clm_frame(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        let body = bytes.get(at + 8..(at + 8 + len).min(bytes.len()))?;
        if id == b"clm " {
            let t = String::from_utf8_lossy(body);
            let n: String = t
                .trim_start_matches("<!>")
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            return n.parse().ok().filter(|n: &usize| (16..=65536).contains(n));
        }
        at += 8 + len + (len & 1);
    }
    None
}

/// Import a WAV: frames of `clm` size, else 2048-sample frames when the
/// length is a multiple of 2048, else one single cycle (up to 8192 samples).
pub fn import_wav_bytes(name: &str, bytes: &[u8]) -> Result<UserTable, String> {
    let mut r = hound::WavReader::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let s = 2f32.powi(spec.bits_per_sample as i32 - 1);
            r.samples::<i32>()
                .filter_map(Result::ok)
                .map(|v| v as f32 / s)
                .collect()
        }
    };
    let mono: Vec<f32> = raw
        .chunks(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect();
    let n = mono.len();
    if n < 8 {
        return Err("the file is too short for a wavetable".into());
    }
    let frame_len = match clm_frame(bytes) {
        Some(f) if n >= f => f,
        _ if n >= FRAME && n.is_multiple_of(FRAME) => FRAME,
        _ if n <= 8192 => n,
        _ => {
            return Err(format!(
                "{n} samples is neither a single cycle (up to 8192) nor whole {FRAME}-sample frames"
            ))
        }
    };
    let frames = (n / frame_len).min(MAX_FRAMES);
    let peak = mono[..frames * frame_len]
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()))
        .max(1e-9);
    let samples: Vec<i16> = mono[..frames * frame_len]
        .iter()
        .map(|v| (v / peak * 32767.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect();
    Ok(UserTable::new(name, frame_len, samples))
}

pub fn import_wav(path: &std::path::Path) -> Result<UserTable, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported".into());
    import_wav_bytes(&name, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_round_trip_and_sine_bin() {
        let n = 64;
        let x: Vec<f32> = (0..n)
            .map(|i| (TAU * 3.0 * i as f64 / n as f64).sin() as f32)
            .collect();
        let sp = spectrum(&x);
        // sin = (e^{i} - e^{-i}) / 2i: bin 3 = -0.5 i
        assert!((sp[3].1 + 0.5).abs() < 1e-6 && sp[3].0.abs() < 1e-6);
        assert!(sp[2].0.abs() + sp[2].1.abs() < 1e-6);
        // non power-of-two lengths take the DFT path with the same answer
        let y: Vec<f32> = (0..60)
            .map(|i| (TAU * 3.0 * i as f64 / 60.0).sin() as f32)
            .collect();
        assert!((spectrum(&y)[3].1 + 0.5).abs() < 1e-6);
    }

    #[test]
    fn mip_levels_hold_no_harmonics_above_their_limit() {
        // a saw (all harmonics) at every level: its spectrum ends at 512 >> k
        let saw: Vec<f32> = (0..GEN_N)
            .map(|i| (2.0 * i as f64 / GEN_N as f64 - 1.0) as f32)
            .collect();
        let t = Table::from_cycles("saw", &[saw]);
        for k in 0..LEVELS {
            let len = LENS[k];
            let cyc: Vec<f32> = (0..len).map(|j| t.data[OFFS[k] + j]).collect();
            let sp = spectrum(&cyc);
            let lim = (MAX_HARM >> k).min(len / 2 - 1);
            let above: f64 = sp.iter().skip(lim + 1).map(|(r, i)| r * r + i * i).sum();
            let kept: f64 = sp.iter().take(lim + 1).map(|(r, i)| r * r + i * i).sum();
            assert!(above < kept * 1e-12, "level {k}: {above} vs {kept}");
            assert_eq!(t.data[OFFS[k] + len], t.data[OFFS[k]], "guard sample");
        }
    }

    #[test]
    fn level_choice_keeps_harmonics_below_nyquist() {
        for &sr in &[44100.0f32, 48000.0, 96000.0] {
            for n in 0..128 {
                let f = 8.18 * 2f32.powf(n as f32 / 12.0);
                match Table::level(f, sr) {
                    Some(k) => assert!(((MAX_HARM >> k) as f32) * f <= sr * 0.5 + 0.01),
                    None => assert!(f > sr * 0.5, "{f} Hz at {sr}"),
                }
            }
        }
        assert_eq!(Table::level(20.0, 48000.0), Some(0));
    }

    #[test]
    fn builtins_are_finite_normalised_and_named() {
        let b = init();
        assert_eq!(b.len(), BUILTIN.len());
        for t in b {
            assert_eq!(t.frames, BUILTIN_FRAMES);
            let peak = t.data.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(t.data.iter().all(|v| v.is_finite()), "{}", t.name);
            assert!(peak > 0.5 && peak < 1.6, "{}: {peak}", t.name);
            // the first and last frames differ (the table morphs)
            let a = t.frame_points(0.0, 64);
            let z = t.frame_points(1.0, 64);
            let d: f32 = a.iter().zip(&z).map(|(x, y)| (x - y).abs()).sum();
            assert!(d > 1.0, "{} does not morph", t.name);
        }
        // the basic table starts as a sine
        let s = b[0].frame_points(0.0, 64);
        let pk = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        for (i, v) in s.iter().map(|v| v / pk).enumerate() {
            let want = (TAU * i as f64 / 64.0).sin() as f32;
            assert!((v - want).abs() < 0.02, "{i}: {v} vs {want}");
        }
    }

    fn wav(samples: &[f32], clm: Option<&str>) -> Vec<u8> {
        let mut c = std::io::Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(
                &mut c,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 44100,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for s in samples {
                w.write_sample((s * 32000.0) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let mut b = c.into_inner();
        if let Some(t) = clm {
            // append a clm chunk and fix the RIFF size
            b.extend_from_slice(b"clm ");
            b.extend_from_slice(&(t.len() as u32).to_le_bytes());
            b.extend_from_slice(t.as_bytes());
            if t.len() % 2 == 1 {
                b.push(0);
            }
            let riff = (b.len() - 8) as u32;
            b[4..8].copy_from_slice(&riff.to_le_bytes());
        }
        b
    }

    #[test]
    fn imports_single_cycles_frames_and_clm_sizes_and_round_trips() {
        // single cycle of 600 samples
        let one: Vec<f32> = (0..600)
            .map(|i| (TAU * i as f64 / 600.0).sin() as f32)
            .collect();
        let u = import_wav_bytes("one", &wav(&one, None)).unwrap();
        assert_eq!((u.frame_len, u.table.frames), (600, 1));
        let p = u.table.frame_points(0.0, 32);
        assert!((p[8] - 1.0).abs() < 0.02, "{}", p[8]);
        // three 2048 frames
        let three: Vec<f32> = (0..3 * 2048)
            .map(|i| ((i % 2048) as f32 / 1024.0 - 1.0) * (1 + i / 2048) as f32 / 3.0)
            .collect();
        let u = import_wav_bytes("three", &wav(&three, None)).unwrap();
        assert_eq!((u.frame_len, u.table.frames), (2048, 3));
        // clm says 256-sample frames
        let small: Vec<f32> = (0..1024)
            .map(|i| (TAU * (i % 256) as f64 / 256.0).sin() as f32)
            .collect();
        let u = import_wav_bytes("clm", &wav(&small, Some("<!>256 10000000 wavetable"))).unwrap();
        assert_eq!((u.frame_len, u.table.frames), (256, 4));
        // too long and not frames
        assert!(import_wav_bytes("x", &wav(&vec![0.1; 9000], None)).is_err());
        assert!(import_wav_bytes("x", b"garbage").is_err());
        // serde: base64 PCM, the mipmaps rebuilt on load
        let t = toml::to_string(&std::collections::BTreeMap::from([("t", u.clone())])).unwrap();
        let back: std::collections::BTreeMap<String, UserTable> = toml::from_str(&t).unwrap();
        assert_eq!(back["t"], u);
        assert_eq!(
            back["t"].table.frame_points(0.3, 16),
            u.table.frame_points(0.3, 16)
        );
    }
}
