//! Advanced Edit: tracker block operations over a *section* (a selection, a
//! column, a track, a block or the whole song) limited by a *content mask*
//! (note / instrument / volume / effect number / effect value).
//!
//! Everything here is plain data work on a `Song` (no UI); the tracker's
//! Advanced Edit panel (src/app/advedit.rs) resolves the section into an
//! `Area` and calls one function per button, so each one is one undo step.
//! Guide: docs/ADVANCED_EDIT.md.
use crate::model::{Cell, Pattern, Song, NOTE_OFF};

/// what an operation works on
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Selection,
    /// the cursor's column (note, instrument, volume or effect) of the cursor's channel, this block
    ColumnPattern,
    /// the same column in every block
    ColumnSong,
    /// the cursor's channel (all columns the mask allows), this block
    TrackPattern,
    /// the cursor's channel in every block
    TrackSong,
    /// every channel of this block
    Pattern,
    /// every channel of every block
    Song,
}

impl Section {
    pub const ALL: [Section; 7] = [
        Section::Selection,
        Section::ColumnPattern,
        Section::ColumnSong,
        Section::TrackPattern,
        Section::TrackSong,
        Section::Pattern,
        Section::Song,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::Selection => "Selection in block",
            Section::ColumnPattern => "Column in block",
            Section::ColumnSong => "Column in song",
            Section::TrackPattern => "Track in block",
            Section::TrackSong => "Track in song",
            Section::Pattern => "Whole block",
            Section::Song => "Whole song",
        }
    }

    /// more than one block
    pub fn in_song(self) -> bool {
        matches!(
            self,
            Section::ColumnSong | Section::TrackSong | Section::Song
        )
    }
}

/// one column of a cell
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Note,
    Inst,
    Vol,
    FxNum,
    FxVal,
}

impl Field {
    pub const ALL: [Field; 5] = [
        Field::Note,
        Field::Inst,
        Field::Vol,
        Field::FxNum,
        Field::FxVal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Field::Note => "Note",
            Field::Inst => "Inst",
            Field::Vol => "Vol",
            Field::FxNum => "FX#",
            Field::FxVal => "FX val",
        }
    }

    /// the pattern cursor's sub-column (`App::col`: 0 note, 1-2 instrument,
    /// 3-4 volume, 5-6 effect number, 7-8 effect value)
    pub fn of_cursor(col: usize) -> Field {
        match col {
            0 => Field::Note,
            1 | 2 => Field::Inst,
            3 | 4 => Field::Vol,
            5 | 6 => Field::FxNum,
            _ => Field::FxVal,
        }
    }

    fn get(self, c: &Cell) -> u8 {
        match self {
            Field::Note => c.note,
            Field::Inst => c.instr,
            Field::Vol => c.vol,
            Field::FxNum => c.fx,
            Field::FxVal => c.fxp,
        }
    }

    fn set(self, c: &mut Cell, v: u8) {
        match self {
            Field::Note => c.note = v,
            Field::Inst => c.instr = v,
            Field::Vol => c.vol = v,
            Field::FxNum => c.fx = v,
            Field::FxVal => c.fxp = v,
        }
    }
}

/// which columns an operation may touch
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mask(pub [bool; 5]);

impl Default for Mask {
    fn default() -> Self {
        Mask([true; 5])
    }
}

impl Mask {
    pub fn has(&self, f: Field) -> bool {
        self.0[f as usize]
    }
    pub fn set(&mut self, f: Field, on: bool) {
        self.0[f as usize] = on;
    }
    pub fn only(f: Field) -> Mask {
        let mut m = Mask([false; 5]);
        m.set(f, true);
        m
    }
    pub fn none(&self) -> bool {
        !self.0.iter().any(|&b| b)
    }
    pub fn and(self, o: Mask) -> Mask {
        Mask(std::array::from_fn(|i| self.0[i] && o.0[i]))
    }
    pub fn fields(self) -> impl Iterator<Item = Field> {
        Field::ALL.into_iter().filter(move |&f| self.has(f))
    }
    /// "Note + Vol", "all columns"
    pub fn describe(&self) -> String {
        if self.0.iter().all(|&b| b) {
            return "all columns".into();
        }
        let v: Vec<&str> = self.fields().map(|f| f.label()).collect();
        if v.is_empty() {
            "nothing".into()
        } else {
            v.join(" + ")
        }
    }
}

/// where the cursor and selection are (what a section is resolved against)
#[derive(Clone, Copy, Debug, Default)]
pub struct Cursor {
    pub pattern: usize,
    pub ch: usize,
    /// sub-column, see `Field::of_cursor`
    pub col: usize,
    /// (row0, row1, ch0, ch1), inclusive
    pub sel: Option<(usize, usize, usize, usize)>,
}

/// a resolved section: blocks, rows (None = every row of each block),
/// channels (inclusive) and the columns allowed
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub blocks: Vec<usize>,
    pub rows: Option<(usize, usize)>,
    pub chans: (usize, usize),
    pub mask: Mask,
}

impl Area {
    fn rows_of(&self, p: &Pattern) -> Option<(usize, usize)> {
        if p.rows == 0 {
            return None;
        }
        let (a, b) = self.rows.unwrap_or((0, p.rows - 1));
        let b = b.min(p.rows - 1);
        (a <= b).then_some((a, b))
    }

    /// short text for the panel: "block 03 · channel 2 · rows 0–15 · Note + Vol"
    pub fn describe(&self, nblocks: usize) -> String {
        let blocks = match self.blocks.as_slice() {
            [b] => format!("block {b:02}"),
            v if v.len() == nblocks => format!("all {} blocks", v.len()),
            v => format!("{} blocks", v.len()),
        };
        let chans = if self.chans.0 == self.chans.1 {
            format!("channel {}", self.chans.0 + 1)
        } else {
            format!("channels {}–{}", self.chans.0 + 1, self.chans.1 + 1)
        };
        let rows = match self.rows {
            Some((a, b)) => format!(" · rows {a}–{b}"),
            None => String::new(),
        };
        format!("{blocks} · {chans}{rows} · {}", self.mask.describe())
    }
}

/// Turn `section` into an `Area` for `song` (Err = a message for the status line).
/// Song sections skip derived blocks (variations rendered from another block).
pub fn resolve(song: &Song, section: Section, mask: Mask, cur: Cursor) -> Result<Area, String> {
    if mask.none() {
        return Err("The content mask is empty: tick Note, Inst, Vol, FX# or FX val".into());
    }
    let chans = song.channels.max(1);
    let ch = cur.ch.min(chans - 1);
    let one = vec![cur.pattern.min(song.patterns.len().saturating_sub(1))];
    let all: Vec<usize> = (0..song.patterns.len())
        .filter(|&i| song.patterns[i].derived.is_none())
        .collect();
    let col_mask = |m: Mask| {
        let f = Field::of_cursor(cur.col);
        let m = m.and(Mask::only(f));
        if m.none() {
            Err(format!(
                "The cursor is in the {} column, which the mask leaves out",
                f.label()
            ))
        } else {
            Ok(m)
        }
    };
    let a = match section {
        Section::Selection => {
            let Some((r0, r1, c0, c1)) = cur.sel else {
                return Err(
                    "Nothing selected: Shift+arrows or drag to select, or pick another section"
                        .into(),
                );
            };
            Area {
                blocks: one,
                rows: Some((r0.min(r1), r0.max(r1))),
                chans: (c0.min(c1).min(chans - 1), c0.max(c1).min(chans - 1)),
                mask,
            }
        }
        Section::ColumnPattern => Area {
            blocks: one,
            rows: None,
            chans: (ch, ch),
            mask: col_mask(mask)?,
        },
        Section::ColumnSong => Area {
            blocks: all,
            rows: None,
            chans: (ch, ch),
            mask: col_mask(mask)?,
        },
        Section::TrackPattern => Area {
            blocks: one,
            rows: None,
            chans: (ch, ch),
            mask,
        },
        Section::TrackSong => Area {
            blocks: all,
            rows: None,
            chans: (ch, ch),
            mask,
        },
        Section::Pattern => Area {
            blocks: one,
            rows: None,
            chans: (0, chans - 1),
            mask,
        },
        Section::Song => Area {
            blocks: all,
            rows: None,
            chans: (0, chans - 1),
            mask,
        },
    };
    if a.blocks.is_empty() || song.patterns.is_empty() {
        return Err("No blocks to work on".into());
    }
    Ok(a)
}

/// copy the masked fields of `src` into `dst`
fn put(dst: &mut Cell, src: &Cell, m: Mask) {
    for f in m.fields() {
        f.set(dst, f.get(src));
    }
}

/// run `f` on every cell of the area (with the block's row range); returns
/// how many cells changed
fn each(song: &mut Song, a: &Area, mut f: impl FnMut(&mut Cell, usize, (usize, usize))) -> usize {
    let mut n = 0;
    for &b in &a.blocks {
        let Some(p) = song.patterns.get_mut(b) else {
            continue;
        };
        let Some((r0, r1)) = a.rows_of(p) else {
            continue;
        };
        for r in r0..=r1 {
            let Some(row) = p.cells.get_mut(r) else { break };
            for c in row.iter_mut().take(a.chans.1 + 1).skip(a.chans.0) {
                let before = *c;
                f(c, r, (r0, r1));
                n += (*c != before) as usize;
            }
        }
    }
    n
}

/// clear the masked columns
pub fn clear(song: &mut Song, a: &Area) -> usize {
    let m = a.mask;
    each(song, a, |c, _, _| put(c, &Cell::default(), m))
}

/// how rows move in `reorder`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reorder {
    /// reverse the rows
    Flip,
    /// keep the even rows (0, 2, 4 …), packed to the top; the rest is cleared
    Shrink,
    /// double the spacing: row i goes to row 2i (what falls past the end is lost)
    Expand,
}

/// Flip / Shrink / Expand the masked columns of each channel within the
/// area's rows (other columns stay where they are).
pub fn reorder(song: &mut Song, a: &Area, how: Reorder) -> usize {
    let mut n = 0;
    for &b in &a.blocks {
        let Some(p) = song.patterns.get_mut(b) else {
            continue;
        };
        let Some((r0, r1)) = a.rows_of(p) else {
            continue;
        };
        let len = r1 - r0 + 1;
        for ch in a.chans.0..=a.chans.1 {
            let col: Vec<Cell> = (r0..=r1)
                .map(|r| p.cells[r].get(ch).copied().unwrap_or_default())
                .collect();
            for i in 0..len {
                let src = match how {
                    Reorder::Flip => Some(len - 1 - i),
                    Reorder::Shrink => (2 * i < len).then_some(2 * i),
                    Reorder::Expand => i.is_multiple_of(2).then_some(i / 2),
                };
                let from = src.map(|s| col[s]).unwrap_or_default();
                if let Some(c) = p.cells[r0 + i].get_mut(ch) {
                    let before = *c;
                    put(c, &from, a.mask);
                    n += (*c != before) as usize;
                }
            }
        }
    }
    n
}

fn is_note(n: u8) -> bool {
    n > 0 && n != NOTE_OFF
}

/// notes with `f(note)`: in range 1..=120 it is used; out of range it is
/// clamped, or with `safe` the note is left alone. Returns (changed, skipped).
fn notes(song: &mut Song, a: &Area, safe: bool, f: impl Fn(i32) -> i32) -> (usize, usize) {
    if !a.mask.has(Field::Note) {
        return (0, 0);
    }
    let mut skipped = 0;
    let n = each(song, a, |c, _, _| {
        if !is_note(c.note) {
            return;
        }
        let v = f(c.note as i32);
        if (1..=120).contains(&v) {
            c.note = v as u8;
        } else if safe {
            skipped += 1;
        } else {
            c.note = v.clamp(1, 120) as u8;
        }
    });
    (n, skipped)
}

/// transpose by `semis`
pub fn transpose(song: &mut Song, a: &Area, semis: i32, safe: bool) -> (usize, usize) {
    notes(song, a, safe, |n| n + semis)
}

/// mirror the notes around `pivot` (49 = C-4): pivot + k becomes pivot - k
pub fn mirror(song: &mut Song, a: &Area, pivot: u8, safe: bool) -> (usize, usize) {
    let p = pivot as i32;
    notes(song, a, safe, |n| 2 * p - n)
}

/// highest volume value (the volume column shows 00..40)
pub const VOL_MAX: u8 = 0x40;

/// tracker-style "0F" presets scaled to ChipWave's 00..40 volume range
pub fn scale_15(v: u8) -> u8 {
    ((v.min(15) as u32 * VOL_MAX as u32 + 7) / 15) as u8
}

/// Volume fade from `from` to `to` (00..40) over the area's rows (each
/// block fades on its own): cells with a note or a volume get the value
/// for their row; empty cells and note-offs are left alone.
pub fn fade(song: &mut Song, a: &Area, from: u8, to: u8) -> usize {
    if !a.mask.has(Field::Vol) {
        return 0;
    }
    let (from, to) = (from.min(VOL_MAX) as f32, to.min(VOL_MAX) as f32);
    each(song, a, |c, r, (r0, r1)| {
        if !is_note(c.note) && c.vol == 0 {
            return;
        }
        let t = if r1 > r0 {
            (r - r0) as f32 / (r1 - r0) as f32
        } else {
            0.0
        };
        let v = (from + (to - from) * t).round() as u8;
        c.vol = v.min(VOL_MAX) + 1;
    })
}

/// instrument `from` becomes `to` (1-based slots)
pub fn replace_instrument(song: &mut Song, a: &Area, from: u8, to: u8) -> usize {
    if !a.mask.has(Field::Inst) || from == 0 || to == 0 || from == to {
        return 0;
    }
    each(song, a, |c, _, _| {
        if c.instr == from {
            c.instr = to;
        }
    })
}

/// the masked cells of a one-block area (rows x channels; columns outside
/// the mask are empty)
pub fn copy(song: &Song, a: &Area) -> Vec<Vec<Cell>> {
    let Some(p) = a.blocks.first().and_then(|&b| song.patterns.get(b)) else {
        return vec![];
    };
    let Some((r0, r1)) = a.rows_of(p) else {
        return vec![];
    };
    (r0..=r1)
        .map(|r| {
            (a.chans.0..=a.chans.1)
                .map(|ch| {
                    let mut c = Cell::default();
                    if let Some(s) = p.cells[r].get(ch) {
                        put(&mut c, s, a.mask);
                    }
                    c
                })
                .collect()
        })
        .collect()
}

/// Paste `clip` into block `block` with its top-left at (row, ch), clipped
/// to the block and the song's channels. Only masked columns are written;
/// with `mix` only cells that are empty (in the masked columns) are written,
/// so a mix-paste never overwrites anything.
pub fn paste(
    song: &mut Song,
    block: usize,
    clip: &[Vec<Cell>],
    (row, ch): (usize, usize),
    mask: Mask,
    mix: bool,
) -> usize {
    let chans = song.channels;
    let Some(p) = song.patterns.get_mut(block) else {
        return 0;
    };
    let mut n = 0;
    for (dr, src_row) in clip.iter().enumerate() {
        let Some(dst_row) = p.cells.get_mut(row + dr) else {
            break;
        };
        for (dc, src) in src_row.iter().enumerate() {
            if ch + dc >= chans {
                break;
            }
            let Some(dst) = dst_row.get_mut(ch + dc) else {
                break;
            };
            // mix-paste: only into cells that are empty in the masked columns
            if mix && mask.fields().any(|f| f.get(dst) != 0) {
                continue;
            }
            let before = *dst;
            put(dst, src, mask);
            n += (*dst != before) as usize;
        }
    }
    n
}

#[cfg(test)]
mod tests;
