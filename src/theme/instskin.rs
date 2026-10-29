//! The built-in instrument skin (docs/INSTSKIN.md): one violet tint that
//! marks ChipWave's own instruments (every generator, the Drum Kit and
//! Sample instruments) wherever they're painted: the Instrument tab, the
//! Devices / Nested Device Chains instrument card and the Browser's
//! ChipWave Instruments rows. Racks, third-party CLAP / VST3 instruments and
//! effect cards stay neutral; the app accent stays for selection, focus and
//! transport.
//!
//! Two ways in:
//! - explicit: [`instskin`] gives the skin for the current theme; a panel
//!   paints [`InstSkin::panel`], [`InstSkin::stripe`], [`InstSkin::badge`]
//!   with it;
//! - scoped: [`SkinScope::enter`] makes the shared painters inside the scope
//!   (knobs, section cards, card pictures) pick the tint up through
//!   [`active`] / [`skin_or`]. The scope is per thread and ends when the
//!   guard drops, so nothing outside a built-in instrument's paint is tinted.
use super::{th, tone, Theme};
use crate::model::InstrumentKind;
use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Stroke};
use std::cell::Cell;

/// The skin's colours for one theme
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstSkin {
    /// knob dots / arcs / pointers, the header stripe, the badge outline
    pub tint: Color32,
    /// the tint as readable text (badge text, knob value readouts)
    pub text: Color32,
    /// the translucent wash over a panel's or card's own fill
    pub wash: Color32,
    /// the thin frame around a skinned panel
    pub edge: Color32,
    /// curves and marks on the dark picture wells
    pub bright: Color32,
}

/// unmultiplied 0xRRGGBB at alpha `a`
fn rgba(c: u32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied((c >> 16) as u8, (c >> 8) as u8, c as u8, a)
}

impl InstSkin {
    /// the violet skin for `t`: Slate and Dark Circuit get a light violet,
    /// Amiga's light Workbench a deeper plum (and a lighter curve for its
    /// navy picture wells). The Matrix gets teal: violet fought its green,
    /// and teal still sets the built-ins apart from the green accent.
    pub fn for_theme(t: &Theme) -> Self {
        // (tint, text, wash, wash alpha, edge alpha)
        let (tint, text, wash, wa, ea): (u32, u32, u32, u8, u8) = if t.name == super::MATRIX.name {
            (0x4FD8C8, 0x9FEFE4, 0x4FD8C8, 22, 110)
        } else if t.light {
            (0x7B3FC4, 0x4E1F8A, 0x8A4FD8, 36, 150)
        } else if t.ember {
            (0xA78BFA, 0xC4B5FD, 0xA78BFA, 26, 110)
        } else {
            (0xB39DFF, 0xCDBEFF, 0xB39DFF, 24, 115)
        };
        Self {
            tint: super::hex(tint),
            text: super::hex(text),
            wash: rgba(wash, wa),
            edge: rgba(tint, ea),
            bright: if t.light {
                tone(super::hex(tint), 0.45)
            } else {
                super::hex(text)
            },
        }
    }

    /// a skinned panel or card: the wash over its fill and the thin frame
    pub fn panel(&self, p: &egui::Painter, r: Rect, rad: f32) {
        p.rect_filled(r, rad, self.wash);
        p.rect_stroke(
            r,
            rad,
            Stroke::new(1.0, self.edge),
            egui::StrokeKind::Inside,
        );
    }

    /// the 3 px header stripe along the top of `r`
    pub fn stripe(&self, p: &egui::Painter, r: Rect, rad: f32) {
        let k = rad.clamp(0.0, 6.0) as u8;
        p.rect_filled(
            Rect::from_min_size(r.min, egui::vec2(r.width(), 3.0)),
            CornerRadius {
                nw: k,
                ne: k,
                sw: 0,
                se: 0,
            },
            self.tint,
        );
    }

    /// the type badge laid out in `ui` (caps, tinted fill, tint outline)
    pub fn badge(&self, ui: &mut egui::Ui, tag: &str) -> egui::Response {
        let g = ui.fonts_mut(|f| {
            f.layout_no_wrap(
                tag.to_uppercase(),
                super::label_font(crate::theme::ts::SMALL),
                self.text,
            )
        });
        let (r, resp) = ui.allocate_exact_size(egui::vec2(g.size().x + 14.0, 17.0), Sense::hover());
        self.paint_badge(ui.painter(), r, g);
        resp
    }

    /// a badge's width for `tag` (to right-align one before painting it)
    pub fn badge_galley(&self, p: &egui::Painter, tag: &str) -> std::sync::Arc<egui::Galley> {
        p.layout_no_wrap(
            tag.to_uppercase(),
            super::label_font(crate::theme::ts::SMALL),
            self.text,
        )
    }

    /// paint a badge with the laid-out label `g` into `r`
    pub fn paint_badge(&self, p: &egui::Painter, r: Rect, g: std::sync::Arc<egui::Galley>) {
        p.rect_filled(r, 4.0, self.tint.gamma_multiply(0.20));
        p.rect_stroke(
            r,
            4.0,
            Stroke::new(1.0, self.tint),
            egui::StrokeKind::Inside,
        );
        p.galley(r.center() - g.size() / 2.0, g, self.text);
    }
}

/// CHIPWAVE_INSTSKIN=off paints the neutral look (comparison shots)
fn switched_off() -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OFF.get_or_init(|| {
        std::env::var("CHIPWAVE_INSTSKIN").is_ok_and(|v| v.eq_ignore_ascii_case("off"))
    })
}

/// the built-in instrument skin for the current theme (None when switched
/// off)
pub fn instskin() -> Option<InstSkin> {
    (!switched_off()).then(|| InstSkin::for_theme(&th()))
}

/// does `k` paint in the built-in skin? Every ChipWave generator, the Drum
/// Kit and Sample instruments; not racks (a container) or plugins
pub fn is_builtin(k: &InstrumentKind) -> bool {
    !matches!(k, InstrumentKind::Plugin(_) | InstrumentKind::Rack(_))
}

thread_local! {
    static ACTIVE: Cell<Option<InstSkin>> = const { Cell::new(None) };
}

/// While alive, the shared painters on this thread (knobs, section cards,
/// card pictures) paint in `skin`; dropping it restores what was before.
/// Nests (an effect inside an instrument can enter `None`).
#[must_use = "the scope ends when the guard drops"]
pub struct SkinScope(Option<InstSkin>);

impl SkinScope {
    pub fn enter(skin: Option<InstSkin>) -> Self {
        Self(ACTIVE.with(|a| a.replace(skin)))
    }
}

impl Drop for SkinScope {
    fn drop(&mut self) {
        ACTIVE.with(|a| a.set(self.0));
    }
}

/// the skin of the enclosing [`SkinScope`], if any
pub fn active() -> Option<InstSkin> {
    ACTIVE.with(|a| a.get())
}

/// a highlight colour: the active skin's tint, else `c` (the app accent)
pub fn skin_or(c: Color32) -> Color32 {
    active().map_or(c, |s| s.tint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_nests_and_restores() {
        let s = InstSkin::for_theme(&crate::theme::SLATE);
        assert_eq!(active(), None);
        {
            let _a = SkinScope::enter(Some(s));
            assert_eq!(skin_or(Color32::RED), s.tint);
            {
                let _b = SkinScope::enter(None);
                assert_eq!(skin_or(Color32::RED), Color32::RED);
            }
            assert_eq!(active(), Some(s));
        }
        assert_eq!(active(), None);
    }

    #[test]
    fn per_theme_values() {
        use crate::theme::{AMIGA, DARK, SLATE};
        assert_eq!(
            InstSkin::for_theme(&SLATE).tint,
            crate::theme::hex(0xA78BFA)
        );
        assert_eq!(InstSkin::for_theme(&DARK).tint, crate::theme::hex(0xB39DFF));
        let a = InstSkin::for_theme(&AMIGA);
        assert_eq!(a.tint, crate::theme::hex(0x7B3FC4));
        // the navy wells get a lighter curve than the plum tint
        assert!(a.bright.r() > a.tint.r());
        // The Matrix: teal, readable on its panels and apart from its accent
        let m = InstSkin::for_theme(&crate::theme::MATRIX);
        assert_eq!(m.tint, crate::theme::hex(0x4FD8C8));
        for bg in [crate::theme::MATRIX.cart_top, crate::theme::MATRIX.pill] {
            assert!(crate::theme::contrast(m.text, bg) >= 4.5);
        }
        assert_ne!(m.tint, crate::theme::MATRIX.accent);
    }

    #[test]
    fn builtins_and_neutral() {
        use crate::model::{InstrumentKind as K, RackP};
        assert!(is_builtin(&K::Synth(Default::default())));
        assert!(is_builtin(&K::Gen(Default::default())));
        assert!(is_builtin(&K::Voice(Default::default())));
        assert!(!is_builtin(&K::Rack(RackP::default())));
    }
}
