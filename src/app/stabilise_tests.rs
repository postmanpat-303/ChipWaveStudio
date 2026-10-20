//! Stabilisation (docs/TIMELINE.md, Del's brief of 20 Oct 2026): regression
//! tests for the inconsistencies found by the audit. Opening another song
//! forgets the old selection; the clipboard only edits the Tracker where it
//! shows; the keys go where the user clicked; the unsaved mark means the
//! song really changed.
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline as tl;
use egui::Key;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn song<R>(h: &Harness<'static, App>, f: impl FnOnce(&Song) -> R) -> R {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    f(&g.song)
}

fn clips(h: &Harness<'static, App>) -> usize {
    song(h, |s| s.timeline.as_ref().map_or(0, |t| t.clips.len()))
}

/// demo `i` opened the normal way (timeline mode, not dirty) on the
/// Timeline view, with the keyboard on the Timeline (as after a click there)
fn opened_demo(i: usize) -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().load_demo(i);
    h.state_mut().view = View::Timeline;
    h.state_mut().pr.focus = false;
    frames(&mut h, 4);
    assert!(song(&h, |s| s.timeline.is_some()), "demo opens in timeline mode");
    assert!(!h.state().dirty, "opening is not an edit");
    h
}

#[test]
fn opening_another_song_forgets_the_old_selection() {
    let mut h = opened_demo(0);
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    assert!(!h.state().tlv.sels.is_empty(), "Ctrl+A selected clips");
    h.state_mut().pr.sel = vec![0, 1];
    h.state_mut().load_demo(1);
    h.state_mut().view = View::Timeline;
    h.state_mut().pr.focus = false;
    frames(&mut h, 4);
    assert!(h.state().tlv.sels.is_empty() && h.state().tlv.sel.is_none());
    assert!(h.state().pr.sel.is_empty());
    // so Delete right after opening removes nothing
    let n = clips(&h);
    key(&mut h, Key::Delete, egui::Modifiers::NONE);
    assert_eq!(clips(&h), n, "Delete removed clips of the newly opened song");
    assert!(!h.state().dirty);
}
