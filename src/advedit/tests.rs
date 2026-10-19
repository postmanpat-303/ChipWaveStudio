use super::*;
use crate::model::{Pattern, Song};

fn n(note: u8, instr: u8) -> Cell {
    Cell {
        note,
        instr,
        ..Default::default()
    }
}

/// a 2-channel song with one 8-row block (and a second block)
fn song() -> Song {
    let mut s = Song::empty();
    s.channels = 2;
    s.patterns = vec![Pattern::new("A", 8, 2), Pattern::new("B", 8, 2)];
    for r in 0..8 {
        s.patterns[0].cells[r][0] = Cell {
            note: 49 + r as u8,
            instr: 1,
            vol: 0,
            fx: 0x0C,
            fxp: r as u8,
        };
    }
    s.patterns[0].cells[0][1] = n(30, 2);
    s.patterns[1].cells[0][0] = n(60, 1);
    s
}

fn cur() -> Cursor {
    Cursor {
        pattern: 0,
        ch: 0,
        col: 0,
        sel: None,
    }
}

fn area(s: &Song, sec: Section, mask: Mask) -> Area {
    resolve(s, sec, mask, cur()).unwrap()
}

fn col(s: &Song, b: usize, ch: usize) -> Vec<u8> {
    s.patterns[b].cells.iter().map(|r| r[ch].note).collect()
}

#[test]
fn sections_resolve_to_blocks_channels_and_rows() {
    let s = song();
    let c = Cursor {
        sel: Some((5, 2, 1, 0)),
        col: 3,
        ch: 1,
        ..cur()
    };
    let a = resolve(&s, Section::Selection, Mask::default(), c).unwrap();
    assert_eq!(
        (a.blocks.clone(), a.rows, a.chans),
        (vec![0], Some((2, 5)), (0, 1))
    );
    // the column sections narrow the mask to the cursor's column (volume here)
    let a = resolve(&s, Section::ColumnSong, Mask::default(), c).unwrap();
    assert_eq!(
        (a.blocks.clone(), a.chans, a.mask),
        (vec![0, 1], (1, 1), Mask::only(Field::Vol))
    );
    assert!(resolve(&s, Section::ColumnPattern, Mask::only(Field::Note), c).is_err());
    assert!(resolve(&s, Section::Selection, Mask::default(), cur()).is_err());
    assert!(resolve(&s, Section::Song, Mask([false; 5]), cur()).is_err());
    let a = resolve(&s, Section::Song, Mask::default(), cur()).unwrap();
    assert_eq!((a.blocks, a.chans, a.rows), (vec![0, 1], (0, 1), None));
}

#[test]
fn flip_reverses_only_the_masked_columns_of_the_rows() {
    let mut s = song();
    let a = area(&s, Section::TrackPattern, Mask::only(Field::Note));
    assert_eq!(reorder(&mut s, &a, Reorder::Flip), 8);
    assert_eq!(col(&s, 0, 0), vec![56, 55, 54, 53, 52, 51, 50, 49]);
    // effect values stayed on their rows; channel 2 untouched
    assert_eq!(s.patterns[0].cells[0][0].fxp, 0);
    assert_eq!(s.patterns[0].cells[7][0].fxp, 7);
    assert_eq!(s.patterns[0].cells[0][1], n(30, 2));
    // flipping twice is the original
    reorder(&mut s, &a, Reorder::Flip);
    assert_eq!(s.patterns, song().patterns);
}

#[test]
fn shrink_keeps_even_rows_packed_up_and_expand_doubles_the_spacing() {
    let mut s = song();
    let c = Cursor {
        sel: Some((0, 7, 0, 0)),
        ..cur()
    };
    let a = resolve(&s, Section::Selection, Mask::default(), c).unwrap();
    reorder(&mut s, &a, Reorder::Shrink);
    assert_eq!(col(&s, 0, 0), vec![49, 51, 53, 55, 0, 0, 0, 0]);
    assert_eq!(
        s.patterns[0].cells[1][0].fxp, 2,
        "every masked column moves"
    );
    reorder(&mut s, &a, Reorder::Expand);
    assert_eq!(col(&s, 0, 0), vec![49, 0, 51, 0, 53, 0, 55, 0]);
    // expanding a full column drops what falls past the end
    let mut s = song();
    reorder(&mut s, &a, Reorder::Expand);
    assert_eq!(col(&s, 0, 0), vec![49, 0, 50, 0, 51, 0, 52, 0]);
}

#[test]
fn safe_transpose_skips_notes_that_would_leave_the_range() {
    let mut s = song();
    s.patterns[0].cells[1][0].note = 115;
    s.patterns[0].cells[2][0].note = NOTE_OFF;
    let a = area(&s, Section::TrackPattern, Mask::default());
    let (changed, skipped) = transpose(&mut s, &a, 12, true);
    assert_eq!((changed, skipped), (6, 1));
    assert_eq!(s.patterns[0].cells[0][0].note, 61);
    assert_eq!(s.patterns[0].cells[1][0].note, 115, "left alone");
    assert_eq!(
        s.patterns[0].cells[2][0].note, NOTE_OFF,
        "note-offs never move"
    );
    // without safe mode the note is clamped to B-9 (120)
    let mut s2 = song();
    s2.patterns[0].cells[1][0].note = 115;
    transpose(&mut s2, &a, 12, false);
    assert_eq!(s2.patterns[0].cells[1][0].note, 120);
    // no Note in the mask: nothing happens
    let a = area(&s, Section::TrackPattern, Mask::only(Field::Vol));
    assert_eq!(transpose(&mut s, &a, 1, true), (0, 0));
}

#[test]
fn mirror_reflects_around_the_pivot() {
    let mut s = song();
    let a = area(&s, Section::TrackPattern, Mask::default());
    mirror(&mut s, &a, 49, true);
    // C-4 stays, C#4 (50) -> B-3 (48), G-4 (56) -> F-3 (42)
    assert_eq!(col(&s, 0, 0), vec![49, 48, 47, 46, 45, 44, 43, 42]);
    // mirroring twice is the original
    mirror(&mut s, &a, 49, true);
    assert_eq!(s.patterns, song().patterns);
    // a low pivot pushes high notes below C-0: safe mode skips them
    let mut s = song();
    let (_, skipped) = mirror(&mut s, &a, 20, true);
    assert_eq!(skipped, 8);
    assert_eq!(s.patterns, song().patterns);
}

#[test]
fn fade_sets_volumes_from_to_over_the_rows_on_notes_only() {
    let mut s = song();
    s.patterns[0].cells[3][0].note = 0; // an empty-note row with no volume: left alone
    s.patterns[0].cells[3][0].vol = 0;
    let a = area(&s, Section::TrackPattern, Mask::default());
    fade(&mut s, &a, 0x40, 0x00);
    let vols: Vec<u8> = s.patterns[0].cells.iter().map(|r| r[0].vol).collect();
    // stored as value + 1; row 0 = 40, row 7 = 00, linear in between
    assert_eq!(vols, vec![0x41, 0x38, 0x2F, 0, 0x1C, 0x13, 0x0A, 0x01]);
    // presets: tracker 0F..01 scaled to 00..40
    assert_eq!(
        (scale_15(0x0F), scale_15(0x01), scale_15(0)),
        (0x40, 0x04, 0)
    );
    // without Vol in the mask nothing changes
    let mut s = song();
    let a = area(&s, Section::TrackPattern, Mask::only(Field::Note));
    assert_eq!(fade(&mut s, &a, 0, 0x40), 0);
}

#[test]
fn mix_paste_only_fills_empty_cells_and_paste_respects_the_mask() {
    let mut s = song();
    let clip = vec![vec![n(70, 3), n(71, 3)], vec![n(72, 3), n(73, 3)]];
    // channel 1 rows 0-1 are full, channel 2 row 0 is full, row 1 empty
    let k = paste(&mut s, 0, &clip, (0, 0), Mask::default(), true);
    assert_eq!(k, 1);
    assert_eq!(s.patterns[0].cells[0][0].note, 49);
    assert_eq!(s.patterns[0].cells[0][1], n(30, 2));
    assert_eq!(s.patterns[0].cells[1][1], n(73, 3));
    // a normal paste of notes only keeps the instruments and effects
    let mut s = song();
    paste(&mut s, 0, &clip, (0, 0), Mask::only(Field::Note), false);
    assert_eq!(
        s.patterns[0].cells[0][0],
        Cell {
            note: 70,
            instr: 1,
            vol: 0,
            fx: 0x0C,
            fxp: 0
        }
    );
    // clipped at the last channel and the last row
    let mut s = song();
    assert_eq!(paste(&mut s, 0, &clip, (7, 1), Mask::default(), false), 1);
    // copy keeps only the masked columns
    let s = song();
    let a = area(&s, Section::TrackPattern, Mask::only(Field::Inst));
    let c = copy(&s, &a);
    assert_eq!((c.len(), c[0].len()), (8, 1));
    assert_eq!(
        c[3][0],
        Cell {
            instr: 1,
            ..Default::default()
        }
    );
}

#[test]
fn replace_instrument_in_the_section_only() {
    let mut s = song();
    let a = area(&s, Section::TrackPattern, Mask::default());
    assert_eq!(replace_instrument(&mut s, &a, 1, 4), 8);
    assert!(s.patterns[0].cells.iter().all(|r| r[0].instr == 4));
    assert_eq!(s.patterns[1].cells[0][0].instr, 1, "other blocks untouched");
    assert_eq!(s.patterns[0].cells[0][1].instr, 2);
    let a = area(&s, Section::Song, Mask::default());
    assert_eq!(replace_instrument(&mut s, &a, 2, 4), 1);
    assert_eq!(s.patterns[1].cells[0][0].instr, 1);
    // the mask must include Inst
    let a = area(&s, Section::Song, Mask::only(Field::Note));
    assert_eq!(replace_instrument(&mut s, &a, 4, 1), 0);
}

#[test]
fn song_sections_skip_derived_blocks_and_clear_obeys_the_mask() {
    let mut s = song();
    s.patterns[1].derived = Some(crate::arrange::Derived {
        source: 0,
        ops: vec![],
        seed: 0,
        drums: 0,
    });
    let a = area(&s, Section::Song, Mask::only(Field::FxVal));
    assert_eq!(a.blocks, vec![0]);
    clear(&mut s, &a);
    assert!(s.patterns[0]
        .cells
        .iter()
        .all(|r| r[0].fxp == 0 && r[0].fx == 0x0C && r[0].note > 0));
    assert_eq!(s.patterns[1].cells[0][0].note, 60);
}
