//! The top bar's three movable panels (Display, Transport, Master): the
//! saved order, where each one sits (the first at the left edge, the middle
//! one centred on the window, the last at the right edge), the compact
//! fallback and drag to reorder. The drawing is in topbar.rs.

use eframe::egui;

/// one of the top bar's panels
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Part {
    /// the LCD display (the classic toolbar: BPM and Speed)
    Display,
    /// play, stop, record, loop, metronome
    Transport,
    /// the compact Master meter
    Master,
}

impl Part {
    pub(super) const ALL: [Part; 3] = [Part::Display, Part::Transport, Part::Master];

    pub(super) fn key(self) -> &'static str {
        match self {
            Part::Display => "display",
            Part::Transport => "transport",
            Part::Master => "master",
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Part::Display => "Display",
            Part::Transport => "Transport",
            Part::Master => "Master",
        }
    }

    pub(super) fn idx(self) -> usize {
        self as usize
    }

    fn of_key(k: &str) -> Option<Part> {
        Part::ALL.into_iter().find(|p| p.key() == k)
    }
}

/// Display at the left, Transport centred, Master at the right
pub(super) const DEFAULT: [Part; 3] = [Part::Display, Part::Transport, Part::Master];

/// the saved order (Settings: `topbar`); anything but the three panels once
/// each gives the default
pub(super) fn order_of(saved: &[String]) -> [Part; 3] {
    let parts: Vec<Part> = saved.iter().filter_map(|k| Part::of_key(k)).collect();
    if parts.len() == 3 && Part::ALL.iter().all(|p| parts.contains(p)) {
        [parts[0], parts[1], parts[2]]
    } else {
        DEFAULT
    }
}

/// the order as saved; the default saves as nothing
pub(super) fn keys_of(o: [Part; 3]) -> Vec<String> {
    if o == DEFAULT {
        vec![]
    } else {
        o.iter().map(|p| p.key().to_string()).collect()
    }
}

/// "Display · Transport · Master"
pub(super) fn names(o: [Part; 3]) -> String {
    o.map(|p| p.name()).join(" · ")
}

/// space between the window edge and the outer panels
pub(super) const EDGE: f32 = 6.0;
/// the grip's room to the left of each panel (shown on hover)
pub(super) const GRIP: f32 = 14.0;
/// the least space between two panels (plus the grip's)
pub(super) const MIN_GAP: f32 = 8.0;

/// each slot's left x for panels `w` wide (0: not shown, its slot stays):
/// the first at the left edge (after its grip), the middle one centred on
/// `mid` (the window's middle), the last at the right edge
pub(super) fn slots(left: f32, right: f32, mid: f32, w: [f32; 3]) -> [f32; 3] {
    [left + EDGE + GRIP, mid - w[1] / 2.0, right - EDGE - w[2]]
}

/// the panels fit side by side without touching
pub(super) fn fits(left: f32, right: f32, mid: f32, w: [f32; 3]) -> bool {
    let x = slots(left, right, mid, w);
    x[0] + w[0] + MIN_GAP + GRIP <= x[1] && x[1] + w[1] + MIN_GAP + GRIP <= x[2]
}

/// the two free gaps (left of the middle panel, right of it) as x ranges
pub(super) fn gaps(left: f32, right: f32, mid: f32, w: [f32; 3]) -> [(f32, f32); 2] {
    let x = slots(left, right, mid, w);
    let end = |i: usize| if w[i] > 0.0 { x[i] - GRIP } else { x[i] };
    [(x[0] + w[0], end(1)), (x[1] + w[1], end(2))]
}

/// where a row of extra controls `we` wide goes: in the right gap, else the
/// left one (centred, its left x), or None: on a second row
pub(super) fn extras_x(gaps: [(f32, f32); 2], we: f32) -> Option<f32> {
    [gaps[1], gaps[0]]
        .into_iter()
        .find(|(a, b)| b - a >= we + 2.0 * MIN_GAP)
        .map(|(a, b)| ((a + b - we) / 2.0).round())
}

/// the drop gap under the pointer: 0 before the first slot ... 3 after the
/// last, from the slots' centres
pub(super) fn drop_gap(centres: [f32; 3], x: f32) -> usize {
    centres.iter().filter(|c| **c < x).count()
}

/// the order after dragging slot `from` to drop gap `gap` (0..=3)
pub(super) fn reorder(o: [Part; 3], from: usize, gap: usize) -> [Part; 3] {
    let mut v = o.to_vec();
    let p = v.remove(from.min(2));
    let at = if gap > from { gap - 1 } else { gap };
    v.insert(at.min(2), p);
    [v[0], v[1], v[2]]
}

/// the top bar's state between frames
pub(crate) struct TopBarUi {
    /// each panel's last drawn width and the tallest one's height (the
    /// next frame lays out with them), by `Part::idx`
    pub w: [f32; 3],
    pub h: f32,
    /// the extra controls' (Octave, Edit / Step, View) last width
    pub extras_w: f32,
    /// last drawn: each slot's panel and the extra controls (tests)
    pub rects: [egui::Rect; 3],
    pub extras: egui::Rect,
    /// the extras are on a second row
    pub second_row: bool,
    /// the slot being dragged by its grip
    pub drag: Option<usize>,
}

impl Default for TopBarUi {
    fn default() -> Self {
        Self {
            w: [0.0; 3],
            h: 0.0,
            extras_w: 0.0,
            rects: [egui::Rect::NOTHING; 3],
            extras: egui::Rect::NOTHING,
            second_row: false,
            drag: None,
        }
    }
}
