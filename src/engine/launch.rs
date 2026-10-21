//! Clip Arranger launches (docs/CLIP_ARRANGER.md). The UI queues a [`Cue`]
//! under the shared lock like every other engine call; the render thread
//! applies it on the next quantise boundary inside `advance_row`. A cue is
//! plain `Copy` data and applying it only flips fields, so the audio path
//! takes no extra locks and allocates nothing.
//!
//! Phase 1: a scene is a block, launched as "loop block N" (the tracker's
//! block loop). Stopped tracks are held silent through their mixer gain.
use super::Engine;
use crate::model::{Song, MAX_CHANNELS};

/// A launch waiting for its boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cue {
    /// block to loop from the boundary (None: keep what plays)
    pub scene: Option<usize>,
    /// timeline songs: timeline rows [a, b) to loop from the boundary
    /// (a scene or slot of note / audio clips; None: keep what plays)
    pub span: Option<(u32, u32)>,
    /// tracks (bit per track) that play again from the boundary
    pub on: u64,
    /// tracks that stop (go silent) from the boundary
    pub off: u64,
    /// stop the transport at the boundary
    pub halt: bool,
    /// the boundary: a row where `(pos - phase) % every == 0` (1 = next row)
    pub every: u32,
    pub phase: u32,
    /// launcher clips (Stage 4b): the clip id each track plays from the
    /// boundary, looped (0: no change). The track leaves the arrangement
    /// until it is stopped (`off`) or the transport starts again.
    pub clips: [u32; MAX_CHANNELS],
}

/// bit of track `tr` in a [`Cue`] mask
pub fn bit(tr: usize) -> u64 {
    if tr < MAX_CHANNELS {
        1u64 << tr
    } else {
        0
    }
}

impl Engine {
    /// Queue a launch. Stopped: a scene starts at once (looping its block)
    /// and a halt does nothing. Playing: merged into the pending cue (a later
    /// scene wins, track masks add up) and applied on the next boundary.
    pub fn launch(&mut self, song: &Song, c: Cue) {
        if !self.playing {
            if let Some(b) = c.scene.filter(|&b| b < song.patterns.len()) {
                self.start(song, 0, 0, true, b);
                self.held = c.off & !c.on;
            } else if let Some((a, b)) = c.span.filter(|s| s.1 > s.0 && song.timeline.is_some()) {
                self.start(song, 0, a as usize, false, 0);
                self.tl_span = Some((a as usize, b as usize));
                self.held = c.off & !c.on;
            } else if c.clips.iter().any(|&x| x != 0) && song.timeline.is_some() {
                // launcher clips from a stop: the transport runs from the
                // start with only the launched tracks sounding
                self.start(song, 0, 0, false, 0);
                self.held = c.off & !c.on;
                self.launch_clips(&c);
            }
            return;
        }
        let mut q = self.cue.unwrap_or_default();
        if c.halt {
            q = c;
        } else {
            if c.scene.is_some() {
                q.scene = c.scene;
                q.span = None;
                q.halt = false;
            }
            if c.span.is_some() {
                q.span = c.span;
                q.scene = None;
                q.halt = false;
            }
            q.on = (q.on & !c.off) | c.on;
            q.off = (q.off & !c.on) | c.off;
            for tr in 0..MAX_CHANNELS {
                if c.clips[tr] != 0 {
                    q.clips[tr] = c.clips[tr];
                } else if c.off & bit(tr) != 0 {
                    q.clips[tr] = 0;
                }
            }
            q.every = c.every;
            q.phase = c.phase;
        }
        self.cue = Some(q);
    }

    /// drop a pending launch (nothing changes at the boundary)
    pub fn cancel_cue(&mut self) {
        self.cue = None;
    }

    /// start the cue's launcher clips at this row (their tracks' arrangement
    /// notes, Tracker note and audio clips end)
    fn launch_clips(&mut self, c: &Cue) {
        let origin = self.rows_played as i64 * crate::timeline::ROW;
        for (tr, &id) in c.clips.iter().enumerate() {
            if id == 0 {
                continue;
            }
            let b = bit(tr);
            self.nv.release_where(&mut self.plugins, b);
            if self.nv.lmask & b == 0 {
                self.av.cut_track(tr);
                if let Some(ch) = self.chans.get_mut(tr) {
                    ch.synth.amp.release();
                }
            }
            self.nv.lmask |= b;
            self.nv.lp[tr] = super::notes::LPlay::new(id, origin);
        }
    }

    /// the launcher clip track `tr` plays (None: the arrangement or nothing)
    pub fn launched(&self, tr: usize) -> Option<u32> {
        (tr < MAX_CHANNELS && self.nv.lmask & bit(tr) != 0).then(|| self.nv.lp[tr].id)
    }

    /// tracks held silent by the Clip Arranger (bit per track)
    pub fn held(&self) -> u64 {
        self.held
    }

    /// a scene (block loop) is what plays now
    pub fn scene(&self) -> Option<usize> {
        (self.playing && self.loop_pattern && !self.tl.on).then_some(self.pattern)
    }

    /// timeline songs: the launched span (timeline rows [a, b)) that loops now
    pub fn span(&self) -> Option<(usize, usize)> {
        if self.playing && self.tl.on {
            self.tl_span
        } else {
            None
        }
    }

    /// the pending launch
    pub fn cue(&self) -> Option<Cue> {
        self.cue
    }

    /// render thread, on a row boundary: apply the pending cue when this row
    /// is its boundary. Returns false when it stopped the transport.
    pub(super) fn cue_step(&mut self, song: &Song) -> bool {
        let Some(c) = self.cue else {
            return true;
        };
        let pos = if self.tl.on { self.tl.row } else { self.row } as i64;
        if (pos - c.phase as i64).rem_euclid(c.every.max(1) as i64) != 0 {
            return true;
        }
        self.cue = None;
        if c.halt {
            self.stop();
            self.held = 0;
            self.tl_span = None;
            return false;
        }
        if let Some(b) = c.scene.filter(|&b| b < song.patterns.len()) {
            if self.tl.on {
                // the arrangement stops; its note and audio clips fade out
                self.tl.on = false;
                self.nv.release_all(&mut self.plugins);
                self.av.release_all();
            }
            self.loop_pattern = true;
            self.tl_span = None;
            self.pattern = b;
            self.row = 0;
            if let Some(k) = song.order.iter().position(|&x| x == b) {
                self.order_pos = k;
            }
        }
        if let Some((a, b)) = c.span.filter(|s| s.1 > s.0 && self.tl.plan.is_some()) {
            // loop the span from its start: from a block loop the timeline
            // takes over again (the plan is kept up to date by the UI)
            self.tl.on = true;
            self.loop_pattern = false;
            self.tl_span = Some((a as usize, b as usize));
            let a = a as usize;
            self.tl.row = a;
            self.tl_cursors(a);
            self.notes_jump(song, a, false);
        }
        self.held = (self.held | c.off) & !c.on;
        // a stopped track leaves its launcher clip; launched ones start now
        self.nv.lmask &= !c.off;
        self.launch_clips(&c);
        // a held track's sounding notes end now (its gain ramps to 0 too)
        for (i, ch) in self.chans.iter_mut().enumerate().take(MAX_CHANNELS) {
            if c.off & (1u64 << i) != 0 {
                ch.synth.amp.release();
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tl_song() -> Song {
        let mut s = crate::fileio::demo_song_n(0);
        let (tl, _) = crate::timeline::from_order(&s);
        s.timeline = Some(Box::new(tl));
        s
    }

    /// render until the engine has played `rows` more rows
    fn play_rows(e: &mut Engine, song: &Song, rows: u64) {
        let target = e.rows_played + rows;
        let mut buf = vec![0.0f32; 128];
        let mut guard = 0;
        while e.rows_played < target && e.playing && guard < 200_000 {
            e.render(song, &mut buf);
            guard += 1;
        }
    }

    fn bar() -> Cue {
        Cue {
            every: 16,
            ..Default::default()
        }
    }

    #[test]
    fn stopped_scene_launch_starts_at_once_looping_its_block() {
        let song = tl_song();
        let mut e = Engine::new(48000.0);
        e.launch(
            &song,
            Cue {
                scene: Some(2),
                ..bar()
            },
        );
        assert!(e.playing);
        assert_eq!(e.scene(), Some(2));
        assert_eq!(e.row, 0);
        assert_eq!(e.cue(), None);
    }

    #[test]
    fn scene_launch_waits_for_the_next_bar_of_the_timeline() {
        let song = tl_song();
        assert!(song.patterns.len() > 3);
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        play_rows(&mut e, &song, 5);
        assert_eq!(e.tl_row(), Some(5));
        e.launch(
            &song,
            Cue {
                scene: Some(3),
                ..bar()
            },
        );
        // rows 6..15 still play the arrangement
        play_rows(&mut e, &song, 10);
        assert_eq!(e.tl_row(), Some(15));
        assert!(e.cue().is_some());
        assert_eq!(e.scene(), None);
        // row 16 is the bar line: block 03 loops from its first row
        play_rows(&mut e, &song, 1);
        assert_eq!(e.scene(), Some(3));
        assert_eq!(e.row, 0);
        assert_eq!(e.cue(), None);
        let rows = song.patterns[3].rows as u64;
        play_rows(&mut e, &song, rows);
        assert_eq!((e.scene(), e.row), (Some(3), 0), "the block loops");
    }

    #[test]
    fn track_stop_and_stop_all_land_on_the_bar() {
        let song = tl_song();
        let mut e = Engine::new(48000.0);
        e.launch(
            &song,
            Cue {
                scene: Some(1),
                ..bar()
            },
        );
        play_rows(&mut e, &song, 3);
        e.launch(
            &song,
            Cue {
                off: bit(2),
                ..bar()
            },
        );
        play_rows(&mut e, &song, 12);
        assert_eq!(e.held(), 0, "not before the bar");
        play_rows(&mut e, &song, 1);
        assert_eq!(e.row, 16);
        assert_eq!(e.held(), bit(2));
        // the held track's fader is closed, the others are not
        let (tgt, _) = e.mix_targets(&song);
        assert_eq!(tgt[2], (0.0, 0.0));
        assert!(tgt[0].0 > 0.0 || !song.mixer.audible(0));
        // play it again, then stop everything
        e.launch(
            &song,
            Cue {
                on: bit(2),
                ..bar()
            },
        );
        play_rows(&mut e, &song, 16);
        assert_eq!(e.held(), 0);
        e.launch(
            &song,
            Cue {
                halt: true,
                ..bar()
            },
        );
        play_rows(&mut e, &song, 40);
        assert!(!e.playing);
        assert_eq!(e.row % 16, 0, "stopped on a bar line");
    }

    #[test]
    fn a_later_scene_replaces_the_pending_one_and_masks_add_up() {
        let song = tl_song();
        let mut e = Engine::new(48000.0);
        e.launch(
            &song,
            Cue {
                scene: Some(0),
                ..bar()
            },
        );
        play_rows(&mut e, &song, 1);
        e.launch(
            &song,
            Cue {
                scene: Some(1),
                off: bit(0) | bit(1),
                ..bar()
            },
        );
        e.launch(
            &song,
            Cue {
                scene: Some(2),
                on: bit(1),
                ..bar()
            },
        );
        let q = e.cue().unwrap();
        assert_eq!((q.scene, q.off, q.on), (Some(2), bit(0), bit(1)));
        e.cancel_cue();
        assert_eq!(e.cue(), None);
    }

    /// launching and holding inside the render loop allocate nothing (after
    /// the warm-up every song needs)
    #[test]
    fn launches_are_allocation_free_in_the_render_loop() {
        let song = tl_song();
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 512];
        for _ in 0..400 {
            e.render(&song, &mut buf);
        }
        let mut cycle = |e: &mut Engine| {
            for k in 0..2000 {
                if k % 300 == 0 {
                    e.launch(
                        &song,
                        Cue {
                            scene: Some(k / 300 % 3),
                            off: bit(k % 5),
                            on: bit((k + 1) % 5),
                            every: 4,
                            ..Default::default()
                        },
                    );
                }
                e.render(&song, &mut buf);
            }
        };
        // warm-up: each block's instruments build their FX state on first use
        cycle(&mut e);
        let n = crate::engine::alloc_count::count(|| cycle(&mut e));
        assert_eq!(n, 0, "launches allocated {n} times");
        assert!(e.playing);
    }

    /// nothing queued: playback is exactly what it was before the Clip Arranger
    #[test]
    fn without_launches_the_render_is_unchanged() {
        let song = tl_song();
        let mut a = Engine::new(48000.0);
        let mut b = Engine::new(48000.0);
        a.start(&song, 0, 0, false, 0);
        b.start(&song, 0, 0, false, 0);
        b.cancel_cue();
        let (mut x, mut y) = (vec![0.0f32; 512], vec![0.0f32; 512]);
        for _ in 0..300 {
            a.render(&song, &mut x);
            b.render(&song, &mut y);
            assert_eq!(x, y);
        }
    }
}
