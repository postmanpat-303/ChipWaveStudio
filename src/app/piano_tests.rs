//! Piano roll UI tests (M3a block columns, M3b note clips).
use super::timeline_m2_tests::{click_at, drag_m, with_mods};
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline as tl;
use egui_kittest::Harness;

/// the demo in the pattern editor on block 0, channel `ch` emptied, with
/// the piano roll in the lower panel at 20 px a row from row 0
fn setup(ch: usize) -> (Harness<'static, App>, Pattern) {
    let mut song = fileio::demo_song();
    for r in song.patterns[0].cells.iter_mut() {
        r[ch] = Cell::default();
    }
    let p0 = song.patterns[0].clone();
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
    (h, p0)
}

fn pat(h: &Harness<'static, App>) -> Pattern {
    h.state().with(|s| s.song.patterns[0].clone())
}

/// screen point of (row, key) in the roll, `dx` pixels into the row
fn at(h: &Harness<'static, App>, row: usize, key: u8, dx: f32) -> Pos2 {
    let pr = &h.state().pr;
    Pos2::new(
        pr.x_of(tl::rows_to_ticks(row)) + dx,
        pr.y_of(key) + pr.key_h / 2.0,
    )
}

#[test]
fn the_piano_roll_tab_opens_on_the_block_column() {
    let (h, _) = setup(3);
    assert_eq!(
        h.state().pr_target(),
        Some(pianoview::Target::Block { pattern: 0, col: 3 })
    );
    assert!(
        h.state().pr.grid_rect.width() > 400.0,
        "drawn in the lower panel"
    );
    assert!(h.state().pr_active(), "the tab takes the keys");
}

#[test]
fn draw_resize_move_delete_round_trips_the_cells() {
    let (mut h, p0) = setup(3);
    // click: a one-row note with the current instrument and a note-off
    let ins = h.state().instr;
    let q = at(&h, 4, 61, 5.0);
    click_at(&mut h, q);
    let p = pat(&h);
    assert_eq!((p.cells[4][3].note, p.cells[4][3].instr), (61, ins));
    assert_eq!(p.cells[5][3].note, NOTE_OFF);
    // drag its right edge to row 8: four rows long
    let (a, b) = (at(&h, 4, 61, 18.0), at(&h, 8, 61, 2.0));
    drag_m(&mut h, a, b, egui::Modifiers::NONE);
    let p = pat(&h);
    assert_eq!(p.cells[5][3].note, 0);
    assert_eq!(p.cells[8][3].note, NOTE_OFF, "length 4 rows");
    // drag the body two rows right and up a tone
    let (a, b) = (at(&h, 5, 61, 5.0), at(&h, 7, 63, 5.0));
    drag_m(&mut h, a, b, egui::Modifiers::NONE);
    let p = pat(&h);
    assert_eq!((p.cells[4][3].note, p.cells[6][3].note), (0, 63));
    assert_eq!(p.cells[10][3].note, NOTE_OFF);
    assert_eq!(p.cells[8][3].note, 0, "its old note-off went with it");
    // velocity lane: drag the bar down to about half
    let vr = h.state().pr.vel_rect;
    let x = h.state().pr.x_of(tl::rows_to_ticks(6)) + 1.0;
    drag_m(
        &mut h,
        Pos2::new(x, vr.top() + 3.0),
        Pos2::new(x, vr.center().y),
        egui::Modifiers::NONE,
    );
    let v = pat(&h).cells[6][3].vol;
    assert!((28..=38).contains(&v), "velocity ~half: vol {v}");
    // the inspector-free key edits: Up transposes the selection
    key(&mut h, Key::ArrowUp, egui::Modifiers::NONE);
    assert_eq!(pat(&h).cells[6][3].note, 64);
    // right-click deletes: the column is exactly as before
    let p = at(&h, 6, 64, 5.0);
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    for pressed in [true, false] {
        with_mods(
            &mut h,
            egui::Modifiers::NONE,
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Default::default(),
            },
        );
    }
    frame(&mut h);
    let diff: Vec<(usize, Cell)> = pat(&h)
        .cells
        .iter()
        .enumerate()
        .filter(|(r, c)| c[3] != p0.cells[*r][3])
        .map(|(r, c)| (r, c[3]))
        .collect();
    assert!(diff.is_empty(), "left over: {diff:?}");
    assert_eq!(pat(&h), p0, "drawing and deleting leaves identical cells");
    // and every step was undoable
    for _ in 0..6 {
        h.state_mut().do_undo(false);
    }
    assert_eq!(pat(&h), p0);
    h.state_mut().do_undo(true);
    assert_ne!(pat(&h), p0, "redo brings the first note back");
}

#[test]
fn keys_delete_select_all_and_leave_effects_alone() {
    let (mut h, _) = setup(3);
    h.state().with(|s| {
        let c = &mut s.song.patterns[0].cells;
        c[0][3] = Cell {
            note: 49,
            instr: 1,
            ..Default::default()
        };
        c[2][3] = Cell {
            note: NOTE_OFF,
            ..Default::default()
        };
        c[3][3].fx = 0x0A;
        c[3][3].fxp = 0x04;
        c[8][3] = Cell {
            note: 52,
            instr: 1,
            vol: 20,
            ..Default::default()
        };
    });
    frame(&mut h);
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    assert_eq!(h.state().pr.sel.len(), 2);
    key(&mut h, Key::ArrowRight, egui::Modifiers::NONE);
    let p = pat(&h);
    assert_eq!((p.cells[1][3].note, p.cells[9][3].note), (49, 52));
    assert_eq!(
        (p.cells[3][3].fx, p.cells[3][3].fxp),
        (0x0A, 0x04),
        "effects stay on their rows"
    );
    assert_eq!(p.cells[9][3].vol, 20);
    key(&mut h, Key::Delete, egui::Modifiers::NONE);
    let p = pat(&h);
    assert!(
        p.cells.iter().all(|r| r[3].note == 0),
        "notes and note-offs gone"
    );
    assert_eq!(p.cells[3][3].fx, 0x0A);
    // clicking the pattern editor gives the keys back
    let pr = h.state().pr.grid_rect;
    click_at(&mut h, Pos2::new(pr.center().x, pr.top() - 120.0));
    assert!(!h.state().pr_active());
}
