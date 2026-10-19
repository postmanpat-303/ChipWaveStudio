//! Track edits from the timeline headers and the Browser: add, reorder
//! (docs/TIMELINE.md "Tracks"). Tracks are mixer channels; the work is
//! done by `crate::chanops`, here: undo, the cursor and status.
use super::*;

impl App {
    /// insert an empty track at `at` named `name` (no snapshot; callers
    /// take one). False with a message when all 32 are used.
    pub(super) fn track_insert(&mut self, at: usize, name: &str) -> bool {
        let ok = self.with(|s| {
            let at = at.min(s.song.channels);
            crate::chanops::insert_channel(&mut s.song, at, name)
        });
        if !ok {
            self.status = "All 32 tracks are used (the channel limit)".into();
            return false;
        }
        let n = self.with(|s| s.song.channels);
        self.ch = at.min(n - 1);
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        true
    }

    /// "+ Add track" / Add track above / below (undoable)
    pub(super) fn track_add(&mut self, at: usize) -> bool {
        if self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status = "All 32 tracks are used (the channel limit)".into();
            return false;
        }
        self.snapshot();
        let name = format!("Track {}", at + 1);
        self.track_insert(at, &name)
    }

    /// move track `from` to index `to` (header drag; undoable). The
    /// channel's pattern columns move with it, so playback is the same.
    pub(super) fn track_move(&mut self, from: usize, to: usize) {
        self.snapshot();
        let ok = self.with(|s| {
            let ok = crate::chanops::move_channel(&mut s.song, from, to);
            if ok && s.engine.playing {
                // the channels' running voices belong to the old order
                s.engine.ensure_plan(&s.song);
            }
            ok
        });
        if !ok {
            return;
        }
        // the cursor channel follows the track it was on
        let c = self.ch;
        self.ch = if c == from {
            to
        } else if from < c && c <= to {
            c - 1
        } else if to <= c && c < from {
            c + 1
        } else {
            c
        };
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        self.status = format!("Moved track {:02} to {:02}", from + 1, to + 1);
    }
}
