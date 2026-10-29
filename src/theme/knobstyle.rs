//! Knob styles (Settings › Look › Knob style): six faces for every rotary
//! control. Classic is each theme's own knob (the studio knob on Slate and
//! The Matrix, the arc knob on the others); the other five replace only the
//! face, so titles, values and the modulation rings drawn round a knob
//! stay where they were. Purely visual: nothing here touches the audio.
use super::{hex, mix, th, tone, Theme};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum KnobStyle {
    #[default]
    Classic = 0,
    Flat,
    Chicken,
    Led,
    Metal,
    Pixel,
}

impl KnobStyle {
    pub const ALL: [KnobStyle; 6] = [
        KnobStyle::Classic,
        KnobStyle::Flat,
        KnobStyle::Chicken,
        KnobStyle::Led,
        KnobStyle::Metal,
        KnobStyle::Pixel,
    ];
    pub fn name(self) -> &'static str {
        match self {
            KnobStyle::Classic => "Classic",
            KnobStyle::Flat => "Flat ring",
            KnobStyle::Chicken => "Chicken-head",
            KnobStyle::Led => "LED ring",
            KnobStyle::Metal => "Brushed metal",
            KnobStyle::Pixel => "Pixel",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            KnobStyle::Classic => "The theme's own knob (the default)",
            KnobStyle::Flat => "A thin value ring round a plain disc",
            KnobStyle::Chicken => "Vintage pointer knob on a ticked scale",
            KnobStyle::Led => "A ring of LEDs lit up to the value",
            KnobStyle::Metal => "Knurled aluminium cap with a value ring",
            KnobStyle::Pixel => "Chunky 8-bit pixels",
        }
    }
    /// the saved number (Settings `knob_style`); unknown numbers are Classic
    pub fn from_u8(n: u8) -> Self {
        Self::ALL.get(n as usize).copied().unwrap_or_default()
    }
}

static STYLE: AtomicU8 = AtomicU8::new(0);
pub fn knob_style() -> KnobStyle {
    KnobStyle::from_u8(STYLE.load(Ordering::Relaxed))
}
pub fn set_knob_style(s: KnobStyle) {
    STYLE.store(s as u8, Ordering::Relaxed);
}

/// What a styled face shows (as `KnobFace`)
#[derive(Clone, Copy)]
pub struct Face {
    pub frac: f32,
    /// where the value arc starts (0.5 = bipolar)
    pub from: f32,
    pub dim: f32,
    pub locked: bool,
    /// discrete steps (1..=16 get a mark per step)
    pub steps: f32,
}

/// how far past `rad` (the value arc's radius) a styled face may paint:
/// the modulation rings start at `rad + 4.5`
pub const REACH: f32 = 4.0;

/// Paint `style` (not Classic) with its value arc at `rad`, centred on `c`.
pub fn paint(p: &egui::Painter, c: Pos2, rad: f32, k: Face, style: KnobStyle) {
    paint_with(p, c, rad, k, style, &th());
}

pub(crate) fn paint_with(
    p: &egui::Painter,
    c: Pos2,
    rad: f32,
    k: Face,
    style: KnobStyle,
    t: &Theme,
) {
    let mut kp = p.clone();
    if k.dim < 1.0 {
        kp.multiply_opacity(k.dim.clamp(0.45, 1.0));
    }
    let p = &kp;
    let v = k.frac.clamp(0.0, 1.0);
    let a0 = std::f32::consts::PI * 0.75;
    let span = std::f32::consts::PI * 1.5;
    let pt = |ang: f32, r: f32| c + egui::vec2(ang.cos(), ang.sin()) * r;
    let acc = if k.locked {
        t.ink2
    } else if t.ember {
        super::skin_or(t.acc_hot)
    } else {
        super::skin_or(t.accent)
    };
    let arc = |from: f32, to: f32, col: Color32, w: f32, r: f32| {
        let n = 32;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| pt(a0 + span * (from + (to - from) * i as f32 / n as f32), r))
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(w, col)));
    };
    let (lo, hi) = if v < k.from { (v, k.from) } else { (k.from, v) };
    let lit = |f: f32, half: f32| hi > lo + 0.002 && f >= lo - half && f <= hi + half;
    let va = a0 + span * v;
    let stepped = (1.0..=16.0).contains(&k.steps);
    let nsteps = k.steps.round() as usize;
    // a mark per step for the faces with a plain ring
    let step_dots = |r: f32| {
        if stepped {
            for s in 0..=nsteps {
                let f = s as f32 / nsteps as f32;
                p.circle_filled(pt(a0 + span * f, r), 1.0, t.ink2.gamma_multiply(0.6));
            }
        }
    };
    match style {
        KnobStyle::Classic => {}
        KnobStyle::Flat => {
            arc(0.0, 1.0, t.ink2.gamma_multiply(0.3), 2.0, rad);
            if hi > lo {
                arc(lo, hi, acc, 2.5, rad);
            }
            p.circle_filled(c, rad - 3.5, t.pill);
            p.line_segment(
                [pt(va, (rad - 3.5) * 0.3), pt(va, rad - 4.5)],
                Stroke::new(2.0, t.ink),
            );
            step_dots(rad + 3.0);
        }
        KnobStyle::Chicken => {
            // scale ticks round a bakelite skirt (lit across the value),
            // a cream chicken-head pointer
            let n = if stepped { nsteps + 1 } else { 11 };
            let half = 0.5 / (n - 1) as f32;
            for s in 0..n {
                let f = s as f32 / (n - 1) as f32;
                let on = lit(f, half);
                let col = if on { acc } else { t.ink2.gamma_multiply(0.6) };
                p.line_segment(
                    [pt(a0 + span * f, rad + 0.5), pt(a0 + span * f, rad + 3.5)],
                    Stroke::new(if on { 2.0 } else { 1.2 }, col),
                );
            }
            p.circle_filled(
                c + egui::vec2(0.0, 1.5),
                rad - 1.0,
                Color32::from_black_alpha(90),
            );
            p.circle_filled(c, rad - 1.5, hex(0x1A1512));
            p.circle_stroke(c, rad - 1.5, Stroke::new(1.0, hex(0x4A3F36)));
            p.circle_filled(c, rad - 3.5, hex(0x2A221D));
            let d = egui::vec2(va.cos(), va.sin());
            let n2 = egui::vec2(-d.y, d.x);
            let tip = c + d * (rad - 0.5);
            let back = c - d * (rad * 0.55);
            let (wb, wt) = (rad * 0.38, rad * 0.14);
            let cream = tone(hex(0xE8DCC0), -0.15);
            p.add(egui::Shape::convex_polygon(
                vec![back + n2 * wb, tip + n2 * wt, tip - n2 * wt, back - n2 * wb],
                cream,
                Stroke::new(1.0, hex(0x6E604A)),
            ));
            p.circle_filled(back, wb, cream);
            p.line_segment(
                [c + d * (rad * 0.1), tip - d * 1.5],
                Stroke::new(1.6, hex(0x1A1512)),
            );
            p.circle_filled(c, rad * 0.12, hex(0x8A7A5E));
        }
        KnobStyle::Led => {
            let n = if stepped {
                nsteps + 1
            } else if rad < 15.0 {
                13
            } else {
                17
            };
            let half = 0.5 / (n - 1) as f32;
            for s in 0..n {
                let f = s as f32 / (n - 1) as f32;
                let q = pt(a0 + span * f, rad + 0.5);
                if lit(f, half) {
                    p.circle_filled(q, 3.2, acc.gamma_multiply(0.25));
                    p.circle_filled(q, 1.9, acc);
                } else {
                    p.circle_filled(q, 1.7, t.dot_off);
                }
            }
            p.circle_filled(
                c + egui::vec2(0.0, 1.0),
                rad - 4.0,
                Color32::from_black_alpha(90),
            );
            p.circle_filled(c, rad - 4.5, hex(0x17191C));
            p.circle_stroke(c, rad - 4.5, Stroke::new(1.0, hex(0x3A3E44)));
            p.line_segment(
                [pt(va, (rad - 4.5) * 0.3), pt(va, rad - 6.0)],
                Stroke::new(2.0, hex(0xE9EBED)),
            );
        }
        KnobStyle::Metal => {
            arc(0.0, 1.0, t.ink2.gamma_multiply(0.3), 2.0, rad + 1.5);
            if hi > lo {
                arc(lo, hi, acc, 2.0, rad + 1.5);
            }
            let r = rad - 1.0;
            p.circle_filled(c + egui::vec2(0.0, 1.5), r, Color32::from_black_alpha(110));
            // a knurled rim, lit from the top left
            p.circle_filled(c, r, hex(0x6B7078));
            let teeth = (r * 2.2) as i32;
            for i in 0..teeth {
                let a = i as f32 / teeth as f32 * std::f32::consts::TAU;
                let l = 0.5 + 0.5 * (a + 2.2).sin();
                p.line_segment(
                    [pt(a, r - 2.5), pt(a, r)],
                    Stroke::new(1.0, mix(hex(0x3E4248), hex(0xC9CDD2), l)),
                );
            }
            // a brushed cap: fine concentric rings, a highlight top left
            let cap = r - 3.0;
            let rings = (cap / 1.2).max(1.0) as i32;
            for i in 0..rings {
                let rr = cap - i as f32 * 1.2;
                let m = 0.55 + 0.075 * (i as f32 * 1.7).sin() + 0.2 * (i as f32 / rings as f32);
                p.circle_filled(c, rr, mix(hex(0x8F949B), hex(0xE3E6EA), m));
            }
            p.circle_stroke(c, cap, Stroke::new(1.0, hex(0x50555C)));
            p.circle_filled(
                c + egui::vec2(-cap * 0.3, -cap * 0.35),
                cap * 0.35,
                Color32::from_white_alpha(28),
            );
            p.line_segment(
                [pt(va, cap * 0.25), pt(va, cap - 1.5)],
                Stroke::new(2.0, hex(0x1B1D20)),
            );
            step_dots(rad + 3.5);
        }
        KnobStyle::Pixel => {
            // a chunky pixel knob: cells snapped to a grid round the centre
            let px = (rad / 6.0).round().clamp(2.0, 4.0);
            let cell = |q: Pos2, col: Color32| {
                let g = c + ((q - c) / px).round() * px;
                p.rect_filled(Rect::from_center_size(g, egui::vec2(px, px)), 0.0, col);
            };
            let rb = rad - 3.0;
            let n = (rb / px).ceil() as i32 + 1;
            for iy in -n..=n {
                for ix in -n..=n {
                    let q = c + egui::vec2(ix as f32 * px, iy as f32 * px);
                    let d = (q - c).length();
                    if d <= rb - px * 0.6 {
                        let s = if iy < 0 && ix < 0 {
                            tone(t.pill, 0.18)
                        } else {
                            t.pill
                        };
                        cell(q, s);
                    } else if d <= rb + px * 0.4 {
                        cell(q, t.ink2.gamma_multiply(0.7));
                    }
                }
            }
            let m = if stepped { nsteps } else { 14 };
            let half = 0.5 / m as f32;
            let ar = rad + 2.5 - px * 0.5;
            for s in 0..=m {
                let f = s as f32 / m as f32;
                cell(
                    pt(a0 + span * f, ar),
                    if lit(f, half) { acc } else { t.dot_off },
                );
            }
            let k = (rb / px) as i32;
            for i in 0..k {
                cell(pt(va, i as f32 * px * 0.95), t.ink);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shapes(style: KnobStyle, t: &Theme, rad: f32, k: Face) -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            paint_with(ui.painter(), Pos2::new(100.0, 100.0), rad, k, style, t);
        });
        out.textures_delta.clear();
        out.shapes
    }

    #[test]
    fn every_style_stays_inside_the_modulation_rings() {
        for t in super::super::THEMES {
            for style in KnobStyle::ALL.into_iter().skip(1) {
                for rad in [9.0, 13.0, 16.0, 24.0, 30.0] {
                    for steps in [0.0, 3.0, 8.0] {
                        let k = Face {
                            frac: 0.7,
                            from: 0.0,
                            dim: 1.0,
                            locked: false,
                            steps,
                        };
                        let mut b = Rect::NOTHING;
                        for s in shapes(style, &t, rad, k) {
                            b = b.union(s.shape.visual_bounding_rect());
                        }
                        let reach = b.width().max(b.height()) / 2.0 - rad;
                        assert!(
                            reach <= REACH + 0.6,
                            "{} {:?} rad {rad}: reaches {reach}",
                            t.name,
                            style
                        );
                        assert!(b.width() > rad, "{style:?} paints something");
                    }
                }
            }
        }
    }

    #[test]
    fn the_value_shows_in_the_accent_and_moves_with_it() {
        // every style paints the value in the accent: more of it for a
        // higher value, none of it at zero
        let t = super::super::SLATE;
        let acc = super::super::skin_or(t.acc_hot);
        let count = |style, frac| {
            shapes(
                style,
                &t,
                16.0,
                Face {
                    frac,
                    from: 0.0,
                    dim: 1.0,
                    locked: false,
                    steps: 0.0,
                },
            )
            .iter()
            .filter(|s| format!("{:?}", s.shape).contains(&format!("{:?}", acc)))
            .count()
        };
        for style in KnobStyle::ALL.into_iter().skip(1) {
            assert_eq!(count(style, 0.0), 0, "{style:?}");
            assert!(count(style, 0.9) > 0, "{style:?}");
            if !matches!(style, KnobStyle::Flat | KnobStyle::Metal) {
                assert!(count(style, 0.9) > count(style, 0.3), "{style:?}");
            }
        }
    }

    #[test]
    fn names_and_saved_numbers() {
        assert_eq!(KnobStyle::from_u8(0), KnobStyle::Classic);
        assert_eq!(KnobStyle::from_u8(5), KnobStyle::Pixel);
        assert_eq!(KnobStyle::from_u8(99), KnobStyle::Classic);
        let names: Vec<_> = KnobStyle::ALL.iter().map(|s| s.name()).collect();
        assert_eq!(
            names,
            [
                "Classic",
                "Flat ring",
                "Chicken-head",
                "LED ring",
                "Brushed metal",
                "Pixel"
            ]
        );
        for (i, s) in KnobStyle::ALL.iter().enumerate() {
            assert_eq!(*s as usize, i);
        }
    }
}
