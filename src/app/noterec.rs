//! MIDI into note clips (M3b): live recording while the timeline plays,
//! with the time stamp of every key (minus the output latency), and
//! Capture, which turns what was just played (recording or not) into a
//! note clip.
//!
//! Record (the toolbar's ● button, R) starts a take: the armed track (or
//! the selected one) records, the song plays from the play marker (after the
//! count-in: Off / 1 / 2 bars, clicking even with the metronome off), and every key goes in at the
//! time it was played, with its length and velocity - chords and
//! overlapping notes as played - into the note clip under the playhead or a
//! new one. A take is one undo step. Step input (one note after the other)
//! is the piano roll's own "Step input" toggle, off by default.
//!
//! The live block: as soon as Record is rolling (after the count-in) the
//! take's clip opens on the track's lane - a new clip from the playhead's
//! bar (the loop's start when loop recording), or the note clip already
//! under the playhead. It fades in, wears the REC style (red tint and
//! outline, a pulsing leading edge, "● REC") and grows with the playhead
//! with no length limit, the notes appearing as they are played (held ones
//! grow too). On stop it settles into a normal clip ending on the bar line
//! after the last note or the stop point, whichever is later (an existing
//! clip never shrinks). Nothing played: a new clip is dropped again (no
//! undo step). Loop recording overdubs in place: the block covers the loop
//! and every pass lands in it. Reduce motion skips the fades and the pulse.
use super::*;
use crate::timeline::{self as tl, Tick, PPQ, ROW};
use std::collections::VecDeque;
use std::time::Instant;

/// a gap this long (seconds) ends the phrase Capture takes
const PHRASE_GAP: f32 = 4.0;
const RECENT_MAX: usize = 1024;
const FOUR_FOUR: tl::SigPoint = tl::SigPoint {
    tick: 0,
    num: 4,
    den: 4,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Played {
    pub(crate) on: Instant,
    pub(crate) off: Option<Instant>,
    pub(crate) key: u8,
    pub(crate) note: u8,
    pub(crate) vel: f32,
}

/// the clip a live recording writes into
#[derive(Clone, Debug)]
struct Take {
    clip: u32,
    data: usize,
    /// held keys: (MIDI key, note index in the data)
    held: Vec<(u8, usize)>,
}

/// a Record take in progress
#[derive(Clone, Debug)]
pub(crate) struct Session {
    /// the track (channel) it records into
    pub(crate) track: usize,
    /// notes recorded so far
    pub(crate) notes: usize,
}

/// the live block of a Record take: its clip, drawn growing with the
/// playhead
#[derive(Clone, Debug)]
pub(crate) struct LiveBlock {
    pub(crate) clip: u32,
    pub(crate) start: Tick,
    /// where the clip ended when the take began (its start for a new clip)
    base_end: Tick,
    /// the furthest the playhead got (ticks)
    pub(crate) reach: f64,
    /// made by this take (an empty take drops it again)
    created: bool,
    /// the timeline's next id before (an empty take puts it back)
    next_id: u32,
    opened: Instant,
    /// recorded notes (index in the clip's data) and when they landed
    landed: Vec<(usize, Instant)>,
}

/// a stopped take settling into a normal clip
#[derive(Clone, Copy, Debug)]
pub(crate) struct Settle {
    clip: u32,
    from: f64,
    to: Tick,
    at: Instant,
}

/// the open / settle fade (seconds)
const LIVE_FADE: f32 = 0.2;
const SETTLE_FADE: f32 = 0.25;

/// how the timeline draws the live block this frame
pub(crate) struct LiveView {
    pub(crate) clip: u32,
    /// the drawn end (ticks)
    pub(crate) end: f64,
    /// the open fade, 0..1
    pub(crate) fade: f32,
    /// the REC style, 1 while recording, falling while it settles
    pub(crate) rec: f32,
    /// recording (the leading edge pulses)
    pub(crate) live: bool,
    /// the leading edge's pulse, 0..1
    pub(crate) pulse: f32,
    /// held notes (index in the data): drawn up to the playhead
    pub(crate) held: Vec<usize>,
    /// landed notes and their fade-in, 0..1
    pub(crate) landed: Vec<(usize, f32)>,
    /// still animating: repaint
    pub(crate) animating: bool,
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x).powi(3)
}

#[derive(Default)]
pub(crate) struct NoteRec {
    take: Option<Take>,
    /// the live block (Record rolling)
    pub(crate) live: Option<LiveBlock>,
    /// the take just stopped, settling
    pub(crate) settle: Option<Settle>,
    /// screenshots: the animations frozen this many seconds in
    pub(crate) anim_at: Option<f32>,
    /// Record is on (the undo hold is taken while it is)
    pub(crate) session: Option<Session>,
    /// the last notes played (for Capture)
    pub(crate) recent: VecDeque<Played>,
}

/// the end of the last phrase: notes after the last gap of PHRASE_GAP
/// seconds, as clip notes from tick 0 at `bpm`
pub(crate) fn phrase(recent: &VecDeque<Played>, bpm: f32, now: Instant) -> Vec<tl::Note> {
    let mut first = recent.len();
    let mut edge: Option<Instant> = None;
    for (i, p) in recent.iter().enumerate().rev() {
        if let Some(e) = edge {
            let end = p.off.unwrap_or(now).max(p.on);
            if e.saturating_duration_since(end).as_secs_f32() > PHRASE_GAP {
                break;
            }
        }
        edge = Some(edge.map_or(p.on, |e: Instant| e.min(p.on)));
        first = i;
    }
    let Some(t0) = recent.get(first).map(|p| p.on) else {
        return vec![];
    };
    let tps = bpm.max(1.0) as f64 / 60.0 * PPQ as f64;
    recent
        .iter()
        .skip(first)
        .map(|p| {
            let s = p.on.saturating_duration_since(t0).as_secs_f64();
            let e = p
                .off
                .unwrap_or(now)
                .saturating_duration_since(t0)
                .as_secs_f64();
            let start = (s * tps).round() as Tick;
            tl::Note {
                start,
                len: ((e * tps).round() as Tick - start).max(1),
                key: p.note,
                vel: tl::roll::vel_u16(p.vel),
                ..Default::default()
            }
        })
        .collect()
}

fn bar_up(t: Tick) -> Tick {
    let bar = 4 * PPQ;
    ((t + bar - 1).div_euclid(bar) * bar).max(bar)
}

impl App {
    /// remember a played note (Capture)
    pub(super) fn rec_remember_on(&mut self, key: u8, note: u8, vel: f32, at: Instant) {
        let r = &mut self.noterec.recent;
        if r.len() >= RECENT_MAX {
            r.pop_front();
        }
        r.push_back(Played {
            on: at,
            off: None,
            key,
            note,
            vel,
        });
    }

    pub(super) fn rec_remember_off(&mut self, key: u8) {
        let now = Instant::now();
        if let Some(p) = self
            .noterec
            .recent
            .iter_mut()
            .rev()
            .find(|p| p.key == key && p.off.is_none())
        {
            p.off = Some(now);
        }
    }

    /// Record on (or the older way: Timeline view, edit mode, playing, live
    /// recording on): the note goes into a note clip on the recording track
    /// at the time it was heard. Returns true when it was taken.
    pub(super) fn rec_note_on(&mut self, key: u8, note: u8, vel: f32, at: Instant) -> bool {
        let ms = self.extra.midi.clone();
        let session = self.noterec.session.clone();
        if session.is_none() && (self.view != View::Timeline || !self.edit || !ms.live_record) {
            return false;
        }
        // the count-in: the key sounds, nothing is recorded
        if session.is_some() && self.with(|s| s.engine.counting_in()) {
            return true;
        }
        // Record rolling: the live block is open before the first note
        if session.is_some() {
            self.rec_live_open();
        }
        let lat = self.audio.as_ref().map_or(0.0, |a| a.latency_ms() / 1000.0) as f64;
        let back = at.elapsed().as_secs_f64() + lat;
        let Some((now, tps, nch)) = self.with(|s| {
            let e = &s.engine;
            let t = e.tl_ticks()?;
            let tps = ROW as f64 / e.row_secs().max(1e-4) as f64;
            s.song.timeline.as_ref()?;
            Some((t, tps, s.song.channels.min(MAX_CHANNELS)))
        }) else {
            return false;
        };
        if nch == 0 {
            return false;
        }
        let mut t = (now - back * tps).max(0.0).round() as Tick;
        if self.extra.rec_quantise {
            let g = self.pr_grid_now().max(1);
            t = (t + g / 2).div_euclid(g) * g;
        }
        // (a note played as the block opened: latency would put it before)
        if let Some(lb) = &self.noterec.live {
            t = t.max(lb.start);
        }
        let track = session.as_ref().map_or(self.ch, |s| s.track).min(nch - 1);
        let vel16 = if ms.velocity {
            tl::roll::vel_u16(vel)
        } else {
            0xFFFF
        };
        // the older way (no Record): the take is still one undo step
        if session.is_none() && self.noterec.take.is_none() {
            self.rec_begin(track);
        }
        self.snapshot();
        let take = self.noterec.take.clone();
        let res = self.with(|s| {
            let t_l = s.song.timeline.as_deref_mut()?;
            // the take's clip, if it still exists and the note is in or after it
            let cur = take.as_ref().and_then(|k| {
                let c = t_l.clips.iter().find(|c| c.id == k.clip)?;
                (c.track as usize == track
                    && t >= c.start
                    && matches!(c.kind, tl::ClipKind::Notes { data } if data as usize == k.data))
                .then(|| k.clone())
            });
            // else the note clip under the playhead on that track (not a
            // looped one: its notes repeat)
            let under = || {
                t_l.clips.iter().find_map(|c| match c.kind {
                    tl::ClipKind::Notes { data }
                        if c.track as usize == track
                            && c.loop_len.is_none()
                            && t >= c.start
                            && t < c.end() =>
                    {
                        Some(Take {
                            clip: c.id,
                            data: data as usize,
                            held: vec![],
                        })
                    }
                    _ => None,
                })
            };
            let cur = cur.or_else(under);
            // a new clip starts on the bar (the loop's start when loop
            // recording, so every pass lands in it: overdub)
            let lp = t_l
                .loop_range
                .filter(|&(a, b)| t_l.loop_on && t >= a && t < b)
                .map(|(a, _)| a);
            let mut take = match cur {
                Some(k) => k,
                None => {
                    let bar = t_l.sig_at(t).bar_ticks().max(1);
                    let start = lp.unwrap_or(t).div_euclid(bar) * bar;
                    let id = tl::new_note_clip(&mut s.song, track, start)?;
                    let t_l = s.song.timeline.as_deref_mut()?;
                    let c = t_l.clips.iter_mut().find(|c| c.id == id)?;
                    let tl::ClipKind::Notes { data } = c.kind else {
                        return None;
                    };
                    // it grows with the take
                    c.len = bar_up(t - start + ROW);
                    let d = t_l.notes[data as usize].make_mut();
                    d.name = format!("Take {}", data + 1);
                    d.len = c.len;
                    Take {
                        clip: id,
                        data: data as usize,
                        held: vec![],
                    }
                }
            };
            let ninst = s.song.instruments.len();
            let t_l = s.song.timeline.as_deref_mut()?;
            let c = t_l.clips.iter_mut().find(|c| c.id == take.clip)?;
            let rel = t - c.start + c.offset;
            let instr = t_l.tracks.get(c.track as usize).map_or(0, |x| x.instr);
            let instr = if instr as usize > ninst { 0 } else { instr };
            let d = t_l.notes.get_mut(take.data)?.make_mut();
            d.notes.push(tl::Note {
                start: rel,
                len: ROW,
                key: note,
                vel: vel16,
                instr,
                ..Default::default()
            });
            take.held.retain(|h| h.0 != key);
            take.held.push((key, d.notes.len() - 1));
            let need = bar_up(rel + ROW);
            d.len = d.len.max(need);
            c.len = c.len.max(need - c.offset);
            let id = c.id;
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some((take, id))
        });
        let Some((take, id)) = res else {
            return false;
        };
        if let Some(lb) = self.noterec.live.as_mut().filter(|lb| lb.clip == id) {
            if let Some(&(_, idx)) = take.held.last() {
                lb.landed.push((idx, Instant::now()));
            }
            lb.reach = lb.reach.max(now);
        }
        self.noterec.take = Some(take);
        if let Some(s) = self.noterec.session.as_mut() {
            s.notes += 1;
        }
        if self.tlv.sel != Some(id) {
            self.tlv.sel = Some(id);
            self.tlv.sels = vec![id];
        }
        self.dirty = true;
        let p = self.with(|s| s.song.timeline.as_deref().map(|x| x.bar_pos(t)));
        if let Some(p) = p {
            self.status = format!(
                "Rec {} at {}.{} track {:02}",
                note_name(note),
                p.bar,
                p.beat,
                track + 1
            );
        }
        true
    }

    /// Step input (the piano roll's "Step input" toggle, off by default):
    /// the key goes into the piano roll's note clip at the step position.
    /// Keys held together land on the same step - a chord - and releasing
    /// the last one moves the step on by the roll's grid. The step starts at
    /// the play marker (or the clip start).
    pub(super) fn step_note_on(&mut self, note: u8, vel: f32) -> bool {
        let ms = self.extra.midi.clone();
        if !self.pr.step_input || self.view != View::Timeline {
            return false;
        }
        let Some(super::pianoview::Target::Notes {
            data,
            clip: Some(id),
        }) = self.pr_target()
        else {
            return false;
        };
        let grid = self.pr_grid_now().max(1);
        let cue = self.tlv.cue;
        let at = match self.pr.step {
            Some((d, t)) if d == data => t,
            _ => self
                .with(|s| {
                    let t = s.song.timeline.as_deref()?;
                    let c = t.clips.iter().find(|c| c.id == id)?;
                    let rel = cue - c.start + c.offset;
                    Some(if cue >= c.start && cue < c.end() {
                        rel.div_euclid(grid) * grid
                    } else {
                        c.offset
                    })
                })
                .unwrap_or(0),
        };
        if !self.pr.step_used {
            self.snapshot();
        }
        let vel16 = if ms.velocity {
            tl::roll::vel_u16(vel)
        } else {
            0xFFFF
        };
        let ok = self.with(|s| {
            let ninst = s.song.instruments.len();
            let t_l = s.song.timeline.as_deref_mut()?;
            let instr = t_l.track_instr(t_l.clips.iter().find(|c| c.id == id)?.track as usize);
            let instr = if instr as usize > ninst { 0 } else { instr };
            let c = t_l.clips.iter_mut().find(|c| c.id == id)?;
            let d = t_l.notes.get_mut(data)?.make_mut();
            // the same key twice on one step is one note
            if !d.notes.iter().any(|n| n.start == at && n.key == note) {
                d.notes.push(tl::Note {
                    start: at,
                    len: grid,
                    key: note,
                    vel: vel16,
                    instr,
                    ..Default::default()
                });
            }
            let need = bar_up(at + grid);
            d.len = d.len.max(need);
            c.len = c.len.max(need - c.offset);
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(())
        });
        if ok.is_none() {
            return false;
        }
        self.pr.step = Some((data, at));
        self.pr.step_used = true;
        self.dirty = true;
        self.status = format!(
            "MIDI step {} (release all keys: next step)",
            note_name(note)
        );
        true
    }

    /// a key released: when none is held, step input moves on
    pub(super) fn step_note_off(&mut self) {
        if !self.pr.step_used || !self.midi.held.is_empty() {
            return;
        }
        self.pr.step_used = false;
        let grid = self.pr_grid_now().max(1);
        if let Some((d, t)) = self.pr.step {
            self.pr.step = Some((d, t + grid));
        }
    }

    /// key released: the recorded note gets its length
    pub(super) fn rec_note_off(&mut self, key: u8) {
        let Some(take) = self.noterec.take.as_mut() else {
            return;
        };
        let Some(k) = take.held.iter().position(|h| h.0 == key) else {
            return;
        };
        let (_, idx) = take.held.remove(k);
        let (clip, data) = (take.clip, take.data);
        let lat = self.audio.as_ref().map_or(0.0, |a| a.latency_ms() / 1000.0) as f64;
        self.with(|s| {
            let e = &s.engine;
            let now = e.tl_ticks();
            let tps = ROW as f64 / e.row_secs().max(1e-4) as f64;
            let Some(t_l) = s.song.timeline.as_deref_mut() else {
                return;
            };
            let Some(c) = t_l.clips.iter_mut().find(|c| c.id == clip) else {
                return;
            };
            let (start, off) = (c.start, c.offset);
            let Some(d) = t_l.notes.get_mut(data) else {
                return;
            };
            let d = d.make_mut();
            let Some(n) = d.notes.get_mut(idx) else {
                return;
            };
            let end = match now {
                Some(t) => (t - lat * tps).round() as Tick - start + off,
                None => n.start + n.len,
            };
            if end > n.start {
                n.len = end - n.start;
            }
            let need = bar_up(n.start + n.len);
            d.len = d.len.max(need);
            c.len = c.len.max(need - off);
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.dirty = true;
    }

    /// Record: start a take (the ● button, R). Stopped: the song plays from
    /// the play marker, after the count-in (Off / 1 / 2 bars, the
    /// Metronome toggle's menu; it clicks even with the metronome off).
    /// Playing: it punches in.
    pub(super) fn rec_start(&mut self) {
        let info = self.with(|s| {
            let t = s.song.timeline.as_deref()?;
            let nch = s.song.channels.min(MAX_CHANNELS);
            let armed = t
                .tracks
                .iter()
                .take(nch)
                .position(|x| x.armed && x.kind != tl::TrackKind::Master);
            Some((nch, armed))
        });
        let Some((nch, armed)) = info else {
            self.status = "Record: the Timeline records (this song is in tracker mode)".into();
            return;
        };
        if nch == 0 {
            self.status = "Record: add a track first".into();
            return;
        }
        let track = armed.unwrap_or(self.ch).min(nch - 1);
        self.rec_begin(track);
        // the count-in clicks whether or not the metronome is on
        let bars = self.extra.count_in_bars() as u32;
        let count = bars > 0;
        let tl_cue = (self.tlv.cue.max(0) / ROW) as usize;
        let cue = self.tlv.cue;
        if let Some(f) = &mut self.fill {
            f.previewing = false;
        }
        // one lock: the count-in holds the song before its first callback
        self.with(|s| {
            if s.preview.take().is_some() {
                s.engine.stop();
            }
            let audio::Shared { song, engine, .. } = s;
            // already playing the timeline: punch in
            if engine.tl_ticks().is_some() {
                return;
            }
            engine.stop();
            engine.start(song, 0, tl_cue, false, 0);
            if count {
                let sig = song
                    .timeline
                    .as_deref()
                    .map_or(FOUR_FOUR, |t| t.sig_at(cue));
                let beat = engine.row_secs() * (sig.beat_ticks() as f32 / ROW as f32);
                let num = sig.num.max(1) as u32;
                engine.count_in(num * bars, num, beat);
            }
        });
        self.status = format!(
            "Recording track {:02}{} (R or Record again: stop recording)",
            track + 1,
            match bars {
                0 => "",
                1 => " after a bar's count-in",
                _ => " after a 2-bar count-in",
            }
        );
    }

    /// a take begins: Record on, one undo step from here
    fn rec_begin(&mut self, track: usize) {
        if self.noterec.session.is_some() {
            return;
        }
        self.snapshot();
        self.undo_hold += 1;
        self.noterec.take = None;
        self.noterec.session = Some(Session { track, notes: 0 });
    }

    /// Record off (playback goes on): held notes get their length, the
    /// take becomes one undo step
    pub(super) fn rec_stop(&mut self) {
        self.rec_live_grow();
        let held: Vec<u8> = self
            .noterec
            .take
            .as_ref()
            .map_or(vec![], |t| t.held.iter().map(|h| h.0).collect());
        for k in held {
            self.rec_note_off(k);
        }
        self.noterec.take = None;
        let kept = self.rec_live_settle();
        if let Some(s) = self.noterec.session.take() {
            self.undo_hold = self.undo_hold.saturating_sub(1);
            self.snapshot();
            self.status = if kept == Some(false) {
                format!(
                    "Recording stopped: nothing was played on track {:02}, so no clip was kept",
                    s.track + 1
                )
            } else {
                format!(
                    "Recorded {} note{} on track {:02}",
                    s.notes,
                    if s.notes == 1 { "" } else { "s" },
                    s.track + 1
                )
            };
        }
    }

    /// Record is rolling (after the count-in): open the live block - the
    /// note clip under the playhead on the recording track, else a new one
    /// from the playhead's bar (the loop's start when loop recording)
    pub(super) fn rec_live_open(&mut self) {
        if self.noterec.live.is_some() {
            return;
        }
        let Some(track) = self.noterec.session.as_ref().map(|s| s.track) else {
            return;
        };
        let res = self.with(|s| {
            if s.engine.counting_in() {
                return None;
            }
            let head = s.engine.tl_ticks()?;
            let t = head.max(0.0).floor() as Tick;
            let t_l = s.song.timeline.as_deref_mut()?;
            let next_id = t_l.next_id;
            let under = t_l.clips.iter().find_map(|c| match c.kind {
                tl::ClipKind::Notes { data }
                    if c.track as usize == track
                        && c.loop_len.is_none()
                        && t >= c.start
                        && t < c.end() =>
                {
                    Some((c.id, data as usize, c.start, c.end()))
                }
                _ => None,
            });
            let (id, data, start, base_end, created) = match under {
                Some((id, data, start, end)) => (id, data, start, end, false),
                None => {
                    let lp = t_l
                        .loop_range
                        .filter(|&(a, b)| t_l.loop_on && t >= a && t < b)
                        .map(|(a, _)| a);
                    let bar = t_l.sig_at(t).bar_ticks().max(1);
                    let start = lp.unwrap_or(t).div_euclid(bar) * bar;
                    let id = tl::new_note_clip(&mut s.song, track, start)?;
                    let t_l = s.song.timeline.as_deref_mut()?;
                    let c = t_l.clips.iter_mut().find(|c| c.id == id)?;
                    let tl::ClipKind::Notes { data } = c.kind else {
                        return None;
                    };
                    // a bar plus headroom for now: it grows with the playhead
                    let len = (t - start + bar - 1).div_euclid(bar).max(1) * bar + 2 * bar;
                    c.len = len;
                    let d = t_l.notes[data as usize].make_mut();
                    d.name = format!("Take {}", data + 1);
                    d.len = len;
                    (id, data as usize, start, start, true)
                }
            };
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some((
                LiveBlock {
                    clip: id,
                    start,
                    base_end,
                    reach: head,
                    created,
                    next_id,
                    opened: Instant::now(),
                    landed: vec![],
                },
                data,
            ))
        });
        let Some((lb, data)) = res else {
            return;
        };
        let id = lb.clip;
        self.noterec.take = Some(Take {
            clip: id,
            data,
            held: vec![],
        });
        self.noterec.live = Some(lb);
        self.noterec.settle = None;
        self.tlv.sel = Some(id);
        self.tlv.sels = vec![id];
    }

    /// the live block follows the playhead: its clip grows bar by bar, kept
    /// two bars ahead (the drawing follows the playhead itself), with no
    /// length limit
    fn rec_live_grow(&mut self) {
        let Some((clip, start, old)) = self
            .noterec
            .live
            .as_ref()
            .map(|l| (l.clip, l.start, l.reach))
        else {
            return;
        };
        let reach = self
            .with(|s| s.engine.tl_ticks())
            .map_or(old, |h| old.max(h));
        if let Some(lb) = self.noterec.live.as_mut() {
            lb.reach = reach;
        }
        self.with(|s| {
            let Some(t_l) = s.song.timeline.as_deref_mut() else {
                return;
            };
            let bar = t_l.sig_at(start).bar_ticks().max(1);
            // two bars of headroom past the playhead, so playback never
            // wraps at the song's end (the drawn edge follows the playhead;
            // the stop trims it to the bar)
            let need =
                ((reach.ceil() as Tick - start).max(1) + bar - 1).div_euclid(bar) * bar + 2 * bar;
            let Some(c) = t_l.clips.iter_mut().find(|c| c.id == clip) else {
                return;
            };
            let tl::ClipKind::Notes { data } = c.kind else {
                return;
            };
            let need_c = need - (c.start - start);
            if c.len >= need_c {
                return;
            }
            c.len = need_c;
            let off = c.offset;
            if let Some(d) = t_l.notes.get_mut(data as usize) {
                let d = d.make_mut();
                d.len = d.len.max(need_c + off);
            }
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
    }

    /// the take stopped: the live block settles into a normal clip ending
    /// on the bar line after the last note or the stop point (an existing
    /// clip never shrinks); an empty new block is dropped. Some(kept) when
    /// there was a block.
    fn rec_live_settle(&mut self) -> Option<bool> {
        let lb = self.noterec.live.take()?;
        let out = self.with(|s| {
            let t_l = s.song.timeline.as_deref_mut()?;
            let ci = t_l.clips.iter().position(|c| c.id == lb.clip)?;
            let tl::ClipKind::Notes { data } = t_l.clips[ci].kind else {
                return None;
            };
            let data = data as usize;
            let off = t_l.clips[ci].offset;
            let notes = &t_l.notes.get(data)?.notes;
            if lb.created && notes.is_empty() {
                t_l.clips.remove(ci);
                let shared = t_l.clips.iter().any(
                    |c| matches!(c.kind, tl::ClipKind::Notes { data: d } if d as usize == data),
                );
                if data + 1 == t_l.notes.len() && !shared {
                    t_l.notes.pop();
                }
                t_l.next_id = lb.next_id;
                let audio::Shared { song, engine, .. } = s;
                engine.ensure_plan(song);
                return Some(None);
            }
            let last = notes
                .iter()
                .map(|n| n.start + n.len - off)
                .max()
                .unwrap_or(0);
            let bar = t_l.sig_at(lb.start).bar_ticks().max(1);
            let end = (lb.reach.ceil() as Tick - lb.start).max(last).max(1);
            let mut len = (end + bar - 1).div_euclid(bar) * bar;
            if !lb.created {
                len = len.max(lb.base_end - lb.start);
            }
            let c = &mut t_l.clips[ci];
            c.len = len;
            let d = t_l.notes[data].make_mut();
            d.len = if lb.created {
                len + off
            } else {
                d.len.max(len + off)
            };
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(Some(lb.start + len))
        });
        match out {
            Some(None) => {
                self.tlv.sel = None;
                self.tlv.sels.clear();
                Some(false)
            }
            Some(Some(to)) => {
                if !self.extra.reduce_motion {
                    self.noterec.settle = Some(Settle {
                        clip: lb.clip,
                        from: lb.reach.max(lb.base_end as f64),
                        to,
                        at: Instant::now(),
                    });
                }
                Some(true)
            }
            None => None,
        }
    }

    /// what the timeline draws for the live (or settling) block
    pub(crate) fn rec_live_view(&self, head: Option<f64>) -> Option<LiveView> {
        let calm = self.extra.reduce_motion;
        let age = |at: Instant| self.noterec.anim_at.unwrap_or(at.elapsed().as_secs_f32());
        if let Some(lb) = &self.noterec.live {
            let el = age(lb.opened);
            let fade = if calm { 1.0 } else { ease(el / LIVE_FADE) };
            let head = head.unwrap_or(lb.reach).max(lb.start as f64);
            let held = self
                .noterec
                .take
                .as_ref()
                .filter(|t| t.clip == lb.clip)
                .map_or(vec![], |t| t.held.iter().map(|h| h.1).collect());
            let landed = lb
                .landed
                .iter()
                .map(|&(i, at)| (i, if calm { 1.0 } else { ease(age(at) / 0.18) }))
                .collect();
            return Some(LiveView {
                clip: lb.clip,
                end: head.max(lb.base_end as f64).max(lb.start as f64 + 1.0),
                fade,
                rec: 1.0,
                live: true,
                pulse: if calm {
                    0.5
                } else {
                    0.5 + 0.5 * (el * std::f32::consts::TAU / 1.2).sin()
                },
                held,
                landed,
                animating: true,
            });
        }
        let st = self.noterec.settle?;
        let k = ease(age(st.at) / SETTLE_FADE);
        Some(LiveView {
            clip: st.clip,
            end: st.from + (st.to as f64 - st.from) * k as f64,
            fade: 1.0,
            rec: 1.0 - k,
            live: false,
            pulse: 0.0,
            held: vec![],
            landed: vec![],
            animating: k < 1.0,
        })
    }

    /// the ● button / R
    pub(super) fn rec_toggle(&mut self) {
        if self.noterec.session.is_some() {
            self.rec_stop();
        } else {
            self.rec_start();
        }
    }

    pub(crate) fn recording(&self) -> bool {
        self.noterec.session.is_some()
    }

    /// once per frame: a take ends when playback stops; the metronome
    /// follows its setting
    pub(super) fn rec_frame(&mut self) {
        let (stopped, beat, bar) = self.with(|s| {
            let e = &s.engine;
            let stopped = !e.playing || (!e.counting_in() && e.tl_ticks().is_none());
            let sig = s
                .song
                .timeline
                .as_deref()
                .map_or(FOUR_FOUR, |t| t.sig_at(0));
            (stopped, sig.beat_ticks(), sig.bar_ticks())
        });
        let on = self.extra.metronome;
        self.with(|s| {
            let m = &mut s.engine.metro;
            m.on = on;
            m.beat = beat;
            m.bar = bar;
        });
        if (self.noterec.take.is_some() || self.noterec.session.is_some()) && stopped {
            self.rec_stop();
        }
        if self.noterec.session.is_some() {
            self.rec_live_open();
            self.rec_live_grow();
        }
        if self
            .noterec
            .settle
            .is_some_and(|st| st.at.elapsed().as_secs_f32() > SETTLE_FADE + 0.1)
            && self.noterec.anim_at.is_none()
        {
            self.noterec.settle = None;
        }
        let r = &mut self.noterec.recent;
        while r.front().is_some_and(|p| p.on.elapsed().as_secs() > 600) {
            r.pop_front();
        }
    }

    /// Capture: the phrase just played becomes a note clip at (track, at)
    pub(super) fn capture_notes(&self) -> Vec<tl::Note> {
        let bpm = self.with(|s| s.song.bpm);
        phrase(&self.noterec.recent, bpm, Instant::now())
    }
}

/// screenshots (--view record-*)
impl App {
    pub(super) fn shot_record(&mut self, view: &str) {
        let bar = 4 * PPQ;
        match view {
            "record-recording" => {
                // a take in progress on the Pad 1 track: chords and a
                // melody, the last note still held
                self.ch = 5;
                self.extra.midi.velocity = true;
                self.rec_start();
                let render = |a: &App, rows: u64| {
                    a.with(|s| {
                        let audio::Shared { song, engine, .. } = s;
                        let target = engine.rows_played + rows;
                        let mut buf = vec![0.0f32; 256];
                        let mut guard = 0;
                        while engine.rows_played < target && engine.playing && guard < 100_000 {
                            engine.render(song, &mut buf);
                            guard += 1;
                        }
                    })
                };
                let now = Instant::now;
                let chords: [&[u8]; 4] =
                    [&[57, 60, 64], &[53, 57, 60], &[48, 52, 55], &[55, 59, 62]];
                for (i, c) in chords.iter().enumerate() {
                    for (j, &k) in c.iter().enumerate() {
                        self.midi_note_on(k, k - 11, 0.55 + 0.1 * j as f32, now());
                    }
                    render(self, 6);
                    self.midi_note_on(72 + i as u8, 61 + i as u8, 0.9, now());
                    render(self, 4);
                    for &k in c.iter() {
                        self.midi_note_off(k);
                    }
                    if i < 3 {
                        self.midi_note_off(72 + i as u8);
                    }
                    render(self, 6);
                }
                self.set_lower(3);
            }
            // the live block: just opening (frozen mid fade), recording
            // with notes and its growing edge, and settled after the stop.
            // CHIPWAVE_SHOT_LIVE_T=<s> freezes the animations that far in.
            "record-live-open" | "record-live" | "record-live-stopped" => {
                // a fresh song with three tracks, recording on the second,
                // the timeline full height
                self.new_song();
                self.track_add(0);
                self.track_add(1);
                self.ch = 1;
                self.extra.full_height = true;
                self.extra.reduce_motion = false;
                self.extra.set_count_in_bars(0);
                self.rec_start();
                self.rec_live_open();
                let t = std::env::var("CHIPWAVE_SHOT_LIVE_T")
                    .ok()
                    .and_then(|v| v.parse::<f32>().ok());
                let render = |a: &mut App, rows: u64| {
                    for _ in 0..rows {
                        a.with(|s| {
                            let audio::Shared { song, engine, .. } = s;
                            let target = engine.rows_played + 1;
                            let mut buf = vec![0.0f32; 256];
                            let mut guard = 0;
                            while engine.rows_played < target && engine.playing && guard < 100_000 {
                                engine.render(song, &mut buf);
                                guard += 1;
                            }
                        });
                        a.rec_live_grow();
                    }
                };
                if view == "record-live-open" {
                    // two bars in: the block appearing, frozen mid fade
                    render(self, 32);
                    self.noterec.anim_at = Some(t.unwrap_or(0.06));
                    return;
                }
                let now = Instant::now;
                let chords: [&[u8]; 6] = [
                    &[57, 60, 64],
                    &[53, 57, 60],
                    &[48, 52, 55],
                    &[55, 59, 62],
                    &[57, 60, 64],
                    &[53, 57, 60],
                ];
                for (i, c) in chords.iter().enumerate() {
                    for (j, &k) in c.iter().enumerate() {
                        self.midi_note_on(k, k - 11, 0.55 + 0.1 * j as f32, now());
                    }
                    render(self, 6);
                    self.midi_note_on(72 + (i % 4) as u8, 61, 0.9, now());
                    render(self, 4);
                    for &k in c.iter() {
                        self.midi_note_off(k);
                    }
                    if i < 5 {
                        self.midi_note_off(72 + (i % 4) as u8);
                    }
                    render(self, 6);
                }
                render(self, 5);
                if view == "record-live" {
                    self.noterec.anim_at = Some(t.unwrap_or(2.7));
                } else {
                    self.rec_stop();
                    self.with(|s| s.engine.stop());
                    match t {
                        Some(t) => self.noterec.anim_at = Some(t),
                        None => self.noterec.settle = None,
                    }
                }
            }
            // the toolbar: idle, playing (Stop lit), the Metronome's ▾ menu
            // open (topbar opens it), and a count-in under way
            "record-idle" | "record-metro-menu" => {}
            "record-playing" => {
                self.extra.metronome = true;
                self.play(0);
            }
            // the LCD toolbar's Master popover open, playing
            "record-lcd-master" => {
                self.extra.metronome = true;
                self.play(0);
                self.lcd.master_open = true;
            }
            "record-countin" => {
                self.ch = 5;
                self.extra.set_count_in_bars(1);
                self.rec_start();
                // a beat and a half in: "Count-in 3" (the shot holds it)
                self.with(|s| {
                    let audio::Shared { song, engine, .. } = s;
                    let beat = engine.row_secs() * (PPQ as f32 / ROW as f32);
                    let mut buf = vec![0.0f32; 2 * (beat * 1.5 * engine.sr) as usize];
                    engine.render(song, &mut buf);
                });
            }
            "record-typing" => {
                // musical typing on, the piano roll open on a note clip,
                // a chord held on the keyboard (A D G)
                self.ch = 5;
                let id = self.with(|s| tl::new_note_clip(&mut s.song, 5, self.tlv.cue));
                if let Some(id) = id {
                    self.tlv.sel = Some(id);
                    self.tlv.sels = vec![id];
                }
                self.set_lower(3);
                self.typing_toggle();
                for k in [egui::Key::A, egui::Key::D, egui::Key::G] {
                    self.typing_on(k);
                }
            }
            "record-master" => {
                // Del's song: two tracks, then + Add track, then Song / Master
                self.new_song();
                self.track_add(0);
                self.track_add(1);
                self.with(|s| {
                    if let Some(t) = s.song.timeline.as_deref_mut() {
                        if let Some(tr) = t.tracks.get_mut(1) {
                            tr.name = "Track 1 copy".into();
                        }
                    }
                    let id = tl::new_note_clip(&mut s.song, 0, 0);
                    let _ = id.map(|_| tl::new_note_clip(&mut s.song, 1, 2 * bar));
                });
                self.tlv.cue = 0;
                self.extra.browser_open = false;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
impl App {
    pub(super) fn noterec_take_done(&self) -> bool {
        self.noterec.take.is_none() && self.noterec.session.is_none()
    }
    pub(super) fn noterec_live(&self) -> Option<(u32, Tick, f64)> {
        self.noterec
            .live
            .as_ref()
            .map(|l| (l.clip, l.start, l.reach))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn capture_takes_the_last_phrase_at_the_song_tempo() {
        let t0 = Instant::now();
        let at = |s: f32| t0 + Duration::from_secs_f32(s);
        let mut r = VecDeque::new();
        // an old phrase, a 6 s pause, then three notes of a beat each at 120 BPM
        r.push_back(Played {
            on: at(0.0),
            off: Some(at(0.4)),
            key: 60,
            note: 49,
            vel: 1.0,
        });
        for (k, s) in [0.0f32, 0.5, 1.0].into_iter().enumerate() {
            r.push_back(Played {
                on: at(6.4 + s),
                off: Some(at(6.4 + s + 0.5)),
                key: 62 + k as u8,
                note: 51 + k as u8,
                vel: 0.5,
            });
        }
        let n = phrase(&r, 120.0, at(9.0));
        assert_eq!(n.len(), 3);
        assert_eq!(
            n.iter().map(|x| x.start).collect::<Vec<_>>(),
            vec![0, PPQ, 2 * PPQ]
        );
        assert!(n.iter().all(|x| x.len == PPQ));
        assert_eq!(n[0].key, 51);
        assert!((tl::roll::vel_f(n[0].vel) - 0.5).abs() < 1e-3);
        // a held note runs until now
        r.back_mut().unwrap().off = None;
        assert_eq!(phrase(&r, 120.0, at(8.4)).last().unwrap().len, 2 * PPQ);
        assert!(phrase(&VecDeque::new(), 120.0, at(1.0)).is_empty());
    }
}
