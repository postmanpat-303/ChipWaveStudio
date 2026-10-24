//! Piano roll copy and paste (Del, 21 Oct 2026): Ctrl-drag drops copies,
//! Ctrl+C / X / V / D on notes while the roll has the keyboard, one undo
//! step each, between clips, linked clips and Tracker block columns.
use super::prollpan_tests::{at, block_roll, clip_notes, clip_roll, notes_in};
use super::timeline_m2_tests::{drag_m, with_mods};
use super::ui_tests::{frame, key};
use super::*;
use crate::timeline::{self as tl, Tick};
use egui_kittest::Harness;

const R: Tick = tl::ROW;
const CMD: egui::Modifiers = egui::Modifiers::COMMAND;
const NONE: egui::Modifiers = egui::Modifiers::NONE;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn undo(h: &mut Harness<'static, App>) {
    key(h, Key::Z, CMD);
    frames(h, 2);
}

/// a drag whose modifiers change: `press` while pressing and moving, `drop`
/// for the last move and the release (Ctrl let go or pressed late)
fn drag_switch(
    h: &mut Harness<'static, App>,
    from: Pos2,
    to: Pos2,
    press: egui::Modifiers,
    drop: egui::Modifiers,
    held_still: bool,
) {
    with_mods(h, press, egui::Event::PointerMoved(from));
    let btn = |pos, pressed, m| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: m,
    };
    with_mods(h, press, btn(from, true, press));
    for k in 1..=8 {
        let p = from + (to - from) * (k as f32 / 8.0);
        with_mods(h, press, egui::Event::PointerMoved(p));
    }
    // the modifier changes with the mouse held still (or in the release
    // frame itself), then the release
    if held_still {
        with_mods(h, drop, egui::Event::PointerMoved(to));
    }
    with_mods(h, drop, btn(to, false, drop));
    with_mods(h, NONE, egui::Event::PointerMoved(to));
    frames(h, 3);
}

#[test]
fn ctrl_drag_leaves_the_originals_and_drops_copies_as_one_undo_step() {
    let (mut h, data) = clip_roll(&[(4 * R, 110), (4 * R, 114), (8 * R, 117)]);
    key(&mut h, Key::A, CMD);
    assert_eq!(h.state().pr.sel.len(), 3);
    let (a, b) = (at(&h, 4, 114, 5.0), at(&h, 12, 116, 5.0));
    drag_m(&mut h, a, b, CMD);
    assert_eq!(
        clip_notes(&h, data),
        vec![
            (4 * R, 110),
            (4 * R, 114),
            (8 * R, 117),
            (12 * R, 112),
            (12 * R, 116),
            (16 * R, 119)
        ],
        "originals stay, copies 8 rows later and 2 keys up"
    );
    let mut sel = h.state().pr.sel.clone();
    sel.sort_unstable();
    assert_eq!(sel, vec![3, 4, 5], "the copies are the selection");
    assert!(
        h.state().status.starts_with("Copied 3"),
        "{}",
        h.state().status
    );
    undo(&mut h);
    assert_eq!(
        clip_notes(&h, data),
        vec![(4 * R, 110), (4 * R, 114), (8 * R, 117)],
        "one undo step takes all the copies away"
    );
}

#[test]
fn ctrl_is_checked_at_the_drop() {
    // Ctrl let go before the release: a plain move
    let (mut h, data) = clip_roll(&[(4 * R, 110)]);
    key(&mut h, Key::A, CMD);
    let (a, b) = (at(&h, 4, 110, 5.0), at(&h, 8, 110, 5.0));
    drag_switch(&mut h, a, b, CMD, NONE, true);
    assert_eq!(
        clip_notes(&h, data),
        vec![(8 * R, 110)],
        "moved, not copied"
    );
    // ... also when Ctrl goes up in the release frame
    let (mut h, data) = clip_roll(&[(4 * R, 110)]);
    key(&mut h, Key::A, CMD);
    let (a, b) = (at(&h, 4, 110, 5.0), at(&h, 8, 110, 5.0));
    drag_switch(&mut h, a, b, CMD, NONE, false);
    assert_eq!(
        clip_notes(&h, data),
        vec![(8 * R, 110)],
        "moved at the drop"
    );
    // Ctrl pressed only at the end: copies
    let (mut h, data) = clip_roll(&[(4 * R, 110)]);
    key(&mut h, Key::A, CMD);
    let (a, b) = (at(&h, 4, 110, 5.0), at(&h, 8, 110, 5.0));
    drag_switch(&mut h, a, b, NONE, CMD, true);
    assert_eq!(clip_notes(&h, data), vec![(4 * R, 110), (8 * R, 110)]);
    let (mut h, data) = clip_roll(&[(4 * R, 110)]);
    key(&mut h, Key::A, CMD);
    let (a, b) = (at(&h, 4, 110, 5.0), at(&h, 8, 110, 5.0));
    drag_switch(&mut h, a, b, NONE, CMD, false);
    assert_eq!(
        clip_notes(&h, data),
        vec![(4 * R, 110), (8 * R, 110)],
        "copied at the drop"
    );
    // and a plain drag still moves
    let (mut h, data) = clip_roll(&[(4 * R, 110)]);
    key(&mut h, Key::A, CMD);
    let (a, b) = (at(&h, 4, 110, 5.0), at(&h, 6, 111, 5.0));
    drag_m(&mut h, a, b, NONE);
    assert_eq!(clip_notes(&h, data), vec![(6 * R, 111)]);
}

#[test]
fn copy_then_paste_at_the_play_marker_keeps_timing_and_pitch() {
    let (mut h, data) = clip_roll(&[(2 * R, 60), (2 * R, 64), (3 * R, 67)]);
    key(&mut h, Key::A, CMD);
    key(&mut h, Key::C, CMD);
    assert_eq!(h.state().pr.clip.len(), 3);
    assert!(h.state().status.starts_with("Copied 3"));
    // the play marker at row 16 (the clip starts at 0)
    h.state_mut().tlv.cue = 16 * R;
    frames(&mut h, 2);
    key(&mut h, Key::V, CMD);
    let n = clip_notes(&h, data);
    assert_eq!(
        &n[3..],
        &[(16 * R, 60), (16 * R, 64), (17 * R, 67)],
        "relative timing and pitch kept"
    );
    let mut sel = h.state().pr.sel.clone();
    sel.sort_unstable();
    assert_eq!(sel, vec![3, 4, 5], "the pasted notes are the selection");
    // again: right after them (2 rows long, the last ends at row 19)
    key(&mut h, Key::V, CMD);
    let n = clip_notes(&h, data);
    assert_eq!(&n[6..], &[(19 * R, 60), (19 * R, 64), (20 * R, 67)]);
    // each paste is one undo step
    undo(&mut h);
    assert_eq!(clip_notes(&h, data).len(), 6);
    undo(&mut h);
    assert_eq!(clip_notes(&h, data).len(), 3);
}

#[test]
fn cut_deletes_and_paste_works_in_another_clip() {
    let (mut h, data) = clip_roll(&[(0, 60), (R, 62)]);
    // a second note clip on the same track, from bar 5
    let id2 = h
        .state()
        .with(|s| tl::new_note_clip(&mut s.song, 0, 64 * R).unwrap());
    h.state().with(|s| {
        let Shared { song, engine, .. } = s;
        engine.ensure_plan(song);
    });
    frames(&mut h, 2);
    key(&mut h, Key::A, CMD);
    key(&mut h, Key::X, CMD);
    assert!(clip_notes(&h, data).is_empty(), "cut");
    assert!(h.state().status.starts_with("Cut 2"));
    // into the other clip (play marker before it: where they were copied)
    h.state_mut().tlv.sel = Some(id2);
    h.state_mut().tlv.sels = vec![id2];
    h.state_mut().pr.focus = true;
    frames(&mut h, 3);
    let Some(super::pianoview::Target::Notes { data: d2, .. }) = h.state().pr.target else {
        panic!("the roll shows the second clip");
    };
    assert_ne!(d2, data);
    key(&mut h, Key::V, CMD);
    assert_eq!(clip_notes(&h, d2), vec![(0, 60), (R, 62)]);
    undo(&mut h);
    assert!(clip_notes(&h, d2).is_empty());
    undo(&mut h);
    assert_eq!(clip_notes(&h, data).len(), 2, "the cut is undone too");
}

#[test]
fn ctrl_d_duplicates_right_after_the_selection() {
    let (mut h, data) = clip_roll(&[(0, 60), (2 * R, 62)]);
    key(&mut h, Key::A, CMD);
    key(&mut h, Key::D, CMD);
    // the selection spans 4 rows (the last note ends at row 4)
    assert_eq!(
        clip_notes(&h, data),
        vec![(0, 60), (2 * R, 62), (4 * R, 60), (6 * R, 62)]
    );
    assert_eq!(h.state().pr.sel.len(), 2);
    key(&mut h, Key::D, CMD);
    assert_eq!(&clip_notes(&h, data)[4..], &[(8 * R, 60), (10 * R, 62)]);
    // the Timeline clips were not duplicated
    let clips = h
        .state()
        .with(|s| s.song.timeline.as_ref().unwrap().clips.len());
    assert_eq!(clips, 1);
    undo(&mut h);
    assert_eq!(clip_notes(&h, data).len(), 4);
}

#[test]
fn nothing_selected_says_so_and_changes_nothing() {
    let (mut h, data) = clip_roll(&[(0, 60)]);
    h.state_mut().pr.sel.clear();
    key(&mut h, Key::C, CMD);
    assert!(h.state().status.starts_with("Select notes to copy"));
    key(&mut h, Key::D, CMD);
    assert!(h.state().status.starts_with("Select notes to duplicate"));
    let clips = h
        .state()
        .with(|s| s.song.timeline.as_ref().unwrap().clips.len());
    assert_eq!(clips, 1, "Ctrl+D in the roll never duplicates the clip");
    assert_eq!(clip_notes(&h, data), vec![(0, 60)]);
}

#[test]
fn the_roll_without_the_keyboard_leaves_its_notes_alone() {
    let (mut h, data) = clip_roll(&[(0, 60)]);
    key(&mut h, Key::A, CMD);
    key(&mut h, Key::C, CMD);
    h.state_mut().pr.focus = false;
    key(&mut h, Key::V, CMD);
    assert_eq!(clip_notes(&h, data), vec![(0, 60)]);
    assert_eq!(h.state().status, super::keys::CLIPBOARD_ELSEWHERE);
}

#[test]
fn notes_from_the_system_clipboard_paste_too() {
    let (mut h, data) = clip_roll(&[]);
    let text = tl::roll::notes_to_text(&[tl::Note {
        start: 0,
        len: 2 * R,
        key: 61,
        vel: 0x8000,
        ..Default::default()
    }]);
    h.input_mut().events.push(egui::Event::Paste(text.clone()));
    frames(&mut h, 3);
    assert_eq!(clip_notes(&h, data), vec![(0, 61)]);
    // other text: the roll's own clipboard (empty here) and a hint
    h.input_mut()
        .events
        .push(egui::Event::Paste("hello".into()));
    frames(&mut h, 3);
    assert_eq!(
        clip_notes(&h, data),
        vec![(0, 61), (2 * R, 61)],
        "own clipboard"
    );
}

#[test]
fn pasting_into_a_linked_clip_edits_every_linked_copy() {
    let (mut h, data) = clip_roll(&[(0, 60)]);
    let id = h
        .state()
        .with(|s| s.song.timeline.as_ref().unwrap().clips[0].id);
    let dup = h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let d = tl::duplicate(t, &[id]);
        let Shared { song, engine, .. } = s;
        engine.ensure_plan(song);
        d
    });
    frames(&mut h, 2);
    let shares = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        dup.iter().all(|d| {
            t.clips
                .iter()
                .any(|c| c.id == *d && c.notes() == Some(data))
        })
    });
    assert!(shares, "a linked copy shares the notes");
    key(&mut h, Key::A, CMD);
    key(&mut h, Key::D, CMD);
    let (a, _) = h
        .state()
        .with(|s| tl::linked(s.song.timeline.as_deref().unwrap(), data));
    assert_eq!(a, 2);
    assert_eq!(
        clip_notes(&h, data),
        vec![(0, 60), (2 * R, 60)],
        "shared data edited"
    );
}

#[test]
fn block_column_copy_paste_duplicate_and_ctrl_drag() {
    let mut h = block_roll(3);
    h.state().with(|s| {
        let p = &mut s.song.patterns[0];
        p.cells[0][3] = Cell {
            note: 49,
            instr: 1,
            ..Default::default()
        };
        p.cells[2][3] = Cell {
            note: 53,
            instr: 1,
            ..Default::default()
        };
        p.cells[4][3].note = crate::model::NOTE_OFF;
    });
    h.state_mut().undo_reset();
    h.state_mut().pr.focus = true;
    frames(&mut h, 2);
    let order = h.state().with(|s| s.song.order.len());
    key(&mut h, Key::A, CMD);
    assert_eq!(h.state().pr.sel.len(), 2);
    key(&mut h, Key::D, CMD);
    assert_eq!(
        notes_in(&h, 3),
        vec![(0, 49), (2, 53), (4, 49), (6, 53)],
        "after the selection (4 rows)"
    );
    assert_eq!(
        h.state().with(|s| s.song.order.len()),
        order,
        "no Order list clip"
    );
    undo(&mut h);
    assert_eq!(notes_in(&h, 3), vec![(0, 49), (2, 53)]);
    // copy, then paste at the Tracker cursor row
    key(&mut h, Key::A, CMD);
    key(&mut h, Key::C, CMD);
    h.state_mut().row = 16;
    key(&mut h, Key::V, CMD);
    assert_eq!(notes_in(&h, 3), vec![(0, 49), (2, 53), (16, 49), (18, 53)]);
    undo(&mut h);
    // Ctrl-drag the first note 8 rows on: a copy
    h.state_mut().pr.sel = vec![0];
    // the notes mid-roll (the two-row toolbar leaves a shorter roll)
    let pr = &mut h.state_mut().pr;
    pr.scroll_y = (120.0 - 51.0) * pr.key_h - pr.grid_rect.height() / 2.0;
    frames(&mut h, 1);
    let (a, b) = (at(&h, 0, 49, 5.0), at(&h, 8, 49, 5.0));
    drag_m(&mut h, a, b, CMD);
    assert_eq!(notes_in(&h, 3), vec![(0, 49), (2, 53), (8, 49)]);
    assert_eq!(h.state().pr.sel, vec![8]);
    undo(&mut h);
    assert_eq!(notes_in(&h, 3), vec![(0, 49), (2, 53)]);
}
