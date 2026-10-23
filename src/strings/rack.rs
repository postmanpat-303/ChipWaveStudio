//! StringWave instances: one engine per (instrument, channel, layer).
//!
//! The engine owns StringWave's voices, effects and limiter, so it renders
//! once per instance, not per voice: the channel (or rack layer) adds the
//! instance's output before its insert chain, as plugin instruments do.
//! Instances are made on the UI thread ([`sync`], [`prepare`]) for the
//! channels the song plays StringWave on; one the scan missed is made on
//! first use. Nothing exists until a song has a StringWave instrument, so
//! every other song renders exactly as before.
//!
//! Rendering runs in chunks of at most [`BLOCK`] frames that end where
//! the next tracker tick starts, and every chunk re-reads the patch,
//! automation and `43xy` values, so every control is live.
use super::engine::{Engine, Snap};
use super::StringsP;
use crate::gen::GenEngine;
use crate::model::{InstrumentKind, Song, MAX_CHANNELS};

/// the slot plays on the channel itself (not as a rack layer)
pub const DIRECT: u8 = 0xFF;
/// most frames rendered at once (the controls are re-read per chunk)
pub const BLOCK: usize = 64;
/// note tags (the engine's per-voice channel): one per source of notes
pub const TRACKER: u16 = 0xFFFF;
/// note-clip voice `k` of a track
pub const CLIP: u16 = 0x100;
/// rack-layer voice `k` of a track
pub const LAYER: u16 = 0x1000;
/// live MIDI key `k`
pub const LIVE: u16 = 0x4000;

/// StringWave patch of instrument `i`
pub fn patch(song: &Song, i: usize) -> Option<&StringsP> {
    match song.instruments.get(i).map(|x| &x.kind) {
        Some(InstrumentKind::Gen(g)) => match &g.engine {
            GenEngine::Strings(p) => Some(p),
            _ => None,
        },
        _ => None,
    }
}

/// the song has a StringWave instrument
pub fn song_has(song: &Song) -> bool {
    (0..song.instruments.len()).any(|i| patch(song, i).is_some())
}

pub struct Slot {
    pub instr: usize,
    pub ch: usize,
    pub layer: u8,
    eng: Box<Engine>,
    l: [f32; BLOCK],
    r: [f32; BLOCK],
    pos: usize,
    len: usize,
    /// frames rendered silent with no voice (rendering stops when long)
    quiet: usize,
    /// the tracker note it holds (MIDI key, volume at the note)
    held: Option<(u8, f32)>,
    /// the last chunk's controls (new notes start from them)
    snap: Snap,
    sr: f32,
}

impl Slot {
    fn new(instr: usize, ch: usize, layer: u8, p: &StringsP, sr: f32) -> Self {
        Self {
            instr,
            ch,
            layer,
            eng: Box::new(Engine::new(sr)),
            l: [0.0; BLOCK],
            r: [0.0; BLOCK],
            pos: 0,
            len: 0,
            quiet: usize::MAX / 2,
            held: None,
            snap: Snap::new(p, &|_| None),
            sr,
        }
    }

    /// the tracker note and its volume
    pub fn held(&self) -> Option<(u8, f32)> {
        self.held
    }

    pub fn needs_chunk(&self) -> bool {
        self.pos >= self.len
    }

    /// silent for half a second with no voice: skip the DSP
    fn idle(&self) -> bool {
        self.quiet as f32 >= self.sr * 0.5
    }

    /// still making sound (voices or effect tails)
    pub fn sounding(&self) -> bool {
        !self.idle()
    }

    /// render the next `n` frames (1..=BLOCK) with controls `snap`
    pub fn chunk(&mut self, snap: Snap, n: usize) {
        let n = n.clamp(1, BLOCK);
        self.snap = snap;
        if self.idle() && self.eng.active_voices() == 0 {
            self.l[..n].fill(0.0);
            self.r[..n].fill(0.0);
        } else {
            self.eng.render(&snap, &mut self.l[..n], &mut self.r[..n]);
            if self.eng.active_voices() == 0 && self.eng.meter_peak < 1e-6 {
                self.quiet = self.quiet.saturating_add(n);
            } else {
                self.quiet = 0;
            }
        }
        self.pos = 0;
        self.len = n;
    }

    pub fn sample(&mut self) -> (f32, f32) {
        if self.pos >= self.len {
            return (0.0, 0.0);
        }
        let k = self.pos;
        self.pos += 1;
        (self.l[k], self.r[k])
    }

    /// tracker pitch (portamento, vibrato) in semitones
    pub fn set_tune(&mut self, semis: f32) {
        self.eng.tune = semis;
    }

    pub fn note_on(&mut self, key: u8, vel: f32, tag: u16) {
        self.quiet = 0;
        let snap = self.snap;
        self.eng.note_on(&snap, key, vel.clamp(0.0, 1.0), None, tag);
    }

    pub fn note_off(&mut self, key: u8, tag: u16) {
        self.eng.note_off(key, tag);
    }

    pub fn active_voices(&self) -> usize {
        self.eng.active_voices()
    }

    /// output peak and limiter reduction (dB) of the last chunk
    pub fn meter(&self) -> (f32, f32) {
        (self.eng.meter_peak, self.eng.meter_gr)
    }
}

pub struct Rack {
    pub slots: Vec<Slot>,
    sr: f32,
    /// live notes: (tag, slot, MIDI key); slot u16::MAX = free
    live: [(u16, u16, u8); 64],
    /// the instrument live MIDI plays (pedal, bend, wheel go to it)
    live_instr: usize,
    sustain: bool,
    bend: f32,
    /// live mod wheel and expression (0..1) of `live_instr`
    wheel: f32,
    expr: f32,
}

impl Default for Rack {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Rack {
    pub fn new(sr: f32) -> Self {
        Self {
            slots: Vec::new(),
            sr,
            live: [(0, u16::MAX, 0); 64],
            live_instr: usize::MAX,
            sustain: false,
            bend: 0.0,
            wheel: 0.0,
            expr: 1.0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn find(&self, instr: usize, ch: usize, layer: u8) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.instr == instr && s.ch == ch && s.layer == layer)
    }

    /// the slot of (instr, ch, layer), made now if the scan missed it
    /// (allocates once; None when `instr` is not StringWave)
    pub fn ensure(&mut self, song: &Song, instr: usize, ch: usize, layer: u8) -> Option<usize> {
        if let Some(i) = self.find(instr, ch, layer) {
            return Some(i);
        }
        let p = patch(song, instr)?;
        if ch >= MAX_CHANNELS {
            return None;
        }
        let s = Slot::new(instr, ch, layer, p, self.sr);
        self.push(s);
        Some(self.slots.len() - 1)
    }

    fn push(&mut self, mut s: Slot) {
        if s.instr == self.live_instr {
            s.eng.wheel = self.wheel;
            s.eng.expression = self.expr;
        }
        self.slots.push(s);
    }

    /// a tracker note on channel `ch`: ends the channel's tracker note,
    /// then plays `key` on (instr, ch)
    pub fn tracker_note(&mut self, song: &Song, instr: usize, ch: usize, key: u8, vel: f32) {
        self.release_ch(ch);
        if let Some(i) = self.ensure(song, instr, ch, DIRECT) {
            let s = &mut self.slots[i];
            s.note_on(key, vel, TRACKER);
            s.held = Some((key, vel.max(1e-3)));
            s.eng.tune = 0.0;
        }
    }

    /// note-off for the tracker note channel `ch` holds
    pub fn release_ch(&mut self, ch: usize) {
        for s in self.slots.iter_mut().filter(|s| s.ch == ch) {
            if let Some((n, _)) = s.held.take() {
                s.note_off(n, TRACKER);
            }
        }
    }

    pub fn live_on(&mut self, song: &Song, instr: usize, ch: usize, key: u8, note: u8, vel: f32) {
        self.live_off(key);
        if instr != self.live_instr {
            self.live_instr = instr;
            for s in &mut self.slots {
                let mine = s.instr == instr;
                s.eng.wheel = if mine { self.wheel } else { 0.0 };
                s.eng.expression = if mine { self.expr } else { 1.0 };
            }
        }
        let Some(i) = self.ensure(song, instr, ch, DIRECT) else {
            return;
        };
        let s = &mut self.slots[i];
        s.eng.set_sustain(self.sustain);
        s.eng.bend = self.bend;
        s.note_on(note, vel, LIVE + key as u16);
        if let Some(f) = self.live.iter_mut().find(|l| l.1 == u16::MAX) {
            *f = (LIVE + key as u16, i as u16, note);
        }
    }

    pub fn live_off(&mut self, key: u8) {
        let tag = LIVE + key as u16;
        for l in self
            .live
            .iter_mut()
            .filter(|l| l.1 != u16::MAX && l.0 == tag)
        {
            if let Some(s) = self.slots.get_mut(l.1 as usize) {
                s.note_off(l.2, tag);
            }
            l.1 = u16::MAX;
        }
    }

    /// the sustain pedal (live instrument's slots)
    pub fn sustain(&mut self, on: bool) {
        self.sustain = on;
        let li = self.live_instr;
        for s in self.slots.iter_mut().filter(|s| s.instr == li) {
            s.eng.set_sustain(on);
        }
    }

    /// pitch bend in semitones (live instrument's slots)
    pub fn bend(&mut self, semis: f32) {
        self.bend = semis;
        let li = self.live_instr;
        for s in self.slots.iter_mut().filter(|s| s.instr == li) {
            s.eng.bend = semis;
        }
    }

    /// live mod wheel (0..1) of the live instrument
    pub fn wheel(&mut self, v: f32) {
        self.wheel = v.clamp(0.0, 1.0);
        let li = self.live_instr;
        for s in self.slots.iter_mut().filter(|s| s.instr == li) {
            s.eng.wheel = self.wheel;
        }
    }

    /// live expression (0..1) of the live instrument
    pub fn expression(&mut self, v: f32) {
        self.expr = v.clamp(0.0, 1.0);
        let li = self.live_instr;
        for s in self.slots.iter_mut().filter(|s| s.instr == li) {
            s.eng.expression = self.expr;
        }
    }

    /// the live instrument changed (a song or the selection): live
    /// controllers start from their rests
    pub fn live_owner(&mut self, instr: usize) {
        if instr != self.live_instr {
            self.live_instr = instr;
            self.wheel = 0.0;
            self.expr = 1.0;
        }
    }

    /// every note off (stop): tails ring out
    pub fn all_off(&mut self) {
        self.sustain = false;
        self.bend = 0.0;
        for s in &mut self.slots {
            s.held = None;
            s.eng.all_notes_off();
            s.eng.bend = 0.0;
            s.eng.tune = 0.0;
        }
        for l in self.live.iter_mut() {
            l.1 = u16::MAX;
        }
    }

    /// silence now (panic, seek): voices and effect tails cleared
    pub fn panic(&mut self) {
        self.all_off();
        for s in &mut self.slots {
            s.eng.reset();
            s.pos = s.len;
            s.quiet = usize::MAX / 2;
        }
    }

    /// instruments were removed / inserted: follow them, drop the others
    pub fn remap(&mut self, f: &dyn Fn(usize) -> Option<usize>) -> Vec<Slot> {
        let mut gone = vec![];
        let mut k = 0;
        while k < self.slots.len() {
            match f(self.slots[k].instr) {
                Some(n) => {
                    self.slots[k].instr = n;
                    k += 1;
                }
                None => gone.push(self.remove(k)),
            }
        }
        if let Some(n) = f(self.live_instr) {
            self.live_instr = n;
        }
        gone
    }

    /// remove slot `k`; live notes on later slots keep their slot
    fn remove(&mut self, k: usize) -> Slot {
        for l in self.live.iter_mut().filter(|l| l.1 != u16::MAX) {
            if l.1 as usize == k {
                l.1 = u16::MAX;
            } else if l.1 as usize > k {
                l.1 -= 1;
            }
        }
        self.slots.remove(k)
    }

    /// active voices of instrument `instr` (all its slots)
    pub fn voices_of(&self, instr: usize) -> usize {
        self.slots
            .iter()
            .filter(|s| s.instr == instr)
            .map(|s| s.active_voices())
            .sum()
    }

    /// the loudest slot of `instr`: (peak, limiter reduction dB)
    pub fn meter_of(&self, instr: usize) -> Option<(f32, f32)> {
        self.slots
            .iter()
            .filter(|s| s.instr == instr)
            .map(|s| s.meter())
            .reduce(|a, b| (a.0.max(b.0), a.1.min(b.1)))
    }

    /// slots to make and slots to drop so the rack matches `song`
    fn plan(
        &self,
        song: &Song,
        extra: Option<(usize, usize)>,
    ) -> (Vec<(usize, usize, u8)>, Vec<usize>) {
        let need = needed(song, extra);
        let add = need
            .iter()
            .copied()
            .filter(|&(i, c, l)| self.find(i, c, l).is_none())
            .collect();
        let drop = (0..self.slots.len())
            .filter(|&k| {
                let s = &self.slots[k];
                patch(song, s.instr).is_none()
                    || (!need.contains(&(s.instr, s.ch, s.layer))
                        && !s.sounding()
                        && s.held.is_none()
                        && !self.live.iter().any(|l| l.1 == k as u16))
            })
            .collect();
        (add, drop)
    }

    fn take(&mut self, drop: &[usize]) -> Vec<Slot> {
        let mut out = vec![];
        for &k in drop.iter().rev() {
            if k < self.slots.len() {
                out.push(self.remove(k));
            }
        }
        out
    }
}

/// The (instrument, channel, layer) slots `song` plays StringWave on:
/// pattern cells (tracker channels and the timeline's block clips), note
/// clips (arrangement and launcher), rack layers, and `extra` (the
/// instrument and channel live MIDI plays).
pub fn needed(song: &Song, extra: Option<(usize, usize)>) -> Vec<(usize, usize, u8)> {
    if !song_has(song) {
        return vec![];
    }
    let ni = song.instruments.len();
    let nch = song.channels.min(MAX_CHANNELS);
    // instrument on channel (any kind: racks expand below)
    let mut used = vec![false; ni * MAX_CHANNELS];
    let mut mark = |i: usize, c: usize| {
        if i < ni && c < MAX_CHANNELS {
            used[i * MAX_CHANNELS + c] = true;
        }
    };
    for p in &song.patterns {
        for row in &p.cells {
            for (c, cell) in row.iter().enumerate().take(nch) {
                if cell.instr > 0 {
                    mark(cell.instr as usize - 1, c);
                }
            }
        }
    }
    if let Some(tl) = song.timeline.as_deref() {
        let launch = tl.launcher.iter().map(|l| &l.clip);
        for clip in tl.clips.iter().chain(launch) {
            let t = clip.track as usize;
            match clip.kind {
                crate::timeline::ClipKind::Block { pattern, col } => {
                    if let Some(p) = song.patterns.get(pattern) {
                        for row in &p.cells {
                            if let Some(cell) = row.get(col as usize) {
                                if cell.instr > 0 {
                                    mark(cell.instr as usize - 1, t);
                                }
                            }
                        }
                    }
                }
                crate::timeline::ClipKind::Notes { data } => {
                    let own = tl.tracks.get(t).map_or(0, |x| x.instr);
                    if let Some(d) = tl.notes.get(data as usize) {
                        for n in &d.notes {
                            let i = if n.instr > 0 { n.instr } else { own };
                            if i > 0 {
                                mark(i as usize - 1, t);
                            }
                        }
                    }
                }
                crate::timeline::ClipKind::Audio { .. } => {}
            }
        }
    }
    if let Some((i, c)) = extra {
        mark(i, c);
    }
    let mut v = vec![];
    for i in 0..ni {
        for c in 0..nch {
            if !used[i * MAX_CHANNELS + c] {
                continue;
            }
            match &song.instruments[i].kind {
                InstrumentKind::Rack(r) => {
                    for (k, l) in r.layers.iter().enumerate().take(crate::model::MAX_LAYERS) {
                        if patch(song, l.slot).is_some() && !v.contains(&(l.slot, c, k as u8)) {
                            v.push((l.slot, c, k as u8));
                        }
                    }
                }
                _ if patch(song, i).is_some() => v.push((i, c, DIRECT)),
                _ => {}
            }
        }
    }
    v
}

/// Make the slots the song needs and drop the ones it no longer does
/// (UI thread; the engines are made and freed outside the lock).
pub fn sync(shared: &crate::audio::SharedRef, extra: Option<(usize, usize)>) {
    let (add, drop_, sr) = {
        let s = crate::audio::lock(shared);
        if s.engine.strings.is_empty() && !song_has(&s.song) {
            return;
        }
        let (a, d) = s.engine.strings.plan(&s.song, extra);
        (a, d, s.engine.strings.sr)
    };
    if !drop_.is_empty() {
        let old = crate::audio::lock(shared).engine.strings.take(&drop_);
        drop(old);
    }
    for (i, c, l) in add {
        let p = {
            let s = crate::audio::lock(shared);
            match patch(&s.song, i) {
                Some(p) => p.clone(),
                None => continue,
            }
        };
        let slot = Slot::new(i, c, l, &p, sr);
        let mut s = crate::audio::lock(shared);
        if patch(&s.song, i).is_some() && s.engine.strings.find(i, c, l).is_none() {
            s.engine.strings.push(slot);
        }
    }
}

/// Offline: make every slot the song needs (export, --render, tests).
pub fn prepare(rack: &mut Rack, song: &Song, sr: f32) {
    rack.sr = sr;
    let (add, drop_) = rack.plan(song, None);
    drop(rack.take(&drop_));
    for (i, c, l) in add {
        if let Some(p) = patch(song, i) {
            let s = Slot::new(i, c, l, p, sr);
            rack.push(s);
        }
    }
}
