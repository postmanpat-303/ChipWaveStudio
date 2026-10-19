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
    }
}
