//! Chord lane and progression generator for the piano roll (docs/CHORDS.md
//! "Chord lane"). Chord markers live on note clips (`NoteClipData::chords`).
//! Progressions come from a table of common, generic progressions per style
//! (roman numerals relative to the key's tonic, borrowed chords written with
//! a flat: bVII, bVI ...) plus seeded variation rules, so "Another one" and
//! the same seed always give the same result.
use crate::chords::{Key, Scale};
use crate::timeline::{Tick, PPQ};
use serde::{Deserialize, Serialize};

const BAR: Tick = 4 * PPQ;

/// chord quality: the intervals above the root
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quality {
    #[default]
    Maj,
    Min,
    Dim,
    Aug,
    Sus2,
    Sus4,
    Dom7,
    Maj7,
    Min7,
    M7b5,
    Dim7,
    Add9,
    Madd9,
    Maj9,
    Min9,
    Dom9,
    Dom13,
    Power,
}

impl Quality {
    /// the chord types the chord pie offers
    pub const MENU: [Quality; 12] = [
        Quality::Maj,
        Quality::Min,
        Quality::Sus2,
        Quality::Sus4,
        Quality::Dom7,
        Quality::Maj7,
        Quality::Min7,
        Quality::Dim,
        Quality::Add9,
        Quality::Min9,
        Quality::Maj9,
        Quality::Power,
    ];
    pub fn iv(self) -> &'static [u8] {
        match self {
            Quality::Maj => &[0, 4, 7],
            Quality::Min => &[0, 3, 7],
            Quality::Dim => &[0, 3, 6],
            Quality::Aug => &[0, 4, 8],
            Quality::Sus2 => &[0, 2, 7],
            Quality::Sus4 => &[0, 5, 7],
            Quality::Dom7 => &[0, 4, 7, 10],
            Quality::Maj7 => &[0, 4, 7, 11],
            Quality::Min7 => &[0, 3, 7, 10],
            Quality::M7b5 => &[0, 3, 6, 10],
            Quality::Dim7 => &[0, 3, 6, 9],
            Quality::Add9 => &[0, 4, 7, 14],
            Quality::Madd9 => &[0, 3, 7, 14],
            Quality::Maj9 => &[0, 4, 7, 11, 14],
            Quality::Min9 => &[0, 3, 7, 10, 14],
            Quality::Dom9 => &[0, 4, 7, 10, 14],
            Quality::Dom13 => &[0, 4, 10, 14, 21],
            Quality::Power => &[0, 7],
        }
    }
    /// chord-symbol suffix ("m", "sus2", "maj7" ...)
    pub fn short(self) -> &'static str {
        match self {
            Quality::Maj => "",
            Quality::Min => "m",
            Quality::Dim => "dim",
            Quality::Aug => "aug",
            Quality::Sus2 => "sus2",
            Quality::Sus4 => "sus4",
            Quality::Dom7 => "7",
            Quality::Maj7 => "maj7",
            Quality::Min7 => "m7",
            Quality::M7b5 => "m7b5",
            Quality::Dim7 => "dim7",
            Quality::Add9 => "add9",
            Quality::Madd9 => "madd9",
            Quality::Maj9 => "maj9",
            Quality::Min9 => "m9",
            Quality::Dom9 => "9",
            Quality::Dom13 => "13",
            Quality::Power => "5",
        }
    }
    /// what the chord pie's centre says
    pub fn long(self) -> &'static str {
        match self {
            Quality::Maj => "Major",
            Quality::Min => "Minor",
            Quality::Dim => "Diminished",
            Quality::Aug => "Augmented",
            Quality::Sus2 => "Sus 2",
            Quality::Sus4 => "Sus 4",
            Quality::Dom7 => "Dominant 7",
            Quality::Maj7 => "Major 7",
            Quality::Min7 => "Minor 7",
            Quality::M7b5 => "Half-dim.",
            Quality::Dim7 => "Dim. 7",
            Quality::Add9 => "Add 9",
            Quality::Madd9 => "Minor add 9",
            Quality::Maj9 => "Major 9",
            Quality::Min9 => "Minor 9",
            Quality::Dom9 => "Dominant 9",
            Quality::Dom13 => "13th",
            Quality::Power => "Power",
        }
    }
    pub fn minor(self) -> bool {
        matches!(
            self,
            Quality::Min
                | Quality::Dim
                | Quality::Min7
                | Quality::M7b5
                | Quality::Dim7
                | Quality::Madd9
                | Quality::Min9
        )
    }
    /// the plain triad this is a colour of
    pub fn base(self) -> Quality {
        match self {
            Quality::Min | Quality::Min7 | Quality::Madd9 | Quality::Min9 => Quality::Min,
            Quality::Dim | Quality::M7b5 | Quality::Dim7 => Quality::Dim,
            Quality::Aug => Quality::Aug,
            _ => Quality::Maj,
        }
    }
}

/// a chord on the chord lane (times relative to the note data)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChordMark {
    pub start: Tick,
    pub len: Tick,
    /// root pitch class (0 = C)
    pub root: u8,
    pub q: Quality,
    /// slash bass pitch class (an inversion), if not the root
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bass: Option<u8>,
}

impl ChordMark {
    pub fn end(&self) -> Tick {
        self.start + self.len
    }
    /// "Gsus2/E"
    pub fn name(&self, key: Key) -> String {
        let mut s = format!("{}{}", key.note_name(self.root), self.q.short());
        if let Some(b) = self.bass.filter(|b| *b != self.root) {
            s.push('/');
            s.push_str(key.note_name(b));
        }
        s
    }
    /// roman numeral in `key` ("bVII", "ii7", "i°")
    pub fn numeral(&self, key: Key) -> String {
        numeral((self.root % 12 + 12 - key.root % 12) % 12, self.q)
    }
    /// the chord's pitch classes (root first)
    pub fn pcs(&self) -> Vec<u8> {
        self.q
            .iv()
            .iter()
            .map(|i| (self.root % 12 + i) % 12)
            .collect()
    }
}

/// roman numeral of a chord `off` semitones above the tonic
pub fn numeral(off: u8, q: Quality) -> String {
    const N: [&str; 12] = [
        "I", "bII", "II", "bIII", "III", "IV", "bV", "V", "bVI", "VI", "bVII", "VII",
    ];
    let mut s = N[(off % 12) as usize].to_string();
    if q.minor() {
        s = s
            .chars()
            .map(|c| if c == 'b' { c } else { c.to_ascii_lowercase() })
            .collect();
    }
    let suf = match q {
        Quality::Maj | Quality::Min => "",
        Quality::Dim => "°",
        Quality::M7b5 => "ø7",
        Quality::Dim7 => "°7",
        Quality::Min7 => "7",
        Quality::Min9 => "9",
        Quality::Madd9 => "add9",
        Quality::Aug => "+",
        q => q.short(),
    };
    s + suf
}

/// parse one roman numeral ("bVII", "ii9", "IVmaj7", "V13", "vii°", "Isus4")
pub fn parse_numeral(t: &str) -> Option<(u8, Quality)> {
    let t = t.trim();
    let (acc, rest) = match t.as_bytes().first()? {
        b'b' => (-1i32, &t[1..]),
        b'#' => (1, &t[1..]),
        _ => (0, t),
    };
    let n = rest
        .chars()
        .take_while(|c| matches!(c, 'I' | 'V' | 'i' | 'v'))
        .count();
    let (rom, suf) = rest.split_at(n);
    let deg = match rom.to_ascii_uppercase().as_str() {
        "I" => 0,
        "II" => 1,
        "III" => 2,
        "IV" => 3,
        "V" => 4,
        "VI" => 5,
        "VII" => 6,
        _ => return None,
    };
    const MAJ: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    let off = (MAJ[deg] + acc).rem_euclid(12) as u8;
    let minor = rom.chars().next()?.is_lowercase();
    let q = match (minor, suf) {
        (false, "") => Quality::Maj,
        (true, "") => Quality::Min,
        (_, "°") | (_, "dim") => Quality::Dim,
        (_, "+") => Quality::Aug,
        (_, "sus2") => Quality::Sus2,
        (_, "sus4") => Quality::Sus4,
        (false, "7") => Quality::Dom7,
        (true, "7") => Quality::Min7,
        (_, "maj7") => Quality::Maj7,
        (_, "ø7") => Quality::M7b5,
        (_, "°7") => Quality::Dim7,
        (false, "add9") => Quality::Add9,
        (true, "add9") => Quality::Madd9,
        (_, "maj9") => Quality::Maj9,
        (false, "9") => Quality::Dom9,
        (true, "9") => Quality::Min9,
        (_, "13") => Quality::Dom13,
        (_, "5") => Quality::Power,
        _ => return None,
    };
    Some((off, q))
}

/// a music style: common progressions and how fast they move
pub struct StyleDef {
    pub name: &'static str,
    pub desc: &'static str,
    /// bars per chord at "Style" speed
    pub bars: f32,
    /// progressions, numerals joined by '-'
    pub progs: &'static [&'static str],
}

/// the styles (generic, centuries-old or genre-standard progressions)
pub const STYLES: &[StyleDef] = &[
    StyleDef {
        name: "Cinematic",
        desc: "Epic minor-key builds, borrowed chords and suspensions (trailer, score)",
        bars: 2.0,
        progs: &[
            "i-bVI-bIII-bVII",
            "i-bIII-bVII-iv",
            "i-iv-bVI-bVII",
            "i-bVI-iv-V",
            "i-bVII-bVI-V",
        ],
    },
    StyleDef {
        name: "Ambient",
        desc: "Slow, floating major 7ths and sus chords",
        bars: 2.0,
        progs: &[
            "Imaj7-IVmaj7",
            "Isus2-vi7-IVmaj7-Isus4",
            "IVmaj7-Imaj7-vi7-Vsus4",
            "i9-bVImaj7",
        ],
    },
    StyleDef {
        name: "Indie Pop",
        desc: "Bright, jangly major-key loops",
        bars: 1.0,
        progs: &["I-iii-IV-vi", "I-V-vi-IV", "IV-I-V-vi", "I-IV-vi-V"],
    },
    StyleDef {
        name: "Emo",
        desc: "Big, emotional power-pop changes",
        bars: 1.0,
        progs: &["vi-IV-I-V", "I-V-vi-IV", "IV-V-vi-I", "vi-V-IV-V"],
    },
    StyleDef {
        name: "Pop",
        desc: "The classic four-chord songs",
        bars: 1.0,
        progs: &["I-V-vi-IV", "vi-IV-I-V", "I-vi-IV-V", "IV-I-V-vi"],
    },
    StyleDef {
        name: "Lo-fi / Neo Soul",
        desc: "Lazy jazzy 7ths and 9ths",
        bars: 1.0,
        progs: &[
            "IVmaj7-iii7-vi7",
            "ii9-V13-Imaj9",
            "Imaj7-vi7-ii7-V7",
            "IVmaj7-V7-iii7-vi7",
        ],
    },
    StyleDef {
        name: "Synthwave",
        desc: "Retro 80s minor drive",
        bars: 1.0,
        progs: &[
            "i-bVI-bIII-bVII",
            "vi-IV-I-V",
            "i-bVII-bVI-bVII",
            "i-iv-bVI-bVII",
        ],
    },
    StyleDef {
        name: "EDM / Trance",
        desc: "Euphoric anthem loops",
        bars: 1.0,
        progs: &[
            "vi-IV-I-V",
            "i-bVI-bIII-bVII",
            "i-bVII-bVI-bVII",
            "i-iv-bVI-bVII",
        ],
    },
    StyleDef {
        name: "Dark / Horror",
        desc: "Chromatic, unstable minor dread",
        bars: 2.0,
        progs: &["i-bVI-iv-V", "i-ii°-bVI-V", "i-bII-bVI-V", "i-bIII-bII-i"],
    },
    StyleDef {
        name: "Folk / Country",
        desc: "Plain major chords and the I-IV-V",
        bars: 1.0,
        progs: &["I-IV-I-V", "I-IV-V-I", "I-vi-IV-V", "I-V-IV-I"],
    },
    StyleDef {
        name: "Blues",
        desc: "The 12-bar and its shuffle cousins",
        bars: 1.0,
        progs: &[
            "I7-I7-I7-I7-IV7-IV7-I7-I7-V7-IV7-I7-V7",
            "I7-IV7-I7-V7",
            "i7-iv7-i7-V7",
        ],
    },
    StyleDef {
        name: "Heroic / Game",
        desc: "Triumphant major with borrowed bVI-bVII",
        bars: 1.0,
        progs: &[
            "I-bVI-bVII-I",
            "I-V-vi-iii-IV-I-IV-V",
            "I-bVII-IV-I",
            "I-IV-bVI-bVII",
        ],
    },
    StyleDef {
        name: "Dark psy",
        desc: "Phrygian tension: the flat II over a droning i",
        bars: 2.0,
        progs: &["i-bII", "i-bII-i-bVII", "i-bII-bIII-bII", "i-bVII-bII-i"],
    },
    StyleDef {
        name: "House",
        desc: "Minor stabs and deep-house 7ths and 9ths",
        bars: 1.0,
        progs: &["i-iv", "ii9-V9", "i7-iv7", "vi9-ii9-V9-Imaj9"],
    },
    StyleDef {
        name: "Jazz",
        desc: "ii-V-I and turnarounds in 7ths",
        bars: 1.0,
        progs: &[
            "ii7-V7-Imaj7",
            "Imaj7-vi7-ii7-V7",
            "iii7-vi7-ii7-V7",
            "iiø7-V7-i7",
        ],
    },
    StyleDef {
        name: "Rock",
        desc: "Mixolydian bVII and power-chord riffs",
        bars: 1.0,
        progs: &["I-bVII-IV", "I-bVII-IV-I", "I-IV-bVII-IV", "I-bIII-IV-I"],
    },
    StyleDef {
        name: "Classical",
        desc: "Andalusian descent and the canon",
        bars: 1.0,
        progs: &[
            "i-bVII-bVI-V",
            "I-V-vi-iii-IV-I-IV-V",
            "I-IV-V-I",
            "i-iv-V-i",
        ],
    },
];

/// style indices the empty-lane pie shows
pub const PIE_STYLES: [usize; 9] = [4, 15, 10, 14, 7, 12, 13, 6, 5];

#[cfg(test)]
pub fn style_index(name: &str) -> Option<usize> {
    STYLES.iter().position(|s| s.name == name)
}

/// a progression's chords (offset above the tonic, quality)
pub fn parse_prog(p: &str) -> Vec<(u8, Quality)> {
    p.split('-').filter_map(parse_numeral).collect()
}

/// minor-key progression (its first chord is minor)
pub fn prog_minor(p: &str) -> bool {
    parse_prog(p).first().is_some_and(|c| c.1.minor())
}

/// a short label for menus ("I–V–vi–IV"; the 12-bar is "12-bar")
pub fn prog_label(p: &str) -> String {
    if p.split('-').count() > 8 {
        return "12-bar blues".into();
    }
    p.replace('-', "–")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Any,
    Major,
    Minor,
}

/// the Generate dialog's settings
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenOpts {
    pub style: usize,
    /// tonic pitch class
    pub root: u8,
    pub mode: Mode,
    /// total length in bars
    pub bars: u32,
    /// -1 slower, 0 style, +1 faster
    pub speed: i8,
    /// 0-based bar the first chord starts on (relative to the clip)
    pub start_bar: u32,
    /// 0..1: 7ths, 9ths and suspensions
    pub colour: f32,
    /// 0..1: chord substitutions
    pub variation: f32,
    /// pick inversions so the bass moves by small steps
    pub walking_bass: bool,
    pub seed: u64,
    /// a fixed progression of the style (pie menu), else picked by seed
    pub prog: Option<usize>,
}

impl Default for GenOpts {
    fn default() -> Self {
        Self {
            style: 0,
            root: 9,
            mode: Mode::Any,
            bars: 8,
            speed: 0,
            start_bar: 0,
            colour: 0.5,
            variation: 0.35,
            walking_bass: true,
            seed: 1,
            prog: None,
        }
    }
}

pub struct GenResult {
    pub key: Key,
    pub prog: usize,
    pub marks: Vec<ChordMark>,
}

/// xorshift: seeded and the same on every platform (shared with
/// `crate::melodygen`)
pub(crate) struct Rng(pub(crate) u64);
impl Rng {
    pub(crate) fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    pub(crate) fn pick(&mut self, n: usize) -> usize {
        ((self.f() * n as f32) as usize).min(n.saturating_sub(1))
    }
}

/// the progressions of `style` that fit `mode` (indices into `progs`)
pub fn progs_for(style: usize, mode: Mode) -> Vec<usize> {
    let st = &STYLES[style.min(STYLES.len() - 1)];
    let all: Vec<usize> = (0..st.progs.len()).collect();
    let want: Vec<usize> = all
        .iter()
        .copied()
        .filter(|&i| match mode {
            Mode::Any => true,
            Mode::Major => !prog_minor(st.progs[i]),
            Mode::Minor => prog_minor(st.progs[i]),
        })
        .collect();
    if want.is_empty() {
        all
    } else {
        want
    }
}

/// build a progression
pub fn generate(o: &GenOpts) -> GenResult {
    let style = o.style.min(STYLES.len() - 1);
    let st = &STYLES[style];
    let mut rng = Rng(o.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let pi = match o.prog {
        Some(p) => p.min(st.progs.len() - 1),
        None => {
            let c = progs_for(style, o.mode);
            c[rng.pick(c.len())]
        }
    };
    let src = st.progs[pi];
    let minor = prog_minor(src);
    let key = Key::new(o.root % 12, if minor { Scale::Minor } else { Scale::Major });
    let mut chords = parse_prog(src);
    // variation: substitutions (never the first chord, the home)
    for c in chords.iter_mut().skip(1) {
        if rng.f() < o.variation * 0.5 {
            *c = match c.1.base() {
                Quality::Maj if rng.f() < 0.5 => ((c.0 + 9) % 12, Quality::Min),
                Quality::Min if rng.f() < 0.5 => ((c.0 + 3) % 12, Quality::Maj),
                Quality::Maj => (
                    c.0,
                    if rng.f() < 0.5 {
                        Quality::Sus4
                    } else {
                        Quality::Sus2
                    },
                ),
                Quality::Min => (c.0, Quality::Min7),
                _ => *c,
            };
        }
    }
    // colour: extensions and suspensions on plain triads
    for c in chords.iter_mut() {
        if !matches!(c.1, Quality::Maj | Quality::Min) || rng.f() >= o.colour * 0.8 {
            continue;
        }
        let rich = rng.f() < o.colour * 0.6;
        c.1 = match c.1 {
            Quality::Maj => match rng.pick(4) {
                0 | 1 => {
                    if rich {
                        Quality::Maj9
                    } else {
                        Quality::Maj7
                    }
                }
                2 => Quality::Sus2,
                _ => {
                    if rich {
                        Quality::Add9
                    } else {
                        Quality::Sus4
                    }
                }
            },
            _ => {
                if rich {
                    Quality::Min9
                } else if rng.f() < 0.5 {
                    Quality::Min7
                } else {
                    Quality::Madd9
                }
            }
        };
    }
    let mul = match o.speed {
        s if s < 0 => 2.0,
        s if s > 0 => 0.5,
        _ => 1.0,
    };
    let per = ((st.bars * mul).max(0.25) * BAR as f32) as Tick;
    let total = o.bars.max(1) as Tick * BAR;
    let base = o.start_bar as Tick * BAR;
    let mut marks = vec![];
    let mut t = 0;
    let mut k = 0;
    let mut prev_bass: Option<u8> = None;
    while t < total && !chords.is_empty() {
        let (off, q) = chords[k % chords.len()];
        let root = (key.root + off) % 12;
        let mut m = ChordMark {
            start: base + t,
            len: per.min(total - t),
            root,
            q,
            bass: None,
        };
        if o.walking_bass {
            if let Some(pb) = prev_bass {
                // the chord tone nearest the last bass note
                let pcs = m.pcs();
                let best = pcs
                    .iter()
                    .take(3)
                    .copied()
                    .min_by_key(|&p| {
                        let d = (p as i32 - pb as i32).rem_euclid(12);
                        d.min(12 - d) * 2 + (p != root) as i32
                    })
                    .unwrap_or(root);
                m.bass = (best != root).then_some(best);
            }
            prev_bass = Some(m.bass.unwrap_or(root));
        }
        marks.push(m);
        t += per;
        k += 1;
    }
    GenResult {
        key,
        prog: pi,
        marks,
    }
}

/// how chord markers become notes
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Voicing {
    #[default]
    Full,
    Triads,
    Shell,
    Bass,
    Pad,
    Light,
    Open,
    Power,
}

impl Voicing {
    pub const ALL: [Voicing; 8] = [
        Voicing::Full,
        Voicing::Triads,
        Voicing::Shell,
        Voicing::Bass,
        Voicing::Pad,
        Voicing::Light,
        Voicing::Open,
        Voicing::Power,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Voicing::Full => "Full",
            Voicing::Triads => "Triads",
            Voicing::Shell => "Shell",
            Voicing::Bass => "Bass only",
            Voicing::Pad => "Pad",
            Voicing::Light => "Light",
            Voicing::Open => "Open",
            Voicing::Power => "Power",
        }
    }
}

/// tracker note of pitch class `pc` in `octave` (C-4 = octave 4)
fn note(pc: u8, octave: i32) -> i32 {
    octave * 12 + 1 + pc as i32
}

/// the notes for `marks`: (start, len, tracker key). Upper voices move to
/// the nearest inversion of each next chord (voice leading).
pub fn voice(marks: &[ChordMark], v: Voicing, octave: u8) -> Vec<(Tick, Tick, u8)> {
    let oct = octave as i32;
    let mut out = vec![];
    let mut prev: Vec<i32> = vec![];
    for m in marks {
        let iv: Vec<i32> = m.q.iv().iter().map(|&i| i as i32).collect();
        let r = note(m.root, oct);
        let third = iv.get(1).copied().unwrap_or(7);
        let seventh = iv.get(3).copied().filter(|x| *x < 12).unwrap_or(7);
        let bass = note(m.bass.unwrap_or(m.root), oct - 1);
        let mut keys: Vec<i32> = match v {
            Voicing::Bass => vec![bass],
            Voicing::Shell => vec![note(m.root, oct - 1), r + third, r + seventh],
            Voicing::Power => vec![r, r + 7, r + 12],
            Voicing::Open => {
                let mut k = vec![r, r + 7, r + third + 12];
                if iv.len() > 3 {
                    k.push(r + iv[3] + 12);
                }
                k
            }
            Voicing::Full | Voicing::Triads | Voicing::Pad | Voicing::Light => {
                let mut tones: Vec<i32> = match v {
                    Voicing::Triads => iv.iter().take(3).map(|i| r + i).collect(),
                    Voicing::Light => iv.iter().skip(1).map(|i| r + i).collect(),
                    _ => iv.iter().map(|i| r + i).collect(),
                };
                if !prev.is_empty() {
                    // try each inversion an octave either way: least motion wins
                    let n = tones.len();
                    let mut best = (i32::MAX, tones.clone());
                    for inv in 0..n {
                        for sh in [-12, 0, 12] {
                            let mut c: Vec<i32> = tones
                                .iter()
                                .enumerate()
                                .map(|(i, t)| t + sh + if i < inv { 12 } else { 0 })
                                .collect();
                            c.sort();
                            let cost: i32 = c
                                .iter()
                                .map(|x| prev.iter().map(|p| (x - p).abs()).min().unwrap_or(0))
                                .sum::<i32>()
                                + (c[0] - note(0, oct)).abs() / 6;
                            if cost < best.0 {
                                best = (cost, c);
                            }
                        }
                    }
                    tones = best.1;
                }
                prev = tones.clone();
                if v == Voicing::Pad {
                    tones.insert(0, note(m.root, oct - 1));
                }
                tones
            }
        };
        if m.bass.is_some() && !matches!(v, Voicing::Bass | Voicing::Pad | Voicing::Shell) {
            keys.insert(0, bass);
        }
        keys.sort();
        keys.dedup();
        for k in keys {
            out.push((m.start, m.len, k.clamp(1, 120) as u8));
        }
    }
    out
}

/// chords that could replace `m` in `key`, grouped by idea: swaps
/// (relative / parallel), colour, borrowed and tension
pub fn suggestions(key: Key, m: &ChordMark) -> Vec<(&'static str, ChordMark)> {
    let mk = |root: u8, q: Quality| ChordMark {
        root: root % 12,
        q,
        bass: None,
        ..m.clone()
    };
    let r = m.root % 12;
    let t = key.root % 12;
    let mut v = vec![];
    if m.q.minor() {
        v.push(("relative major", mk(r + 3, Quality::Maj)));
        v.push(("parallel major", mk(r, Quality::Maj)));
        v.push(("colour", mk(r, Quality::Min7)));
        v.push(("colour", mk(r, Quality::Min9)));
    } else {
        v.push(("relative minor", mk(r + 9, Quality::Min)));
        v.push(("parallel minor", mk(r, Quality::Min)));
        v.push(("colour", mk(r, Quality::Maj7)));
        v.push(("colour", mk(r, Quality::Sus2)));
    }
    v.push(("borrowed bVI", mk(t + 8, Quality::Maj)));
    v.push(("borrowed bVII", mk(t + 10, Quality::Maj)));
    v.push(("tension V7", mk(t + 7, Quality::Dom7)));
    v.push(("its dominant", mk(r + 7, Quality::Dom7)));
    v.retain(|(_, c)| !(c.root == m.root && c.q == m.q));
    v
}

/// chords from notes: per `step` ticks, the chord whose tones cover the
/// most sounding time (Maj, Min, Dim, Sus4, 7ths); repeats merge
pub fn detect(notes: &[crate::timeline::Note], len: Tick, step: Tick) -> Vec<ChordMark> {
    let step = step.max(PPQ);
    let cands = [
        Quality::Maj,
        Quality::Min,
        Quality::Dim,
        Quality::Sus4,
        Quality::Dom7,
        Quality::Maj7,
        Quality::Min7,
    ];
    let mut out: Vec<ChordMark> = vec![];
    let mut t = 0;
    while t < len {
        let mut h = [0f32; 12];
        let mut low: Option<(u8, u8)> = None;
        for n in notes {
            let ov = (n.start + n.len).min(t + step) - n.start.max(t);
            if ov > 0 && n.key > 0 {
                let pc = (n.key - 1) % 12;
                h[pc as usize] += ov as f32;
                if low.is_none_or(|(k, _)| n.key < k) {
                    low = Some((n.key, pc));
                }
            }
        }
        let total: f32 = h.iter().sum();
        if total > 0.0 {
            let mut best = (f32::MIN, 0u8, Quality::Maj);
            for root in 0..12u8 {
                for q in cands {
                    let pcs: Vec<u8> = q.iv().iter().map(|i| (root + i) % 12).collect();
                    let hit: f32 = pcs.iter().map(|p| h[*p as usize]).sum();
                    let miss = total - hit;
                    let mut score = hit - 1.2 * miss - 0.15 * total * (pcs.len() as f32 - 3.0);
                    if h[root as usize] > 0.0 {
                        score += 0.1 * total;
                    }
                    if score > best.0 {
                        best = (score, root, q);
                    }
                }
            }
            let bass = low.map(|l| l.1).filter(|b| *b != best.1);
            match out.last_mut() {
                Some(l) if l.end() == t && l.root == best.1 && l.q == best.2 && l.bass == bass => {
                    l.len += step
                }
                _ => out.push(ChordMark {
                    start: t,
                    len: step.min(len - t),
                    root: best.1,
                    q: best.2,
                    bass,
                }),
            }
        }
        t += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numerals_round_trip() {
        for t in [
            "I", "bVII", "ii7", "IVmaj7", "V13", "ii9", "i", "bII", "iiø7", "vii°", "Isus4",
        ] {
            let (o, q) = parse_numeral(t).unwrap_or_else(|| panic!("{t}"));
            assert_eq!(numeral(o, q), t, "{t}");
        }
        for s in STYLES {
            for p in s.progs {
                assert_eq!(parse_prog(p).len(), p.split('-').count(), "{} {p}", s.name);
            }
        }
    }

    #[test]
    fn generate_is_seeded_and_fills_the_length() {
        let o = GenOpts {
            style: style_index("Cinematic").unwrap(),
            root: 4,
            mode: Mode::Minor,
            colour: 0.0,
            variation: 0.0,
            walking_bass: false,
            prog: Some(1),
            ..Default::default()
        };
        let r = generate(&o);
        let key = r.key;
        let names: Vec<String> = r.marks.iter().map(|m| m.name(key)).collect();
        assert_eq!(
            names,
            ["Em", "G", "D", "Am"],
            "E minor i-bIII-bVII-iv, 2 bars each"
        );
        assert_eq!(r.marks.iter().map(|m| m.len).sum::<Tick>(), 8 * BAR);
        let num: Vec<String> = r.marks.iter().map(|m| m.numeral(key)).collect();
        assert_eq!(num, ["i", "bIII", "bVII", "iv"]);
        let a = generate(&GenOpts {
            seed: 7,
            colour: 0.8,
            variation: 0.8,
            ..o
        });
        let b = generate(&GenOpts {
            seed: 7,
            colour: 0.8,
            variation: 0.8,
            ..o
        });
        assert_eq!(a.marks, b.marks);
        // faster: twice the chords
        assert_eq!(generate(&GenOpts { speed: 1, ..o }).marks.len(), 8);
        // walking bass gives slash chords that move by small steps
        let w = generate(&GenOpts {
            walking_bass: true,
            ..o
        });
        assert!(w.marks.iter().any(|m| m.bass.is_some()));
        assert!(w.marks.iter().any(|m| m.name(key).contains('/')));
    }

    #[test]
    fn voicings_and_detect() {
        let o = GenOpts {
            style: style_index("Pop").unwrap(),
            root: 0,
            prog: Some(0),
            colour: 0.0,
            variation: 0.0,
            walking_bass: false,
            bars: 4,
            ..Default::default()
        };
        let r = generate(&o);
        let n = voice(&r.marks, Voicing::Triads, 4);
        assert_eq!(n.len(), 12);
        // voice leading: no upper voice jumps more than 5 semitones
        let at = |s: Tick| {
            n.iter()
                .filter(|x| x.0 == s)
                .map(|x| x.2 as i32)
                .collect::<Vec<_>>()
        };
        for w in r.marks.windows(2) {
            let (a, b) = (at(w[0].start), at(w[1].start));
            for x in &b {
                assert!(
                    a.iter().map(|y| (x - y).abs()).min().unwrap() <= 5,
                    "{a:?} -> {b:?}"
                );
            }
        }
        // and the notes detect back as the same chords
        let notes: Vec<crate::timeline::Note> = n
            .iter()
            .map(|&(start, len, key)| crate::timeline::Note {
                start,
                len,
                key,
                vel: 50000,
                ..Default::default()
            })
            .collect();
        let d = detect(&notes, 4 * BAR, BAR);
        let key = r.key;
        let names: Vec<String> = d
            .iter()
            .map(|m| {
                let mut m = m.clone();
                m.bass = None;
                m.name(key)
            })
            .collect();
        assert_eq!(names, ["C", "G", "Am", "F"]);
        assert!(!suggestions(key, &r.marks[0]).is_empty());
    }
}
