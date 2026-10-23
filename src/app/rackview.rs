//! The Rack (rack layout A, approved by Del (postmanpat)): the lower panel's
//! Notes | Rack toggle shows the track's sound as one row, instrument →
//! effects → the `+` drop zone. Every card gets a side tab: on / bypass,
//! remove, collapse and ⋮ (a menu with the rest). A click selects a card
//! (a violet outline on a built-in instrument, the accent otherwise); Delete,
//! Ctrl+D and 0 act on the selected card while the Rack was the last panel
//! clicked, so Delete on the Timeline still deletes clips. The row draws
//! with the device-card painters of devices.rs; nothing new is stored in
//! the song.
//! Original ChipWave code.
use super::chains::Sel;
use super::fxview::FxOp;
use super::*;
use crate::fx::{self, DevPath};

/// a side tab's width
pub(crate) const TAB_W: f32 = 22.0;

/// the Rack's selected card
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Pick {
    /// the instrument card
    #[default]
    Inst,
    /// effect `n` of the chain
    Fx(usize),
}

/// what a side tab's buttons asked for
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabHit {
    Power,
    Remove,
    Collapse,
}

/// the outline / tab colour of a selected card: the built-in skin's tint
/// for a built-in instrument, the accent otherwise
pub(crate) fn pick_colour(builtin_inst: bool) -> Color32 {
    let t = th();
    if builtin_inst {
        crate::theme::instskin().map_or(t.accent, |s| s.tint)
    } else {
        t.accent
    }
}

/// the tab's ink on a selected (filled) tab
fn on_ink() -> Color32 {
    if th().light {
        Color32::WHITE
    } else {
        Color32::from_rgb(20, 16, 28)
    }
}

fn icon_power(p: &egui::Painter, c: Pos2, col: Color32) {
    let pts: Vec<Pos2> = (0..=20)
        .map(|k| {
            let a = -std::f32::consts::FRAC_PI_2
                + 0.65
                + k as f32 / 20.0 * (std::f32::consts::TAU - 1.3);
            c + egui::vec2(a.cos(), a.sin()) * 5.5
        })
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.6, col)));
    p.line_segment(
        [c - egui::vec2(0.0, 7.0), c - egui::vec2(0.0, 1.0)],
        Stroke::new(1.6, col),
    );
}

fn icon_x(p: &egui::Painter, c: Pos2, col: Color32) {
    for s in [-1.0, 1.0] {
        p.line_segment(
            [c + egui::vec2(-4.5, -4.5 * s), c + egui::vec2(4.5, 4.5 * s)],
            Stroke::new(1.6, col),
        );
    }
}

fn icon_chev(p: &egui::Painter, c: Pos2, right: bool, col: Color32) {
    let s = if right { 1.0 } else { -1.0 };
    p.add(egui::Shape::line(
        vec![
            c + egui::vec2(-2.5 * s, -5.0),
            c + egui::vec2(2.5 * s, 0.0),
            c + egui::vec2(-2.5 * s, 5.0),
        ],
        Stroke::new(1.6, col),
    ));
}

fn icon_more(p: &egui::Painter, c: Pos2, col: Color32) {
    for k in -1..=1 {
        p.circle_filled(c + egui::vec2(0.0, k as f32 * 5.0), 1.8, col);
    }
}

impl App {
    /// the Rack has the keyboard: it shows and was the last panel clicked
    pub(crate) fn rack_focus(&self) -> bool {
        self.dev.focus && !self.dev.tree && self.devices_visible()
    }

    /// a side tab at `tab` for the card `card` (`pick`); `on` / `collapsed`
    /// are the card's state, `power` and `remove` whether it has those
    /// buttons (an instrument has neither: it stays, Replace swaps it)
    #[allow(clippy::too_many_arguments)]
    pub(super) fn rack_tab(
        &mut self,
        ui: &mut egui::Ui,
        tab: Rect,
        card: Rect,
        pick: Pick,
        on: bool,
        collapsed: bool,
        builtin: bool,
        idx: usize,
        ops: &mut Vec<FxOp>,
    ) -> Option<TabHit> {
        let t = th();
        let sel = self.dev.pick == pick;
        let hl = pick_colour(builtin && pick == Pick::Inst);
        let p = ui.painter_at(tab.union(card).expand(4.0));
        p.rect_filled(
            tab,
            egui::CornerRadius {
                nw: 6,
                sw: 6,
                ne: 0,
                se: 0,
            },
            if sel { hl } else { t.ink2.gamma_multiply(0.16) },
        );
        let ink = if sel { on_ink() } else { t.ink2 };
        let is_fx = matches!(pick, Pick::Fx(_));
        let id = egui::Id::unique(("rack_tab", idx, pick));
        let cx = tab.center().x;
        let mut y = tab.top() + 14.0;
        let mut hit = None;
        // the tab itself (under its buttons): a double-click expands or
        // collapses the card (Del (postmanpat) 23 Oct 2026)
        let bg = ui
            .interact(tab, id.with("tab"), Sense::click())
            .on_hover_text(if collapsed {
                "Double-click to expand"
            } else {
                "Double-click to collapse to a strip"
            });
        if bg.double_clicked() {
            hit = Some(TabHit::Collapse);
        }
        let button = |p: &egui::Painter, y: f32, key: &str, tip: &str| {
            let r = Rect::from_center_size(Pos2::new(cx, y), egui::vec2(20.0, 22.0));
            let resp = ui
                .interact(r, id.with(key), Sense::click())
                .on_hover_text(tip);
            if resp.hovered() {
                p.rect_filled(r, 4, ink.gamma_multiply(0.18));
            }
            resp
        };
        if is_fx {
            let r = button(
                &p,
                y,
                "pw",
                if on {
                    "On: click to bypass  (0)"
                } else {
                    "Bypassed: click to turn on  (0)"
                },
            );
            icon_power(
                &p,
                Pos2::new(cx, y),
                if on { ink } else { ink.gamma_multiply(0.45) },
            );
            if r.clicked() {
                hit = Some(TabHit::Power);
            }
            y += 26.0;
            let r = button(&p, y, "x", "Remove  (Delete · Ctrl+Z puts it back)");
            icon_x(
                &p,
                Pos2::new(cx, y),
                if r.hovered() { t.brick } else { ink },
            );
            if r.clicked() {
                hit = Some(TabHit::Remove);
            }
            y += 26.0;
        }
        let r = button(
            &p,
            y,
            "col",
            if collapsed {
                "Expand"
            } else {
                "Collapse to a strip"
            },
        );
        icon_chev(&p, Pos2::new(cx, y), collapsed, ink);
        if r.clicked() {
            hit = Some(TabHit::Collapse);
        }
        y += 26.0;
        let mr = button(
            &p,
            y,
            "more",
            if is_fx {
                "More: presets, duplicate, move, all parameters, bypass, remove"
            } else {
                "More: replace, presets, advanced editor, make rack, save rack"
            },
        );
        icon_more(&p, Pos2::new(cx, y), ink);
        if mr.clicked() {
            self.dev.pick = pick;
        }
        egui::Popup::menu(&mr).show(|ui| {
            ui.set_min_width(200.0);
            self.rack_menu(ui, idx, pick, ops);
        });
        if sel {
            p.rect_stroke(
                tab.union(card).expand(1.0),
                7,
                Stroke::new(2.0, hl),
                egui::StrokeKind::Outside,
            );
        }
        hit
    }

    /// the ⋮ menu (also the card's right-click menu)
    pub(super) fn rack_menu(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        pick: Pick,
        ops: &mut Vec<FxOp>,
    ) {
        match pick {
            Pick::Inst => self.rack_inst_menu(ui, idx),
            Pick::Fx(i) => self.rack_fx_menu(ui, idx, i, ops),
        }
    }

    fn rack_inst_menu(&mut self, ui: &mut egui::Ui, idx: usize) {
        let (name, rack, layer) = self.with(|s| {
            let i = s.song.instruments.get(idx);
            (
                i.map(|i| i.name.clone()).unwrap_or_default(),
                matches!(i.map(|i| &i.kind), Some(InstrumentKind::Rack(_))),
                s.song.rack_of(idx).is_some(),
            )
        });
        ui.label(egui::RichText::new(name).strong());
        if ui
            .button("Replace instrument…")
            .on_hover_text("pick the new one in Browser › Instruments; the effects stay")
            .clicked()
        {
            self.track_change_instrument(self.ch);
            ui.close();
        }
        ui.menu_button("Presets", |ui| self.rack_presets(ui, idx));
        if ui
            .button("Open advanced editor")
            .on_hover_text("every control of this instrument")
            .clicked()
        {
            self.instr = (idx + 1) as u8;
            self.ip.advanced = true;
            self.set_lower(1);
            ui.close();
        }
        ui.separator();
        if !rack
            && !layer
            && ui
                .button("Make rack")
                .on_hover_text("put this instrument in a rack as its first layer")
                .clicked()
        {
            self.make_rack(idx);
            ui.close();
        }
        let root = self.chains_root();
        if ui
            .add_enabled(root.is_some(), egui::Button::new("Save rack…"))
            .on_hover_text("save the whole row (instrument, layers, effects) to My Racks")
            .clicked()
        {
            if let Some(r) = root {
                self.open_save_rack(r);
            }
            ui.close();
        }
        if ui
            .button(if self.dev.inst_folded {
                "Expand"
            } else {
                "Collapse"
            })
            .clicked()
        {
            self.dev.inst_folded = !self.dev.inst_folded;
            ui.close();
        }
        ui.label(
            egui::RichText::new("Automate: right-click a knob")
                .small()
                .color(th().ink2),
        );
    }

    fn rack_fx_menu(&mut self, ui: &mut egui::Ui, idx: usize, i: usize, ops: &mut Vec<FxOp>) {
        let Some((d, n)) = self.with(|s| {
            let c = &s.song.instruments.get(idx)?.fx;
            Some((c.get(i)?.clone(), c.len()))
        }) else {
            return;
        };
        ui.label(egui::RichText::new(d.title()).strong());
        let pre = fx::presets(d.kind);
        if !pre.is_empty() {
            ui.menu_button("Presets", |ui| {
                for (k, (pn, _)) in pre.iter().enumerate() {
                    if ui.button(*pn).clicked() {
                        ops.push(FxOp::Preset(i, k));
                        ui.close();
                    }
                }
            });
        }
        let key = (
            idx,
            super::devcards::open_key(&self.with(|s| s.song.instruments[idx].fx.clone()), i),
        );
        let open = self.dev.open.contains(&key);
        if ui
            .button(if open {
                "Hide extra parameters"
            } else {
                "Show all parameters"
            })
            .clicked()
        {
            if !self.dev.open.remove(&key) {
                self.dev.open.insert(key);
            }
            ui.close();
        }
        if ui
            .button("Open in the chain editor")
            .on_hover_text("the detailed view: split paths, every parameter")
            .clicked()
        {
            self.dev.tree = true;
            self.extra.chains_tree_folded = false;
            self.dev.sel = Some(Sel::Dev(idx, DevPath::top(i)));
            ui.close();
        }
        ui.separator();
        let mut item = |ui: &mut egui::Ui, txt: &str, short: &str, op: FxOp, en: bool| {
            let b = egui::Button::new(txt).shortcut_text(short);
            if ui.add_enabled(en, b).clicked() {
                ops.push(op);
                ui.close();
            }
        };
        item(ui, "Duplicate", "Ctrl+D", FxOp::Dup(i), true);
        item(
            ui,
            "Move left",
            "",
            FxOp::Move(i, i.saturating_sub(1)),
            i > 0,
        );
        item(
            ui,
            "Move right",
            "",
            FxOp::Move(i, (i + 2).min(n)),
            i + 1 < n,
        );
        item(
            ui,
            if d.enabled { "Bypass" } else { "Turn on" },
            "0",
            FxOp::Toggle(i),
            true,
        );
        item(
            ui,
            if d.collapsed { "Expand" } else { "Collapse" },
            "",
            FxOp::Collapse(i),
            true,
        );
        item(ui, "Remove", "Delete", FxOp::Remove(i), true);
    }

    /// Delete / Backspace, Ctrl+D and 0 on the selected card while the Rack
    /// has the keyboard; true when used
    pub(crate) fn rack_key(&mut self, key: egui::Key, m: egui::Modifiers) -> bool {
        if !self.rack_focus() {
            return false;
        }
        let idx = (self.instr as usize).wrapping_sub(1);
        let Some(n) = self.with(|s| s.song.instruments.get(idx).map(|i| i.fx.len())) else {
            return false;
        };
        let fx = match self.dev.pick {
            Pick::Fx(i) if i < n => Some(i),
            _ => None,
        };
        let plain = !m.command && !m.alt && !m.shift;
        let op = match key {
            egui::Key::Delete | egui::Key::Backspace if plain => match fx {
                Some(i) => FxOp::Remove(i),
                None => {
                    self.status =
                        "The instrument stays: right-click it › Replace instrument… swaps it"
                            .into();
                    return true;
                }
            },
            egui::Key::D if m.command && !m.alt && !m.shift => match fx {
                Some(i) => FxOp::Dup(i),
                None => return false,
            },
            egui::Key::Num0 if plain => match fx {
                Some(i) => FxOp::Toggle(i),
                None => return false,
            },
            _ => return false,
        };
        // the selection follows: a removed card selects its neighbour
        if let FxOp::Remove(i) = op {
            self.dev.pick = if n <= 1 {
                Pick::Inst
            } else {
                Pick::Fx(i.min(n - 2))
            };
        } else if let FxOp::Dup(i) = op {
            self.dev.pick = Pick::Fx(i + 1);
        }
        self.apply_fx_ops(idx, vec![op]);
        true
    }
}

/// the instrument card's width on the Rack: room for its face's columns
pub(crate) fn inst_card_w(app: &App, idx: usize) -> f32 {
    let kind = app.with(|s| {
        s.song.instruments.get(idx).map(|i| {
            if matches!(i.kind, InstrumentKind::Rack(_)) {
                2
            } else if crate::drumkit::kit_of(i).is_some() {
                1
            } else {
                0
            }
        })
    });
    match kind {
        Some(2) => 300.0,
        Some(1) => 5.0 * super::knob::CELL.x + 16.0,
        _ => 4.0 * super::knob::CELL.x + 16.0,
    }
}

/// a child ui at `r` that leaves the parent's cursor alone
fn at<R>(ui: &mut egui::Ui, r: Rect, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut c = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    c.set_clip_rect(r.expand(2.0).intersect(ui.clip_rect()));
    f(&mut c)
}

impl App {
    /// the Rack instrument card under its header: the face (Preset ▾,
    /// curated knobs and Volume; a Drum Kit's pad; a rack's layers), a
    /// picture when there is room, and Advanced ›
    pub(super) fn rack_inst_body(
        &mut self,
        ui: &mut egui::Ui,
        p: &egui::Painter,
        main: Rect,
        foot_h: f32,
        idx: usize,
        tag: &str,
    ) {
        let t = th();
        let cell = super::knob::CELL;
        let (rack, kit, presets) = self.with(|s| {
            let i = s.song.instruments.get(idx);
            (
                matches!(i.map(|i| &i.kind), Some(InstrumentKind::Rack(_))),
                i.is_some_and(|i| crate::drumkit::kit_of(i).is_some()),
                i.is_some_and(|i| !super::instpanel::instcard::preset_list(i).is_empty()),
            )
        });
        let body = Rect::from_min_max(
            main.min,
            Pos2::new(main.right(), main.bottom() - foot_h - 4.0),
        );
        let cols = ((body.width() / cell.x).floor() as usize).max(1);
        if rack {
            at(ui, body, |ui| self.rack_layers(ui, idx));
        } else {
            let top = if kit || presets { 26.0 } else { 0.0 };
            let rows = (((body.height() - top) / cell.y).floor() as usize).clamp(1, 3);
            // a tall panel (full height) has room for the picture too
            let spare = body.height() - top - 2.0 * cell.y - 8.0;
            let mut face = body;
            if spare >= 60.0 && !kit {
                let g = Rect::from_min_size(
                    Pos2::new(body.left(), body.top() + top),
                    egui::vec2(body.width(), spare.min(160.0)),
                );
                super::devcards::well(p, g);
                match self.inst_card_viz(idx) {
                    Some(v) => super::knob::draw_viz(p, g.shrink(2.0), &v),
                    None => {
                        p.text(
                            g.center(),
                            egui::Align2::CENTER_CENTER,
                            tag,
                            FontId::proportional(14.0),
                            t.ink2,
                        );
                    }
                }
                face = body;
            }
            let shown = at(ui, face, |ui| {
                if kit {
                    self.dk_rack_face(ui, idx, cols, rows);
                    1
                } else if spare >= 60.0 {
                    // the preset row, the picture, then the knobs
                    let mut n = 0;
                    if presets {
                        n = self.rack_face(ui, idx, 0, 0, true);
                    }
                    ui.add_space(spare.min(160.0) + 4.0);
                    n + self.rack_face(ui, idx, cols, 2, false)
                } else {
                    self.rack_face(ui, idx, cols, rows, true)
                }
            });
            if shown == 0 {
                at(ui, face, |ui| {
                    if self.inst_key_knobs(ui, idx, cols.min(4)) == 0 {
                        ui.label(
                            egui::RichText::new("its controls are in Advanced ›").color(t.ink2),
                        );
                    }
                });
            }
        }
        let b = Rect::from_min_max(
            Pos2::new(main.right() - 92.0, main.bottom() - foot_h),
            main.max,
        );
        if ui
            .put(b, egui::Button::new("Advanced ›").small())
            .on_hover_text("every control of this instrument (the full editor)")
            .clicked()
        {
            self.instr = (idx + 1) as u8;
            self.ip.advanced = true;
            self.set_lower(1);
        }
    }

    /// a rack's layers in its card: letter, name · type, level, Open ›
    /// (the layer's own Rack: its sound and effects), then + Layer
    fn rack_layers(&mut self, ui: &mut egui::Ui, idx: usize) {
        let t = th();
        let layers: Vec<(usize, String, String, f32, bool)> = self.with(|s| {
            let Some(InstrumentKind::Rack(r)) = s.song.instruments.get(idx).map(|i| &i.kind) else {
                return vec![];
            };
            r.layers
                .iter()
                .map(|l| {
                    let (n, tg) = s
                        .song
                        .instruments
                        .get(l.slot)
                        .map(|i| (i.name.clone(), params::type_tag(&i.kind).to_string()))
                        .unwrap_or_default();
                    (l.slot, n, tg, l.level_db, l.on)
                })
                .collect()
        });
        if layers.is_empty() {
            ui.label(egui::RichText::new("No layers yet").color(t.ink2));
        }
        for (k, (slot, name, tag, db, on)) in layers.iter().enumerate() {
            let (r, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(
                r.shrink2(egui::vec2(0.0, 2.0)),
                5,
                t.ink2.gamma_multiply(0.10),
            );
            let letter = (b'A' + k as u8) as char;
            p.text(
                Pos2::new(r.left() + 12.0, r.center().y),
                egui::Align2::CENTER_CENTER,
                letter,
                crate::theme::label_font(13.0),
                if *on { t.accent } else { t.ink2 },
            );
            let open = Rect::from_min_max(
                Pos2::new(r.right() - 60.0, r.top() + 5.0),
                Pos2::new(r.right() - 4.0, r.bottom() - 5.0),
            );
            let dbs = if *db <= -60.0 {
                "-inf dB".to_string()
            } else {
                format!("{db:+.1} dB")
            };
            p.text(
                Pos2::new(open.left() - 6.0, r.center().y),
                egui::Align2::RIGHT_CENTER,
                &dbs,
                FontId::proportional(11.0),
                t.ink2,
            );
            let nr = Rect::from_min_max(
                Pos2::new(r.left() + 24.0, r.top()),
                Pos2::new(open.left() - 58.0, r.bottom()),
            );
            let nf = FontId::proportional(12.0);
            let txt = super::pattern::fit(p, &format!("{name} · {tag}"), &nf, nr.width());
            p.text(
                nr.left_center(),
                egui::Align2::LEFT_CENTER,
                txt,
                nf,
                if *on { t.ink } else { t.ink2 },
            );
            if ui
                .put(open, egui::Button::new("Open ›").small())
                .on_hover_text(format!("{name}'s own Rack: its sound and effects"))
                .clicked()
            {
                self.instr = (*slot + 1) as u8;
                self.dev.pick = Pick::Inst;
            }
        }
        if layers.len() < 4
            && ui
                .button("+ Layer")
                .on_hover_text("a new layer (an init synth); drop an instrument on a layer's Open › to swap it")
                .clicked()
        {
            self.add_layer(idx, None);
        }
    }

    /// above the row when the Rack shows a rack's layer: back to the rack
    pub(super) fn rack_back_bar(&mut self, ui: &mut egui::Ui, idx: usize) {
        let Some((r, rname, k)) = self.with(|s| {
            let r = s.song.rack_of(idx)?;
            let ri = s.song.instruments.get(r)?;
            let k = match &ri.kind {
                InstrumentKind::Rack(rp) => rp.layers.iter().position(|l| l.slot == idx)?,
                _ => return None,
            };
            Some((r, ri.name.clone(), k))
        }) else {
            return;
        };
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            if ui
                .small_button(format!("‹ Back to {rname}"))
                .on_hover_text("the rack this layer plays in")
                .clicked()
            {
                self.instr = (r + 1) as u8;
                self.dev.pick = Pick::Inst;
            }
            ui.label(
                egui::RichText::new(format!("layer {} of {rname}", (b'A' + k as u8) as char))
                    .color(th().ink2),
            );
        });
    }
}
