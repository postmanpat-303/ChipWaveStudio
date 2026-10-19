//! FX view: the selected instrument's insert chain as horizontal device
//! panels, each with a vertical side strip (see docs/DESIGN_LANGUAGE.md).
use super::*;
use crate::fx::{self, Device, FxKind};

const STRIP_W: f32 = 26.0;
const BODY_W: f32 = 200.0;

pub(super) enum FxOp {
    Add(FxKind, Option<usize>),
    AddPlugin(crate::plugin::PluginPatch),
    Remove(usize),
    Dup(usize),
    Toggle(usize),
    Collapse(usize),
    Move(usize, usize),
    Preset(usize, usize),
    /// device, param, normalised value, starts a new undo step
    Param(usize, usize, f32, bool),
    /// a Grid FX device's new patch, starts a new undo step
    Grid(usize, Box<crate::grid::GridP>, bool),
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// strip colour: muted type tint, grey when bypassed
pub(super) fn strip_fill(kind: FxKind, on: bool) -> Color32 {
    let t = th();
    if !on {
        return if t.light {
            Color32::from_gray(176)
        } else {
            Color32::from_gray(64)
        };
    }
    lerp(
        t.cart_top,
        hex(kind.tint()),
        if t.light { 0.45 } else { 0.7 },
    )
}

fn strip_ink(on: bool) -> Color32 {
    let t = th();
    match (t.light, on) {
        (true, _) => t.ink,
        (false, true) => Color32::from_gray(232),
        (false, false) => Color32::from_gray(150),
    }
}

pub(super) fn apply_preset(d: &mut Device, p: usize) {
    if let Some((_, vals)) = fx::presets(d.kind).get(p) {
        for (k, v) in vals.iter() {
            d.set_real(k, *v);
        }
    }
}

impl App {
    pub(super) fn fx_view(&mut self, ui: &mut egui::Ui) {
        let idx = (self.instr as usize).wrapping_sub(1);
        let (names, chain) = self.with(|s| {
            (
                s.song
                    .instruments
                    .iter()
                    .map(|i| i.name.clone())
                    .collect::<Vec<_>>(),
                s.song.instruments.get(idx).map(|i| i.fx.clone()),
            )
        });
        let mut ops: Vec<FxOp> = vec![];
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("FX chain").strong());
            let cur = names.get(idx).cloned().unwrap_or_else(|| "(no instrument)".into());
            egui::ComboBox::from_id_salt("fx_inst")
                .selected_text(format!("{:02X} {cur}", self.instr))
                .show_ui(ui, |ui| {
                    for (k, n) in names.iter().enumerate() {
                        if ui.selectable_label(k == idx, format!("{:02X} {n}", k + 1)).clicked() {
                            self.instr = (k + 1) as u8;
                        }
                    }
                });
            if chain.is_some() {
                ui.menu_button("+ Device", |ui| {
                    for k in FxKind::ALL {
                        ui.menu_button(k.name(), |ui| {
                            if ui.button("Default").clicked() {
                                ops.push(FxOp::Add(k, None));
                                ui.close();
                            }
                            for (p, (pn, _)) in fx::presets(k).iter().enumerate() {
                                if ui.button(*pn).clicked() {
                                    ops.push(FxOp::Add(k, Some(p)));
                                    ui.close();
                                }
                            }
                        });
                    }
                    ui.separator();
                    if let Some(pp) = self.plugin_fx_menu(ui) {
                        ops.push(FxOp::AddPlugin(pp));
                    }
                });
            }
            ui.label(
                egui::RichText::new(
                    "Up/Down = instrument · click name = collapse · drag name = reorder · power = bypass · right-click = menu",
                )
                .small()
                .color(th().ink2),
            );
        });
        if chain.is_some() {
            // modulators of the instrument: drag one onto a slider
            let (mut m, live, has_patch) = self.with(|s| {
                (
                    s.song.modulation.clone(),
                    s.engine.mods.out,
                    s.song
                        .instruments
                        .get(idx)
                        .is_some_and(|i| i.kind.patch().is_some()),
                )
            });
            let before = m.clone();
            let mut out = super::modview::ModOut::default();
            super::modview::strip(
                ui,
                &mut m,
                idx,
                has_patch,
                &live,
                &mut self.ip.mod_sel,
                &mut out,
            );
            if m != before {
                self.with(|s| s.song.modulation = m);
                self.dirty = true;
            }
            self.mod_out(out);
        }
        ui.add_space(6.0);
        let Some(chain) = chain else {
            ui.label("Select an instrument to edit its FX chain.");
            return;
        };
        let h = (ui.available_height() - 12.0).clamp(160.0, 360.0);
        let mut rects: Vec<Rect> = vec![];
        let pointer = ui.ctx().pointer_latest_pos();
        let released = ui.input(|i| i.pointer.any_released());
        egui::ScrollArea::horizontal()
            .id_salt("fx_scroll")
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    io_tag(ui, "IN", h);
                    for (i, d) in chain.iter().enumerate() {
                        rects.push(self.device_panel(ui, i, d, h, chain.len(), &mut ops));
                    }
                    if chain.is_empty() {
                        ui.add_sized(
                            [220.0, h],
                            egui::Label::new(
                                egui::RichText::new("empty chain\n+ Device adds one")
                                    .color(th().ink2),
                            ),
                        );
                    }
                    io_tag(ui, "OUT", h);
                });
                // drag to reorder: insertion marker + drop
                if let (Some(from), Some(p)) = (self.fx_drag, pointer) {
                    let to = rects
                        .iter()
                        .position(|r| p.x < r.center().x)
                        .unwrap_or(rects.len());
                    let x = if to < rects.len() {
                        rects[to].left() - 4.0
                    } else {
                        rects.last().map(|r| r.right() + 4.0).unwrap_or(0.0)
                    };
                    if let Some(r) = rects.first() {
                        ui.painter()
                            .vline(x, r.y_range(), Stroke::new(3.0, th().accent));
                    }
                    if released {
                        self.fx_drag = None;
                        if to != from && to != from + 1 {
                            ops.push(FxOp::Move(from, to));
                        }
                    }
                } else if released {
                    self.fx_drag = None;
                }
            });
        self.grid_fx_window(ui.ctx(), idx, &chain, &mut ops);
        self.apply_fx_ops(idx, ops);
    }

    /// the patch editor of a Grid FX device (a window over the FX view)
    fn grid_fx_window(
        &mut self,
        ctx: &egui::Context,
        idx: usize,
        chain: &[Device],
        ops: &mut Vec<FxOp>,
    ) {
        let Some((o, di)) = self.ip.grid_fx else {
            return;
        };
        let Some(mut g) = chain
            .get(di)
            .filter(|d| o == idx && d.kind == FxKind::Grid)
            .and_then(|d| d.grid.clone())
        else {
            self.ip.grid_fx = None;
            return;
        };
        let mut open = true;
        let mut st = std::mem::take(&mut self.ip.grid_fx_ui);
        let mut changed = false;
        let patches = crate::grid::fx_presets();
        egui::Window::new(format!("Grid FX · device {}", di + 1))
            .open(&mut open)
            .default_size([980.0, 560.0])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "Audio In carries the signal into the patch; Note follows the channel's note. \
                         The device's Macro 1-8 and Mix are automatable and drag-to-modulate targets.",
                    )
                    .small()
                    .color(th().ink2),
                );
                changed = super::instpanel::gridcanvas::grid_editor(
                    ui,
                    &mut g,
                    &mut st,
                    0x6F00 + di,
                    &patches,
                    &mut |ui, s, def, dflt, v| {
                        let text = def.show(*v);
                        super::knob::knob(
                            ui,
                            ui.make_persistent_id(("gfx_knob", di, s)),
                            v,
                            super::knob::KnobArgs {
                                name: def.name,
                                value_text: text,
                                default: dflt,
                                steps: 0.0,
                                enabled: true,
                                highlight: false,
                                badge: None,
                                locked: None,
                                tip: def.tip,
                                rings: &[],
                                live: None,
                            },
                        );
                    },
                );
            });
        self.ip.grid_fx_ui = st;
        let held = ctx.input(|i| i.pointer.any_down());
        if changed {
            ops.push(FxOp::Grid(di, g, !self.ip.grid_fx_held));
            self.ip.grid_fx_held = held;
        } else if !held {
            self.ip.grid_fx_held = false;
        }
        if !open {
            self.ip.grid_fx = None;
        }
    }

    pub(super) fn apply_fx_ops(&mut self, idx: usize, ops: Vec<FxOp>) {
        for op in ops {
            if let FxOp::AddPlugin(p) = op {
                self.add_plugin_fx(idx as u8 + 1, p);
                continue;
            }
            if matches!(op, FxOp::Dup(_)) {
                // the copy starts from the plugin's current settings
                self.plug.state_dirty = true;
                self.capture_plugins();
            }
            if !matches!(op, FxOp::Param(_, _, _, false) | FxOp::Grid(_, _, false)) {
                self.snapshot();
            }
            let msg = self.with(|s| {
                let Some(inst) = s.song.instruments.get_mut(idx) else {
                    return String::new();
                };
                let c = &mut inst.fx;
                match op {
                    FxOp::Add(k, p) if c.len() < 16 => {
                        let mut d = Device::new(k);
                        if let Some(p) = p {
                            apply_preset(&mut d, p);
                        }
                        c.push(d);
                        format!("Added {}", k.name())
                    }
                    FxOp::Add(..) => "Chain is full (16 devices)".into(),
                    FxOp::Remove(i) if i < c.len() => {
                        format!("Removed {}", c.remove(i).title())
                    }
                    FxOp::Dup(i) if i < c.len() && c.len() < 16 => {
                        let mut d = c[i].clone();
                        d.renew_uid();
                        c.insert(i + 1, d);
                        "Duplicated device".into()
                    }
                    FxOp::Toggle(i) if i < c.len() => {
                        c[i].enabled = !c[i].enabled;
                        format!(
                            "{} {}",
                            c[i].kind.name(),
                            if c[i].enabled { "on" } else { "bypassed" }
                        )
                    }
                    FxOp::Collapse(i) if i < c.len() => {
                        c[i].collapsed = !c[i].collapsed;
                        String::new()
                    }
                    FxOp::Move(from, to) if from < c.len() => {
                        let d = c.remove(from);
                        let to = if to > from { to - 1 } else { to }.min(c.len());
                        c.insert(to, d);
                        "Moved device".into()
                    }
                    FxOp::Preset(i, p) if i < c.len() => {
                        apply_preset(&mut c[i], p);
                        format!("Preset: {}", fx::presets(c[i].kind)[p].0)
                    }
                    FxOp::Grid(i, g, _) => {
                        if let Some(d) = c.get_mut(i).filter(|d| d.kind == FxKind::Grid) {
                            d.grid = Some(g);
                        }
                        String::new()
                    }
                    FxOp::Param(i, p, v, _) => {
                        if let Some(x) = c.get_mut(i).and_then(|d| d.params.get_mut(p)) {
                            *x = v.clamp(0.0, 1.0);
                        }
                        String::new()
                    }
                    _ => String::new(),
                }
            });
            if !msg.is_empty() {
                self.status = msg;
            }
            self.dirty = true;
        }
    }

    /// One device: side strip + (unless collapsed) the parameter body.
    fn device_panel(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        d: &Device,
        h: f32,
        n: usize,
        ops: &mut Vec<FxOp>,
    ) -> Rect {
        let w = if d.collapsed {
            STRIP_W
        } else {
            STRIP_W + BODY_W
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
        let t = th();
        let p = ui.painter_at(rect.expand(2.0));
        let strip = Rect::from_min_size(rect.min, egui::vec2(STRIP_W, h));
        let fill = strip_fill(d.kind, d.enabled);
        let ink = strip_ink(d.enabled);
        let body = Rect::from_min_max(Pos2::new(strip.right(), rect.top()), rect.max);
        if !d.collapsed {
            p.rect_filled(
                body,
                egui::CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 8,
                    se: 8,
                },
                t.cart_top,
            );
        }
        p.rect_filled(
            strip,
            if d.collapsed {
                egui::CornerRadius::same(8)
            } else {
                egui::CornerRadius {
                    nw: 8,
                    sw: 8,
                    ne: 0,
                    se: 0,
                }
            },
            fill,
        );
        p.rect_stroke(
            rect,
            8,
            Stroke::new(1.0, t.ink2.gamma_multiply(0.45)),
            egui::StrokeKind::Inside,
        );
        // --- strip icons, top to bottom: power, preset folder, collapse
        let icon = |k: usize| {
            Rect::from_center_size(
                Pos2::new(strip.center().x, strip.top() + 14.0 + k as f32 * 24.0),
                egui::vec2(20.0, 20.0),
            )
        };
        let id = ui.make_persistent_id(("fxdev", i));
        // power
        let r0 = icon(0);
        let pw = ui
            .interact(r0, id.with("pw"), Sense::click())
            .on_hover_text(if d.enabled { "bypass" } else { "enable" });
        let pc = if d.enabled { t.accent } else { ink };
        let c0 = r0.center();
        if pw.hovered() {
            p.circle_filled(c0, 10.0, ink.gamma_multiply(0.15));
        }
        let rad = 5.5;
        let pts: Vec<Pos2> = (0..=20)
            .map(|k| {
                let a = -std::f32::consts::FRAC_PI_2
                    + 0.6
                    + k as f32 / 20.0 * (std::f32::consts::TAU - 1.2);
                Pos2::new(c0.x + rad * a.cos(), c0.y + rad * a.sin())
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(1.6, pc)));
        p.line_segment(
            [c0 - egui::vec2(0.0, 7.0), c0 - egui::vec2(0.0, 1.5)],
            Stroke::new(1.6, pc),
        );
        if pw.clicked() {
            ops.push(FxOp::Toggle(i));
        }
        // preset folder
        let r1 = icon(1);
        let fr = ui
            .interact(r1, id.with("pre"), Sense::click())
            .on_hover_text("presets");
        if fr.hovered() {
            p.circle_filled(r1.center(), 10.0, ink.gamma_multiply(0.15));
        }
        let fb = Rect::from_center_size(r1.center() + egui::vec2(0.0, 1.0), egui::vec2(12.0, 8.0));
        p.rect_stroke(fb, 1, Stroke::new(1.3, ink), egui::StrokeKind::Middle);
        p.line_segment(
            [
                fb.left_top() + egui::vec2(0.0, -2.0),
                fb.left_top() + egui::vec2(5.0, -2.0),
            ],
            Stroke::new(1.3, ink),
        );
        egui::Popup::menu(&fr).show(|ui| {
            ui.label(egui::RichText::new(format!("{} presets", d.title())).strong());
            if d.kind == FxKind::Plugin {
                ui.label(
                    egui::RichText::new("plugin presets live in its editor")
                        .small()
                        .color(th().ink2),
                );
            }
            for (k, (pn, _)) in fx::presets(d.kind).iter().enumerate() {
                if ui.button(*pn).clicked() {
                    ops.push(FxOp::Preset(i, k));
                    ui.close();
                }
            }
        });
        // collapse chevron
        let r2 = icon(2);
        let cr = ui
            .interact(r2, id.with("col"), Sense::click())
            .on_hover_text(if d.collapsed { "expand" } else { "collapse" });
        if cr.hovered() {
            p.circle_filled(r2.center(), 10.0, ink.gamma_multiply(0.15));
        }
        let c2 = r2.center();
        let s = if d.collapsed { 1.0 } else { -1.0 };
        p.add(egui::Shape::line(
            vec![
                c2 + egui::vec2(-2.5 * s, -4.0),
                c2 + egui::vec2(2.5 * s, 0.0),
                c2 + egui::vec2(-2.5 * s, 4.0),
            ],
            Stroke::new(1.6, ink),
        ));
        if cr.clicked() {
            ops.push(FxOp::Collapse(i));
        }
        // routing icon at the bottom: in -> out (insert)
        let rr = Rect::from_center_size(
            Pos2::new(strip.center().x, strip.bottom() - 14.0),
            egui::vec2(20.0, 20.0),
        );
        let c3 = rr.center();
        let st = Stroke::new(1.3, ink);
        p.line_segment(
            [c3 + egui::vec2(-6.0, 4.0), c3 + egui::vec2(-6.0, -4.0)],
            st,
        );
        p.line_segment([c3 + egui::vec2(-6.0, 0.0), c3 + egui::vec2(5.0, 0.0)], st);
        p.add(egui::Shape::line(
            vec![
                c3 + egui::vec2(2.0, -3.0),
                c3 + egui::vec2(5.5, 0.0),
                c3 + egui::vec2(2.0, 3.0),
            ],
            st,
        ));
        ui.interact(rr, id.with("route"), Sense::hover())
            .on_hover_text("insert: instrument -> this device -> next device / mixer channel");
        // name, bottom-to-top, in the space between the icons
        let name_r = Rect::from_min_max(
            Pos2::new(strip.left(), icon(2).bottom() + 6.0),
            Pos2::new(strip.right(), rr.top() - 6.0),
        );
        let galley =
            ui.painter()
                .layout_no_wrap(d.title().to_uppercase(), FontId::proportional(12.5), ink);
        let gw = galley.size().x.min(name_r.height());
        let gh = galley.size().y;
        let pos = Pos2::new(
            strip.center().x - gh * 0.5,
            name_r.bottom() - (name_r.height() - gw) * 0.0,
        );
        p.add(
            egui::epaint::TextShape::new(pos, galley, ink).with_angle(-std::f32::consts::FRAC_PI_2),
        );
        let _ = gw;
        let nr = ui
            .interact(name_r, id.with("name"), Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text(format!(
                "{}: click to {}, drag to reorder",
                d.title(),
                if d.collapsed { "expand" } else { "collapse" }
            ));
        if nr.clicked() {
            ops.push(FxOp::Collapse(i));
        }
        if nr.drag_started() {
            self.fx_drag = Some(i);
        }
        let menu = |ui: &mut egui::Ui, ops: &mut Vec<FxOp>| {
            for (txt, op) in [
                (if d.enabled { "Bypass" } else { "Enable" }, FxOp::Toggle(i)),
                (
                    if d.collapsed { "Expand" } else { "Collapse" },
                    FxOp::Collapse(i),
                ),
                ("Duplicate", FxOp::Dup(i)),
                ("Move left", FxOp::Move(i, i.saturating_sub(1))),
                ("Move right", FxOp::Move(i, (i + 2).min(n))),
                ("Remove", FxOp::Remove(i)),
            ] {
                if ui.button(txt).clicked() {
                    ops.push(op);
                    ui.close();
                }
            }
        };
        nr.context_menu(|ui| menu(ui, ops));
        if d.collapsed {
            return rect;
        }
        // --- body: parameters
        let inner = body.shrink2(egui::vec2(10.0, 8.0));
        let lay = egui::Layout::top_down(egui::Align::LEFT);
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(lay), |ui| {
            ui.set_clip_rect(inner);
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(d.title()).strong().color(t.ink))
                        .truncate(),
                );
                if !d.enabled {
                    ui.label(egui::RichText::new("bypassed").small().color(t.ink2));
                }
            });
            if d.kind == FxKind::Plugin {
                let owner = (self.instr as usize).wrapping_sub(1);
                self.plugin_fx_body(ui, owner, i, d);
                return;
            }
            if d.kind == FxKind::Grid {
                let owner = (self.instr as usize).wrapping_sub(1);
                let n = d.grid.as_ref().map_or(0, |g| g.modules.len());
                if ui
                    .button(format!("Edit patch… ({n} modules)"))
                    .on_hover_text("open the Grid FX patch editor")
                    .clicked()
                {
                    self.ip.grid_fx = Some((owner, i));
                }
            }
            ui.add_space(2.0);
            ui.spacing_mut().slider_width = inner.width() - 4.0;
            ui.spacing_mut().item_spacing.y = 1.0;
            for (k, info) in fx::params(d.kind).iter().enumerate() {
                let v = d.params.get(k).copied().unwrap_or(0.0);
                let (row, _) =
                    ui.allocate_exact_size(egui::vec2(inner.width(), 15.0), Sense::hover());
                let pr = ui.painter();
                pr.text(
                    row.left_center(),
                    egui::Align2::LEFT_CENTER,
                    info.name,
                    FontId::proportional(11.0),
                    t.ink2,
                );
                pr.text(
                    row.right_center(),
                    egui::Align2::RIGHT_CENTER,
                    info.format(v),
                    FontId::monospace(11.0),
                    t.value,
                );
                if !info.choices.is_empty() {
                    let cur = info.to_real(v) as usize;
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (ci, cn) in info.choices.iter().enumerate() {
                            if ui
                                .selectable_label(cur == ci, egui::RichText::new(*cn).small())
                                .clicked()
                            {
                                ops.push(FxOp::Param(i, k, info.to_norm(ci as f32), true));
                            }
                        }
                    });
                } else {
                    let mut nv = v;
                    let r = ui.add(
                        egui::Slider::new(&mut nv, 0.0..=1.0)
                            .show_value(false)
                            .trailing_fill(true),
                    );
                    let owner = (self.instr as usize).wrapping_sub(1);
                    let tid = crate::automation::Target::Fx(owner, i, k).id();
                    let mut lane = false;
                    self.mod_slider(ui, &r, &tid, v, Some(&mut lane));
                    if lane {
                        self.add_lane(crate::automation::Target::Fx(owner, i, k));
                    }
                    if r.double_clicked() {
                        ops.push(FxOp::Param(i, k, info.to_norm(info.def), true));
                    } else if r.changed() {
                        ops.push(FxOp::Param(i, k, nv, r.drag_started() || !r.dragged()));
                    } else if r.drag_started() {
                        ops.push(FxOp::Param(i, k, nv, true));
                    }
                }
            }
        });
        rect
    }
}

fn io_tag(ui: &mut egui::Ui, s: &str, h: f32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, h), Sense::hover());
    let p = ui.painter();
    p.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        s,
        FontId::monospace(11.0),
        th().ink2,
    );
    p.hline(
        r.x_range(),
        r.center().y + 12.0,
        Stroke::new(1.0, th().ink2),
    );
}
