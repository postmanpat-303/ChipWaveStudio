//! On-screen transpose controls: the Block bar buttons (-12 -1 +1 +12), the
//! Edit > Transpose submenu and its "Transpose by…" amount. Every one of them
//! calls `App::transpose` (src/app/edit.rs), the same path as Alt+= / Alt+-,
//! so they transpose the selection (or the cursor channel without one), are
//! undoable and leave a status message.
use super::*;

/// (semitones, Block bar label, shortcut)
pub(super) const STEPS: [(i32, &str, &str); 4] = [
    (-12, "-12", "Alt+Shift+-"),
    (-1, "-1", "Alt+-"),
    (1, "+1", "Alt+="),
    (12, "+12", "Alt+Shift+="),
];

/// range of the "Transpose by…" amount
pub(super) const BY_RANGE: std::ops::RangeInclusive<i32> = -24..=24;

fn amount_text(semis: i32) -> String {
    match semis.abs() {
        0 => "no change".into(),
        12 => format!("{} octave", if semis > 0 { "up an" } else { "down an" }),
        24 => format!("{} two octaves", if semis > 0 { "up" } else { "down" }),
        1 => format!("{} a semitone", if semis > 0 { "up" } else { "down" }),
        n => format!("{} {n} semitones", if semis > 0 { "up" } else { "down" }),
    }
}

impl App {
    /// what a transpose would affect right now: "selection (…)" or "channel N"
    pub(super) fn transpose_target(&self) -> String {
        match self.sel_rect() {
            Some((r0, r1, c0, c1)) => {
                let n = self.rows();
                let rows = format!("rows {}–{}", self.row_text(r0, n), self.row_text(r1, n));
                if c0 == c1 {
                    format!("selection ({rows}, channel {})", c0 + 1)
                } else {
                    format!("selection ({rows}, channels {}–{})", c0 + 1, c1 + 1)
                }
            }
            None => format!("channel {} (whole block {:02})", self.ch + 1, self.pattern),
        }
    }

    fn transpose_tip(&self, semis: i32, shortcut: &str) -> String {
        let mut a = amount_text(semis);
        a[..1].make_ascii_uppercase();
        format!(
            "{a}: {}\nShortcut: {shortcut} · Ctrl+Z undoes",
            self.transpose_target()
        )
    }

    /// Block bar: "Transpose" and the -12 / -1 / +1 / +12 buttons
    pub(super) fn transpose_bar(&mut self, ui: &mut egui::Ui) {
        let target = if self.sel_rect().is_some() {
            "selection"
        } else {
            "channel"
        };
        ui.label("Transpose").on_hover_text(format!(
            "transpose the {target}: -12 / +12 = octave, -1 / +1 = semitone \
             (Alt+- / Alt+=, add Shift for octaves). Edit › Transpose has more, including any amount."
        ));
        for (semis, label, sc) in STEPS {
            let tip = self.transpose_tip(semis, sc);
            if ui.small_button(label).on_hover_text(tip).clicked() {
                self.transpose(semis);
            }
        }
    }

    /// Edit > Transpose submenu
    pub(super) fn transpose_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(format!("Affects: {}", self.transpose_target()))
                .small()
                .color(th().ink2),
        );
        let items: [(&str, i32, &str); 4] = [
            ("Up semitone", 1, "Alt+="),
            ("Down semitone", -1, "Alt+-"),
            ("Up octave", 12, "Alt+Shift+="),
            ("Down octave", -12, "Alt+Shift+-"),
        ];
        for (text, semis, sc) in items {
            let tip = self.transpose_tip(semis, sc);
            if ui
                .add(egui::Button::new(text).shortcut_text(sc))
                .on_hover_text(tip)
                .clicked()
            {
                self.transpose(semis);
                ui.close();
            }
        }
        ui.separator();
        // the amount lives in egui memory (not saved), starting at +7 (a fifth)
        let id = egui::Id::unique("transpose_by_amount");
        ui.menu_button("Transpose by…", |ui| {
            let mut n: i32 = ui.data(|d| d.get_temp(id)).unwrap_or(7);
            ui.horizontal(|ui| {
                ui.label("Semitones");
                ui.add(
                    egui::DragValue::new(&mut n)
                        .range(BY_RANGE)
                        .speed(0.2)
                        .custom_formatter(|v, _| format!("{:+}", v as i32)),
                )
                .on_hover_text("-24 to +24 semitones (12 = one octave)");
            });
            ui.data_mut(|d| d.insert_temp(id, n));
            let tip = self.transpose_tip(n, "Alt+= / Alt+- step by 1");
            if ui
                .add_enabled(n != 0, egui::Button::new(format!("Transpose {n:+}")))
                .on_hover_text(tip)
                .clicked()
            {
                self.transpose(n);
                ui.close();
            }
        });
    }
}
