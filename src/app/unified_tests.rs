//! The unified workflow (docs/unified/PLAN.md): Stage 2, the Timeline stays
//! on top. The sample editor, Open in Tracker and the effects open under
//! it; the lower panel leaves at least three track rows (1366x768) and the
//! current track scrolls into view; the panel header names what it edits.
use super::ui_tests::{click_at, exact, frame, harness_with, loop_instrument};
use super::*;
use egui_kittest::Harness;

pub(super) fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// `song` in timeline mode on the Timeline, the Tracker hidden (the default)
pub(super) fn tl_song(song: Song) -> Harness<'static, App> {
    let mut s = song;
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().extra.show_tracker = false;
    h.state_mut().default_layout();
    frames(&mut h, 3);
    h
}

pub(super) fn tl_demo() -> Harness<'static, App> {
    tl_song(fileio::demo_song())
}

#[allow(dead_code)]
pub(super) fn laptop(h: &mut Harness<'static, App>) {
    h.set_size(egui::vec2(1366.0, 768.0));
    frames(h, 3);
}

#[allow(dead_code)]
fn lanes_rows(h: &Harness<'static, App>) -> f32 {
    h.state().tlv.lanes.height() / h.state().tlv.track_h_pts()
}

#[test]
fn open_in_sample_editor_keeps_the_timeline_on_top() {
    let mut s = fileio::demo_song();
    s.instruments.push(loop_instrument());
    let slot = s.instruments.len() as u8;
    let mut h = tl_song(s);
    h.state_mut().instr = slot;
    h.state_mut().show_instrument_panel();
    frames(&mut h, 3);
    let r = exact(&h, "Open in Sample editor");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    assert_eq!(h.state().view, View::Timeline, "the timeline stays");
    assert!(h.state().ip.sample && h.state().sample_shown());
    assert!(h.state().tlv.lanes.height() > 0.0, "the timeline is drawn");
    exact(&h, "Sample editor");
    // the sample editor's own tools work there (Transients slices it)
    let r = exact(&h, "Transients");
    click_at(&mut h, r.center());
    assert_eq!(h.state().smp.slices.len(), 8, "{:?}", h.state().smp.slices);
    // Ctrl+Z over the sample editor uses its history, elsewhere the song's
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(Pos2::new(700.0, 20.0)));
    frames(&mut h, 2);
    assert!(!h.state().sample_undo_here(), "pointer on the menu bar");
    let r = exact(&h, "Sample editor");
    h.input_mut().events.push(egui::Event::PointerMoved(r.center()));
    frames(&mut h, 2);
    assert!(h.state().sample_undo_here(), "pointer over the sample editor");
    // back to the knobs, and Full view on request
    let r = exact(&h, "Back to the knobs");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert!(!h.state().ip.sample && !h.state().sample_shown());
    h.state_mut().ip.sample = true;
    frames(&mut h, 2);
    let r = exact(&h, "Full view  Ctrl+5");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Sample);
}

#[test]
fn without_the_tracker_tracker_clips_open_in_the_piano_roll() {
    let mut h = tl_demo();
    assert!(!h.state().tracker_on());
    h.state_mut().set_lower(1);
    h.state_mut().timeline_open(2, 1, 0, usize::MAX);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Timeline, "the timeline stays");
    assert!(h.state().pr.open, "the piano roll shows the clip");
    assert_eq!((h.state().pattern, h.state().ch), (2, 1));
    // with the Tracker on, Open in Tracker still opens the Tracker
    h.state_mut().extra.show_tracker = true;
    h.state_mut().timeline_open(2, 1, 0, usize::MAX);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Pattern);
}
