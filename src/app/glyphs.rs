//! Small vector glyphs for the track header icon tiles (0.2.4 "inspired"
//! polish): one per instrument type tag (`params::type_tag`), plus audio
//! and Song/Master tracks. Drawn with strokes so they scale with the UI
//! and need no icon font.
use super::egui;
use egui::{Color32, Painter, Pos2, Rect, Stroke};

/// the glyph family of a type tag ("Synth", "Drum", "Voice", ... or
/// "Audio" / "Master" / "" for none)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Glyph {
    Wave,
    Saw,
    Bars,
    Mic,
    Drum,
    Spark,
    Noise,
    Stack,
    Grid,
    Plug,
    Sum,
    Note,
    // track icons (track pie › Icon)
    Keys,
    Square,
    HiHat,
    Guitar,
    Clef,
    Violin,
    Brass,
    Voice,
    // track pie segments
    Pencil,
    Palette,
    Tiles,
    Plus,
    Rack,
    Route,
    Folder,
    /// Save CPU (on the roadmap; not drawn anywhere yet)
    #[allow(dead_code)]
    Snow,
    Trash,
}

/// the glyph of a picked track icon (`None`: Auto, use the instrument's)
pub(crate) fn icon_glyph(i: crate::timeline::TrackIcon) -> Option<Glyph> {
    use crate::timeline::TrackIcon as I;
    Some(match i {
        I::Auto => return None,
        I::Keys => Glyph::Keys,
        I::Synth => Glyph::Saw,
        I::Sine => Glyph::Wave,
        I::Square => Glyph::Square,
        I::Drums => Glyph::Drum,
        I::HiHat => Glyph::HiHat,
        I::Guitar => Glyph::Guitar,
        I::Bass => Glyph::Clef,
        I::Mic => Glyph::Mic,
        I::Violin => Glyph::Violin,
        I::Brass => Glyph::Brass,
        I::Voice => Glyph::Voice,
        I::Sparkle => Glyph::Spark,
        I::Waveform => Glyph::Bars,
        I::Noise => Glyph::Noise,
        I::Pad => Glyph::Stack,
    })
}

/// a track's header glyph: its picked icon, else the instrument type's
pub(crate) fn track_glyph(icon: crate::timeline::TrackIcon, tag: &str) -> Glyph {
    icon_glyph(icon).unwrap_or_else(|| glyph_for(tag))
}

pub(crate) fn glyph_for(tag: &str) -> Glyph {
    match tag {
        "Synth" | "FM-4" => Glyph::Wave,
        "Moog" | "Analog" => Glyph::Saw,
        "Sample" | "Audio" => Glyph::Bars,
        "Voice" => Glyph::Mic,
        "Drum" | "Kit" => Glyph::Drum,
        "Chip FX" => Glyph::Spark,
        "Noise" => Glyph::Noise,
        "Wave" => Glyph::Stack,
        "Grid" => Glyph::Grid,
        "CLAP" | "VST3" => Glyph::Plug,
        "Master" => Glyph::Sum,
        _ => Glyph::Note,
    }
}

/// a rounded tile in `fill` with the glyph in `ink`
pub(crate) fn tile(p: &Painter, r: Rect, g: Glyph, fill: Color32, ink: Color32, polished: bool) {
    let rad = (r.width() * 0.22).clamp(2.0, 6.0);
    if polished {
        p.rect_filled(r, rad, fill);
        // a soft top light
        let hi = Rect::from_min_max(r.min, Pos2::new(r.right(), r.center().y));
        p.rect_filled(
            hi,
            egui::CornerRadius {
                nw: rad as u8,
                ne: rad as u8,
                sw: 0,
                se: 0,
            },
            Color32::from_white_alpha(18),
        );
    } else {
        p.rect_filled(r, 2, fill);
    }
    paint(p, r.shrink(r.width() * 0.22), g, ink);
}

pub(crate) fn paint(p: &Painter, r: Rect, g: Glyph, c: Color32) {
    let w = (r.width() / 9.0).clamp(1.0, 2.2);
    let s = Stroke::new(w, c);
    let at = |fx: f32, fy: f32| Pos2::new(r.left() + fx * r.width(), r.top() + fy * r.height());
    match g {
        Glyph::Wave => {
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let u = i as f32 / 16.0;
                    at(u, 0.5 - 0.38 * (u * std::f32::consts::TAU).sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, s));
        }
        Glyph::Saw => {
            p.add(egui::Shape::line(
                vec![
                    at(0.0, 0.8),
                    at(0.45, 0.2),
                    at(0.45, 0.8),
                    at(1.0, 0.2),
                    at(1.0, 0.8),
                ],
                s,
            ));
        }
        Glyph::Bars => {
            for (i, h) in [0.3f32, 0.7, 1.0, 0.55, 0.8, 0.35].iter().enumerate() {
                let x = r.left() + (i as f32 + 0.5) / 6.0 * r.width();
                let y0 = r.center().y - h * r.height() * 0.48;
                let y1 = r.center().y + h * r.height() * 0.48;
                p.line_segment([Pos2::new(x, y0), Pos2::new(x, y1)], s);
            }
        }
        Glyph::Mic => {
            let cap = Rect::from_min_max(at(0.32, 0.0), at(0.68, 0.62));
            p.rect_filled(cap, cap.width() * 0.5, c);
            let pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let a = std::f32::consts::PI * i as f32 / 10.0;
                    at(0.5 - 0.3 * a.cos(), 0.42 + 0.3 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, s));
            p.line_segment([at(0.5, 0.72), at(0.5, 1.0)], s);
        }
        Glyph::Drum => {
            p.circle_stroke(r.center(), r.width() * 0.42, s);
            p.circle_filled(r.center(), r.width() * 0.13, c);
        }
        Glyph::Spark => {
            p.add(egui::Shape::line(
                vec![
                    at(0.6, 0.0),
                    at(0.25, 0.55),
                    at(0.55, 0.55),
                    at(0.4, 1.0),
                    at(0.8, 0.4),
                    at(0.5, 0.4),
                    at(0.6, 0.0),
                ],
                s,
            ));
        }
        Glyph::Noise => {
            let ys = [0.5, 0.15, 0.8, 0.3, 0.9, 0.1, 0.65, 0.35, 0.5];
            let pts: Vec<Pos2> = ys
                .iter()
                .enumerate()
                .map(|(i, y)| at(i as f32 / (ys.len() - 1) as f32, *y))
                .collect();
            p.add(egui::Shape::line(pts, s));
        }
        Glyph::Stack => {
            for k in 0..3 {
                let y0 = 0.2 + k as f32 * 0.3;
                let pts: Vec<Pos2> = (0..=12)
                    .map(|i| {
                        let u = i as f32 / 12.0;
                        at(u, y0 - 0.12 * (u * std::f32::consts::TAU).sin())
                    })
                    .collect();
                p.add(egui::Shape::line(pts, Stroke::new(w * 0.85, c)));
            }
        }
        Glyph::Grid => {
            for i in 0..3 {
                for j in 0..3 {
                    p.circle_filled(
                        at(0.15 + i as f32 * 0.35, 0.15 + j as f32 * 0.35),
                        w * 1.1,
                        c,
                    );
                }
            }
        }
        Glyph::Plug => {
            let body = Rect::from_min_max(at(0.2, 0.3), at(0.8, 0.72));
            p.rect_filled(body, 2.0, c);
            p.line_segment([at(0.38, 0.0), at(0.38, 0.3)], s);
            p.line_segment([at(0.62, 0.0), at(0.62, 0.3)], s);
            p.line_segment([at(0.5, 0.72), at(0.5, 1.0)], s);
        }
        Glyph::Sum => {
            p.add(egui::Shape::line(
                vec![
                    at(0.85, 0.1),
                    at(0.15, 0.1),
                    at(0.55, 0.5),
                    at(0.15, 0.9),
                    at(0.85, 0.9),
                ],
                s,
            ));
        }
        Glyph::Note => {
            p.circle_filled(at(0.35, 0.8), r.width() * 0.17, c);
            p.line_segment([at(0.5, 0.78), at(0.5, 0.05)], s);
            p.line_segment([at(0.5, 0.05), at(0.85, 0.3)], s);
        }
        Glyph::Keys => {
            let b = Rect::from_min_max(at(0.0, 0.1), at(1.0, 0.9));
            p.rect_stroke(b, 1.5, s, egui::StrokeKind::Inside);
            for k in 1..4 {
                let x = b.left() + b.width() * k as f32 / 4.0;
                p.line_segment([Pos2::new(x, b.top()), Pos2::new(x, b.bottom())], s);
            }
            for k in [1.0f32, 2.0, 3.0] {
                let x = b.left() + b.width() * k / 4.0;
                let bk = Rect::from_center_size(
                    Pos2::new(x, b.top() + b.height() * 0.3),
                    egui::vec2(b.width() * 0.13, b.height() * 0.6),
                );
                p.rect_filled(bk, 0.5, c);
            }
        }
        Glyph::Square => {
            p.add(egui::Shape::line(
                vec![
                    at(0.0, 0.8),
                    at(0.0, 0.2),
                    at(0.5, 0.2),
                    at(0.5, 0.8),
                    at(1.0, 0.8),
                    at(1.0, 0.2),
                ],
                s,
            ));
        }
        Glyph::HiHat => {
            p.line_segment([at(0.05, 0.3), at(0.95, 0.3)], s);
            p.line_segment([at(0.2, 0.42), at(0.8, 0.42)], s);
            p.line_segment([at(0.5, 0.05), at(0.5, 1.0)], s);
            p.line_segment([at(0.25, 1.0), at(0.75, 1.0)], s);
        }
        Glyph::Guitar => {
            p.circle_stroke(at(0.32, 0.72), r.width() * 0.24, s);
            p.circle_filled(at(0.32, 0.72), r.width() * 0.07, c);
            p.line_segment([at(0.45, 0.58), at(0.95, 0.05)], s);
            p.line_segment([at(0.82, 0.05), at(1.0, 0.2)], s);
        }
        Glyph::Clef => {
            let pts: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let a =
                        -std::f32::consts::PI * 0.9 + i as f32 / 12.0 * std::f32::consts::PI * 1.4;
                    at(0.38 + 0.3 * a.cos(), 0.38 + 0.3 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts.clone(), s));
            p.line_segment([pts[pts.len() - 1], at(0.1, 1.0)], s);
            p.circle_filled(at(0.15, 0.38), w * 1.3, c);
            p.circle_filled(at(0.92, 0.25), w * 0.9, c);
            p.circle_filled(at(0.92, 0.52), w * 0.9, c);
        }
        Glyph::Violin => {
            p.circle_stroke(at(0.5, 0.42), r.width() * 0.2, s);
            p.circle_stroke(at(0.5, 0.75), r.width() * 0.25, s);
            p.line_segment([at(0.5, 0.0), at(0.5, 0.95)], s);
            p.line_segment([at(0.1, 0.1), at(0.95, 0.95)], Stroke::new(w * 0.8, c));
        }
        Glyph::Brass => {
            p.add(egui::Shape::line(
                vec![at(0.0, 0.42), at(0.6, 0.42), at(1.0, 0.1)],
                s,
            ));
            p.add(egui::Shape::line(
                vec![at(0.0, 0.58), at(0.6, 0.58), at(1.0, 0.9)],
                s,
            ));
            p.line_segment([at(1.0, 0.1), at(1.0, 0.9)], s);
            p.circle_filled(at(0.3, 0.3), w * 1.1, c);
        }
        Glyph::Voice => {
            p.circle_stroke(at(0.35, 0.35), r.width() * 0.22, s);
            p.add(egui::Shape::line(
                vec![
                    at(0.0, 1.0),
                    at(0.08, 0.7),
                    at(0.35, 0.62),
                    at(0.62, 0.7),
                    at(0.7, 1.0),
                ],
                s,
            ));
            for (k, h) in [(0.8f32, 0.18f32), (0.95, 0.32)] {
                p.line_segment([at(k, 0.4 - h), at(k, 0.4 + h)], s);
            }
        }
        Glyph::Pencil => {
            p.line_segment([at(0.15, 0.85), at(0.8, 0.2)], Stroke::new(w * 1.6, c));
            p.line_segment([at(0.0, 1.0), at(0.15, 0.85)], s);
            p.line_segment([at(0.72, 0.12), at(0.88, 0.28)], s);
        }
        Glyph::Palette => {
            p.circle_stroke(r.center(), r.width() * 0.46, s);
            for (fx, fy) in [(0.3f32, 0.35f32), (0.55, 0.22), (0.75, 0.42), (0.35, 0.65)] {
                p.circle_filled(at(fx, fy), w * 1.2, c);
            }
        }
        Glyph::Tiles => {
            for i in 0..3 {
                for j in 0..3 {
                    let q = Rect::from_min_size(
                        at(i as f32 * 0.36, j as f32 * 0.36),
                        egui::vec2(r.width() * 0.26, r.height() * 0.26),
                    );
                    p.rect_filled(q, 1.0, c);
                }
            }
        }
        Glyph::Plus => {
            p.line_segment([at(0.5, 0.05), at(0.5, 0.95)], s);
            p.line_segment([at(0.05, 0.5), at(0.95, 0.5)], s);
        }
        Glyph::Rack => {
            for k in 0..2 {
                let y0 = 0.1 + k as f32 * 0.45;
                let b = Rect::from_min_max(at(0.0, y0), at(1.0, y0 + 0.35));
                p.rect_stroke(b, 1.5, s, egui::StrokeKind::Inside);
                p.circle_filled(at(0.2, y0 + 0.175), w * 0.9, c);
            }
        }
        Glyph::Route => {
            p.circle_filled(at(0.12, 0.5), w * 1.6, c);
            p.circle_filled(at(0.88, 0.15), w * 1.4, c);
            p.circle_filled(at(0.88, 0.85), w * 1.4, c);
            p.add(egui::Shape::line(
                vec![at(0.12, 0.5), at(0.5, 0.5), at(0.88, 0.15)],
                s,
            ));
            p.line_segment([at(0.5, 0.5), at(0.88, 0.85)], s);
        }
        Glyph::Folder => {
            p.add(egui::Shape::closed_line(
                vec![
                    at(0.0, 0.2),
                    at(0.38, 0.2),
                    at(0.48, 0.32),
                    at(1.0, 0.32),
                    at(1.0, 0.88),
                    at(0.0, 0.88),
                ],
                s,
            ));
        }
        Glyph::Snow => {
            for k in 0..3 {
                let a = k as f32 * std::f32::consts::PI / 3.0;
                let (dx, dy) = (0.48 * a.cos(), 0.48 * a.sin());
                p.line_segment([at(0.5 - dx, 0.5 - dy), at(0.5 + dx, 0.5 + dy)], s);
            }
            p.circle_filled(r.center(), w * 1.1, c);
        }
        Glyph::Trash => {
            p.line_segment([at(0.0, 0.18), at(1.0, 0.18)], s);
            p.line_segment([at(0.35, 0.05), at(0.65, 0.05)], s);
            p.add(egui::Shape::line(
                vec![at(0.12, 0.18), at(0.2, 1.0), at(0.8, 1.0), at(0.88, 0.18)],
                s,
            ));
            p.line_segment([at(0.4, 0.35), at(0.42, 0.85)], s);
            p.line_segment([at(0.6, 0.35), at(0.58, 0.85)], s);
        }
    }
}
