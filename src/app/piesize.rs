//! How big the pie menus are drawn (Del, 25 Oct 2026: "pie menu a bit
//! small"): one factor on every pie's radii, icons and labels (the clip,
//! empty-lane and chord pies, the track pie and its Add ring). Settings ›
//! Look › Pie menus picks Small (the old size), Normal (30 % bigger, the
//! default) or Large; it shrinks when the window is too small for the
//! biggest pie (the track pie's Add ring) to fit.
use eframe::egui;
use std::cell::Cell;

/// the old size (before 25 Oct 2026)
pub(crate) const SMALL: f32 = 1.0;
/// the default: 30 % bigger
pub(crate) const NORMAL: f32 = 1.3;
pub(crate) const LARGE: f32 = 1.5;

/// the track pie's Add ring's outer fit radius at factor 1 (trackring::R_FIT)
const FIT_1: f32 = 274.0;

thread_local! {
    // per thread: the UI runs on one thread; tests run side by side
    static K: Cell<f32> = const { Cell::new(NORMAL) };
}

/// the factor in use
pub(crate) fn k() -> f32 {
    K.with(|k| k.get())
}

/// the setting's factor (0 Normal, 1 Small, 2 Large; Normal is 0 so old
/// settings files get the new default)
pub(crate) fn of_setting(v: u8) -> f32 {
    match v {
        1 => SMALL,
        2 => LARGE,
        _ => NORMAL,
    }
}

/// the factor for a window: the chosen size, smaller when the biggest pie
/// would not fit in `screen` (never below what fits, nor above the choice)
pub(crate) fn fit(user: f32, screen: egui::Rect) -> f32 {
    let room = (screen.width().min(screen.height()) / 2.0 - 1.0) / FIT_1;
    user.min(room).max(0.6)
}

/// set the factor for this frame
pub(crate) fn set(user: f32, screen: egui::Rect) {
    K.with(|k| k.set(fit(user, screen)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_is_about_thirty_percent_bigger_and_fits_1366x768() {
        let sc = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1366.0, 768.0));
        assert_eq!(fit(of_setting(0), sc), NORMAL);
        assert!((1.25..=1.35).contains(&NORMAL));
        // Large is trimmed to what fits
        let l = fit(of_setting(2), sc);
        assert!(l > NORMAL && FIT_1 * l <= 384.0, "{l}");
        // a small window shrinks it
        let tiny = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0));
        assert!(FIT_1 * fit(NORMAL, tiny) <= 250.0);
    }

    #[test]
    fn every_pie_fits_at_1366x768_opened_at_an_edge_and_reads() {
        use crate::app::{radial, trackpie, trackring};
        let sc = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1366.0, 768.0));
        set(of_setting(0), sc);
        let k = k();
        assert!((1.25..=1.35).contains(&k), "{k}");
        // wedge labels: at least 12 pt at Text size 100 %
        assert!(9.5 * k >= 12.0 && 10.5 * k >= 13.0);
        let inside = |c: egui::Pos2, r: f32| {
            egui::Rect::from_center_size(c, egui::Vec2::splat(2.0 * r)).shrink(-0.01)
        };
        for p in [
            egui::pos2(0.0, 0.0),
            egui::pos2(1366.0, 768.0),
            egui::pos2(5.0, 400.0),
            egui::pos2(1360.0, 10.0),
            egui::pos2(683.0, 767.0),
        ] {
            // the clip / empty-lane / chord pies
            let mut pie = radial::Radial::new(p, "Take 1", "", vec![]);
            pie.clamp_to(sc);
            assert!(sc.contains_rect(inside(pie.at, radial::r_out())), "{p:?}");
            // the track pie and its 13-slot Add ring
            let c = trackpie::clamp_centre(p, sc);
            assert!(sc.contains_rect(inside(c, trackring::r2())), "{p:?} {c:?}");
            assert!(sc.contains_rect(inside(c, trackpie::r_out())));
        }
        // 13 slots: each wide enough for a two-line label at the bigger size
        let rm = (trackring::r1() + trackring::r2()) / 2.0;
        let chord = 2.0 * rm * (std::f32::consts::TAU / 13.0 / 2.0).sin();
        assert!(chord * 0.86 >= 13.0 * k.sqrt() * 6.0, "{chord}");
    }
}
