//! Timeline gestures from the arp2 round: the loop region's ends and band
//! on the ruler (resize, move, swap, snap, double-click selects) and
//! Ctrl-drag duplicating clips (independent copies, cross-track, undo).
use super::timeline_m2_tests::{click_at, drag_m, with_mods};
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline::{self as tl, ClipKind, Tick, TrackKind, PPQ, ROW};
use egui_kittest::Harness;

const BAR: Tick = 4 * PPQ;

fn timeline(h: &Harness<'static, App>) -> tl::Timeline {
    h.state()
        .with(|s| s.song.timeline.as_deref().cloned())
        .expect("timeline mode")
}

/// the demo in timeline mode, 40 px a beat from bar 1, Bar snap, a loop
/// region over bars 3-5 (ticks 2..4 bars)
fn setup() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    h.state_mut().enter_timeline();
    h.state_mut().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        t.loop_range = Some((2 * BAR, 4 * BAR));
        t.loop_on = true;
    });
    h.state_mut().undo_reset();
    h.state_mut().tlv.zoom = 40.0;
    h.state_mut().tlv.scroll = 0.0;
    h.state_mut().tlv.snap = super::timeline::Snap::Bar;
    frame(&mut h);
    frame(&mut h);
    h
}

fn x(h: &Harness<'static, App>, t: Tick) -> f32 {
    h.state().tlv.x_of(t)
}

fn loop_range(h: &Harness<'static, App>) -> Option<(Tick, Tick)> {
    timeline(h).loop_range
}

fn hover(h: &mut Harness<'static, App>, p: Pos2) {
    with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    frame(h);
}

fn cursor(h: &Harness<'static, App>) -> egui::CursorIcon {
    h.output().platform_output.cursor_icon
}

#[test]
fn loop_edge_and_shift_helpers_snap_swap_and_keep_a_minimum() {
    let g = BAR;
    // the end out by a bar
    assert_eq!(
        tl::loop_edge((2 * g, 4 * g), false, 5 * g, g),
        (2 * g, 5 * g)
    );
    // the start in, up to one grid step short of the end
    assert_eq!(
        tl::loop_edge((2 * g, 4 * g), true, 4 * g, g),
        (4 * g, 5 * g)
    );
    assert_eq!(
        tl::loop_edge((2 * g, 4 * g), true, 3 * g + ROW, g),
        (3 * g, 4 * g)
    );
    // past the other end: they swap
    assert_eq!(
        tl::loop_edge((2 * g, 4 * g), true, 6 * g, g),
        (4 * g, 6 * g)
    );
    assert_eq!(tl::loop_edge((2 * g, 4 * g), false, g, g), (g, 2 * g));
    // never before 0, never shorter than a row
    assert_eq!(tl::loop_edge((0, g), false, 0, 0), (0, ROW));
    assert_eq!(tl::loop_edge((2 * g, 4 * g), false, -5, g), (0, 2 * g));
    // move: length kept, start on the grid, not before 0
    assert_eq!(tl::loop_shift((2 * g, 4 * g), g + 100, g), (3 * g, 5 * g));
    assert_eq!(tl::loop_shift((2 * g, 4 * g), -9 * g, g), (0, 2 * g));
    assert_eq!(
        tl::loop_shift((2 * g, 4 * g), ROW, ROW),
        (2 * g + ROW, 4 * g + ROW)
    );
}

#[test]
fn dragging_the_loop_ends_resizes_it_on_the_grid_with_named_undo() {
    let mut h = setup();
    let ruler = h.state().tlv.ruler;
    let y = ruler.center().y;
    // hovering an end: the resize cursor
    let a0_ = Pos2::new(x(&h, 4 * BAR) + 2.0, y);
    hover(&mut h, a0_);
    assert_eq!(cursor(&h), egui::CursorIcon::ResizeHorizontal);
    // the end out by a bar and a bit: snaps to bar 6 (tick 5 bars)
    let e = Pos2::new(x(&h, 4 * BAR) + 1.0, y);
    let (a0_, a1_, a2_) = (e, Pos2::new(x(&h, 5 * BAR) + 9.0, y), egui::Modifiers::NONE);
    drag_m(&mut h, a0_, a1_, a2_);
    assert_eq!(
        loop_range(&h),
        Some((2 * BAR, 5 * BAR)),
        "{}",
        h.state().status
    );
    assert!(timeline(&h).loop_on, "still on");
    assert_eq!(h.state().undo_text(false), "Undo: Resize loop");
    // the play marker did not move (no seek from an edge drag)
    assert_eq!(h.state().tlv.cue, 0);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(loop_range(&h), Some((2 * BAR, 4 * BAR)));
    // Alt: by rows
    let s = Pos2::new(x(&h, 2 * BAR) - 1.0, y);
    let to = x(&h, 2 * BAR + 3 * ROW) + 1.0;
    drag_m(&mut h, s, Pos2::new(to, y), egui::Modifiers::ALT);
    assert_eq!(loop_range(&h), Some((2 * BAR + 3 * ROW, 4 * BAR)));
    // the start dragged past the end: they swap, no zero-length region
    let s = Pos2::new(x(&h, 2 * BAR + 3 * ROW), y);
    let (a0_, a1_, a2_) = (s, Pos2::new(x(&h, 6 * BAR) + 2.0, y), egui::Modifiers::NONE);
    drag_m(&mut h, a0_, a1_, a2_);
    assert_eq!(loop_range(&h), Some((4 * BAR, 6 * BAR)));
    // the start onto the end: one grid step (a bar) long
    let s = Pos2::new(x(&h, 4 * BAR) - 1.0, y);
    let (a0_, a1_, a2_) = (s, Pos2::new(x(&h, 6 * BAR) - 2.0, y), egui::Modifiers::NONE);
    drag_m(&mut h, a0_, a1_, a2_);
    let (a, b) = loop_range(&h).unwrap();
    assert!(b - a >= BAR && a >= 0, "{a} {b}");
}

#[test]
fn dragging_the_loop_band_moves_it_and_double_click_selects_its_clips() {
    let mut h = setup();
    let ruler = h.state().tlv.ruler;
    let y = ruler.bottom() - 4.0;
    let p = Pos2::new(x(&h, 3 * BAR), y);
    hover(&mut h, p);
    assert_eq!(cursor(&h), egui::CursorIcon::Grab);
    // a bar and a bit right: by one bar, the length kept
    let (a0_, a1_, a2_) = (
        p,
        p + egui::vec2(x(&h, BAR) - x(&h, 0) + 8.0, 0.0),
        egui::Modifiers::NONE,
    );
    drag_m(&mut h, a0_, a1_, a2_);
    assert_eq!(
        loop_range(&h),
        Some((3 * BAR, 5 * BAR)),
        "{}",
        h.state().status
    );
    assert_eq!(h.state().undo_text(false), "Undo: Move loop");
    assert_eq!(h.state().tlv.cue, 0, "no seek");
    // what plays: the engine wraps at the moved region
    let t = timeline(&h);
    assert_eq!(t.loop_range, Some((3 * BAR, 5 * BAR)));
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(loop_range(&h), Some((2 * BAR, 4 * BAR)));
    assert_eq!(h.state().undo_text(true), "Redo: Move loop");
    // double-click the band: the clips in the loop get selected
    let p = Pos2::new(x(&h, 3 * BAR), y);
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        frame(&mut h);
    }
    frame(&mut h);
    let t = timeline(&h);
    let mut want: Vec<u32> = t
        .clips
        .iter()
        .filter(|c| c.start < 4 * BAR && c.end() > 2 * BAR)
        .map(|c| c.id)
        .collect();
    assert!(!want.is_empty());
    let mut got = h.state().tlv.sels.clone();
    want.sort();
    got.sort();
    assert_eq!(got, want, "{}", h.state().status);
    assert!(
        h.state().status.starts_with("Selected the"),
        "{}",
        h.state().status
    );
    assert_eq!(loop_range(&h), Some((2 * BAR, 4 * BAR)), "unchanged");
}

#[test]
fn shift_drag_still_draws_a_new_loop_and_a_plain_click_seeks() {
    let mut h = setup();
    let y = h.state().tlv.ruler.center().y;
    // away from the region: a plain click seeks
    let a0_ = Pos2::new(x(&h, 6 * BAR) + 5.0, y);
    click_at(&mut h, a0_);
    assert_eq!(h.state().tlv.cue, 6 * BAR);
    let a = Pos2::new(x(&h, 5 * BAR) + 3.0, y);
    let (a0_, a1_, a2_) = (
        a,
        Pos2::new(x(&h, 7 * BAR) - 3.0, y),
        egui::Modifiers::SHIFT,
    );
    drag_m(&mut h, a0_, a1_, a2_);
    assert_eq!(loop_range(&h), Some((5 * BAR, 7 * BAR)));
    assert_eq!(h.state().undo_text(false), "Undo: Set loop");
}

/// a Tracker or note clip on track 0, at least 2 bars, on screen
fn some_clip(h: &Harness<'static, App>) -> tl::Clip {
    let c = h.state().tlv.cache.as_ref().unwrap();
    c.tl.clips
        .iter()
        .find(|c| {
            c.track == 0
                && c.len >= 2 * BAR
                && c.audio().is_none()
                && h.state()
                    .tlv
                    .clip_rect(c.id)
                    .is_some_and(|r| h.state().tlv.lanes.contains(r.center()))
        })
        .expect("a clip on screen")
        .clone()
}

fn mid(h: &Harness<'static, App>, id: u32) -> Pos2 {
    h.state().tlv.clip_rect(id).unwrap().center()
}

fn track_h(h: &Harness<'static, App>) -> f32 {
    let tv = &h.state().tlv;
    if tv.track_h > 0.0 {
        tv.track_h
    } else {
        super::timeline::DEFAULT_TRACK_H
    }
}

/// the block or notes a clip plays
fn source(k: &ClipKind) -> (u8, usize) {
    match *k {
        ClipKind::Block { pattern, .. } => (0, pattern),
        ClipKind::Notes { data } => (1, data as usize),
        ClipKind::Audio { asset } => (2, asset as usize),
    }
}

#[test]
fn ctrl_drag_makes_independent_copies_that_move_and_one_undo_step() {
    let mut h = setup();
    let cl = some_clip(&h);
    let n = timeline(&h).clips.len();
    let nb = h.state().with(|s| s.song.patterns.len());
    let p = mid(&h, cl.id);
    let bar_px = x(&h, BAR) - x(&h, 0);
    // mid-drag: the copy cursor
    let m = egui::Modifiers::COMMAND;
    with_mods(&mut h, m, egui::Event::PointerMoved(p));
    let btn = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: m,
    };
    with_mods(&mut h, m, btn(p, true));
    for k in 1..=8 {
        let q = p + egui::vec2((2.0 * bar_px + 7.0) * k as f32 / 8.0, 0.0);
        with_mods(&mut h, m, egui::Event::PointerMoved(q));
    }
    assert_eq!(cursor(&h), egui::CursorIcon::Copy);
    assert_eq!(
        timeline(&h).clips.len(),
        n + 1,
        "the copy is made at the start"
    );
    with_mods(
        &mut h,
        m,
        btn(p + egui::vec2(2.0 * bar_px + 7.0, 0.0), false),
    );
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    for _ in 0..3 {
        frame(&mut h);
    }
    let t = timeline(&h);
    assert_eq!(t.clips.len(), n + 1);
    // the original stays, the copy is 2 bars on (snapped), selected
    let orig = t.clips.iter().find(|c| c.id == cl.id).unwrap();
    assert_eq!((orig.start, orig.track), (cl.start, cl.track));
    let sels = h.state().tlv.sels.clone();
    assert_eq!(sels.len(), 1);
    assert_ne!(sels[0], cl.id);
    let copy = t.clips.iter().find(|c| c.id == sels[0]).unwrap();
    assert_eq!(
        (copy.start, copy.track, copy.len),
        (cl.start + 2 * BAR, 0, cl.len)
    );
    // independent: its own block or notes (same content)
    assert_ne!(source(&copy.kind), source(&cl.kind), "not linked");
    assert_eq!(source(&copy.kind).0, source(&cl.kind).0);
    if let ClipKind::Block { pattern, .. } = copy.kind {
        assert_eq!(h.state().with(|s| s.song.patterns.len()), nb + 1);
        let (a, b) = h.state().with(|s| {
            let ClipKind::Block { pattern: o, .. } = cl.kind else {
                unreachable!()
            };
            (s.song.patterns[o].clone(), s.song.patterns[pattern].clone())
        });
        assert_eq!(a.rows, b.rows);
    }
    assert_eq!(h.state().undo_text(false), "Undo: Duplicate clip");
    // one undo: the copy and its block go
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(timeline(&h).clips.len(), n);
    assert_eq!(h.state().with(|s| s.song.patterns.len()), nb);
    // a plain drag still moves
    let p = mid(&h, cl.id);
    drag_m(
        &mut h,
        p,
        p + egui::vec2(bar_px + 5.0, 0.0),
        egui::Modifiers::NONE,
    );
    let t = timeline(&h);
    assert_eq!(t.clips.len(), n);
    assert_eq!(
        t.clips.iter().find(|c| c.id == cl.id).unwrap().start,
        cl.start + BAR
    );
    // a Ctrl-press that does not move: nothing is left behind
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    let p = mid(&h, cl.id);
    drag_m(
        &mut h,
        p,
        p + egui::vec2(3.0, 0.0),
        egui::Modifiers::COMMAND,
    );
    assert_eq!(timeline(&h).clips.len(), n);
    assert_eq!(h.state().with(|s| s.song.patterns.len()), nb);
}

#[test]
fn ctrl_drag_copies_a_selection_across_tracks_but_not_onto_audio() {
    let mut h = setup();
    let t0 = timeline(&h);
    // two clips on track 0 selected
    let mut two: Vec<tl::Clip> = t0
        .clips
        .iter()
        .filter(|c| c.track == 0 && c.audio().is_none())
        .cloned()
        .collect();
    two.sort_by_key(|c| c.start);
    two.truncate(2);
    assert_eq!(two.len(), 2, "the demo has two clips on track 0");
    assert!(h.state().tlv.clip_rect(two[0].id).is_some_and(|r| h
        .state()
        .tlv
        .lanes
        .contains(r.center())));
    let ids: Vec<u32> = two.iter().map(|c| c.id).collect();
    h.state_mut().tlv.sels = ids.clone();
    h.state_mut().tlv.sel = Some(ids[0]);
    frame(&mut h);
    let n = t0.clips.len();
    let p = mid(&h, ids[0]);
    let th = track_h(&h);
    assert!(t0.tracks.len() > 2 && t0.tracks[1].kind != TrackKind::Audio);
    // one track down
    drag_m(&mut h, p, p + egui::vec2(0.0, th), egui::Modifiers::COMMAND);
    let t = timeline(&h);
    assert_eq!(t.clips.len(), n + 2, "{}", h.state().status);
    assert_eq!(h.state().undo_text(false), "Undo: Duplicate clips (2)");
    for c in &two {
        let o = t.clips.iter().find(|x| x.id == c.id).unwrap();
        assert_eq!((o.start, o.track), (c.start, 0), "originals stay");
    }
    let copies: Vec<&tl::Clip> = t
        .clips
        .iter()
        .filter(|c| h.state().tlv.sels.contains(&c.id))
        .collect();
    assert_eq!(copies.len(), 2);
    assert!(copies.iter().all(|c| c.track == 1), "copied a track down");
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(timeline(&h).clips.len(), n);
    // track 1 made an audio track: the copies keep their track
    h.state_mut().with(|s| {
        s.song.timeline.as_deref_mut().unwrap().tracks[1].kind = TrackKind::Audio;
    });
    // (the view's cache keys on the song's content, not track kinds)
    h.state_mut().tlv.cache = None;
    h.state_mut().tlv.sels = ids.clone();
    frame(&mut h);
    let p = mid(&h, ids[0]);
    let bar_px = x(&h, BAR) - x(&h, 0);
    drag_m(
        &mut h,
        p,
        p + egui::vec2(bar_px * 8.0, th),
        egui::Modifiers::COMMAND,
    );
    let t = timeline(&h);
    assert_eq!(t.clips.len(), n + 2);
    let copies: Vec<&tl::Clip> = t
        .clips
        .iter()
        .filter(|c| h.state().tlv.sels.contains(&c.id))
        .collect();
    assert!(
        copies.iter().all(|c| c.track == 0),
        "not onto the audio track"
    );
    assert!(copies.iter().all(|c| c.audio().is_none()));
}

#[test]
fn ctrl_d_puts_the_copy_right_after_the_selection() {
    let mut h = setup();
    let cl = some_clip(&h);
    let a0_ = mid(&h, cl.id);
    click_at(&mut h, a0_);
    key(&mut h, Key::D, egui::Modifiers::COMMAND);
    let t = timeline(&h);
    let s = h.state().tlv.sels.clone();
    let c = t.clips.iter().find(|c| c.id == s[0]).unwrap();
    assert_eq!((c.start, c.track), (cl.end(), cl.track));
}
