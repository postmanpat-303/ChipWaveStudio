//! Double-click a clip on the Timeline: the loop exactly around it, shown on
//! the bar ruler; again: loop off (Del, 25 Oct 2026: "if i double click a
//! block it should create a highlight loop marker on the bar timeline").
//! Alt+double-click, Enter and the pie still open it.
use super::ui_tests::{double_click, double_click_with, frame, harness_with, key};
use super::*;
use egui::{Key, Pos2};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn loop_state(h: &Harness<'static, App>) -> (bool, Option<(i64, i64)>) {
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        (t.loop_on, t.loop_range.map(|(a, b)| (a as i64, b as i64)))
    })
}

fn loop_label(h: &Harness<'static, App>) -> String {
    let n = h
        .query_all_by(|n| n.label().is_some_and(|l| l.starts_with("Loop (L)")))
        .next()
        .expect("the transport's Loop button");
    n.accesskit_node().label().unwrap_or_default().to_string()
}

#[test]
fn double_click_loops_the_clip_and_again_turns_it_off() {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.set_size(egui::vec2(1366.0, 768.0));
    frames(&mut h, 3);
    // a block clip past the start, on screen
    let (id, start, end) = {
        let c = h.state().tlv.cache.as_ref().unwrap();
        let cl =
            c.tl.clips
                .iter()
                .find(|cl| cl.start > 0 && cl.block().is_some())
                .expect("a clip");
        (cl.id, cl.start, cl.end())
    };
    h.state_mut().tlv.scroll = 0.0;
    frames(&mut h, 2);
    let r = h.state().tlv.clip_rect(id).expect("drawn");
    let p = Pos2::new(r.center().x, r.center().y);
    assert!(h.state().tlv.lanes.contains(p), "{r:?}");
    let view = h.state().view;
    double_click(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(loop_state(&h), (true, Some((start as i64, end as i64))));
    let st = h.state().status.clone();
    assert!(st.starts_with("Loop set to bar"), "{st}");
    assert_eq!(h.state().view, view, "a double-click no longer leaves");
    // the transport's Loop button agrees
    assert!(loop_label(&h).contains(": on"), "{}", loop_label(&h));
    // again: loop off, the region kept
    double_click(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(loop_state(&h), (false, Some((start as i64, end as i64))));
    assert!(
        h.state().status.starts_with("Loop off"),
        "{}",
        h.state().status
    );
    assert!(loop_label(&h).contains(": off"));
    // L brings it back, the button follows
    key(&mut h, Key::L, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert_eq!(loop_state(&h).0, true);
    assert!(loop_label(&h).contains(": on"));
    // the button turns it off; a double-click turns it on again
    h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with("Loop (L)")))
        .next()
        .unwrap()
        .click();
    frames(&mut h, 2);
    assert_eq!(loop_state(&h).0, false);
    double_click(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(loop_state(&h), (true, Some((start as i64, end as i64))));
    // one undo step each
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(loop_state(&h).0, false, "Ctrl+Z undoes the double-click");
    // Alt+double-click still opens the clip
    double_click_with(&mut h, p, egui::Modifiers::ALT);
    frames(&mut h, 2);
    assert_eq!(loop_state(&h).0, false, "Alt+double-click leaves the loop");
    assert!(
        h.state().status.contains("Piano roll") || h.state().view == View::Pattern,
        "Alt+double-click opens the editor: {}",
        h.state().status
    );
}

#[test]
fn a_double_click_on_another_clip_moves_the_loop_to_it() {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.set_size(egui::vec2(1366.0, 768.0));
    frames(&mut h, 3);
    let clips: Vec<(u32, i64, i64)> = {
        let c = h.state().tlv.cache.as_ref().unwrap();
        c.tl.clips
            .iter()
            .filter(|cl| cl.block().is_some())
            .map(|cl| (cl.id, cl.start as i64, cl.end() as i64))
            .collect()
    };
    let a = clips[0];
    let b = *clips.iter().find(|c| (c.1, c.2) != (a.1, a.2)).unwrap();
    for (id, s0, e0) in [a, b] {
        let r = h.state().tlv.clip_rect(id).unwrap();
        if !h.state().tlv.lanes.contains(r.center()) {
            continue;
        }
        double_click(&mut h, r.center());
        frames(&mut h, 2);
        assert_eq!(loop_state(&h), (true, Some((s0, e0))));
    }
}
