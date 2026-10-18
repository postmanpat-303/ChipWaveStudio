//! Speech for the Voice instrument (0.2.3.10): consonants (plosives,
//! fricatives, nasals) as a phoneme timeline, and the "Throat" mode, a
//! waveguide vocal tract.
//!
//! Consonants: noise bursts / frication through a band-pass shaped per place
//! of articulation, voicing on/off, a voice bar for voiced closures, nasal
//! murmur, and formant transitions from the consonant's locus into the next
//! vowel (the formant smoothing makes the glide). Locus values after
//! P. Delattre, A. Liberman & F. Cooper (1955), "Acoustic loci and
//! transitional cues for consonants", JASA 27(4); noise bands after
//! K. N. Stevens, "Acoustic Phonetics" (MIT Press, 1998). Timings are
//! ChipWave's own, tuned for tracker syllables.
//!
//! Throat mode is inspired by **Pink Trombone** by **Neil Thapen** (2017,
//! MIT licence, https://dood.al/pinktrombone/): a 44-section Kelly-Lochbaum
//! tube with a tongue-shaped diameter profile, lips, a 28-section nasal
//! branch behind a velum, glottal and lip reflections, and turbulence noise
//! injected at narrow constrictions. This is an independent Rust
//! implementation of that published structure (the MIT notice is reproduced
//! in THIRD_PARTY_LICENSES); the vowel and consonant tract shapes are
//! ChipWave's own. The tube model itself goes back to J. L. Kelly &
//! C. C. Lochbaum (1962), "Speech synthesis", Proc. 4th Int. Congr. Acoustics.
use crate::synth::Svf;
use crate::voice::{formants, Formants, Register, Vowel};

/// phonemes per word
pub const MAX_PH: usize = 24;
/// words per Voice patch
pub const MAX_WORDS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ph {
    V(Vowel),
    P,
    B,
    T,
    D,
    K,
    G,
    S,
    Z,
    Sh,
    F,
    /// "v" (voiced labiodental fricative)
    Vf,
    H,
    M,
    N,
    Ng,
    Pause,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Vowel,
    Plosive,
    Fricative,
    Nasal,
    Pause,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Lips,
    Teeth,
    Ridge,
    Palate,
    Velum,
    Glottis,
}

impl Ph {
    pub fn class(self) -> Class {
        use Ph::*;
        match self {
            V(_) => Class::Vowel,
            P | B | T | D | K | G => Class::Plosive,
            S | Z | Sh | F | Vf | H => Class::Fricative,
            M | N | Ng => Class::Nasal,
            Pause => Class::Pause,
        }
    }
    pub fn voiced(self) -> bool {
        use Ph::*;
        matches!(self, V(_) | B | D | G | Z | Vf | M | N | Ng)
    }
    pub fn place(self) -> Place {
        use Ph::*;
        match self {
            P | B | M => Place::Lips,
            F | Vf => Place::Teeth,
            T | D | S | Z | N => Place::Ridge,
            Sh => Place::Palate,
            K | G | Ng => Place::Velum,
            _ => Place::Glottis,
        }
    }
    pub fn name(self) -> &'static str {
        use Ph::*;
        match self {
            V(v) => v.name(),
            P => "p",
            B => "b",
            T => "t",
            D => "d",
            K => "k",
            G => "g",
            S => "s",
            Z => "z",
            Sh => "sh",
            F => "f",
            Vf => "v",
            H => "h",
            M => "m",
            N => "n",
            Ng => "ng",
            Pause => ".",
        }
    }
}

/// Parse a word ("da", "ka-ta", "shoom") into `out`; returns the count.
/// Letters: vowels a e i o u, er, ae (ee = i, oo = u, y = i, w = u, r = er);
/// plosives p b t d k g (c q = k, x = k s, ch = t sh, j = d sh);
/// fricatives s z sh f v h (th = f); nasals m n ng; `.` `-` `,` = a short
/// pause. Spaces and other characters are ignored. Allocation-free.
pub fn parse_word(s: &str, out: &mut [Ph; MAX_PH]) -> usize {
    let b = s.as_bytes();
    let mut n = 0;
    let mut push = |p: Ph, n: &mut usize| {
        if *n < MAX_PH {
            out[*n] = p;
            *n += 1;
        }
    };
    let mut i = 0;
    while i < b.len() {
        let c = b[i].to_ascii_lowercase();
        let d = b.get(i + 1).map(|x| x.to_ascii_lowercase()).unwrap_or(0);
        let two = match (c, d) {
            (b's', b'h') => Some(&[Ph::Sh][..]),
            (b'n', b'g') => Some(&[Ph::Ng][..]),
            (b'e', b'r') => Some(&[Ph::V(Vowel::Er)][..]),
            (b'a', b'e') => Some(&[Ph::V(Vowel::Ae)][..]),
            (b'e', b'e') => Some(&[Ph::V(Vowel::I)][..]),
            (b'o', b'o') => Some(&[Ph::V(Vowel::U)][..]),
            (b'c', b'h') => Some(&[Ph::T, Ph::Sh][..]),
            (b't', b'h') => Some(&[Ph::F][..]),
            _ => None,
        };
        if let Some(ps) = two {
            for &p in ps {
                push(p, &mut n);
            }
            i += 2;
            continue;
        }
        let one: &[Ph] = match c {
            b'a' => &[Ph::V(Vowel::A)],
            b'e' => &[Ph::V(Vowel::E)],
            b'i' | b'y' => &[Ph::V(Vowel::I)],
            b'o' => &[Ph::V(Vowel::O)],
            b'u' | b'w' => &[Ph::V(Vowel::U)],
            b'r' => &[Ph::V(Vowel::Er)],
            b'p' => &[Ph::P],
            b'b' => &[Ph::B],
            b't' => &[Ph::T],
            b'd' => &[Ph::D],
            b'k' | b'c' | b'q' => &[Ph::K],
            b'g' => &[Ph::G],
            b'x' => &[Ph::K, Ph::S],
            b'j' => &[Ph::D, Ph::Sh],
            b's' => &[Ph::S],
            b'z' => &[Ph::Z],
            b'f' => &[Ph::F],
            b'v' => &[Ph::Vf],
            b'h' => &[Ph::H],
            b'm' => &[Ph::M],
            b'n' => &[Ph::N],
            b'.' | b'-' | b',' => &[Ph::Pause],
            _ => &[],
        };
        for &p in one {
            push(p, &mut n);
        }
        i += 1;
    }
    n
}

/// "da ka" -> "d A k A": how ChipWave reads a word (UI hint)
pub fn describe(s: &str) -> String {
    let mut ph = [Ph::Pause; MAX_PH];
    let n = parse_word(s, &mut ph);
    ph[..n]
        .iter()
        .map(|p| p.name())
        .collect::<Vec<_>>()
        .join(" ")
}

// ---- timing (seconds at rate 1)
const CLOSE_VL: f32 = 0.055;
const CLOSE_VD: f32 = 0.045;
const BURST: f32 = 0.010;
const ASP_VL: f32 = 0.040;
const ASP_VD: f32 = 0.008;
const FRIC_VL: f32 = 0.110;
const FRIC_VD: f32 = 0.085;
const H_DUR: f32 = 0.070;
const NASAL: f32 = 0.075;
const PAUSE: f32 = 0.080;
/// a vowel that is not the last phoneme of the word
pub const VOWEL: f32 = 0.150;

fn dur(p: Ph, last: bool) -> f32 {
    match p.class() {
        Class::Vowel if last => f32::INFINITY,
        Class::Vowel => VOWEL,
        Class::Plosive if p.voiced() => CLOSE_VD + BURST + ASP_VD,
        Class::Plosive => CLOSE_VL + BURST + ASP_VL,
        Class::Fricative if p == Ph::H => H_DUR,
        Class::Fricative if p.voiced() => FRIC_VD,
        Class::Fricative => FRIC_VL,
        Class::Nasal => NASAL,
        Class::Pause => PAUSE,
    }
}

/// What the generator should do right now.
#[derive(Clone, Copy, Debug)]
pub struct Ctrl {
    /// voicing (glottal source) gain 0..1
    pub vg: f32,
    /// frication / burst noise gain (formant mode: through the noise band)
    pub ng: f32,
    /// aspiration noise into the formants / tract 0..1
    pub asp: f32,
    /// noise band centre and bandwidth (Hz, bass register)
    pub nfc: f32,
    pub nbw: f32,
    /// formant targets (formant mode)
    pub form: Formants,
    /// vowel the tongue is heading for (Throat mode)
    pub vowel: Vowel,
    /// Throat mode constriction: (tract index, diameter)
    pub constr: Option<(f32, f32)>,
    /// velum opening 0.01 (closed) .. 0.45 (nasal)
    pub velum: f32,
    /// formant glide time in ms while speaking
    pub morph_ms: f32,
}

/// F1-F3 of a consonant's locus (bass register, Hz)
fn locus(place: Place) -> [f32; 3] {
    match place {
        Place::Lips | Place::Teeth => [250.0, 800.0, 2200.0],
        Place::Ridge => [250.0, 1700.0, 2700.0],
        Place::Palate => [260.0, 2000.0, 2500.0],
        Place::Velum => [260.0, 1950.0, 2150.0],
        Place::Glottis => [500.0, 1500.0, 2500.0],
    }
}

/// (centre, bandwidth, gain) of a plosive burst / fricative noise
fn noise_band(p: Ph) -> (f32, f32, f32) {
    use Ph::*;
    match p {
        P | B => (1200.0, 1600.0, 0.5),
        T | D => (4500.0, 3000.0, 0.85),
        K | G => (2300.0, 1200.0, 0.85),
        S | Z => (6500.0, 3000.0, 0.3),
        Sh => (3200.0, 1600.0, 0.35),
        F | Vf => (5500.0, 6000.0, 0.16),
        _ => (2000.0, 2000.0, 0.0),
    }
}

/// Throat-mode tract index of a place (0 = glottis .. 44 = lips)
fn tract_index(place: Place) -> f32 {
    match place {
        Place::Lips => 42.5,
        Place::Teeth => 41.0,
        Place::Ridge => 36.5,
        Place::Palate => 33.5,
        Place::Velum => 22.0,
        Place::Glottis => 0.0,
    }
}

/// formants of a register scaled from the bass-register locus
fn reg_scale(reg: Register) -> f32 {
    formants(reg, Vowel::E).f[1] / formants(Register::Bass, Vowel::E).f[1]
}

/// the phoneme timeline of one word (fixed size, allocation-free)
#[derive(Clone, Copy, Debug)]
pub struct Speech {
    ph: [Ph; MAX_PH],
    n: u8,
    i: u8,
    t: f32,
}

impl Default for Speech {
    fn default() -> Self {
        Self {
            ph: [Ph::Pause; MAX_PH],
            n: 0,
            i: 0,
            t: 0.0,
        }
    }
}

impl Speech {
    /// start speaking `word` (an empty word stops speech)
    pub fn speak(&mut self, word: &str) {
        self.n = parse_word(word, &mut self.ph) as u8;
        self.i = 0;
        self.t = 0.0;
    }
    pub fn active(&self) -> bool {
        self.n > 0
    }
    /// the word ended on a consonant: silent until the next word
    pub fn done(&self) -> bool {
        self.n > 0 && self.i >= self.n
    }
    #[cfg(test)]
    pub fn current(&self) -> Option<Ph> {
        (self.i < self.n).then(|| self.ph[self.i as usize])
    }
    /// advance by `dt` seconds of speech (already scaled by the rate)
    pub fn advance(&mut self, dt: f32) {
        if self.n == 0 {
            return;
        }
        self.t += dt;
        while self.i < self.n {
            let d = dur(self.ph[self.i as usize], self.i + 1 == self.n);
            if self.t < d {
                break;
            }
            self.t -= d;
            self.i += 1;
        }
    }

    fn next_vowel(&self) -> Vowel {
        let (n, i) = (self.n as usize, self.i as usize);
        let fwd = (i..n).find_map(|j| match self.ph[j] {
            Ph::V(v) => Some(v),
            _ => None,
        });
        fwd.or_else(|| {
            (0..i.min(n)).rev().find_map(|j| match self.ph[j] {
                Ph::V(v) => Some(v),
                _ => None,
            })
        })
        .unwrap_or(Vowel::A)
    }

    /// control values for the current phoneme; `rate` speeds transitions
    pub fn ctrl(&self, reg: Register, rate: f32) -> Ctrl {
        let nv = self.next_vowel();
        let fnv = formants(reg, nv);
        let mut c = Ctrl {
            vg: 1.0,
            ng: 0.0,
            asp: 0.0,
            nfc: 2000.0,
            nbw: 2000.0,
            form: fnv,
            vowel: nv,
            constr: None,
            velum: 0.01,
            morph_ms: 35.0 / rate.max(0.25),
        };
        if self.done() {
            c.vg = 0.0;
            return c;
        }
        let p = self.ph[self.i as usize];
        let sc = reg_scale(reg);
        let loc = |mut f: Formants| {
            let l = locus(p.place());
            for (d, v) in f.f.iter_mut().zip(l) {
                *d = v * sc;
            }
            f
        };
        let (nfc, nbw, ngain) = noise_band(p);
        c.nfc = nfc;
        c.nbw = nbw;
        let idx = tract_index(p.place());
        match p.class() {
            Class::Vowel => {}
            Class::Pause => c.vg = 0.0,
            Class::Plosive => {
                let close = if p.voiced() { CLOSE_VD } else { CLOSE_VL };
                c.form = loc(fnv);
                if self.t < close {
                    c.constr = Some((idx, 0.0));
                    if p.voiced() {
                        // voice bar: low murmur behind the closure
                        c.vg = 0.06;
                        c.form.f[0] = 200.0;
                        for k in 1..5 {
                            c.form.db[k] = -40.0;
                        }
                    } else {
                        c.vg = 0.0;
                    }
                } else if self.t < close + BURST {
                    c.vg = if p.voiced() { 0.6 } else { 0.0 };
                    c.ng = ngain;
                    c.constr = Some((idx, 0.45));
                } else {
                    // release: formants glide from the locus to the vowel
                    c.form = fnv;
                    if !p.voiced() {
                        c.vg = 0.0;
                        c.asp = 0.55;
                    }
                }
            }
            Class::Fricative if p == Ph::H => {
                c.vg = 0.0;
                c.asp = 0.85;
            }
            Class::Fricative => {
                c.vg = if p.voiced() { 0.45 } else { 0.0 };
                c.ng = ngain;
                c.form = loc(fnv);
                c.constr = Some((idx, if p == Ph::Sh { 0.5 } else { 0.42 }));
            }
            Class::Nasal => {
                c.vg = 0.3;
                let l = locus(p.place());
                c.form.f = [270.0, l[1] * sc, l[2] * sc, fnv.f[3], fnv.f[4]];
                c.form.db = [0.0, -18.0, -24.0, -32.0, -36.0];
                c.constr = Some((idx, 0.0));
                c.velum = 0.45;
            }
        }
        if let Class::Vowel = p.class() {
            if let Ph::V(v) = p {
                c.form = formants(reg, v);
                c.vowel = v;
            }
        }
        c
    }
}

// ------------------------------------------------------------ Throat tract

const TN: usize = 44;
const NOSE_N: usize = 28;
const NOSE_START: usize = TN - NOSE_N + 1;
const BLADE: usize = 10;
const TIP: usize = 32;
const LIP: usize = 39;
const GLOT_REFL: f32 = 0.75;
const LIP_REFL: f32 = -0.85;
/// internal tract rate at formant shift 1 (Pink Trombone runs two tract
/// steps per 44.1 kHz sample)
const TRACT_RATE: f32 = 88_200.0;

/// tongue (index, diameter) and lip opening of a vowel in the tract
pub fn vowel_shape(v: Vowel) -> (f32, f32, f32) {
    match v {
        Vowel::A => (13.0, 2.6, 1.5),
        Vowel::E => (25.0, 2.6, 1.5),
        Vowel::I => (28.0, 2.05, 1.4),
        Vowel::O => (16.0, 2.4, 0.8),
        Vowel::U => (21.5, 2.05, 0.55),
        Vowel::Er => (20.0, 2.45, 1.1),
        Vowel::Ae => (24.5, 3.1, 1.5),
    }
}

/// A Kelly-Lochbaum vocal tract with a nasal branch (see the module notes).
/// Fixed size, allocation-free.
#[derive(Clone, Copy)]
pub struct Tract {
    r: [f32; TN],
    l: [f32; TN],
    jr: [f32; TN + 1],
    jl: [f32; TN + 1],
    refl: [f32; TN + 1],
    d: [f32; TN],
    target: [f32; TN],
    nr: [f32; NOSE_N],
    nl: [f32; NOSE_N],
    njr: [f32; NOSE_N + 1],
    njl: [f32; NOSE_N + 1],
    nrefl: [f32; NOSE_N + 1],
    na: [f32; NOSE_N],
    velum: f32,
    refl_left: f32,
    refl_right: f32,
    refl_nose: f32,
    /// constriction for turbulence (index, current diameter)
    turb_at: Option<f32>,
    turb_f: Svf,
    acc: f32,
    prev: f32,
    cur: f32,
    init: bool,
}

impl Default for Tract {
    fn default() -> Self {
        Self::new()
    }
}

fn rest_diameter(i: usize) -> f32 {
    if (i as f32) < 7.0 * TN as f32 / 44.0 - 0.5 {
        0.6
    } else if i < 12 {
        1.1
    } else {
        1.5
    }
}

impl Tract {
    pub fn new() -> Self {
        let mut t = Self {
            r: [0.0; TN],
            l: [0.0; TN],
            jr: [0.0; TN + 1],
            jl: [0.0; TN + 1],
            refl: [0.0; TN + 1],
            d: [1.5; TN],
            target: [1.5; TN],
            nr: [0.0; NOSE_N],
            nl: [0.0; NOSE_N],
            njr: [0.0; NOSE_N + 1],
            njl: [0.0; NOSE_N + 1],
            nrefl: [0.0; NOSE_N + 1],
            na: [0.0; NOSE_N],
            velum: 0.01,
            refl_left: 0.0,
            refl_right: 0.0,
            refl_nose: 0.0,
            turb_at: None,
            turb_f: Svf::default(),
            acc: 0.0,
            prev: 0.0,
            cur: 0.0,
            init: true,
        };
        for i in 0..NOSE_N {
            let x = 2.0 * i as f32 / NOSE_N as f32;
            let d = if x < 1.0 {
                0.4 + 1.6 * x
            } else {
                0.5 + 1.5 * (2.0 - x)
            };
            let d = d.min(1.9);
            t.na[i] = d * d;
        }
        t
    }

    /// Set the target shape: tongue body (index 12..29, diameter 2.05..3.5),
    /// lip opening, an optional constriction (index, diameter) and the
    /// velum; then move the tract towards it over `dt` seconds. `speed`
    /// scales how fast the articulators move.
    pub fn shape(
        &mut self,
        tongue: (f32, f32),
        lips: f32,
        constr: Option<(f32, f32)>,
        velum: f32,
        dt: f32,
        speed: f32,
    ) {
        let (ti, td) = tongue;
        for i in 0..TN {
            let mut d = rest_diameter(i);
            if (BLADE..LIP).contains(&i) {
                let t = 1.1 * std::f32::consts::PI * (ti - i as f32) / (TIP - BLADE) as f32;
                let fixed = 2.0 + (td - 2.0) / 1.5;
                let mut curve = (1.5 - fixed + 1.7) * t.cos();
                if i == LIP - 1 {
                    curve *= 0.8;
                }
                if i == BLADE || i == LIP - 2 {
                    curve *= 0.94;
                }
                d = 1.5 - curve;
            }
            if i >= LIP {
                d = d.min(lips);
            }
            self.target[i] = d.max(0.0);
        }
        self.turb_at = None;
        if let Some((idx, dia)) = constr {
            let width = if idx < 25.0 {
                10.0
            } else if idx >= TIP as f32 {
                5.0
            } else {
                10.0 - 5.0 * (idx - 25.0) / (TIP as f32 - 25.0)
            };
            for i in 0..TN {
                let rel = (i as f32 - idx).abs() - 0.5;
                let shrink = if rel <= 0.0 {
                    0.0
                } else if rel > width {
                    1.0
                } else {
                    0.5 * (1.0 - (std::f32::consts::PI * rel / width).cos())
                };
                if dia < self.target[i] {
                    self.target[i] = dia + (self.target[i] - dia) * shrink;
                }
            }
            self.turb_at = Some(idx);
        }
        // articulators move at a limited speed (closing faster than opening)
        let up = 25.0 * speed * dt;
        let down = 50.0 * speed * dt;
        for i in 0..TN {
            let (c, t) = (self.d[i], self.target[i]);
            self.d[i] = if self.init {
                t
            } else if c < t {
                (c + up).min(t)
            } else {
                (c - down).max(t)
            };
        }
        let k = if self.init {
            1.0
        } else {
            1.0 - (-dt / 0.02).exp()
        };
        self.velum += (velum - self.velum) * k;
        self.init = false;
        self.reflections();
    }

    fn reflections(&mut self) {
        let a = |d: f32| d * d;
        for i in 1..TN {
            let (a0, a1) = (a(self.d[i - 1]), a(self.d[i]));
            self.refl[i] = if a1 == 0.0 || a0 + a1 < 1e-9 {
                0.999
            } else {
                (a0 - a1) / (a0 + a1)
            };
        }
        self.na[0] = self.velum * self.velum;
        for i in 1..NOSE_N {
            let (a0, a1) = (self.na[i - 1], self.na[i]);
            self.nrefl[i] = (a0 - a1) / (a0 + a1).max(1e-9);
        }
        let s = (a(self.d[NOSE_START]) + a(self.d[NOSE_START + 1]) + self.na[0]).max(1e-9);
        self.refl_left = (2.0 * a(self.d[NOSE_START]) - s) / s;
        self.refl_right = (2.0 * a(self.d[NOSE_START + 1]) - s) / s;
        self.refl_nose = (2.0 * self.na[0] - s) / s;
    }

    fn step(&mut self, glot: f32, turb: f32) -> f32 {
        if let Some(idx) = self.turb_at {
            let i = idx.floor() as usize;
            let delta = idx - i as f32;
            if i + 2 < TN {
                let d = self.d[i.min(TN - 1)];
                let thin = (8.0 * (0.7 - d)).clamp(0.0, 1.0);
                let open = (30.0 * (d - 0.3)).clamp(0.0, 1.0);
                let n0 = turb * (1.0 - delta) * thin * open;
                let n1 = turb * delta * thin * open;
                self.r[i + 1] += n0 * 0.5;
                self.l[i + 1] += n0 * 0.5;
                self.r[i + 2] += n1 * 0.5;
                self.l[i + 2] += n1 * 0.5;
            }
        }
        self.jr[0] = self.l[0] * GLOT_REFL + glot;
        self.jl[TN] = self.r[TN - 1] * LIP_REFL;
        for i in 1..TN {
            let w = self.refl[i] * (self.r[i - 1] + self.l[i]);
            self.jr[i] = self.r[i - 1] - w;
            self.jl[i] = self.l[i] + w;
        }
        let i = NOSE_START;
        let r = self.refl_left;
        self.jl[i] = r * self.r[i - 1] + (1.0 + r) * (self.nl[0] + self.l[i]);
        let r = self.refl_right;
        self.jr[i] = r * self.l[i] + (1.0 + r) * (self.nl[0] + self.r[i - 1]);
        let r = self.refl_nose;
        self.njr[0] = r * self.nl[0] + (1.0 + r) * (self.l[i] + self.r[i - 1]);
        for i in 0..TN {
            self.r[i] = self.jr[i] * 0.999;
            self.l[i] = self.jl[i + 1] * 0.999;
        }
        let lip = self.r[TN - 1];
        self.njl[NOSE_N] = self.nr[NOSE_N - 1] * LIP_REFL;
        for i in 1..NOSE_N {
            let w = self.nrefl[i] * (self.nr[i - 1] + self.nl[i]);
            self.njr[i] = self.nr[i - 1] - w;
            self.njl[i] = self.nl[i] + w;
        }
        for i in 0..NOSE_N {
            self.nr[i] = self.njr[i] * 0.999;
            self.nl[i] = self.njl[i + 1] * 0.999;
        }
        lip + self.nr[NOSE_N - 1]
    }

    /// One output sample at `sr`; `shift` scales the formants (the internal
    /// tract rate). `noise` is white noise for the turbulence.
    pub fn render(&mut self, glot: f32, noise: f32, turb_gain: f32, sr: f32, shift: f32) -> f32 {
        let ratio = (TRACT_RATE * shift / sr).clamp(0.5, 4.0);
        // band-limited turbulence around 1-2 kHz (Pink Trombone: 1 kHz, Q 0.5)
        let g = (std::f32::consts::PI * 1500.0_f32.min(sr * 0.45) / sr).tan();
        let (_, bp, _) = self.turb_f.process_gk(noise, g, 2.0);
        let turb = bp * turb_gain;
        self.acc += ratio;
        while self.acc >= 1.0 {
            self.prev = self.cur;
            self.cur = self.step(glot, turb);
            self.acc -= 1.0;
        }
        let v = self.prev + (self.cur - self.prev) * self.acc;
        if v.is_finite() {
            v
        } else {
            *self = Self::new();
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_parse_into_phonemes() {
        assert_eq!(describe("da ka"), "d A k A");
        assert_eq!(describe("SHoom"), "sh U m");
        assert_eq!(describe("ng-er"), "ng . ER");
        assert_eq!(describe("tax!"), "t A k s");
        assert_eq!(describe("chae"), "t sh AE");
        assert_eq!(describe(""), "");
        let long = "ka".repeat(40);
        let mut ph = [Ph::Pause; MAX_PH];
        assert_eq!(parse_word(&long, &mut ph), MAX_PH);
    }

    #[test]
    fn timeline_walks_closure_burst_release_and_holds_the_last_vowel() {
        let mut s = Speech::default();
        s.speak("ta");
        assert_eq!(s.current(), Some(Ph::T));
        let c = s.ctrl(Register::Bass, 1.0);
        assert_eq!(c.vg, 0.0, "voiceless closure is silent");
        assert_eq!(c.constr, Some((36.5, 0.0)));
        s.advance(CLOSE_VL + 0.002);
        let c = s.ctrl(Register::Bass, 1.0);
        assert!(c.ng > 0.5 && c.nfc > 3000.0, "alveolar burst {c:?}");
        s.advance(BURST);
        let c = s.ctrl(Register::Bass, 1.0);
        assert!(c.asp > 0.0 && c.vg == 0.0, "aspiration");
        s.advance(ASP_VL);
        assert_eq!(s.current(), Some(Ph::V(Vowel::A)));
        s.advance(10.0);
        assert_eq!(s.current(), Some(Ph::V(Vowel::A)), "last vowel holds");
        // ending on a consonant goes silent
        s.speak("ak");
        s.advance(VOWEL + 1.0);
        assert!(s.done());
        assert_eq!(s.ctrl(Register::Bass, 1.0).vg, 0.0);
        // nasal opens the velum, voiced
        s.speak("ma");
        let c = s.ctrl(Register::Bass, 1.0);
        assert!(c.velum > 0.3 && c.vg > 0.2);
    }

    fn tract_vowel(v: Vowel) -> Vec<f32> {
        let mut t = Tract::new();
        let (ti, td, lip) = vowel_shape(v);
        let sr = 48000.0;
        let mut out = vec![];
        let mut ph = 0.0f32;
        for n in 0..24000 {
            if n % 16 == 0 {
                t.shape((ti, td), lip, None, 0.01, 16.0 / sr, 1.0);
            }
            // an impulse train: a flat source spectrum
            ph += 110.0 / sr;
            let g = if ph >= 1.0 {
                ph -= 1.0;
                1.0
            } else {
                0.0
            };
            out.push(t.render(g, 0.0, 0.0, sr, 1.0));
        }
        out.split_off(4800)
    }

    /// F1 / F2 from the harmonic envelope (110 Hz source), pre-emphasised
    /// +6 dB/oct to undo the source tilt: the loudest harmonic in 200-1000 Hz
    /// and in 700-3000 Hz (excluding F1's neighbours)
    fn f1_f2(x: &[f32]) -> (f32, f32) {
        let sr = 48000.0;
        let amp = |f: f32| -> f32 {
            let w = 2.0 * std::f32::consts::PI * f / sr;
            let (mut s1, mut s2) = (0.0f32, 0.0f32);
            for &v in x {
                let s = v + 2.0 * w.cos() * s1 - s2;
                s2 = s1;
                s1 = s;
            }
            (s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2).sqrt()
        };
        let env: Vec<(f32, f32)> = (2..28)
            .map(|h| (h as f32 * 110.0, amp(h as f32 * 110.0)))
            .collect();
        let pick = |lo: f32, hi: f32, not: f32| {
            env.iter()
                .filter(|(f, _)| *f >= lo && *f <= hi && (*f - not).abs() > 250.0)
                .fold((0.0, 0.0), |b, &(f, a)| if a > b.1 { (f, a) } else { b })
                .0
        };
        let f1 = pick(200.0, 1000.0, -1e9);
        (f1, pick(700.0, 3000.0, f1))
    }

    #[test]
    fn throat_vowels_have_the_right_formant_pattern() {
        let a = f1_f2(&tract_vowel(Vowel::A));
        let i = f1_f2(&tract_vowel(Vowel::I));
        let u = f1_f2(&tract_vowel(Vowel::U));
        eprintln!("A {a:?} I {i:?} U {u:?}");
        assert!(
            a.0 > i.0 + 150.0 && a.0 > u.0 + 150.0,
            "A has the highest F1"
        );
        assert!(
            i.1 > a.1 + 400.0 && i.1 > u.1 + 600.0,
            "I has the highest F2"
        );
        for v in Vowel::ALL {
            let x = tract_vowel(v);
            assert!(x.iter().all(|s| s.is_finite()));
            let rms = (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt();
            assert!(rms > 0.01 && rms < 5.0, "{v:?} rms {rms}");
        }
    }

    #[test]
    fn tract_closure_silences_and_nose_still_hums() {
        let sr = 48000.0;
        let run = |constr: Option<(f32, f32)>, velum: f32| {
            let mut t = Tract::new();
            let mut ph = 0.0f32;
            let mut e = 0.0;
            for n in 0..24000 {
                if n % 16 == 0 {
                    t.shape((25.0, 2.6), 1.5, constr, velum, 16.0 / sr, 1.0);
                }
                ph = (ph + 110.0 / sr) % 1.0;
                let g = if ph < 0.6 {
                    (std::f32::consts::PI * ph / 0.3).sin()
                } else {
                    0.0
                };
                let y = t.render(g, 0.0, 0.0, sr, 1.0);
                if n > 4800 {
                    e += y * y;
                }
            }
            e
        };
        let open = run(None, 0.01);
        let shut = run(Some((42.5, 0.0)), 0.01);
        let hum = run(Some((42.5, 0.0)), 0.45);
        eprintln!("open {open} shut {shut} hum {hum}");
        assert!(shut < open * 0.05, "lips closed");
        assert!(hum > shut * 5.0, "nasal murmur through the nose");
    }
}
