//! Pattern editing operations: selection, copy/cut/paste, clear, transpose, row shifting.
use super::*;

impl App {
    /// selection as (row0, row1, ch0, ch1), inclusive and clamped to the block
    pub(super) fn sel_rect(&self) -> Option<(usize, usize, usize, usize)> {
        let ((r0, c0), (r1, c1)) = self.sel?;
        let rows = self.rows();
        let chans = self.with(|s| s.song.channels);
        if rows == 0 || chans == 0 {
            return None;
        }
        Some((
            r0.min(r1).min(rows - 1),
            r0.max(r1).min(rows - 1),
            c0.min(c1).min(chans - 1),
            c0.max(c1).min(chans - 1),
        ))
    }

    pub(super) fn for_sel(&mut self, mut f: impl FnMut(&mut Cell)) -> bool {
        let Some((r0, r1, c0, c1)) = self.sel_rect() else {
            return false;
        };
        self.snapshot();
        let p = self.pattern;
        self.with(|s| {
            if let Some(p) = s.song.patterns.get_mut(p) {
                for row in &mut p.cells[r0..=r1] {
                    for c in &mut row[c0..=c1] {
                        f(c);
                    }
                }
            }
        });
        true
    }

    pub(super) fn copy(&mut self, cut: bool, ctx: &egui::Context) {
        // a song with no tracks (File > New) has no cells to copy
        if self.with(|s| s.song.channels) == 0 || self.rows() == 0 {
            self.status = "Nothing to copy: the song has no tracks yet".into();
            return;
        }
        let rect = self
            .sel_rect()
            .unwrap_or((self.row, self.row, self.ch, self.ch));
        let (r0, r1, c0, c1) = rect;
        let p = self.pattern;
        self.clip = self.with(|s| {
            s.song.patterns[p].cells[r0..=r1]
                .iter()
                .map(|r| r[c0..=c1].to_vec())
                .collect()
        });
        // mirror to the system clipboard; this also makes egui deliver
        // Event::Paste on Ctrl+V (it sends nothing when the clipboard is empty)
        ctx.copy_text(input::clip_to_text(&self.clip));
        if cut {
            if self.sel.is_none() {
                self.sel = Some(((r0, c0), (r1, c1)));
                self.clear_sel();
                self.sel = None;
            } else {
                self.clear_sel();
            }
        }
        self.status = format!(
            "{} {} rows x {} channels",
            if cut { "Cut" } else { "Copied" },
            r1 - r0 + 1,
            c1 - c0 + 1
        );
    }

    pub(super) fn paste(&mut self, text: Option<String>) {
        if let Some(c) = text.as_deref().and_then(input::clip_from_text) {
            self.clip = c;
        }
        if self.clip.is_empty() {
            self.status = "Nothing to paste (copy cells with Ctrl+C first)".into();
            return;
        }
        if !self.edit {
            self.status = "Edit mode is off (Esc) - paste ignored".into();
            return;
        }
        if self.with(|s| s.song.channels) == 0 || self.rows() == 0 {
            self.status = "Nothing to paste into: the song has no tracks yet".into();
            return;
        }
        self.snapshot();
        let (p, r0, c0) = (self.pattern, self.row, self.ch);
        let clip = self.clip.clone();
        self.with(|s| {
            let chans = s.song.channels;
            if let Some(p) = s.song.patterns.get_mut(p) {
                for (dr, row) in clip.iter().enumerate() {
                    let Some(dst) = p.cells.get_mut(r0 + dr) else {
                        break;
                    };
                    for (dc, cell) in row.iter().enumerate() {
                        if c0 + dc < chans {
                            dst[c0 + dc] = *cell;
                        }
                    }
                }
            }
        });
        let h = clip.len();
        let w = clip.first().map(|r| r.len()).unwrap_or(1);
        let last_r = (r0 + h - 1).min(self.rows() - 1);
        let last_c = (c0 + w - 1).min(self.with(|s| s.song.channels) - 1);
        self.sel = Some(((r0, c0), (last_r, last_c)));
        self.status = format!("Pasted {h} rows x {w} channels");
    }

    pub(super) fn clear_sel(&mut self) {
        if self.for_sel(|c| *c = Cell::default()) {
            self.status = "Cleared selection".into();
        }
    }

    pub(super) fn transpose(&mut self, semis: i32) {
        let tr = |c: &mut Cell| {
            if c.note > 0 && c.note != NOTE_OFF {
                c.note = (c.note as i32 + semis).clamp(1, 120) as u8;
            }
        };
        if self.for_sel(tr) {
            self.status = format!("Transposed selection by {semis:+} semitones");
            return;
        }
        self.snapshot();
        let (p, ch) = (self.pattern, self.ch);
        self.with(|s| {
            if let Some(p) = s.song.patterns.get_mut(p) {
                for row in &mut p.cells {
                    tr(&mut row[ch]);
                }
            }
        });
        self.status = format!("Transposed channel {} by {semis:+} semitones", self.ch + 1);
    }

    pub(super) fn shift_rows(&mut self, insert: bool) {
        self.snapshot();
        let (p, ch, row) = (self.pattern, self.ch, self.row);
        self.with(|s| {
            if let Some(p) = s.song.patterns.get_mut(p) {
                let n = p.rows;
                if insert {
                    for r in (row + 1..n).rev() {
                        p.cells[r][ch] = p.cells[r - 1][ch];
                    }
                    p.cells[row][ch] = Cell::default();
                } else if row > 0 {
                    for r in row - 1..n - 1 {
                        p.cells[r][ch] = p.cells[r + 1][ch];
                    }
                    p.cells[n - 1][ch] = Cell::default();
                }
            }
        });
        if !insert && self.row > 0 {
            self.row -= 1;
        }
    }
}
