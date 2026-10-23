//! The calmer layout (docs/TIMELINE.md "Layout"): a slim toolbar, tracker
//! controls only in the Tracker view, ORDER + TRACKER last, the lower-panel
//! tab order and Song › Back to tracker mode…
use super::ui_tests::{frame, harness_with, key};
use super::viewbar::{VTab, VTABS};
use super::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn has(h: &Harness<'static, App>, text: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(text))
        .next()
        .is_some()
}

fn tl_demo() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    h
}

#[test]
fn view_tabs_end_with_order_and_tracker() {
    let tabs: Vec<VTab> = VTABS.iter().map(|t| t.0).collect();
    assert_eq!(tabs.first(), Some(&VTab::Timeline));
    assert_eq!(&tabs[tabs.len() - 2..], &[VTab::Arranger, VTab::Tracker]);
    let names: Vec<&str> = VTABS.iter().map(|t| t.1).collect();
    assert!(names.contains(&"ORDER") && !names.contains(&"ARRANGER"));
}

#[test]
fn tracker_controls_show_in_the_tracker_only_and_keys_still_work() {
    let mut h = tl_demo();
    assert!(!has(&h, "Play block"), "no Play block on the Timeline");
    assert!(!has(&h, "Edit mode off") && !has(&h, "Edit mode ON"));
    // one Play / Stop toggle: Play while stopped
    assert!(has(&h, "Play song") || has(&h, "Play"));
    assert!(!has(&h, "Stop"), "no separate Stop button while stopped");
    // the octave key still works outside the Tracker
    let o = h.state().octave;
    key(&mut h, Key::F2, egui::Modifiers::NONE);
    frames(&mut h, 1);
    assert_ne!(h.state().octave, o, "F-key octave works on the Timeline");
    // the toolbar has no view switch while the view tab bar is on
    assert!(h.state().extra.view_bar);
    assert!(!has(&h, "Mixer"), "the view tab bar switches views");
    h.state_mut().view = View::Pattern;
    frames(&mut h, 2);
    assert!(has(&h, "Play block"));
    assert!(has(&h, "Edit mode off") || has(&h, "Edit mode ON"));
    // with the view tab bar hidden the toolbar's view switch comes back
    h.state_mut().extra.view_bar = false;
    frames(&mut h, 2);
    assert!(has(&h, "Mixer") && has(&h, "Tracker") && has(&h, "Order list"));
}

#[test]
fn quiet_header_and_back_to_tracker_mode_in_the_song_menu() {
    let mut h = tl_demo();
    assert!(!has(&h, "Timeline mode") && !has(&h, "Back to tracker mode…"));
    h.get_by_label("Song").click();
    frames(&mut h, 2);
    h.get_by_label("Back to tracker mode…").click();
    frames(&mut h, 2);
    assert!(h.state().tlv.ask_leave, "it asks first");
    assert_eq!(h.state().view, View::Timeline);
}

#[test]
fn lower_panel_notes_rack_then_order_list_and_automation() {
    let h = tl_demo();
    let x = |t: &str| {
        h.query_all_by(|n| n.label().as_deref() == Some(t))
            .map(|n| n.rect().left())
            .fold(f32::NAN, f32::min)
    };
    // Notes | Rack; the Order list and Automation sit after them (Tracker)
    let xs = ["Notes", "Rack", "Order list", "Automation"].map(x);
    assert!(xs.iter().all(|v| v.is_finite()), "{xs:?}");
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
}

#[test]
fn play_and_stop_are_one_toggle() {
    let mut h = tl_demo();
    let play = if h.state().tracker_on() {
        "Play song"
    } else {
        "Play"
    };
    assert!(has(&h, play) && !has(&h, "Stop"));
    h.get_by_label(play).click();
    frames(&mut h, 2);
    assert!(h.state().with(|s| s.engine.playing), "Play plays");
    assert!(has(&h, "Stop") && !has(&h, play), "it turns into Stop");
    h.get_by_label("Stop").click();
    frames(&mut h, 2);
    assert!(!h.state().with(|s| s.engine.playing), "Stop stops");
    assert!(has(&h, play), "and back to Play");
    // Space still toggles
    key(&mut h, Key::Space, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().with(|s| s.engine.playing), "Space plays");
    assert!(has(&h, "Stop"));
    key(&mut h, Key::Space, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(!h.state().with(|s| s.engine.playing), "Space stops");
}
