//! Block (pattern) length (0.2.3.17): the Length field and quick buttons in
//! the Block bar, Song > Block menu, Ctrl+Shift+PgDn / PgUp, and the
//! "rows would be cut" confirmation. Every change is one undo step.
use super::*;

/// what to do with the current block's length
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum LenOp {
    /// new length in rows: growing adds empty rows, shrinking cuts the end
    Set(usize),
    /// x2 (stretch): double the length and spread the notes out (row r -> 2r)
    Stretch,
    /// /2 (squash): halve the length and pull the notes in (row r -> r/2)
    Squash,
}

/// quick lengths in the dropdown
pub(super) const PRESETS: [usize; 8] = [16, 32, 48, 64, 96, 128, 192, 256];

impl App {
    /// Change the current block's length, asking first when notes or
    /// effects would be lost.
    pub(super) fn block_length(&mut self, op: LenOp) {
        let p = self.pattern;
        let Some((rows, lost)) = self.with(|s| {
            s.song.patterns.get(p).map(|pt| {
                let lost = match op {
                    LenOp::Set(n) => pt.cut_by(n.clamp(1, max_rows(pt.rows))),
                    LenOp::Stretch => 0,
                    LenOp::Squash => pt.lost_by_squash(),
                };
                (pt.rows, lost)
            })
        }) else {
            return;
        };
        let max = max_rows(rows);
        match op {
            LenOp::Set(n) if n.clamp(1, max) == rows => return,
            LenOp::Stretch if rows * 2 > max => {
                self.status = format!("Block {p:02} can't be stretched past {max} rows");
                return;
            }
            LenOp::Squash if rows < 2 => return,
            _ => {}
        }
        if lost > 0 {
            self.len_confirm = Some((p, op, lost));
        } else {
            self.block_length_now(p, op);
        }
    }

    /// apply without asking (after the confirmation)
    pub(super) fn block_length_now(&mut self, p: usize, op: LenOp) {
        self.snapshot();
        let res = self.with(|s| {
            let pt = s.song.patterns.get_mut(p)?;
            let old = pt.rows;
            match op {
                LenOp::Set(n) => pt.set_rows(n.clamp(1, max_rows(old))),
                LenOp::Stretch => pt.stretch(),
                LenOp::Squash => pt.squash(),
            }
            let new = pt.rows;
            // the player never sits past the end of a shortened block
            if s.engine.pattern == p && s.engine.row >= new {
                s.engine.row = new - 1;
            }
            Some((old, new))
        });
        let Some((old, new)) = res else { return };
        if self.pattern == p {
            self.row = match op {
                LenOp::Stretch => (self.row * 2).min(new - 1),
                LenOp::Squash => self.row / 2,
                LenOp::Set(_) => self.row.min(new - 1),
            };
            self.sel = None;
        }
        self.dirty = true;
        let how = match op {
            LenOp::Set(_) => "",
            LenOp::Stretch => " (stretched: notes spread out)",
            LenOp::Squash => " (squashed: notes pulled in)",
        };
        self.status = format!("Block {p:02} length {old} to {new} rows{how} · Ctrl+Z undoes");
    }

    /// Length field, quick buttons and presets for the Block bar
    pub(super) fn block_len_controls(&mut self, ui: &mut egui::Ui, rows: usize) {
        ui.label("Length");
        let mut n = rows;
        let r = ui
            .add(
                egui::DragValue::new(&mut n)
                    .range(1..=max_rows(rows))
                    .speed(0.25),
            )
            .on_hover_text(
                "rows in this block (1–256). Growing adds empty rows; shrinking asks first \
                 if notes would be cut. Ctrl+Z undoes.",
            );
        // apply typed values when editing ends, dragged ones as they change
        if r.changed() && !r.has_focus() || r.lost_focus() && n != rows {
            self.block_length(LenOp::Set(n));
        }
        egui::ComboBox::from_id_salt("block_len_presets")
            .width(18.0)
            .selected_text("")
            .show_ui(ui, |ui| {
                for p in PRESETS {
                    if ui
                        .selectable_label(p == rows, format!("{p} rows"))
                        .clicked()
                    {
                        self.block_length(LenOp::Set(p));
                    }
                }
            })
            .response
            .on_hover_text("common block lengths");
        let small =
            |ui: &mut egui::Ui, t: &str, tip: &str| ui.small_button(t).on_hover_text(tip).clicked();
        if small(
            ui,
            "/2",
            "halve the length (keeps the first half; Ctrl+Shift+PgUp)",
        ) {
            self.block_length(LenOp::Set(rows.div_ceil(2)));
        }
        if small(
            ui,
            "x2",
            "double the length (adds empty rows; Ctrl+Shift+PgDn)",
        ) {
            self.block_length(LenOp::Set(rows * 2));
        }
        if small(ui, "-16", "16 rows shorter") {
            self.block_length(LenOp::Set(rows.saturating_sub(16).max(1)));
        }
        if small(ui, "+16", "16 rows longer") {
            self.block_length(LenOp::Set(rows + 16));
        }
    }

    /// Song > Block submenu
    pub(super) fn block_len_menu(&mut self, ui: &mut egui::Ui) {
        let rows = self.rows();
        ui.label(
            egui::RichText::new(format!("Block {:02}: {rows} rows", self.pattern))
                .small()
                .color(th().ink2),
        );
        ui.menu_button("Set length…", |ui| {
            let mut n = rows;
            ui.horizontal(|ui| {
                ui.label("Rows");
                let r = ui.add(egui::DragValue::new(&mut n).range(1..=max_rows(rows)));
                if r.changed() && !r.has_focus() || r.lost_focus() && n != rows {
                    self.block_length(LenOp::Set(n));
                }
            });
            ui.separator();
            for p in PRESETS {
                if ui
                    .selectable_label(p == rows, format!("{p} rows"))
                    .clicked()
                {
                    self.block_length(LenOp::Set(p));
                    ui.close();
                }
            }
        });
        let mut item = |ui: &mut egui::Ui, t: &str, sc: &str, tip: &str, op: LenOp| {
            if ui
                .add(egui::Button::new(t).shortcut_text(sc))
                .on_hover_text(tip)
                .clicked()
            {
                self.block_length(op);
                ui.close();
            }
        };
        item(
            ui,
            "Double length",
            "Ctrl+Shift+PgDn",
            "adds empty rows at the end",
            LenOp::Set(rows * 2),
        );
        item(
            ui,
            "Halve length",
            "Ctrl+Shift+PgUp",
            "keeps the first half (asks if notes would be cut)",
            LenOp::Set(rows.div_ceil(2)),
        );
        ui.separator();
        item(
            ui,
            "Stretch x2",
            "",
            "double the length and spread the notes out: row r moves to 2r (half speed)",
            LenOp::Stretch,
        );
        item(
            ui,
            "Squash /2",
            "",
            "halve the length and pull the notes in: row r moves to r/2 (double speed)",
            LenOp::Squash,
        );
    }

    /// "Cut N notes?" confirmation
    pub(super) fn block_len_dialog(&mut self, ctx: &egui::Context) {
        let Some((p, op, lost)) = self.len_confirm else {
            return;
        };
        let rows = self.with(|s| s.song.patterns.get(p).map(|x| x.rows).unwrap_or(0));
        let (mut yes, mut no) = (false, false);
        egui::Window::new("Shorten block?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                match op {
                    LenOp::Set(n) => ui.label(format!(
                        "Shortening block {p:02} from {rows} to {n} rows removes {lost} cell(s) with notes or effects after row {}.",
                        self.row_text(n.saturating_sub(1), rows)
                    )),
                    _ => ui.label(format!(
                        "Squashing block {p:02} to {} rows drops {lost} cell(s) that land on a row already in use.",
                        rows.div_ceil(2)
                    )),
                };
                ui.label("You can undo this with Ctrl+Z.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    yes = ui.button(format!("Remove {lost} cell(s) and resize")).clicked();
                    no = ui.button("Cancel").clicked();
                });
            });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            no = true;
        }
        if yes {
            self.len_confirm = None;
            self.block_length_now(p, op);
        } else if no {
            self.len_confirm = None;
        }
    }

    /// a row number as the user sees it (Decimal / Hex setting)
    pub(super) fn row_text(&self, r: usize, rows: usize) -> String {
        crate::model::row_label(r, rows, self.extra.row_hex)
    }
}

/// what the length controls allow (more if the block is already longer)
fn max_rows(current: usize) -> usize {
    Pattern::MAX_EDIT_ROWS.max(current)
}
