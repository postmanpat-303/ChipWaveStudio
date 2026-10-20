//! Timeline mode (M2) UI tests: the switch prompt and its undo, dragging
//! clips (move, Alt-copy, edges), S to split, the loop region on the ruler,
//! and the play marker.
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline::{Tick, PPQ, ROW};
use egui_kittest::Harness;

pub(super) fn with_mods(h: &mut Harness<'static, App>, m: egui::Modifiers, e: egui::Event) {
    h.input_mut().events.push(egui::Event::ModifiersChanged(m));
    h.input_mut().events.push(e);
    frame(h);
}

/// a real mouse drag with modifiers held throughout
pub(super) fn drag_m(h: &mut Harness<'static, App>, from: Pos2, to: Pos2, m: egui::Modifiers) {
    with_mods(h, m, egui::Event::PointerMoved(from));
    let btn = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: m,
    };
    with_mods(h, m, btn(from, true));
    for k in 1..=8 {
        let p = from + (to - from) * (k as f32 / 8.0);
        with_mods(h, m, egui::Event::PointerMoved(p));
    }
    with_mods(h, m, btn(to, false));
    with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(to));
    for _ in 0..3 {
        frame(h);
    }
}

pub(super) fn click_at(h: &mut Harness<'static, App>, p: Pos2) {
    with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    for pressed in [true, false] {
        with_mods(
            h,
            egui::Modifiers::NONE,
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        );
    }
    for _ in 0..3 {
        frame(h);
    }
}

fn timeline(h: &Harness<'static, App>) -> crate::timeline::Timeline {
    h.state()
        .with(|s| s.song.timeline.as_deref().cloned())
        .expect("timeline mode")
}

/// the demo on the Timeline view, `mode`: switched to timeline mode, at
/// 40 px a beat from bar 1
fn setup(mode: bool) -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    assert_eq!(h.state().view, View::Timeline);
    if mode {
        h.state_mut().enter_timeline();
        h.state_mut().undo_reset();
    }
    h.state_mut().tlv.zoom = 40.0;
    h.state_mut().tlv.scroll = 0.0;
    h.state_mut().tlv.snap = super::timeline::Snap::Bar;
    frame(&mut h);
    frame(&mut h);
    h
}

/// a clip on track 1 that is at least 2 bars long and starts on screen
fn some_clip(h: &Harness<'static, App>) -> crate::timeline::Clip {
    let c = h.state().tlv.cache.as_ref().unwrap();
    c.tl.clips
        .iter()
        .find(|c| {
            c.track == 0
                && c.len >= 8 * PPQ
                && h.state().tlv.lanes.contains(Pos2::new(
                    h.state().tlv.x_of(c.start) + 30.0,
                    h.state().tlv.lanes.top() + 10.0,
                ))
        })
        .expect("a clip on screen")
        .clone()
}

fn mid(h: &Harness<'static, App>, id: u32) -> Pos2 {
    h.state().tlv.clip_rect(id).unwrap().center()
}

#[test]
fn an_edit_in_tracker_mode_asks_to_switch_and_undo_takes_it_back() {
    let mut h = setup(false);
    let cl = some_clip(&h);
    let p = mid(&h, cl.id);
    click_at(&mut h, p);
    assert_eq!(h.state().tlv.sels, vec![cl.id]);
    // M mutes: first the prompt
    key(&mut h, Key::M, egui::Modifiers::NONE);
    assert!(h.state().tlv.ask_switch.is_some());
    assert!(h.state().with(|s| s.song.timeline.is_none()));
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().tlv.ask_switch.is_none());
    let t = timeline(&h);
    assert!(
        t.clips.iter().find(|c| c.id == cl.id).unwrap().muted,
        "the edit ran after the switch"
    );
    // undo 1: the mute; undo 2: the switch
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert!(
        !timeline(&h)
            .clips
            .iter()
            .find(|c| c.id == cl.id)
            .unwrap()
            .muted
    );
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert!(
        h.state().with(|s| s.song.timeline.is_none()),
        "back in tracker mode"
    );
    // redo brings timeline mode back
    key(
        &mut h,
        Key::Z,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    assert!(h.state().with(|s| s.song.timeline.is_some()));
}

#[test]
fn dragging_moves_by_snapped_bars_and_alt_drag_copies() {
    let mut h = setup(true);
    let cl = some_clip(&h);
    let bar_px = h.state().tlv.x_of(4 * PPQ) - h.state().tlv.x_of(0);
    let p = mid(&h, cl.id);
    // a bar and a bit: snaps to one bar
    drag_m(
        &mut h,
        p,
        p + egui::vec2(bar_px + 9.0, 0.0),
        egui::Modifiers::NONE,
    );
    let t = timeline(&h);
    let moved = t.clips.iter().find(|c| c.id == cl.id).unwrap();
    assert_eq!(moved.start, cl.start + 4 * PPQ, "{}", h.state().status);
    // one undo step for the whole drag
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(
        timeline(&h)
            .clips
            .iter()
            .find(|c| c.id == cl.id)
            .unwrap()
            .start,
        cl.start
    );
    // Alt-drag one bar: a copy there, the original stays
    let n = timeline(&h).clips.len();
    let p = mid(&h, cl.id);
    drag_m(
        &mut h,
        p,
        p + egui::vec2(bar_px * 2.0, 0.0),
        egui::Modifiers::ALT,
    );
    let t = timeline(&h);
    assert_eq!(t.clips.len(), n + 1);
    assert_eq!(
        t.clips.iter().find(|c| c.id == cl.id).unwrap().start,
        cl.start
    );
    let copy = t.clips.iter().find(|c| c.id >= t.next_id - 1).unwrap();
    assert_eq!(
        (copy.start, copy.kind.clone()),
        (cl.start + 8 * PPQ, cl.kind.clone())
    );
}

#[test]
fn edges_trim_and_loop_and_s_splits_under_the_mouse() {
    let mut h = setup(true);
    // 12 px a beat: the whole clip and 4 more bars fit on screen
    h.state_mut().tlv.zoom = 12.0;
    frame(&mut h);
    let cl = some_clip(&h);
    let r = h.state().tlv.clip_rect(cl.id).unwrap();
    assert!(r.right() + 4.0 * 48.0 < h.state().tlv.lanes.right());
    let bar_px = h.state().tlv.x_of(4 * PPQ) - h.state().tlv.x_of(0);
    // right edge out by 4 bars: longer, looping when past its block
    let e = Pos2::new(r.right() - 2.0, r.center().y);
    drag_m(
        &mut h,
        e,
        e + egui::vec2(bar_px * 4.0, 0.0),
        egui::Modifiers::NONE,
    );
    let c = timeline(&h)
        .clips
        .iter()
        .find(|c| c.id == cl.id)
        .unwrap()
        .clone();
    assert_eq!(c.len, cl.len + 16 * PPQ, "{}", h.state().status);
    assert!(c.loop_len.is_some());
    // left edge in by a bar: trimmed
    let s = Pos2::new(r.left() + 2.0, r.center().y);
    drag_m(
        &mut h,
        s,
        s + egui::vec2(bar_px, 0.0),
        egui::Modifiers::NONE,
    );
    let c2 = timeline(&h)
        .clips
        .iter()
        .find(|c| c.id == cl.id)
        .unwrap()
        .clone();
    assert_eq!((c2.start, c2.end()), (c.start + 4 * PPQ, c.end()));
    // S with the mouse inside the clip, a bar in: the split preview there;
    // the click splits it into two clips
    let n = timeline(&h).clips.len();
    let at = h.state().tlv.x_of(c2.start + 4 * PPQ);
    with_mods(
        &mut h,
        egui::Modifiers::NONE,
        egui::Event::PointerMoved(Pos2::new(at + 1.0, r.center().y)),
    );
    frame(&mut h);
    key(&mut h, Key::S, egui::Modifiers::NONE);
    assert_eq!(timeline(&h).clips.len(), n, "S alone only shows where");
    assert_eq!(h.state().tlv.split_at, Some((cl.id, c2.start + 4 * PPQ)));
    click_at(&mut h, Pos2::new(at + 1.0, r.center().y));
    let t = timeline(&h);
    assert_eq!(t.clips.len(), n + 1, "{}", h.state().status);
    assert_eq!(
        t.clips.iter().find(|c| c.id == cl.id).unwrap().end(),
        c2.start + 4 * PPQ
    );
}

#[test]
fn ruler_click_moves_the_play_marker_and_shift_drag_sets_the_loop() {
    let mut h = setup(true);
    let ruler = h.state().tlv.ruler;
    let x = |h: &Harness<'static, App>, t: Tick| h.state().tlv.x_of(t);
    // a click near bar 3 snaps to it
    let p = Pos2::new(x(&h, 8 * PPQ) + 6.0, ruler.center().y);
    click_at(&mut h, p);
    assert_eq!(h.state().tlv.cue, 8 * PPQ);
    // Space from there: the engine starts on that timeline row
    h.state_mut().with(|s| s.engine.stop());
    key(&mut h, Key::Space, egui::Modifiers::NONE);
    let row = h.state().with(|s| s.engine.tl_row());
    assert!(
        row.is_some_and(|r| r >= (8 * PPQ / ROW) as usize),
        "{row:?}"
    );
    h.state_mut().with(|s| s.engine.stop());
    // Shift-drag bar 2 -> bar 4: the loop region, switched on
    let a = Pos2::new(x(&h, 4 * PPQ) + 3.0, ruler.center().y);
    let b = Pos2::new(x(&h, 12 * PPQ) - 3.0, ruler.center().y);
    drag_m(&mut h, a, b, egui::Modifiers::SHIFT);
    let t = timeline(&h);
    assert_eq!(t.loop_range, Some((4 * PPQ, 12 * PPQ)));
    assert!(t.loop_on);
    // L toggles it
    key(&mut h, Key::L, egui::Modifiers::NONE);
    assert!(!timeline(&h).loop_on);
}

#[test]
fn ctrl_d_duplicates_and_delete_removes_the_selection() {
    let mut h = setup(true);
    let cl = some_clip(&h);
    let p = mid(&h, cl.id);
    click_at(&mut h, p);
    let n = timeline(&h).clips.len();
    key(&mut h, Key::D, egui::Modifiers::COMMAND);
    assert_eq!(timeline(&h).clips.len(), n + 1);
    let copy = h.state().tlv.sels.clone();
    assert_eq!(copy.len(), 1);
    assert_ne!(copy[0], cl.id, "the copy is selected");
    key(&mut h, Key::Delete, egui::Modifiers::NONE);
    assert_eq!(timeline(&h).clips.len(), n);
    assert!(timeline(&h).clips.iter().any(|c| c.id == cl.id));
    // Ctrl+Shift+D: make unique (its own block)
    let p = mid(&h, cl.id);
    click_at(&mut h, p);
    let nb = h.state().with(|s| s.song.patterns.len());
    key(
        &mut h,
        Key::D,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    assert_eq!(h.state().with(|s| s.song.patterns.len()), nb + 1);
    let c = timeline(&h)
        .clips
        .iter()
        .find(|c| c.id == cl.id)
        .unwrap()
        .clone();
    assert_eq!(c.block().unwrap().0, nb);
}
