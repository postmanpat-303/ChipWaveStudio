//! Piano roll: click vs hold-to-pan vs drag, middle-drag pan, selected-note
//! colours, and moving a selection as a group (clamped, one undo step)
use super::pianoview::{press_decision, sel_note_colours, Press, HOLD_PX, HOLD_S};
use super::timeline_m2_tests::{click_at, drag_m, with_mods};
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline::{self as tl, Tick};
use egui_kittest::Harness;

#[test]
fn press_decision_click_hold_drag() {
    // still and quick: undecided while held, a click on release
    assert_eq!(press_decision(0.05, 0.0, true), Press::Pending);
    assert_eq!(press_decision(0.10, 2.0, false), Press::Click);
    // held still long enough: pan, even if it then moves or lets go
    assert_eq!(press_decision(HOLD_S, 1.0, true), Press::Hold);
    assert_eq!(press_decision(0.4, 0.0, false), Press::Hold);
    // moved past the few pixels first: the usual drag, whatever the time
    assert_eq!(press_decision(0.05, HOLD_PX + 1.0, true), Press::Drag);
    assert_eq!(press_decision(1.0, 40.0, true), Press::Drag);
    assert!((0.2..=0.3).contains(&HOLD_S), "about 250 ms");
}

#[test]
fn selected_notes_get_their_own_colour_in_both_themes_and_keep_velocity() {
    for th in [crate::theme::DARK, crate::theme::AMIGA] {
        let (loud, edge, ink) = sel_note_colours(&th, 1.0);
        let (quiet, _, _) = sel_note_colours(&th, 0.2);
        assert_ne!(loud, th.note, "{}", th.name);
        assert!(loud.r() > loud.b() + 80, "orange: {loud:?}");
        assert!(
            crate::theme::luma(quiet) < crate::theme::luma(loud),
            "velocity reads"
        );
        assert!(
            crate::theme::luma(edge) > crate::theme::luma(loud),
            "lighter outline"
        );
        let d = (crate::theme::luma(ink) - crate::theme::luma(loud)).abs();
        assert!(d > 0.4, "text readable on the fill");
    }
}

// ---- block column (tracker-mode) roll, as piano_tests::setup

fn block_roll(ch: usize) -> Harness<'static, App> {
    let mut song = fileio::demo_song();
    for r in song.patterns[0].cells.iter_mut() {
        r[ch] = Cell::default();
    }
    let mut h = harness_with(song);
    {
        let a = h.state_mut();
        a.pattern = 0;
        a.ch = ch;
        a.set_lower(3);
        a.undo_reset();
    }
    frame(&mut h);
    frame(&mut h);
    let a = h.state_mut();
    a.pr.zoom = 20.0;
    a.pr.scroll_x = 0.0;
    a.pr.scroll_y = (120.0 - 60.0) * a.pr.key_h - a.pr.grid_rect.height() / 2.0;
    frame(&mut h);
    h
}

fn at(h: &Harness<'static, App>, row: usize, key: u8, dx: f32) -> Pos2 {
    let pr = &h.state().pr;
    Pos2::new(
        pr.x_of(tl::rows_to_ticks(row)) + dx,
        pr.y_of(key) + pr.key_h / 2.0,
    )
}

fn notes_in(h: &Harness<'static, App>, ch: usize) -> Vec<(usize, u8)> {
    h.state().with(|s| {
        s.song.patterns[0]
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| (1..=120).contains(&c[ch].note))
            .map(|(r, c)| (r, c[ch].note))
            .collect()
    })
}

fn btn(pos: Pos2, pressed: bool, button: egui::PointerButton) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    }
}

#[test]
fn quick_click_adds_a_note_but_a_held_press_pans_and_adds_nothing() {
    let mut h = block_roll(3);
    let none = egui::Modifiers::NONE;
    // a quick click: a note
    {
        let q = at(&h, 4, 61, 5.0);
        click_at(&mut h, q);
    }
    assert_eq!(notes_in(&h, 3), vec![(4, 61)]);
    // press and hold still, then drag: the grid follows the hand
    let p = at(&h, 12, 58, 5.0);
    let (sx, sy) = (h.state().pr.scroll_x, h.state().pr.scroll_y);
    with_mods(&mut h, none, egui::Event::PointerMoved(p));
    with_mods(&mut h, none, btn(p, true, egui::PointerButton::Primary));
    std::thread::sleep(std::time::Duration::from_millis(
        (HOLD_S * 1000.0) as u64 + 60,
    ));
    frame(&mut h);
    assert!(h.state().pr.pan.is_some(), "held: panning");
    for k in 1..=6 {
        with_mods(
            &mut h,
            none,
            egui::Event::PointerMoved(p + egui::vec2(-20.0 * k as f32, -6.0 * k as f32)),
        );
    }
    let to = p + egui::vec2(-120.0, -36.0);
    with_mods(&mut h, none, btn(to, false, egui::PointerButton::Primary));
    frame(&mut h);
    frame(&mut h);
    assert!(h.state().pr.pan.is_none(), "released: the pan ends");
    assert!(h.state().pr.scroll_x > sx, "dragged left: later in time");
    assert!(
        h.state().pr.scroll_y > sy,
        "dragged up: lower keys come into view"
    );
    assert_eq!(notes_in(&h, 3), vec![(4, 61)], "no note from the pan");
    // a held press released without moving adds nothing either
    let q = at(&h, 20, 60, 5.0);
    with_mods(&mut h, none, egui::Event::PointerMoved(q));
    with_mods(&mut h, none, btn(q, true, egui::PointerButton::Primary));
    std::thread::sleep(std::time::Duration::from_millis(
        (HOLD_S * 1000.0) as u64 + 60,
    ));
    frame(&mut h);
    with_mods(&mut h, none, btn(q, false, egui::PointerButton::Primary));
    frames(&mut h, 2);
    assert_eq!(notes_in(&h, 3).len(), 1);
}

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

#[test]
fn a_quick_drag_still_draws_and_shift_drag_still_boxes() {
    let mut h = block_roll(3);
    // quick drag on empty grid: a note whose length follows the drag
    let (a, b) = (at(&h, 2, 60, 3.0), at(&h, 6, 60, 3.0));
    drag_m(&mut h, a, b, egui::Modifiers::NONE);
    let n = notes_in(&h, 3);
    assert_eq!(n, vec![(2, 60)], "drawn");
    assert!(h.state().pr.pan.is_none());
    // Shift-drag over it: selected by the box
    h.state_mut().pr.sel.clear();
    let (a, b) = (at(&h, 1, 62, 0.0), at(&h, 4, 58, 0.0));
    drag_m(&mut h, a, b, egui::Modifiers::SHIFT);
    assert_eq!(h.state().pr.sel, vec![2]);
}

#[test]
fn middle_drag_pans() {
    let mut h = block_roll(3);
    let p = at(&h, 10, 60, 5.0);
    let sx = h.state().pr.scroll_x;
    let none = egui::Modifiers::NONE;
    with_mods(&mut h, none, egui::Event::PointerMoved(p));
    with_mods(&mut h, none, btn(p, true, egui::PointerButton::Middle));
    for k in 1..=4 {
        with_mods(
            &mut h,
            none,
            egui::Event::PointerMoved(p + egui::vec2(-25.0 * k as f32, 0.0)),
        );
    }
    with_mods(
        &mut h,
        none,
        btn(
            p + egui::vec2(-100.0, 0.0),
            false,
            egui::PointerButton::Middle,
        ),
    );
    frames(&mut h, 2);
    assert!(h.state().pr.scroll_x > sx);
    assert!(notes_in(&h, 3).is_empty());
}

#[test]
fn block_selection_moves_as_a_group_with_one_undo_step() {
    let mut h = block_roll(3);
    for (r, k) in [(4, 60), (8, 64), (12, 67)] {
        {
            let q = at(&h, r, k, 5.0);
            click_at(&mut h, q);
        }
    }
    assert_eq!(notes_in(&h, 3), vec![(4, 60), (8, 64), (12, 67)]);
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    assert_eq!(h.state().pr.sel.len(), 3, "Ctrl+A selects them all");
    // a press on a selected note without moving keeps the selection
    let p = at(&h, 8, 64, 5.0);
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    with_mods(
        &mut h,
        egui::Modifiers::NONE,
        btn(p, true, egui::PointerButton::Primary),
    );
    frames(&mut h, 2);
    assert_eq!(h.state().pr.sel.len(), 3, "press alone keeps the selection");
    with_mods(
        &mut h,
        egui::Modifiers::NONE,
        btn(p, false, egui::PointerButton::Primary),
    );
    frames(&mut h, 2);
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    // grab the middle one: two rows later, two semitones up
    {
        let (a, b) = (at(&h, 8, 64, 5.0), at(&h, 10, 66, 5.0));
        drag_m(&mut h, a, b, egui::Modifiers::NONE);
    }
    assert_eq!(notes_in(&h, 3), vec![(6, 62), (10, 66), (14, 69)]);
    assert_eq!(h.state().pr.sel.len(), 3, "still selected");
    // clamped at row 0: drag far left keeps the spacing
    {
        let (a, b) = (at(&h, 10, 66, 5.0), at(&h, 0, 66, 2.0));
        drag_m(&mut h, a, b, egui::Modifiers::NONE);
    }
    assert_eq!(notes_in(&h, 3), vec![(0, 62), (4, 66), (8, 69)]);
    // one undo step per drag
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(notes_in(&h, 3), vec![(6, 62), (10, 66), (14, 69)]);
}

// ---- note clip roll (timeline mode)

fn clip_roll(keys: &[(Tick, u8)]) -> (Harness<'static, App>, usize) {
    let mut s = Song::clean();
    crate::chanops::insert_channel(&mut s, 0, "Keys");
    let id = tl::new_note_clip(&mut s, 0, 0).unwrap();
    let tl0 = s.timeline.as_deref_mut().unwrap();
    let data = tl0.notes.len() - 1;
    tl0.notes[data].make_mut().notes = keys
        .iter()
        .map(|&(start, key)| tl::Note {
            start,
            len: tl::ROW * 2,
            key,
            vel: 0xC000,
            ..Default::default()
        })
        .collect();
    let mut h = harness_with(s);
    h.state_mut().default_layout();
    // the roll has been clicked into (the default layout leaves the keys
    // with the Timeline)
    h.state_mut().pr.focus = true;
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().undo_reset();
    frames(&mut h, 3);
    let a = h.state_mut();
    a.pr.zoom = 20.0;
    a.pr.scroll_x = 0.0;
    a.pr.grid = tl::ROW;
    // key 119 at the top (a valid scroll: the wheel code clamps it)
    a.pr.scroll_y = (120.0 - 119.0) * a.pr.key_h;
    frames(&mut h, 2);
    (h, data)
}

fn clip_notes(h: &Harness<'static, App>, data: usize) -> Vec<(Tick, u8)> {
    h.state().with(|s| {
        s.song.timeline.as_ref().unwrap().notes[data]
            .notes
            .iter()
            .map(|n| (n.start, n.key))
            .collect()
    })
}

#[test]
fn clip_selection_moves_as_a_group_and_clamps_at_the_top_key() {
    let r = tl::ROW;
    let (mut h, data) = clip_roll(&[(4 * r, 110), (4 * r, 114), (8 * r, 117)]);
    assert!(matches!(
        h.state().pr.target,
        Some(super::pianoview::Target::Notes { .. })
    ));
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    assert_eq!(h.state().pr.sel.len(), 3);
    // two rows later, one semitone up
    {
        let (a, b) = (at(&h, 4, 114, 5.0), at(&h, 6, 115, 5.0));
        drag_m(&mut h, a, b, egui::Modifiers::NONE);
    }
    assert_eq!(
        clip_notes(&h, data),
        vec![(6 * r, 111), (6 * r, 115), (10 * r, 118)]
    );
    // ten semitones up and far left: the top note stops at 120, bar 0 stops
    // the earliest, the chord keeps its shape
    {
        let (a, b) = (at(&h, 6, 115, 5.0), at(&h, 0, 118, 1.0));
        drag_m(&mut h, a, b, egui::Modifiers::NONE);
    }
    let n = clip_notes(&h, data);
    assert_eq!(n, vec![(0, 113), (0, 117), (4 * r, 120)], "clamped");
    assert_eq!(h.state().pr.sel.len(), 3);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(
        clip_notes(&h, data),
        vec![(6 * r, 111), (6 * r, 115), (10 * r, 118)]
    );
}
