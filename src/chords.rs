//! Chord helper theory: scales, key detection, diatonic chords, progressions
//! and writing them into a block as chords, arpeggios or a bass line.
//!
//! Key detection is the Krumhansl–Schmuckler key-finding algorithm with the
//! Krumhansl & Kessler (1982) probe-tone profiles (C. L. Krumhansl,
//! "Cognitive Foundations of Musical Pitch", Oxford University Press, 1990),
//! blended with how much of the song's notes fall inside each scale. The
//! workflow (key, chord pads, progressions, scale lock) is inspired by
//! Scaler by Plugin Boutique; no code or data from it is used.
use serde::{Deserialize, Serialize};

use crate::model::{Pattern, Song, NOTE_OFF};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scale {
    Major,
    Minor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    PhrygianDominant,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
}

impl Scale {
    pub const ALL: [Scale; 13] = [
        Scale::Major,
        Scale::Minor,
        Scale::HarmonicMinor,
        Scale::MelodicMinor,
        Scale::Dorian,
        Scale::Phrygian,
        Scale::Lydian,
        Scale::Mixolydian,
        Scale::Locrian,
        Scale::PhrygianDominant,
        Scale::MajorPentatonic,
        Scale::MinorPentatonic,
        Scale::Blues,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Scale::Major => "major",
            Scale::Minor => "minor",
            Scale::HarmonicMinor => "harmonic minor",
            Scale::MelodicMinor => "melodic minor",
            Scale::Dorian => "dorian",
            Scale::Phrygian => "phrygian",
            Scale::Lydian => "lydian",
            Scale::Mixolydian => "mixolydian",
            Scale::Locrian => "locrian",
            Scale::PhrygianDominant => "phrygian dominant",
            Scale::MajorPentatonic => "major pentatonic",
            Scale::MinorPentatonic => "minor pentatonic",
            Scale::Blues => "blues",
        }
    }
    /// semitones above the root
    pub fn steps(self) -> &'static [u8] {
        match self {
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Scale::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            Scale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Scale::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Scale::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Scale::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Scale::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Scale::PhrygianDominant => &[0, 1, 4, 5, 7, 8, 10],
            Scale::MajorPentatonic => &[0, 2, 4, 7, 9],
            Scale::MinorPentatonic => &[0, 3, 5, 7, 10],
            Scale::Blues => &[0, 3, 5, 6, 7, 10],
        }
    }
    /// does the scale sound major (major third above the root)?
    fn majorish(self) -> bool {
        self.steps().contains(&4)
    }
}

/// a key: root pitch class (0 = C) and scale
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    pub root: u8,
    pub scale: Scale,
}

const SHARP: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const FLAT: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

impl Key {
    pub fn new(root: u8, scale: Scale) -> Self {
        Self {
            root: root % 12,
            scale,
        }
    }
    /// flats for flat keys (F, Bb, Eb ... major; D, G, C ... minor)
    fn flats(self) -> bool {
        let major_root = if self.scale.majorish() {
            self.root
        } else {
            (self.root + 3) % 12
        };
        [5, 10, 3, 8, 1, 6].contains(&major_root)
    }
    pub fn note_name(self, pc: u8) -> &'static str {
        if self.flats() {
            FLAT[pc as usize % 12]
        } else {
            SHARP[pc as usize % 12]
        }
    }
    pub fn name(self) -> String {
        format!("{} {}", self.note_name(self.root), self.scale.name())
    }
    /// 12-bit mask of the pitch classes in the key (bit 0 = C)
    pub fn mask(self) -> u16 {
        self.scale
            .steps()
            .iter()
            .fold(0, |m, s| m | 1 << ((self.root + s) % 12))
    }
    pub fn contains(self, pc: u8) -> bool {
        self.mask() & (1 << (pc % 12)) != 0
    }
    /// nearest tracker note in the key (ties go down); note-off / empty stay
    pub fn snap(self, note: u8) -> u8 {
        snap_mask(note, self.mask())
    }
}

/// "A minor", "F# phrygian dominant", "Bb major" (case-insensitive)
pub fn parse_key(s: &str) -> Option<Key> {
    let s = s.trim();
    let (root, rest) = s.split_once(' ')?;
    let r = root.to_lowercase();
    let pc = (0..12u8)
        .find(|&p| SHARP[p as usize].to_lowercase() == r || FLAT[p as usize].to_lowercase() == r)?;
    let rest = rest.trim().to_lowercase();
    let scale = Scale::ALL.into_iter().find(|x| x.name() == rest)?;
    Some(Key::new(pc, scale))
}

/// tracker note (1 = C-0) to pitch class
pub fn pc_of(note: u8) -> u8 {
    note.saturating_sub(1) % 12
}

fn snap_mask(note: u8, mask: u16) -> u8 {
    if note == 0 || note == NOTE_OFF || mask == 0 {
        return note;
    }
    for d in 0..12i32 {
        for n in [note as i32 - d, note as i32 + d] {
            if (1..=120).contains(&n) && mask & (1 << pc_of(n as u8)) != 0 {
                return n as u8;
            }
        }
    }
    note
}

/// Scale lock for threads that only see an atomic: 0 = off, else
/// bit 16 set and the 12-bit pitch-class mask.
pub fn pack(lock: Option<Key>) -> u32 {
    lock.map_or(0, |k| 1 << 16 | k.mask() as u32)
}
pub fn snap_packed(note: u8, packed: u32) -> u8 {
    if packed & 1 << 16 == 0 {
        note
    } else {
        snap_mask(note, (packed & 0xFFF) as u16)
    }
}

// ------------------------------------------------------------- detection

/// Krumhansl & Kessler (1982) probe-tone ratings, tonic first
const KK_MAJOR: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KK_MINOR: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// Pitch-class weights of the song's melodic notes: each note counts for
/// the rows it lasts (until the next note or note-off on its channel, at
/// most a bar), times how often its block is in the order list. Drum
/// channels are skipped.
pub fn histogram(song: &Song) -> [f32; 12] {
    let drums = crate::arrange::drum_mask(song);
    let mut h = [0.0f32; 12];
    let bar = (song.lines_per_beat.max(1) * 4) as usize;
    for (bi, p) in song.patterns.iter().enumerate() {
        let plays = song.order.iter().filter(|&&o| o == bi).count().max(1) as f32;
        for ch in 0..song.channels.min(p.cells.first().map_or(0, |r| r.len())) {
            if ch < 32 && drums & (1 << ch) != 0 {
                continue;
            }
            let mut last: Option<(u8, usize)> = None;
            let mut flush = |last: &mut Option<(u8, usize)>, r: usize| {
                if let Some((n, r0)) = last.take() {
                    h[pc_of(n) as usize] += (r - r0).clamp(1, bar) as f32 * plays;
                }
            };
            for r in 0..p.rows.min(p.cells.len()) {
                let c = p.cells[r][ch];
                if c.note == NOTE_OFF {
                    flush(&mut last, r);
                } else if c.note > 0 {
                    flush(&mut last, r);
                    last = Some((c.note, r));
                }
            }
            flush(&mut last, p.rows);
        }
    }
    h
}

fn pearson(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let ma = a.iter().sum::<f32>() / 12.0;
    let mb = b.iter().sum::<f32>() / 12.0;
    let (mut n, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        n += (a[i] - ma) * (b[i] - mb);
        da += (a[i] - ma).powi(2);
        db += (b[i] - mb).powi(2);
    }
    if da <= 0.0 || db <= 0.0 {
        0.0
    } else {
        n / (da * db).sqrt()
    }
}

/// Krumhansl–Schmuckler correlation of `h` with `key` (major profile for
/// major-sounding scales, minor otherwise)
pub fn ks_score(h: &[f32; 12], key: Key) -> f32 {
    let prof = if key.scale.majorish() {
        &KK_MAJOR
    } else {
        &KK_MINOR
    };
    let mut rot = [0.0; 12];
    for (i, r) in rot.iter_mut().enumerate() {
        *r = prof[(i + 12 - key.root as usize) % 12];
    }
    pearson(h, &rot)
}

/// share of the note weight inside the key (0..1)
pub fn fit(h: &[f32; 12], key: Key) -> f32 {
    let tot: f32 = h.iter().sum();
    if tot <= 0.0 {
        return 0.0;
    }
    (0..12u8)
        .filter(|&p| key.contains(p))
        .map(|p| h[p as usize])
        .sum::<f32>()
        / tot
}

/// Keys ranked best first: (key, score 0..1). Empty when the song has no
/// melodic notes. Major / minor rank by K-S correlation and fit; the other
/// scales must fit at least as well as the best major / minor key to show.
pub fn detect(song: &Song) -> Vec<(Key, f32)> {
    detect_hist(&histogram(song))
}

pub fn detect_hist(h: &[f32; 12]) -> Vec<(Key, f32)> {
    if h.iter().sum::<f32>() <= 0.0 {
        return vec![];
    }
    let mut v: Vec<(Key, f32)> = vec![];
    for root in 0..12 {
        for scale in Scale::ALL {
            let k = Key::new(root, scale);
            let r = ks_score(h, k).max(0.0);
            let f = fit(h, k);
            // fewer notes in the scale = easier to "fit": a pentatonic only
            // wins when the song really avoids the other notes
            let size_pen = (7.0 - scale.steps().len() as f32).max(0.0) * 0.04;
            // major / minor are the reference; modes need a better fit
            let mode_pen = match scale {
                Scale::Major | Scale::Minor => 0.0,
                _ => 0.03,
            };
            v.push((k, 0.5 * r + 0.5 * f - size_pen - mode_pen));
        }
    }
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

// ---------------------------------------------------------------- chords

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChordKind {
    Triad,
    Seventh,
    Sus2,
    Sus4,
    Add9,
    Power,
}

impl ChordKind {
    pub const ALL: [ChordKind; 6] = [
        ChordKind::Triad,
        ChordKind::Seventh,
        ChordKind::Sus2,
        ChordKind::Sus4,
        ChordKind::Add9,
        ChordKind::Power,
    ];
    pub fn name(self) -> &'static str {
        match self {
            ChordKind::Triad => "triad",
            ChordKind::Seventh => "7th",
            ChordKind::Sus2 => "sus2",
            ChordKind::Sus4 => "sus4",
            ChordKind::Add9 => "add9",
            ChordKind::Power => "power",
        }
    }
}

/// a chord: root pitch class and intervals above it (ascending, 0 first)
#[derive(Clone, Debug, PartialEq)]
pub struct Chord {
    pub root: u8,
    pub iv: Vec<u8>,
}

impl Chord {
    /// quality suffix ("m", "dim", "maj7", "sus4" ...)
    pub fn suffix(&self) -> &'static str {
        match self.iv.as_slice() {
            [0, 4, 7] => "",
            [0, 3, 7] => "m",
            [0, 3, 6] => "dim",
            [0, 4, 8] => "aug",
            [0, 4, 7, 11] => "maj7",
            [0, 3, 7, 10] => "m7",
            [0, 4, 7, 10] => "7",
            [0, 3, 6, 10] => "m7b5",
            [0, 3, 6, 9] => "dim7",
            [0, 3, 7, 11] => "mMaj7",
            [0, 4, 8, 11] => "+maj7",
            [0, 2, 7] => "sus2",
            [0, 5, 7] => "sus4",
            [0, 4, 7, 14] => "add9",
            [0, 3, 7, 14] => "madd9",
            [0, 7] => "5",
            _ => "?",
        }
    }
    pub fn name(&self, key: Key) -> String {
        format!("{}{}", key.note_name(self.root), self.suffix())
    }
    /// tracker notes from `octave` (C-octave = root octave), `inversion`
    /// lifts the lowest notes by an octave
    pub fn notes(&self, octave: u8, inversion: usize) -> Vec<u8> {
        let base = octave as i32 * 12 + 1 + self.root as i32;
        let mut n: Vec<i32> = self.iv.iter().map(|&i| base + i as i32).collect();
        for k in 0..inversion.min(n.len().saturating_sub(1)) {
            n[k] += 12;
        }
        n.sort();
        n.into_iter().map(|x| x.clamp(1, 120) as u8).collect()
    }
}

/// The chord on scale degree `deg` (0-based), stacking every other scale
/// note (thirds in a seven-note scale).
pub fn diatonic(key: Key, deg: usize, kind: ChordKind) -> Chord {
    let st = key.scale.steps();
    let n = st.len();
    let tone = |k: usize| -> u8 {
        let i = deg + k;
        st[i % n] + 12 * (i / n) as u8
    };
    let r = tone(0);
    let rel = |k: usize| tone(k) - r;
    let iv = match kind {
        ChordKind::Triad => vec![0, rel(2), rel(4)],
        ChordKind::Seventh => vec![0, rel(2), rel(4), rel(6)],
        // sus chords replace the third with the scale's 2nd / 4th
        ChordKind::Sus2 => vec![0, rel(1), rel(4)],
        ChordKind::Sus4 => vec![0, rel(3), rel(4)],
        ChordKind::Add9 => vec![0, rel(2), rel(4), rel(1) + 12],
        ChordKind::Power => vec![0, rel(4)],
    };
    Chord {
        root: (key.root + r) % 12,
        iv,
    }
}

/// roman numeral of degree `deg` for this chord (upper case = major)
pub fn roman(key: Key, deg: usize, chord: &Chord) -> String {
    const R: [&str; 8] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII"];
    let base = R[deg.min(7)];
    let third = chord.iv.get(1).copied().unwrap_or(4);
    let minor = third == 3;
    let mut s = if minor {
        base.to_lowercase()
    } else {
        base.to_string()
    };
    // a flat degree (bII in phrygian, bVI / bVII in minor)
    let st = key.scale.steps();
    let major_steps = [0u8, 2, 4, 5, 7, 9, 11];
    if st.len() == 7 && st[deg % 7] < major_steps[deg % 7] {
        s = format!("b{s}");
    }
    match chord.suffix() {
        "dim" => s + "°",
        "aug" => s + "+",
        "m" | "" => s,
        "m7" => s + "7",
        suf => s + suf,
    }
}

/// One chord of a progression.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub deg: usize,
    pub kind: ChordKind,
    pub inversion: usize,
    /// length in beats
    pub beats: f32,
}

impl Step {
    pub fn new(deg: usize, kind: ChordKind) -> Self {
        Self {
            deg,
            kind,
            inversion: 0,
            beats: 4.0,
        }
    }
}

/// Preset progressions: (name, degrees, hint). Degrees index the current
/// scale, so "i-VI-III-VII" in A minor is Am F C G.
pub const PRESETS: &[(&str, &[usize], &str)] = &[
    (
        "Uplifting i–VI–III–VII",
        &[0, 5, 2, 6],
        "trance anthem (Am F C G)",
    ),
    ("Pop I–V–vi–IV", &[0, 4, 5, 3], "the four chords (C G Am F)"),
    ("Sad vi–IV–I–V", &[5, 3, 0, 4], "in a major key"),
    ("Epic i–VII–VI–VII", &[0, 6, 5, 6], "minor, rising"),
    (
        "Andalusian i–VII–VI–V",
        &[0, 6, 5, 4],
        "flamenco descent (harmonic minor for a major V)",
    ),
    ("Dark i–iv–i–v", &[0, 3, 0, 4], "minor, brooding"),
    (
        "Psy i–II–i–VII",
        &[0, 1, 0, 6],
        "use phrygian: the flat II is the tension",
    ),
    ("Jazz ii–V–I–I", &[1, 4, 0, 0], "use 7th chords"),
    ("50s I–vi–IV–V", &[0, 5, 3, 4], "doo-wop"),
    ("Royal road IV–V–iii–vi", &[3, 4, 2, 5], "J-pop"),
];

// ------------------------------------------------------------ writing

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArpPattern {
    Up,
    Down,
    UpDown,
    Random,
}

impl ArpPattern {
    pub const ALL: [ArpPattern; 4] = [
        ArpPattern::Up,
        ArpPattern::Down,
        ArpPattern::UpDown,
        ArpPattern::Random,
    ];
    pub fn name(self) -> &'static str {
        match self {
            ArpPattern::Up => "up",
            ArpPattern::Down => "down",
            ArpPattern::UpDown => "up-down",
            ArpPattern::Random => "random",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BassRhythm {
    /// one note per chord
    Whole,
    /// every 8th note
    Eighths,
    /// the "and" between beats (trance offbeat)
    Offbeat,
    /// three 16ths after each beat (psytrance rolling bass)
    Rolling,
    /// root / octave alternating 8ths
    Octaves,
}

impl BassRhythm {
    pub const ALL: [BassRhythm; 5] = [
        BassRhythm::Whole,
        BassRhythm::Eighths,
        BassRhythm::Offbeat,
        BassRhythm::Rolling,
        BassRhythm::Octaves,
    ];
    pub fn name(self) -> &'static str {
        match self {
            BassRhythm::Whole => "whole",
            BassRhythm::Eighths => "8ths",
            BassRhythm::Offbeat => "offbeat",
            BassRhythm::Rolling => "rolling 16ths",
            BassRhythm::Octaves => "octaves",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Style {
    /// stacked over consecutive channels, held for the chord's length
    Chords,
    /// one channel; `rate` = rows per arp note, `octaves` = range
    Arp {
        pattern: ArpPattern,
        rate: usize,
        octaves: u8,
    },
    /// one channel, chord roots
    Bass(BassRhythm),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WriteOpts {
    pub style: Style,
    /// first channel (0-based)
    pub channel: usize,
    pub instr: u8,
    /// octave of the chord roots (tracker octave, 4 = C-4)
    pub octave: u8,
    /// rows per beat (song lines_per_beat)
    pub rows_per_beat: usize,
    pub start_row: usize,
    /// repeat the progression until the block ends
    pub fill: bool,
    /// volume column (0 = none, else 1..=65 like Cell::vol)
    pub vol: u8,
    pub seed: u64,
}

/// What `write` did.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub notes: usize,
    pub chords: usize,
    /// channels used (first..first+n)
    pub channels: usize,
    /// chord tones dropped for lack of channels
    pub dropped: usize,
}

/// Write `prog` into `p`. The target channels are cleared from `start_row`
/// to the end of the progression (or the block, with `fill`); effect
/// columns are kept. Notes end with a note-off where the next chord or the
/// written range ends.
pub fn write(p: &mut Pattern, key: Key, prog: &[Step], o: &WriteOpts) -> Result<Written, String> {
    let nch = p.cells.first().map_or(0, |r| r.len());
    if prog.is_empty() {
        return Err("the progression is empty".into());
    }
    if o.channel >= nch {
        return Err("no such channel".into());
    }
    if o.start_row >= p.rows {
        return Err("start row is past the end of the block".into());
    }
    let rpb = o.rows_per_beat.max(1);
    let lens: Vec<usize> = prog
        .iter()
        .map(|s| ((s.beats * rpb as f32).round() as usize).max(1))
        .collect();
    // chord spans (start, end, step)
    let mut spans = vec![];
    let mut r = o.start_row;
    'outer: loop {
        for (i, s) in prog.iter().enumerate() {
            if r >= p.rows {
                break 'outer;
            }
            let e = (r + lens[i]).min(p.rows);
            spans.push((r, e, *s));
            r = e;
        }
        if !o.fill {
            break;
        }
    }
    let end = r.min(p.rows);
    let chords: Vec<Chord> = spans
        .iter()
        .map(|(_, _, s)| diatonic(key, s.deg, s.kind))
        .collect();
    let width = match o.style {
        Style::Chords => chords.iter().map(|c| c.iv.len()).max().unwrap_or(1),
        _ => 1,
    };
    let used = width.min(nch - o.channel);
    let dropped: usize = match o.style {
        Style::Chords => chords.iter().map(|c| c.iv.len().saturating_sub(used)).sum(),
        _ => 0,
    };
    for row in &mut p.cells[o.start_row..end] {
        for c in &mut row[o.channel..o.channel + used] {
            c.note = 0;
            c.instr = 0;
            c.vol = 0;
        }
    }
    let mut notes = 0;
    let mut put = |p: &mut Pattern, r: usize, ch: usize, note: u8| {
        let c = &mut p.cells[r][ch];
        c.note = note;
        c.instr = if note == NOTE_OFF { 0 } else { o.instr };
        c.vol = if note == NOTE_OFF { 0 } else { o.vol };
        if note != NOTE_OFF {
            notes += 1;
        }
    };
    let mut rng = crate::arrange::Rng::new(o.seed);
    for ((s0, e0, st), ch) in spans.iter().zip(&chords) {
        let tones = ch.notes(o.octave, st.inversion);
        match o.style {
            Style::Chords => {
                for (k, &n) in tones.iter().take(used).enumerate() {
                    put(p, *s0, o.channel + k, n);
                }
            }
            Style::Arp {
                pattern,
                rate,
                octaves,
            } => {
                let mut seq: Vec<u8> = vec![];
                for oc in 0..octaves.max(1) {
                    seq.extend(tones.iter().map(|&n| (n + 12 * oc).min(120)));
                }
                let order: Vec<u8> = match pattern {
                    ArpPattern::Up => seq.clone(),
                    ArpPattern::Down => seq.iter().rev().copied().collect(),
                    ArpPattern::UpDown => {
                        let mut v = seq.clone();
                        v.extend(seq.iter().rev().skip(1).take(seq.len().saturating_sub(2)));
                        v
                    }
                    ArpPattern::Random => seq.clone(),
                };
                let mut k = 0;
                let mut r = *s0;
                while r < *e0 {
                    let n = if pattern == ArpPattern::Random {
                        order[rng.below(order.len())]
                    } else {
                        order[k % order.len()]
                    };
                    put(p, r, o.channel, n);
                    k += 1;
                    r += rate.max(1);
                }
            }
            Style::Bass(rh) => {
                let root = tones[0].saturating_sub(24).max(1);
                let beat = rpb;
                let eighth = (rpb / 2).max(1);
                let sixteenth = (rpb / 4).max(1);
                let rows: Vec<(usize, u8)> = match rh {
                    BassRhythm::Whole => vec![(*s0, root)],
                    BassRhythm::Eighths => (*s0..*e0).step_by(eighth).map(|r| (r, root)).collect(),
                    BassRhythm::Offbeat => (*s0..*e0)
                        .filter(|r| (r - s0) % beat == eighth % beat && beat > 1)
                        .map(|r| (r, root))
                        .collect(),
                    BassRhythm::Rolling => (*s0..*e0)
                        .filter(|r| (r - s0) % beat != 0 && (r - s0) % sixteenth == 0)
                        .map(|r| (r, root))
                        .collect(),
                    BassRhythm::Octaves => (*s0..*e0)
                        .step_by(eighth)
                        .enumerate()
                        .map(|(i, r)| {
                            (
                                r,
                                if i % 2 == 1 {
                                    (root + 12).min(120)
                                } else {
                                    root
                                },
                            )
                        })
                        .collect(),
                };
                for (r, n) in rows {
                    put(p, r, o.channel, n);
                }
            }
        }
    }
    // note-off at the end of the written range on each channel used
    if end < p.rows {
        for k in 0..used {
            if p.cells[end][o.channel + k].note == 0 {
                put(p, end, o.channel + k, NOTE_OFF);
            }
        }
    }
    Ok(Written {
        notes,
        chords: spans.len(),
        channels: used,
        dropped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Cell;

    fn am() -> Key {
        Key::new(9, Scale::Minor)
    }

    #[test]
    fn scales_names_and_snap() {
        let c = Key::new(0, Scale::Major);
        assert_eq!(c.mask(), 0b1010_1011_0101);
        assert_eq!(c.name(), "C major");
        assert_eq!(Key::new(3, Scale::Minor).name(), "Eb minor");
        assert_eq!(Key::new(5, Scale::Major).name(), "F major");
        assert_eq!(Key::new(10, Scale::Major).name(), "Bb major");
        assert_eq!(Key::new(2, Scale::Minor).note_name(10), "Bb");
        // C# (note 38 = C#-3) snaps down to C-3 in C major; ties go down
        assert_eq!(c.snap(38), 37);
        assert_eq!(c.snap(37), 37);
        assert_eq!(c.snap(NOTE_OFF), NOTE_OFF);
        assert_eq!(c.snap(0), 0);
        // F# (pc 6) in A minor: F (5) and G (7) are equally near -> F
        assert_eq!(am().snap(1 + 4 * 12 + 6), 1 + 4 * 12 + 5);
        let pk = pack(Some(am()));
        for n in 1..=120u8 {
            assert!(am().contains(pc_of(snap_packed(n, pk))));
            assert_eq!(snap_packed(n, 0), n);
        }
        assert_eq!(parse_key("a minor"), Some(am()));
        assert_eq!(parse_key("Bb major"), Some(Key::new(10, Scale::Major)));
        assert_eq!(
            parse_key("F# phrygian dominant"),
            Some(Key::new(6, Scale::PhrygianDominant))
        );
        assert_eq!(parse_key("H minor"), None);
        for s in Scale::ALL {
            assert_eq!(parse_key(&Key::new(2, s).name()), Some(Key::new(2, s)));
            let st = s.steps();
            assert_eq!(st[0], 0);
            assert!(st.windows(2).all(|w| w[0] < w[1]) && *st.last().unwrap() < 12);
        }
    }

    #[test]
    fn diatonic_chords_names_and_romans() {
        let k = am();
        let names: Vec<String> = (0..7)
            .map(|d| diatonic(k, d, ChordKind::Triad).name(k))
            .collect();
        assert_eq!(names, ["Am", "Bdim", "C", "Dm", "Em", "F", "G"]);
        let rn: Vec<String> = (0..7)
            .map(|d| roman(k, d, &diatonic(k, d, ChordKind::Triad)))
            .collect();
        assert_eq!(rn, ["i", "ii°", "bIII", "iv", "v", "bVI", "bVII"]);
        let c = Key::new(0, Scale::Major);
        let sev: Vec<String> = (0..7)
            .map(|d| diatonic(c, d, ChordKind::Seventh).name(c))
            .collect();
        assert_eq!(sev, ["Cmaj7", "Dm7", "Em7", "Fmaj7", "G7", "Am7", "Bm7b5"]);
        assert_eq!(diatonic(c, 0, ChordKind::Sus4).name(c), "Csus4");
        assert_eq!(diatonic(c, 1, ChordKind::Sus2).name(c), "Dsus2");
        assert_eq!(diatonic(c, 0, ChordKind::Add9).name(c), "Cadd9");
        assert_eq!(diatonic(c, 4, ChordKind::Power).name(c), "G5");
        // harmonic minor: major V
        let h = Key::new(9, Scale::HarmonicMinor);
        assert_eq!(diatonic(h, 4, ChordKind::Triad).name(h), "E");
        // phrygian: flat II
        let ph = Key::new(9, Scale::Phrygian);
        assert_eq!(diatonic(ph, 1, ChordKind::Triad).name(ph), "A#");
        assert_eq!(roman(ph, 1, &diatonic(ph, 1, ChordKind::Triad)), "bII");
        // notes: Am triad at octave 3, inversions
        let a = diatonic(k, 0, ChordKind::Triad);
        assert_eq!(a.notes(3, 0), [46, 49, 53]); // A-3 C-4 E-4
        assert_eq!(a.notes(3, 1), [49, 53, 58]); // C E A
        assert_eq!(a.notes(3, 2), [53, 58, 61]); // E A C
                                                 // pentatonic chords still stack scale tones
        let p = Key::new(0, Scale::MajorPentatonic);
        assert_eq!(diatonic(p, 0, ChordKind::Triad).iv, [0, 4, 9]);
    }

    fn melody(key_notes: &[u8], rows: usize) -> Song {
        let mut s = Song::empty();
        s.instruments = crate::fileio::demo_song().instruments;
        // a melodic instrument (not drum-named)
        s.instruments[0].name = "Lead".into();
        s.patterns[0] = Pattern::new("m", rows, 4);
        for r in (0..rows).step_by(2) {
            s.patterns[0].cells[r][0] = Cell {
                note: key_notes[(r / 2) % key_notes.len()],
                instr: 1,
                ..Default::default()
            };
        }
        s
    }

    #[test]
    fn detects_keys_from_melodies() {
        // C major scale walk, tonic weighted
        let c = [37, 39, 41, 42, 44, 46, 48, 49, 37, 44, 37, 41];
        let k = detect(&melody(&c, 64));
        assert_eq!(k[0].0, Key::new(0, Scale::Major), "{:?}", &k[..3]);
        // A natural minor: A B C D E F G with A / E weight
        let a = [46, 48, 49, 51, 53, 54, 56, 46, 53, 46, 49, 46];
        let k = detect(&melody(&a, 64));
        assert_eq!(k[0].0, am(), "{:?}", &k[..3]);
        // with G# instead of G: harmonic minor fits best among A-rooted keys
        let h = [46, 48, 49, 51, 53, 54, 57, 46, 53, 46, 57, 46];
        let k = detect(&melody(&h, 64));
        let best_a = k.iter().find(|x| x.0.root == 9).unwrap();
        assert_eq!(best_a.0.scale, Scale::HarmonicMinor, "{:?}", &k[..4]);
        assert!(detect(&Song::empty()).is_empty());
    }

    #[test]
    fn detects_the_demo_songs_keys() {
        // Lumen Drift is in A minor (docs/CHANGELOG.md)
        let k = detect(&crate::fileio::demo_song());
        assert_eq!(k[0].0.root, 9, "{:?}", &k[..4]);
        assert!(!k[0].0.scale.majorish(), "{:?}", &k[..4]);
    }

    #[test]
    fn write_chords_arps_and_bass() {
        let k = am();
        let prog: Vec<Step> = [0, 5, 2, 6]
            .iter()
            .map(|&d| Step::new(d, ChordKind::Triad))
            .collect();
        let base = WriteOpts {
            style: Style::Chords,
            channel: 1,
            instr: 2,
            octave: 3,
            rows_per_beat: 4,
            start_row: 0,
            fill: false,
            vol: 0,
            seed: 1,
        };
        // chords: 4 bars of 16 rows over channels 1..=3
        let mut p = Pattern::new("b", 64, 6);
        p.cells[5][2].fx = 0x0C;
        let w = write(&mut p, k, &prog, &base).unwrap();
        assert_eq!(
            w,
            Written {
                notes: 12,
                chords: 4,
                channels: 3,
                dropped: 0
            }
        );
        let col =
            |p: &Pattern, r: usize| -> Vec<u8> { (1..4).map(|c| p.cells[r][c].note).collect() };
        assert_eq!(col(&p, 0), [46, 49, 53]); // Am
        assert_eq!(col(&p, 16), [42, 46, 49]); // F
        assert_eq!(col(&p, 32), [37, 41, 44]); // C
        assert_eq!(col(&p, 48), [44, 48, 51]); // G
        assert_eq!(p.cells[0][1].instr, 2);
        assert_eq!(p.cells[5][2].fx, 0x0C, "effect columns are kept");
        assert!(p.cells.iter().all(|r| r[0].is_empty() && r[4].is_empty()));
        // only 2 channels left: the top note is dropped
        let mut p = Pattern::new("b", 64, 3);
        let w = write(&mut p, k, &prog, &base).unwrap();
        assert_eq!((w.channels, w.dropped), (2, 4));
        // 7th chords + fill: 8 bars of chords in a 128-row block, note-off at the end of a short range
        let mut p = Pattern::new("b", 128, 8);
        let mut pr7 = prog.clone();
        pr7.iter_mut().for_each(|s| s.kind = ChordKind::Seventh);
        let w = write(&mut p, k, &pr7, &WriteOpts { fill: true, ..base }).unwrap();
        assert_eq!((w.chords, w.channels), (8, 4));
        let mut p = Pattern::new("b", 128, 8);
        write(
            &mut p,
            k,
            &prog,
            &WriteOpts {
                start_row: 8,
                ..base
            },
        )
        .unwrap();
        assert_eq!(p.cells[8][1].note, 46);
        assert_eq!(p.cells[72][1].note, NOTE_OFF);
        // arp up, 16ths, one octave: Am = A C E A C E ...
        let mut p = Pattern::new("b", 64, 4);
        let arp = Style::Arp {
            pattern: ArpPattern::Up,
            rate: 1,
            octaves: 1,
        };
        write(
            &mut p,
            k,
            &prog,
            &WriteOpts {
                style: arp,
                channel: 0,
                ..base
            },
        )
        .unwrap();
        let notes: Vec<u8> = (0..6).map(|r| p.cells[r][0].note).collect();
        assert_eq!(notes, [46, 49, 53, 46, 49, 53]);
        assert_eq!(p.cells[16][0].note, 42, "F on the next bar");
        // up-down over 2 octaves does not repeat the turning notes
        let mut p = Pattern::new("b", 64, 4);
        let arp = Style::Arp {
            pattern: ArpPattern::UpDown,
            rate: 2,
            octaves: 2,
        };
        write(
            &mut p,
            k,
            &prog,
            &WriteOpts {
                style: arp,
                channel: 0,
                ..base
            },
        )
        .unwrap();
        let notes: Vec<u8> = (0..20)
            .step_by(2)
            .map(|r| p.cells[r][0].note)
            .take(10)
            .collect();
        assert_eq!(&notes[..8], &[46, 49, 53, 58, 61, 65, 61, 58]);
        // bass: rolling 16ths = rows 1,2,3 of every beat, two octaves down
        let mut p = Pattern::new("b", 64, 4);
        write(
            &mut p,
            k,
            &prog,
            &WriteOpts {
                style: Style::Bass(BassRhythm::Rolling),
                channel: 0,
                ..base
            },
        )
        .unwrap();
        let on: Vec<usize> = (0..8).filter(|&r| p.cells[r][0].note > 0).collect();
        assert_eq!(on, [1, 2, 3, 5, 6, 7]);
        assert_eq!(p.cells[1][0].note, 46 - 24);
        let mut p = Pattern::new("b", 64, 4);
        write(
            &mut p,
            k,
            &prog,
            &WriteOpts {
                style: Style::Bass(BassRhythm::Offbeat),
                channel: 0,
                ..base
            },
        )
        .unwrap();
        let on: Vec<usize> = (0..16).filter(|&r| p.cells[r][0].note > 0).collect();
        assert_eq!(on, [2, 6, 10, 14]);
        // errors
        let mut p = Pattern::new("b", 64, 4);
        assert!(write(&mut p, k, &[], &base).is_err());
        assert!(write(&mut p, k, &prog, &WriteOpts { channel: 9, ..base }).is_err());
        assert!(write(
            &mut p,
            k,
            &prog,
            &WriteOpts {
                start_row: 64,
                ..base
            }
        )
        .is_err());
    }

    #[test]
    fn every_preset_writes_in_every_scale() {
        for s in Scale::ALL {
            for (_, degs, _) in PRESETS {
                let prog: Vec<Step> = degs
                    .iter()
                    .map(|&d| Step::new(d, ChordKind::Triad))
                    .collect();
                let mut p = Pattern::new("b", 64, 4);
                let w = write(
                    &mut p,
                    Key::new(4, s),
                    &prog,
                    &WriteOpts {
                        style: Style::Chords,
                        channel: 0,
                        instr: 1,
                        octave: 3,
                        rows_per_beat: 4,
                        start_row: 0,
                        fill: true,
                        vol: 0,
                        seed: 3,
                    },
                )
                .unwrap();
                assert!(w.notes >= 4);
                for r in &p.cells {
                    for c in r {
                        if c.note > 0 && c.note != NOTE_OFF {
                            assert!(Key::new(4, s).contains(pc_of(c.note)), "{s:?}");
                        }
                    }
                }
            }
        }
    }
}
