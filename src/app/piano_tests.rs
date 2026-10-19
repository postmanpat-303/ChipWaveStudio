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

// ---------------------------------------------------------------- M3b

/// the demo in timeline mode on the Timeline view, 40 px a beat
fn tl_setup() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    h.state_mut().enter_timeline();
    h.state_mut().undo_reset();
    h.state_mut().tlv.zoom = 40.0;
    h.state_mut().tlv.scroll = 0.0;
    frame(&mut h);
    frame(&mut h);
    h
}

fn note_data(h: &Harness<'static, App>) -> tl::NoteClipData {
    let id = h.state().tlv.sel.expect("a selected clip");
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else {
            panic!("not a note clip")
        };
        (*t.notes[data as usize]).clone()
    })
}

#[test]
fn shift_double_click_makes_a_note_clip_and_the_roll_draws_chords() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().new_song();
    frame(&mut h);
    assert!(
        h.state().with(|s| s.song.timeline.is_some()),
        "new songs are in timeline mode"
    );
    h.state_mut().view = View::Timeline;
    frame(&mut h);
    frame(&mut h);
    // empty space on track 2, Shift held
    let lanes = h.state().tlv.lanes;
    let p = Pos2::new(lanes.left() + 30.0, lanes.top() + 46.0 * 1.5);
    let m = egui::Modifiers::SHIFT;
    with_mods(&mut h, m, egui::Event::PointerMoved(p));
    let mut t = h.ctx.input(|i| i.time);
    for _ in 0..2 {
        for pressed in [true, false] {
            t += 0.05;
            h.input_mut().time = Some(t);
            with_mods(
                &mut h,
                m,
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: m,
                },
            );
        }
    }
    h.input_mut().time = None;
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    frame(&mut h);
    frame(&mut h);
    let n0 = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().notes.len());
    assert_eq!(n0, 1, "a note clip was made");
    assert!(matches!(
        h.state().pr_target(),
        Some(pianoview::Target::Notes { .. })
    ));
    assert!(h.state().pr.open, "the piano roll opens");
    // a chord: C major key, triad on the clicked D -> D F A
    h.state()
        .with(|s| s.song.info.key = Some(crate::chords::Key::new(0, crate::chords::Scale::Major)));
    {
        let a = h.state_mut();
        a.pr.chord = Some(crate::chords::ChordKind::Triad);
        a.pr.zoom = 20.0;
        a.pr.scroll_x = 0.0;
        a.pr.scroll_y = (120.0 - 55.0) * a.pr.key_h - a.pr.grid_rect.height() / 2.0;
    }
    frame(&mut h);
    let q = at(&h, 2, 51, 3.0); // D-4
    click_at(&mut h, q);
    let d = note_data(&h);
    let mut keys: Vec<u8> = d.notes.iter().map(|n| n.key).collect();
    keys.sort();
    assert_eq!(keys, vec![51, 54, 58], "D F A");
    assert!(d.notes.iter().all(|n| n.start == 2 * tl::ROW));
    // it plays: the plan has the three spans
    let spans = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        tl::build_plan(&s.song, t)
            .notes
            .iter()
            .map(|v| v.len())
            .sum::<usize>()
    });
    assert_eq!(spans, 3);
    // free timing: grid off, nudge right by one tick, then Q quantises back
    h.state_mut().pr.grid = 1;
    key(&mut h, Key::ArrowRight, egui::Modifiers::NONE);
    assert!(note_data(&h)
        .notes
        .iter()
        .all(|n| n.start == 2 * tl::ROW + 1));
    h.state_mut().pr.grid = tl::ROW;
    key(&mut h, Key::Q, egui::Modifiers::NONE);
    assert!(note_data(&h).notes.iter().all(|n| n.start == 2 * tl::ROW));
    // drag one note's right edge: the selection (the chord) gets longer
    let (a, b) = (at(&h, 2, 51, 18.0), at(&h, 6, 51, 1.0));
    drag_m(&mut h, a, b, egui::Modifiers::NONE);
    assert!(
        note_data(&h).notes.iter().all(|n| n.len == 4 * tl::ROW),
        "{:?}",
        note_data(&h).notes
    );
    key(&mut h, Key::Delete, egui::Modifiers::NONE);
    assert!(note_data(&h).notes.is_empty());
}

const PPQ_T: tl::Tick = tl::PPQ;

#[test]
fn midi_records_into_a_note_clip_while_the_timeline_plays_and_capture() {
    let mut h = tl_setup();
    {
        let a = h.state_mut();
        a.edit = true;
        a.extra.midi.live_record = true;
        a.extra.midi.velocity = true;
        a.ch = 2;
    }
    h.state().with(|s| {
        let audio::Shared { song, engine, .. } = s;
        engine.start(song, 0, 20, false, 0);
    });
    let now = std::time::Instant::now();
    h.state_mut().midi_note_on(60, 49, 0.5, now);
    h.state_mut().midi_note_on(64, 53, 1.0, now);
    h.state_mut().midi_note_off(60);
    let d = note_data(&h);
    assert_eq!(d.notes.len(), 2);
    let id = h.state().tlv.sel.unwrap();
    let clip = h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .clone()
    });
    assert_eq!(clip.track, 2, "the cursor track");
    assert_eq!(clip.start, 16 * tl::ROW, "starts on the bar");
    // row 20 (minus nothing: no audio device in tests) = 4 rows into the clip
    assert!(
        d.notes.iter().all(|n| (n.start - 4 * tl::ROW).abs() <= 2),
        "{:?}",
        d.notes
    );
    assert!((tl::roll::vel_f(d.notes[0].vel) - 0.5).abs() < 1e-3);
    // stop: the take ends, the held note gets its length
    h.state().with(|s| s.engine.stop());
    frame(&mut h);
    assert!(h.state().noterec_take_done());
    // Capture: what was played becomes a clip at the given spot
    h.state_mut()
        .tl_apply(super::timeline::TlOp::Capture(3, 32 * PPQ_T));
    frame(&mut h);
    let d = note_data(&h);
    assert_eq!(d.notes.len(), 2);
    assert_eq!(d.notes[0].start, 0);
    assert!(h.state().with(|s| s
        .song
        .timeline
        .as_deref()
        .unwrap()
        .clips
        .iter()
        .any(|c| c.track == 3 && c.start == 32 * PPQ_T)));
}
