//! Song-wide undo/redo.
//!
//! Each history entry is a full *logical* copy of the song, but items are
//! shared between entries through `Arc`s: an edit that touches one block stores
//! one new block, and sample PCM is shared copy-on-write (`SampleData`).
//! New entries are found by comparing the live song with the current entry
//! (`History::commit`), so every kind of edit is covered (patterns, order,
//! instruments + their FX chains, mixer, tempo/LPB, channels, roles, lanes,
//! fill-to-length accepts) without each edit site having to describe itself.
//! Slider drags are committed when the mouse is released (one step), and runs
//! of parameter-only changes close together in time are merged.
use crate::automation::Lane;
use crate::model::{Instrument, Mixer, Pattern, Song, SongInfo};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// most steps kept
pub const MAX_STEPS: usize = 1000;
/// approximate memory cap for the newly stored items of all steps
pub const MAX_BYTES: usize = 256 * 1024 * 1024;
/// parameter-only changes closer together than this merge into one step
pub const COALESCE: Duration = Duration::from_millis(800);

/// small song fields, compared and stored as a whole
#[derive(Clone, Debug, PartialEq)]
struct Meta {
    title: String,
    bpm: f32,
    speed: u32,
    lines_per_beat: u32,
    channels: usize,
    channel_names: Vec<String>,
    order: Vec<usize>,
    mixer: Mixer,
    info: SongInfo,
}

impl Meta {
    fn of(s: &Song) -> Self {
        Meta {
            title: s.title.clone(),
            bpm: s.bpm,
            speed: s.speed,
            lines_per_beat: s.lines_per_beat,
            channels: s.channels,
            channel_names: s.channel_names.clone(),
            order: s.order.clone(),
            mixer: s.mixer.clone(),
            info: s.info.clone(),
        }
    }
    fn same(&self, s: &Song) -> bool {
        self.title == s.title
            && self.bpm.to_bits() == s.bpm.to_bits()
            && self.speed == s.speed
            && self.lines_per_beat == s.lines_per_beat
            && self.channels == s.channels
            && self.channel_names == s.channel_names
            && self.order == s.order
            && self.mixer == s.mixer
            && self.info == s.info
    }
    fn bytes(&self) -> usize {
        256 + self.order.len() * 8 + self.mixer.channels.len() * 32
    }
    /// bit mask of the fields that differ from the live song (used so that
    /// e.g. a tempo change never merges with a mute click)
    fn diff_mask(&self, s: &Song) -> Mask {
        let mut m: Mask = [0; 4];
        let mut set = |bit: u32, differs: bool| {
            if differs {
                let b = bit.min(255);
                m[(b / 64) as usize] |= 1 << (b % 64);
            }
        };
        set(0, self.title != s.title);
        set(1, self.bpm.to_bits() != s.bpm.to_bits());
        set(2, self.speed != s.speed);
        set(3, self.lines_per_beat != s.lines_per_beat);
        set(
            4,
            self.channels != s.channels || self.channel_names != s.channel_names,
        );
        set(5, self.order != s.order);
        set(6, self.info != s.info);
        set(7, self.mixer.master.to_bits() != s.mixer.master.to_bits());
        set(8, self.mixer.returns != s.mixer.returns);
        set(9, self.mixer.channels.len() != s.mixer.channels.len());
        for (c, (a, b)) in self
            .mixer
            .channels
            .iter()
            .zip(&s.mixer.channels)
            .enumerate()
        {
            let base = 12 + c as u32 * 5;
            set(base, a.volume.to_bits() != b.volume.to_bits());
            set(base + 1, a.pan.to_bits() != b.pan.to_bits());
            set(base + 2, a.send != b.send);
            set(base + 3, a.mute != b.mute);
            set(base + 4, a.solo != b.solo);
        }
        m
    }
    /// on/off switches (mute, solo, channel count, order...) are never merged
    fn is_toggle(mask: Mask) -> bool {
        let mut t: Mask = [(1 << 4) | (1 << 5) | (1 << 9), 0, 0, 0];
        for c in 0..crate::model::MAX_CHANNELS {
            for bit in [12 + c * 5 + 3, 12 + c * 5 + 4] {
                t[bit / 64] |= 1 << (bit % 64);
            }
        }
        mask.iter().zip(&t).any(|(a, b)| a & b != 0)
    }
}

/// One point in the history.
#[derive(Clone, Debug)]
pub struct State {
    meta: Meta,
    patterns: Vec<Arc<Pattern>>,
    instruments: Vec<Arc<Instrument>>,
    lanes: Vec<Arc<Lane>>,
    /// approximate size of what this state added (not shared with its predecessor)
    bytes: usize,
}

/// 256-bit field mask: 12 song fields + 5 bits per mixer channel (32 max)
type Mask = [u64; 4];

/// What a commit changed (used to decide whether steps may be merged).
#[derive(Clone, Debug, PartialEq)]
pub struct Sig {
    /// changed song fields (see `Meta::diff_mask`)
    meta: Mask,
    patterns: bool,
    lanes: bool,
    /// changed instrument slots (same count before and after)
    instruments: Vec<usize>,
    instruments_resized: bool,
}

impl Sig {
    /// only "parameter" changes: knob/slider-like edits that may coalesce
    fn mergeable(&self) -> bool {
        !self.patterns && !self.lanes && !self.instruments_resized && !Meta::is_toggle(self.meta)
    }
}

fn pattern_bytes(p: &Pattern) -> usize {
    96 + p.name.len()
        + p.cells.len() * (24 + p.cells.first().map_or(0, |r| r.len()) * 5)
        + p.cache.len() * (24 + p.cache.first().map_or(0, |r| r.len()) * 5)
}

fn instrument_bytes(i: &Instrument, prev: Option<&Instrument>) -> usize {
    use crate::model::InstrumentKind::Sample;
    let mut b = 512 + i.fx.len() * 128;
    if let Sample { data, .. } = &i.kind {
        let shared = matches!(prev.map(|p| &p.kind), Some(Sample { data: d, .. }) if d.same(data));
        if !shared {
            b += data.len() * 4;
        }
    }
    b
}

fn lane_bytes(l: &Lane) -> usize {
    128 + l.points.len() * std::mem::size_of::<crate::automation::Point>()
}

/// Reuse `prev` items that equal the live ones (same index, or one off for an
/// insert/delete just before); clone the rest. Returns (items, new bytes, changed).
fn share<T: Clone + PartialEq>(
    live: &[T],
    prev: &[Arc<T>],
    mut size: impl FnMut(&T, Option<&T>) -> usize,
) -> (Vec<Arc<T>>, usize, Vec<usize>) {
    let mut out = Vec::with_capacity(live.len());
    let mut bytes = 0;
    let mut changed = vec![];
    for (i, x) in live.iter().enumerate() {
        let hit = [Some(i), i.checked_sub(1), Some(i + 1)]
            .into_iter()
            .flatten()
            .filter_map(|j| prev.get(j))
            .find(|p| ***p == *x);
        match hit {
            Some(p) => out.push(p.clone()),
            None => {
                bytes += size(x, prev.get(i).map(|a| &**a));
                changed.push(i);
                out.push(Arc::new(x.clone()));
            }
        }
    }
    (out, bytes, changed)
}

fn same_items<T: PartialEq>(live: &[T], prev: &[Arc<T>]) -> bool {
    live.len() == prev.len() && live.iter().zip(prev).all(|(a, b)| *a == **b)
}

impl State {
    pub fn capture(song: &Song) -> State {
        let empty = State {
            meta: Meta::of(song),
            patterns: vec![],
            instruments: vec![],
            lanes: vec![],
            bytes: 0,
        };
        empty.next(song).map(|(s, _)| s).unwrap_or(empty)
    }

    /// cheap "nothing changed" test (no allocation)
    pub fn matches(&self, song: &Song) -> bool {
        self.meta.same(song)
            && same_items(&song.patterns, &self.patterns)
            && same_items(&song.instruments, &self.instruments)
            && same_items(&song.automation, &self.lanes)
    }

    /// the state for `song`, sharing unchanged items with `self`; None if equal
    fn next(&self, song: &Song) -> Option<(State, Sig)> {
        if self.matches(song) && !self.patterns.is_empty() {
            return None;
        }
        let meta_same = self.meta.same(song);
        let (patterns, pb, pc) = share(&song.patterns, &self.patterns, |p, _| pattern_bytes(p));
        let (instruments, ib, ic) = share(&song.instruments, &self.instruments, instrument_bytes);
        let (lanes, lb, lc) = share(&song.automation, &self.lanes, |l, _| lane_bytes(l));
        let meta = if meta_same {
            self.meta.clone()
        } else {
            Meta::of(song)
        };
        let sig = Sig {
            meta: if meta_same {
                [0; 4]
            } else {
                self.meta.diff_mask(song)
            },
            patterns: !pc.is_empty() || patterns.len() != self.patterns.len(),
            lanes: !lc.is_empty() || lanes.len() != self.lanes.len(),
            instruments_resized: instruments.len() != self.instruments.len(),
            instruments: ic,
        };
        let bytes = pb + ib + lb + if meta_same { 0 } else { meta.bytes() };
        Some((
            State {
                meta,
                patterns,
                instruments,
                lanes,
                bytes,
            },
            sig,
        ))
    }

    /// write this state into the song (UI-only fields such as zoom are kept)
    pub fn apply(&self, s: &mut Song) {
        let m = &self.meta;
        s.title.clone_from(&m.title);
        s.bpm = m.bpm;
        s.speed = m.speed;
        s.lines_per_beat = m.lines_per_beat;
        s.channels = m.channels;
        s.channel_names.clone_from(&m.channel_names);
        s.order.clone_from(&m.order);
        s.mixer.clone_from(&m.mixer);
        s.info.clone_from(&m.info);
        s.patterns = self.patterns.iter().map(|p| (**p).clone()).collect();
        s.instruments = self.instruments.iter().map(|p| (**p).clone()).collect();
        s.automation = self.lanes.iter().map(|p| (**p).clone()).collect();
    }
}

/// Undo/redo stacks around the current state.
pub struct History {
    cur: State,
    undo: VecDeque<State>,
    redo: Vec<State>,
    bytes: usize,
    /// signature + time of the last commit (None = next change starts a new step)
    last: Option<(Sig, Instant)>,
}

impl History {
    pub fn new(song: &Song) -> Self {
        History {
            cur: State::capture(song),
            undo: VecDeque::new(),
            redo: vec![],
            bytes: 0,
            last: None,
        }
    }

    /// forget everything (new / opened song)
    pub fn reset(&mut self, song: &Song) {
        *self = History::new(song);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn len(&self) -> usize {
        self.undo.len()
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Record the song if it changed since the last commit. Returns true when
    /// it did. With `may_merge` (keyboard-driven changes, e.g. holding a key on
    /// a focused slider or typing in a field), parameter-only changes to the
    /// same fields within `COALESCE` of the previous commit extend that step.
    /// Mouse gestures pass `may_merge = false`: every click or drag is a step.
    pub fn commit(&mut self, song: &Song, now: Instant, may_merge: bool) -> bool {
        let Some((next, sig)) = self.cur.next(song) else {
            return false;
        };
        let merge = may_merge
            && matches!(&self.last, Some((s, t)) if *s == sig && sig.mergeable()
            && now.duration_since(*t) < COALESCE);
        let old = std::mem::replace(&mut self.cur, next);
        if merge {
            // the step's "before" state is already on the stack; drop the intermediate
            drop(old);
        } else {
            self.bytes += old.bytes;
            self.undo.push_back(old);
            self.evict();
        }
        self.drop_redo();
        self.last = Some((sig, now));
        true
    }

    /// Take the song as it is now into the current state without making an
    /// undo step (plugin state copied into the song before a save).
    pub fn absorb(&mut self, song: &Song) {
        if let Some((next, _)) = self.cur.next(song) {
            self.cur = next;
        }
    }

    /// Start a new step: commit pending changes and don't merge into them.
    pub fn boundary(&mut self, song: &Song, now: Instant) -> bool {
        let c = self.commit(song, now, false);
        self.last = None;
        c
    }

    fn drop_redo(&mut self) {
        for s in self.redo.drain(..) {
            self.bytes = self.bytes.saturating_sub(s.bytes);
        }
    }

    fn evict(&mut self) {
        while self.undo.len() > MAX_STEPS || (self.bytes > MAX_BYTES && self.undo.len() > 1) {
            if let Some(s) = self.undo.pop_front() {
                self.bytes = self.bytes.saturating_sub(s.bytes);
            }
        }
    }

    /// Undo (or redo) one step into `song`. Pending changes are committed first.
    pub fn step(&mut self, song: &mut Song, redo: bool, now: Instant) -> bool {
        self.commit(song, now, false);
        self.last = None;
        let target = if redo {
            self.redo.pop()
        } else {
            self.undo.pop_back()
        };
        let Some(t) = target else { return false };
        let old = std::mem::replace(&mut self.cur, t);
        if redo {
            self.undo.push_back(old);
        } else {
            self.redo.push(old);
        }
        self.cur.apply(song);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn t0() -> Instant {
        Instant::now()
    }
    fn later(t: Instant, ms: u64) -> Instant {
        t + Duration::from_millis(ms)
    }
    fn song() -> Song {
        crate::fileio::demo_song()
    }

    #[test]
    fn typed_note_undo_redo() {
        let mut s = song();
        let mut h = History::new(&s);
        let t = t0();
        assert!(!h.commit(&s, t, true), "no change, no step");
        let before = s.patterns[0].cells[3][1];
        s.patterns[0].cells[3][1].note = 50;
        assert!(h.commit(&s, t, true));
        assert!(h.step(&mut s, false, later(t, 10)));
        assert_eq!(s.patterns[0].cells[3][1], before);
        assert!(h.step(&mut s, true, later(t, 20)));
        assert_eq!(s.patterns[0].cells[3][1].note, 50);
        assert!(!h.step(&mut s, true, later(t, 30)));
    }

    #[test]
    fn every_note_is_its_own_step() {
        let mut s = song();
        let mut h = History::new(&s);
        let t = t0();
        for r in 0..5 {
            s.patterns[0].cells[r][0].note = 40 + r as u8;
            h.commit(&s, later(t, r as u64), true);
        }
        assert_eq!(h.len(), 5, "pattern edits never coalesce");
    }

    #[test]
    fn unchanged_blocks_are_shared() {
        let mut s = song();
        for i in 0..40 {
            s.patterns
                .push(Pattern::new(&format!("B{i}"), ROWS, s.channels));
        }
        let mut h = History::new(&s);
        let prev = h.cur.clone();
        s.patterns[7].cells[0][0].note = 30;
        h.commit(&s, t0(), true);
        let shared = h
            .cur
            .patterns
            .iter()
            .zip(&prev.patterns)
            .filter(|(a, b)| Arc::ptr_eq(a, b))
            .count();
        assert_eq!(shared, s.patterns.len() - 1);
        // one block's worth of bytes, not the whole song
        assert!(h.cur.bytes < pattern_bytes(&s.patterns[7]) * 2);
    }

    #[test]
    fn delete_block_shares_shifted_items() {
        let mut s = song();
        for i in 0..10 {
            s.patterns
                .push(Pattern::new(&format!("B{i}"), ROWS, s.channels));
            s.patterns.last_mut().unwrap().cells[0][0].note = i + 1;
        }
        let mut h = History::new(&s);
        let prev = h.cur.clone();
        s.patterns.remove(3);
        h.commit(&s, t0(), true);
        assert_eq!(h.cur.bytes, 0, "a delete stores nothing new");
        assert!(Arc::ptr_eq(&h.cur.patterns[5], &prev.patterns[6]));
        let mut s2 = s.clone();
        h.step(&mut s2, false, t0());
        assert_eq!(s2.patterns.len(), s.patterns.len() + 1);
    }

    #[test]
    fn instrument_delete_keeps_fx_with_their_instrument() {
        let mut s = song();
        assert!(s.instruments.len() >= 3);
        s.instruments[2].fx = vec![crate::fx::Device::new(crate::fx::FxKind::Bitcrusher)];
        s.instruments[1].fx = vec![crate::fx::Device::new(crate::fx::FxKind::Delay)];
        let orig = s.clone();
        let mut h = History::new(&s);
        s.instruments.remove(1);
        h.commit(&s, t0(), true);
        assert_eq!(s.instruments[1].fx[0].kind, crate::fx::FxKind::Bitcrusher);
        h.step(&mut s, false, t0());
        assert_eq!(s.instruments, orig.instruments);
        assert_eq!(s.instruments[1].fx[0].kind, crate::fx::FxKind::Delay);
        assert_eq!(s.instruments[2].fx[0].kind, crate::fx::FxKind::Bitcrusher);
    }

    #[test]
    fn instrument_add_and_patch_edit() {
        let mut s = song();
        let orig = s.clone();
        let mut h = History::new(&s);
        s.instruments.push(s.instruments[0].clone());
        h.boundary(&s, t0());
        if let InstrumentKind::Synth(p) = &mut s.instruments[0].kind {
            p.cutoff = 123.0;
        }
        s.instruments[0].volume = 0.25;
        s.instruments[0].transpose = 5;
        h.boundary(&s, t0());
        h.step(&mut s, false, t0());
        assert_eq!(s.instruments[0], orig.instruments[0]);
        assert_eq!(s.instruments.len(), orig.instruments.len() + 1);
        h.step(&mut s, false, t0());
        assert_eq!(s.instruments, orig.instruments);
    }

    #[test]
    fn mixer_tempo_lpb_channels_roles() {
        let mut s = song();
        let orig = s.clone();
        let mut h = History::new(&s);
        let t = t0();
        s.mixer.channels[0].volume = 0.1;
        s.mixer.channels[1].pan = -0.5;
        s.mixer.channels[2].mute = true;
        s.mixer.channels[3].solo = true;
        h.boundary(&s, t);
        s.bpm = 150.0;
        s.lines_per_beat = 8;
        h.boundary(&s, t);
        s.channels += 1;
        s.channel_names.push("New".into());
        for p in &mut s.patterns {
            for r in &mut p.cells {
                r.push(Cell::default());
            }
        }
        h.boundary(&s, t);
        s.patterns[0].role = Some(crate::arrange::Role::Chorus);
        h.boundary(&s, t);
        for _ in 0..4 {
            h.step(&mut s, false, t);
        }
        assert_eq!(s.mixer, orig.mixer);
        assert_eq!(s.bpm, orig.bpm);
        assert_eq!(s.lines_per_beat, orig.lines_per_beat);
        assert_eq!(s.channels, orig.channels);
        assert_eq!(s.channel_names, orig.channel_names);
        assert_eq!(s.patterns, orig.patterns);
        for _ in 0..4 {
            h.step(&mut s, true, t);
        }
        assert_eq!(s.bpm, 150.0);
        assert_eq!(s.patterns[0].role, Some(crate::arrange::Role::Chorus));
        assert_eq!(s.channels, orig.channels + 1);
    }

    #[test]
    fn slider_runs_coalesce_but_not_across_boundaries() {
        let mut s = song();
        let mut h = History::new(&s);
        let t = t0();
        for k in 0..20 {
            s.instruments[0].volume = k as f32 / 20.0;
            h.commit(&s, later(t, k * 50), true);
        }
        assert_eq!(h.len(), 1, "one drag = one step");
        // a pause starts a new step
        s.instruments[0].volume = 0.9;
        h.commit(&s, later(t, 5000), true);
        assert_eq!(h.len(), 2);
        // mouse gestures never merge
        s.instruments[0].volume = 0.8;
        h.commit(&s, later(t, 5005), false);
        assert_eq!(h.len(), 3);
        // a different control starts a new step
        s.bpm += 1.0;
        h.commit(&s, later(t, 5010), true);
        assert_eq!(h.len(), 4);
        // an explicit boundary prevents merging
        h.boundary(&s, later(t, 5020));
        s.bpm += 1.0;
        h.commit(&s, later(t, 5030), true);
        assert_eq!(h.len(), 5);
        // the same field by keyboard merges...
        s.bpm += 1.0;
        h.commit(&s, later(t, 5040), true);
        assert_eq!(h.len(), 5);
        // ...but a different song field (mute after tempo) never does
        s.mixer.channels[0].mute ^= true;
        h.commit(&s, later(t, 5050), true);
        assert_eq!(h.len(), 6);
        s.mixer.channels[1].mute ^= true;
        h.commit(&s, later(t, 5060), true);
        assert_eq!(h.len(), 7, "different channel = different step");
    }

    #[test]
    fn wide_songs_keep_per_channel_undo_steps() {
        let mut s = song();
        s.channels = crate::model::MAX_CHANNELS;
        assert_eq!(s.mixer.channels.len(), 32);
        let t = t0();
        let mut h = History::new(&s);
        let n = h.len();
        s.mixer.channels[31].mute ^= true;
        h.commit(&s, later(t, 10), true);
        s.mixer.channels[31].mute ^= true;
        h.commit(&s, later(t, 20), true);
        assert_eq!(h.len(), n + 2, "mute toggles on ch 32 never merge");
        s.mixer.channels[30].volume = 0.5;
        h.commit(&s, later(t, 30), true);
        s.mixer.channels[30].volume = 0.4;
        h.commit(&s, later(t, 40), true);
        assert_eq!(h.len(), n + 3, "same fader merges");
        s.mixer.channels[31].volume = 0.4;
        h.commit(&s, later(t, 50), true);
        assert_eq!(h.len(), n + 4, "another channel's fader is a new step");
    }

    #[test]
    fn fill_accept_with_tempo_nudge_is_one_step() {
        let mut s = song();
        let orig = s.clone();
        let mut h = History::new(&s);
        s.order = vec![0, 1, 0, 1, 1, 0];
        s.patterns.push(Pattern::new("var", ROWS, s.channels));
        s.bpm *= 1.015;
        h.boundary(&s, t0());
        assert_eq!(h.len(), 1);
        h.step(&mut s, false, t0());
        assert_eq!(s.order, orig.order);
        assert_eq!(s.patterns.len(), orig.patterns.len());
        assert_eq!(s.bpm, orig.bpm);
    }

    #[test]
    fn lanes_and_fx_chain_edits() {
        let mut s = song();
        let orig = s.clone();
        let mut h = History::new(&s);
        s.automation[0].points.clear();
        h.boundary(&s, t0());
        s.instruments[1]
            .fx
            .push(crate::fx::Device::new(crate::fx::FxKind::Reverb));
        h.boundary(&s, t0());
        s.mixer.returns[0].level = 0.0;
        h.boundary(&s, t0());
        for _ in 0..3 {
            h.step(&mut s, false, t0());
        }
        assert_eq!(s.automation, orig.automation);
        assert_eq!(s.instruments, orig.instruments);
        assert_eq!(s.mixer, orig.mixer);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut s = song();
        let mut h = History::new(&s);
        s.bpm = 100.0;
        h.boundary(&s, t0());
        h.step(&mut s, false, t0());
        assert!(h.can_redo());
        s.speed = 3;
        h.boundary(&s, t0());
        assert!(!h.can_redo());
    }

    #[test]
    fn sample_pcm_is_shared_not_copied() {
        let mut s = song();
        s.instruments.push(Instrument {
            name: "smp".into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Sample {
                file: String::new(),
                rate: 44100,
                data: vec![0.5; 1_000_000].into(),
                base_note: 48,
                lp: Default::default(),
            },
        });
        let mut h = History::new(&s);
        let n = s.instruments.len() - 1;
        s.instruments[n].volume = 0.5;
        h.commit(&s, t0(), true);
        assert!(
            h.cur.bytes < 10_000,
            "volume change stored {} bytes",
            h.cur.bytes
        );
        // a PCM edit is copy-on-write and does count
        if let InstrumentKind::Sample { data, .. } = &mut s.instruments[n].kind {
            data[0] = 0.0;
        }
        h.boundary(&s, later(t0(), 2000));
        assert!(h.cur.bytes >= 4_000_000);
        h.step(&mut s, false, later(t0(), 2100));
        match &s.instruments[n].kind {
            InstrumentKind::Sample { data, .. } => assert_eq!(data[0], 0.5),
            _ => unreachable!(),
        }
    }

    #[test]
    fn eviction_caps_steps_and_memory() {
        let mut s = song();
        let mut h = History::new(&s);
        let t = t0();
        for k in 0..(MAX_STEPS + 50) {
            s.patterns[0].cells[k % 16][0].note = (k % 100) as u8 + 1;
            h.commit(&s, t, true);
        }
        assert_eq!(h.len(), MAX_STEPS);
        // memory cap: big sample edits
        let mut s = song();
        s.instruments.push(Instrument {
            name: "big".into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Sample {
                file: String::new(),
                rate: 44100,
                data: vec![0.0; 8_000_000].into(),
                base_note: 48,
                lp: Default::default(),
            },
        });
        let n = s.instruments.len() - 1;
        let mut h = History::new(&s);
        for k in 0..20 {
            if let InstrumentKind::Sample { data, .. } = &mut s.instruments[n].kind {
                data[0] = k as f32;
            }
            h.boundary(&s, t);
        }
        assert!(h.bytes() <= MAX_BYTES + 40_000_000, "{}", h.bytes());
        assert!(h.len() < 20);
    }
}
