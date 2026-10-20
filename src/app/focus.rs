//! Focus on the Timeline and the piano roll (Del, 20 Oct 2026). With
//! Settings › Show Tracker (experimental) off - the default - the TRACKER
//! and ORDER tabs and the tracker-mode toggles are hidden. The Tracker is
//! still a specialised view: View › Tracker, Ctrl+1 and Ctrl+2 open it.
//! Its code and the file format are unchanged; tracker songs open in
//! timeline mode through the existing lossless conversion (`convert_on_open`).
use super::*;

impl App {
    /// where Settings and dialogs go back to: the Timeline, or the Tracker
    /// when it is shown
    pub(super) fn home_view(&self) -> View {
        if self.tracker_on() {
            View::Pattern
        } else {
            View::Timeline
        }
    }

    /// the views the View menu and the toolbar offer: with the Tracker
    /// hidden, the Tracker stays in the menu (a specialised view) and the
    /// Order list goes (Ctrl+2 still opens it)
    pub(super) fn views_shown(&self) -> Vec<(View, &'static str, &'static str)> {
        super::topbar::VIEWS
            .iter()
            .copied()
            .filter(|(v, _, _)| self.tracker_on() || *v != View::Arranger)
            .collect()
    }
}

impl App {
    /// screenshots of the focus pass (docs, review): the demo as it opens
    /// now (timeline mode, Tracker hidden). focus-playing: playing from the
    /// middle of the song; focus-chord: a Tracker clip turned into a note
    /// clip with chords in the piano roll; focus-prompt: the save prompt;
    /// focus-window: the Mixer, like Del's screenshot, cleaned up
    pub(super) fn shot_focus(&mut self, view: &str) {
        use crate::timeline as tl;
        self.extra.show_tracker = false;
        self.extra.browser_open = false;
        self.psetup = None;
        self.with(|s| {
            super::file::convert_on_open(&mut s.song, true);
            let Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.default_layout();
        let bar = 4 * tl::PPQ;
        let end = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.clips.iter().map(|c| c.end()).max())
                .unwrap_or(0)
        });
        self.tlv.cue = (end / 2) / bar * bar;
        match view {
            "focus-playing" => self.play(0),
            "focus-window" => self.view = View::Mixer,
            "focus-prompt" => {
                self.dirty = true;
                self.prompt = Some(crate::prompt::Pending::Quit);
            }
            "focus-chord" => {
                // a Tracker clip on a pad track, as a note clip
                // with a triad on each of its first notes
                let id = self.with(|s| {
                    let t = s.song.timeline.as_deref()?;
                    let (id, b, col) = t
                        .clips
                        .iter()
                        .filter(|c| c.track == 5 && c.start > 0)
                        .chain(t.clips.iter().filter(|c| c.track >= 2))
                        .find_map(|c| c.block().map(|(b, col)| (c.id, b, col)))?;
                    let data = tl::block_col_to_notes(&mut s.song, b, col)?;
                    let t = s.song.timeline.as_deref_mut()?;
                    let d = t.notes.get_mut(data)?.make_mut();
                    let firsts: Vec<tl::Note> = d.notes.iter().take(4).cloned().collect();
                    for n in firsts {
                        for iv in [4u8, 7] {
                            d.notes.push(tl::Note {
                                key: (n.key + iv).min(120),
                                ..n.clone()
                            });
                        }
                    }
                    let Shared { song, engine, .. } = s;
                    engine.ensure_plan(song);
                    Some(id)
                });
                if let Some(id) = id {
                    self.tlv.sel = Some(id);
                    self.tlv.sels = vec![id];
                    let tr = self.with(|s| {
                        s.song.timeline.as_deref().and_then(|t| {
                            t.clips
                                .iter()
                                .find(|c| c.id == id)
                                .map(|c| (c.track as usize, c.start))
                        })
                    });
                    if let Some((tr, start)) = tr {
                        self.ch = tr;
                        self.tlv.cue = start;
                    }
                    self.timeline_open_roll();
                    self.status =
                        "Chord: this Tracker clip is a note clip now (Ctrl+Z puts it back)".into();
                }
            }
            v if v.starts_with("record-") => self.shot_record(v),
            _ => {}
        }
    }
}
