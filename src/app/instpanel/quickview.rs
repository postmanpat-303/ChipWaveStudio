//! Quick-mod in the instrument panel: the dot on every knob opens a small
//! "Modulate <param> with…" popover, and one Movement list per instrument
//! shows everything that moves (source → target · amount · ×). Where it
//! lands is `crate::quickmod`'s job (an engine's own slots first, else a
//! song-level modulator), so songs keep their format.
use super::*;
use crate::automation::Target;
use crate::modulation::LfoShape;
use crate::quickmod::{self as qm, RowId, Source, Spec};

/// the open popover
pub(in crate::app) struct Quick {
    pub inst: usize,
    /// the parameter key it belongs to ("" = opened from the Movement list)
    pub key: String,
    pub target: Option<Target>,
    pub name: String,
    pub spec: Spec,
    /// editing an existing movement instead of adding one
    pub edit: Option<RowId>,
    /// a "slots full" or other message
    pub msg: Option<String>,
    pub opened: f64,
    /// the frame it was last drawn (drawn once per frame)
    pub drawn: u64,
    /// undo snapshot taken for this edit
    pub snap: bool,
}

/// beyond the song's route indices: a ring of an engine's own route
pub(super) const NATIVE_BASE: usize = 1 << 20;

/// the colour of an engine's own route `j` (song modulators use theirs)
pub(super) fn native_colour(j: usize) -> egui::Color32 {
    super::super::modview::mod_colour(j + 3)
}

fn shape_icon(ui: &mut egui::Ui, s: LfoShape, on: bool) -> egui::Response {
    let t = th();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(30.0, 22.0), Sense::click());
    let p = ui.painter();
    let fill = if on {
        t.accent.gamma_multiply(0.35)
    } else if resp.hovered() {
        t.ink2.gamma_multiply(0.15)
    } else {
        egui::Color32::TRANSPARENT
    };
    p.rect_filled(r, 4.0, fill);
    p.rect_stroke(
        r,
        4.0,
        Stroke::new(
            1.0,
            if on {
                t.accent
            } else {
                t.ink2.gamma_multiply(0.5)
            },
        ),
        egui::StrokeKind::Inside,
    );
    let ir = r.shrink2(egui::vec2(6.0, 6.0));
    let pts: Vec<Pos2> = (0..=24)
        .map(|i| {
            let ph = i as f32 / 24.0;
            // the square's edges as steps
            let y = s.at(ph.min(0.9999));
            Pos2::new(
                ir.left() + ph * ir.width(),
                ir.center().y - y * ir.height() * 0.5,
            )
        })
        .collect();
    p.add(egui::Shape::line(
        pts,
        Stroke::new(1.5, if on { t.ink } else { t.ink2 }),
    ));
    let nm = s.name();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, nm));
    resp.on_hover_text(nm)
}

/// ‹ 1/4 › : the rate in note values (or Free, then Hz)
fn rate_ui(ui: &mut egui::Ui, sync: &mut f32, rate: &mut f32) -> bool {
    use super::super::modview::SYNCS;
    let mut changed = false;
    let cur = SYNCS
        .iter()
        .position(|s| (s.0 - *sync).abs() < 1e-4)
        .unwrap_or(3);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Rate").small());
        if ui
            .add_enabled(cur > 0, egui::Button::new("‹").small())
            .on_hover_text("faster")
            .clicked()
        {
            *sync = SYNCS[cur - 1].0;
            changed = true;
        }
        let name = if cur == 0 { "Free" } else { SYNCS[cur].1 };
        ui.add_sized(
            egui::vec2(52.0, 18.0),
            egui::Label::new(egui::RichText::new(name).strong()),
        )
        .on_hover_text("cycle length, in step with the song's tempo (Free = in Hz)");
        if ui
            .add_enabled(cur + 1 < SYNCS.len(), egui::Button::new("›").small())
            .on_hover_text("slower")
            .clicked()
        {
            *sync = SYNCS[cur + 1].0;
            changed = true;
        }
        if *sync <= 0.0 {
            changed |= ui
                .add(
                    egui::DragValue::new(rate)
                        .range(0.02..=20.0)
                        .speed(0.02)
                        .suffix(" Hz")
                        .max_decimals(2),
                )
                .changed();
        }
    });
    changed
}

fn amount_ui(ui: &mut egui::Ui, a: &mut f32) -> bool {
    let mut pct = *a * 100.0;
    let r = ui
        .horizontal(|ui| {
            ui.label(egui::RichText::new("Amount").small());
            ui.spacing_mut().slider_width = 150.0;
            ui.add(
                egui::Slider::new(&mut pct, -100.0..=100.0)
                    .suffix(" %")
                    .max_decimals(0),
            )
            .on_hover_text("how far it moves the knob (of its whole travel, ±)")
        })
        .inner;
    if r.changed() {
        *a = (pct / 100.0).clamp(-1.0, 1.0);
    }
    r.changed()
}

/// the target part of a song route's label without the instrument's name
fn short_label(label: &str, inst: &Instrument) -> String {
    label
        .strip_prefix(&format!("{} ", inst.name))
        .unwrap_or(label)
        .to_string()
}

impl App {
    /// every movement of the instrument being edited
    pub(super) fn quick_rows(&self, inst: &Instrument, cx: &Cx) -> Vec<qm::Row> {
        let labels = &cx.labels;
        let m = &cx.modm;
        let lbl = |id: &str| {
            let r = m.routes.iter().position(|r| r.target == id);
            r.and_then(|r| labels.get(r))
                .map(|l| short_label(l, inst))
                .unwrap_or_else(|| id.to_string())
        };
        qm::rows(inst, cx.idx, m, &lbl)
    }

    /// open the popover for knob `p` (editing what already moves it, if
    /// anything does)
    pub(super) fn quick_open(&mut self, p: &Param, inst: &Instrument, cx: &Cx, now: f64) {
        let Some(tg) = p.target else {
            return;
        };
        let rows = self.quick_rows(inst, cx);
        let edit = rows.iter().find(|r| r.knobs.contains(&tg)).map(|r| r.id);
        self.quick_open_at(cx.idx, &p.key, Some(tg), &p.name, edit, now);
    }

    pub(super) fn quick_open_at(
        &mut self,
        inst: usize,
        key: &str,
        target: Option<Target>,
        name: &str,
        edit: Option<RowId>,
        now: f64,
    ) {
        self.ip.quick = Some(Quick {
            inst,
            key: key.to_string(),
            target,
            name: name.to_string(),
            spec: Spec::default(),
            edit,
            msg: None,
            opened: now,
            drawn: u64::MAX,
            snap: false,
        });
    }

    /// draw the popover under `anchor` when it belongs to this knob / row
    pub(super) fn quick_popover(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        anchor: Rect,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let frame = ctx.cumulative_pass_nr();
        let Some(q) = self.ip.quick.as_mut() else {
            return;
        };
        if q.inst != cx.idx || q.key != key || q.drawn == frame {
            return;
        }
        q.drawn = frame;
        let t = th();
        let rows = self.quick_rows(inst, cx);
        let bpm = self.with(|s| s.song.bpm);
        let Some(mut q) = self.ip.quick.take() else {
            return;
        };
        // an edited row that is gone (removed elsewhere): add mode
        if q.edit.is_some_and(|e| !rows.iter().any(|r| r.id == e)) {
            q.edit = None;
        }
        let w = 290.0;
        let screen = ctx.content_rect();
        let mut pos = Pos2::new(anchor.left(), anchor.bottom() + 4.0);
        if pos.y + 250.0 > screen.bottom() {
            pos.y = (anchor.top() - 254.0).max(screen.top());
        }
        pos.x = pos.x.min(screen.right() - w - 16.0).max(screen.left());
        let mut close = false;
        let mut add = false;
        let mut remove = None;
        let area = egui::Area::new(egui::Id::unique("quickmod_pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(w);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Modulate {} with…", q.name)).strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").on_hover_text("close (Esc)").clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(2.0);
                    if let Some(e) = q.edit {
                        let Some(row) = rows.iter().find(|r| r.id == e) else {
                            return;
                        };
                        let col = row
                            .modk
                            .map(super::super::modview::mod_colour)
                            .unwrap_or(t.accent);
                        ui.horizontal(|ui| {
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 4.5, col);
                            ui.label(format!("{} → {}", row.source, row.target));
                        });
                        if row.per_note {
                            ui.label(
                                egui::RichText::new("per note: every voice moves on its own")
                                    .small()
                                    .color(t.ink2),
                            );
                        }
                        // a song-level LFO / random: its shape and rate
                        if let (RowId::Song(_), Some(k)) = (row.id, row.modk) {
                            let md = &mut cx.modm.mods[k];
                            let shared = cx.modm.routes.iter().filter(|r| r.src == md.id).count();
                            let md = &mut cx.modm.mods[k];
                            if matches!(md.kind, crate::modulation::ModKind::Lfo) {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Shape").small());
                                    for s in LfoShape::ALL {
                                        if shape_icon(ui, s, md.shape == s).clicked() {
                                            md.shape = s;
                                        }
                                    }
                                });
                            }
                            if matches!(
                                md.kind,
                                crate::modulation::ModKind::Lfo
                                    | crate::modulation::ModKind::Random
                            ) {
                                rate_ui(ui, &mut md.sync, &mut md.rate);
                            }
                            if shared > 1 {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "shape and rate are shared by {shared} knobs"
                                    ))
                                    .small()
                                    .color(t.ink2),
                                );
                            }
                        }
                        let mut a = row.amount;
                        if amount_ui(ui, &mut a) {
                            if !q.snap {
                                q.snap = true;
                            }
                            qm::set_amount(inst, &mut cx.modm, row.id, a);
                            cx.changed = true;
                        }
                        ui.label(egui::RichText::new(row.text.clone()).small().color(t.value));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button("Remove")
                                .on_hover_text("stop this movement")
                                .clicked()
                            {
                                remove = Some(row.id);
                            }
                            if q.target.is_some()
                                && ui
                                    .button("+ Add another")
                                    .on_hover_text("move this knob with a second source too")
                                    .clicked()
                            {
                                q.edit = None;
                                q.msg = None;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Done").clicked() {
                                        close = true;
                                    }
                                },
                            );
                        });
                        return;
                    }
                    // ---- add ----
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                        for s in Source::ALL {
                            let on = q.spec.source == s;
                            let b = ui.add(
                                egui::Button::selectable(on, s.name())
                                    .min_size(egui::vec2(52.0, 22.0)),
                            );
                            if b.clicked() {
                                q.spec.source = s;
                                q.msg = None;
                            }
                        }
                    });
                    ui.add_space(2.0);
                    if q.spec.source == Source::Lfo {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Shape").small());
                            for s in LfoShape::ALL {
                                if shape_icon(ui, s, q.spec.shape == s).clicked() {
                                    q.spec.shape = s;
                                }
                            }
                        });
                    }
                    if q.spec.source.has_rate() {
                        rate_ui(ui, &mut q.spec.sync, &mut q.spec.rate);
                    }
                    amount_ui(ui, &mut q.spec.amount);
                    // where it would go (a dry run on copies)
                    let hint = q.target.map(|tg| {
                        let mut i2 = inst.clone();
                        let mut m2 = cx.modm.clone();
                        qm::add(&mut i2, cx.idx, &mut m2, tg, &q.spec, bpm)
                    });
                    let ok = matches!(hint, Some(Ok(_)));
                    match (&q.msg, &hint) {
                        (Some(m), _) | (None, Some(Err(m))) => {
                            ui.label(egui::RichText::new(m).small().color(t.brick));
                        }
                        (None, Some(Ok(a))) => {
                            let txt = match (&a.note, a.row) {
                                (Some(n), _) => n.clone(),
                                (None, RowId::Native(_)) => format!(
                                    "uses {}'s own modulation",
                                    qm::engine(&inst.kind)
                                        .map(|e| e.engine())
                                        .unwrap_or("the instrument")
                                ),
                                (None, RowId::Song(_)) => String::new(),
                            };
                            if !txt.is_empty() {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(txt).small().color(t.ink2),
                                    )
                                    .wrap(),
                                );
                            }
                        }
                        _ => {}
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(ok, egui::Button::new("Add")).clicked() {
                            add = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            });
        let rect = area.response.rect;
        let outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !rect.contains(p) && !anchor.contains(p))
        });
        let now = ctx.input(|i| i.time);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || (outside && now - q.opened > 0.2) {
            close = true;
        }
        if add {
            if let Some(tg) = q.target {
                self.snapshot();
                match qm::add(inst, cx.idx, &mut cx.modm, tg, &q.spec, bpm) {
                    Ok(a) => {
                        cx.changed = true;
                        self.status = match a.note {
                            Some(n) => n,
                            None => format!(
                                "{} now moves with {}: drag its ring to change how far",
                                q.name,
                                q.spec.source.name()
                            ),
                        };
                        if let RowId::Song(r) = a.row {
                            self.ip.mod_sel = cx.modm.routes.get(r).map(|x| x.src);
                        }
                        close = true;
                    }
                    Err(e) => {
                        self.status = e.clone();
                        q.msg = Some(e);
                    }
                }
            }
        }
        if let Some(id) = remove {
            match id {
                RowId::Native(n) => {
                    self.snapshot();
                    qm::remove_native(inst, n);
                    cx.changed = true;
                }
                RowId::Song(r) => cx.mod_out.remove_route = Some(r),
            }
            self.status = format!("{}: movement removed", q.name);
            close = true;
        }
        if q.snap {
            // one undo step for the amount edit, taken before it lands
            q.snap = false;
        }
        if !close {
            self.ip.quick = Some(q);
        }
    }

    /// the Movement list of the instrument being edited
    pub(super) fn movement_list(
        &mut self,
        ui: &mut egui::Ui,
        inst: &mut Instrument,
        cx: &mut Cx,
        head: bool,
    ) {
        let t = th();
        let rows = self.quick_rows(inst, cx);
        let now = ui.input(|i| i.time);
        ui.horizontal(|ui| {
            if head {
                ui.label(egui::RichText::new("Movement").strong());
            }
            if !rows.is_empty() {
                ui.label(
                    egui::RichText::new(format!("{} active", rows.len()))
                        .small()
                        .color(t.ink2),
                );
            }
        });
        if rows.is_empty() {
            ui.add(
                egui::Label::new(
                    egui::RichText::new("Click the dot on any knob to add movement").color(t.ink2),
                )
                .wrap(),
            );
            return;
        }
        let mut native_j = 0;
        for row in &rows {
            let col = match row.modk {
                Some(k) => super::super::modview::mod_colour(k),
                None => {
                    native_j += 1;
                    native_colour(native_j - 1)
                }
            };
            let key = format!("move.{:?}", row.id);
            let resp = ui
                .horizontal(|ui| {
                    // the colour dot: a song-level source can still be
                    // dragged onto any knob (or FX slider)
                    let (dr, dresp) = ui.allocate_exact_size(egui::vec2(12.0, 18.0), Sense::drag());
                    ui.painter().circle_filled(dr.center(), 4.5, col);
                    if let (Some(k), true) = (row.modk, dresp.dragged()) {
                        dresp.dnd_set_drag_payload(super::super::modview::ModDrag(
                            cx.modm.mods[k].id,
                        ));
                    }
                    let lw = (ui.available_width() - 110.0).max(80.0);
                    let txt = format!(
                        "{} → {}{}",
                        row.source,
                        row.target,
                        if row.per_note { " (per note)" } else { "" }
                    );
                    let lr = ui
                        .add_sized(
                            egui::vec2(lw, 18.0),
                            egui::Label::new(egui::RichText::new(&txt))
                                .truncate()
                                .sense(Sense::click()),
                        )
                        .on_hover_text(format!("{txt}  {}\nclick to edit", row.text));
                    if lr.clicked() {
                        let tg = row.knobs.first().copied();
                        self.quick_open_at(cx.idx, &key, tg, &row.target, Some(row.id), now);
                    }
                    let mut pct = row.amount * 100.0;
                    let dv = ui
                        .add(
                            egui::DragValue::new(&mut pct)
                                .range(-100.0..=100.0)
                                .speed(0.5)
                                .suffix(" %")
                                .max_decimals(0),
                        )
                        .on_hover_text(format!("amount: {}", row.text));
                    if dv.changed() {
                        qm::set_amount(inst, &mut cx.modm, row.id, pct / 100.0);
                        cx.changed = true;
                    }
                    let x = ui.small_button("×").on_hover_text("remove this movement");
                    let nm = format!("Remove {txt}");
                    x.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &nm));
                    if x.clicked() {
                        match row.id {
                            RowId::Native(n) => {
                                self.snapshot();
                                qm::remove_native(inst, n);
                                cx.changed = true;
                            }
                            RowId::Song(r) => cx.mod_out.remove_route = Some(r),
                        }
                    }
                    if let RowId::Song(r) = row.id {
                        lr.context_menu(|ui| {
                            if ui.button("Add automation lane (amount)").clicked() {
                                cx.mod_out.lane = Some(r);
                                ui.close();
                            }
                        });
                    }
                })
                .response;
            self.quick_popover(&ui.ctx().clone(), &key, resp.rect, inst, cx);
        }
    }
}
