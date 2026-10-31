//! Measured widths for rows of widgets (hdrfix): headers that sit several
//! buttons side by side ask these how wide their text really is (font,
//! Text size, UI scale), so a card or row can make room before anything
//! is drawn, instead of guessing with fixed pixel widths and letting the
//! last widgets paint over the first. Original ChipWave code.
use eframe::egui;

/// the width `text` lays out at in `ui` (one line, never wrapped)
pub(super) fn text_w(ui: &egui::Ui, text: impl Into<egui::WidgetText>) -> f32 {
    text.into()
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        )
        .size()
        .x
}

/// the width of a button (or menu button) showing `text`, with `pad` of
/// horizontal button padding on each side
pub(super) fn button_w(ui: &egui::Ui, text: impl Into<egui::WidgetText>, pad: f32) -> f32 {
    text_w(ui, text) + 2.0 * pad
}
