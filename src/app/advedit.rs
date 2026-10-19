//! The tracker's Advanced Edit panel (Ctrl+Shift+A, or "Adv. Edit" in the
//! Block bar): a section to process, a content mask, and block operations
//! (cut / copy / paste / mix-paste, flip, shrink, expand, transpose with a
//! safe mode, mirror, volume fades, instrument replace). Docked right of the
//! pattern or floating. The operations themselves live in src/advedit.rs;
//! every button is one undo step. Guide: docs/ADVANCED_EDIT.md.
use super::*;
use crate::advedit::{self as ae, Area, Field, Mask, Reorder, Section};

/// width of the docked panel
pub(super) const ADV_W: f32 = 236.0;

/// the panel's settings (kept while the app runs)
pub(crate) struct AdvUi {
    pub(crate) section: Section,
    pub(crate) mask: Mask,
    /// transpose / mirror leave out-of-range notes alone (else they clamp)
    pub(crate) safe: bool,
    /// mirror pivot note (49 = C-4)
    pub(crate) pivot: u8,
    pub(crate) fade_from: u8,
    pub(crate) fade_to: u8,
    pub(crate) rep_from: u8,
    pub(crate) rep_to: u8,
}

impl Default for AdvUi {
    fn default() -> Self {
        Self {
            section: Section::Selection,
            mask: Mask::default(),
            safe: true,
            pivot: 49,
            fade_from: ae::VOL_MAX,
            fade_to: 0,
            rep_from: 1,
            rep_to: 1,
        }
    }
}

/// one Advanced Edit button
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AdvOp {
    Cut,
    Copy,
    Paste,
    MixPaste,
    Flip,
    Shrink,
    Expand,
    Clear,
    Transpose(i32),
    Mirror,
    /// volume fade from, to (00..40)
    Fade(u8, u8),
    Replace,
}

impl App {
    pub(super) fn toggle_adv_edit(&mut self) {
        self.extra.adv_edit_open = !self.extra.adv_edit_open;
        if self.extra.adv_edit_open && self.view != View::Pattern {
            self.view = View::Pattern;
        }
        self.save_settings();
        self.status = if self.extra.adv_edit_open {
            "Advanced Edit shown (Ctrl+Shift+A hides it)".into()
        } else {
            "Advanced Edit hidden (Ctrl+Shift+A)".into()
        };
    }

    fn adv_cursor(&self) -> ae::Cursor {
        ae::Cursor {
            pattern: self.pattern,
            ch: self.ch,
            col: self.col,
            sel: self.sel_rect(),
        }
    }

    pub(crate) fn adv_area(&self) -> Result<Area, String> {
        let (sec, mask, cur) = (self.adv.section, self.adv.mask, self.adv_cursor());
        self.with(|s| ae::resolve(&s.song, sec, mask, cur))
    }

    /// run one operation (one undo step); the status line says what happened
    pub(crate) fn adv_op(&mut self, op: AdvOp, ctx: Option<&egui::Context>) {
        let area = match self.adv_area() {
            Ok(a) => a,
            // a paste only needs the mask (it goes to the cursor)
            Err(e) if !matches!(op, AdvOp::Paste | AdvOp::MixPaste) || self.adv.mask.none() => {
                self.status = e;
                return;
            }
            Err(_) => Area {
                blocks: vec![self.pattern],
                rows: None,
                chans: (self.ch, self.ch),
                mask: self.adv.mask,
            },
        };
        let what = self.with(|s| area.describe(s.song.patterns.len()));
        let u = &self.adv;
        let (safe, pivot, rf, rt) = (u.safe, u.pivot, u.rep_from, u.rep_to);
        match op {
            AdvOp::Copy | AdvOp::Cut => {
                if self.adv.section.in_song() {
                    self.status = "Cut / Copy work on one block: pick a section in pattern".into();
                    return;
                }
                self.clip = self.with(|s| ae::copy(&s.song, &area));
                if let Some(ctx) = ctx {
                    // like Ctrl+C: the system clipboard too, so Ctrl+V pastes the same
                    ctx.copy_text(input::clip_to_text(&self.clip));
                }
                let (h, w) = (self.clip.len(), self.clip.first().map_or(0, |r| r.len()));
                if op == AdvOp::Cut {
                    self.snapshot();
                    self.with(|s| ae::clear(&mut s.song, &area));
                }
                let verb = if op == AdvOp::Cut { "Cut" } else { "Copied" };
                self.status = format!("{verb} {h} rows x {w} channels ({})", area.mask.describe());
            }
            AdvOp::Paste | AdvOp::MixPaste => {
                if self.clip.is_empty() {
                    self.status = "Nothing to paste (Copy or Ctrl+C first)".into();
                    return;
                }
                let mix = op == AdvOp::MixPaste;
                let (b, at, clip, mask) = (
                    self.pattern,
                    (self.row, self.ch),
                    self.clip.clone(),
                    area.mask,
                );
                self.snapshot();
                let n = self.with(|s| ae::paste(&mut s.song, b, &clip, at, mask, mix));
                self.status = format!(
                    "{} {n} cells at row {}, channel {} ({})",
                    if mix { "Mix-pasted" } else { "Pasted" },
                    at.0,
                    at.1 + 1,
                    mask.describe()
                );
            }
            AdvOp::Flip | AdvOp::Shrink | AdvOp::Expand => {
                let (how, verb) = match op {
                    AdvOp::Flip => (Reorder::Flip, "Flipped"),
                    AdvOp::Shrink => (Reorder::Shrink, "Shrank"),
                    _ => (Reorder::Expand, "Expanded"),
                };
                self.snapshot();
                let n = self.with(|s| ae::reorder(&mut s.song, &area, how));
                self.status = format!("{verb} {what} ({n} cells changed)");
            }
            AdvOp::Clear => {
                self.snapshot();
                let n = self.with(|s| ae::clear(&mut s.song, &area));
                self.status = format!("Cleared {n} cells: {what}");
            }
            AdvOp::Transpose(_) | AdvOp::Mirror => {
                if !area.mask.has(Field::Note) {
                    self.status = "Tick Note in the content mask to change notes".into();
                    return;
                }
                self.snapshot();
                let (n, skipped) = self.with(|s| match op {
                    AdvOp::Transpose(k) => ae::transpose(&mut s.song, &area, k, safe),
                    _ => ae::mirror(&mut s.song, &area, pivot, safe),
                });
                let verb = match op {
                    AdvOp::Transpose(k) => format!("Transposed {k:+}"),
                    _ => format!("Mirrored around {}", note_name(pivot)),
                };
                let skip = if skipped > 0 {
                    format!(", {skipped} left alone (out of range, safe mode)")
                } else {
                    String::new()
                };
                self.status = format!("{verb}: {n} notes{skip} · {what}");
            }
            AdvOp::Fade(from, to) => {
                if !area.mask.has(Field::Vol) {
                    self.status = "Tick Vol in the content mask to fade".into();
                    return;
                }
                self.snapshot();
                let n = self.with(|s| ae::fade(&mut s.song, &area, from, to));
                self.status = format!("Volume fade {from:02X} -> {to:02X}: {n} cells · {what}");
            }
            AdvOp::Replace => {
                if !area.mask.has(Field::Inst) {
                    self.status = "Tick Inst in the content mask to replace instruments".into();
                    return;
                }
                self.snapshot();
                let n = self.with(|s| ae::replace_instrument(&mut s.song, &area, rf, rt));
                self.status = format!("Instrument {rf:02X} -> {rt:02X}: {n} cells · {what}");
            }
        }
        self.dirty = true;
    }

    /// the panel docked in `r` (right of the pattern)
    pub(super) fn adv_docked(&mut self, ui: &mut egui::Ui, r: Rect) {
        let t = th();
        ui.painter().rect_filled(r, 6, theme::shade(&t, 0.06));
        ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink(6.0)), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("adv_edit_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| self.adv_contents(ui));
        });
    }

    /// the floating window (when undocked)
    pub(super) fn adv_window(&mut self, ctx: &egui::Context) {
        if !(self.extra.adv_edit_open && self.extra.adv_edit_float && self.view == View::Pattern) {
            return;
        }
        let mut open = true;
        egui::Window::new("Advanced Edit")
            .id(egui::Id::unique("adv_edit_window"))
            .open(&mut open)
            .resizable(false)
            .default_width(ADV_W)
            .show(ctx, |ui| self.adv_contents(ui));
        if !open {
            self.toggle_adv_edit();
        }
    }

    fn adv_contents(&mut self, ui: &mut egui::Ui) {
        let t = th();
        ui.spacing_mut().item_spacing.y = 3.0;
        let head = |ui: &mut egui::Ui, s: &str| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(s).strong().size(12.5).color(t.accent));
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("A<>E  Advanced Edit").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("x")
                    .on_hover_text("hide (Ctrl+Shift+A)")
                    .clicked()
                {
                    self.toggle_adv_edit();
                }
                let fl = self.extra.adv_edit_float;
                if ui
                    .small_button(if fl { "Dock" } else { "Float" })
                    .on_hover_text(if fl {
                        "dock right of the pattern"
                    } else {
                        "a floating window"
                    })
                    .clicked()
                {
                    self.extra.adv_edit_float = !fl;
                    self.save_settings();
                }
            });
        });
        // what the next operation affects
        let aff = match self.adv_area() {
            Ok(a) => self.with(|s| a.describe(s.song.patterns.len())),
            Err(e) => e,
        };
        ui.add(egui::Label::new(egui::RichText::new(aff).small().color(t.ink2)).wrap());

        head(ui, "Section to process");
        for s in Section::ALL {
            ui.radio_value(&mut self.adv.section, s, s.label());
        }

        head(ui, "Content mask");
        egui::Grid::new("adv_mask")
            .num_columns(3)
            .spacing([6.0, 2.0])
            .show(ui, |ui| {
                for (i, f) in Field::ALL.into_iter().enumerate() {
                    let mut on = self.adv.mask.has(f);
                    let r = ui
                        .checkbox(&mut on, f.label())
                        .on_hover_text("right-click: this column only (solo mask)");
                    if r.changed() {
                        self.adv.mask.set(f, on);
                    }
                    if r.secondary_clicked() {
                        self.adv.mask = Mask::only(f);
                    }
                    if i == 2 {
                        ui.end_row();
                    }
                }
                if ui
                    .small_button("All")
                    .on_hover_text("every column")
                    .clicked()
                {
                    self.adv.mask = Mask::default();
                }
                ui.end_row();
            });
        ui.label(egui::RichText::new("RMB = solo mask").small().color(t.ink2));

        let mut op = None;
        head(ui, "Cut / Copy / Paste");
        let w3 = ((ui.available_width() - 8.0) / 3.0).max(40.0);
        let btn = |ui: &mut egui::Ui, txt: &str, tip: &str| {
            ui.add_sized([w3, 20.0], egui::Button::new(txt))
                .on_hover_text(tip)
                .clicked()
        };
        ui.horizontal(|ui| {
            if btn(ui, "Cut", "copy the section (masked columns) and clear it") {
                op = Some(AdvOp::Cut);
            }
            if btn(ui, "Copy", "copy the section (masked columns)") {
                op = Some(AdvOp::Copy);
            }
            if btn(ui, "Paste", "paste at the cursor, masked columns only") {
                op = Some(AdvOp::Paste);
            }
        });
        ui.horizontal(|ui| {
            if btn(ui, "Mix-Paste", "paste at the cursor only into empty cells") {
                op = Some(AdvOp::MixPaste);
            }
            if btn(ui, "Clear", "clear the masked columns of the section") {
                op = Some(AdvOp::Clear);
            }
        });
        ui.horizontal(|ui| {
            if btn(ui, "Flip", "reverse the rows") {
                op = Some(AdvOp::Flip);
            }
            if btn(ui, "Shrink", "keep the even rows, packed up (half length)") {
                op = Some(AdvOp::Shrink);
            }
            if btn(ui, "Expand", "double the spacing (row n goes to row 2n)") {
                op = Some(AdvOp::Expand);
            }
        });
        ui.label(
            egui::RichText::new("Shrink: even rows, pack up")
                .small()
                .color(t.ink2),
        );

        head(ui, "Notes");
        ui.checkbox(&mut self.adv.safe, "Safe mode").on_hover_text(
            "on: notes that would go below C-0 or above B-9 are left alone\noff: they stop at C-0 / B-9",
        );
        let w4 = ((ui.available_width() - 12.0) / 4.0).max(30.0);
        ui.horizontal(|ui| {
            for k in [-12, -1, 1, 12] {
                if ui
                    .add_sized([w4, 20.0], egui::Button::new(format!("{k:+}")))
                    .on_hover_text(format!("transpose the section by {k:+} semitones"))
                    .clicked()
                {
                    op = Some(AdvOp::Transpose(k));
                }
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button(format!("Mirror {}", note_name(self.adv.pivot)))
                .on_hover_text(
                    "mirror the notes around the pivot: a step above becomes a step below",
                )
                .clicked()
            {
                op = Some(AdvOp::Mirror);
            }
            ui.label("pivot");
            ui.add(
                egui::DragValue::new(&mut self.adv.pivot)
                    .range(1..=120)
                    .speed(0.2)
                    .custom_formatter(|v, _| note_name(v as u8)),
            )
            .on_hover_text("pivot note (C-4 by default)");
        });

        head(ui, "Vol fade");
        let hexv = |v: f64, _| format!("{:02X}", v as u8);
        ui.horizontal(|ui| {
            ui.label("From");
            ui.add(
                egui::DragValue::new(&mut self.adv.fade_from)
                    .range(0..=ae::VOL_MAX)
                    .speed(0.3)
                    .custom_formatter(hexv),
            );
            ui.label("To");
            ui.add(
                egui::DragValue::new(&mut self.adv.fade_to)
                    .range(0..=ae::VOL_MAX)
                    .speed(0.3)
                    .custom_formatter(hexv),
            );
            if ui
                .button("Fade")
                .on_hover_text(
                    "volumes from -> to over the section's rows (cells with a note or a volume)",
                )
                .clicked()
            {
                op = Some(AdvOp::Fade(self.adv.fade_from, self.adv.fade_to));
            }
        });
        let (hi, lo) = (ae::scale_15(0x0F), ae::scale_15(0x01));
        ui.horizontal(|ui| {
            if ui
                .add_sized([w3 * 1.5, 20.0], egui::Button::new("Fade 0F->01"))
                .on_hover_text(format!(
                    "fade out: {hi:02X} -> {lo:02X} (0F..01 scaled to ChipWave's 00..40)"
                ))
                .clicked()
            {
                op = Some(AdvOp::Fade(hi, lo));
            }
            if ui
                .add_sized([w3 * 1.5, 20.0], egui::Button::new("Fade 01->0F"))
                .on_hover_text(format!("fade in: {lo:02X} -> {hi:02X}"))
                .clicked()
            {
                op = Some(AdvOp::Fade(lo, hi));
            }
        });

        head(ui, "Instruments");
        let names: Vec<String> =
            self.with(|s| s.song.instruments.iter().map(|i| i.name.clone()).collect());
        if names.is_empty() {
            ui.label(
                egui::RichText::new("No instruments in this song")
                    .small()
                    .color(t.ink2),
            );
        } else {
            let n = names.len() as u8;
            self.adv.rep_from = self.adv.rep_from.clamp(1, n);
            self.adv.rep_to = self.adv.rep_to.clamp(1, n);
            let pick = |ui: &mut egui::Ui, id: &str, v: &mut u8| {
                let cur = format!("{:02X} {}", *v, names[*v as usize - 1]);
                egui::ComboBox::from_id_salt(id)
                    .width(ui.available_width() - 34.0)
                    .selected_text(egui::RichText::new(cur).monospace())
                    .show_ui(ui, |ui| {
                        for (i, nm) in names.iter().enumerate() {
                            ui.selectable_value(v, i as u8 + 1, format!("{:02X} {nm}", i + 1));
                        }
                    });
            };
            ui.horizontal(|ui| {
                ui.label("Replace");
                pick(ui, "adv_rep_from", &mut self.adv.rep_from);
            });
            ui.horizontal(|ui| {
                ui.label("with   ");
                pick(ui, "adv_rep_to", &mut self.adv.rep_to);
            });
            if ui
                .button("Replace instrument")
                .on_hover_text("every note of the first instrument in the section uses the second")
                .clicked()
            {
                op = Some(AdvOp::Replace);
            }
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Every button is one undo step (Ctrl+Z)")
                .small()
                .color(t.ink2),
        );
        if let Some(op) = op {
            let ctx = ui.ctx().clone();
            self.adv_op(op, Some(&ctx));
        }
    }
}
