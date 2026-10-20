//! Focus on the Timeline and the piano roll (Del, 20 Oct 2026): with
//! Settings › Show Tracker (experimental) off - the default - the Tracker
//! and Order list views stay out of the way. Their code, keys (when shown)
//! and the file format are unchanged; tracker songs open in timeline mode
//! (the lossless, bit-identical conversion) and play from the Timeline.
use super::*;

impl App {
    /// keep the hidden Tracker hidden: any way into the Tracker or Order
    /// list view (an old shortcut, a dialog closing back to it, a block
    /// opened from somewhere) lands on the Timeline, and a tracker-mode
    /// song switches to timeline mode. Screenshots are left alone.
    pub(super) fn tracker_guard(&mut self) {
        if self.tracker_on() || self.shot.is_some() {
            return;
        }
        if matches!(self.view, View::Pattern | View::Arranger) {
            self.view = View::Timeline;
            self.arr_focus = false;
        }
        self.extra.adv_edit_open = false;
        self.with(|s| {
            if super::file::convert_on_open(&mut s.song, true) {
                let Shared { song, engine, .. } = s;
                engine.ensure_plan(song);
            }
        });
    }

    /// the views the View menu and toolbar offer
    pub(super) fn views_shown(&self) -> Vec<(View, &'static str, &'static str)> {
        super::topbar::VIEWS
            .iter()
            .copied()
            .filter(|(v, _, _)| self.tracker_on() || !matches!(v, View::Pattern | View::Arranger))
            .collect()
    }

    /// Ctrl+1 / Ctrl+2 with the Tracker hidden
    pub(super) fn tracker_hidden_note(&mut self) {
        self.status = "The Tracker is hidden: Settings › Show Tracker (experimental)".into();
    }
}
