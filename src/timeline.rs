//! Timeline model (docs/TIMELINE.md, docs/DAW_TIMELINE_PLAN.md §1.3): tracks
//! (= mixer channels), clips placed in musical ticks, a tempo map with ramps,
//! a time-signature map and markers, plus the lossless converters between a
//! tracker song's order list and a timeline:
//!
//! * [`from_order`] derives the timeline of a tracker-mode song (one clip per
//!   order entry and non-empty block column, runs of one block merged into a
//!   looped clip). The Timeline view shows this; nothing is saved.
//! * [`consolidate`] turns any timeline back into blocks + an order list
//!   (segments between clip boundaries, identical segments shared). Order
//!   list -> timeline -> consolidate renders bit-exact (tests below).
//!
//! Format rules (future-proofing, docs/research/microsoft-midi.md):
//! every field is `serde(default)`; notes carry 16-bit velocity, release
//! velocity, per-note tuning, chance and optional expression lanes that map
//! 1:1 onto CLAP note expressions; automation values are 0..=1 floats,
//! quantised only at the destination; the innermost automation lane wins
//! (clip > track > song, [`innermost`]); every clip has a random seed and
//! anything random may depend only on `(seed, loop pass)` ([`ClipRng`]);
//! note data is pooled behind [`Shared`] (an `Arc`) so undo snapshots stay
//! cheap. The engine plays tracker-mode songs exactly as before.
// Parts of the format (note clips, expressions, ramps, seeds) are only read
// by later milestones (M2-M5); they are serialised and tested now.
#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::automation::Lane;
use crate::model::{Cell, Pattern, Song, MAX_CHANNELS, ROWS, ROWS_PER_BEAT};

/// Musical time: 960 ticks per quarter note (the same PPQ as Microsoft's
/// MIDI 2.0 sequencer). A tracker row (a 16th) is 240 ticks, so triplets
/// and 1/64 swing positions are exact.
pub type Tick = i64;
pub const PPQ: Tick = 960;
pub const ROW: Tick = PPQ / ROWS_PER_BEAT as Tick;
/// longest consolidated block (the pattern editor's maximum)
const MAX_SEG_ROWS: usize = Pattern::MAX_EDIT_ROWS;

pub const fn rows_to_ticks(rows: usize) -> Tick {
    rows as Tick * ROW
}

/// shared, copy-on-write pool entry: cloning a song (undo, save snapshots)
/// shares it; the first edit copies only this entry (`Arc::make_mut`)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shared<T>(pub Arc<T>);

impl<T> Shared<T> {
    pub fn new(v: T) -> Self {
        Self(Arc::new(v))
    }
    pub fn same(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }
}
impl<T: Clone> Shared<T> {
    pub fn make_mut(&mut self) -> &mut T {
        Arc::make_mut(&mut self.0)
    }
}
impl<T> std::ops::Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T: Serialize> Serialize for Shared<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.as_ref().serialize(s)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Shared<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(Shared::new)
    }
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}
fn one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Timeline {
    /// index == mixer channel (at most MAX_CHANNELS)
    pub tracks: Vec<Track>,
    /// sorted by (track, start) ([`Timeline::sort`]); ids are stable
    pub clips: Vec<Clip>,
    /// pooled piano-roll contents, shared by linked clips (`ClipKind::Notes`)
    pub notes: Vec<Shared<NoteClipData>>,
    /// tempo map; empty = the song's BPM throughout
    pub tempo: Vec<TempoPoint>,
    /// time-signature map; empty = 4/4. Ruler and snap only.
    pub signature: Vec<SigPoint>,
    pub markers: Vec<Marker>,
    pub loop_range: Option<(Tick, Tick)>,
    /// UI only (zoom, scroll)
    pub view: TimelineView,
    /// next clip id to hand out
    pub next_id: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    /// tracker clips and (M3b) note clips
    #[default]
    Note,
    /// audio clips (M4)
    Audio,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    /// mirrors `song.channel_names[i]`
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<u32>,
    #[serde(skip_serializing_if = "is_default")]
    pub kind: TrackKind,
    /// lane height in points (0 = the view's default)
    #[serde(skip_serializing_if = "is_default")]
    pub height: f32,
    #[serde(skip_serializing_if = "is_default")]
    pub folded: bool,
    #[serde(skip_serializing_if = "is_default")]
    pub armed: bool,
    /// track automation (positions in rows from the song start, values 0..=1)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<Lane>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClipKind {
    /// tracker data: column `col` of block `pattern` (row r = tick r*ROW)
    Block { pattern: usize, col: u8 },
    /// polyphonic notes in `Timeline::notes[data]` (piano roll, M3b)
    Notes { data: u32 },
}

impl Default for ClipKind {
    fn default() -> Self {
        ClipKind::Block { pattern: 0, col: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clip {
    pub id: u32,
    pub track: u16,
    /// placement on the timeline
    pub start: Tick,
    pub len: Tick,
    /// where in the source playback starts (left trim)
    #[serde(skip_serializing_if = "is_default")]
    pub offset: Tick,
    /// Some: the source loops every `loop_len` ticks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_len: Option<Tick>,
    pub kind: ClipKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<u32>,
    #[serde(skip_serializing_if = "is_default")]
    pub muted: bool,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub gain: f32,
    #[serde(skip_serializing_if = "is_default")]
    pub fade_in: Tick,
    #[serde(skip_serializing_if = "is_default")]
    pub fade_out: Tick,
    /// random seed: chance, humanise and generators draw only from
    /// `ClipRng::new(seed, pass)`, so export and seeks are bit-exact
    pub seed: u32,
    /// clip automation: positions in rows relative to the clip start,
    /// values 0..=1; inside the clip it overrides track and song lanes
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<Lane>,
}

impl Default for Clip {
    fn default() -> Self {
        Self {
            id: 0,
            track: 0,
            start: 0,
            len: rows_to_ticks(ROWS),
            offset: 0,
            loop_len: None,
            kind: ClipKind::default(),
            name: None,
            colour: None,
            muted: false,
            gain: 1.0,
            fade_in: 0,
            fade_out: 0,
            seed: 0,
            auto: vec![],
        }
    }
}

impl Clip {
    pub fn end(&self) -> Tick {
        self.start + self.len
    }
    /// source position (ticks into the block / note data) at timeline tick `t`
    pub fn source_at(&self, t: Tick) -> Tick {
        let s = self.offset + (t - self.start);
        match self.loop_len {
            Some(l) if l > 0 => s.rem_euclid(l),
            _ => s,
        }
    }
    /// loop pass at timeline tick `t` (0 = first time through the source)
    pub fn pass_at(&self, t: Tick) -> u32 {
        let s = self.offset + (t - self.start).max(0);
        match self.loop_len {
            Some(l) if l > 0 => (s / l).clamp(0, u32::MAX as Tick) as u32,
            _ => 0,
        }
    }
    /// timeline ticks inside the clip where the source wraps to its start
    pub fn loop_points(&self) -> impl Iterator<Item = Tick> + '_ {
        let l = self.loop_len.filter(|l| *l > 0).unwrap_or(0);
        let first = if l > 0 {
            // first t > start with (offset + t - start) % l == 0
            let r = self.offset.rem_euclid(l);
            self.start + if r == 0 { l } else { l - r }
        } else {
            Tick::MAX
        };
        let end = self.end();
        (0..)
            .map(move |k: Tick| first.saturating_add(k.saturating_mul(l.max(1))))
            .take_while(move |&t| l > 0 && t < end)
    }
    pub fn block(&self) -> Option<(usize, usize)> {
        match self.kind {
            ClipKind::Block { pattern, col } => Some((pattern, col as usize)),
            ClipKind::Notes { .. } => None,
        }
    }
}

/// Deterministic per-clip randomness: the only RNG timeline playback may use.
/// The same (seed, pass) always gives the same stream (SplitMix64).
#[derive(Clone, Copy, Debug)]
pub struct ClipRng(u64);

impl ClipRng {
    pub fn new(seed: u32, pass: u32) -> Self {
        Self(((seed as u64) << 32 | pass as u64) ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) as u32
    }
    /// 0..1
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// a stable seed for a new clip (from its id; never from the clock)
pub fn seed_for(id: u32) -> u32 {
    ClipRng::new(id, 0x5EED).next_u32()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteClipData {
    pub name: String,
    pub len: Tick,
    pub notes: Vec<Note>,
}

/// One piano-roll note. Velocity is 16-bit (MIDI 2.0); tracker volumes and
/// MIDI 1.0 velocities convert with [`vel_from_7`] / [`vel_to_7`]
/// (MIDI M2-115 bit scaling, exactly reversible).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    /// relative to the note data start
    pub start: Tick,
    pub len: Tick,
    /// tracker note number (1 = C-0, as `Cell::note`)
    pub key: u8,
    pub vel: u16,
    /// release velocity (0 = unset)
    #[serde(skip_serializing_if = "is_default")]
    pub rel_vel: u16,
    /// instrument slot (1-based, 0 = the track's current one)
    #[serde(skip_serializing_if = "is_default")]
    pub instr: u8,
    /// optional tracker command (effect, parameter)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fx: Option<(u8, u8)>,
    /// per-note detune in semitones (CLAP tuning expression)
    #[serde(skip_serializing_if = "is_default")]
    pub tune: f32,
    /// percent chance to play, 0 = always (drawn from the clip's ClipRng)
    #[serde(skip_serializing_if = "is_default")]
    pub chance: u8,
    /// per-note expression lanes (empty for almost every note)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expr: Vec<NoteExpr>,
}

impl Default for Note {
    fn default() -> Self {
        Self {
            start: 0,
            len: ROW,
            key: 49,
            vel: 0xFFFF,
            rel_vel: 0,
            instr: 0,
            fx: None,
            tune: 0.0,
            chance: 0,
            expr: vec![],
        }
    }
}

/// Per-note expressions, numbered as CLAP's `CLAP_NOTE_EXPRESSION_*`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExprKind {
    #[default]
    Volume,
    Pan,
    /// semitones (MIDI 2.0 per-note pitch)
    Tuning,
    Vibrato,
    Expression,
    /// CC74 / timbre
    Brightness,
    Pressure,
}

impl ExprKind {
    pub const ALL: [ExprKind; 7] = [
        ExprKind::Volume,
        ExprKind::Pan,
        ExprKind::Tuning,
        ExprKind::Vibrato,
        ExprKind::Expression,
        ExprKind::Brightness,
        ExprKind::Pressure,
    ];
    /// the matching `clap_note_expression` id
    pub fn clap_id(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteExpr {
    pub kind: ExprKind,
    /// (tick relative to the note start, value: 0..=1, or semitones for Tuning)
    pub points: Vec<(Tick, f32)>,
}

/// MIDI 2.0 M2-115 "min-centre-max" upscaling of a 7-bit value to 16 bits.
pub fn vel_from_7(v: u8) -> u16 {
    let v = v.min(127) as u32;
    let mut out = v << 9;
    if v <= 64 {
        return out as u16;
    }
    let mut rep = (v & 0x3F) << 3;
    while rep != 0 {
        out |= rep;
        rep >>= 6;
    }
    out as u16
}

/// 16-bit velocity back to 7 bits (M2-115 downscaling: a plain shift)
pub fn vel_to_7(v: u16) -> u8 {
    (v >> 9) as u8
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoPoint {
    pub tick: Tick,
    pub bpm: f32,
    /// ramp linearly (in BPM) to the next point instead of jumping there
    #[serde(default, skip_serializing_if = "is_default")]
    pub ramp: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigPoint {
    /// should sit on a bar line of the previous signature
    pub tick: Tick,
    pub num: u8,
    pub den: u8,
}

impl SigPoint {
    pub fn bar_ticks(&self) -> Tick {
        let den = self.den.clamp(1, 64) as Tick;
        self.num.max(1) as Tick * PPQ * 4 / den
    }
    pub fn beat_ticks(&self) -> Tick {
        PPQ * 4 / self.den.clamp(1, 64) as Tick
    }
}

const FOUR_FOUR: SigPoint = SigPoint {
    tick: 0,
    num: 4,
    den: 4,
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Marker {
    pub name: String,
    pub tick: Tick,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<u32>,
    /// Some: a track tag ("filter opens here") instead of a song marker
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineView {
    /// pixels per beat (0 = fit the song)
    pub zoom: f32,
    /// first visible tick
    pub scroll: Tick,
    /// vertical scroll in points
    pub scroll_y: f32,
    /// default track height in points
    pub track_h: f32,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            zoom: 0.0,
            scroll: 0,
            scroll_y: 0.0,
            track_h: 46.0,
        }
    }
}

/// bar.beat position (1-based) plus the ticks into the beat
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarPos {
    pub bar: i64,
    pub beat: i64,
    pub tick: Tick,
}

impl Timeline {
    /// sort clips by (track, start, id)
    pub fn sort(&mut self) {
        self.clips.sort_by_key(|c| (c.track, c.start, c.id));
    }

    /// end of the last clip (0 when empty)
    pub fn end(&self) -> Tick {
        self.clips.iter().map(|c| c.end()).max().unwrap_or(0)
    }

    pub fn new_id(&mut self) -> u32 {
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Tempo at `t` (ramps interpolate linearly in BPM).
    pub fn bpm_at(&self, t: Tick, song_bpm: f32) -> f32 {
        let k = self.tempo.partition_point(|p| p.tick <= t);
        if k == 0 {
            return self.tempo.first().map_or(song_bpm, |p| p.bpm);
        }
        let a = self.tempo[k - 1];
        match self.tempo.get(k) {
            Some(b) if a.ramp && b.tick > a.tick => {
                let f = (t - a.tick) as f64 / (b.tick - a.tick) as f64;
                (a.bpm as f64 + (b.bpm - a.bpm) as f64 * f) as f32
            }
            _ => a.bpm,
        }
    }

    /// Seconds from tick 0 to `t` through the tempo map.
    pub fn seconds_at(&self, t: Tick, song_bpm: f32) -> f64 {
        let secs = |ticks: f64, bpm: f64| ticks * 60.0 / (PPQ as f64 * bpm.max(1.0));
        let first_bpm = self.tempo.first().map_or(song_bpm, |p| p.bpm) as f64;
        let Some(first) = self.tempo.first() else {
            return secs(t as f64, song_bpm as f64);
        };
        if t <= first.tick {
            return secs(t as f64, first_bpm);
        }
        let mut s = secs(first.tick as f64, first_bpm);
        for (k, a) in self.tempo.iter().enumerate() {
            if a.tick >= t {
                break;
            }
            let next = self.tempo.get(k + 1);
            let seg_end = next.map_or(t, |b| b.tick.min(t));
            let d = (seg_end - a.tick) as f64;
            match next {
                Some(b) if a.ramp && b.tick > a.tick && (b.bpm - a.bpm).abs() > 1e-6 => {
                    // bpm(x) = b0 + (b1-b0) x/D; time = ∫ 60/(PPQ bpm) dx
                    let span = (b.tick - a.tick) as f64;
                    let k = (b.bpm - a.bpm) as f64 / span;
                    let b0 = a.bpm as f64;
                    let b_end = b0 + k * d;
                    s += 60.0 / (PPQ as f64 * k) * (b_end.max(1.0) / b0.max(1.0)).ln();
                }
                _ => s += secs(d, a.bpm as f64),
            }
        }
        s
    }

    fn sigs(&self) -> &[SigPoint] {
        if self.signature.is_empty() {
            std::slice::from_ref(&FOUR_FOUR)
        } else {
            &self.signature
        }
    }

    /// signature in force at `t`
    pub fn sig_at(&self, t: Tick) -> SigPoint {
        let s = self.sigs();
        let k = s.partition_point(|p| p.tick <= t);
        s[k.saturating_sub(1)]
    }

    /// (tick, bar number from 1) of every bar line in [t0, t1)
    pub fn bars_in(&self, t0: Tick, t1: Tick) -> Vec<(Tick, i64)> {
        let mut out = vec![];
        let s = self.sigs();
        let mut bar = 1i64;
        for (k, p) in s.iter().enumerate() {
            // the first signature counts from tick 0
            let start = if k == 0 { 0 } else { p.tick };
            let end = s.get(k + 1).map_or(Tick::MAX, |n| n.tick);
            let bt = p.bar_ticks().max(1);
            let n_bars = if end == Tick::MAX {
                Tick::MAX
            } else {
                (end - start + bt - 1) / bt
            };
            if end <= t0 {
                bar += n_bars;
                continue;
            }
            let first = ((t0 - start).max(0)) / bt;
            let mut i = first;
            while i < n_bars {
                let t = start + i * bt;
                if t >= t1 || t >= end {
                    break;
                }
                if t >= t0 {
                    out.push((t, bar + i));
                }
                i += 1;
            }
            if end >= t1 {
                break;
            }
            bar += n_bars;
        }
        out
    }

    /// bar.beat of `t` (1-based)
    pub fn bar_pos(&self, t: Tick) -> BarPos {
        let s = self.sigs();
        let mut bar = 1i64;
        for (k, p) in s.iter().enumerate() {
            let end = s.get(k + 1).map_or(Tick::MAX, |n| n.tick);
            let bt = p.bar_ticks().max(1);
            let start = if k == 0 { 0 } else { p.tick };
            if t < end || k + 1 == s.len() {
                let d = (t - start).max(0);
                let bi = d / bt;
                let rem = d - bi * bt;
                let beat = p.beat_ticks().max(1);
                return BarPos {
                    bar: bar + bi,
                    beat: rem / beat + 1,
                    tick: rem % beat,
                };
            }
            bar += (end - start + bt - 1) / bt;
        }
        BarPos {
            bar,
            beat: 1,
            tick: 0,
        }
    }

    /// per-track clip ranges for fast visible-range queries
    pub fn index(&self) -> TrackIndex {
        let n = self.tracks.len().max(
            self.clips
                .iter()
                .map(|c| c.track as usize + 1)
                .max()
                .unwrap_or(0),
        );
        let mut ranges = vec![0..0; n];
        let mut max_len = vec![0; n];
        let mut i = 0;
        while i < self.clips.len() {
            let t = self.clips[i].track as usize;
            let j = i + self.clips[i..].partition_point(|c| c.track as usize == t);
            if t < n {
                ranges[t] = i..j;
                max_len[t] = self.clips[i..j].iter().map(|c| c.len).max().unwrap_or(0);
            }
            i = j.max(i + 1);
        }
        TrackIndex { ranges, max_len }
    }
}

/// The value of one target where several automation layers overlap:
/// the innermost lane wins (clip, then track, then song).
pub fn innermost(clip: Option<f32>, track: Option<f32>, song: Option<f32>) -> Option<f32> {
    clip.or(track).or(song)
}

/// Clips per track (indices into `Timeline::clips`, which must be sorted).
#[derive(Clone, Debug, Default)]
pub struct TrackIndex {
    pub ranges: Vec<std::ops::Range<usize>>,
    pub max_len: Vec<Tick>,
}

impl TrackIndex {
    /// indices of clips on `track` overlapping [t0, t1): O(log n + visible)
    pub fn visible<'a>(
        &self,
        tl: &'a Timeline,
        track: usize,
        t0: Tick,
        t1: Tick,
    ) -> impl Iterator<Item = usize> + 'a {
        let r = self.ranges.get(track).cloned().unwrap_or(0..0);
        let clips = &tl.clips[r.clone()];
        let lo = t0 - self.max_len.get(track).copied().unwrap_or(0);
        let a = clips.partition_point(|c| c.start <= lo);
        let b = clips.partition_point(|c| c.start < t1).max(a);
        (r.start + a..r.start + b).filter(move |&i| tl.clips[i].end() > t0)
    }
}

// ---------------------------------------------------------------------------
// order list <-> timeline

/// Rows of `p` that the tracker actually plays: up to and including the
/// first row with a `0F00` pattern break (in any channel < `channels`).
pub fn effective_rows(p: &Pattern, channels: usize) -> usize {
    let rows = p.rows.max(1);
    for (r, row) in p.cells.iter().enumerate().take(rows) {
        if row
            .iter()
            .take(channels)
            .any(|c| c.fx == 0x0F && c.fxp == 0)
        {
            return r + 1;
        }
    }
    rows
}

/// Where each order-list entry sits on the timeline (derived timelines only).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrderMap {
    /// start tick of each order entry
    pub starts: Vec<Tick>,
    /// effective rows of each entry (a pattern break shortens it)
    pub lens: Vec<usize>,
    /// full block rows of each entry (what the engine's `song_pos` counts)
    pub full: Vec<usize>,
}

impl OrderMap {
    pub fn end(&self) -> Tick {
        self.starts
            .last()
            .map_or(0, |s| s + rows_to_ticks(*self.lens.last().unwrap_or(&0)))
    }
    /// (order entry, row) at tick `t` (clamped to the song)
    pub fn entry_at(&self, t: Tick) -> (usize, usize) {
        if self.starts.is_empty() {
            return (0, 0);
        }
        let k = self.starts.partition_point(|&s| s <= t).saturating_sub(1);
        let row = ((t - self.starts[k]).max(0) / ROW) as usize;
        (k, row.min(self.lens[k].saturating_sub(1)))
    }
    pub fn tick_of(&self, entry: usize, row: usize) -> Tick {
        self.starts.get(entry).copied().unwrap_or(0) + rows_to_ticks(row)
    }
    /// Timeline tick of the engine's position: `order_pos` plus `song_pos`
    /// (lines from the song start, counting full block lengths).
    pub fn tick_of_song_pos(&self, order_pos: usize, song_pos: f32) -> f64 {
        let Some(&st) = self.starts.get(order_pos) else {
            return 0.0;
        };
        let full_before: usize = self.full[..order_pos].iter().sum();
        let into = (song_pos as f64 - full_before as f64).clamp(0.0, self.lens[order_pos] as f64);
        st as f64 + into * ROW as f64
    }
}

/// The timeline of a tracker-mode song: track i = channel i, one clip per
/// order entry and non-empty column (empty columns hold the channel's state,
/// exactly like a gap), runs of one block on a track merged into one looped
/// clip. Block roles become markers.
pub fn from_order(song: &Song) -> (Timeline, OrderMap) {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    let mut tl = Timeline {
        tracks: (0..nch)
            .map(|i| Track {
                name: song.channel_names.get(i).cloned().unwrap_or_default(),
                ..Default::default()
            })
            .collect(),
        tempo: vec![TempoPoint {
            tick: 0,
            bpm: song.bpm,
            ramp: false,
        }],
        next_id: 1,
        ..Default::default()
    };
    // which (block, column) has notes or commands in its played rows
    let eff: Vec<usize> = song
        .patterns
        .iter()
        .map(|p| effective_rows(p, nch))
        .collect();
    let used: Vec<Vec<bool>> = song
        .patterns
        .iter()
        .zip(&eff)
        .map(|(p, &e)| {
            let mut u = vec![false; nch];
            for row in p.cells.iter().take(e) {
                for (c, cell) in row.iter().enumerate().take(nch) {
                    u[c] |= !cell.is_empty();
                }
            }
            u
        })
        .collect();
    let mut map = OrderMap::default();
    let mut t: Tick = 0;
    let mut last: Vec<Option<usize>> = vec![None; nch];
    let mut prev_role = None;
    for &b in &song.order {
        let Some(p) = song.patterns.get(b) else {
            continue;
        };
        let rows = eff[b];
        let len = rows_to_ticks(rows);
        map.starts.push(t);
        map.lens.push(rows);
        map.full.push(p.rows);
        if p.role.is_some() && p.role != prev_role {
            tl.markers.push(Marker {
                name: p.role.map(|r| r.name()).unwrap_or_default().into(),
                tick: t,
                ..Default::default()
            });
        }
        prev_role = p.role;
        for (c, slot) in last.iter_mut().enumerate() {
            if !used[b][c] {
                *slot = None;
                continue;
            }
            let kind = ClipKind::Block {
                pattern: b,
                col: c as u8,
            };
            if let Some(k) = *slot {
                let cl = &mut tl.clips[k];
                let whole =
                    cl.offset == 0 && cl.loop_len.unwrap_or(cl.len) == len && cl.len % len == 0;
                if cl.kind == kind && cl.end() == t && whole {
                    cl.len += len;
                    cl.loop_len = Some(len);
                    continue;
                }
            }
            let id = tl.new_id();
            tl.clips.push(Clip {
                id,
                track: c as u16,
                start: t,
                len,
                kind,
                seed: seed_for(id),
                ..Default::default()
            });
            *slot = Some(tl.clips.len() - 1);
        }
        t += len;
    }
    tl.sort();
    (tl, map)
}

/// Turn a timeline back into blocks and an order list (for tracker export,
/// MOD/MMD and "back to tracker mode"). Every span between two clip
/// boundaries (starts, ends, loop wraps, at most 256 rows) becomes a
/// segment; segments that play the same source rows on every track share
/// one block. Where clips overlap on a track, the later one plays. Block
/// clips must sit on the row grid. Returns (blocks, order).
/// a consolidated segment: rows + per-track (block, col, first row) sources
type SegKey = (usize, Vec<Option<(usize, usize, usize)>>);

pub fn consolidate(song: &Song, tl: &Timeline) -> Result<(Vec<Pattern>, Vec<usize>), String> {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    let mut clips: Vec<&Clip> = vec![];
    for c in &tl.clips {
        let Some((pat, col)) = c.block() else {
            return Err(format!(
                "clip {} holds piano-roll notes: converting those to blocks is not supported yet",
                c.id
            ));
        };
        if c.len <= 0 || c.muted || c.track as usize >= nch {
            continue;
        }
        if pat >= song.patterns.len() || col >= MAX_CHANNELS {
            return Err(format!("clip {} refers to a missing block or column", c.id));
        }
        let on_grid = [c.start, c.len, c.offset, c.loop_len.unwrap_or(ROW)]
            .iter()
            .all(|v| v % ROW == 0);
        if !on_grid || c.start < 0 || c.loop_len.is_some_and(|l| l <= 0) {
            return Err(format!("clip {} is not on the row grid", c.id));
        }
        clips.push(c);
    }
    clips.sort_by_key(|c| (c.track, c.start, c.id));
    let end = clips.iter().map(|c| c.end()).max().unwrap_or(0);
    let mut cuts: BTreeSet<Tick> = BTreeSet::new();
    cuts.insert(0);
    cuts.insert(end.max(rows_to_ticks(ROWS)));
    for c in &clips {
        cuts.insert(c.start);
        cuts.insert(c.end());
        cuts.extend(c.loop_points());
    }
    let cuts: Vec<Tick> = cuts.into_iter().collect();
    let mut per: Vec<Vec<&Clip>> = vec![vec![]; nch];
    for c in &clips {
        per[c.track as usize].push(c);
    }
    let max_len: Vec<Tick> = per
        .iter()
        .map(|v| v.iter().map(|c| c.len).max().unwrap_or(0))
        .collect();
    let eff: Vec<usize> = song
        .patterns
        .iter()
        .map(|p| effective_rows(p, nch))
        .collect();
    let mut blocks: Vec<Pattern> = vec![];
    let mut seen: HashMap<SegKey, usize> = HashMap::new();
    let mut order = vec![];
    for w in cuts.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        while a < b {
            let rows = (((b - a) / ROW) as usize).min(MAX_SEG_ROWS);
            // what each track plays from tick a: (block, column, first source row)
            let key: Vec<Option<(usize, usize, usize)>> = per
                .iter()
                .enumerate()
                .map(|(tr, v)| {
                    let k = v.partition_point(|c| c.start <= a);
                    v[..k]
                        .iter()
                        .rev()
                        .take_while(|c| c.start + max_len[tr] > a)
                        .find(|c| a < c.end())
                        .and_then(|c| {
                            let (p, col) = c.block()?;
                            Some((p, col, (c.source_at(a) / ROW) as usize))
                        })
                })
                .collect();
            let k = (rows, key);
            let idx = match seen.get(&k) {
                Some(&i) => i,
                None => {
                    let mut p = Pattern::new("", rows, nch);
                    let srcs: BTreeSet<(usize, usize)> =
                        k.1.iter().flatten().map(|s| (s.0, s.2)).collect();
                    for (tr, src) in k.1.iter().enumerate() {
                        let Some((sp, col, s0)) = *src else { continue };
                        let from = &song.patterns[sp];
                        for r in 0..rows {
                            let sr = s0 + r;
                            if sr >= eff[sp] {
                                break;
                            }
                            let mut cell = from
                                .cells
                                .get(sr)
                                .and_then(|row| row.get(col))
                                .copied()
                                .unwrap_or_default();
                            // a break only ends the block on the segment's last row
                            if cell.fx == 0x0F && cell.fxp == 0 && r + 1 < rows {
                                cell.fx = 0;
                            }
                            p.cells[r][tr] = cell;
                        }
                    }
                    p.name = match srcs.iter().next() {
                        None => "Gap".into(),
                        Some(&(sp, s0)) if srcs.len() == 1 => {
                            let src = &song.patterns[sp];
                            if s0 == 0 && rows == eff[sp] {
                                p.role = src.role;
                                src.name.clone()
                            } else {
                                format!("{} {}-{}", src.name, s0, s0 + rows - 1)
                            }
                        }
                        _ => format!("Mix {:02}", blocks.len()),
                    };
                    blocks.push(p);
                    if blocks.len() > crate::fileio::MAX_BLOCKS {
                        return Err("the timeline needs too many blocks".into());
                    }
                    seen.insert(k, blocks.len() - 1);
                    blocks.len() - 1
                }
            };
            order.push(idx);
            a += rows_to_ticks(rows);
        }
    }
    Ok((blocks, order))
}

/// `song` with its blocks and order list replaced by the consolidated timeline
pub fn consolidated_song(song: &Song, tl: &Timeline) -> Result<Song, String> {
    let (patterns, order) = consolidate(song, tl)?;
    let mut s = song.clone();
    s.patterns = patterns;
    s.order = order;
    Ok(s)
}

/// The rows the tracker plays, in order (breaks applied, `0F00` cells kept
/// only on a block's last played row). Two songs with the same stream play
/// the same notes and commands.
pub fn row_stream(song: &Song) -> Vec<Vec<Cell>> {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    let mut out = vec![];
    for &b in &song.order {
        let Some(p) = song.patterns.get(b) else {
            continue;
        };
        let e = effective_rows(p, nch);
        for r in 0..e {
            let mut row: Vec<Cell> = (0..nch)
                .map(|c| {
                    p.cells
                        .get(r)
                        .and_then(|x| x.get(c))
                        .copied()
                        .unwrap_or_default()
                })
                .collect();
            if r + 1 < e {
                for c in &mut row {
                    if c.fx == 0x0F && c.fxp == 0 {
                        c.fx = 0;
                    }
                }
            }
            out.push(row);
        }
    }
    out
}

/// timeline.toml text (written by timeline-mode songs from M2 on)
pub fn to_toml(tl: &Timeline) -> Result<String, String> {
    toml::to_string(tl).map_err(|e| e.to_string())
}

pub fn from_toml(text: &str) -> Result<Timeline, String> {
    let mut tl: Timeline = toml::from_str(text).map_err(|e| e.to_string())?;
    tl.tracks.truncate(MAX_CHANNELS);
    tl.sort();
    tl.tempo.retain(|p| p.bpm.is_finite());
    for p in &mut tl.tempo {
        p.bpm = p.bpm.clamp(20.0, 999.0);
    }
    tl.tempo.sort_by_key(|p| p.tick);
    tl.signature.sort_by_key(|p| p.tick);
    let max_id = tl.clips.iter().map(|c| c.id).max().unwrap_or(0);
    tl.next_id = tl.next_id.max(max_id + 1);
    Ok(tl)
}

#[cfg(test)]
mod tests;
