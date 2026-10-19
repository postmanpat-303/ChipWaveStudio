//! Song data model: patterns of cells, an order list, instruments.
use serde::{Deserialize, Serialize};

use crate::synth::Patch;

pub const NOTE_OFF: u8 = 255;
pub const ROWS: usize = 64;
/// channels (tracks) per song: each song sets its own count, 1..=MAX_CHANNELS
/// (0.2.3.8 raised this from 8 to 32)
pub const MAX_CHANNELS: usize = 32;
/// Rows per beat for everything that measures time: a row is always a
/// sixteenth note (the engine's tick length is `sr*60 / (bpm*4*speed)`).
/// `Song::lines_per_beat` is kept in files for compatibility but never
/// changes timing (the timeline work fixed the places that used it as if
/// it did).
pub const ROWS_PER_BEAT: u32 = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// 0 = empty, 1..=120 = note (1 = C-0), 255 = note off
    pub note: u8,
    /// 0 = none, 1..=255 instrument slot
    pub instr: u8,
    /// 0 = none, 1..=65 => volume 0x00..0x40 (+1)
    pub vol: u8,
    pub fx: u8,
    pub fxp: u8,
}

impl Cell {
    pub fn is_empty(&self) -> bool {
        *self == Cell::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    pub rows: usize,
    /// cells[row][channel]
    pub cells: Vec<Vec<Cell>>,
    /// arrangement role tag (Fill to length)
    #[serde(default)]
    pub role: Option<crate::arrange::Role>,
    /// derived block (variation of another block); cells are rendered from the source
    #[serde(default)]
    pub derived: Option<crate::arrange::Derived>,
    /// cells as last rendered from the source (detects hand edits -> bake)
    #[serde(skip)]
    pub cache: Vec<Vec<Cell>>,
}

impl Pattern {
    pub fn new(name: &str, rows: usize, channels: usize) -> Self {
        Self {
            name: name.into(),
            rows,
            cells: vec![vec![Cell::default(); channels]; rows],
            role: None,
            derived: None,
            cache: vec![],
        }
    }

    /// longest block the length controls offer (files allow longer ones)
    pub const MAX_EDIT_ROWS: usize = 256;

    /// Cells (any channel) with something in them at or after row `rows`:
    /// what shortening the block to `rows` would cut.
    pub fn cut_by(&self, rows: usize) -> usize {
        self.cells
            .iter()
            .skip(rows)
            .flat_map(|r| r.iter())
            .filter(|c| !c.is_empty())
            .count()
    }

    /// Cells that halving with `squash` would lose (an odd row whose cell
    /// can't move onto an empty even row above it).
    pub fn lost_by_squash(&self) -> usize {
        let mut n = 0;
        for r in (1..self.rows).step_by(2) {
            for (ch, c) in self.cells[r].iter().enumerate() {
                if !c.is_empty() && !self.cells[r - 1][ch].is_empty() {
                    n += 1;
                }
            }
        }
        n
    }

    /// New length: growing adds empty rows, shrinking drops the rows after
    /// the new end. A derived block becomes an ordinary one (its cells are
    /// now hand-shaped).
    pub fn set_rows(&mut self, rows: usize) {
        let rows = rows.max(1);
        let ch = self.cells.first().map(|r| r.len()).unwrap_or(0);
        self.cells.resize(rows, vec![Cell::default(); ch]);
        self.rows = rows;
        self.derived = None;
        self.cache.clear();
    }

    /// Double the length and move every row r to 2r (notes keep their
    /// musical position; the tempo of the music halves relative to rows).
    pub fn stretch(&mut self) {
        let old = std::mem::take(&mut self.cells);
        let ch = old.first().map(|r| r.len()).unwrap_or(0);
        let rows = self.rows * 2;
        self.cells = vec![vec![Cell::default(); ch]; rows];
        for (r, row) in old.into_iter().enumerate().take(self.rows) {
            self.cells[r * 2] = row;
        }
        self.rows = rows;
        self.derived = None;
        self.cache.clear();
    }

    /// Halve the length and move row r to r/2. An odd row's cell moves up
    /// only where the even row above is empty; otherwise it is dropped
    /// (`lost_by_squash` counts those).
    pub fn squash(&mut self) {
        let rows = self.rows.div_ceil(2).max(1);
        let ch = self.cells.first().map(|r| r.len()).unwrap_or(0);
        let mut cells = vec![vec![Cell::default(); ch]; rows];
        for (r, row) in self.cells.iter().enumerate().take(self.rows) {
            for (c, cell) in row.iter().enumerate() {
                if cell.is_empty() {
                    continue;
                }
                let t = &mut cells[r / 2][c];
                if r % 2 == 0 || t.is_empty() {
                    *t = *cell;
                }
            }
        }
        self.cells = cells;
        self.rows = rows;
        self.derived = None;
        self.cache.clear();
    }
}

/// How the pattern editor numbers rows (Settings > Look > Pattern).
/// Decimal (0.2.3.17 default): 00..63, three digits for blocks over 100
/// rows. Hex: 00..3F, three digits over 256 rows.
pub fn row_label(r: usize, rows: usize, hex: bool) -> String {
    if hex {
        let w = if rows > 0x100 { 3 } else { 2 };
        format!("{r:0w$X}")
    } else {
        let w = if rows > 100 { 3 } else { 2 };
        format!("{r:0w$}")
    }
}

/// Sample PCM, shared copy-on-write: cloning a song (undo history, save and
/// export snapshots) shares the buffer; the first write makes a private copy.
#[derive(Clone, Debug, Default)]
pub struct SampleData(std::sync::Arc<Vec<f32>>);

impl SampleData {
    pub fn new(v: Vec<f32>) -> Self {
        Self(std::sync::Arc::new(v))
    }
    /// same buffer (no copy happened since the clone)
    pub fn same(&self, o: &SampleData) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &o.0)
    }
}

impl From<Vec<f32>> for SampleData {
    fn from(v: Vec<f32>) -> Self {
        Self::new(v)
    }
}

impl FromIterator<f32> for SampleData {
    fn from_iter<I: IntoIterator<Item = f32>>(it: I) -> Self {
        Self::new(it.into_iter().collect())
    }
}

impl std::ops::Deref for SampleData {
    type Target = Vec<f32>;
    fn deref(&self) -> &Vec<f32> {
        &self.0
    }
}

impl std::ops::DerefMut for SampleData {
    fn deref_mut(&mut self) -> &mut Vec<f32> {
        std::sync::Arc::make_mut(&mut self.0)
    }
}

impl PartialEq for SampleData {
    fn eq(&self, o: &Self) -> bool {
        self.same(o) || *self.0 == *o.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum InstrumentKind {
    Synth(Patch),
    /// mono f32 sample data at `rate`
    Sample {
        file: String,
        rate: u32,
        #[serde(skip)]
        data: SampleData,
        base_note: u8,
        /// loop mode / points / finetune
        #[serde(default)]
        lp: crate::sample::SampleLoop,
    },
    /// Voice (glitch): synthetic vowel generator (src/voice.rs)
    Voice(crate::voice::VoicePatch),
    /// Generator (drum synth, kit; src/gen.rs)
    Gen(crate::gen::GenPatch),
    /// CLAP plugin instrument (src/plugin/, 0.2.3.16)
    Plugin(crate::plugin::PluginPatch),
}

impl InstrumentKind {
    /// the synth patch (a Voice's envelopes / filter / LFO live in `base`)
    pub fn patch(&self) -> Option<&Patch> {
        match self {
            InstrumentKind::Synth(p) => Some(p),
            InstrumentKind::Voice(v) => Some(&v.base),
            InstrumentKind::Gen(g) => Some(&g.base),
            InstrumentKind::Sample { .. } | InstrumentKind::Plugin(_) => None,
        }
    }
    pub fn patch_mut(&mut self) -> Option<&mut Patch> {
        match self {
            InstrumentKind::Synth(p) => Some(p),
            InstrumentKind::Voice(v) => Some(&mut v.base),
            InstrumentKind::Gen(g) => Some(&mut g.base),
            InstrumentKind::Sample { .. } | InstrumentKind::Plugin(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instrument {
    pub name: String,
    pub volume: f32,
    /// per-instrument transpose in semitones (applied at playback)
    #[serde(default)]
    pub transpose: i8,
    /// insert FX chain (after the voice, before the mixer channel)
    #[serde(default)]
    pub fx: Vec<crate::fx::Device>,
    pub kind: InstrumentKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Song {
    pub title: String,
    pub bpm: f32,
    /// ticks per row
    pub speed: u32,
    pub lines_per_beat: u32,
    pub channels: usize,
    pub channel_names: Vec<String>,
    pub patterns: Vec<Pattern>,
    pub order: Vec<usize>,
    /// index 0 => instrument slot 1
    pub instruments: Vec<Instrument>,
    pub mixer: Mixer,
    /// arranger zoom in pixels per row (ChipWave-only, saved in arranger.toml)
    pub arranger_zoom: f32,
    /// automation lanes (song scope), saved as automation/NN.toml
    pub automation: Vec<crate::automation::Lane>,
    /// song info (artist, album, year, comment); used for export tags
    pub info: SongInfo,
    /// Some = **timeline mode** (docs/TIMELINE.md): the clips are the song
    /// and the order list is not played. Saved as `timeline.toml`.
    #[serde(skip)]
    pub timeline: Option<Box<crate::timeline::Timeline>>,
    /// drag-to-modulate: modulators and routes (Unreleased), saved as
    /// `modulation.toml`
    #[serde(skip)]
    pub modulation: crate::modulation::Modulation,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SongInfo {
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub year: String,
    #[serde(default)]
    pub comment: String,
    /// song key for the chord helper and scale lock (0.2.3.9)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<crate::chords::Key>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChanMix {
    /// linear gain 0..=2 (1 = 0 dB)
    pub volume: f32,
    /// -1 (left) ..= 1 (right)
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    /// post-fader sends to return A (delay) and B (reverb), 0..=1
    #[serde(default)]
    pub send: [f32; 2],
    /// sidechain ducking depth 0..=1, keyed by notes on `Mixer::duck_key`
    #[serde(default)]
    pub duck: f32,
}

impl Default for ChanMix {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pan: 0.0,
            mute: false,
            solo: false,
            send: [0.0; 2],
            duck: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mixer {
    pub channels: Vec<ChanMix>,
    pub master: f32,
    /// send/return buses: A = delay, B = reverb
    #[serde(default = "default_returns")]
    pub returns: Vec<Return>,
    /// sidechain key channel (0-based): its notes duck channels with `duck` > 0
    #[serde(default)]
    pub duck_key: usize,
    /// ducking recovery time in seconds
    #[serde(default = "default_duck_release")]
    pub duck_release: f32,
    /// master limiter (true-peak, lookahead); bypass = classic soft clip
    #[serde(default)]
    pub limiter: crate::master::LimiterSettings,
}

pub fn default_duck_release() -> f32 {
    0.2
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Return {
    pub name: String,
    pub level: f32,
    pub device: crate::fx::Device,
}

pub fn default_returns() -> Vec<Return> {
    use crate::fx::{Device, FxKind};
    let mut d = Device::new(FxKind::Delay);
    d.set_real("mix", 1.0);
    let mut r = Device::new(FxKind::Reverb);
    r.set_real("mix", 1.0);
    vec![
        Return {
            name: "A Delay".into(),
            level: 1.0,
            device: d,
        },
        Return {
            name: "B Reverb".into(),
            level: 1.0,
            device: r,
        },
    ]
}

impl Default for Mixer {
    fn default() -> Self {
        Self {
            channels: vec![ChanMix::default(); MAX_CHANNELS],
            master: 1.0,
            returns: default_returns(),
            duck_key: 0,
            duck_release: default_duck_release(),
            limiter: Default::default(),
        }
    }
}

impl Mixer {
    /// effective audibility of a channel (mute + solo logic)
    pub fn audible(&self, ch: usize) -> bool {
        let any_solo = self.channels.iter().any(|c| c.solo);
        match self.channels.get(ch) {
            Some(c) => !c.mute && (!any_solo || c.solo),
            None => true,
        }
    }
    pub fn fix(&mut self) {
        self.channels.resize(MAX_CHANNELS, ChanMix::default());
        self.master = self.master.clamp(0.0, 2.0);
        self.limiter.fix();
        let def = default_returns();
        if self.returns.len() != def.len() {
            self.returns = def;
        }
        for r in &mut self.returns {
            r.device.fix();
            r.level = r.level.clamp(0.0, 2.0);
        }
        for c in &mut self.channels {
            c.send = c.send.map(|v| v.clamp(0.0, 1.0));
            c.volume = c.volume.clamp(0.0, 2.0);
            c.pan = c.pan.clamp(-1.0, 1.0);
        }
    }
}

/// gain to dB text
pub fn db_text(g: f32) -> String {
    if g <= 0.0001 {
        "-inf dB".into()
    } else {
        format!("{:+.1} dB", 20.0 * g.log10())
    }
}

impl Song {
    pub fn empty() -> Self {
        Self {
            title: "Untitled".into(),
            bpm: 125.0,
            speed: 6,
            lines_per_beat: 4,
            channels: 4,
            channel_names: (1..=4).map(|i| format!("Ch {i}")).collect(),
            patterns: vec![Pattern::new("Block 00", ROWS, 4)],
            order: vec![0],
            instruments: vec![],
            mixer: Mixer::default(),
            arranger_zoom: 2.0,
            automation: vec![],
            info: SongInfo::default(),
            timeline: None,
            modulation: Default::default(),
        }
    }

    /// seconds per row: 4 rows per beat at the given BPM, as the engine plays
    /// it (speed only subdivides ticks; `lines_per_beat` does not count)
    pub fn row_seconds(&self) -> f64 {
        60.0 / (self.bpm as f64 * ROWS_PER_BEAT as f64)
    }

    pub fn length_seconds(&self) -> f64 {
        if let Some(tl) = &self.timeline {
            return tl.seconds_at(crate::timeline::play_end(tl), self.bpm);
        }
        let rows: usize = self
            .order
            .iter()
            .filter_map(|&p| self.patterns.get(p))
            .map(|p| p.rows)
            .sum();
        rows as f64 * self.row_seconds()
    }

    /// Cells in hand-written (non-derived) blocks that use instrument slot
    /// `slot` (1-based): what a delete would remove.
    pub fn instrument_uses(&self, slot: u8) -> usize {
        self.patterns
            .iter()
            .filter(|p| p.derived.is_none())
            .flat_map(|p| p.cells.iter().flatten())
            .filter(|c| slot > 0 && c.instr == slot)
            .count()
    }

    /// Remove instrument `idx` (0-based). Its notes are cleared (effect
    /// columns stay), later slots move down by one in every block and
    /// automation lane, and its own automation lanes go. The FX chain lives
    /// in the instrument, so every other instrument keeps its own. Returns
    /// the number of cells cleared (as `instrument_uses`).
    /// Modulation routes were removed (`keep[k]` = route k survives): the
    /// route-depth lanes (`mod.K.amount`) follow, lanes of removed routes go.
    pub fn routes_changed(&mut self, keep: &[bool]) {
        if keep.iter().all(|k| *k) {
            return;
        }
        let mut map = Vec::with_capacity(keep.len());
        let mut n = 0;
        for &k in keep {
            map.push(if k { Some(n) } else { None });
            n += k as usize;
        }
        for lanes in self.lane_lists_mut() {
            lanes.retain_mut(|l| match crate::automation::Target::parse(&l.target) {
                Some(crate::automation::Target::ModAmount(r)) => {
                    match map.get(r).copied().flatten() {
                        Some(nr) => {
                            l.target = crate::automation::Target::ModAmount(nr).id();
                            true
                        }
                        None => false,
                    }
                }
                _ => true,
            });
        }
    }

    /// every list of automation lanes: the song's, then each timeline
    /// track's and clip's (M5), for edits that renumber targets
    pub fn lane_lists_mut(&mut self) -> Vec<&mut Vec<crate::automation::Lane>> {
        let mut v = vec![&mut self.automation];
        if let Some(tl) = self.timeline.as_deref_mut() {
            for t in &mut tl.tracks {
                v.push(&mut t.auto);
            }
            for c in &mut tl.clips {
                v.push(&mut c.auto);
            }
        }
        v
    }
    /// remove modulator `k` with its routes (and their depth lanes)
    pub fn remove_modulator(&mut self, k: usize) {
        let keep = self.modulation.remove(k);
        self.routes_changed(&keep);
    }
    /// remove modulation route `k` (and its depth lane)
    pub fn remove_route(&mut self, k: usize) {
        let keep = self.modulation.remove_route(k);
        self.routes_changed(&keep);
    }

    pub fn delete_instrument(&mut self, idx: usize) -> usize {
        if idx >= self.instruments.len() || idx > 254 {
            return 0;
        }
        let used = self.instrument_uses(idx as u8 + 1);
        self.instruments.remove(idx);
        let slot = idx as u8 + 1;
        let fix = |c: &mut Cell| {
            if c.instr == slot {
                c.instr = 0;
                if c.note != NOTE_OFF {
                    c.note = 0;
                    c.vol = 0;
                }
            } else if c.instr > slot {
                c.instr -= 1;
            }
        };
        for p in &mut self.patterns {
            p.cells.iter_mut().flatten().for_each(fix);
            p.cache.iter_mut().flatten().for_each(fix);
        }
        for lanes in self.lane_lists_mut() {
            lanes.retain_mut(|l| match lane_instrument(&l.target) {
                Some(i) if i == idx => false,
                Some(i) if i > idx => {
                    l.target = with_lane_instrument(&l.target, i - 1);
                    true
                }
                _ => true,
            });
        }
        let keep = self.modulation.delete_instrument(idx);
        self.routes_changed(&keep);
        used
    }

    /// Move instrument `from` to position `to` (0-based; the slots between
    /// shift by one). Every note, cached block and automation lane follows
    /// its instrument, so the song sounds the same. Returns the old -> new
    /// index map (for the engine), None if an index is out of range.
    pub fn move_instrument(&mut self, from: usize, to: usize) -> Option<Vec<usize>> {
        let n = self.instruments.len();
        if from >= n || to >= n || n > 255 {
            return None;
        }
        let mut order: Vec<usize> = (0..n).collect();
        let x = order.remove(from);
        order.insert(to, x);
        // map[old] = new
        let mut map = vec![0; n];
        for (new, &old) in order.iter().enumerate() {
            map[old] = new;
        }
        let old = std::mem::take(&mut self.instruments);
        let mut slots: Vec<Option<Instrument>> = old.into_iter().map(Some).collect();
        self.instruments = order.iter().map(|&o| slots[o].take().unwrap()).collect();
        self.remap_slots(&map);
        Some(map)
    }

    /// renumber instrument slots in cells, block caches and lanes (map[old] = new, 0-based)
    fn remap_slots(&mut self, map: &[usize]) {
        let fix = |c: &mut Cell| {
            if c.instr > 0 {
                if let Some(&n) = map.get(c.instr as usize - 1) {
                    c.instr = n as u8 + 1;
                }
            }
        };
        for p in &mut self.patterns {
            p.cells.iter_mut().flatten().for_each(fix);
            p.cache.iter_mut().flatten().for_each(fix);
        }
        for lanes in self.lane_lists_mut() {
            for l in lanes.iter_mut() {
                if let Some(i) = lane_instrument(&l.target) {
                    if let Some(&n) = map.get(i) {
                        l.target = with_lane_instrument(&l.target, n);
                    }
                }
            }
        }
        self.modulation.remap(map);
    }

    /// Swap instrument slot `from` for `to` (1-based) on every note that
    /// uses it, in block `only` or the whole song. Returns the cells changed.
    pub fn replace_instrument_uses(&mut self, from: u8, to: u8, only: Option<usize>) -> usize {
        if from == 0 || to == 0 || from == to || to as usize > self.instruments.len() {
            return 0;
        }
        let mut n = 0;
        for (i, p) in self.patterns.iter_mut().enumerate() {
            if only.is_some_and(|b| b != i) || p.derived.is_some() {
                continue;
            }
            for c in p.cells.iter_mut().flatten() {
                if c.instr == from {
                    c.instr = to;
                    n += 1;
                }
            }
        }
        n
    }

    /// Insert `inst` as a new slot after the last one; None if all 255 are used.
    pub fn add_instrument(&mut self, inst: Instrument) -> Option<usize> {
        if self.instruments.len() >= 255 {
            return None;
        }
        self.instruments.push(inst);
        Some(self.instruments.len() - 1)
    }

    /// Append a copy of instrument `idx` (0-based, with its FX chain) and
    /// return the new index. Nothing else changes; None if the 255 slots
    /// are full.
    pub fn duplicate_instrument(&mut self, idx: usize) -> Option<usize> {
        if self.instruments.len() >= 255 {
            return None;
        }
        let mut i = self.instruments.get(idx)?.clone();
        i.name = format!("{} copy", i.name);
        self.instruments.push(i);
        Some(self.instruments.len() - 1)
    }
}

/// 0-based instrument of an `instrument.N....` automation target
pub(crate) fn lane_instrument(target: &str) -> Option<usize> {
    let mut p = target.split('.');
    if p.next()? != "instrument" {
        return None;
    }
    p.next()?
        .parse::<usize>()
        .ok()
        .filter(|n| *n >= 1)
        .map(|n| n - 1)
}

pub(crate) fn with_lane_instrument(target: &str, idx: usize) -> String {
    let rest = target.splitn(3, '.').nth(2).unwrap_or("");
    format!("instrument.{}.{}", idx + 1, rest)
}

pub fn note_name(n: u8) -> String {
    match n {
        0 => "---".into(),
        NOTE_OFF => "===".into(),
        n => {
            const NAMES: [&str; 12] = [
                "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
            ];
            let i = (n - 1) as usize;
            format!("{}{}", NAMES[i % 12], i / 12)
        }
    }
}

pub fn parse_note(s: &str) -> Option<u8> {
    let s = s.trim();
    if s == "---" || s.is_empty() {
        return Some(0);
    }
    if s == "===" || s == "^^^" || s == "OFF" {
        return Some(NOTE_OFF);
    }
    let b = s.as_bytes();
    if b.len() != 3 {
        return None;
    }
    let base = match b[0] {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let sharp = if b[1] == b'#' { 1 } else { 0 };
    let oct = (b[2] as char).to_digit(10)? as u8;
    Some(oct * 12 + base + sharp + 1)
}

/// MIDI-like frequency for our note numbering (C-4 = note 49 = 261.63 Hz)
pub fn note_freq(n: u8) -> f32 {
    let midi = n as f32 - 1.0 + 12.0; // C-0 => midi 12
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn note_roundtrip() {
        for n in 1..=96u8 {
            assert_eq!(parse_note(&note_name(n)), Some(n));
        }
        assert!((note_freq(parse_note("A-4").unwrap()) - 440.0).abs() < 0.01);
    }

    fn inst(name: &str) -> Instrument {
        Instrument {
            name: name.into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Synth(Default::default()),
        }
    }

    #[test]
    fn delete_instrument_remaps_notes_lanes_and_keeps_fx() {
        let mut s = Song::empty();
        s.instruments = vec![inst("a"), inst("b"), inst("c")];
        s.instruments[2]
            .fx
            .push(crate::fx::Device::new(crate::fx::FxKind::Delay));
        let c = |note, instr| Cell {
            note,
            instr,
            vol: 10,
            fx: 0xF,
            fxp: 0x80,
        };
        s.patterns[0].cells[0][0] = c(49, 2);
        s.patterns[0].cells[1][0] = c(50, 3);
        s.patterns[0].cells[2][0] = c(NOTE_OFF, 2);
        s.patterns[0].cells[3][0] = c(51, 1);
        let lane = |t: &str| crate::automation::Lane {
            target: t.into(),
            label: String::new(),
            points: vec![],
            enabled: true,
        };
        s.automation = vec![
            lane("instrument.2.cutoff"),
            lane("instrument.3.fx.1.mix"),
            lane("mixer.ch.2.volume"),
        ];
        assert_eq!(s.instrument_uses(2), 2);
        assert_eq!(s.delete_instrument(1), 2);
        let names: Vec<_> = s.instruments.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["a", "c"]);
        assert_eq!(s.instruments[1].fx.len(), 1, "c keeps its FX chain");
        let cells = &s.patterns[0].cells;
        assert_eq!(
            (cells[0][0].note, cells[0][0].instr, cells[0][0].fx),
            (0, 0, 0xF)
        );
        assert_eq!((cells[1][0].note, cells[1][0].instr), (50, 2));
        assert_eq!((cells[2][0].note, cells[2][0].instr), (NOTE_OFF, 0));
        assert_eq!((cells[3][0].note, cells[3][0].instr), (51, 1));
        let t: Vec<_> = s.automation.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(t, ["instrument.2.fx.1.mix", "mixer.ch.2.volume"]);
        // duplicate appends a copy with the chain
        assert_eq!(s.duplicate_instrument(1), Some(2));
        assert_eq!(s.instruments[2].name, "c copy");
        assert_eq!(s.instruments[2].fx.len(), 1);
        assert_eq!(s.delete_instrument(9), 0);
    }

    #[test]
    fn move_instrument_keeps_the_song_sounding_the_same() {
        let mut s = crate::fileio::demo_song();
        s.automation.push(crate::automation::Lane::new(
            crate::automation::Target::Cutoff(1),
            "cut".into(),
        ));
        let lane_t = s.automation.last().unwrap().target.clone();
        assert!(lane_t.starts_with("instrument.2."), "{lane_t}");
        let names: Vec<String> = s.instruments.iter().map(|i| i.name.clone()).collect();
        let uses: Vec<usize> = (1..=names.len() as u8)
            .map(|k| s.instrument_uses(k))
            .collect();
        let mut song = s.clone();
        song.order.truncate(2);
        song.automation.clear();
        let before = crate::engine::render_song(&song, 22050);
        let map = s.move_instrument(1, 4).unwrap();
        assert_eq!(map[1], 4);
        assert_eq!(s.instruments[4].name, names[1]);
        assert_eq!(s.instruments[1].name, names[2]);
        assert_eq!(s.instrument_uses(5), uses[1]);
        assert_eq!(s.instrument_uses(2), uses[2]);
        assert!(s
            .automation
            .last()
            .unwrap()
            .target
            .starts_with("instrument.5."));
        let mut song = s.clone();
        song.order.truncate(2);
        song.automation.clear();
        let after = crate::engine::render_song(&song, 22050);
        assert_eq!(before, after, "reordering does not change the sound");
        // and back
        s.move_instrument(4, 1).unwrap();
        assert_eq!(s.instruments[1].name, names[1]);
        assert!(s.move_instrument(0, 99).is_none());
    }

    #[test]
    fn replace_instrument_uses_in_song_or_one_block() {
        let mut s = crate::fileio::demo_song();
        let u1 = s.instrument_uses(1);
        let in_b0 = s.patterns[0]
            .cells
            .iter()
            .flatten()
            .filter(|c| c.instr == 1)
            .count();
        assert!(u1 > in_b0 && in_b0 > 0);
        let mut one = s.clone();
        assert_eq!(one.replace_instrument_uses(1, 3, Some(0)), in_b0);
        assert_eq!(one.instrument_uses(1), u1 - in_b0);
        assert_eq!(s.replace_instrument_uses(1, 3, None), u1);
        assert_eq!(s.instrument_uses(1), 0);
        assert_eq!(s.replace_instrument_uses(1, 200, None), 0, "no such slot");
    }
}

#[cfg(test)]
mod block_len_tests {
    use super::*;

    fn note(p: &mut Pattern, r: usize, ch: usize, n: u8) {
        p.cells[r][ch] = Cell {
            note: n,
            instr: 1,
            ..Default::default()
        };
    }

    #[test]
    fn row_labels_decimal_and_hex_with_adaptive_width() {
        assert_eq!(row_label(0, 64, false), "00");
        assert_eq!(row_label(13, 64, false), "13");
        assert_eq!(row_label(63, 64, false), "63");
        assert_eq!(row_label(99, 100, false), "99");
        assert_eq!(row_label(5, 128, false), "005");
        assert_eq!(row_label(127, 128, false), "127");
        assert_eq!(row_label(13, 64, true), "0D");
        assert_eq!(row_label(255, 256, true), "FF");
        assert_eq!(row_label(256, 300, true), "100");
        assert_eq!(row_label(1, 300, true), "001");
    }

    #[test]
    fn grow_adds_empty_rows_and_shrink_counts_what_it_cuts() {
        let mut p = Pattern::new("b", 64, 4);
        note(&mut p, 10, 0, 49);
        note(&mut p, 40, 2, 50);
        note(&mut p, 63, 3, 51);
        assert_eq!(p.cut_by(64), 0);
        assert_eq!(p.cut_by(32), 2);
        assert_eq!(p.cut_by(11), 2);
        assert_eq!(p.cut_by(10), 3);
        p.set_rows(96);
        assert_eq!((p.rows, p.cells.len()), (96, 96));
        assert!(p.cells[64..].iter().flatten().all(|c| c.is_empty()));
        assert_eq!(p.cells[95].len(), 4);
        p.set_rows(32);
        assert_eq!((p.rows, p.cells.len()), (32, 32));
        assert_eq!(p.cells[10][0].note, 49);
        p.set_rows(0);
        assert_eq!(p.rows, 1, "never empty");
    }

    #[test]
    fn stretch_and_squash_move_notes_and_squash_reports_losses() {
        let mut p = Pattern::new("b", 16, 2);
        note(&mut p, 0, 0, 49);
        note(&mut p, 3, 0, 50);
        note(&mut p, 15, 1, 51);
        p.stretch();
        assert_eq!(p.rows, 32);
        assert_eq!(
            (p.cells[0][0].note, p.cells[6][0].note, p.cells[30][1].note),
            (49, 50, 51)
        );
        assert_eq!(p.lost_by_squash(), 0);
        p.squash();
        assert_eq!(p.rows, 16);
        assert_eq!(
            (p.cells[0][0].note, p.cells[3][0].note, p.cells[15][1].note),
            (49, 50, 51)
        );
        // odd row onto a busy even row: lost, onto an empty one: moved up
        let mut q = Pattern::new("b", 4, 1);
        note(&mut q, 0, 0, 49);
        note(&mut q, 1, 0, 50);
        note(&mut q, 3, 0, 52);
        assert_eq!(q.lost_by_squash(), 1);
        q.squash();
        assert_eq!(q.rows, 2);
        assert_eq!((q.cells[0][0].note, q.cells[1][0].note), (49, 52));
        // odd lengths round up
        let mut o = Pattern::new("b", 5, 1);
        o.squash();
        assert_eq!(o.rows, 3);
    }

    #[test]
    fn resizing_a_derived_block_makes_it_ordinary() {
        let mut p = Pattern::new("b", 16, 1);
        p.derived = Some(crate::arrange::Derived {
            source: 0,
            ops: vec![],
            seed: 0,
            drums: 0,
        });
        p.cache = p.cells.clone();
        p.set_rows(32);
        assert!(p.derived.is_none() && p.cache.is_empty());
    }
}
