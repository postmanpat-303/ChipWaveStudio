//! Sheet music export (File > Export sheet music…, docs/NOTATION.md).
//!
//! The song's notes are read the way playback reads them (the timeline plan:
//! tracker blocks cell by cell, note clips as note spans; a tracker-mode song
//! is converted with `timeline::from_order` first), grouped into parts,
//! quantised to a grid, cut into bars of the song's time signatures and
//! split into values that can be written (ties join the pieces). Two
//! writers turn the result into a LilyPond `.ly` file and a MusicXML 4.0
//! file; a PDF is made by running LilyPond itself, when it is installed (it
//! is a separate program and is never bundled, see `lily`).
//!
//! Nothing here touches the audio: the export only reads the song.
use crate::chords::{Key, Scale};
use crate::drumkit;
use crate::gen::{DrumKind, GenEngine, KIT_MAP};
use crate::model::{InstrumentKind, Song};
use crate::timeline::{self as tl, SigPoint, Tick, Timeline, TrackKind, PPQ, ROW};

pub mod lily;
mod ly;
#[cfg(test)]
mod tests;
mod xml;

pub use ly::to_ly;
pub use xml::to_musicxml;

/// a note as the song plays it (ticks from the song start)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ev {
    pub start: Tick,
    pub end: Tick,
    /// tracker note number (1 = C-0, 49 = C-4)
    pub key: u8,
    /// instrument slot (1-based, 0 = none)
    pub instr: u8,
}

/// the notes of one track
#[derive(Clone, Debug, Default)]
pub struct TrackNotes {
    pub name: String,
    pub events: Vec<Ev>,
    pub muted: bool,
}

/// a drum sound on the drum staff
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Drum {
    Kick,
    Snare,
    Clap,
    HatClosed,
    HatOpen,
    Crash,
    Ride,
    TomLow,
    TomMid,
    TomHigh,
    Perc,
}

/// how each drum is written: LilyPond drum name, MusicXML display step and
/// octave, General MIDI key, an x notehead, a name
pub struct DrumInfo {
    pub ly: &'static str,
    pub step: char,
    pub octave: u8,
    pub gm: u8,
    pub x_head: bool,
    pub name: &'static str,
}

impl Drum {
    pub fn info(self) -> DrumInfo {
        let d = |ly, step, octave, gm, x_head, name| DrumInfo {
            ly,
            step,
            octave,
            gm,
            x_head,
            name,
        };
        match self {
            Drum::Kick => d("bd", 'F', 4, 36, false, "Kick"),
            Drum::Snare => d("sn", 'C', 5, 38, false, "Snare"),
            Drum::Clap => d("hc", 'D', 5, 39, true, "Clap"),
            Drum::HatClosed => d("hh", 'G', 5, 42, true, "Closed hi-hat"),
            Drum::HatOpen => d("hho", 'G', 5, 46, true, "Open hi-hat"),
            Drum::Crash => d("cymc", 'A', 5, 49, true, "Crash"),
            Drum::Ride => d("cymr", 'F', 5, 51, true, "Ride"),
            Drum::TomLow => d("toml", 'A', 4, 45, false, "Low tom"),
            Drum::TomMid => d("tommh", 'D', 5, 48, false, "Mid tom"),
            Drum::TomHigh => d("tomh", 'E', 5, 50, false, "High tom"),
            Drum::Perc => d("cb", 'E', 5, 56, false, "Percussion"),
        }
    }
}

/// a drum from a sound's name (None = not a drum name)
fn drum_by_name(name: &str) -> Option<Drum> {
    let n = name.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| n.contains(x));
    if has(&["kick", "bass drum", "bassdrum", "bd "]) || n == "bd" {
        Some(Drum::Kick)
    } else if has(&["snare", "rim"]) {
        Some(Drum::Snare)
    } else if has(&["clap"]) {
        Some(Drum::Clap)
    } else if has(&["open hat", "open hh", "open hi", "ohh", "hat open"]) || n == "oh" {
        Some(Drum::HatOpen)
    } else if has(&["hat", "hh", "hi-hat"]) {
        Some(Drum::HatClosed)
    } else if has(&["crash", "cymbal", "splash", "china"]) {
        Some(Drum::Crash)
    } else if has(&["ride"]) {
        Some(Drum::Ride)
    } else if has(&["tom"]) {
        Some(Drum::TomMid)
    } else if has(&[
        "perc", "shaker", "cowbell", "conga", "bongo", "tamb", "clave", "block",
    ]) {
        Some(Drum::Perc)
    } else {
        None
    }
}

fn drum_by_kind(kind: DrumKind, name: &str) -> Drum {
    match kind {
        DrumKind::Kick => Drum::Kick,
        DrumKind::Snare => Drum::Snare,
        DrumKind::Clap => Drum::Clap,
        DrumKind::Hat => {
            if drum_by_name(name) == Some(Drum::HatOpen) {
                Drum::HatOpen
            } else {
                Drum::HatClosed
            }
        }
        DrumKind::Tom => Drum::TomMid,
        DrumKind::Perc => match drum_by_name(name) {
            Some(d @ (Drum::Crash | Drum::Ride)) => d,
            _ => Drum::Perc,
        },
    }
}

/// the drum note `key` plays on instrument slot `instr` (None = a pitched
/// note)
pub fn drum_of(song: &Song, instr: u8, key: u8) -> Option<Drum> {
    let inst = song.instruments.get((instr as usize).checked_sub(1)?)?;
    match &inst.kind {
        InstrumentKind::Gen(g) => match &g.engine {
            GenEngine::Drum(p) => Some(drum_by_kind(p.kind, &inst.name)),
            GenEngine::Kit(_) => {
                let pc = (key as i32 - drumkit::C4_KEY).rem_euclid(12) as usize;
                let (piece, off) = KIT_MAP[pc];
                Some(match piece {
                    0 => Drum::Kick,
                    1 => Drum::Snare,
                    2 => Drum::Clap,
                    3 => Drum::HatClosed,
                    4 => Drum::HatOpen,
                    5 if off < -5.0 => Drum::TomLow,
                    5 if off > 2.0 => Drum::TomHigh,
                    5 => Drum::TomMid,
                    _ => Drum::Perc,
                })
            }
            GenEngine::Drums(k) => {
                let (i, _) = k.pad_for_key(key as i32)?;
                let p = &k.pads[i];
                let d = drum_by_name(&p.name).unwrap_or_else(|| drum_by_kind(p.drum.kind, &p.name));
                Some(match d {
                    Drum::TomMid if p.drum.kind == DrumKind::Tom => {
                        if p.key as i32 + (p.tune as i32) < drumkit::C4_KEY - 3 {
                            Drum::TomLow
                        } else if p.key as i32 + (p.tune as i32) > drumkit::C4_KEY + 6 {
                            Drum::TomHigh
                        } else {
                            Drum::TomMid
                        }
                    }
                    d => d,
                })
            }
            _ => None,
        },
        _ => drum_by_name(&inst.name),
    }
}

/// a sound effect instrument (noise risers and impacts): left out of the
/// score unless ticked
fn is_sfx(song: &Song, instr: u8) -> bool {
    let Some(inst) = (instr as usize)
        .checked_sub(1)
        .and_then(|i| song.instruments.get(i))
    else {
        return false;
    };
    matches!(
        &inst.kind,
        InstrumentKind::Gen(g) if matches!(g.engine, GenEngine::Noise(_))
    )
}

/// the song's timeline (a tracker-mode song converted, as playback does)
pub fn timeline_of(song: &Song) -> std::borrow::Cow<'_, Timeline> {
    match song.timeline.as_deref() {
        Some(t) => std::borrow::Cow::Borrowed(t),
        None => std::borrow::Cow::Owned(tl::from_order(song).0),
    }
}

/// every note track's notes, in the order playback plays them. Tracker
/// blocks: a note lasts until the next note or note-off on its track, or
/// until a gap between blocks; an empty instrument column keeps the last
/// instrument. Note clips: their note spans (chance and conditions as a
/// full play rolls them).
pub fn extract(song: &Song, t: &Timeline) -> Vec<TrackNotes> {
    let plan = tl::build_plan(song, t);
    let nch = song.channels.min(crate::model::MAX_CHANNELS);
    let mut out = vec![];
    for (tr, track) in t.tracks.iter().enumerate().take(nch) {
        if track.kind != TrackKind::Note {
            continue;
        }
        let mut evs: Vec<Ev> = vec![];
        let mut cur = t.track_instr(tr);
        // block notes
        let mut open: Option<Ev> = None;
        let mut last_end = 0usize;
        for seg in plan.tracks.get(tr).map_or(&[][..], |v| &v[..]) {
            if seg.start != last_end {
                if let Some(mut o) = open.take() {
                    o.end = last_end as Tick * ROW;
                    evs.push(o);
                }
            }
            let limit = tl::source_limit(song, seg.pattern);
            for row in seg.start..seg.end {
                let c = tl::plan_cell(song, seg, row, limit);
                if c.instr != 0 {
                    cur = c.instr;
                }
                if c.note == 0 {
                    continue;
                }
                let at = row as Tick * ROW;
                if let Some(mut o) = open.take() {
                    o.end = at;
                    evs.push(o);
                }
                if (1..=120).contains(&c.note) {
                    open = Some(Ev {
                        start: at,
                        end: at,
                        key: c.note,
                        instr: cur,
                    });
                }
            }
            last_end = seg.end;
        }
        if let Some(mut o) = open.take() {
            o.end = last_end as Tick * ROW;
            evs.push(o);
        }
        // note clip notes
        for n in plan.notes.get(tr).map_or(&[][..], |v| &v[..]) {
            if !(1..=120).contains(&n.key) || n.end <= n.start {
                continue;
            }
            let instr = if n.instr != 0 {
                n.instr
            } else {
                t.track_instr(tr)
            };
            evs.push(Ev {
                start: n.start,
                end: n.end,
                key: n.key,
                instr: if instr != 0 { instr } else { cur },
            });
        }
        evs.retain(|e| e.end > e.start);
        evs.sort_by_key(|e| (e.start, e.key));
        let muted = song.mixer.channels.get(tr).is_some_and(|m| m.mute);
        let name = if track.name.trim().is_empty() {
            song.channel_names
                .get(tr)
                .filter(|n| !n.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Track {}", tr + 1))
        } else {
            track.name.clone()
        };
        out.push(TrackNotes {
            name,
            events: evs,
            muted,
        });
    }
    out
}

/// one part (a staff, a grand staff or a drum staff) the dialog lists
#[derive(Clone, Debug, PartialEq)]
pub struct PartSpec {
    /// indices into the `extract` list
    pub tracks: Vec<usize>,
    pub name: String,
    pub drums: bool,
    /// only sound effects (noise risers, impacts): unticked by default
    pub sfx: bool,
    pub muted: bool,
    pub notes: usize,
    /// ticked for export
    pub on: bool,
}

/// the instrument most of `evs` play (0 = none)
fn main_instr(evs: &[Ev]) -> u8 {
    let mut n = [0usize; 256];
    for e in evs {
        n[e.instr as usize] += 1;
    }
    (0..256)
        .max_by_key(|&i| (n[i], usize::MAX - i))
        .unwrap_or(0) as u8
}

/// the parts of a song: one per track with notes, or (`merge`) one per
/// instrument when several tracks play the same one (a chord spread over
/// three columns becomes one staff of chords)
pub fn parts(song: &Song, tracks: &[TrackNotes], merge: bool) -> Vec<PartSpec> {
    let mut out: Vec<(u8, PartSpec)> = vec![];
    for (i, t) in tracks.iter().enumerate() {
        if t.events.is_empty() {
            continue;
        }
        let drums_n = t
            .events
            .iter()
            .filter(|e| drum_of(song, e.instr, e.key).is_some())
            .count();
        let drums = drums_n * 2 > t.events.len();
        let sfx = t.events.iter().all(|e| is_sfx(song, e.instr));
        let mi = main_instr(&t.events);
        if merge && mi != 0 {
            // every drum track shares one drum staff; other tracks share a
            // staff with the tracks on the same instrument
            if let Some((_, p)) = out
                .iter_mut()
                .find(|(m, p)| p.drums == drums && p.sfx == sfx && (*m == mi || (drums && !sfx)))
            {
                p.tracks.push(i);
                p.notes += t.events.len();
                p.muted &= t.muted;
                p.on = !p.sfx && !p.muted;
                continue;
            }
        }
        out.push((
            mi,
            PartSpec {
                tracks: vec![i],
                name: t.name.clone(),
                drums,
                sfx,
                muted: t.muted,
                notes: t.events.len(),
                on: !sfx && !t.muted,
            },
        ));
    }
    // a part merged from several tracks is named after its instrument
    // (drums: "Drums")
    for (mi, p) in out.iter_mut() {
        if p.tracks.len() > 1 && p.drums {
            p.name = "Drums".into();
        } else if p.tracks.len() > 1 {
            if let Some(inst) = song.instruments.get((*mi as usize).wrapping_sub(1)) {
                if !inst.name.trim().is_empty() {
                    p.name = inst.name.clone();
                }
            }
        }
    }
    out.into_iter().map(|(_, p)| p).collect()
}

/// the key to write
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyChoice {
    /// the song's key (Song info / chord helper), else the detected one
    Auto,
    /// this key
    Key(Key),
    /// no key signature (C major, accidentals as needed)
    None,
}

/// what the dialog asks for
#[derive(Clone, Debug, PartialEq)]
pub struct Opts {
    pub title: String,
    pub composer: String,
    /// quantise grid in ticks (120 = 1/32, 240 = 1/16, 480 = 1/8, 960 = 1/4)
    pub grid: Tick,
    /// first and last bar, 1-based and inclusive (0 = the song's last bar)
    pub bars: (i64, i64),
    pub key: KeyChoice,
    /// write drum parts
    pub drums: bool,
    pub merge: bool,
    /// the parts (from `parts`), with their ticks
    pub parts: Vec<PartSpec>,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            title: String::new(),
            composer: String::new(),
            grid: ROW,
            bars: (1, 0),
            key: KeyChoice::Auto,
            drums: true,
            merge: true,
            parts: vec![],
        }
    }
}

/// grid choices for the dialog: (ticks, label)
pub const GRIDS: [(Tick, &str); 4] = [
    (PPQ, "1/4"),
    (PPQ / 2, "1/8"),
    (PPQ / 4, "1/16"),
    (PPQ / 8, "1/32"),
];

/// the clef of a staff
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clef {
    Treble,
    /// treble, sounding an octave higher than written (high leads, bells)
    Treble8va,
    /// treble, sounding an octave lower (guitar range)
    Treble8vb,
    Bass,
    /// bass, sounding an octave lower (sub bass)
    Bass8vb,
    Percussion,
}

impl Clef {
    pub fn ly(self) -> &'static str {
        match self {
            Clef::Treble => "treble",
            Clef::Treble8va => "\"treble^8\"",
            Clef::Treble8vb => "\"treble_8\"",
            Clef::Bass => "bass",
            Clef::Bass8vb => "\"bass_8\"",
            Clef::Percussion => "percussion",
        }
    }
    /// MusicXML sign, line and octave change
    pub fn xml(self) -> (&'static str, u8, i8) {
        match self {
            Clef::Treble => ("G", 2, 0),
            Clef::Treble8va => ("G", 2, 1),
            Clef::Treble8vb => ("G", 2, -1),
            Clef::Bass => ("F", 4, 0),
            Clef::Bass8vb => ("F", 4, -1),
            Clef::Percussion => ("percussion", 2, 0),
        }
    }
}

/// what sounds at one moment of a staff
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Rest,
    /// MIDI pitches, low to high
    Notes(Vec<u8>),
    Drums(Vec<Drum>),
}

/// one written value
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub dur: Tick,
    pub content: Content,
    /// tied to the next item
    pub tie: bool,
    /// tied from the previous item
    pub tied: bool,
    /// a whole-bar rest
    pub bar_rest: bool,
}

/// one staff's music, bar by bar
#[derive(Clone, Debug, PartialEq)]
pub struct Staff {
    pub clef: Clef,
    pub bars: Vec<Vec<Item>>,
}

/// one part of the score
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub drums: bool,
    /// one staff, or two (a grand staff, upper first)
    pub staves: Vec<Staff>,
}

/// a bar of the score
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub start: Tick,
    pub len: Tick,
    pub num: u8,
    pub den: u8,
    /// tempo mark at the bar start: (bpm, text such as "accel.")
    pub tempo: Option<(f32, &'static str)>,
}

/// the key signature: fifths (-7..7, flats negative) and minor
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeySig {
    pub fifths: i8,
    pub minor: bool,
}

/// the engraved score, ready for the writers
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub title: String,
    pub composer: String,
    pub key: KeySig,
    pub bars: Vec<Bar>,
    /// rehearsal marks (timeline markers): (bar index, name)
    pub marks: Vec<(usize, String)>,
    pub parts: Vec<Part>,
}

/// major tonic pitch class of each key signature, fifths -7..=7
const SIG_TONIC: [(i8, u8, char, i8); 15] = [
    (-7, 11, 'C', -1),
    (-6, 6, 'G', -1),
    (-5, 1, 'D', -1),
    (-4, 8, 'A', -1),
    (-3, 3, 'E', -1),
    (-2, 10, 'B', -1),
    (-1, 5, 'F', 0),
    (0, 0, 'C', 0),
    (1, 7, 'G', 0),
    (2, 2, 'D', 0),
    (3, 9, 'A', 0),
    (4, 4, 'E', 0),
    (5, 11, 'B', 0),
    (6, 6, 'F', 1),
    (7, 1, 'C', 1),
];

fn major_mask(tonic: u8) -> u16 {
    [0u8, 2, 4, 5, 7, 9, 11]
        .iter()
        .fold(0u16, |m, s| m | 1 << ((tonic + s) % 12))
}

impl KeySig {
    /// the signature that holds most of the key's notes (fewest
    /// accidentals on a tie), minor for minor-type scales
    pub fn of(key: Key) -> KeySig {
        let mask = key
            .scale
            .steps()
            .iter()
            .fold(0u16, |m, s| m | 1 << ((key.root + s) % 12));
        let minor = matches!(
            key.scale,
            Scale::Minor
                | Scale::HarmonicMinor
                | Scale::MelodicMinor
                | Scale::MinorPentatonic
                | Scale::Blues
        );
        // the tonic should be in the signature's scale; prefer the
        // signature whose major (or relative minor) tonic is the root
        let best = SIG_TONIC
            .iter()
            .filter(|s| s.0.abs() <= 6)
            .max_by_key(|s| {
                let hits = (major_mask(s.1) & mask).count_ones() as i32;
                let home = if minor {
                    (s.1 + 9) % 12 == key.root
                } else {
                    s.1 == key.root
                };
                (hits, home as i32, -(s.0.abs() as i32), s.0 > 0)
            })
            .unwrap();
        KeySig {
            fifths: best.0,
            minor,
        }
    }
    /// (letter, alteration) of the major tonic
    fn tonic(self) -> (char, i8, u8) {
        let s = SIG_TONIC.iter().find(|s| s.0 == self.fifths).unwrap();
        (s.2, s.3, s.1)
    }
    /// (letter, alteration) of the key's own tonic (relative minor when
    /// minor)
    pub fn home(self) -> (char, i8) {
        let (l, a, pc) = self.tonic();
        if !self.minor {
            return (l, a);
        }
        let li = LETTERS.iter().position(|&x| x == l).unwrap();
        let ml = LETTERS[(li + 5) % 7];
        let mpc = (pc + 9) % 12;
        let nat = LETTER_PC[(li + 5) % 7];
        let alt = (mpc as i8 - nat as i8 + 18).rem_euclid(12) - 6;
        (ml, alt)
    }
    /// how to write MIDI pitch `p`: (letter, alteration, octave) with the
    /// key's own letters for its notes, sharps or flats for the others
    pub fn spell(self, p: u8) -> (char, i8, i32) {
        let pc = p % 12;
        let (tl_, ta, tpc) = self.tonic();
        let ti = LETTERS.iter().position(|&x| x == tl_).unwrap();
        let _ = ta;
        let steps = [0u8, 2, 4, 5, 7, 9, 11];
        let mut spelled = None;
        for (d, s) in steps.iter().enumerate() {
            if (tpc + s) % 12 == pc {
                let li = (ti + d) % 7;
                let nat = LETTER_PC[li];
                let alt = (pc as i8 - nat as i8 + 18).rem_euclid(12) - 6;
                spelled = Some((LETTERS[li], alt));
            }
        }
        let (l, a) = spelled.unwrap_or_else(|| {
            if self.fifths < 0 {
                FLAT_SPELL[pc as usize]
            } else {
                SHARP_SPELL[pc as usize]
            }
        });
        let li = LETTERS.iter().position(|&x| x == l).unwrap();
        let oct = (p as i32 - a as i32 - LETTER_PC[li] as i32).div_euclid(12) - 1;
        (l, a, oct)
    }
}

const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
const LETTER_PC: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
const SHARP_SPELL: [(char, i8); 12] = [
    ('C', 0),
    ('C', 1),
    ('D', 0),
    ('D', 1),
    ('E', 0),
    ('F', 0),
    ('F', 1),
    ('G', 0),
    ('G', 1),
    ('A', 0),
    ('A', 1),
    ('B', 0),
];
const FLAT_SPELL: [(char, i8); 12] = [
    ('C', 0),
    ('D', -1),
    ('D', 0),
    ('E', -1),
    ('E', 0),
    ('F', 0),
    ('G', -1),
    ('G', 0),
    ('A', -1),
    ('A', 0),
    ('B', -1),
    ('B', 0),
];

/// MIDI pitch of a tracker note on an instrument (its Transpose applied)
fn midi_of(song: &Song, e: &Ev) -> u8 {
    let tr = (e.instr as usize)
        .checked_sub(1)
        .and_then(|i| song.instruments.get(i))
        .map_or(0, |i| i.transpose as i32);
    (e.key as i32 + 11 + tr).clamp(0, 127) as u8
}

/// the key the score is written in
pub fn score_key(song: &Song, tracks: &[TrackNotes], choice: KeyChoice) -> Option<Key> {
    match choice {
        KeyChoice::Key(k) => Some(k),
        KeyChoice::None => None,
        KeyChoice::Auto => song.info.key.or_else(|| detect_key(song, tracks)),
    }
}

/// the most likely key from the pitched notes (time-weighted)
pub fn detect_key(song: &Song, tracks: &[TrackNotes]) -> Option<Key> {
    let mut h = [0f32; 12];
    for t in tracks {
        for e in &t.events {
            if drum_of(song, e.instr, e.key).is_none() && !is_sfx(song, e.instr) {
                h[(midi_of(song, e) % 12) as usize] += (e.end - e.start).min(4 * PPQ) as f32;
            }
        }
    }
    crate::chords::detect_hist(&h).first().map(|k| k.0)
}

/// the bars of the song: (start, length, signature)
fn song_bars(t: &Timeline, end: Tick) -> Vec<(Tick, Tick, SigPoint)> {
    let end = end.max(1);
    t.bars_in(0, end)
        .into_iter()
        .map(|(tick, _)| {
            let s = t.sig_at(tick);
            (tick, s.bar_ticks().max(1), s)
        })
        .collect()
}

/// how many bars the song has (the dialog's bar range)
pub fn bar_count(song: &Song) -> i64 {
    let t = timeline_of(song);
    song_bars(&t, tl::play_end(&t)).len().max(1) as i64
}

/// note values that can be written: (ticks, the undotted value the start
/// must line up with)
const VALUES: [(Tick, Tick); 12] = [
    (6 * PPQ, 4 * PPQ),
    (4 * PPQ, 4 * PPQ),
    (3 * PPQ, 2 * PPQ),
    (2 * PPQ, 2 * PPQ),
    (3 * PPQ / 2, PPQ),
    (PPQ, PPQ),
    (3 * PPQ / 4, PPQ / 2),
    (PPQ / 2, PPQ / 2),
    (3 * PPQ / 8, PPQ / 4),
    (PPQ / 4, PPQ / 4),
    (3 * PPQ / 16, PPQ / 8),
    (PPQ / 8, PPQ / 8),
];

/// smallest value written (a 32nd)
pub const MIN_VALUE: Tick = PPQ / 8;

/// can a value of `d` ticks (undotted value `a`) start `p` ticks into a
/// bar of `bar` ticks: whole notes start the bar, quarters and longer start
/// on a beat, shorter values on a multiple of their own undotted value
fn fits(p: Tick, d: Tick, a: Tick, bar: Tick) -> bool {
    p + d <= bar
        && if a >= 4 * PPQ {
            p == 0
        } else if d >= PPQ {
            p % PPQ == 0
        } else {
            p % a == 0
        }
}

/// cut `len` ticks starting `pos` ticks into a bar of `bar` ticks into
/// values that can be written (fewest pieces, plain values before dotted
/// ones); `dots` = dotted values too (notes; rests are written undotted)
pub fn split(pos: Tick, len: Tick, bar: Tick, dots: bool) -> Vec<Tick> {
    if len <= 0 {
        return vec![];
    }
    let u = MIN_VALUE;
    if pos % u == 0 && len % u == 0 {
        // shortest path over 32nd steps: cost = 10 per piece + 1 per dot
        let n = (len / u) as usize;
        let mut best: Vec<Option<(u32, Tick)>> = vec![None; n + 1];
        best[0] = Some((0, 0));
        for i in 0..n {
            let Some((c, _)) = best[i] else { continue };
            let p = pos + i as Tick * u;
            for &(d, a) in VALUES.iter().filter(|&&(d, a)| dots || d == a) {
                let j = i + (d / u) as usize;
                if j > n || !fits(p, d, a, bar.max(pos + len)) {
                    continue;
                }
                let cost = c + 10 + u32::from(d != a);
                if best[j].is_none_or(|(bc, _)| cost < bc) {
                    best[j] = Some((cost, d));
                }
            }
        }
        if best[n].is_some() {
            let mut out = vec![];
            let mut j = n;
            while j > 0 {
                let d = best[j].unwrap().1;
                out.push(d);
                j -= (d / u) as usize;
            }
            out.reverse();
            return out;
        }
    }
    // off the 32nd grid (should not happen): biggest values first
    let mut out = vec![];
    let mut left = len;
    while left > 0 {
        let d = VALUES
            .iter()
            .find(|&&(d, _)| d <= left)
            .map_or(left, |v| v.0);
        out.push(d);
        left -= d;
    }
    out
}

fn snap(t: Tick, bar0: Tick, g: Tick) -> Tick {
    bar0 + ((t - bar0) as f64 / g as f64).round() as Tick * g
}

/// a span of a staff before it is cut into bars
#[derive(Clone, Debug)]
struct Span {
    start: Tick,
    end: Tick,
    content: Content,
}

/// quantise a staff's notes and lay them end to end: notes starting
/// together become a chord, a note is cut at the next onset, gaps are rests
fn spans(notes: &[(Tick, Tick, Content)], bars: &[Bar], g: Tick, drums: bool) -> Vec<Span> {
    let bar_at = |t: Tick| {
        let k = bars.partition_point(|b| b.start <= t).saturating_sub(1);
        bars[k]
    };
    let mut q: Vec<(Tick, Tick, Content)> = notes
        .iter()
        .map(|(s, e, c)| {
            let b = bar_at(*s);
            let qs = snap(*s, b.start, g);
            let be = bar_at(*e);
            let mut qe = snap(*e, be.start, g);
            if qe <= qs {
                qe = qs + g;
            }
            (qs, qe, c.clone())
        })
        .collect();
    q.sort_by_key(|x| x.0);
    // merge same-start notes into chords
    let mut chords: Vec<Span> = vec![];
    for (s, e, c) in q {
        if let Some(last) = chords.last_mut() {
            if last.start == s {
                last.end = last.end.max(e);
                match (&mut last.content, c) {
                    (Content::Notes(a), Content::Notes(b)) => a.extend(b),
                    (Content::Drums(a), Content::Drums(b)) => a.extend(b),
                    _ => {}
                }
                continue;
            }
        }
        chords.push(Span {
            start: s,
            end: e,
            content: c,
        });
    }
    for c in chords.iter_mut() {
        match &mut c.content {
            Content::Notes(a) => {
                a.sort();
                a.dedup();
            }
            Content::Drums(a) => {
                a.sort();
                a.dedup();
            }
            Content::Rest => {}
        }
    }
    let n = chords.len();
    for i in 0..n {
        if i + 1 < n {
            let next = chords[i + 1].start;
            chords[i].end = chords[i].end.min(next);
        }
        if drums {
            // a hit lasts at most to the end of its beat
            let b = bar_at(chords[i].start);
            let beat = (b.len / b.num.max(1) as Tick).max(g);
            let into = chords[i].start - b.start;
            let beat_end = b.start + (into / beat + 1) * beat;
            chords[i].end = chords[i].end.min(beat_end);
        }
    }
    let t0 = bars.first().map_or(0, |b| b.start);
    let t1 = bars.last().map_or(0, |b| b.start + b.len);
    let mut out = vec![];
    let mut at = t0;
    for c in chords {
        if c.start >= t1 || c.end <= t0 {
            continue;
        }
        let (s, e) = (c.start.max(t0), c.end.min(t1));
        if s > at {
            out.push(Span {
                start: at,
                end: s,
                content: Content::Rest,
            });
        }
        let s = s.max(at);
        if e > s {
            out.push(Span {
                start: s,
                end: e,
                content: c.content,
            });
            at = e;
        }
    }
    if at < t1 {
        out.push(Span {
            start: at,
            end: t1,
            content: Content::Rest,
        });
    }
    out
}

/// cut spans into bars of written values (ties join a note's pieces;
/// drums are never tied, their extra pieces are rests)
fn engrave(sp: &[Span], bars: &[Bar]) -> Vec<Vec<Item>> {
    let mut out = vec![];
    let mut k = 0usize;
    for b in bars {
        let (bs, be) = (b.start, b.start + b.len);
        let mut items: Vec<Item> = vec![];
        while k < sp.len() && sp[k].end <= bs {
            k += 1;
        }
        let mut j = k;
        while j < sp.len() && sp[j].start < be {
            let s = &sp[j];
            let (a, e) = (s.start.max(bs), s.end.min(be));
            if s.content == Content::Rest && a == bs && e == be {
                items.push(Item {
                    dur: b.len,
                    content: Content::Rest,
                    tie: false,
                    tied: false,
                    bar_rest: true,
                });
            } else {
                let pieces = split(a - bs, e - a, b.len, s.content != Content::Rest);
                let np = pieces.len();
                for (i, d) in pieces.into_iter().enumerate() {
                    let first_of_note = a == s.start && i == 0;
                    let last_of_note = e == s.end && i + 1 == np;
                    let content = match &s.content {
                        Content::Drums(_) if !first_of_note => Content::Rest,
                        c => c.clone(),
                    };
                    let pitched = matches!(content, Content::Notes(_));
                    items.push(Item {
                        dur: d,
                        content,
                        tie: pitched && !last_of_note,
                        tied: pitched && !first_of_note,
                        bar_rest: false,
                    });
                }
            }
            if s.end <= be {
                j += 1;
            } else {
                break;
            }
        }
        k = j;
        out.push(items);
    }
    out
}

/// choose clefs for pitched notes: one staff, or a grand staff when the
/// notes spread over both clefs
fn clefs(pitches: &[u8]) -> Vec<Clef> {
    if pitches.is_empty() {
        return vec![Clef::Treble];
    }
    let lo = *pitches.iter().min().unwrap();
    let hi = *pitches.iter().max().unwrap();
    let avg = pitches.iter().map(|&p| p as f32).sum::<f32>() / pitches.len() as f32;
    if lo < 53 && hi > 67 && hi - lo > 24 {
        return vec![Clef::Treble, Clef::Bass];
    }
    if avg >= 81.0 && lo >= 67 {
        vec![Clef::Treble8va]
    } else if avg >= 59.0 {
        vec![Clef::Treble]
    } else if avg >= 52.0 && lo >= 48 {
        vec![Clef::Treble8vb]
    } else if avg < 36.0 && hi < 48 {
        vec![Clef::Bass8vb]
    } else {
        vec![Clef::Bass]
    }
}

/// the score bars [first, last] (1-based, last 0 = the end) with their
/// tempo marks
fn score_bars(song: &Song, t: &Timeline, range: (i64, i64)) -> Vec<Bar> {
    let all = song_bars(t, tl::play_end(t));
    let n = all.len() as i64;
    let last = if range.1 <= 0 { n } else { range.1.min(n) };
    let first = range.0.clamp(1, last.max(1));
    let mut out: Vec<Bar> = all[(first - 1) as usize..last.max(first) as usize]
        .iter()
        .map(|&(start, len, s)| Bar {
            start,
            len,
            num: s.num.max(1),
            den: s.den.clamp(1, 32),
            tempo: None,
        })
        .collect();
    let pts: Vec<tl::TempoPoint> = if t.tempo.is_empty() {
        vec![tl::TempoPoint {
            tick: 0,
            bpm: song.bpm,
            ramp: false,
        }]
    } else {
        t.tempo.clone()
    };
    let mut last_bpm = f32::NAN;
    for (bi, b) in out.iter_mut().enumerate() {
        let end = b.start + b.len;
        let inside: Vec<usize> = (0..pts.len())
            .filter(|&i| pts[i].tick >= b.start && pts[i].tick < end)
            .collect();
        let (bpm, text) = match inside.last() {
            Some(&i) => {
                let p = &pts[i];
                let text = match pts.get(i + 1) {
                    Some(n) if p.ramp && n.bpm > p.bpm => "accel.",
                    Some(n) if p.ramp && n.bpm < p.bpm => "rit.",
                    _ => "",
                };
                (p.bpm, text)
            }
            None => (t.bpm_at(b.start, song.bpm), ""),
        };
        if bi == 0 || !text.is_empty() || (!inside.is_empty() && (bpm - last_bpm).abs() >= 0.5) {
            b.tempo = Some((bpm, text));
            last_bpm = bpm;
        }
    }
    out
}

/// build the score from the song and the dialog's choices
pub fn build(song: &Song, o: &Opts) -> Score {
    let t = timeline_of(song);
    let tracks = extract(song, &t);
    let bars = score_bars(song, &t, o.bars);
    let g = o.grid.clamp(MIN_VALUE, PPQ);
    let key = score_key(song, &tracks, o.key)
        .map(KeySig::of)
        .unwrap_or(KeySig {
            fifths: 0,
            minor: false,
        });
    let mut parts = vec![];
    let specs = if o.parts.is_empty() {
        self::parts(song, &tracks, o.merge)
    } else {
        o.parts.clone()
    };
    for p in specs.iter().filter(|p| p.on && (o.drums || !p.drums)) {
        let evs: Vec<Ev> = p
            .tracks
            .iter()
            .filter_map(|&i| tracks.get(i))
            .flat_map(|t| t.events.iter().copied())
            .collect();
        if p.drums {
            let notes: Vec<_> = evs
                .iter()
                .map(|e| {
                    let d = drum_of(song, e.instr, e.key).unwrap_or(Drum::Perc);
                    (e.start, e.end, Content::Drums(vec![d]))
                })
                .collect();
            let sp = spans(&notes, &bars, g, true);
            parts.push(Part {
                name: p.name.clone(),
                drums: true,
                staves: vec![Staff {
                    clef: Clef::Percussion,
                    bars: engrave(&sp, &bars),
                }],
            });
            continue;
        }
        let pitched: Vec<(Tick, Tick, u8)> = evs
            .iter()
            .map(|e| (e.start, e.end, midi_of(song, e)))
            .collect();
        let in_range: Vec<u8> = pitched
            .iter()
            .filter(|n| {
                bars.first().is_some_and(|b| n.0 >= b.start)
                    && bars.last().is_some_and(|b| n.0 < b.start + b.len)
            })
            .map(|n| n.2)
            .collect();
        let cl = clefs(&in_range);
        let mut staves = vec![];
        for (si, &c) in cl.iter().enumerate() {
            let notes: Vec<_> = pitched
                .iter()
                .filter(|n| cl.len() == 1 || (si == 0) == (n.2 >= 60))
                .map(|n| (n.0, n.1, Content::Notes(vec![n.2])))
                .collect();
            let sp = spans(&notes, &bars, g, false);
            staves.push(Staff {
                clef: c,
                bars: engrave(&sp, &bars),
            });
        }
        parts.push(Part {
            name: p.name.clone(),
            drums: false,
            staves,
        });
    }
    let marks = t
        .markers
        .iter()
        .filter_map(|m| {
            let i = bars
                .iter()
                .position(|b| m.tick >= b.start && m.tick < b.start + b.len)?;
            Some((i, m.name.clone()))
        })
        .filter(|m| !m.1.trim().is_empty())
        .collect();
    Score {
        title: o.title.clone(),
        composer: o.composer.clone(),
        key,
        bars,
        marks,
        parts,
    }
}

/// the dialog's starting choices for a song: its title, the composer
/// from Settings (`composer`; blank stays blank, nothing is made up) and
/// its parts
pub fn default_opts(song: &Song, composer: &str) -> Opts {
    let t = timeline_of(song);
    let tracks = extract(song, &t);
    Opts {
        title: song.title.trim().to_string(),
        composer: composer.trim().to_string(),
        parts: parts(song, &tracks, true),
        ..Default::default()
    }
}

/// the parts again after the Merge box changes, keeping the ticks of
/// parts that stay the same
pub fn reparts(song: &Song, merge: bool, old: &[PartSpec]) -> Vec<PartSpec> {
    let t = timeline_of(song);
    let tracks = extract(song, &t);
    let mut ps = parts(song, &tracks, merge);
    for p in ps.iter_mut() {
        if let Some(o) = old
            .iter()
            .find(|o| o.tracks.iter().any(|t| p.tracks.contains(t)))
        {
            p.on = o.on;
        }
    }
    ps
}

/// written length of a value: (LilyPond duration, MusicXML type, dotted)
pub fn value_name(d: Tick) -> (&'static str, &'static str, bool) {
    match d {
        x if x == 6 * PPQ => ("1.", "whole", true),
        x if x == 4 * PPQ => ("1", "whole", false),
        x if x == 3 * PPQ => ("2.", "half", true),
        x if x == 2 * PPQ => ("2", "half", false),
        x if x == 3 * PPQ / 2 => ("4.", "quarter", true),
        x if x == PPQ => ("4", "quarter", false),
        x if x == 3 * PPQ / 4 => ("8.", "eighth", true),
        x if x == PPQ / 2 => ("8", "eighth", false),
        x if x == 3 * PPQ / 8 => ("16.", "16th", true),
        x if x == PPQ / 4 => ("16", "16th", false),
        x if x == 3 * PPQ / 16 => ("32.", "32nd", true),
        _ => ("32", "32nd", false),
    }
}
