//! The transport's Loop toggle (Del, 25 Oct 2026: "we need a playloop
//! symbol"): the Timeline's loop, in step with L, the LCD's LOOP pill and
//! the Timeline toolbar's Loop; its tip names the region.
use super::ui_tests::{frame, harness_with, key};
use super::*;
use egui::Key;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn looped(h: &Harness<'static, App>) -> bool {
    h.state()
        .with(|s| s.song.timeline.as_deref().is_some_and(|t| t.loop_on))
}

/// the Loop button's label (its tip, which says on / off and the bars)
fn loop_label(h: &Harness<'static, App>) -> String {
    let n = h
        .query_all_by(|n| n.label().is_some_and(|l| l.starts_with("Loop (L)")))
        .next()
        .expect("the transport's Loop button");
    n.accesskit_node().label().unwrap_or_default().to_string()
}

fn click_loop(h: &mut Harness<'static, App>) {
    h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with("Loop (L)")))
        .next()
        .unwrap()
        .click();
    frames(h, 2);
}

#[test]
fn the_transport_loop_button_is_the_timelines_loop() {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.set_size(egui::vec2(1366.0, 768.0));
    frames(&mut h, 3);
    // no region yet: the existing behaviour (it says how to set one)
    assert!(
        loop_label(&h).contains("no loop region"),
        "{}",
        loop_label(&h)
    );
    click_loop(&mut h);
    assert!(!looped(&h));
    assert!(
        h.state().status.starts_with("No loop region"),
        "{}",
        h.state().status
    );
    // bars 2–3
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        t.loop_range = Some((4 * crate::timeline::PPQ, 3 * 4 * crate::timeline::PPQ));
        t.loop_on = false;
    });
    frames(&mut h, 2);
    assert_eq!(loop_label(&h), "Loop (L): off · bars 2–3");
    click_loop(&mut h);
    assert!(looped(&h), "the button switches the Timeline's loop on");
    assert_eq!(loop_label(&h), "Loop (L): on · bars 2–3");
    assert_eq!(h.state().status, "Loop on");
    // the LCD's LOOP pill is the same switch
    if h.state().lcd_on() {
        h.get_by_label("LOOP").click();
        frames(&mut h, 2);
        assert!(!looped(&h));
        assert_eq!(loop_label(&h), "Loop (L): off · bars 2–3");
        click_loop(&mut h);
        assert!(looped(&h));
    }
    // the Timeline toolbar's Loop
    h.get_by_label("Loop").click();
    frames(&mut h, 2);
    assert!(!looped(&h));
    assert!(loop_label(&h).contains(": off"));
    // L
    key(&mut h, Key::L, egui::Modifiers::NONE);
    assert!(looped(&h), "{}", h.state().status);
    assert!(loop_label(&h).contains(": on"));
    // one undo step, as the header's Loop
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(!looped(&h));
}
