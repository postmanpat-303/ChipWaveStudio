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
    // new songs start with no tracks: four empty ones, as + Add track would
    h.state_mut().set_channels(4);
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
    // a note goes back by the time since its key went down (`now`); on a
    // busy CI runner that can be a few ms, so allow what really elapsed
    let slack = {
        let tps = h
            .state()
            .with(|s| tl::ROW as f64 / s.engine.row_secs().max(1e-4) as f64);
        (now.elapsed().as_secs_f64() * tps).ceil() as tl::Tick
    };
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
        d.notes
            .iter()
            .all(|n| n.start <= 4 * tl::ROW + 2 && n.start >= 4 * tl::ROW - 2 - slack),
        "slack {slack}: {:?}",
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

// ------------------------------------------------- click entry, eraser, tools

/// an empty 2-bar note clip on track 3 open in the roll, 20 px a row
fn clip_setup() -> Harness<'static, App> {
    let mut h = tl_setup();
    let id = h
        .state()
        .with(|s| {
            let id = tl::new_note_clip(&mut s.song, 2, 0)?;
            let t = s.song.timeline.as_deref_mut()?;
            let c = t.clips.iter_mut().find(|c| c.id == id)?;
            c.len = 8 * PPQ_T;
            let tl::ClipKind::Notes { data } = c.kind else {
                return None;
            };
            t.notes[data as usize].make_mut().len = 8 * PPQ_T;
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(id)
        })
        .expect("a note clip");
    {
        let a = h.state_mut();
        a.tlv.sel = Some(id);
        a.tlv.sels = vec![id];
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

fn lens(h: &Harness<'static, App>) -> Vec<(tl::Tick, u8, tl::Tick)> {
    let mut v: Vec<_> = note_data(h)
        .notes
        .iter()
        .map(|n| (n.start, n.key, n.len))
        .collect();
    v.sort();
    v
}

fn right_drag(h: &mut Harness<'static, App>, from: Pos2, to: Pos2) {
    right_path(h, &[from, to]);
}

/// a right-button swipe through these points
fn right_path(h: &mut Harness<'static, App>, pts: &[Pos2]) {
    let m = egui::Modifiers::NONE;
    let btn = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: m,
    };
    with_mods(h, m, egui::Event::PointerMoved(pts[0]));
    with_mods(h, m, btn(pts[0], true));
    for w in pts.windows(2) {
        for k in 1..=6 {
            with_mods(
                h,
                m,
                egui::Event::PointerMoved(w[0] + (w[1] - w[0]) * (k as f32 / 6.0)),
            );
        }
    }
    let last = *pts.last().unwrap();
    with_mods(h, m, btn(last, false));
    frame(h);
}

#[test]
fn a_single_click_draws_a_note_the_length_of_the_last_one() {
    let mut h = clip_setup();
    let r = tl::ROW;
    // the first note: one grid step (1/16)
    {
        let p0 = at(&h, 0, 60, 5.0);
        click_at(&mut h, p0);
    }
    assert_eq!(lens(&h), vec![(0, 60, r)]);
    // stretch it to 4 rows: the next click draws 4 rows
    {
        let p0 = at(&h, 0, 60, 18.0);
        let p1 = at(&h, 4, 60, 1.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    assert_eq!(h.state().pr.last_len, 4 * r);
    {
        let p0 = at(&h, 8, 62, 5.0);
        click_at(&mut h, p0);
    }
    assert_eq!(
        lens(&h)[1],
        (8 * r, 62, 4 * r),
        "same length as the last note"
    );
    // a click on an existing note selects it (no new note) and takes its length
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let d = t.notes.last_mut().unwrap().make_mut();
        d.notes.push(tl::Note {
            start: 16 * r,
            len: 2 * r,
            key: 64,
            vel: tl::roll::vel_u16(1.0),
            ..Default::default()
        });
    });
    frame(&mut h);
    {
        let p0 = at(&h, 16, 64, 5.0);
        click_at(&mut h, p0);
    }
    assert_eq!(
        note_data(&h).notes.len(),
        3,
        "clicking a note doesn't add one"
    );
    assert_eq!(h.state().pr.last_len, 2 * r);
    // a slightly long, wobbly click (becomes a tiny drag) keeps that length
    let p = at(&h, 20, 65, 5.0);
    drag_m(&mut h, p, p + egui::vec2(3.0, 1.0), egui::Modifiers::NONE);
    assert!(lens(&h).contains(&(20 * r, 65, 2 * r)), "{:?}", lens(&h));
    // a new note's right edge dragged sets the length and is remembered
    // (a drag on empty space box-selects now)
    {
        let p0 = at(&h, 24, 67, 5.0);
        click_at(&mut h, p0);
        let p0 = at(&h, 25, 67, 18.0);
        let p1 = at(&h, 27, 67, 1.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    assert!(lens(&h).contains(&(24 * r, 67, 3 * r)), "{:?}", lens(&h));
    assert_eq!(h.state().pr.last_len, 3 * r);
    // the toolbar shows it
    assert_eq!(pianoview::len_name(3 * r), "3/16");
    assert_eq!(pianoview::len_name(4 * PPQ_T), "1 bar");
    // every step undoes
    let n = note_data(&h).notes.len();
    h.state_mut().do_undo(false);
    assert!(lens(&h).contains(&(24 * r, 67, 2 * r)), "the resize undoes");
    h.state_mut().do_undo(false);
    assert_eq!(note_data(&h).notes.len(), n - 1, "then the new note");
}

#[test]
fn shift_click_toggles_the_selection_and_never_draws() {
    let mut h = clip_setup();
    let r = tl::ROW;
    for (row, key) in [(0, 60), (4, 62), (8, 64)] {
        {
            let p0 = at(&h, row, key, 5.0);
            click_at(&mut h, p0);
        }
    }
    let id_of = |h: &Harness<'static, App>, s: tl::Tick| {
        note_data(h)
            .notes
            .iter()
            .position(|n| n.start == s)
            .unwrap()
    };
    let shift = egui::Modifiers::SHIFT;
    let sclick = |h: &mut Harness<'static, App>, p: Pos2| {
        with_mods(h, shift, egui::Event::PointerMoved(p));
        for pressed in [true, false] {
            with_mods(
                h,
                shift,
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: shift,
                },
            );
        }
        with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
        frame(h);
    };
    {
        let p0 = at(&h, 0, 60, 5.0);
        click_at(&mut h, p0);
    }
    {
        let p0 = at(&h, 4, 62, 5.0);
        sclick(&mut h, p0);
    }
    let mut sel = h.state().pr.sel.clone();
    sel.sort();
    assert_eq!(sel, vec![id_of(&h, 0), id_of(&h, 4 * r)]);
    // Shift+click again takes it out
    {
        let p0 = at(&h, 4, 62, 5.0);
        sclick(&mut h, p0);
    }
    assert_eq!(h.state().pr.sel, vec![id_of(&h, 0)]);
    // Shift+click on empty space draws nothing
    {
        let p0 = at(&h, 12, 70, 5.0);
        sclick(&mut h, p0);
    }
    assert_eq!(note_data(&h).notes.len(), 3);
    // Shift+box adds the other two
    {
        let p0 = at(&h, 3, 65, 2.0);
        let p1 = at(&h, 10, 61, 2.0);
        drag_m(&mut h, p0, p1, shift);
    }
    assert_eq!(h.state().pr.sel.len(), 3);
    // moving drags the whole selection; a plain click selects just one
    {
        let p0 = at(&h, 4, 62, 5.0);
        let p1 = at(&h, 6, 62, 5.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    let starts: Vec<tl::Tick> = lens(&h).iter().map(|x| x.0).collect();
    assert_eq!(starts, vec![2 * r, 6 * r, 10 * r]);
    {
        let p0 = at(&h, 6, 62, 5.0);
        click_at(&mut h, p0);
    }
    assert_eq!(h.state().pr.sel.len(), 1);
}

#[test]
fn right_drag_erases_in_one_undo_step_with_smoke() {
    let mut h = clip_setup();
    let r = tl::ROW;
    for (row, key) in [(0, 60), (2, 61), (4, 60), (6, 61), (8, 66)] {
        {
            let p0 = at(&h, row, key, 5.0);
            click_at(&mut h, p0);
        }
    }
    h.state_mut().undo_reset();
    assert_eq!(note_data(&h).notes.len(), 5);
    // swipe across the first four (zig-zag between keys 60 and 61)
    {
        let pts = [
            at(&h, 0, 60, 2.0),
            at(&h, 2, 61, 5.0),
            at(&h, 4, 60, 5.0),
            at(&h, 6, 61, 5.0),
        ];
        right_path(&mut h, &pts);
    }
    assert_eq!(lens(&h), vec![(8 * r, 66, r)], "only the far note is left");
    assert!(!h.state().pr.smoke.is_empty(), "notes go up in smoke");
    h.state_mut().do_undo(false);
    frame(&mut h);
    assert_eq!(
        note_data(&h).notes.len(),
        5,
        "one undo brings the whole swipe back"
    );
    // a plain right-click deletes one note; on empty space it does nothing
    {
        let p0 = at(&h, 8, 66, 5.0);
        let p1 = at(&h, 8, 66, 5.0);
        right_drag(&mut h, p0, p1);
    }
    assert_eq!(note_data(&h).notes.len(), 4);
    {
        let p0 = at(&h, 20, 50, 5.0);
        let p1 = at(&h, 20, 50, 5.0);
        right_drag(&mut h, p0, p1);
    }
    assert_eq!(note_data(&h).notes.len(), 4);
    // Flat style: no smoke
    h.state_mut().extra.flat_ui = true;
    h.state_mut().pr.smoke.clear();
    {
        let p0 = at(&h, 0, 60, 5.0);
        let p1 = at(&h, 0, 60, 5.0);
        right_drag(&mut h, p0, p1);
    }
    assert!(h.state().pr.smoke.is_empty());
    assert_eq!(note_data(&h).notes.len(), 3);
}

#[test]
fn tools_pencil_paints_select_boxes_and_smart_snap() {
    let mut h = clip_setup();
    let r = tl::ROW;
    h.state_mut().pr.last_len = 2 * r;
    h.state_mut().pr.tool = pianoview::PrTool::Pencil;
    {
        let p0 = at(&h, 0, 60, 3.0);
        let p1 = at(&h, 7, 60, 2.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    assert_eq!(
        lens(&h),
        vec![
            (0, 60, 2 * r),
            (2 * r, 60, 2 * r),
            (4 * r, 60, 2 * r),
            (6 * r, 60, 2 * r)
        ]
    );
    // Select tool: a box on empty space selects, never draws
    h.state_mut().pr.tool = pianoview::PrTool::Select;
    {
        let p0 = at(&h, 1, 62, 2.0);
        let p1 = at(&h, 5, 58, 2.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    assert_eq!(note_data(&h).notes.len(), 4);
    assert_eq!(h.state().pr.sel.len(), 3);
    // keys 1 2 3 pick the tools
    key(&mut h, Key::Num1, egui::Modifiers::NONE);
    assert_eq!(h.state().pr.tool, pianoview::PrTool::Pointer);
    // Smart snap follows the zoom
    h.state_mut().pr.grid = 0;
    h.state_mut().pr.zoom = 20.0;
    assert_eq!(h.state().pr_grid_now(), r);
    h.state_mut().pr.zoom = 4.0;
    assert_eq!(h.state().pr_grid_now(), PPQ_T);
}

/// a left-button drag through these points with `m` held
fn left_path(h: &mut Harness<'static, App>, pts: &[Pos2], m: egui::Modifiers) {
    let btn = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: m,
    };
    with_mods(h, m, egui::Event::PointerMoved(pts[0]));
    with_mods(h, m, btn(pts[0], true));
    for w in pts.windows(2) {
        for k in 1..=6 {
            with_mods(
                h,
                m,
                egui::Event::PointerMoved(w[0] + (w[1] - w[0]) * (k as f32 / 6.0)),
            );
        }
    }
    let last = *pts.last().unwrap();
    with_mods(h, m, btn(last, false));
    with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(last));
    frame(h);
}

/// put notes (start row, key, length in ticks) straight into the clip
fn put_notes(h: &mut Harness<'static, App>, notes: &[(usize, u8, tl::Tick)]) {
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let d = t.notes.last_mut().unwrap().make_mut();
        for &(row, key, len) in notes {
            d.notes.push(tl::Note {
                start: tl::rows_to_ticks(row),
                len,
                key,
                vel: tl::roll::vel_u16(1.0),
                ..Default::default()
            });
        }
    });
    frame(h);
}

/// the selected notes as (start row, key), sorted
fn sel_notes(h: &Harness<'static, App>) -> Vec<(tl::Tick, u8)> {
    let d = note_data(h);
    let mut v: Vec<_> = h
        .state()
        .pr
        .sel
        .iter()
        .map(|&i| (d.notes[i].start / tl::ROW, d.notes[i].key))
        .collect();
    v.sort();
    v
}

#[test]
fn pointer_drag_on_empty_space_box_selects_shift_adds_and_notes_still_move() {
    let mut h = clip_setup();
    let r = tl::ROW;
    assert_eq!(h.state().pr.tool, pianoview::PrTool::Pointer);
    put_notes(&mut h, &[(0, 60, r), (4, 62, r), (8, 64, r), (12, 66, r)]);
    // a plain drag from empty space: a box (no new note)
    {
        let p0 = at(&h, 3, 65, 2.0);
        let p1 = at(&h, 10, 61, 2.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    assert_eq!(note_data(&h).notes.len(), 4, "the box draws nothing");
    assert_eq!(sel_notes(&h), vec![(4, 62), (8, 64)]);
    // Shift+box adds
    {
        let p0 = at(&h, 11, 68, 2.0);
        let p1 = at(&h, 14, 65, 2.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::SHIFT);
    }
    assert_eq!(sel_notes(&h), vec![(4, 62), (8, 64), (12, 66)]);
    // a plain box again replaces the selection
    {
        let p0 = at(&h, 0, 62, 0.0);
        let p1 = at(&h, 2, 58, 2.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    assert_eq!(sel_notes(&h), vec![(0, 60)]);
    // dragging a note still moves it (and only that one: it was selected)
    {
        let p0 = at(&h, 0, 60, 5.0);
        let p1 = at(&h, 2, 61, 5.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    assert!(lens(&h).contains(&(2 * r, 61, r)), "{:?}", lens(&h));
    assert_eq!(note_data(&h).notes.len(), 4);
    // a click on empty space still draws a note
    {
        let p0 = at(&h, 20, 70, 5.0);
        click_at(&mut h, p0);
    }
    assert_eq!(note_data(&h).notes.len(), 5, "click draws");
    assert!(lens(&h).iter().any(|x| x.0 == 20 * r && x.1 == 70));
}

#[test]
fn alt_drag_draws_a_line_that_selects_the_notes_it_crosses() {
    let mut h = clip_setup();
    let r = tl::ROW;
    // five notes: three on C4 (the last one very short) and two on A#4
    put_notes(
        &mut h,
        &[(0, 60, r), (4, 70, r), (8, 60, r), (12, 70, r), (16, 60, 6)],
    );
    // C4 to B4 in view (the two-row toolbar leaves a shorter roll)
    {
        let pr = &mut h.state_mut().pr;
        pr.scroll_y = (120.0 - 64.0) * pr.key_h - pr.grid_rect.height() / 2.0;
    }
    frame(&mut h);
    let alt = egui::Modifiers::ALT;
    // a wavy line along C4 that dips below, never up to A#4
    let path = [
        at(&h, 0, 58, 5.0),
        at(&h, 0, 60, 8.0),
        at(&h, 5, 57, 0.0),
        at(&h, 8, 60, 10.0),
        at(&h, 13, 57, 0.0),
        at(&h, 16, 60, 1.0),
        at(&h, 18, 57, 0.0),
    ];
    left_path(&mut h, &path, alt);
    assert_eq!(note_data(&h).notes.len(), 5, "the line draws nothing");
    assert_eq!(
        sel_notes(&h),
        vec![(0, 60), (8, 60), (16, 60)],
        "3 of 5, the short one too"
    );
    assert!(h.state().pr.line.is_empty(), "the line goes when let go");
    // Shift+Alt adds the A#4 at row 4
    let shift_alt = egui::Modifiers::SHIFT | egui::Modifiers::ALT;
    let pts = [at(&h, 3, 71, 0.0), at(&h, 5, 68, 0.0)];
    left_path(&mut h, &pts, shift_alt);
    assert_eq!(sel_notes(&h), vec![(0, 60), (4, 70), (8, 60), (16, 60)]);
    // plain Alt replaces
    let pts = [at(&h, 11, 71, 0.0), at(&h, 14, 68, 0.0)];
    left_path(&mut h, &pts, alt);
    assert_eq!(sel_notes(&h), vec![(12, 70)]);
    assert_eq!(note_data(&h).notes.len(), 5);
}

#[test]
fn block_roll_remembers_the_length_too() {
    let (mut h, _) = setup(3);
    {
        let p0 = at(&h, 0, 61, 5.0);
        click_at(&mut h, p0);
    }
    {
        let p0 = at(&h, 0, 61, 18.0);
        let p1 = at(&h, 3, 61, 2.0);
        drag_m(&mut h, p0, p1, egui::Modifiers::NONE);
    }
    {
        let p0 = at(&h, 8, 63, 5.0);
        click_at(&mut h, p0);
    }
    let p = pat(&h);
    assert_eq!(p.cells[8][3].note, 63);
    assert_eq!(
        p.cells[11][3].note, NOTE_OFF,
        "three rows, like the last note"
    );
}

// ------------------------------------------------------------ chord lane

fn marks(h: &Harness<'static, App>) -> Vec<crate::chordgen::ChordMark> {
    note_data(h).chords
}

fn lane_at(h: &Harness<'static, App>, tick: tl::Tick) -> Pos2 {
    let r = h.state().cl.rect;
    Pos2::new(h.state().pr.x_of(tick) + 6.0, r.center().y)
}

fn dbl(h: &mut Harness<'static, App>, p: Pos2) {
    with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    let mut t = h.ctx.input(|i| i.time);
    for _ in 0..2 {
        for pressed in [true, false] {
            t += 0.05;
            h.input_mut().time = Some(t);
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
    }
    h.input_mut().time = None;
    frame(h);
    frame(h);
}

#[test]
fn chord_lane_adds_inserts_from_the_pie_and_undoes() {
    let mut h = clip_setup();
    h.state()
        .with(|s| s.song.info.key = Some(crate::chords::Key::new(4, crate::chords::Scale::Minor)));
    h.state_mut().pr.zoom = 8.0;
    frame(&mut h);
    assert!(
        h.state().cl.rect.height() > 10.0,
        "the lane shows on note clips"
    );
    let bar = 4 * PPQ_T;
    // double-click: the home chord, one bar
    let p = lane_at(&h, 0);
    dbl(&mut h, p);
    let m = marks(&h);
    assert_eq!(m.len(), 1, "{m:?}");
    assert_eq!(
        (m[0].start, m[0].len, m[0].root, m[0].q),
        (0, bar, 4, crate::chordgen::Quality::Min)
    );
    // right-click at bar 2: the style pie; Pop's first progression (with
    // the panel's Write set to Markers only; notes are the default)
    assert!(h.state().cl.gen.write, "Write notes is the default");
    h.state_mut().cl.gen.write = false;
    let p = lane_at(&h, 2 * bar);
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
    let (w, c) = {
        let (pie, _) = h.state().cl.pie.as_ref().expect("the pie opened");
        (pie.wedge_centre(0, 75.0), pie.child_centre(0, 0))
    };
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(w));
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(c));
    assert_eq!(
        h.state()
            .cl
            .pie
            .as_ref()
            .map(|p| (p.0.hover, p.0.hover_child)),
        Some((Some(0), Some(0)))
    );
    click_at(&mut h, c);
    assert!(h.state().cl.pie.is_none(), "picking closes it");
    let m = marks(&h);
    assert_eq!(m.len(), 5, "{m:?}");
    assert_eq!(m[1].start, 2 * bar);
    assert_eq!(
        note_data(&h).notes.len(),
        0,
        "markers only (Write set to Markers only)"
    );
    // right-click a chord: its pie; Delete
    let p = lane_at(&h, 0);
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
    let d = h
        .state()
        .cl
        .pie
        .as_ref()
        .expect("chord pie")
        .0
        .wedge_centre(6, 75.0);
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(d));
    click_at(&mut h, d);
    assert_eq!(marks(&h).len(), 4);
    assert_eq!(marks(&h)[0].start, 2 * bar);
    // one undo per action
    h.state_mut().do_undo(false);
    assert_eq!(marks(&h).len(), 5);
    h.state_mut().do_undo(false);
    assert_eq!(marks(&h).len(), 1);
    // empty chord lists save exactly as before
    let d = tl::NoteClipData {
        name: "x".into(),
        len: 8,
        ..Default::default()
    };
    let s = toml::to_string(&d).unwrap();
    assert!(!s.contains("chords"), "{s}");
}
