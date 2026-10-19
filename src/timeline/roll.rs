//! Piano roll edits (M3a/M3b), without UI.
//!
//! **Block columns (mono, M3a):** a column of a block read as notes on the
//! row grid. A note starts at a cell with a note number and lasts until the
//! next cell with a note or a note-off in that column (or the block end).
//! Edits are *local*: they touch only the cells of the notes involved, so
//! everything else (effect commands, lone instrument or volume changes)
//! stays exactly as it was, and drawing a note into silence and deleting it
//! again gives back identical cells.
//!
//! **Note clips (poly, M3b):** free-timed [`Note`]s in [`NoteClipData`].
use super::*;
use crate::model::{Cell, Pattern, NOTE_OFF};

/// One note of a block column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonoNote {
    pub row: usize,
    pub len: usize,
    /// tracker note number (1 = C-0)
    pub key: u8,
    pub instr: u8,
    /// volume column (0 = none, 1..=65)
    pub vol: u8,
    /// it ends on an explicit note-off (not on the next note / block end)
    pub off: bool,
}

fn cell(p: &Pattern, row: usize, col: usize) -> Cell {
    p.cells
        .get(row)
        .and_then(|r| r.get(col))
        .copied()
        .unwrap_or_default()
}

fn cell_mut(p: &mut Pattern, row: usize, col: usize) -> Option<&mut Cell> {
    p.cells.get_mut(row).and_then(|r| r.get_mut(col))
}

fn is_note(n: u8) -> bool {
    n > 0 && n != NOTE_OFF
}

/// The notes of column `col` (all rows of the block, breaks included).
pub fn col_notes(p: &Pattern, col: usize) -> Vec<MonoNote> {
    let rows = p.rows.min(p.cells.len());
    let mut out: Vec<MonoNote> = vec![];
    for r in 0..rows {
        let c = cell(p, r, col);
        if c.note == 0 {
            continue;
        }
        if let Some(last) = out.last_mut() {
            if last.len == 0 {
                last.len = r - last.row;
                last.off = c.note == NOTE_OFF;
            }
        }
        if is_note(c.note) {
            out.push(MonoNote {
                row: r,
                len: 0,
                key: c.note,
                instr: c.instr,
                vol: c.vol,
                off: false,
            });
        }
    }
    if let Some(last) = out.last_mut() {
        if last.len == 0 {
            last.len = rows - last.row;
        }
    }
    out
}

/// The note sounding at `row` (started at or before it), if any.
pub fn note_at(p: &Pattern, col: usize, row: usize) -> Option<MonoNote> {
    col_notes(p, col)
        .into_iter()
        .find(|n| n.row <= row && row < n.row + n.len)
}

/// is anything sounding just before `row`?
fn sounding_before(p: &Pattern, col: usize, row: usize) -> bool {
    (0..row).rev().find_map(|r| {
        let n = cell(p, r, col).note;
        (n != 0).then_some(is_note(n))
    }) == Some(true)
}

/// Write a note at `row` for `len` rows. Notes starting inside it are
/// replaced and note-offs inside it removed; a note-off is added at its end
/// unless something else (a note or note-off) is there already.
pub fn put_note(p: &mut Pattern, col: usize, row: usize, len: usize, key: u8, instr: u8, vol: u8) {
    let rows = p.rows.min(p.cells.len());
    if row >= rows || !is_note(key) {
        return;
    }
    let end = (row + len.max(1)).min(rows);
    for r in row + 1..end {
        if let Some(c) = cell_mut(p, r, col) {
            if c.note != 0 {
                c.note = 0;
                c.instr = 0;
                c.vol = 0;
            }
        }
    }
    if let Some(c) = cell_mut(p, row, col) {
        c.note = key;
        c.instr = instr;
        c.vol = vol;
    }
    if end < rows {
        if let Some(c) = cell_mut(p, end, col) {
            if c.note == 0 {
                c.note = NOTE_OFF;
            }
        }
    }
}

/// Delete the note starting at `row`. Its note-off goes too when it is now
/// pointless (nothing would be sounding there).
pub fn delete_note(p: &mut Pattern, col: usize, row: usize) -> bool {
    let Some(n) = col_notes(p, col).into_iter().find(|n| n.row == row) else {
        return false;
    };
    let before = sounding_before(p, col, row);
    if let Some(c) = cell_mut(p, row, col) {
        c.note = 0;
        c.instr = 0;
        c.vol = 0;
    }
    if n.off && !before {
        if let Some(c) = cell_mut(p, n.row + n.len, col) {
            if c.note == NOTE_OFF {
                c.note = 0;
            }
        }
    }
    true
}

/// Resize the note at `row` to `len` rows (moves its note-off).
pub fn resize_note(p: &mut Pattern, col: usize, row: usize, len: usize) {
    let Some(n) = col_notes(p, col).into_iter().find(|n| n.row == row) else {
        return;
    };
    if n.off {
        if let Some(c) = cell_mut(p, n.row + n.len, col) {
            if c.note == NOTE_OFF {
                c.note = 0;
            }
        }
    }
    put_note(p, col, row, len, n.key, n.instr, n.vol);
}

/// Move the note at `row` to `to_row` and `key` (keeps its length,
/// instrument and volume; effect commands stay on their rows).
pub fn move_note(p: &mut Pattern, col: usize, row: usize, to_row: usize, key: u8) {
    let Some(n) = col_notes(p, col).into_iter().find(|n| n.row == row) else {
        return;
    };
    // take it out with its own note-off
    if let Some(c) = cell_mut(p, row, col) {
        c.note = 0;
        c.instr = 0;
        c.vol = 0;
    }
    if n.off {
        if let Some(c) = cell_mut(p, n.row + n.len, col) {
            if c.note == NOTE_OFF {
                c.note = 0;
            }
        }
    }
    put_note(p, col, to_row, n.len, key, n.instr, n.vol);
}

/// Set the volume column of the note at `row` (velocity, 0 = none).
pub fn set_note_vol(p: &mut Pattern, col: usize, row: usize, vol: u8) {
    if let Some(c) = cell_mut(p, row, col) {
        if is_note(c.note) {
            c.vol = vol.min(65);
        }
    }
}

/// velocity 0..1 shown for a volume column value (none = full)
pub fn vol_to_vel(vol: u8) -> f32 {
    if vol == 0 {
        1.0
    } else {
        (vol - 1) as f32 / 64.0
    }
}

/// volume column for a velocity 0..1
pub fn vel_to_vol(v: f32) -> u8 {
    1 + (v.clamp(0.0, 1.0) * 64.0).round() as u8
}

// ---------------------------------------------------------------------------
// note clips

/// 16-bit velocity as 0..1
pub fn vel_f(v: u16) -> f32 {
    v as f32 / 65535.0
}

pub fn vel_u16(v: f32) -> u16 {
    (v.clamp(0.0, 1.0) * 65535.0).round() as u16
}

impl NoteClipData {
    /// keep notes sorted by (start, key)
    pub fn sort(&mut self) {
        self.notes.sort_by_key(|n| (n.start, n.key));
    }

    /// the length that holds every note (at least one bar of 4/4)
    pub fn fit_len(&self) -> Tick {
        self.notes
            .iter()
            .map(|n| n.start + n.len)
            .max()
            .unwrap_or(0)
            .max(4 * PPQ)
    }
}

/// nearest multiple of `g`
pub fn snap(t: Tick, g: Tick) -> Tick {
    let g = g.max(1);
    (t + g / 2).div_euclid(g) * g
}

/// Quantise note starts (and ends, if `ends`) to `grid` ticks; `strength`
/// 0..1 moves part of the way. Lengths stay at least one grid step.
pub fn quantise(notes: &mut [Note], idx: &[usize], grid: Tick, strength: f32, ends: bool) {
    let g = grid.max(1);
    let s = strength.clamp(0.0, 1.0) as f64;
    for &i in idx {
        let Some(n) = notes.get_mut(i) else { continue };
        let end = n.start + n.len;
        let q = |t: Tick| {
            let target = snap(t, g);
            t + ((target - t) as f64 * s).round() as Tick
        };
        n.start = q(n.start).max(0);
        let e = if ends { q(end) } else { n.start + n.len };
        n.len = (e - n.start).max(if ends { g.min(n.len.max(1)) } else { 1 });
    }
}

/// Transpose by `semis`, clamped to the tracker range (C-0..B-9).
pub fn transpose(notes: &mut [Note], idx: &[usize], semis: i32) {
    for &i in idx {
        if let Some(n) = notes.get_mut(i) {
            n.key = (n.key as i32 + semis).clamp(1, 120) as u8;
        }
    }
}

/// Write note data into a new block, one column per voice: overlapping notes
/// go to the next free column (first fit). Notes are rounded to rows.
/// Returns the block and how many columns it used, or an error if it needs
/// more than `max_cols` columns or 256 rows.
pub fn notes_to_block(
    data: &NoteClipData,
    len: Tick,
    max_cols: usize,
    name: &str,
    channels: usize,
) -> Result<(Pattern, usize), String> {
    let rows = ((len.max(ROW) + ROW - 1) / ROW) as usize;
    if rows > Pattern::MAX_EDIT_ROWS {
        return Err(format!("at most {} rows", Pattern::MAX_EDIT_ROWS));
    }
    let mut p = Pattern::new(name, rows, channels.max(1));
    // per column: the row where it is free again
    let mut free: Vec<usize> = vec![];
    let mut notes: Vec<&Note> = data.notes.iter().filter(|n| n.start < len).collect();
    notes.sort_by_key(|n| (n.start, n.key));
    for n in notes {
        let r0 = ((n.start + ROW / 2) / ROW).max(0) as usize;
        if r0 >= rows {
            continue;
        }
        let r1 = (((n.start + n.len).min(len) + ROW / 2) / ROW).max(r0 as i64 + 1) as usize;
        let col = match free.iter().position(|&f| f <= r0) {
            Some(c) => c,
            None => {
                if free.len() >= max_cols.min(channels) {
                    return Err(format!(
                        "needs more than {} columns (overlapping notes)",
                        max_cols.min(channels)
                    ));
                }
                free.push(0);
                free.len() - 1
            }
        };
        let vol = if n.vel == 0xFFFF {
            0
        } else {
            vel_to_vol(vel_f(n.vel))
        };
        put_note(&mut p, col, r0, r1 - r0, n.key, n.instr, vol);
        if let Some((fx, fxp)) = n.fx {
            if let Some(c) = cell_mut(&mut p, r0, col) {
                c.fx = fx;
                c.fxp = fxp;
            }
        }
        // a note may start on this note's note-off row (it replaces it)
        free[col] = r1;
    }
    Ok((p, free.len().max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(p: &Pattern, c: usize) -> Vec<Cell> {
        p.cells.iter().map(|r| r[c]).collect()
    }

    fn pat() -> Pattern {
        let mut p = Pattern::new("t", 16, 2);
        p.cells[0][0] = Cell {
            note: 49,
            instr: 1,
            ..Default::default()
        };
        p.cells[4][0] = Cell {
            note: NOTE_OFF,
            ..Default::default()
        };
        p.cells[8][0] = Cell {
            note: 52,
            instr: 2,
            vol: 33,
            fx: 0x0C,
            fxp: 0x20,
        };
        p.cells[10][0].fx = 0x01; // a slide on an empty row
        p.cells[10][0].fxp = 0x04;
        p.cells[12][0] = Cell {
            note: 56,
            instr: 1,
            ..Default::default()
        };
        p.cells[3][1] = Cell {
            note: 37,
            instr: 3,
            ..Default::default()
        };
        p
    }

    #[test]
    fn column_reads_as_notes() {
        let p = pat();
        let n = col_notes(&p, 0);
        assert_eq!(n.len(), 3);
        assert_eq!((n[0].row, n[0].len, n[0].key, n[0].off), (0, 4, 49, true));
        assert_eq!((n[1].row, n[1].len, n[1].vol, n[1].off), (8, 4, 33, false));
        assert_eq!((n[2].row, n[2].len), (12, 4), "to the block end");
        assert_eq!(note_at(&p, 0, 10).unwrap().row, 8);
        assert_eq!(note_at(&p, 0, 5), None, "silence after the note-off");
        assert_eq!(col_notes(&p, 1)[0].len, 13);
    }

    #[test]
    fn drawing_into_silence_and_deleting_gives_identical_cells() {
        let p0 = pat();
        let mut p = p0.clone();
        put_note(&mut p, 0, 5, 2, 61, 4, 0);
        let n = note_at(&p, 0, 5).unwrap();
        assert_eq!((n.row, n.len, n.key, n.instr, n.off), (5, 2, 61, 4, true));
        assert_eq!(p.cells[7][0].note, NOTE_OFF);
        assert!(delete_note(&mut p, 0, 5));
        assert_eq!(col(&p, 0), col(&p0, 0));
        assert_eq!(p, p0);
    }

    #[test]
    fn moving_and_moving_back_gives_identical_cells() {
        let p0 = pat();
        let mut p = p0.clone();
        // the first note (0..4, note-off) up a fifth and to row 1
        move_note(&mut p, 0, 0, 1, 56);
        let n = col_notes(&p, 0);
        assert_eq!((n[0].row, n[0].len, n[0].key, n[0].instr), (1, 4, 56, 1));
        assert_eq!(p.cells[5][0].note, NOTE_OFF);
        assert_eq!(p.cells[4][0].note, 0);
        move_note(&mut p, 0, 1, 0, 49);
        assert_eq!(p, p0);
        // effect commands never move: the 0C20 stays on row 8 with its note
        let mut q = p0.clone();
        move_note(&mut q, 0, 8, 9, 52);
        assert_eq!((q.cells[8][0].fx, q.cells[8][0].fxp), (0x0C, 0x20));
        assert_eq!(q.cells[9][0].vol, 33, "the volume (velocity) goes with it");
    }

    #[test]
    fn resizing_moves_the_note_off_and_back() {
        let p0 = pat();
        let mut p = p0.clone();
        resize_note(&mut p, 0, 0, 6);
        assert_eq!(p.cells[4][0].note, 0);
        assert_eq!(p.cells[6][0].note, NOTE_OFF);
        assert_eq!(note_at(&p, 0, 0).unwrap().len, 6);
        resize_note(&mut p, 0, 0, 4);
        assert_eq!(p, p0);
        // a note that ran into the next note gets a note-off when shortened
        resize_note(&mut p, 0, 8, 2);
        assert_eq!(p.cells[10][0].note, NOTE_OFF);
        assert_eq!(
            (p.cells[10][0].fx, p.cells[10][0].fxp),
            (0x01, 0x04),
            "fx kept"
        );
        // growing over the next note replaces it
        resize_note(&mut p, 0, 8, 8);
        assert_eq!(col_notes(&p, 0).len(), 2);
        assert_eq!(note_at(&p, 0, 15).unwrap().row, 8);
    }

    #[test]
    fn velocity_is_the_volume_column() {
        let mut p = pat();
        assert_eq!(vol_to_vel(0), 1.0);
        assert_eq!(vel_to_vol(vol_to_vel(33)), 33);
        for v in 1..=65 {
            assert_eq!(vel_to_vol(vol_to_vel(v)), v);
        }
        set_note_vol(&mut p, 0, 0, 17);
        assert_eq!(p.cells[0][0].vol, 17);
        set_note_vol(&mut p, 0, 4, 17);
        assert_eq!(p.cells[4][0].vol, 0, "not on a note-off");
    }

    #[test]
    fn quantise_transpose_and_convert_to_block() {
        let mut d = NoteClipData {
            notes: vec![
                Note {
                    start: 10,
                    len: 230,
                    key: 49,
                    ..Default::default()
                },
                Note {
                    start: 250,
                    len: 470,
                    key: 53,
                    ..Default::default()
                },
                Note {
                    start: 245,
                    len: 700,
                    key: 56,
                    vel: vel_from_7(64),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let all: Vec<usize> = (0..3).collect();
        quantise(&mut d.notes, &all, ROW, 1.0, false);
        assert_eq!(
            d.notes.iter().map(|n| n.start).collect::<Vec<_>>(),
            vec![0, 240, 240]
        );
        quantise(&mut d.notes, &[1], ROW, 0.5, false);
        transpose(&mut d.notes, &[0], 12);
        assert_eq!(d.notes[0].key, 61);
        transpose(&mut d.notes, &[0], 200);
        assert_eq!(d.notes[0].key, 120);
        d.notes[0].key = 49;
        let (p, cols) = notes_to_block(&d, 16 * ROW, 8, "N", 4).unwrap();
        assert_eq!(cols, 2, "two notes overlap");
        assert_eq!(p.cells[0][0].note, 49);
        assert_eq!(p.cells[1][0].note, 53, "replaces the note-off");
        assert_eq!(p.cells[1][1].note, 56);
        assert_eq!(p.cells[1][1].vol, vel_to_vol(vel_f(vel_from_7(64))));
        assert_eq!(p.cells[3][1].note, 0, "no note-off when 56 ends at row 4");
        assert_eq!(p.cells[4][1].note, NOTE_OFF);
        assert!(notes_to_block(&d, 16 * ROW, 1, "N", 4).is_err());
    }
}
