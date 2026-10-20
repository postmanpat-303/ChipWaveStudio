//! MIDI into note clips (M3b): live recording while the timeline plays,
//! with the time stamp of every key (minus the output latency), and
//! Capture, which turns what was just played (recording or not) into a
//! note clip.
//!
//! Record (the toolbar's ● button, R) starts a take: the armed track (or
//! the selected one) records, the song plays from the play marker (after a
//! one-bar count-in when the metronome is on), and every key goes in at the
//! time it was played, with its length and velocity - chords and
//! overlapping notes as played - into the note clip under the playhead or a
//! new one. A take is one undo step. Step input (one note after the other)
//! is the piano roll's own "Step input" toggle, off by default.
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

#[derive(Default)]
pub(crate) struct NoteRec {
    take: Option<Take>,
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
            let t_l = s.song.timeline.as_deref_mut()?;
            let c = t_l.clips.iter_mut().find(|c| c.id == take.clip)?;
            let rel = t - c.start + c.offset;
            let d = t_l.notes.get_mut(take.data)?.make_mut();
            d.notes.push(tl::Note {
                start: rel,
                len: ROW,
                key: note,
                vel: vel16,
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
                "● Rec {} at {}.{} track {:02}",
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
            let t_l = s.song.timeline.as_deref_mut()?;
            let c = t_l.clips.iter_mut().find(|c| c.id == id)?;
            let d = t_l.notes.get_mut(data)?.make_mut();
            // the same key twice on one step is one note
            if !d.notes.iter().any(|n| n.start == at && n.key == note) {
                d.notes.push(tl::Note {
                    start: at,
                    len: grid,
                    key: note,
                    vel: vel16,
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
    /// the play marker, after a bar's count-in when the metronome is on.
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
        let count = self.extra.metronome && self.extra.count_in;
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
                engine.count_in(sig.num.max(1) as u32, beat);
            }
        });
        self.status = format!(
            "● Recording track {:02}{} (R or ● again: stop recording)",
            track + 1,
            if count { " after a bar's count-in" } else { "" }
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
        let held: Vec<u8> = self
            .noterec
            .take
            .as_ref()
            .map_or(vec![], |t| t.held.iter().map(|h| h.0).collect());
        for k in held {
            self.rec_note_off(k);
        }
        self.noterec.take = None;
        if let Some(s) = self.noterec.session.take() {
            self.undo_hold = self.undo_hold.saturating_sub(1);
            self.snapshot();
            self.status = format!(
                "Recorded {} note{} on track {:02}",
                s.notes,
                if s.notes == 1 { "" } else { "s" },
                s.track + 1
            );
        }
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

#[cfg(test)]
impl App {
    pub(super) fn noterec_take_done(&self) -> bool {
        self.noterec.take.is_none() && self.noterec.session.is_none()
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
