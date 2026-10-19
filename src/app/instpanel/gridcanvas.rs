//! The Grid generator's patching canvas (Instrument panel › Grid tab):
//! module cards with ports and knobs, cables drawn between them.
//!
//! - right-click the background (or **+ Module**) to add a module there
//! - drag a module by its title; right-click the title to delete it
//! - drag from an output port (right) to an input port (left) to cable them;
//!   click an input port to unplug it
//! - the knobs are the instrument panel's knobs: right-click to automate or
//!   MIDI-learn, drop a modulator chip on them to modulate
use super::*;
use crate::grid::{spec, GridP, Kind, KINDS, MAX_P};
use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Stroke};

#[derive(Default)]
pub(in crate::app) struct GridUi {
    /// a cable being dragged from (module, output)
    drag: Option<(u16, u8)>,
    /// where a background right-click asked for a module (canvas units)
    add_at: Option<egui::Vec2>,
}

const W: f32 = 150.0;
const HEAD: f32 = 20.0;
const ROW: f32 = 16.0;
const MODE_H: f32 = 22.0;
const KNOB_W: f32 = 66.0;
const KNOB_H: f32 = 70.0;
const PORT_R: f32 = 5.0;

fn mod_height(k: Kind) -> f32 {
    let sp = spec(k);
    let rows = sp.ins.len().max(sp.outs.len()) as f32;
    let krows = sp.params.len().div_ceil(2) as f32;
    HEAD + if sp.modes.is_empty() { 0.0 } else { MODE_H } + rows * ROW + 4.0 + krows * KNOB_H + 4.0
}

fn cat_colour(cat: &str) -> Color32 {
    match cat {
        "In / Out" => Color32::from_rgb(0x9A, 0xA8, 0xF0),
        "Sources" => Color32::from_rgb(0x8F, 0xD9, 0xA3),
        "Filters" => Color32::from_rgb(0x6F, 0xB0, 0xE8),
        "Modulators" => Color32::from_rgb(0xE8, 0xC3, 0x5A),
        "Math" => Color32::from_rgb(0xB4, 0xBC, 0xC8),
        _ => Color32::from_rgb(0xD9, 0x8A, 0xD0),
    }
}

fn cable_path(a: Pos2, b: Pos2) -> [Pos2; 4] {
    let dx = ((b.x - a.x).abs() * 0.5).max(40.0);
    [a, pos2(a.x + dx, a.y), pos2(b.x - dx, b.y), b]
}

/// the "Add module" menu items; true = one was added
fn add_menu(ui: &mut egui::Ui, g: &mut GridP, at: egui::Vec2) -> bool {
    let mut added = false;
    for cat in ["In / Out", "Sources", "Filters", "Modulators", "Math", "FX"] {
        ui.menu_button(cat, |ui| {
            for k in KINDS {
                let sp = spec(k);
                if sp.cat != cat {
                    continue;
                }
                if ui.button(sp.name).on_hover_text(sp.tip).clicked() {
                    added |= g.add(k, at.x.max(0.0), at.y.max(0.0)).is_some();
                    ui.close();
                }
            }
        });
    }
    added
}

impl App {
    /// the Grid tab: the canvas over the whole tab area
    pub(super) fn grid_canvas(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let t = th();
        let InstrumentKind::Gen(gp) = &inst.kind else {
            return;
        };
        let crate::gen::GenEngine::Grid(orig) = &gp.engine else {
            return;
        };
        let orig = orig.clone();
        let mut g = orig.clone();
        let mut changed = false;
        // toolbar
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.menu_button("+ Module", |ui| {
                let at = vec2(
                    20.0 + (g.modules.len() % 6) as f32 * 30.0,
                    20.0 + (g.modules.len() % 6) as f32 * 24.0,
                );
                changed |= add_menu(ui, &mut g, at);
            })
            .response
            .on_hover_text("add a module (or right-click the canvas)");
            ui.label(
                egui::RichText::new(format!(
                    "{}/{} modules · {} cables · drag output → input to patch, click an input to unplug, right-click a title to delete",
                    g.modules.len(),
                    crate::grid::MAX_MODULES,
                    g.cables.len()
                ))
                .small()
                .color(t.ink2),
            );
        });
        let mut gst = std::mem::take(&mut self.ip.grid);
        let st = &mut gst;
        let ext = g.modules.iter().fold(vec2(900.0, 400.0), |e, m| {
            vec2(
                e.x.max(m.x + W + 300.0),
                e.y.max(m.y + mod_height(m.kind) + 200.0),
            )
        });
        let mut knob_slots: Vec<(usize, Rect)> = vec![];
        let mut in_ports: Vec<(u16, u8, Rect)> = vec![];
        let mut out_pos: Vec<(u16, u8, Pos2)> = vec![];
        let mut in_pos: Vec<(u16, u8, Pos2)> = vec![];
        let mut remove: Option<u16> = None;
        let mut moved: Option<(u16, egui::Vec2)> = None;
        let mut unplug: Option<(u16, u8)> = None;
        let mut modes: Vec<(u16, u8)> = vec![];
        let mut release_at: Option<Pos2> = None;
        egui::ScrollArea::both()
            .id_salt(("grid_canvas", cx.idx))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (area, bg) = ui.allocate_exact_size(ext, Sense::click());
                let o = area.min.to_vec2();
                let p = ui.painter_at(area);
                p.rect_filled(area, 4.0, t.crt_bot);
                // dot grid
                let mut y = area.min.y;
                while y < area.max.y {
                    let mut x = area.min.x;
                    while x < area.max.x {
                        p.circle_filled(pos2(x, y), 0.8, t.crt_grid);
                        x += 24.0;
                    }
                    y += 24.0;
                }
                if bg.secondary_clicked() {
                    st.add_at = bg.interact_pointer_pos().map(|q| (q - o).to_vec2());
                }
                bg.context_menu(|ui| {
                    ui.label(egui::RichText::new("Add module").strong());
                    let at = st.add_at.unwrap_or(vec2(20.0, 20.0)).to_pos2().to_vec2();
                    changed |= add_menu(ui, &mut g, at);
                });
                // modules
                for m in &g.modules {
                    let sp = spec(m.kind);
                    let r = Rect::from_min_size(pos2(m.x, m.y) + o, vec2(W, mod_height(m.kind)));
                    let col = cat_colour(sp.cat);
                    p.rect_filled(r, 5.0, t.cart_bot);
                    p.rect_stroke(
                        r,
                        5.0,
                        Stroke::new(1.0, col.gamma_multiply(0.7)),
                        egui::StrokeKind::Inside,
                    );
                    let head = Rect::from_min_size(r.min, vec2(W, HEAD));
                    p.rect_filled(head, 5.0, col.gamma_multiply(0.35));
                    p.text(
                        head.left_center() + vec2(6.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        g.label(m.id),
                        egui::FontId::proportional(12.5),
                        t.ink,
                    );
                    let hr = ui
                        .interact(
                            head,
                            ui.scope_id().with(("grid_head", cx.idx, m.id)),
                            Sense::click_and_drag(),
                        )
                        .on_hover_text(sp.tip);
                    if hr.dragged() {
                        moved = Some((m.id, hr.drag_delta()));
                    }
                    hr.context_menu(|ui| {
                        ui.label(egui::RichText::new(g.label(m.id)).strong());
                        if ui.button("Delete module").clicked() {
                            remove = Some(m.id);
                            ui.close();
                        }
                    });
                    let mut yy = head.max.y;
                    if !sp.modes.is_empty() {
                        let mr = Rect::from_min_size(
                            pos2(r.min.x + 6.0, yy + 1.0),
                            vec2(W - 12.0, MODE_H - 2.0),
                        );
                        let mut cur = m.mode;
                        ui.scope_builder(egui::UiBuilder::new().max_rect(mr), |ui| {
                            egui::ComboBox::from_id_salt(("grid_mode", cx.idx, m.id))
                                .width(W - 20.0)
                                .selected_text(sp.modes.get(cur as usize).copied().unwrap_or("?"))
                                .show_ui(ui, |ui| {
                                    for (k, name) in sp.modes.iter().enumerate() {
                                        ui.selectable_value(&mut cur, k as u8, *name);
                                    }
                                });
                        });
                        if cur != m.mode {
                            modes.push((m.id, cur));
                        }
                        yy += MODE_H;
                    }
                    // ports
                    for (j, (name, _)) in sp.ins.iter().enumerate() {
                        let c = pos2(r.min.x, yy + j as f32 * ROW + ROW * 0.5);
                        let plugged = g.cables.iter().any(|x| x.to == m.id && x.inp as usize == j);
                        p.circle_filled(c, PORT_R, if plugged { col } else { t.slate });
                        p.circle_stroke(c, PORT_R, Stroke::new(1.0, col));
                        p.text(
                            c + vec2(9.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            *name,
                            egui::FontId::proportional(11.0),
                            t.ink2,
                        );
                        let pr = Rect::from_center_size(c, vec2(16.0, 14.0));
                        let resp = ui
                            .interact(
                                pr,
                                ui.scope_id().with(("grid_in", cx.idx, m.id, j)),
                                Sense::click(),
                            )
                            .on_hover_text(if plugged {
                                "click to unplug"
                            } else {
                                "drop a cable here"
                            });
                        if resp.clicked() && plugged {
                            unplug = Some((m.id, j as u8));
                        }
                        in_ports.push((m.id, j as u8, pr));
                        in_pos.push((m.id, j as u8, c));
                    }
                    for (j, name) in sp.outs.iter().enumerate() {
                        let c = pos2(r.max.x, yy + j as f32 * ROW + ROW * 0.5);
                        p.circle_filled(c, PORT_R, col);
                        p.text(
                            c - vec2(9.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            *name,
                            egui::FontId::proportional(11.0),
                            t.ink2,
                        );
                        let pr = Rect::from_center_size(c, vec2(16.0, 14.0));
                        let resp = ui
                            .interact(
                                pr,
                                ui.scope_id().with(("grid_out", cx.idx, m.id, j)),
                                Sense::drag(),
                            )
                            .on_hover_text("drag to an input");
                        if resp.drag_started() {
                            st.drag = Some((m.id, j as u8));
                        }
                        out_pos.push((m.id, j as u8, c));
                    }
                    yy += sp.ins.len().max(sp.outs.len()) as f32 * ROW + 4.0;
                    for j in 0..sp.params.len().min(MAX_P) {
                        let kr = Rect::from_min_size(
                            pos2(
                                r.min.x + 8.0 + (j % 2) as f32 * KNOB_W,
                                yy + (j / 2) as f32 * KNOB_H,
                            ),
                            vec2(KNOB_W, KNOB_H),
                        );
                        knob_slots.push((m.slots[j] as usize, kr));
                    }
                }
                // cables
                let stroke_for = |from: u16| {
                    let k = g.module(from).map(|m| m.kind).unwrap_or(Kind::Knob);
                    Stroke::new(2.0, cat_colour(spec(k).cat).gamma_multiply(0.9))
                };
                for c in &g.cables {
                    let a = out_pos
                        .iter()
                        .find(|x| x.0 == c.from && x.1 == c.out)
                        .map(|x| x.2);
                    let b = in_pos
                        .iter()
                        .find(|x| x.0 == c.to && x.1 == c.inp)
                        .map(|x| x.2);
                    if let (Some(a), Some(b)) = (a, b) {
                        p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                            cable_path(a, b),
                            false,
                            Color32::TRANSPARENT,
                            stroke_for(c.from),
                        ));
                    }
                }
                if let Some((from, out)) = st.drag {
                    let a = out_pos
                        .iter()
                        .find(|x| x.0 == from && x.1 == out)
                        .map(|x| x.2);
                    let ptr = ui.input(|i| i.pointer.latest_pos());
                    if let (Some(a), Some(b)) = (a, ptr) {
                        p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                            cable_path(a, b),
                            false,
                            Color32::TRANSPARENT,
                            Stroke::new(2.5, t.accent),
                        ));
                        ui.ctx().request_repaint();
                    }
                    if ui.input(|i| i.pointer.any_released()) {
                        release_at = ptr;
                    }
                }
                // knobs (the panel's own knob widgets)
                for (s, kr) in &knob_slots {
                    if let Some(prm) = layout.find(&format!("gd.{s}")) {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(*kr), |ui| {
                            self.knob_ui(ui, prm, inst, cx);
                        });
                    }
                }
            });
        let st = &mut gst;
        if let Some(at) = release_at {
            if let Some((from, out)) = st.drag.take() {
                if let Some((to, inp, _)) = in_ports.iter().find(|x| x.2.expand(4.0).contains(at)) {
                    changed |= g.connect(from, out, *to, *inp);
                }
            }
        } else if st.drag.is_some() && !ui.input(|i| i.pointer.any_down()) {
            st.drag = None;
        }
        self.ip.grid = gst;
        if let Some((id, d)) = moved {
            if let Some(m) = g.modules.iter_mut().find(|m| m.id == id) {
                m.x = (m.x + d.x).max(0.0);
                m.y = (m.y + d.y).max(0.0);
                changed = true;
            }
        }
        if let Some((id, k)) = unplug {
            g.disconnect(id, k);
            changed = true;
        }
        for (id, md) in modes {
            g.set_mode(id, md);
            changed = true;
        }
        if let Some(id) = remove {
            g.remove(id);
            changed = true;
        }
        if changed {
            // structure from the canvas; knob values from the panel's knobs,
            // except slots the canvas itself set (new modules)
            if let InstrumentKind::Gen(gp) = &mut inst.kind {
                if let crate::gen::GenEngine::Grid(live) = &mut gp.engine {
                    live.modules = g.modules;
                    live.cables = g.cables;
                    for (k, v) in g.p.iter().enumerate() {
                        if orig.p.get(k) != Some(v) {
                            if let Some(x) = live.p.get_mut(k) {
                                *x = *v;
                            }
                        }
                    }
                    live.locks = g.locks & live.locks | (g.locks ^ orig.locks) & g.locks;
                }
            }
            cx.changed = true;
        }
    }
}
