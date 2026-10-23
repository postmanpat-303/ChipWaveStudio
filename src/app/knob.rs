//! Rotary knob widget and the small card visualisers (ADSR, filter curve,
//! waveform, sample peaks, FM algorithm) of the lower-panel instrument editor
//! (0.2.3.19). Original ChipWave code, painted with egui shapes.
use super::params::Viz;
use crate::theme::th;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke};

/// one knob cell (knob + name + value)
pub(super) const CELL: egui::Vec2 = egui::vec2(60.0, 66.0);
const RAD: f32 = 14.0;

/// the knob's centre below its cell's top (Slate: the name sits above it)
pub(super) fn centre_dy() -> f32 {
    if th().ember {
        31.0
    } else {
        4.0 + RAD
    }
}

pub(super) struct KnobOut {
    pub resp: egui::Response,
    pub changed: bool,
    pub lock_clicked: bool,
    /// a depth handle was dragged: (route, change of its amount)
    pub ring_drag: Option<(usize, f32)>,
    /// Alt-drag on the knob: change of the selected modulator's depth
    pub alt_drag: Option<f32>,
}

pub(super) struct KnobArgs<'a> {
    pub name: &'a str,
    pub value_text: String,
    /// 0..1 position of the default (double-click)
    pub default: f32,
    /// discrete steps over the range (0 = continuous)
    pub steps: f32,
    pub enabled: bool,
    /// search match: accent ring
    pub highlight: bool,
    pub badge: Option<&'a str>,
    pub locked: Option<bool>,
    pub tip: &'a str,
    /// modulation routes onto this knob (drawn as rings outside the arc)
    pub rings: &'a [super::modview::Ring],
    /// the modulated position now (a dot on the arc)
    pub live: Option<f32>,
    /// automation LED (lane colour) when this parameter has a lane
    pub auto_led: Option<Color32>,
}

/// A knob for a 0..1 position. Drag up/down (or left/right) to turn it,
/// Ctrl (or Shift) for fine steps, double-click resets it, the wheel nudges it.
pub(super) fn knob(ui: &mut egui::Ui, id: egui::Id, v: &mut f32, a: KnobArgs) -> KnobOut {
    let t = th();
    let (cell, _) = ui.allocate_exact_size(CELL, Sense::hover());
    // Ember (Slate): name above, a pic-2 knurled knob, value below
    let kc = Pos2::new(cell.center().x, cell.top() + centre_dy());
    let krect = Rect::from_center_size(kc, egui::vec2(RAD * 2.0 + 6.0, RAD * 2.0 + 6.0));
    let sense = if a.enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(krect, id, sense);
    let mut changed = false;
    let before = *v;
    let mut alt_drag = None;
    let m = ui.input(|i| i.modifiers);
    let fine = if m.ctrl || m.command || m.shift {
        0.1
    } else {
        1.0
    };
    if a.enabled && resp.dragged() && m.alt {
        let d = resp.drag_delta();
        alt_drag = Some((d.x - d.y) * 0.005 * fine);
    } else if a.enabled {
        if resp.dragged() {
            let d = resp.drag_delta();
            // keep the unrounded position while dragging (stepped knobs)
            let nid = id.with("drag_pos");
            let n0 = if resp.drag_started() {
                *v
            } else {
                ui.data(|m| m.get_temp::<f32>(nid)).unwrap_or(*v)
            };
            let n = (n0 + (d.x - d.y) * 0.005 * fine).clamp(0.0, 1.0);
            ui.data_mut(|m| m.insert_temp(nid, n));
            *v = n;
        }
        if resp.double_clicked() {
            *v = a.default;
        }
        changed = (*v - before).abs() > 1e-7;
    }
    let p = ui.painter_at(cell.expand(2.0));
    let dim: f32 = if a.enabled { 1.0 } else { 0.4 };
    if a.highlight {
        p.circle_stroke(kc, RAD + 4.0, Stroke::new(2.0, t.fx));
    }
    let a0 = std::f32::consts::PI * 0.75;
    let span = std::f32::consts::PI * 1.5;
    let pt = |ang: f32, r: f32| kc + egui::vec2(ang.cos(), ang.sin()) * r;
    let arc_r = |from: f32, to: f32, col: Color32, w: f32, r: f32| {
        let n = 28;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| pt(a0 + span * (from + (to - from) * i as f32 / n as f32), r))
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(w, col)));
    };
    let locked = a.locked == Some(true);
    // bipolar ranges (default in the middle) draw from the centre
    let from = if (a.default - 0.5).abs() < 0.01 {
        0.5
    } else {
        0.0
    };
    // the one knob painter (theme.rs): Slate's knurled knob or the classic cap
    crate::theme::knob_face(
        &p,
        kc,
        RAD,
        crate::theme::KnobFace {
            frac: *v,
            from,
            dim,
            locked,
            steps: a.steps,
        },
    );
    // modulation: a ring per route (value → value + depth) with a handle
    // that sets the depth, and the live modulated value as a dot
    let mut ring_drag = None;
    for (j, rg) in a.rings.iter().enumerate().take(3) {
        let r = RAD + if t.ember { 6.0 } else { 4.5 } + j as f32 * 3.0;
        let to = (*v + rg.amount).clamp(0.0, 1.0);
        arc_r(*v, to, rg.col, if rg.selected { 2.5 } else { 1.8 }, r);
        let hp = pt(a0 + span * to, r);
        let hr = ui.interact(
            Rect::from_center_size(hp, egui::vec2(9.0, 9.0)),
            id.with(("ring", j)),
            Sense::drag(),
        );
        let big = hr.hovered() || hr.dragged();
        p.circle_filled(hp, if big { 3.5 } else { 2.5 }, rg.col);
        if hr.dragged() {
            let d = hr.drag_delta();
            ring_drag = Some((rg.route, (d.x - d.y) * 0.005 * fine));
        }
        hr.on_hover_text(format!(
            "modulation depth {:+.0} % (drag)",
            rg.amount * 100.0
        ));
    }
    if let Some(lv) = a.live {
        if !a.rings.is_empty() {
            p.circle_filled(pt(a0 + span * lv, RAD), 2.6, t.ink);
        }
    }
    if let Some(led) = a.auto_led {
        let lp = Pos2::new(kc.x + RAD - 1.0, kc.y - RAD + 2.0);
        p.circle_filled(lp, 3.6, led);
        p.circle_stroke(lp, 3.6, Stroke::new(1.0, Color32::from_black_alpha(160)));
    }
    let (small, mono) = if t.ember {
        (crate::theme::label_font(11.0), crate::theme::num_font(10.0))
    } else {
        (
            egui::FontId::proportional(10.5),
            egui::FontId::monospace(10.0),
        )
    };
    let name = if t.ember {
        a.name.to_uppercase()
    } else {
        a.name.to_string()
    };
    let name_col = if a.enabled {
        t.ink
    } else {
        t.ink2.gamma_multiply(0.7)
    };
    let mut job = egui::text::LayoutJob::simple_singleline(name, small.clone(), name_col);
    job.wrap = egui::text::TextWrapping::truncate_at_width(CELL.x - 2.0);
    job.halign = egui::Align::Center;
    let g = ui.fonts_mut(|f| f.layout_job(job));
    let name_y = if t.ember {
        cell.top() + 1.0
    } else {
        kc.y + RAD + 3.0
    };
    p.galley(Pos2::new(cell.center().x, name_y), g, name_col);
    let mut job = egui::text::LayoutJob::simple_singleline(
        a.value_text.clone(),
        mono,
        if t.ember { t.acc_hot } else { t.value }.gamma_multiply(dim),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(CELL.x);
    job.halign = egui::Align::Center;
    let g = ui.fonts_mut(|f| f.layout_job(job));
    let val_y = if t.ember {
        kc.y + RAD + 7.0
    } else {
        kc.y + RAD + 16.0
    };
    p.galley(Pos2::new(cell.center().x, val_y), g, t.value);
    if let Some(b) = a.badge {
        p.text(
            Pos2::new(cell.left() + 2.0, cell.top() + 2.0),
            egui::Align2::LEFT_TOP,
            b,
            egui::FontId::monospace(8.5),
            t.ink2.gamma_multiply(0.9),
        );
    }
    // the Dice lock: a tiny padlock in the top-right corner
    let mut lock_clicked = false;
    if let Some(l) = a.locked {
        let lr = Rect::from_min_size(
            Pos2::new(cell.right() - 13.0, cell.top() + 1.0),
            egui::vec2(12.0, 13.0),
        );
        let lresp = ui
            .interact(lr, id.with("lock"), Sense::click())
            .on_hover_text(if l {
                "locked: Dice and Mutate keep this value (click to unlock)"
            } else {
                "click to lock: Dice and Mutate leave it alone"
            });
        lock_clicked = lresp.clicked();
        let c = if l {
            t.fx
        } else if lresp.hovered() {
            t.ink2
        } else {
            t.ink2.gamma_multiply(0.35)
        };
        let body = Rect::from_min_size(
            Pos2::new(lr.left() + 2.0, lr.top() + 6.0),
            egui::vec2(8.0, 6.0),
        );
        p.rect_filled(body, 1.0, c);
        let sh = Stroke::new(1.2, c);
        let cx = body.center().x;
        let pts: Vec<Pos2> = (0..=8)
            .map(|k| {
                let ang = std::f32::consts::PI * (1.0 + k as f32 / 8.0);
                Pos2::new(cx + 2.6 * ang.cos(), body.top() + 2.8 * ang.sin())
            })
            .collect();
        p.add(egui::Shape::line(pts, sh));
    }
    // value bubble while dragging
    if resp.dragged() {
        let lp = ui
            .ctx()
            .layer_painter(egui::LayerId::new(egui::Order::Tooltip, id.with("bub")));
        let txt = format!("{}: {}", a.name, a.value_text);
        let g = ui.fonts_mut(|f| f.layout_no_wrap(txt, egui::FontId::proportional(12.0), t.ink));
        let r = Rect::from_center_size(
            Pos2::new(kc.x, cell.top() - 12.0),
            g.size() + egui::vec2(10.0, 4.0),
        );
        lp.rect_filled(r, 4.0, t.cart_top);
        lp.rect_stroke(r, 4.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
        lp.galley(r.min + egui::vec2(5.0, 2.0), g, t.ink);
    }
    let resp = if a.tip.is_empty() {
        resp.on_hover_text(format!(
            "{}: {}\ndrag · Ctrl+drag fine · double-click default · right-click: automate / MIDI learn",
            a.name, a.value_text
        ))
    } else {
        resp.on_hover_text(format!(
            "{}: {}\n{}\ndrag · Ctrl+drag fine · double-click default · right-click: automate / MIDI learn",
            a.name, a.value_text, a.tip
        ))
    };
    let nm = a.name.to_string();
    let vt = a.value_text.clone();
    resp.widget_info(|| {
        let mut w = egui::WidgetInfo::labeled(egui::Role::Slider, a.enabled, &nm);
        w.value = Some(before as f64);
        w.current_text_value = Some(vt.clone());
        w
    });
    KnobOut {
        resp,
        changed,
        lock_clicked,
        ring_drag,
        alt_drag,
    }
}

/// draw a card picture into `r`
pub(super) fn draw_viz(p: &egui::Painter, r: Rect, v: &Viz) {
    let mut t = th();
    if let Some(s) = crate::theme::active_skin() {
        // a built-in instrument's picture: curves and marks in the tint
        t.acc_hot = s.bright;
        t.accent = s.tint;
        t.instr = s.bright;
        t.note = s.bright;
        t.fx = s.bright;
    }
    if t.ember {
        crate::ember::well(p, r, 4);
    } else {
        p.rect_filled(r, 4.0, t.crt_bot);
    }
    let line = t.instr;
    // Ember: crisp acc-hot curves over a bloom, an accent fill below
    let curve = |pts: Vec<Pos2>, fill_to: Option<f32>| {
        if t.ember {
            if let Some(b) = fill_to {
                crate::ember::fill_under(p, &pts, b, t.accent, 0.28);
            }
            crate::ember::glow_line(p, &pts, 1.8, t.acc_hot);
        } else {
            p.add(egui::Shape::line(pts, Stroke::new(1.5, line)));
        }
    };
    let grid = t.crt_grid;
    let fill = line.gamma_multiply(0.18);
    let inner = r.shrink2(egui::vec2(6.0, 5.0));
    let x = |f: f32| inner.left() + f.clamp(0.0, 1.0) * inner.width();
    let y = |f: f32| inner.bottom() - f.clamp(-0.05, 1.05) * inner.height();
    for k in 1..4 {
        let gx = inner.left() + inner.width() * k as f32 / 4.0;
        p.line_segment(
            [
                Pos2::new(gx, r.top() + 2.0),
                Pos2::new(gx, r.bottom() - 2.0),
            ],
            Stroke::new(1.0, grid),
        );
    }
    match v {
        Viz::Adsr(a, d, sus, rel) => {
            // time axis: attack / decay / hold / release share the width
            let (a, d, rel) = (a.max(0.0), d.max(0.0), rel.max(0.0));
            let hold = ((a + d + rel) * 0.35).max(0.05);
            let tot = (a + d + hold + rel).max(1e-4);
            let xs = [0.0, a / tot, (a + d) / tot, (a + d + hold) / tot, 1.0];
            let ys = [0.0, 1.0, *sus, *sus, 0.0];
            let pts: Vec<Pos2> = xs
                .iter()
                .zip(ys)
                .map(|(&a, b)| Pos2::new(x(a), y(b)))
                .collect();
            if !t.ember {
                p.add(egui::Shape::convex_polygon(
                    vec![pts[0], pts[1], Pos2::new(pts[1].x, y(0.0))],
                    fill,
                    Stroke::NONE,
                ));
            }
            curve(pts.clone(), Some(y(0.0)));
            for q in &pts[1..4] {
                if t.ember {
                    crate::ember::glow_dot(p, *q, 3.0, t.accent);
                    p.circle_filled(*q, 3.0, t.acc_hot);
                    p.circle_filled(*q, 1.4, hex_white());
                } else {
                    p.circle_filled(*q, 2.5, t.note);
                }
            }
        }
        Viz::Filter(cut, res, mode) => {
            let q = 0.6 + res.clamp(0.0, 1.0) * 9.0;
            let n = 64;
            let pts: Vec<Pos2> = (0..=n)
                .map(|k| {
                    let fx = k as f32 / n as f32;
                    let f = 20.0 * 1000f32.powf(fx); // 20 Hz .. 20 kHz
                    let w = f / cut.max(20.0);
                    let den = ((1.0 - w * w).powi(2) + (w / q).powi(2)).sqrt().max(1e-4);
                    let mag = match mode {
                        1 => w * w / den,
                        2 => (w / q) / den,
                        _ => 1.0 / den,
                    };
                    let db = 20.0 * mag.max(1e-4).log10();
                    Pos2::new(x(fx), y((db + 36.0) / 54.0))
                })
                .collect();
            curve(pts, Some(r.bottom() - 1.0));
            let cx = x((cut.max(20.0) / 20.0).log10() / 3.0);
            let mark = if t.ember { t.acc_hot } else { t.fx };
            p.line_segment(
                [Pos2::new(cx, inner.top()), Pos2::new(cx, inner.bottom())],
                Stroke::new(1.0, mark.gamma_multiply(if t.ember { 0.35 } else { 0.6 })),
            );
            let txt = if *cut >= 1000.0 {
                format!("{:.2}k", cut / 1000.0)
            } else {
                format!("{cut:.0}")
            };
            p.text(
                Pos2::new(cx + 3.0, inner.top()),
                egui::Align2::LEFT_TOP,
                txt,
                egui::FontId::monospace(9.0),
                mark,
            );
        }
        Viz::Wave(w) => {
            let n = w.len().max(2) - 1;
            // two cycles
            let pts: Vec<Pos2> = (0..=2 * n)
                .map(|k| {
                    let v = w[k % (n + 1)];
                    Pos2::new(x(k as f32 / (2 * n) as f32), y(0.5 + v * 0.45))
                })
                .collect();
            p.line_segment(
                [
                    Pos2::new(inner.left(), y(0.5)),
                    Pos2::new(inner.right(), y(0.5)),
                ],
                Stroke::new(1.0, grid),
            );
            curve(pts, None);
        }
        Viz::Peaks(pk) => {
            if pk.is_empty() {
                return;
            }
            let n = pk.len() as f32;
            for (k, (lo, hi)) in pk.iter().enumerate() {
                let fx = (k as f32 + 0.5) / n;
                p.line_segment(
                    [
                        Pos2::new(x(fx), y(0.5 + hi * 0.5)),
                        Pos2::new(x(fx), y(0.5 + lo * 0.5)),
                    ],
                    Stroke::new(1.5, if t.ember { t.acc_hot } else { line }),
                );
            }
        }
        Viz::Algo(a) => {
            let (mods, car, name) = crate::gen2::ALGOS[(*a as usize).min(7)];
            // operator boxes in a row; modulator links as arcs above, carriers underlined
            let bw = inner.width() / 4.0;
            let c =
                |o: usize| Pos2::new(inner.left() + bw * (o as f32 + 0.5), inner.center().y + 4.0);
            for (o, m) in mods.iter().enumerate() {
                for src in 0..4 {
                    if m & (1 << src) != 0 {
                        let (a, b) = (c(src), c(o));
                        let mid =
                            Pos2::new((a.x + b.x) / 2.0, inner.top() + 2.0 + (src as f32) * 2.0);
                        let pts: Vec<Pos2> = (0..=12)
                            .map(|k| {
                                let u = k as f32 / 12.0;
                                let q = 1.0 - u;
                                Pos2::new(
                                    q * q * a.x + 2.0 * q * u * mid.x + u * u * b.x,
                                    q * q * (a.y - 8.0) + 2.0 * q * u * mid.y + u * u * (b.y - 8.0),
                                )
                            })
                            .collect();
                        p.add(egui::Shape::line(pts, Stroke::new(1.2, t.fx)));
                    }
                }
            }
            for o in 0..4 {
                let rc = Rect::from_center_size(c(o), egui::vec2(18.0, 14.0));
                let carrier = car & (1 << o) != 0;
                p.rect_filled(rc, 3.0, if carrier { line } else { t.pill });
                p.text(
                    rc.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", o + 1),
                    egui::FontId::monospace(10.0),
                    if carrier { t.crt_bot } else { t.ink },
                );
            }
            p.text(
                Pos2::new(inner.right(), inner.bottom() + 2.0),
                egui::Align2::RIGHT_BOTTOM,
                name,
                egui::FontId::monospace(9.0),
                t.ink2,
            );
        }
    }
}

fn hex_white() -> Color32 {
    Color32::from_rgb(0xFF, 0xF1, 0xD6)
}
