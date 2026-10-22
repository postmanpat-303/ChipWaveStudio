//! Painters and small widgets of the Ember skin (the "Slate" theme): pic-2
//! bevelled panels, recessed display wells, LCD readouts, LED dots and dot
//! meters, outlined buttons and LED selectors, the pic-1 big knob (silver
//! ring, glossy centre, orange readout, segmented LED arc) and the pic-2
//! knurled knob. Glow ("bloom") is faked with wider low-alpha strokes; it
//! is only used on lit things (LED arcs, readouts, curves, meters, the
//! playhead and selected notes), never on buttons.
//!
//! Paint only: nothing here touches audio or the song.
use crate::theme::{hex, label_font, mix, num_font, polished, th};
use eframe::egui::{self, Align2, Color32, CornerRadius, Painter, Pos2, Rect, Stroke, Vec2};

/// panel top / raised / highlight
pub const BG2: Color32 = hex(0x2F3338);
pub const BG3: Color32 = hex(0x3A3E44);
pub const BG4: Color32 = hex(0x454A51);
pub const LINE: Color32 = hex(0x3B3F45);
pub const LINE2: Color32 = hex(0x6A7078);
pub const INK3: Color32 = hex(0x767C83);
pub const INK4: Color32 = hex(0x50555C);
pub const DISP0: Color32 = hex(0x141619);
pub const DISP1: Color32 = hex(0x1A1C20);
pub const GRID: Color32 = hex(0x262A2F);
pub const GRID2: Color32 = hex(0x30353B);
pub const AMBER: Color32 = hex(0xFFD36A);
pub const RED: Color32 = hex(0xFF5A3C);
pub const KNOB_WELL: Color32 = hex(0x1A100D);
pub const LED_BED: Color32 = hex(0x1E1513);
/// unselected piano-roll notes
pub const NOTE: Color32 = hex(0xCBCDD2);

/// glow on/off (flat mode turns the bloom off)
pub fn bloom() -> bool {
    polished()
}

fn alpha(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a.clamp(0.0, 1.0))
}

/// 135° start, 270° sweep, clockwise on screen
pub fn knob_angle(frac: f32) -> f32 {
    (135.0 + 270.0 * frac.clamp(0.0, 1.0)).to_radians()
}
fn at(c: Pos2, r: f32, a: f32) -> Pos2 {
    c + Vec2::new(a.cos(), a.sin()) * r
}

/// a filled disc whose colour follows `col(k)`, k = 0 at the top, 1 at
/// the bottom (vertical gradient) - or radially when `radial`
fn disc(p: &Painter, c: Pos2, r: f32, radial: bool, col: impl Fn(f32) -> Color32) {
    if r <= 0.2 {
        return;
    }
    let n = ((r * 1.6) as usize).clamp(16, 72);
    let mut m = egui::Mesh::default();
    m.colored_vertex(c, if radial { col(0.0) } else { col(0.5) });
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let q = at(c, r, a);
        let k = if radial {
            1.0
        } else {
            ((q.y - (c.y - r)) / (2.0 * r)).clamp(0.0, 1.0)
        };
        m.colored_vertex(q, col(k));
    }
    for i in 0..n as u32 {
        m.add_triangle(0, 1 + i, 1 + (i + 1) % n as u32);
    }
    p.add(egui::Shape::mesh(m));
}

/// multi-stop lerp
fn stops(s: &[(f32, Color32)], k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    for w in s.windows(2) {
        if k <= w[1].0 {
            let t = (k - w[0].0) / (w[1].0 - w[0].0).max(1e-4);
            let (a, b) = (w[0].1, w[1].1);
            let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            return Color32::from_rgba_premultiplied(
                l(a.r(), b.r()),
                l(a.g(), b.g()),
                l(a.b(), b.b()),
                l(a.a(), b.a()),
            );
        }
    }
    s.last().map(|x| x.1).unwrap_or_default()
}

/// accent ramp of the LED arc: acc-deep › acc › acc-hot
pub fn ramp(k: f32) -> Color32 {
    let t = th();
    stops(&[(0.0, t.acc_deep), (0.55, t.accent), (1.0, t.acc_hot)], k)
}

// ---------------------------------------------------------------- surfaces

/// pic-2 bevelled panel: bg2 › bg1, 1 px edge, inner top highlight and
/// bottom shadow, soft drop shadow
pub fn panel(p: &Painter, r: Rect, rad: u8) {
    let t = th();
    if polished() {
        p.add(
            egui::Shadow {
                offset: [0, 2],
                blur: 10,
                spread: 0,
                color: Color32::from_black_alpha(80),
            }
            .as_shape(r, CornerRadius::same(rad)),
        );
    }
    let mut m = egui::Mesh::default();
    crate::theme::round_mesh(&mut m, r, [rad as f32; 4], t.cart_top, t.cart_bot);
    p.add(egui::Shape::mesh(m));
    bevel(p, r, rad as f32);
}

/// the bevel lines of a panel or raised control
pub fn bevel(p: &Painter, r: Rect, rad: f32) {
    let t = th();
    let x = (r.left() + rad)..=(r.right() - rad);
    p.hline(
        x.clone(),
        r.top() + 1.5,
        Stroke::new(1.0, Color32::from_white_alpha(19)),
    );
    p.hline(
        x,
        r.bottom() - 1.5,
        Stroke::new(1.0, Color32::from_black_alpha(89)),
    );
    p.rect_stroke(
        r,
        CornerRadius::same(rad as u8),
        Stroke::new(1.0, t.edge),
        egui::StrokeKind::Inside,
    );
}

/// a section card inside a panel (instrument sections, Devices cards)
pub fn card(p: &Painter, r: Rect) {
    panel(p, r, 6);
    screws(p, r);
}

/// four 8 px corner screws with a slot at 35°
pub fn screws(p: &Painter, r: Rect) {
    if r.width() < 60.0 || r.height() < 40.0 {
        return;
    }
    let i = 7.0;
    for c in [
        Pos2::new(r.left() + i, r.top() + i),
        Pos2::new(r.right() - i, r.top() + i),
        Pos2::new(r.left() + i, r.bottom() - i),
        Pos2::new(r.right() - i, r.bottom() - i),
    ] {
        screw(p, c, 3.6);
    }
}
pub fn screw(p: &Painter, c: Pos2, r: f32) {
    p.circle_filled(
        c + Vec2::new(0.0, 0.6),
        r + 0.6,
        Color32::from_black_alpha(90),
    );
    disc(p, c, r, false, |k| {
        stops(
            &[
                (0.0, hex(0x9AA0A7)),
                (0.5, hex(0x5E646B)),
                (1.0, hex(0x2E3236)),
            ],
            k,
        )
    });
    let a = 35f32.to_radians();
    let d = Vec2::new(a.cos(), a.sin()) * (r * 0.75);
    p.line_segment([c - d, c + d], Stroke::new(1.0, hex(0x1B1D20)));
}

/// a recessed display well (waveform, filter, ADSR, EQ, spectrum, lanes)
pub fn well(p: &Painter, r: Rect, rad: u8) {
    let mut m = egui::Mesh::default();
    crate::theme::round_mesh(&mut m, r, [rad as f32; 4], DISP1, DISP0);
    p.add(egui::Shape::mesh(m));
    // inset shadow under the top edge, faint light on the bottom edge
    let x = (r.left() + rad as f32 * 0.5)..=(r.right() - rad as f32 * 0.5);
    for (i, a) in [70u8, 38, 16].iter().enumerate() {
        p.hline(
            x.clone(),
            r.top() + 1.5 + i as f32,
            Stroke::new(1.0, Color32::from_black_alpha(*a)),
        );
    }
    p.hline(
        x,
        r.bottom() + 0.5,
        Stroke::new(1.0, Color32::from_white_alpha(14)),
    );
    p.rect_stroke(
        r,
        CornerRadius::same(rad),
        Stroke::new(1.0, th().edge),
        egui::StrokeKind::Inside,
    );
}

/// LCD readout: near-black well, Geist Mono acc-hot with a glow, an
/// optional uppercase unit in ink3 before it
pub fn lcd(p: &Painter, r: Rect, unit: &str, text: &str, size: f32) {
    let t = th();
    p.rect_filled(r, 3, hex(0x0F1012));
    p.hline(
        (r.left() + 2.0)..=(r.right() - 2.0),
        r.top() + 1.5,
        Stroke::new(1.0, Color32::from_black_alpha(90)),
    );
    p.rect_stroke(r, 3, Stroke::new(1.0, t.edge), egui::StrokeKind::Inside);
    p.hline(
        (r.left() + 2.0)..=(r.right() - 2.0),
        r.bottom() + 0.5,
        Stroke::new(1.0, Color32::from_white_alpha(12)),
    );
    let mut x = r.left() + 7.0;
    if !unit.is_empty() {
        let g = p.layout_no_wrap(unit.to_uppercase(), label_font(size * 0.72), INK3);
        p.galley(
            Pos2::new(x, r.center().y - g.size().y * 0.5),
            g.clone(),
            INK3,
        );
        x += g.size().x + 6.0;
    }
    let pos = if unit.is_empty() {
        (r.center(), Align2::CENTER_CENTER)
    } else {
        (
            Pos2::new(r.right() - 7.0, r.center().y),
            Align2::RIGHT_CENTER,
        )
    };
    let _ = x;
    glow_text(p, pos.0, pos.1, text, num_font(size), t.acc_hot, true);
}

/// text with a soft glow of the same colour (lit readouts only)
pub fn glow_text(
    p: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font: egui::FontId,
    col: Color32,
    lit: bool,
) -> Rect {
    if lit && bloom() {
        let g = alpha(col, 0.16);
        for d in [
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, -1.0),
            Vec2::new(0.0, 1.0),
        ] {
            p.text(pos + d * 1.2, anchor, text, font.clone(), g);
        }
        let g2 = alpha(col, 0.06);
        for d in [
            Vec2::new(-2.0, -1.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(-1.0, 2.0),
            Vec2::new(1.0, -2.0),
        ] {
            p.text(pos + d * 1.3, anchor, text, font.clone(), g2);
        }
    }
    p.text(pos, anchor, text, font, col)
}

// ------------------------------------------------------------------- glow

/// a crisp line over a wider blurred copy (curves, playhead)
pub fn glow_line(p: &Painter, pts: &[Pos2], w: f32, col: Color32) {
    if pts.len() < 2 {
        return;
    }
    if bloom() {
        let soft = th().accent;
        for (k, a) in [(5.0f32, 0.07f32), (3.2, 0.13), (2.2, 0.24)] {
            p.add(egui::Shape::line(
                pts.to_vec(),
                Stroke::new(w * k, alpha(soft, a)),
            ));
        }
    }
    p.add(egui::Shape::line(pts.to_vec(), Stroke::new(w, col)));
}

/// the accent fill under a curve, `a0` at the curve fading to 0 at `base`
pub fn fill_under(p: &Painter, pts: &[Pos2], base: f32, col: Color32, a0: f32) {
    if pts.len() < 2 {
        return;
    }
    let mut m = egui::Mesh::default();
    let top = alpha(col, a0);
    for w in pts.windows(2) {
        let i = m.vertices.len() as u32;
        m.colored_vertex(w[0], top);
        m.colored_vertex(w[1], top);
        m.colored_vertex(Pos2::new(w[1].x, base), Color32::TRANSPARENT);
        m.colored_vertex(Pos2::new(w[0].x, base), Color32::TRANSPARENT);
        m.add_triangle(i, i + 1, i + 2);
        m.add_triangle(i, i + 2, i + 3);
    }
    p.add(egui::Shape::mesh(m));
}

/// a soft halo around a rect (selected notes, lit buttons of displays)
pub fn glow_rect(p: &Painter, r: Rect, rad: u8, col: Color32) {
    if !bloom() {
        return;
    }
    for (e, a) in [(1.0f32, 0.42f32), (2.5, 0.2), (4.5, 0.08)] {
        p.rect_stroke(
            r.expand(e),
            rad.saturating_add(e as u8),
            Stroke::new(1.6, alpha(col, a)),
            egui::StrokeKind::Outside,
        );
    }
}

/// a soft halo around a point
pub fn glow_dot(p: &Painter, c: Pos2, r: f32, col: Color32) {
    if !bloom() {
        return;
    }
    p.circle_filled(c, r * 2.2, alpha(col, 0.07));
    p.circle_filled(c, r * 1.6, alpha(col, 0.14));
}

// ------------------------------------------------------------- LEDs, meters

/// an LED dot: lit = warm white › acc-hot › acc with a glow; off = dot-off
/// with an inner shadow
pub fn led(p: &Painter, c: Pos2, r: f32, on: bool) {
    let t = th();
    if on {
        glow_dot(p, c, r, t.accent);
        disc(p, c, r, true, |k| {
            if k < 0.5 {
                hex(0xFFF1D6)
            } else {
                t.accent
            }
        });
        p.circle_filled(c, r * 0.62, t.acc_hot);
        p.circle_filled(c - Vec2::splat(r * 0.2), r * 0.3, hex(0xFFF1D6));
    } else {
        p.circle_filled(c, r, t.dot_off);
        p.circle_stroke(
            c + Vec2::new(0.0, 0.4),
            r - 0.6,
            Stroke::new(1.0, Color32::from_black_alpha(70)),
        );
    }
}

/// a lit dot in a meter colour
fn meter_dot(p: &Painter, c: Pos2, r: f32, col: Color32) {
    glow_dot(p, c, r, col);
    p.circle_filled(c, r, col);
    p.circle_filled(
        c - Vec2::splat(r * 0.22),
        r * 0.45,
        mix(col, hex(0xFFF6E6), 0.55),
    );
}

/// LED dot meter. `frac` 0..1, optional peak-hold 0..1, `warn`: the last 6
/// dots go amber / the top 2 red (levels; never for gain reduction).
/// Vertical meters fill bottom-up, horizontal ones left-to-right.
pub fn dot_meter(p: &Painter, r: Rect, frac: f32, peak: Option<f32>, vertical: bool, warn: bool) {
    let t = th();
    let (len, cross) = if vertical {
        (r.height(), r.width())
    } else {
        (r.width(), r.height())
    };
    let d = cross.clamp(2.0, 7.0);
    let gap = (d * 0.5).max(2.0);
    let n = (((len + gap) / (d + gap)).floor() as usize).max(1);
    let pitch = if n > 1 {
        (len - d) / (n - 1) as f32
    } else {
        0.0
    };
    let lit = (frac.clamp(0.0, 1.0) * n as f32).round() as usize;
    let pk = peak.map(|x| ((x.clamp(0.0, 1.0) * n as f32).round() as usize).max(1) - 1);
    for i in 0..n {
        let c = if vertical {
            Pos2::new(r.center().x, r.bottom() - d * 0.5 - i as f32 * pitch)
        } else {
            Pos2::new(r.left() + d * 0.5 + i as f32 * pitch, r.center().y)
        };
        let col = if warn && i + 2 >= n {
            RED
        } else if warn && i + 6 >= n {
            AMBER
        } else {
            t.acc_hot
        };
        if i < lit || pk == Some(i) && pk.is_some_and(|k| k > 0 || lit > 0) {
            meter_dot(p, c, d * 0.5, col);
        } else {
            p.circle_filled(c, d * 0.5, t.dot_off);
            p.circle_stroke(
                c + Vec2::new(0.0, 0.3),
                d * 0.5 - 0.5,
                Stroke::new(0.8, Color32::from_black_alpha(60)),
            );
        }
    }
}

/// dBFS (-60..+6) to a meter fraction
pub fn db_frac(db: f32) -> f32 {
    ((db + 60.0) / 66.0).clamp(0.0, 1.0)
}

// ------------------------------------------------------------------ knobs

/// pic-1 knob: bed, segmented LED arc with bloom, silver ring with a
/// notch, glossy dark well, orange readout (dim when `lit` is false).
/// `rad` is the outer radius (r 50 of the 100-unit design box).
pub fn knob_big(
    p: &Painter,
    c: Pos2,
    rad: f32,
    frac: f32,
    bipolar: bool,
    readout: &str,
    lit: bool,
) {
    let t = th();
    let s = rad / 50.0;
    let frac = frac.clamp(0.0, 1.0);
    // 1 bed
    p.circle_filled(c, 49.3 * s, Color32::from_black_alpha(140));
    p.circle_stroke(c, 43.3 * s, Stroke::new(10.0 * s, LED_BED));
    // 2-3 LED arc + bloom
    let segs = 31;
    let on = |k: f32| {
        if bipolar {
            let (a, b) = if frac < 0.5 { (frac, 0.5) } else { (0.5, frac) };
            k >= a - 0.017 && k <= b + 0.017
        } else {
            k <= frac + 0.017 && frac > 0.004
        }
    };
    let w = (3.3 * s).max(1.2);
    let seg = |k: f32, width: f32, col: Color32| {
        let a = knob_angle(k);
        p.line_segment(
            [at(c, 39.5 * s, a), at(c, 47.0 * s, a)],
            Stroke::new(width, col),
        );
    };
    if bloom() {
        for i in 0..segs {
            let k = i as f32 / (segs - 1) as f32;
            if on(k) {
                let col = ramp(k);
                seg(k, w + 5.0 * s.max(0.5), alpha(col, 0.10));
                seg(k, w + 2.5 * s.max(0.5), alpha(col, 0.20));
            }
        }
    }
    for i in 0..segs {
        let k = i as f32 / (segs - 1) as f32;
        seg(k, w, if on(k) { ramp(k) } else { t.led_off });
    }
    // 4 silver ring
    p.circle_filled(
        c + Vec2::new(0.0, 2.0 * s),
        35.0 * s,
        Color32::from_black_alpha(110),
    );
    disc(p, c, 34.0 * s, false, |k| {
        stops(
            &[
                (0.0, hex(0xF7F5F2)),
                (0.35, hex(0xD3CEC9)),
                (0.75, hex(0x8E8781)),
                (1.0, hex(0x4C4642)),
            ],
            k,
        )
    });
    let a = knob_angle(frac);
    p.line_segment(
        [at(c, 25.5 * s, a), at(c, 33.4 * s, a)],
        Stroke::new((2.4 * s).max(1.2), hex(0x2A2522)),
    );
    // 5 glossy well
    let warm = mix(KNOB_WELL, t.accent, 0.10);
    disc(p, c, 25.0 * s, false, |k| {
        stops(&[(0.0, warm), (0.6, KNOB_WELL), (1.0, hex(0x0B0706))], k)
    });
    p.circle_stroke(c, 25.0 * s, Stroke::new(1.0, hex(0x0B0706)));
    p.circle_stroke(c, 24.2 * s, Stroke::new(0.8, alpha(t.accent, 0.32)));
    // gloss: a faint arc across the top
    let gl: Vec<Pos2> = (0..=10)
        .map(|i| at(c, 19.0 * s, (205.0 + 13.0 * i as f32).to_radians()))
        .collect();
    p.add(egui::Shape::line(
        gl,
        Stroke::new(2.4 * s, Color32::from_white_alpha(16)),
    ));
    // 6 readout
    let n = readout.chars().count();
    let size = match n {
        0..=2 => 17.0,
        3 => 15.0,
        4 => 12.5,
        _ => 10.5,
    } * s;
    let col = if lit { t.acc_hot } else { INK3 };
    if size >= 5.0 {
        glow_text(
            p,
            c + Vec2::new(0.0, 0.5),
            Align2::CENTER_CENTER,
            readout,
            num_font(size),
            col,
            lit,
        );
    }
}

/// What a studio knob shows besides its position (`knob_studio`).
#[derive(Clone, Copy, Default)]
pub struct StudioKnob<'a> {
    /// 0..1 position
    pub frac: f32,
    /// the dot ring lights from the centre (pan, detune...)
    pub bipolar: bool,
    /// drawn in caps above the knob ("" = none)
    pub title: &'a str,
    /// drawn in orange under the knob ("" = none)
    pub value: &'a str,
    /// scale numbers around the knob at (text, 0..1 position); the one
    /// nearest the value is orange. Empty: `min_max` instead.
    pub scale: &'a [(&'a str, f32)],
    /// end labels at the bottom left and right ("" = none)
    pub min_max: (&'a str, &'a str),
    /// dots in the ring (0 = by size)
    pub dots: usize,
    /// 0..1 opacity (disabled knobs)
    pub dim: f32,
    /// a locked value (Dice / Mutate keep it): the ring lights in ink
    pub locked: bool,
}

/// the gap between a studio knob of radius `rad` and its dot ring
fn studio_gap(rad: f32) -> f32 {
    if rad < 12.0 {
        3.0
    } else {
        (rad * 0.2).max(4.0)
    }
}

/// the dot size of a studio knob of radius `rad`
fn studio_dot(rad: f32) -> f32 {
    (rad * 0.06).clamp(1.1, 2.2)
}

/// the largest studio knob radius whose dot ring stays inside `outer`
/// (a knob that replaces an older one keeps its footprint)
pub fn studio_rad(outer: f32) -> f32 {
    let mut r = outer;
    while r > 4.0 && r + studio_gap(r) + studio_dot(r) > outer {
        r -= 0.25;
    }
    r
}

/// The studio knob: a matte dark hardware knob with a ridged edge, one
/// thin orange pointer, a ring of small dots lit orange up to the value,
/// scale numbers (or min / max) outside, the title in caps above and the
/// value in orange below. `rad` is the knob's outer radius; the dots sit
/// at 1.2 x and the labels at about 1.55 x it. Readable from `rad` 11 up:
/// under 18 the scale and end labels are left out.
pub fn knob_studio(p: &Painter, c: Pos2, rad: f32, k: &StudioKnob) {
    let t = th();
    let mut kp = p.clone();
    if k.dim < 1.0 {
        kp.multiply_opacity(k.dim.clamp(0.3, 1.0));
    }
    let p = &kp;
    let frac = k.frac.clamp(0.0, 1.0);
    let orange = t.acc_hot;
    let small = rad < 18.0;
    // dot ring
    let n = if k.dots > 1 {
        k.dots
    } else if rad >= 24.0 {
        15
    } else if rad >= 16.0 {
        11
    } else {
        9
    };
    let dr = studio_dot(rad);
    let ring = rad + studio_gap(rad);
    let lit_col = if k.locked { t.ink2 } else { orange };
    for i in 0..n {
        let f = i as f32 / (n - 1) as f32;
        let half = 0.5 / (n - 1) as f32;
        let lit = if k.bipolar {
            let (a, b) = if frac < 0.5 { (frac, 0.5) } else { (0.5, frac) };
            f >= a - half && f <= b + half
        } else {
            frac > 0.004 && f <= frac + half
        };
        let q = at(c, ring, knob_angle(f));
        if lit {
            p.circle_filled(q, dr, lit_col);
        } else {
            p.circle_filled(q, dr, hex(0x101113));
            p.circle_stroke(q, dr, Stroke::new(0.6, Color32::from_white_alpha(10)));
        }
    }
    // drop shadow and bezel: a matte ring, lit a touch from above
    p.circle_filled(
        c + Vec2::new(0.0, rad * 0.06 + 0.5),
        rad + 0.5,
        Color32::from_black_alpha(130),
    );
    disc(p, c, rad, false, |y| {
        stops(&[(0.0, hex(0x2C2E31)), (1.0, hex(0x151618))], y)
    });
    p.circle_stroke(
        c,
        rad - 0.5,
        Stroke::new(1.0, Color32::from_black_alpha(150)),
    );
    // the cap: a ridged (gently scalloped) edge, matte, lit from above
    let cap = rad * 0.84;
    let lobes = if small { 8 } else { 10 };
    let amp = if small { 0.025 } else { 0.035 };
    let seg = ((cap * 2.4) as usize).clamp(40, 160);
    let edge = |i: usize| {
        let a = i as f32 / seg as f32 * std::f32::consts::TAU;
        let r = cap * (1.0 - amp + amp * (a * lobes as f32).cos());
        at(c, r, a)
    };
    let shade = |q: Pos2, r: f32| {
        let y = ((q.y - (c.y - r)) / (2.0 * r)).clamp(0.0, 1.0);
        stops(
            &[
                (0.0, hex(0x3B3D41)),
                (0.45, hex(0x26282B)),
                (1.0, hex(0x121315)),
            ],
            y,
        )
    };
    let mut m = egui::Mesh::default();
    m.colored_vertex(c, shade(c, cap));
    for i in 0..seg {
        let q = edge(i);
        m.colored_vertex(q, shade(q, cap));
    }
    for i in 0..seg as u32 {
        m.add_triangle(0, 1 + i, 1 + (i + 1) % seg as u32);
    }
    p.add(egui::Shape::mesh(m));
    let outline: Vec<Pos2> = (0..seg).map(edge).collect();
    p.add(egui::Shape::closed_line(
        outline,
        Stroke::new(1.0, Color32::from_black_alpha(170)),
    ));
    // a soft rim light along the top of the cap
    let rim: Vec<Pos2> = (0..=16)
        .map(|i| at(c, cap - 1.2, (215.0 + 110.0 * i as f32 / 16.0).to_radians()))
        .collect();
    p.add(egui::Shape::line(
        rim,
        Stroke::new(1.0, Color32::from_white_alpha(22)),
    ));
    // the face: a shallow dish, darker at the top
    let face = rad * 0.58;
    disc(p, c, face, false, |y| {
        stops(&[(0.0, hex(0x17181A)), (1.0, hex(0x2A2C2F))], y)
    });
    p.circle_stroke(c, face, Stroke::new(1.0, Color32::from_black_alpha(120)));
    // the pointer: one thin orange line across the cap's rim
    let a = knob_angle(frac);
    p.line_segment(
        [at(c, face + rad * 0.04, a), at(c, cap - rad * 0.03, a)],
        Stroke::new((rad * 0.075).clamp(1.3, 2.4), lit_col),
    );
    // labels outside: scale numbers, or min / max at the bottom
    let lf = label_font((rad * 0.42).clamp(10.0, 13.0));
    if !small && !k.scale.is_empty() {
        let best = k
            .scale
            .iter()
            .enumerate()
            .min_by(|a, b| {
                (a.1 .1 - frac)
                    .abs()
                    .partial_cmp(&(b.1 .1 - frac).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|x| x.0);
        for (i, (txt, f)) in k.scale.iter().enumerate() {
            let q = at(c, ring + 9.0, knob_angle(*f));
            let col = if Some(i) == best { orange } else { t.ink2 };
            let al = match q.x - c.x {
                d if d < -rad * 0.3 => Align2::RIGHT_CENTER,
                d if d > rad * 0.3 => Align2::LEFT_CENTER,
                _ => Align2::CENTER_CENTER,
            };
            p.text(
                q + Vec2::new((q.x - c.x).signum() * -3.0, 0.0),
                al,
                *txt,
                lf.clone(),
                col,
            );
        }
    }
    // min / max under the two ends of the dot ring
    let ends = !small && k.scale.is_empty() && (k.min_max.0, k.min_max.1) != ("", "");
    let ends_y = c.y + ring * 0.71 + dr + 2.0;
    if ends {
        let (lo, hi) = k.min_max;
        let dx = ring * 0.71;
        p.text(
            Pos2::new(c.x - dx, ends_y),
            Align2::CENTER_TOP,
            lo,
            lf.clone(),
            t.ink2,
        );
        p.text(
            Pos2::new(c.x + dx, ends_y),
            Align2::CENTER_TOP,
            hi,
            lf.clone(),
            t.ink2,
        );
    }
    // title above, value below in orange
    let top = if !small && !k.scale.is_empty() {
        ring + 16.0
    } else {
        ring + 2.0
    };
    if !k.title.is_empty() {
        p.text(
            Pos2::new(c.x, c.y - top),
            Align2::CENTER_BOTTOM,
            k.title.to_uppercase(),
            label_font(if small { 11.0 } else { 12.5 }),
            t.ink,
        );
    }
    if !k.value.is_empty() {
        // between the ends of the ring, or under the min / max labels
        let y = if ends {
            ends_y + lf.size + 2.0
        } else {
            c.y + ring - dr
        };
        p.text(
            Pos2::new(c.x, y),
            Align2::CENTER_TOP,
            k.value,
            num_font(if small { 10.5 } else { 11.5 }),
            orange,
        );
    }
}

/// A studio knob you can turn: drag up / right to raise it (Ctrl or Shift
/// for fine steps), double-click for `def`. `norm` is 0..1.
pub fn knob_studio_ui(
    ui: &mut egui::Ui,
    c: Pos2,
    rad: f32,
    id: egui::Id,
    norm: &mut f32,
    def: f32,
    look: StudioKnob,
) -> egui::Response {
    let hit = egui::Rect::from_center_size(c, Vec2::splat(rad * 2.0 + 8.0));
    let resp = ui.interact(hit, id, egui::Sense::click_and_drag());
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.ctrl || i.modifiers.shift) {
            0.1
        } else {
            1.0
        };
        let d = resp.drag_delta();
        *norm = (*norm + (d.x - d.y) * 0.004 * fine).clamp(0.0, 1.0);
    }
    if resp.double_clicked() {
        *norm = def;
    }
    knob_studio(
        ui.painter(),
        c,
        rad,
        &StudioKnob {
            frac: *norm,
            ..look
        },
    );
    let (name, x) = (look.title.to_string(), *norm);
    resp.widget_info(|| {
        let mut w = egui::WidgetInfo::labeled(egui::Role::Slider, true, &name);
        w.value = Some(x as f64);
        w
    });
    resp
}

/// the pic-1 fader: dark slot, accent fill below the silver cap, a dB scale
/// with 0 in acc-hot. `frac` = cap position 0..1 (bottom..top).
pub fn fader(p: &Painter, slot: Rect, frac: f32, scale: &[(&str, f32)], hovered: bool) {
    let t = th();
    let frac = frac.clamp(0.0, 1.0);
    // scale left of the slot
    for (txt, k) in scale {
        let y = slot.bottom() - k * slot.height();
        let zero = *txt == "0";
        let col = if zero { t.acc_hot } else { INK3 };
        p.hline(
            (slot.left() - 6.0)..=(slot.left() - 2.0),
            y,
            Stroke::new(1.0, if zero { t.accent } else { INK4 }),
        );
        glow_text(
            p,
            Pos2::new(slot.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            txt,
            label_font(10.0),
            col,
            zero,
        );
    }
    p.rect_filled(slot, 2, hex(0x0E0F11));
    p.rect_stroke(slot, 2, Stroke::new(1.0, t.edge), egui::StrokeKind::Outside);
    let y = slot.bottom() - frac * slot.height();
    let fill = Rect::from_min_max(
        Pos2::new(slot.left() + 1.0, y),
        slot.right_bottom() - Vec2::new(1.0, 1.0),
    );
    if fill.height() > 0.5 {
        if bloom() {
            p.rect_filled(fill.expand2(Vec2::new(2.5, 0.0)), 2, alpha(t.accent, 0.16));
        }
        p.rect_filled(fill, 1, t.accent);
    }
    // silver cap
    let cap = Rect::from_center_size(Pos2::new(slot.center().x, y), Vec2::new(22.0, 13.0));
    p.rect_filled(
        cap.translate(Vec2::new(0.0, 1.5)),
        2,
        Color32::from_black_alpha(120),
    );
    let mut m = egui::Mesh::default();
    crate::theme::round_mesh(
        &mut m,
        cap,
        [2.0; 4],
        if hovered {
            hex(0xFFFFFF)
        } else {
            hex(0xF2F0ED)
        },
        hex(0x8E8781),
    );
    p.add(egui::Shape::mesh(m));
    p.rect_stroke(
        cap,
        2,
        Stroke::new(1.0, hex(0x2A2522)),
        egui::StrokeKind::Inside,
    );
    p.hline(
        (cap.left() + 3.0)..=(cap.right() - 3.0),
        cap.center().y,
        Stroke::new(1.5, t.acc_deep),
    );
}

// ------------------------------------------------------------------ widgets

/// pic-2 outlined button: bg3 › bg2, 1 px line2 outline, uppercase
/// condensed text; selected = acc-hot text + an LED dot, lighter outline,
/// no fill and no glow.
pub fn button(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response {
    button_sized(ui, text, selected, 0.0)
}

pub fn button_sized(ui: &mut egui::Ui, text: &str, selected: bool, min_w: f32) -> egui::Response {
    button_ex(ui, text, selected, false, Vec2::new(min_w, 0.0))
}

/// the outlined button with a minimum size; `primary` (the default action of
/// a dialog) is lit: an orange outline with a faint bloom and orange text
pub fn button_ex(
    ui: &mut egui::Ui,
    text: &str,
    selected: bool,
    primary: bool,
    min: Vec2,
) -> egui::Response {
    let t = th();
    let font = label_font(12.5);
    let txt = text.to_uppercase();
    let g = ui
        .painter()
        .layout_no_wrap(txt.clone(), font.clone(), t.ink);
    let dot = if selected { 10.0 } else { 0.0 };
    let w = (g.size().x + dot + 16.0).max(min.x);
    let h = ui.spacing().interact_size.y.max(20.0).max(min.y);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::click());
    let label = text.to_string();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, &label)
    });
    if ui.is_rect_visible(r) {
        paint_button(
            ui.painter(),
            r,
            &resp,
            text,
            selected,
            primary,
            ui.is_enabled(),
        );
    }
    resp
}

/// paint the outlined button into `r` (shared by `button_ex` and the
/// existing toolbar / piano-roll buttons in Slate): bevelled face, 1 px
/// outline, capital label; `selected` adds the LED dot and orange text,
/// `primary` the lit orange outline
pub fn paint_button(
    p: &Painter,
    r: Rect,
    resp: &egui::Response,
    text: &str,
    selected: bool,
    primary: bool,
    enabled: bool,
) {
    let t = th();
    let font = label_font(if r.height() < 20.0 { 11.5 } else { 12.5 });
    let txt = text.to_uppercase();
    let g = p.layout_no_wrap(txt.clone(), font.clone(), t.ink);
    let dot = if selected { 10.0 } else { 0.0 };
    let pressed = resp.is_pointer_button_down_on();
    let (top, bot) = if pressed {
        (BG2, BG3)
    } else if resp.hovered() {
        (BG4, BG3)
    } else {
        (BG3, BG2)
    };
    let mut m = egui::Mesh::default();
    crate::theme::round_mesh(&mut m, r, [4.0; 4], top, bot);
    p.add(egui::Shape::mesh(m));
    if !pressed {
        p.hline(
            (r.left() + 3.0)..=(r.right() - 3.0),
            r.top() + 1.5,
            Stroke::new(1.0, Color32::from_white_alpha(16)),
        );
    }
    let outline = if primary {
        if bloom() {
            p.rect_stroke(
                r.expand(1.5),
                5,
                Stroke::new(3.0, alpha(t.accent, 0.18)),
                egui::StrokeKind::Inside,
            );
        }
        if resp.hovered() {
            t.acc_hot
        } else {
            t.accent
        }
    } else if selected || resp.hovered() {
        hex(0x8A8F96)
    } else {
        LINE2
    };
    p.rect_stroke(r, 4, Stroke::new(1.0, outline), egui::StrokeKind::Inside);
    let col = if !enabled {
        INK3
    } else if selected || primary {
        t.acc_hot
    } else {
        t.ink
    };
    let tw = g.size().x + dot;
    let x0 = r.center().x - tw * 0.5;
    if selected {
        led(p, Pos2::new(x0 + 3.0, r.center().y), 3.0, true);
    }
    p.text(
        Pos2::new(x0 + dot, r.center().y),
        Align2::LEFT_CENTER,
        txt,
        font,
        col,
    );
    if resp.has_focus() {
        p.rect_stroke(
            r.expand(2.0),
            5,
            Stroke::new(1.5, t.focus_ring),
            egui::StrokeKind::Outside,
        );
    }
}

/// one LED + label (a toggle or one option of an LED selector)
pub fn led_item(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let t = th();
    let font = label_font(12.5);
    let col = if on { t.acc_hot } else { INK3 };
    let g = ui.painter().layout_no_wrap(text.to_string(), font, col);
    let (r, resp) = ui.allocate_exact_size(
        Vec2::new(g.size().x + 12.0, g.size().y.max(16.0)),
        egui::Sense::click(),
    );
    let label = text.to_string();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::RadioButton, ui.is_enabled(), on, &label)
    });
    if ui.is_rect_visible(r) {
        let p = ui.painter();
        led(p, Pos2::new(r.left() + 3.5, r.center().y), 3.5, on);
        let col = if resp.hovered() && !on { t.ink2 } else { col };
        p.galley_with_override_text_color(
            Pos2::new(r.left() + 12.0, r.center().y - g.size().y * 0.5),
            g,
            col,
        );
    }
    resp
}

/// pic-1 tab underline: acc-deep › acc-hot › acc-deep with a glow, plus a
/// faint accent wash above it
pub fn tab_underline(p: &Painter, r: Rect, h: f32) {
    let t = th();
    let line = Rect::from_min_max(Pos2::new(r.left(), r.bottom() - h), r.right_bottom());
    p.rect_filled(
        Rect::from_min_max(r.left_top(), line.left_top() + Vec2::new(line.width(), 0.0)),
        2,
        alpha(t.accent, 0.08),
    );
    if bloom() {
        p.rect_filled(line.expand2(Vec2::new(1.0, 2.5)), 3, alpha(t.accent, 0.14));
    }
    let mut m = egui::Mesh::default();
    let (l, mid, rr) = (line.left(), line.center().x, line.right());
    for (x0, x1, c0, c1) in [
        (l, mid, t.acc_deep, t.acc_hot),
        (mid, rr, t.acc_hot, t.acc_deep),
    ] {
        let i = m.vertices.len() as u32;
        m.colored_vertex(Pos2::new(x0, line.top()), c0);
        m.colored_vertex(Pos2::new(x1, line.top()), c1);
        m.colored_vertex(Pos2::new(x1, line.bottom()), c1);
        m.colored_vertex(Pos2::new(x0, line.bottom()), c0);
        m.add_triangle(i, i + 1, i + 2);
        m.add_triangle(i, i + 2, i + 3);
    }
    p.add(egui::Shape::mesh(m));
}

/// uppercase condensed section title in ink2
pub fn title(p: &Painter, pos: Pos2, text: &str) -> Rect {
    p.text(
        pos,
        Align2::LEFT_CENTER,
        text.to_uppercase(),
        label_font(13.0),
        th().ink,
    )
}

/// uppercase condensed knob label in ink2
pub fn knob_label(p: &Painter, pos: Pos2, text: &str, size: f32) -> Rect {
    p.text(
        pos,
        Align2::CENTER_CENTER,
        text.to_uppercase(),
        label_font(size),
        th().ink2,
    )
}

/// a pic-1 tab: ink3 text, the active one in ink with the glowing
/// underline. Classic themes: a selectable label.
pub fn tab(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let t = th();
    if !t.ember {
        return ui.selectable_label(selected, egui::RichText::new(text).strong());
    }
    let font = egui::FontId::proportional(13.0);
    let g = ui.painter().layout_no_wrap(text.to_string(), font, t.ink);
    let (r, resp) = ui.allocate_exact_size(
        Vec2::new(g.size().x + 20.0, ui.available_height().clamp(20.0, 26.0)),
        egui::Sense::click(),
    );
    let label = text.to_string();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, &label));
    if ui.is_rect_visible(r) {
        let p = ui.painter();
        if selected {
            tab_underline(p, r, 3.0);
        }
        let col = if selected {
            t.ink
        } else if resp.hovered() {
            t.ink2
        } else {
            INK3
        };
        p.galley_with_override_text_color(
            r.center() - g.size() * 0.5 - Vec2::new(0.0, 1.0),
            g,
            col,
        );
        if resp.has_focus() {
            p.rect_stroke(
                r,
                3,
                Stroke::new(1.5, t.focus_ring),
                egui::StrokeKind::Inside,
            );
        }
    }
    resp
}

/// a heading shown in capitals (the condensed label font) whose accessible
/// name keeps the normal spelling, so screen readers say "Master", not
/// "M A S T E R", and the UI tests find it by its usual name
pub fn caps_label(ui: &mut egui::Ui, text: &str, size: f32) -> egui::Response {
    let r = ui.label(
        egui::RichText::new(text.to_uppercase())
            .font(label_font(size))
            .color(th().ink),
    );
    let name = text.to_string();
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
    r
}

/// an on/off toolbar toggle: Slate draws the outlined button with its LED
/// dot when on; the classic themes keep the selectable label (`classic`)
pub fn toggle(ui: &mut egui::Ui, on: bool, classic: egui::RichText, text: &str) -> egui::Response {
    if th().ember {
        button(ui, text, on)
    } else {
        ui.selectable_label(on, classic)
    }
}
