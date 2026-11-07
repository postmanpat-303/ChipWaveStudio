//! A reusable radial (pie) menu: a translucent disc under the mouse with
//! wedges around a centre label. Hovering a wedge lights it with an accent
//! arc and, when it has children, slides out an outer ring with them.
//! Click a wedge or a child to choose it; the centre, a click outside or
//! Esc cancels. Painted on the foreground layer, so any view can open one:
//! keep a `Radial` in the view's state and call `show` every frame.
use super::*;
use std::f32::consts::{PI, TAU};

/// small vector icons for wedges (the UI font has no symbol glyphs)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RIcon {
    Play,
    Edit,
    Transpose,
    Type,
    Bulb,
    Notes,
    Generate,
    Select,
    Delete,
    Add,
    More,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RadialItem {
    pub label: String,
    pub icon: Option<RIcon>,
    /// the outer ring's labels (empty: none)
    pub children: Vec<String>,
    /// shown greyed: hovering lights nothing, picking it does nothing
    /// (the pie's owner says why on the status line)
    pub dim: bool,
    /// compass pies: fixed place, 0 at 12 o'clock, clockwise. None on the
    /// older equal-wedge pies (chords, knobs)
    pub slot: Option<u8>,
    /// compass: choosing this wedge replaces the ring instead of acting
    pub goes: bool,
    /// stable id for the action (the label is only what is drawn)
    pub key: String,
}

impl RadialItem {
    pub(crate) fn new(
        label: impl Into<String>,
        icon: Option<RIcon>,
        children: Vec<String>,
    ) -> Self {
        let label = label.into();
        Self {
            key: label.clone(),
            label,
            icon,
            children,
            dim: false,
            slot: None,
            goes: false,
        }
    }

    pub(crate) fn at(mut self, slot: u8) -> Self {
        self.slot = Some(slot);
        self
    }

    pub(crate) fn goes(mut self) -> Self {
        self.goes = true;
        if !self.label.contains('›') && !self.label.ends_with('…') {
            self.label.push_str(" ›");
        }
        self
    }

    pub(crate) fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    /// greyed (see `dim`)
    pub(crate) fn dimmed(mut self) -> Self {
        self.dim = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RadialEv {
    None,
    /// the mouse moved onto this wedge / child (fires once per change)
    Hover(usize, Option<usize>),
    /// chosen (the menu has closed)
    Pick(usize, Option<usize>),
    /// compass: this wedge replaces the ring. The pie stays open
    Deeper(usize),
    Cancel,
}

#[derive(Clone, Debug)]
pub(crate) struct Radial {
    pub at: Pos2,
    pub title: String,
    pub sub: String,
    pub items: Vec<RadialItem>,
    pub hover: Option<usize>,
    pub hover_child: Option<usize>,
    /// ignore the click that opened it
    frames: u32,
    /// accent colour for the hovered wedge (defaults to the theme's)
    pub accent: Option<Color32>,
    /// eight fixed places, gaps where a place is empty, one ring
    pub compass: bool,
}

// the radii at Settings › Look › Pie menus › Small (the old size); the
// pies draw them times piesize::k()
const R_IN0: f32 = 40.0;
const R_MID0: f32 = 112.0;
const R_OUT0: f32 = 182.0;
pub(crate) fn r_in() -> f32 {
    R_IN0 * super::piesize::k()
}
pub(crate) fn r_mid() -> f32 {
    R_MID0 * super::piesize::k()
}
pub(crate) fn r_out() -> f32 {
    R_OUT0 * super::piesize::k()
}

/// single-ring compass radii (Small). Eight labels at 11pt fit the chord
const COMPASS_IN0: f32 = 58.0;
const COMPASS_OUT0: f32 = 172.0;
pub(crate) fn compass_rin() -> f32 {
    COMPASS_IN0 * super::piesize::k()
}
pub(crate) fn compass_rout() -> f32 {
    COMPASS_OUT0 * super::piesize::k()
}
/// label size for a compass wedge: grows with the pie, never under 11pt
/// before Text size (the type scale then keeps it at or above 10pt)
pub(crate) fn compass_label_pt() -> f32 {
    (11.0 * super::piesize::k()).max(11.0)
}

impl Radial {
    pub(crate) fn new(
        at: Pos2,
        title: impl Into<String>,
        sub: impl Into<String>,
        items: Vec<RadialItem>,
    ) -> Self {
        Self {
            at,
            title: title.into(),
            sub: sub.into(),
            items,
            hover: None,
            hover_child: None,
            frames: 0,
            accent: None,
            compass: false,
        }
    }

    /// the eight-place menu. `items` carry [`RadialItem::slot`]
    pub(crate) fn compass(
        at: Pos2,
        title: impl Into<String>,
        sub: impl Into<String>,
        items: Vec<RadialItem>,
    ) -> Self {
        let mut s = Self::new(at, title, sub, items);
        s.compass = true;
        s
    }

    /// swap the ring in place (a › wedge, or Back). The click that opened
    /// the new ring is not also a choice
    pub(crate) fn replace(
        &mut self,
        title: impl Into<String>,
        sub: impl Into<String>,
        items: Vec<RadialItem>,
    ) {
        self.title = title.into();
        self.sub = sub.into();
        self.items = items;
        self.hover = None;
        self.hover_child = None;
        self.frames = 0;
        self.compass = true;
    }

    /// where a test or a pointer should press wedge `i`
    #[cfg(test)]
    pub(crate) fn hit_point(&self, i: usize) -> Pos2 {
        let r = if self.compass {
            (compass_rin() + compass_rout()) / 2.0
        } else {
            (r_in() + r_mid()) / 2.0
        };
        self.wedge_centre(i, r)
    }

    fn wedge(&self) -> f32 {
        TAU / self.items.len().max(1) as f32
    }
    /// centre angle of item `i` (0 = top, clockwise)
    fn angle_of(&self, i: usize) -> f32 {
        if self.compass {
            let s = self.items.get(i).and_then(|it| it.slot).unwrap_or(i as u8);
            -PI / 2.0 + s as f32 * TAU / 8.0
        } else {
            -PI / 2.0 + i as f32 * self.wedge()
        }
    }
    /// (from, to) angles of item `i`'s outer ring and the width of one child
    fn ring_span(&self, i: usize) -> (f32, f32, f32) {
        let n = self.items[i].children.len().max(1) as f32;
        let each = (0.34f32).max(self.wedge() / n).min(0.6);
        let span = each * n;
        let c = self.angle_of(i);
        (c - span / 2.0, c + span / 2.0, each)
    }
    /// screen point at radius `r` on wedge `i`'s centre line
    pub(crate) fn wedge_centre(&self, i: usize, r: f32) -> Pos2 {
        let a = self.angle_of(i);
        self.at + egui::vec2(a.cos(), a.sin()) * r
    }
    /// screen point of child `k` of wedge `i` in the outer ring
    pub(crate) fn child_centre(&self, i: usize, k: usize) -> Pos2 {
        let (from, _, each) = self.ring_span(i);
        let a = from + (k as f32 + 0.5) * each;
        self.at + egui::vec2(a.cos(), a.sin()) * ((r_mid() + r_out()) / 2.0)
    }
    /// keep the disc on screen
    pub(crate) fn clamp_to(&mut self, screen: Rect) {
        let r = if self.compass {
            compass_rout() + 8.0
        } else {
            r_out() + 4.0
        };
        self.at.x = self.at.x.clamp(
            screen.left() + r,
            (screen.right() - r).max(screen.left() + r),
        );
        self.at.y = self.at.y.clamp(
            screen.top() + r,
            (screen.bottom() - r).max(screen.top() + r),
        );
    }

    /// what is under `p`: (item, child)
    pub(crate) fn hit(&self, p: Pos2) -> Option<(usize, Option<usize>)> {
        if self.compass {
            return self.hit_compass(p);
        }
        let d = p - self.at;
        let r = d.length();
        if r < r_in() || r > r_out() || self.items.is_empty() {
            return None;
        }
        let a = d.y.atan2(d.x);
        let norm = |x: f32| (x + PI).rem_euclid(TAU) - PI;
        if r > r_mid() {
            // the hovered wedge's ring first
            if let Some(i) = self.hover.filter(|&i| !self.items[i].children.is_empty()) {
                let (from, _to, each) = self.ring_span(i);
                let k = (norm(a - from).rem_euclid(TAU) / each).floor() as usize;
                if k < self.items[i].children.len() {
                    return Some((i, Some(k)));
                }
            }
            return None;
        }
        let w = self.wedge();
        let k = (norm(a - (-PI / 2.0 - w / 2.0)).rem_euclid(TAU) / w).floor() as usize;
        Some((k.min(self.items.len() - 1), None))
    }

    /// which of the eight places `p` is in, if it is inside the ring
    fn compass_slot_at(&self, p: Pos2) -> Option<u8> {
        let d = p - self.at;
        let r = d.length();
        if r < compass_rin() || r > compass_rout() + 4.0 {
            return None;
        }
        let step = TAU / 8.0;
        let ang = (d.y.atan2(d.x) + PI / 2.0).rem_euclid(TAU);
        let slot = ((ang + step / 2.0).rem_euclid(TAU) / step).floor() as u8 % 8;
        Some(slot)
    }

    fn hit_compass(&self, p: Pos2) -> Option<(usize, Option<usize>)> {
        let slot = self.compass_slot_at(p)?;
        let i = self.items.iter().position(|it| it.slot == Some(slot))?;
        Some((i, None))
    }

    /// draw and handle input; call every frame while open
    pub(crate) fn show(&mut self, ctx: &egui::Context) -> RadialEv {
        self.frames += 1;
        // a see-through cover over everything: widgets below get no clicks
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("radial_cover"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                ui.allocate_response(screen.size(), Sense::click_and_drag());
            });
        let (pos, clicked, esc) = ctx.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.button_clicked(egui::PointerButton::Primary)
                    || (self.frames > 2
                        && i.pointer.button_clicked(egui::PointerButton::Secondary)),
                i.key_pressed(egui::Key::Escape),
            )
        });
        let mut ev = RadialEv::None;
        if let Some(p) = pos {
            let h = self.hit(p);
            let (hi, hc) = match h {
                Some((i, c)) => (Some(i), c),
                // in the ring gap or the centre: keep the wedge, drop the child
                None if (p - self.at).length() > r_mid()
                    && (p - self.at).length() <= r_out() + 30.0 =>
                {
                    (self.hover, None)
                }
                None => (None, None),
            };
            if hi != self.hover || hc != self.hover_child {
                self.hover = hi;
                self.hover_child = hc;
                if let Some(i) = hi {
                    ev = RadialEv::Hover(i, hc);
                }
            }
            if clicked && self.frames > 1 {
                ev = match self.hit(p) {
                    Some((i, None))
                        if self.compass
                            && self.items.get(i).is_some_and(|it| it.goes && !it.dim) =>
                    {
                        RadialEv::Deeper(i)
                    }
                    Some((i, c)) => RadialEv::Pick(i, c),
                    None => RadialEv::Cancel,
                };
            }
        }
        if esc {
            ev = RadialEv::Cancel;
        }
        self.paint(ctx);
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        ctx.request_repaint();
        ev
    }

    pub(crate) fn paint(&self, ctx: &egui::Context) {
        if self.compass {
            self.paint_compass(ctx);
            return;
        }
        let t = th();
        let p = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::unique("radial_menu"),
        ));
        let c = self.at;
        let accent = self.accent.unwrap_or(t.accent);
        let base = if t.light {
            Color32::from_rgba_unmultiplied(236, 238, 244, 236)
        } else {
            Color32::from_rgba_unmultiplied(14, 26, 48, 232)
        };
        let ink = t.ink;
        // Settings › Look › Pie menus: labels, icons and the title grow with
        // the radii
        let pk = super::piesize::k();
        // shadow + disc
        p.circle_filled(
            c + egui::vec2(0.0, 3.0),
            r_mid() + 3.0,
            Color32::from_black_alpha(70),
        );
        p.circle_filled(c, r_mid(), base);
        let w = self.wedge();
        for (i, it) in self.items.iter().enumerate() {
            let a0 = self.angle_of(i) - w / 2.0;
            let on = self.hover == Some(i) && !it.dim;
            if on {
                sector(
                    &p,
                    c,
                    r_in(),
                    r_mid(),
                    a0,
                    a0 + w,
                    accent.gamma_multiply(0.28),
                );
                arc(
                    &p,
                    c,
                    r_mid() - 2.0,
                    a0 + 0.02,
                    a0 + w - 0.02,
                    Stroke::new(4.0, accent),
                );
            }
            // separators
            let dir = egui::vec2(a0.cos(), a0.sin());
            p.line_segment(
                [c + dir * r_in(), c + dir * r_mid()],
                Stroke::new(1.0, Color32::from_white_alpha(28)),
            );
            let mid = self.angle_of(i);
            let rc = (r_in() + r_mid()) / 2.0 + 4.0;
            let at = c + egui::vec2(mid.cos(), mid.sin()) * rc;
            let col = if on {
                if t.light {
                    ink
                } else {
                    Color32::WHITE
                }
            } else if it.dim {
                ink.gamma_multiply(0.42)
            } else {
                ink
            };
            let label_y = if it.icon.is_some() { 11.0 * pk } else { 0.0 };
            if let Some(ic) = it.icon {
                // icons sit in a small round well
                p.circle_filled(
                    at - egui::vec2(0.0, 6.0 * pk),
                    11.0 * pk,
                    Color32::from_white_alpha(if on { 30 } else { 14 }),
                );
                paint_ricon(&p, ic, at - egui::vec2(0.0, 6.0 * pk), col);
            }
            p.text(
                at + egui::vec2(0.0, label_y),
                egui::Align2::CENTER_CENTER,
                &it.label,
                crate::theme::prop(pk * if it.label.len() > 9 { 9.5 } else { 10.5 }),
                col,
            );
            // the outer ring
            if on && !it.children.is_empty() {
                let (from, _to, each) = self.ring_span(i);
                for (k, ch) in it.children.iter().enumerate() {
                    let b0 = from + k as f32 * each;
                    let hot = self.hover_child == Some(k);
                    sector(
                        &p,
                        c,
                        r_mid() + 4.0,
                        r_out(),
                        b0 + 0.006,
                        b0 + each - 0.006,
                        if hot {
                            accent.gamma_multiply(0.85)
                        } else {
                            base
                        },
                    );
                    let am = b0 + each / 2.0;
                    let rr = (r_mid() + r_out()) / 2.0 + 2.0;
                    let pos = c + egui::vec2(am.cos(), am.sin()) * rr;
                    // rotate long labels along the radius? keep them level and small
                    let fs = if ch.chars().count() > 10 {
                        8.5
                    } else if ch.chars().count() > 6 {
                        9.5
                    } else {
                        10.5
                    };
                    p.text(
                        pos,
                        egui::Align2::CENTER_CENTER,
                        ch,
                        crate::theme::prop(fs * pk),
                        if hot { Color32::WHITE } else { ink },
                    );
                }
            }
        }
        // centre
        p.circle_filled(
            c,
            r_in() - 2.0,
            if t.light {
                Color32::from_gray(250)
            } else {
                Color32::from_rgb(8, 16, 32)
            },
        );
        p.circle_stroke(
            c,
            r_in() - 2.0,
            Stroke::new(1.0, Color32::from_white_alpha(40)),
        );
        p.text(
            c - egui::vec2(0.0, 6.0 * pk),
            egui::Align2::CENTER_CENTER,
            &self.title,
            crate::theme::prop(crate::theme::ts::LABEL * pk),
            ink,
        );
        p.text(
            c + egui::vec2(0.0, 8.0 * pk),
            egui::Align2::CENTER_CENTER,
            &self.sub,
            crate::theme::prop(crate::theme::ts::TINY * pk),
            t.ink2,
        );
        p.circle_stroke(c, r_mid(), Stroke::new(1.0, Color32::from_white_alpha(36)));
    }
}

impl Radial {
    fn paint_compass(&self, ctx: &egui::Context) {
        let t = th();
        let p = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::unique("radial_menu"),
        ));
        let c = self.at;
        let accent = self.accent.unwrap_or(t.accent);
        let ink = t.ink;
        let rin = compass_rin();
        let rout = compass_rout();
        let step = TAU / 8.0;
        p.circle_filled(
            c + egui::vec2(0.0, 3.0),
            rout + 2.0,
            Color32::from_black_alpha(70),
        );
        let font = crate::theme::prop(compass_label_pt());
        for it in &self.items {
            let Some(slot) = it.slot else { continue };
            let mid = -PI / 2.0 + slot as f32 * step;
            let a0 = mid - step / 2.0 + 0.02;
            let a1 = mid + step / 2.0 - 0.02;
            let on = self
                .hover
                .is_some_and(|i| self.items.get(i).is_some_and(|h| h.slot == Some(slot)))
                && !it.dim;
            let fill = if it.key == "remove"
                || it.label.starts_with("Delete")
                || it.label.starts_with("Remove effect")
            {
                Color32::from_rgb(0x3A, 0x28, 0x28)
            } else if on {
                Color32::from_rgba_unmultiplied(0x4A, 0x52, 0x5B, 235)
            } else {
                Color32::from_rgba_unmultiplied(0x2A, 0x2E, 0x33, 235)
            };
            sector(&p, c, rin, rout, a0, a1, fill);
            if on {
                arc(
                    &p,
                    c,
                    rout - 2.0,
                    a0 + 0.02,
                    a1 - 0.02,
                    Stroke::new(3.0, accent),
                );
            }
            let col = if it.dim {
                ink.gamma_multiply(0.42)
            } else if it.label.starts_with("Delete") {
                Color32::from_rgb(0xE0, 0x64, 0x5A)
            } else if on && !t.light {
                Color32::WHITE
            } else {
                ink
            };
            let at = c + egui::vec2(mid.cos(), mid.sin()) * ((rin + rout) / 2.0);
            let chord = 2.0 * ((rin + rout) / 2.0) * (step / 2.0).sin() * 0.90;
            paint_fitted(&p, at, &it.label, font.clone(), col, chord);
        }
        p.circle_filled(c, rin - 3.0, Color32::from_rgb(0x1F, 0x22, 0x26));
        p.circle_stroke(
            c,
            rin - 3.0,
            Stroke::new(1.0, Color32::from_white_alpha(40)),
        );
        let title_f = crate::theme::prop(crate::theme::ts::LABEL.max(11.0));
        let sub_f = crate::theme::prop(crate::theme::ts::SMALL);
        p.text(
            c - egui::vec2(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            &self.title,
            title_f,
            t.ink2,
        );
        p.text(
            c + egui::vec2(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            &self.sub,
            sub_f,
            ink,
        );
    }
}

/// one line, or two when the line would cross into the next wedge
fn paint_fitted(
    p: &egui::Painter,
    at: Pos2,
    label: &str,
    font: egui::FontId,
    col: Color32,
    chord: f32,
) {
    let width = |s: &str| p.layout_no_wrap(s.to_string(), font.clone(), col).size().x;
    if width(label) <= chord || !label.contains(' ') {
        p.text(at, egui::Align2::CENTER_CENTER, label, font, col);
        return;
    }
    let (a, b) = label.split_once(' ').unwrap();
    let small = font.clone();
    p.text(
        at - egui::vec2(0.0, 8.0),
        egui::Align2::CENTER_CENTER,
        a,
        small.clone(),
        col,
    );
    p.text(
        at + egui::vec2(0.0, 8.0),
        egui::Align2::CENTER_CENTER,
        b,
        small,
        col,
    );
}

/// a filled annular sector, as convex quads
pub(crate) fn sector(
    p: &egui::Painter,
    c: Pos2,
    r0: f32,
    r1: f32,
    a0: f32,
    a1: f32,
    fill: Color32,
) {
    let n = ((a1 - a0).abs() / 0.08).ceil().max(1.0) as usize;
    for k in 0..n {
        let x0 = a0 + (a1 - a0) * k as f32 / n as f32;
        let x1 = a0 + (a1 - a0) * (k + 1) as f32 / n as f32;
        let v = |a: f32, r: f32| c + egui::vec2(a.cos(), a.sin()) * r;
        p.add(egui::Shape::convex_polygon(
            vec![v(x0, r0), v(x0, r1), v(x1, r1), v(x1, r0)],
            fill,
            Stroke::NONE,
        ));
    }
}

pub(crate) fn arc(p: &egui::Painter, c: Pos2, r: f32, a0: f32, a1: f32, st: Stroke) {
    let n = ((a1 - a0).abs() / 0.05).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=n)
        .map(|k| {
            let a = a0 + (a1 - a0) * k as f32 / n as f32;
            c + egui::vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(egui::Shape::line(pts, st));
}

pub(crate) fn paint_ricon(p: &egui::Painter, ic: RIcon, c: Pos2, col: Color32) {
    // Settings › Look › Pie menus: icons grow with the pie
    let z = super::piesize::k();
    let st = Stroke::new(1.4 * z, col);
    let v = |x: f32, y: f32| egui::vec2(x * z, y * z);
    match ic {
        RIcon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![c + v(-3.0, -5.0), c + v(5.0, 0.0), c + v(-3.0, 5.0)],
                col,
                Stroke::NONE,
            ));
        }
        RIcon::Edit => {
            p.line_segment(
                [c + v(-5.0, 5.0), c + v(4.0, -4.0)],
                Stroke::new(2.2 * z, col),
            );
            p.line_segment([c + v(-5.0, 5.0), c + v(-6.0, 6.0)], st);
        }
        RIcon::Transpose => {
            p.line_segment([c + v(-3.0, -5.0), c + v(-3.0, 5.0)], st);
            p.line_segment([c + v(-3.0, -5.0), c + v(-5.5, -2.5)], st);
            p.line_segment([c + v(3.0, 5.0), c + v(3.0, -5.0)], st);
            p.line_segment([c + v(3.0, 5.0), c + v(5.5, 2.5)], st);
        }
        RIcon::Type => {
            p.circle_stroke(c + v(-2.0, 2.0), 3.5 * z, st);
            p.line_segment([c + v(1.0, -1.0), c + v(5.0, -5.0)], st);
            p.line_segment([c + v(2.0, -5.0), c + v(5.0, -5.0)], st);
            p.line_segment([c + v(5.0, -5.0), c + v(5.0, -2.0)], st);
        }
        RIcon::Bulb => {
            p.circle_stroke(c + v(0.0, -1.5), 4.0 * z, st);
            p.line_segment([c + v(-2.0, 4.5), c + v(2.0, 4.5)], st);
        }
        RIcon::Notes => {
            for k in 0..3 {
                let y = -4.0 + k as f32 * 4.0;
                p.rect_filled(
                    Rect::from_min_size(c + v(-5.0 + k as f32 * 2.0, y - 1.2), v(6.0, 2.4)),
                    1.0,
                    col,
                );
            }
        }
        RIcon::Generate => {
            p.rect_stroke(
                Rect::from_center_size(c, v(10.0, 10.0)),
                2.0,
                st,
                egui::StrokeKind::Inside,
            );
            p.line_segment([c + v(0.0, -3.0), c + v(0.0, 3.0)], st);
            p.line_segment([c + v(-3.0, 0.0), c + v(3.0, 0.0)], st);
        }
        RIcon::Select => {
            let b = Rect::from_center_size(c, v(11.0, 9.0));
            let cs = [
                b.left_top(),
                b.right_top(),
                b.right_bottom(),
                b.left_bottom(),
            ];
            for k in 0..4 {
                p.extend(egui::Shape::dashed_line(
                    &[cs[k], cs[(k + 1) % 4]],
                    st,
                    2.0,
                    1.6,
                ));
            }
        }
        RIcon::Delete => {
            p.rect_stroke(
                Rect::from_min_size(c + v(-3.5, -2.5), v(7.0, 8.0)),
                1.0,
                st,
                egui::StrokeKind::Inside,
            );
            p.line_segment([c + v(-5.0, -3.5), c + v(5.0, -3.5)], st);
            p.line_segment([c + v(-1.5, -5.5), c + v(1.5, -5.5)], st);
        }
        RIcon::Add => {
            p.line_segment(
                [c + v(0.0, -5.0), c + v(0.0, 5.0)],
                Stroke::new(1.8 * z, col),
            );
            p.line_segment(
                [c + v(-5.0, 0.0), c + v(5.0, 0.0)],
                Stroke::new(1.8 * z, col),
            );
        }
        RIcon::More => {
            for k in -1..=1 {
                p.circle_filled(c + v(k as f32 * 4.0, 0.0), 1.6 * z, col);
            }
        }
    }
}

// ------------------------------------------------------------ n-slot rings
// A ring of `n` equal slots, slot 0 centred on angle `a0`, clockwise (the
// track pie's Add rings, `trackring.rs`; the same geometry as its segments).

/// the centre angle of slot `k` of `n`
pub(crate) fn slot_angle(a0: f32, n: usize, k: usize) -> f32 {
    a0 + k as f32 * TAU / n.max(1) as f32
}

/// the slot of `n` (slot 0 centred on `a0`) that angle `a` falls in
pub(crate) fn slot_at(a0: f32, n: usize, a: f32) -> usize {
    let n = n.max(1);
    let step = TAU / n as f32;
    let x = (a - a0 + step / 2.0).rem_euclid(TAU);
    ((x / step) as usize).min(n - 1)
}

/// `p` moved in so a disc of radius `r` around it is inside `screen`
/// (centred on an axis the screen is too small for). The compass clamps
/// with its own radius; the nine-wedge pie still uses this.
#[allow(dead_code)]
pub(crate) fn clamp_disc(p: Pos2, screen: Rect, r: f32) -> Pos2 {
    let c = |v: f32, lo: f32, hi: f32| {
        if hi < lo {
            (lo + hi) / 2.0
        } else {
            v.clamp(lo, hi)
        }
    };
    Pos2::new(
        c(p.x, screen.left() + r, screen.right() - r),
        c(p.y, screen.top() + r, screen.bottom() - r),
    )
}

/// unit vectors along slot `k`'s arc (`gap` radians kept at each side,
/// `steps` pieces): the outline / mesh of an annulus slot
pub(crate) fn slot_arc(a0: f32, n: usize, k: usize, gap: f32, steps: usize) -> Vec<egui::Vec2> {
    let step = TAU / n.max(1) as f32;
    let c = slot_angle(a0, n, k);
    let (s0, s1) = (c - step / 2.0 + gap, c + step / 2.0 - gap);
    (0..=steps.max(1))
        .map(|j| {
            let a = s0 + (s1 - s0) * j as f32 / steps.max(1) as f32;
            egui::vec2(a.cos(), a.sin())
        })
        .collect()
}

/// a filled annulus strip between radii `r0` and `r1` along `pts` (unit
/// vectors, `slot_arc`), shaded from `fill_in` to `fill_out`
pub(crate) fn strip_mesh(
    c: Pos2,
    pts: &[egui::Vec2],
    r0: f32,
    r1: f32,
    fill_in: Color32,
    fill_out: Color32,
) -> egui::epaint::Mesh {
    let mut mesh = egui::epaint::Mesh::default();
    for v in pts {
        mesh.colored_vertex(c + *v * r0, fill_in);
        mesh.colored_vertex(c + *v * r1, fill_out);
    }
    for j in 0..(pts.len() as u32).saturating_sub(1) {
        let a = 2 * j;
        mesh.add_triangle(a, a + 1, a + 3);
        mesh.add_triangle(a, a + 3, a + 2);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_finds_wedges_and_ring_children() {
        let items = (0..4)
            .map(|i| {
                RadialItem::new(
                    format!("w{i}"),
                    None,
                    vec!["a".into(), "b".into(), "c".into()],
                )
            })
            .collect();
        let mut r = Radial::new(Pos2::new(300.0, 300.0), "T", "", items);
        // top wedge is item 0, right is item 1 (clockwise)
        // (at any Settings › Look › Pie menus size)
        let wedge = (r_in() + r_mid()) / 2.0;
        let ring = (r_mid() + r_out()) / 2.0;
        assert_eq!(r.hit(Pos2::new(300.0, 300.0 - wedge)), Some((0, None)));
        assert_eq!(r.hit(Pos2::new(300.0 + wedge, 300.0)), Some((1, None)));
        assert_eq!(r.hit(Pos2::new(300.0, 300.0)), None, "centre cancels");
        r.hover = Some(1);
        // the middle child of item 1 sits on its centre line, in the outer ring
        assert_eq!(r.hit(Pos2::new(300.0 + ring, 300.0)), Some((1, Some(1))));
    }

    #[test]
    fn n_slot_rings_round_trip_and_clamp() {
        for n in [1usize, 7, 9, 13] {
            let a0 = -1.1f32;
            for k in 0..n {
                let a = slot_angle(a0, n, k);
                assert_eq!(slot_at(a0, n, a), k, "n {n} k {k}");
                // just inside both edges
                let half = TAU / n as f32 / 2.0;
                assert_eq!(slot_at(a0, n, a - half + 0.01), k);
                assert_eq!(slot_at(a0, n, a + half - 0.01), k);
                assert_eq!(slot_at(a0, n, a + TAU * 3.0), k, "wraps");
            }
        }
        let sc = Rect::from_min_size(Pos2::ZERO, egui::vec2(1366.0, 768.0));
        let r = 274.0;
        for p in [
            Pos2::new(5.0, 5.0),
            Pos2::new(1360.0, 760.0),
            Pos2::new(683.0, 384.0),
        ] {
            let q = clamp_disc(p, sc, r);
            assert!(sc.shrink(r - 0.01).contains(q), "{p:?} -> {q:?}");
        }
        assert_eq!(
            clamp_disc(Pos2::new(683.0, 384.0), sc, r),
            Pos2::new(683.0, 384.0)
        );
        // too small: centred
        let tiny = Rect::from_min_size(Pos2::ZERO, egui::vec2(300.0, 200.0));
        assert_eq!(
            clamp_disc(Pos2::new(1.0, 1.0), tiny, r),
            Pos2::new(150.0, 100.0)
        );
        let pts = slot_arc(0.0, 4, 1, 0.01, 8);
        assert_eq!(pts.len(), 9);
        assert!(
            (pts[4] - egui::vec2(0.0, 1.0)).length() < 1e-4,
            "slot 1 of 4 points down"
        );
        let m = strip_mesh(Pos2::ZERO, &pts, 10.0, 20.0, Color32::RED, Color32::BLUE);
        assert_eq!(m.vertices.len(), 18);
        assert_eq!(m.indices.len(), 8 * 6);
    }
}
