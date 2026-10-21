//! Nested Device Chains (docs/racks/PLAN.md, requested by Del (postmanpat)):
//! the lower panel's device view grown into a tree. Left: the selected
//! instrument as a tree (its effects in order; later its layers and the
//! paths of a Split). Right: whatever row is selected, with its real
//! controls (the instrument card and Open full editor, or a device's card
//! with every parameter). The 0.2.4 card row stays as Cards (D1).
//!
//! Every edit goes through `apply_fx_ops`, so undo, automation lanes and
//! the engine's chain rebuild work as they did for the card row.
//! Original ChipWave code.
use super::fxview::FxOp;
use super::*;
use crate::fx::{self, DevPath, Device, FxKind};

/// one row of the tree
pub(crate) const ROW: f32 = 21.0;
/// indent per level
const INDENT: f32 = 14.0;
/// the folded tree's width
pub(crate) const SLIM_TREE: f32 = 150.0;

/// what is selected in the tree
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Sel {
    /// an instrument slot (0-based)
    Inst(usize),
    /// a device of instrument `.0`'s chain
    Dev(usize, DevPath),
    /// path `.2` of the Split at `.1`
    Path(usize, DevPath, usize),
    /// layer `.1` of rack `.0`
    Layer(usize, usize),
}

/// where Add Device puts the new device
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AddAt {
    pub(crate) inst: usize,
    /// the chain: `[]` the instrument's, `[i, p]` path p of Split i, ...
    pub(crate) chain: Vec<u8>,
    /// index in that chain (usize::MAX = the end)
    pub(crate) at: usize,
    pub(crate) query: String,
    pub(crate) pick: Option<AddPick>,
    pub(crate) focus: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AddPick {
    Kind(FxKind),
    Plugin(crate::plugin::PluginPatch),
}

/// `op` addressed into the chain at `prefix` (`[]` = the instrument's)
pub(crate) fn wrap(prefix: &[u8], op: FxOp) -> FxOp {
    match prefix {
        [i, p, rest @ ..] => FxOp::In(*i as usize, *p as usize, Box::new(wrap(rest, op))),
        _ => op,
    }
}

/// what a row shows
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RowKind {
    /// an instrument (the root)
    Inst(usize),
    /// "Effects": the heading of instrument `.0`'s chain
    Effects(usize),
    /// a device
    Dev(usize, DevPath),
    /// path `.2` of the Split at `.1` (instrument `.0`)
    Path(usize, DevPath, usize),
    /// "Layers": the heading of rack `.0`'s layers
    Layers(usize),
    /// layer `.1` of rack `.0`
    Layer(usize, usize),
    /// a quiet line ("none yet")
    Note(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) depth: u8,
    pub(crate) kind: RowKind,
    /// remembered open / closed state key ("" = cannot fold)
    pub(crate) key: String,
    pub(crate) open: bool,
}

/// the fold key of a chain heading / Split / layer
fn fold_key(kind: &RowKind) -> String {
    match kind {
        RowKind::Inst(i) => format!("i{i}"),
        RowKind::Effects(i) => format!("e{i}"),
        RowKind::Dev(i, p) => format!("d{i}.{:?}", p.as_slice()),
        RowKind::Path(i, sp, p) => format!("p{i}.{:?}.{p}", sp.as_slice()),
        RowKind::Layers(i) => format!("ls{i}"),
        RowKind::Layer(i, k) => format!("l{i}.{k}"),
        RowKind::Note(_) => String::new(),
    }
}

/// The rows of the tree for instrument `root`, with the rows the user
/// closed (`closed`, fold keys) folded. Pure: tests build it from a song.
pub(crate) fn tree_rows(song: &Song, root: usize, closed: &[String]) -> Vec<Row> {
    let mut out = vec![];
    let Some(_inst) = song.instruments.get(root) else {
        return out;
    };
    let push = |out: &mut Vec<Row>, depth: u8, kind: RowKind, foldable: bool| -> bool {
        let key = if foldable {
            fold_key(&kind)
        } else {
            String::new()
        };
        let open = !foldable || !closed.contains(&key);
        out.push(Row {
            depth,
            kind,
            key,
            open,
        });
        open
    };
    if !push(&mut out, 0, RowKind::Inst(root), true) {
        return out;
    }
    // a rack: its layers (each with its own effects), then the rack's effects
    if let InstrumentKind::Rack(r) = &_inst.kind {
        if push(&mut out, 1, RowKind::Layers(root), true) {
            if r.layers.is_empty() {
                push(
                    &mut out,
                    2,
                    RowKind::Note("no layers yet: + Add layer".into()),
                    false,
                );
            }
            for (k, l) in r.layers.iter().enumerate() {
                if push(&mut out, 2, RowKind::Layer(root, k), true)
                    && song.instruments.get(l.slot).is_some()
                {
                    chain_rows(song, l.slot, &mut out, 3, closed);
                }
            }
        }
    }
    chain_rows(song, root, &mut out, 1, closed);
    out
}

/// the instruments a rack's tree shows: the rack and its layer slots
pub(crate) fn in_tree(song: &Song, root: usize, i: usize) -> bool {
    i == root
        || matches!(&song.instruments.get(root).map(|x| &x.kind),
            Some(InstrumentKind::Rack(r)) if r.layers.iter().any(|l| l.slot == i))
}

/// layer `k` of rack `rack`
pub(crate) fn layer_of(song: &Song, rack: usize, k: usize) -> Option<&crate::model::Layer> {
    match &song.instruments.get(rack)?.kind {
        InstrumentKind::Rack(r) => r.layers.get(k),
        _ => None,
    }
}

/// "Effects" and one row per device of instrument `idx`'s own chain
fn chain_rows(song: &Song, idx: usize, out: &mut Vec<Row>, depth: u8, closed: &[String]) {
    let Some(inst) = song.instruments.get(idx) else {
        return;
    };
    let kind = RowKind::Effects(idx);
    let key = fold_key(&kind);
    let open = !closed.contains(&key);
    out.push(Row {
        depth,
        kind,
        key,
        open,
    });
    if !open {
        return;
    }
    if inst.fx.is_empty() {
        out.push(Row {
            depth: depth + 1,
            kind: RowKind::Note(
                "none yet: + Add device, or drag one from Browser › Effects".into(),
            ),
            key: String::new(),
            open: true,
        });
    }
    for (i, d) in inst.fx.iter().enumerate() {
        dev_rows(idx, d, DevPath::top(i), out, depth + 1, closed);
    }
}

/// a device's row, and for a Split its paths and their devices
fn dev_rows(idx: usize, d: &Device, at: DevPath, out: &mut Vec<Row>, depth: u8, closed: &[String]) {
    let kind = RowKind::Dev(idx, at);
    let split = d.kind == FxKind::Split;
    let key = if split {
        fold_key(&kind)
    } else {
        String::new()
    };
    let open = !split || !closed.contains(&key);
    out.push(Row {
        depth,
        kind,
        key,
        open,
    });
    if !split || !open {
        return;
    }
    for (p, path) in d.paths.iter().enumerate() {
        let kind = RowKind::Path(idx, at, p);
        let key = fold_key(&kind);
        let open = !closed.contains(&key);
        out.push(Row {
            depth: depth + 1,
            kind,
            key,
            open,
        });
        if !open {
            continue;
        }
        if path.devices.is_empty() {
            out.push(Row {
                depth: depth + 2,
                kind: RowKind::Note("dry: the sound as it comes in".into()),
                key: String::new(),
                open: true,
            });
        }
        for (j, x) in path.devices.iter().enumerate() {
            if let Some(c) = at.child(p, j) {
                dev_rows(idx, x, c, out, depth + 2, closed);
            }
        }
    }
}

/// the device a selection points at
pub(crate) fn sel_device(song: &Song, sel: Sel) -> Option<&Device> {
    match sel {
        Sel::Dev(i, p) | Sel::Path(i, p, _) => fx::dev_at(&song.instruments.get(i)?.fx, &p),
        Sel::Inst(_) | Sel::Layer(..) => None,
    }
}

/// "Effects › Split › Path B" for the chain `prefix` of `fx`
pub(crate) fn chain_crumb(fx: &[Device], prefix: &[u8]) -> String {
    let mut out = vec!["Effects".to_string()];
    let mut c = fx;
    let mut rest = prefix;
    while let [i, p, more @ ..] = rest {
        let Some(d) = c.get(*i as usize) else { break };
        out.push(d.title());
        out.push(format!("Path {}", super::fxview::path_letter(*p as usize)));
        match d.paths.get(*p as usize) {
            Some(x) => c = &x.devices,
            None => break,
        }
        rest = more;
    }
    out.join(" › ")
}

/// "Instrument › EQ" style breadcrumb for the detail header
pub(crate) fn sel_crumb(song: &Song, sel: Sel) -> String {
    let name = |i: usize| {
        song.instruments
            .get(i)
            .map(|x| format!("{:02X} {}", i + 1, x.name))
            .unwrap_or_default()
    };
    match sel {
        Sel::Inst(i) => name(i),
        Sel::Layer(r, k) => format!(
            "{} › Layer {} › {}",
            name(r),
            super::rackops::layer_letter(k),
            layer_of(song, r, k)
                .map(|l| name(l.slot))
                .unwrap_or_default()
        ),
        Sel::Dev(i, p) => {
            let fxl = &song.instruments[i].fx;
            let d = sel_device(song, sel).map(|d| d.title()).unwrap_or_default();
            format!(
                "{} › {} › {d}  (device {} of {})",
                name(i),
                chain_crumb(fxl, p.chain()),
                p.index() + 1,
                fx::chain_at(fxl, p.chain()).map_or(0, |c| c.len())
            )
        }
        Sel::Path(i, sp, p) => {
            let fxl = &song.instruments[i].fx;
            let mut pre = sp.as_slice().to_vec();
            pre.push(p as u8);
            format!("{} › {}", name(i), chain_crumb(fxl, &pre))
        }
    }
}

impl App {
    /// the instrument the tree shows: the current one
    pub(crate) fn chains_root(&self) -> Option<usize> {
        let n = self.with(|s| s.song.instruments.len());
        let i = (self.instr as usize).wrapping_sub(1);
        if i >= n {
            return None;
        }
        // a layer opens its rack's tree
        Some(self.with(|s| s.song.rack_of(i)).unwrap_or(i))
    }

    /// a selection that no longer exists (undo, removed device, another
    /// instrument) falls back to the root instrument
    fn chains_fix_sel(&mut self, root: usize) {
        let ok = match self.dev.sel {
            Some(Sel::Inst(i)) => i == root,
            Some(Sel::Layer(r, k)) => r == root && self.with(|x| layer_of(&x.song, r, k).is_some()),
            Some(s @ Sel::Dev(i, _)) => {
                self.with(|x| in_tree(&x.song, root, i) && sel_device(&x.song, s).is_some())
            }
            Some(s @ Sel::Path(i, _, p)) => self.with(|x| {
                in_tree(&x.song, root, i)
                    && sel_device(&x.song, s).is_some_and(|d| p < d.paths.len())
            }),
            None => false,
        };
        if !ok {
            // a layer picked in the Instruments list: its row in the rack
            let cur = (self.instr as usize).wrapping_sub(1);
            let k = self.with(|x| match &x.song.instruments.get(root).map(|i| &i.kind) {
                Some(InstrumentKind::Rack(r)) if cur != root => {
                    r.layers.iter().position(|l| l.slot == cur)
                }
                _ => None,
            });
            self.dev.sel = Some(match k {
                Some(k) => Sel::Layer(root, k),
                None => Sel::Inst(root),
            });
        }
    }

    /// the Nested Device Chains view (the lower panel's tab)
    pub(super) fn chains_view(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let Some(root) = self.chains_root() else {
            self.dev.row = ui.max_rect();
            self.dev.inst = Rect::NOTHING;
            self.dev.cards.clear();
            self.dev.tree_rows.clear();
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("This track has no instrument yet").strong());
                ui.label(
                    egui::RichText::new(
                        "Choose one above, drag one in from the Browser, or pick a track that plays one",
                    )
                    .color(t.ink2),
                );
                ui.menu_button("New instrument", |ui| self.new_instrument_menu(ui));
            });
            return;
        };
        self.chains_fix_sel(root);
        let sel = self.dev.sel.unwrap_or(Sel::Inst(root));
        let mut ops: Vec<(usize, FxOp)> = vec![];
        // ---- header: what is shown, then the panel's actions
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let (name, tag) = self.with(|s| {
                let i = &s.song.instruments[root];
                (i.name.clone(), params::type_tag(&i.kind).to_string())
            });
            let track = self.with(|s| s.song.channel_names.get(self.ch).cloned());
            let lead = match track {
                Some(tn) if self.track_slot(self.ch) as usize == root + 1 => {
                    format!("Track {:02} {tn} › ", self.ch + 1)
                }
                _ => String::new(),
            };
            ui.label(egui::RichText::new(lead).color(t.ink2));
            ui.label(egui::RichText::new(format!("{:02X} {name}", root + 1)).strong());
            ui.label(egui::RichText::new(tag).small().color(t.ink2));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                if ui
                    .button("+ Add Device")
                    .on_hover_text(
                        "a ChipWave effect, a Split or a plugin effect: after the selected device, or at the end of the selected chain",
                    )
                    .clicked()
                {
                    self.add_device_here();
                }
                if ui
                    .button("Cards")
                    .on_hover_text(
                        "the chain as a row of cards, left to right (the 0.2.4 view); Tree comes back from there",
                    )
                    .clicked()
                {
                    self.extra.chains_cards = true;
                    self.save_settings();
                }
                let fold = if self.extra.chains_tree_folded {
                    "Unfold tree"
                } else {
                    "Fold tree"
                };
                if ui
                    .button(fold)
                    .on_hover_text("the tree column as a slim list of names, or full width again")
                    .clicked()
                {
                    self.extra.chains_tree_folded = !self.extra.chains_tree_folded;
                    self.save_settings();
                }
            });
        });
        let body = ui.available_rect_before_wrap();
        let tw = if self.extra.chains_tree_folded {
            SLIM_TREE
        } else {
            self.extra
                .chains_tree_w
                .clamp(220.0, (body.width() * 0.55).max(220.0))
        };
        let tree_r = Rect::from_min_size(body.min, egui::vec2(tw, body.height()));
        let handle = Rect::from_min_size(
            Pos2::new(tree_r.right(), body.top()),
            egui::vec2(6.0, body.height()),
        );
        let detail_r = Rect::from_min_max(Pos2::new(handle.right() + 2.0, body.top()), body.max);
        // tree column
        ui.scope_builder(egui::UiBuilder::new().max_rect(tree_r), |ui| {
            self.chains_tree(ui, root, sel, &mut ops)
        });
        // the resize handle between tree and detail
        let hr = ui
            .interact(handle, ui.make_persistent_id("chains_split"), Sense::drag())
            .on_hover_text("drag to make the tree wider or narrower");
        if hr.hovered() || hr.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if hr.dragged() {
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                self.extra.chains_tree_folded = false;
                self.extra.chains_tree_w = (p.x - tree_r.left()).clamp(220.0, 700.0);
            }
        }
        if hr.drag_stopped() {
            self.save_settings();
        }
        ui.painter().vline(
            handle.center().x,
            handle.y_range().shrink(4.0),
            Stroke::new(
                1.0,
                if hr.hovered() || hr.dragged() {
                    t.accent
                } else {
                    t.border.gamma_multiply(0.6)
                },
            ),
        );
        // detail
        ui.scope_builder(egui::UiBuilder::new().max_rect(detail_r), |ui| {
            ui.set_clip_rect(detail_r.intersect(ui.clip_rect()));
            self.chains_detail(ui, root, sel, &mut ops)
        });
        // Browser drops: on the tree's effect rows (vertical marker)
        self.dev.row = body;
        self.dev.vertical = true;
        let mut by_inst: Vec<(usize, Vec<FxOp>)> = vec![];
        for (i, op) in ops {
            match by_inst.last_mut() {
                Some((j, v)) if *j == i => v.push(op),
                _ => by_inst.push((i, vec![op])),
            }
        }
        for (i, v) in by_inst {
            self.apply_fx_ops(i, v);
        }
        if let Some(m) = self.dev.tree_drop.take() {
            self.chains_drop(m);
        }
        let ctx = ui.ctx().clone();
        self.add_device_window(&ctx);
    }

    /// the tree column
    fn chains_tree(
        &mut self,
        ui: &mut egui::Ui,
        root: usize,
        sel: Sel,
        ops: &mut Vec<(usize, FxOp)>,
    ) {
        let t = th();
        let rows = self.with(|s| tree_rows(&s.song, root, &self.extra.chains_closed));
        let slim = self.extra.chains_tree_folded;
        let mut toggle: Option<String> = None;
        let mut pick: Option<Sel> = None;
        let mut add: Option<(usize, Vec<u8>, usize)> = None;
        let mut add_layer: Option<usize> = None;
        let mut layer_set: Option<(usize, usize, crate::model::Layer)> = None;
        let (is_rack, n_layers) =
            self.with(|s| match &s.song.instruments.get(root).map(|i| &i.kind) {
                Some(InstrumentKind::Rack(r)) => (true, r.layers.len()),
                _ => (false, 0),
            });
        let mut drawn: Vec<(Rect, RowKind)> = vec![];
        let pointer = ui.ctx().pointer_latest_pos();
        let released = ui.input(|i| i.pointer.any_released());
        let ctrl = ui.input(|i| i.modifiers.command);
        ui.painter()
            .rect_filled(ui.max_rect(), 4.0, t.ink2.gamma_multiply(0.05));
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("CHAINS").small().strong().color(t.ink2));
            if !slim {
                ui.label(
                    egui::RichText::new(
                        "drag rows to reorder · Ctrl-drop copies · right-click for more",
                    )
                    .small()
                    .color(t.ink2),
                );
            }
        });
        let link = |ui: &mut egui::Ui, r: Rect, id: egui::Id, text: &str, tip: &str| -> bool {
            let g = ui.painter().layout_no_wrap(
                text.to_string(),
                FontId::proportional(11.5),
                th().accent,
            );
            let b = Rect::from_min_size(
                Pos2::new(r.right() - g.size().x - 10.0, r.center().y - 8.0),
                egui::vec2(g.size().x + 6.0, 16.0),
            );
            let resp = ui.interact(b, id, Sense::click()).on_hover_text(tip);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, text));
            if resp.hovered() {
                ui.painter()
                    .rect_filled(b, 3.0, th().accent.gamma_multiply(0.15));
            }
            ui.painter()
                .galley(b.min + egui::vec2(3.0, 1.0), g, th().accent);
            resp.clicked()
        };
        egui::ScrollArea::vertical()
            .id_salt("chains_tree")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for row in &rows {
                    let (r, resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), ROW),
                        Sense::click_and_drag(),
                    );
                    let x0 = r.left() + 6.0 + row.depth as f32 * INDENT;
                    let this_sel = match &row.kind {
                        RowKind::Inst(i) => Some(Sel::Inst(*i)),
                        RowKind::Dev(i, p) => Some(Sel::Dev(*i, *p)),
                        RowKind::Path(i, sp, p) => Some(Sel::Path(*i, *sp, *p)),
                        RowKind::Layer(r, k) => Some(Sel::Layer(*r, *k)),
                        _ => None,
                    };
                    let is_sel = this_sel == Some(sel);
                    let p = ui.painter_at(r);
                    if is_sel {
                        p.rect_filled(r, 3.0, t.accent.gamma_multiply(0.22));
                        p.vline(r.left() + 1.0, r.y_range(), Stroke::new(2.0, t.accent));
                    } else if resp.hovered() {
                        p.rect_filled(r, 3.0, t.ink.gamma_multiply(0.06));
                    }
                    for k in 0..row.depth {
                        p.vline(
                            r.left() + 6.0 + k as f32 * INDENT + INDENT * 0.5,
                            r.y_range(),
                            Stroke::new(1.0, t.border.gamma_multiply(0.45)),
                        );
                    }
                    let mut x = x0;
                    if !row.key.is_empty() {
                        let a = Rect::from_min_size(Pos2::new(x, r.top()), egui::vec2(14.0, ROW));
                        let ar = ui
                            .interact(
                                a,
                                ui.make_persistent_id(("chains_fold", &row.key)),
                                Sense::click(),
                            )
                            .on_hover_text(if row.open { "fold" } else { "unfold" });
                        fold_arrow(
                            &p,
                            a.center(),
                            row.open,
                            if ar.hovered() { t.accent } else { t.ink2 },
                        );
                        if ar.clicked() {
                            toggle = Some(row.key.clone());
                        }
                    }
                    x += 15.0;
                    let font = FontId::proportional(12.5);
                    let small = FontId::proportional(11.0);
                    let after = |p: &egui::Painter, g: Rect, s: &str, c: Color32| {
                        if !slim && !s.is_empty() {
                            p.text(
                                Pos2::new(g.right() + 8.0, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                s,
                                FontId::proportional(11.0),
                                c,
                            );
                        }
                    };
                    match &row.kind {
                        RowKind::Inst(i) => {
                            let (name, tag) = self.with(|s| {
                                s.song
                                    .instruments
                                    .get(*i)
                                    .map(|x| {
                                        (x.name.clone(), params::type_tag(&x.kind).to_string())
                                    })
                                    .unwrap_or_default()
                            });
                            let g = p.text(
                                Pos2::new(x, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("{:02X} {name}", i + 1),
                                FontId::proportional(13.0),
                                t.acc_hot,
                            );
                            after(&p, g, &tag, t.ink2);
                        }
                        RowKind::Effects(i) => {
                            let rack = *i == root && is_rack;
                            let g = p.text(
                                Pos2::new(x, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                if rack { "Rack effects" } else { "Effects" },
                                font.clone(),
                                t.ink,
                            );
                            after(
                                &p,
                                g,
                                if rack {
                                    "after the layers are mixed"
                                } else if *i != root {
                                    "this layer only"
                                } else {
                                    "in order, after the instrument"
                                },
                                t.ink2,
                            );
                            if !slim
                                && link(
                                    ui,
                                    r,
                                    ui.make_persistent_id(("chains_add_e", *i)),
                                    "+ Add device",
                                    "Add Device: a ChipWave effect, a Split or a plugin effect at the end of this chain",
                                )
                            {
                                add = Some((*i, vec![], usize::MAX));
                            }
                        }
                        RowKind::Path(i, sp, pi) => {
                            let path = self.with(|s| {
                                sel_device(&s.song, Sel::Path(*i, *sp, *pi))
                                    .and_then(|d| d.paths.get(*pi).cloned())
                            });
                            let Some(path) = path else {
                                continue;
                            };
                            let dot = Rect::from_center_size(
                                Pos2::new(x + 5.0, r.center().y),
                                egui::vec2(14.0, 14.0),
                            );
                            let dr = ui
                                .interact(
                                    dot,
                                    ui.make_persistent_id(("chains_pon", *i, *sp, *pi)),
                                    Sense::click(),
                                )
                                .on_hover_text(if path.on {
                                    "on: click to mute this path"
                                } else {
                                    "muted: click to switch it on"
                                });
                            if path.on {
                                p.circle_filled(dot.center(), 4.5, t.accent);
                            } else {
                                p.circle_stroke(dot.center(), 4.0, Stroke::new(1.2, t.ink2));
                            }
                            if dr.clicked() {
                                ops.push((*i, wrap(sp.chain(), FxOp::PathOn(sp.index(), *pi))));
                            }
                            let g = p.text(
                                Pos2::new(x + 14.0, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("Path {}", super::fxview::path_letter(*pi)),
                                font.clone(),
                                if path.on { t.ink } else { t.ink2 },
                            );
                            after(&p, g, &path_summary(&path), t.ink2);
                            if !slim
                                && link(
                                    ui,
                                    r,
                                    ui.make_persistent_id(("chains_add_p", *i, *sp, *pi)),
                                    "+ Add device",
                                    "Add Device at the end of this path",
                                )
                            {
                                let mut c = sp.as_slice().to_vec();
                                c.push(*pi as u8);
                                add = Some((*i, c, usize::MAX));
                            }
                        }
                        RowKind::Dev(i, path) => {
                            let d =
                                self.with(|s| sel_device(&s.song, Sel::Dev(*i, *path)).cloned());
                            let Some(d) = d else {
                                continue;
                            };
                            let dot = Rect::from_center_size(
                                Pos2::new(x + 5.0, r.center().y),
                                egui::vec2(14.0, 14.0),
                            );
                            let dr = ui
                                .interact(
                                    dot,
                                    ui.make_persistent_id(("chains_pw", *i, *path)),
                                    Sense::click(),
                                )
                                .on_hover_text(if d.enabled {
                                    "on: click to bypass"
                                } else {
                                    "bypassed: click to switch it on"
                                });
                            if d.enabled {
                                p.circle_filled(dot.center(), 4.5, t.accent);
                            } else {
                                p.circle_stroke(dot.center(), 4.0, Stroke::new(1.2, t.ink2));
                            }
                            let k = path.index();
                            let pre = path.chain().to_vec();
                            if dr.clicked() {
                                ops.push((*i, wrap(&pre, FxOp::Toggle(k))));
                            }
                            let g = p.text(
                                Pos2::new(x + 14.0, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!("{}  {}", k + 1, d.title()),
                                font.clone(),
                                if d.enabled { t.ink } else { t.ink2 },
                            );
                            if !d.enabled {
                                after(&p, g, "bypassed", t.brick);
                            } else if d.kind == FxKind::Split {
                                after(&p, g, &format!("{} parallel paths", d.paths.len()), t.ink2);
                                if !slim
                                    && d.paths.len() < fx::MAX_PATHS
                                    && link(
                                        ui,
                                        r,
                                        ui.make_persistent_id(("chains_addpath", *i, *path)),
                                        "+ Add path",
                                        "one more parallel path (dry until you add devices; at most 4)",
                                    )
                                {
                                    ops.push((*i, wrap(&pre, FxOp::AddPath(k))));
                                }
                            }
                            if resp.drag_started() {
                                self.dev.tree_drag = Some(Sel::Dev(*i, *path));
                            }
                            resp.context_menu(|ui| {
                                ui.label(egui::RichText::new(d.title()).strong());
                                for (txt, op) in [
                                    (if d.enabled { "Bypass" } else { "Enable" }, FxOp::Toggle(k)),
                                    ("Duplicate", FxOp::Dup(k)),
                                    ("Move up", FxOp::Move(k, k.saturating_sub(1))),
                                    ("Move down", FxOp::Move(k, k + 2)),
                                    ("Remove", FxOp::Remove(k)),
                                ] {
                                    if ui.button(txt).clicked() {
                                        ops.push((*i, wrap(&pre, op)));
                                        ui.close();
                                    }
                                }
                                if ui.button("Add device after this…").clicked() {
                                    add = Some((*i, pre.clone(), k + 1));
                                    ui.close();
                                }
                            });
                        }
                        RowKind::Layers(rk) => {
                            let g = p.text(
                                Pos2::new(x, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                "Layers",
                                font.clone(),
                                t.ink,
                            );
                            after(&p, g, "each plays every note it takes, then they are mixed", t.ink2);
                            if !slim
                                && n_layers < crate::model::MAX_LAYERS
                                && link(
                                    ui,
                                    r,
                                    ui.make_persistent_id(("chains_add_l", *rk)),
                                    "+ Add layer",
                                    "a new synth layer (pick an instrument already in the song in the rack's detail)",
                                )
                            {
                                add_layer = Some(*rk);
                            }
                        }
                        RowKind::Layer(rk, k) => {
                            let info = self.with(|s| {
                                let l = layer_of(&s.song, *rk, *k)?.clone();
                                let x = s.song.instruments.get(l.slot)?;
                                Some((
                                    l,
                                    x.name.clone(),
                                    params::type_tag(&x.kind).to_string(),
                                    matches!(x.kind, InstrumentKind::Plugin(_)),
                                ))
                            });
                            let Some((l, name, tag, plug)) = info else {
                                continue;
                            };
                            let dot = Rect::from_center_size(
                                Pos2::new(x + 5.0, r.center().y),
                                egui::vec2(14.0, 14.0),
                            );
                            let dr = ui
                                .interact(
                                    dot,
                                    ui.make_persistent_id(("chains_lon", *rk, *k)),
                                    Sense::click(),
                                )
                                .on_hover_text(if l.on {
                                    "on: click to mute this layer"
                                } else {
                                    "muted: click to switch it on"
                                });
                            if l.on {
                                p.circle_filled(dot.center(), 4.5, t.accent);
                            } else {
                                p.circle_stroke(dot.center(), 4.0, Stroke::new(1.2, t.ink2));
                            }
                            if dr.clicked() {
                                let mut nl = l.clone();
                                nl.on = !nl.on;
                                layer_set = Some((*rk, *k, nl));
                            }
                            let live = l.on && !plug;
                            let g = p.text(
                                Pos2::new(x + 14.0, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                format!(
                                    "Layer {}  {:02X} {name}",
                                    super::rackops::layer_letter(*k),
                                    l.slot + 1
                                ),
                                font.clone(),
                                if live { t.ink } else { t.ink2 },
                            );
                            let note = if plug {
                                "plugin instruments can't play in a rack yet".to_string()
                            } else if !l.on {
                                "muted".to_string()
                            } else {
                                layer_summary(&l, &tag)
                            };
                            after(&p, g, &note, if plug || !l.on { t.brick } else { t.ink2 });
                        }
                        RowKind::Note(s) => {
                            p.text(
                                Pos2::new(x, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                s,
                                small.clone(),
                                t.ink2,
                            );
                        }
                    }
                    if let Some(s) = this_sel {
                        if resp.clicked() {
                            pick = Some(s);
                        }
                    } else if resp.clicked() && !row.key.is_empty() {
                        toggle = Some(row.key.clone());
                    }
                    drawn.push((r, row.kind.clone()));
                }
            });
        // drag a device row: a marker where it lands (any chain it may go
        // in), then the move; Ctrl held on the drop copies it
        if let Some(Sel::Dev(i, from)) = self.dev.tree_drag {
            let target = pointer.and_then(|p| drop_target(&drawn, p));
            if let Some((y, (ti, chain, at))) = target {
                let r = ui.max_rect();
                super::devices::paint_marker_h(ui.painter(), y, r.x_range().shrink(8.0));
                ui.ctx().set_cursor_icon(if ctrl {
                    egui::CursorIcon::Copy
                } else {
                    egui::CursorIcon::Grabbing
                });
                if released {
                    self.dev.tree_drop = Some(TreeDrop {
                        from: (i, from),
                        to: (ti, chain, at),
                        copy: ctrl,
                    });
                }
            }
            if released {
                self.dev.tree_drag = None;
            }
        } else if released {
            self.dev.tree_drag = None;
        }
        // for Browser drops: the instrument row and the effect rows
        self.dev.inst = drawn
            .iter()
            .find(|(_, k)| matches!(k, RowKind::Inst(i) if *i == root))
            .map_or(Rect::NOTHING, |(r, _)| *r);
        self.dev.cards = drawn
            .iter()
            .filter(|(_, k)| matches!(k, RowKind::Dev(i, p) if *i == root && p.depth() == 0))
            .map(|(r, _)| *r)
            .collect();
        self.dev.tree_rows = drawn;
        if let Some(k) = toggle {
            if let Some(at) = self.extra.chains_closed.iter().position(|x| *x == k) {
                self.extra.chains_closed.remove(at);
            } else {
                self.extra.chains_closed.push(k);
            }
            self.save_settings();
        }
        if let Some(s) = pick {
            self.dev.sel = Some(s);
        }
        if let Some((i, chain, at)) = add {
            self.open_add_device(i, chain, at);
        }
        if let Some(r) = add_layer {
            self.add_layer(r, None);
        }
        if let Some((r, k, l)) = layer_set {
            self.set_layer(r, k, l, true);
        }
    }

    /// Add Device for the chain `chain` of instrument `inst`, at `at`
    pub(crate) fn open_add_device(&mut self, inst: usize, chain: Vec<u8>, at: usize) {
        self.dev.add = Some(AddAt {
            inst,
            chain,
            at,
            query: String::new(),
            pick: Some(AddPick::Kind(FxKind::Eq)),
            focus: true,
        });
    }

    /// where the header's + Add Device puts a device: after the selected
    /// device, at the end of a selected path, else at the end of the chain
    pub(crate) fn add_device_here(&mut self) {
        let Some(root) = self.chains_root() else {
            return;
        };
        match self.dev.sel {
            Some(Sel::Dev(i, p)) => self.open_add_device(i, p.chain().to_vec(), p.index() + 1),
            Some(Sel::Path(i, sp, p)) => {
                let mut c = sp.as_slice().to_vec();
                c.push(p as u8);
                self.open_add_device(i, c, usize::MAX)
            }
            Some(Sel::Inst(i)) => self.open_add_device(i, vec![], usize::MAX),
            // a layer: the end of that layer's own chain
            Some(Sel::Layer(r, k)) => {
                let slot = self.with(|s| layer_of(&s.song, r, k).map(|l| l.slot));
                self.open_add_device(slot.unwrap_or(r), vec![], usize::MAX)
            }
            None => self.open_add_device(root, vec![], usize::MAX),
        }
    }

    /// the detail area: the selected row's controls
    fn chains_detail(
        &mut self,
        ui: &mut egui::Ui,
        root: usize,
        sel: Sel,
        ops: &mut Vec<(usize, FxOp)>,
    ) {
        let t = th();
        let crumb = self.with(|s| sel_crumb(&s.song, sel));
        match sel {
            Sel::Layer(r, k) => {
                self.layer_detail(ui, r, k, crumb);
            }
            Sel::Inst(i) => {
                let (rack, no_rack) = self.with(|s| {
                    (
                        matches!(s.song.instruments[i].kind, InstrumentKind::Rack(_)),
                        super::rackops::no_rack_reason(&s.song, i),
                    )
                });
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(egui::RichText::new(crumb).strong()).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !rack
                            && ui
                                .button("Open full editor  Ctrl+I")
                                .on_hover_text("every control of this instrument (the Instrument tab)")
                                .clicked()
                        {
                            self.instr = (i + 1) as u8;
                            self.set_lower(1);
                        }
                        if rack {
                            self.rack_add_layer_buttons(ui, i);
                        } else if ui
                            .add_enabled(no_rack.is_none(), egui::Button::new("Make a rack from this instrument"))
                            .on_hover_text(
                                "this sound becomes layer A of a rack in the same slot (it sounds the same); then add more layers",
                            )
                            .on_disabled_hover_text(no_rack.unwrap_or(""))
                            .clicked()
                        {
                            self.make_rack(i);
                        }
                    });
                });
                let h = (ui.available_height() - 6.0).max(120.0);
                let w = super::devcards::CARD_W.min(ui.available_width() - 8.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0);
                    let keep = self.dev.card_w;
                    self.dev.card_w = w;
                    let fold = self.dev.inst_folded;
                    self.dev.inst_folded = false;
                    self.devices_inst_card(ui, h, i);
                    self.dev.inst_folded = fold;
                    self.dev.card_w = keep;
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 8.0).max(120.0));
                        self.chains_flow(ui, root);
                    });
                });
            }
            Sel::Path(i, sp, p) => {
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(egui::RichText::new(crumb).strong()).truncate());
                });
                ui.add_space(4.0);
                if let Some(d) = self.with(|s| sel_device(&s.song, sel).cloned()) {
                    let mut local = vec![];
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            self.split_path_controls(ui, i, &d, p, &mut local);
                        });
                    });
                    for op in local {
                        ops.push((i, wrap(sp.chain(), op_at(sp.index(), op))));
                    }
                }
            }
            Sel::Dev(i, path) => {
                let Some(d) = self.with(|s| sel_device(&s.song, sel).cloned()) else {
                    return;
                };
                let k = path.index();
                let pre = path.chain().to_vec();
                let nested = path.depth() > 0;
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(egui::RichText::new(crumb).strong()).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Remove")
                            .on_hover_text("remove (Ctrl+Z brings it back)")
                            .clicked()
                        {
                            ops.push((i, wrap(&pre, FxOp::Remove(k))));
                        }
                        if ui
                            .button("Duplicate")
                            .on_hover_text("a copy right after it")
                            .clicked()
                        {
                            ops.push((i, wrap(&pre, FxOp::Dup(k))));
                        }
                        let b = if d.enabled { "Bypass" } else { "Enable" };
                        if ui
                            .button(b)
                            .on_hover_text("bypass: the sound passes it unchanged")
                            .clicked()
                        {
                            ops.push((i, wrap(&pre, FxOp::Toggle(k))));
                        }
                    });
                });
                let h = (ui.available_height() - 6.0).max(120.0);
                let n = self.with(|s| {
                    fx::chain_at(&s.song.instruments[i].fx, path.chain()).map_or(0, |c| c.len())
                });
                let mut local: Vec<FxOp> = vec![];
                let mut moves: Vec<TreeDrop> = vec![];
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0);
                    let keep = self.dev.card_w;
                    self.dev.card_w = super::devcards::CARD_W.min(ui.available_width() * 0.42);
                    let mut card = d.clone();
                    card.collapsed = false;
                    let siblings = self.with(|s| {
                        fx::chain_at(&s.song.instruments[i].fx, path.chain())
                            .map(|c| c.to_vec())
                            .unwrap_or_default()
                    });
                    let key = super::devcards::open_key(&siblings, k);
                    // the card without its own "…" column: All parameters is beside it
                    self.dev.open.remove(&(i, key));
                    self.dev.no_auto = nested;
                    self.devices_card(ui, i, k, &card, key, h, n, &mut local);
                    self.dev.card_w = keep;
                    ui.add_space(10.0);
                    let rest = Rect::from_min_max(
                        ui.cursor().min,
                        Pos2::new(ui.max_rect().right() - 6.0, ui.cursor().min.y + h),
                    );
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rest), |ui| {
                        super::devcards::well(ui.painter(), rest);
                        let inner = rest.shrink(8.0);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            if d.kind == FxKind::Split {
                                ui.label(
                                    egui::RichText::new("PATHS").small().strong().color(t.ink2),
                                );
                                egui::ScrollArea::vertical()
                                    .id_salt(("split_paths", i, path))
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        for p in 0..d.paths.len() {
                                            self.split_path_controls(ui, i, &d, p, &mut local);
                                            ui.separator();
                                        }
                                        if d.paths.len() < fx::MAX_PATHS
                                            && ui
                                                .button("+ Add path")
                                                .on_hover_text("one more parallel path (at most 4)")
                                                .clicked()
                                        {
                                            local.push(FxOp::AddPath(k));
                                        }
                                    });
                                return;
                            }
                            let more_h = (inner.height() - 64.0).max(80.0);
                            ui.label(
                                egui::RichText::new("ALL PARAMETERS").small().strong().color(t.ink2),
                            );
                            ui.allocate_ui(egui::vec2(inner.width(), more_h), |ui| {
                                self.devices_more(ui, i, k, &card, &mut local);
                            });
                            if nested {
                                ui.label(
                                    egui::RichText::new(
                                        "Automation lanes: devices in the main chain and layer chains only for now",
                                    )
                                    .small()
                                    .color(t.ink2),
                                );
                            }
                            self.move_or_copy(ui, i, path, &d, &mut moves);
                        });
                    });
                });
                self.dev.no_auto = false;
                if self.dev.drag.is_some() && ui.input(|x| x.pointer.any_released()) {
                    self.dev.drag = None;
                }
                for op in local {
                    // (the card's own collapse means nothing here)
                    if matches!(op, FxOp::Collapse(_)) {
                        continue;
                    }
                    let op = match op {
                        // the card addresses the device by its index in its chain
                        FxOp::AddPath(_) => FxOp::AddPath(k),
                        o => o,
                    };
                    ops.push((i, wrap(&pre, op)));
                }
                if let Some(m) = moves.pop() {
                    self.dev.tree_drop = Some(m);
                }
            }
        }
    }

    /// one path of Split `d`: on / off, level, pan, remove (ops addressed
    /// to the Split by its own index, wrapped by the caller)
    fn split_path_controls(
        &mut self,
        ui: &mut egui::Ui,
        inst: usize,
        d: &Device,
        p: usize,
        local: &mut Vec<FxOp>,
    ) {
        let Some(path) = d.paths.get(p) else {
            return;
        };
        // the Split's own index is filled in by the caller (op_at)
        let k = usize::MAX;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Path {}", super::fxview::path_letter(p))).strong(),
            );
            ui.label(
                egui::RichText::new(path_summary(path))
                    .small()
                    .color(th().ink2),
            );
        });
        ui.horizontal(|ui| {
            let mut on = path.on;
            if ui
                .checkbox(&mut on, "On")
                .on_hover_text("a muted path adds nothing to the sound")
                .changed()
            {
                local.push(FxOp::PathOn(k, p));
            }
            let mut lv = path.level_db;
            let r = ui
                .add(
                    egui::Slider::new(&mut lv, -60.0..=6.0)
                        .text("Level")
                        .suffix(" dB")
                        .fixed_decimals(1)
                        // only edits round: showing -6.02 dB must not change it
                        .clamping(egui::SliderClamping::Edits),
                )
                .on_hover_text(
                    "this path's level in the mix (0 dB = as it comes out of its devices)",
                );
            if r.changed() {
                local.push(FxOp::PathLevel(k, p, lv, r.drag_started() || !r.dragged()));
            }
        });
        ui.horizontal(|ui| {
            let mut pan = path.pan;
            let r = ui
                .add(
                    egui::Slider::new(&mut pan, -1.0..=1.0)
                        .text("Pan")
                        .fixed_decimals(2)
                        .clamping(egui::SliderClamping::Edits),
                )
                .on_hover_text("left -1 · centre 0 · right +1");
            if r.changed() {
                local.push(FxOp::PathPan(k, p, pan, r.drag_started() || !r.dragged()));
            }
            if d.paths.len() > 2
                && ui
                    .button("Remove path")
                    .on_hover_text("this path and its devices go (Ctrl+Z brings them back)")
                    .clicked()
            {
                local.push(FxOp::RemovePath(k, p));
            }
        });
        let _ = inst;
    }

    /// "+ Add layer" and "Add an instrument as a layer" for rack `r`
    fn rack_add_layer_buttons(&mut self, ui: &mut egui::Ui, r: usize) {
        let (n, cands) = self.with(|s| {
            let n = layer_count(&s.song, r);
            let c: Vec<(usize, String, Option<&'static str>)> = s
                .song
                .instruments
                .iter()
                .enumerate()
                .filter(|(k, _)| *k != r)
                .map(|(k, x)| {
                    (
                        k,
                        format!("{:02X} {}", k + 1, x.name),
                        super::rackops::no_layer_reason(&s.song, r, k),
                    )
                })
                .collect();
            (n, c)
        });
        let full = n >= crate::model::MAX_LAYERS;
        ui.add_enabled_ui(!full, |ui| {
            ui.menu_button("Add an instrument as a layer", |ui| {
                ui.label(
                    egui::RichText::new("it keeps its slot; the rack plays it too")
                        .small()
                        .color(th().ink2),
                );
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (k, name, why) in &cands {
                            if ui
                                .add_enabled(why.is_none(), egui::Button::new(name))
                                .on_disabled_hover_text(why.unwrap_or(""))
                                .clicked()
                            {
                                self.add_layer(r, Some(*k));
                                ui.close();
                            }
                        }
                    });
            })
            .response
            .on_disabled_hover_text("a rack has at most 4 layers");
            if ui
                .button("+ Add layer")
                .on_hover_text("a new synth layer (at most 4)")
                .on_disabled_hover_text("a rack has at most 4 layers")
                .clicked()
            {
                self.add_layer(r, None);
            }
        });
    }

    /// layer `k` of rack `r`: its settings (mockup 1), its instrument card
    fn layer_detail(&mut self, ui: &mut egui::Ui, r: usize, k: usize, crumb: String) {
        let t = th();
        let Some((l, plug)) = self.with(|s| {
            let l = layer_of(&s.song, r, k)?.clone();
            let plug = matches!(
                s.song.instruments.get(l.slot).map(|x| &x.kind),
                Some(InstrumentKind::Plugin(_))
            );
            Some((l, plug))
        }) else {
            return;
        };
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.add(egui::Label::new(egui::RichText::new(crumb).strong()).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("Remove layer")
                    .on_hover_text("out of the rack; the instrument stays in the song (Ctrl+Z brings the layer back)")
                    .clicked()
                {
                    self.remove_layer(r, k);
                }
                if ui
                    .button("Open full editor  Ctrl+I")
                    .on_hover_text("every control of this layer's instrument (the Instrument tab)")
                    .clicked()
                {
                    self.instr = (l.slot + 1) as u8;
                    self.set_lower(1);
                }
            });
        });
        if plug {
            ui.label(
                egui::RichText::new(
                    "Plugin instruments can't play in a rack yet: this layer is silent. Use it on a track of its own.",
                )
                .color(t.brick),
            );
        }
        let h = (ui.available_height() - 6.0).max(120.0);
        let w = super::devcards::CARD_W.min(ui.available_width() * 0.5);
        let mut nl = l.clone();
        let mut step = false;
        let mut changed = false;
        ui.horizontal_top(|ui| {
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - w - 16.0).clamp(200.0, 330.0));
                ui.label(egui::RichText::new("LAYER").small().strong().color(t.ink2));
                if ui
                    .checkbox(&mut nl.on, "On")
                    .on_hover_text("a muted layer plays nothing")
                    .changed()
                {
                    changed = true;
                    step = true;
                }
                let mut slider = |ui: &mut egui::Ui, s: egui::Slider, tip: &str| {
                    let r = ui.add(s.clamping(egui::SliderClamping::Edits)).on_hover_text(tip);
                    if r.changed() {
                        changed = true;
                        step |= r.drag_started() || !r.dragged();
                    }
                };
                slider(
                    ui,
                    egui::Slider::new(&mut nl.level_db, -60.0..=6.0)
                        .text("Level")
                        .suffix(" dB")
                        .fixed_decimals(1),
                    "this layer's level in the rack (0 dB = as its instrument sounds)",
                );
                slider(
                    ui,
                    egui::Slider::new(&mut nl.pan, -1.0..=1.0).text("Pan").fixed_decimals(2),
                    "left -1 · centre 0 · right +1",
                );
                slider(
                    ui,
                    egui::Slider::new(&mut nl.transpose, -48..=48)
                        .text("Transpose")
                        .suffix(" st"),
                    "semitones, on top of the rack's and the instrument's own transpose",
                );
                ui.label(egui::RichText::new("KEY RANGE").small().strong().color(t.ink2));
                let mut lo = nl.key_lo.max(1);
                let mut hi = nl.key_hi.max(1);
                slider(
                    ui,
                    egui::Slider::new(&mut lo, 1..=127)
                        .text("Lowest")
                        .custom_formatter(|v, _| crate::model::note_name(v as u8)),
                    "the lowest note this layer plays",
                );
                slider(
                    ui,
                    egui::Slider::new(&mut hi, 1..=127)
                        .text("Highest")
                        .custom_formatter(|v, _| crate::model::note_name(v as u8)),
                    "the highest note this layer plays",
                );
                if lo != nl.key_lo.max(1) {
                    nl.key_lo = if lo <= 1 { 0 } else { lo };
                    nl.key_hi = nl.key_hi.max(nl.key_lo);
                }
                if hi != nl.key_hi.max(1) {
                    nl.key_hi = hi;
                    nl.key_lo = nl.key_lo.min(hi);
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Tracker blocks play layers with the note and its volume; slides, vibrato and arpeggio don't reach layers yet.",
                    )
                    .small()
                    .color(t.ink2),
                );
            });
            ui.add_space(10.0);
            let keep = self.dev.card_w;
            self.dev.card_w = w;
            let fold = self.dev.inst_folded;
            self.dev.inst_folded = false;
            self.devices_inst_card(ui, h, l.slot);
            self.dev.inst_folded = fold;
            self.dev.card_w = keep;
        });
        if changed && nl != l {
            self.set_layer(r, k, nl, step);
        }
    }

    /// "MOVE OR COPY": buttons for the other chains this device can go in
    fn move_or_copy(
        &mut self,
        ui: &mut egui::Ui,
        inst: usize,
        at: DevPath,
        d: &Device,
        moves: &mut Vec<TreeDrop>,
    ) {
        let t = th();
        let targets = self.with(|s| move_targets(&s.song, inst, at, d));
        if targets.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("MOVE OR COPY")
                .small()
                .strong()
                .color(t.ink2),
        )
        .on_hover_text("or drag the row in the tree; hold Ctrl while dropping to copy");
        let copy = ui.input(|i| i.modifiers.command);
        ui.horizontal_wrapped(|ui| {
            for (label, to) in targets {
                if ui
                    .small_button(format!("To {label}"))
                    .on_hover_text("moves it to the end of that chain (Ctrl-click copies)")
                    .clicked()
                {
                    moves.push(TreeDrop {
                        from: (inst, at),
                        to,
                        copy,
                    });
                }
            }
        });
    }

    /// a device dropped (or Move to…): within its chain a plain move, to
    /// another chain (a Split's path, the main chain, another instrument's
    /// chain) a move with its automation lanes dropped when it goes inside
    /// a Split (D8); `copy` leaves the original. One undo step.
    pub(crate) fn chains_drop(&mut self, m: TreeDrop) {
        let (fi, from) = m.from;
        let (ti, to, at) = m.to;
        if fi == ti && to.as_slice() == from.chain() {
            let op = if m.copy {
                let d = self.with(|s| {
                    s.song
                        .instruments
                        .get(fi)
                        .and_then(|x| fx::dev_at(&x.fx, &from).cloned())
                });
                let Some(mut d) = d else {
                    return;
                };
                d.renew_uid();
                FxOp::Insert(at, Box::new(d))
            } else {
                FxOp::Move(from.index(), at)
            };
            self.apply_fx_ops(fi, vec![wrap(&to, op)]);
            return;
        }
        let Some(d) = self.with(|s| {
            s.song
                .instruments
                .get(fi)
                .and_then(|x| fx::dev_at(&x.fx, &from).cloned())
        }) else {
            return;
        };
        // where it may go (also checked by the ops themselves)
        let own = from.as_slice();
        if fi == ti && to.len() > own.len() && to.starts_with(own) {
            self.status = "A Split can't go inside itself".into();
            return;
        }
        if !to.is_empty() && d.has_plugin() {
            self.status = format!(
                "{} can't go inside a Split yet: plugin effects go in the main chain or a layer's chain",
                d.title()
            );
            return;
        }
        if to.len() / 2 + d.split_depth() > fx::MAX_SPLIT_DEPTH {
            self.status = "One Split can sit inside another, but not deeper".into();
            return;
        }
        let full = self.with(|s| {
            s.song
                .instruments
                .get(ti)
                .and_then(|x| fx::chain_at(&x.fx, &to))
                .is_none_or(|c| c.len() >= fx::MAX_DEVICES)
        });
        if full {
            self.status = "That chain is full (16 devices)".into();
            return;
        }
        self.snapshot();
        let mut nd = d.clone();
        if m.copy {
            nd.renew_uid();
        }
        let name = d.title();
        let moved = self.with(|s| {
            // in first, then the original out (its path shifts when the
            // new device went in before it in a chain it sits under)
            let at_real = if to.is_empty() {
                s.song.insert_device(ti, at, nd)?
            } else {
                let c = fx::chain_at_mut(&mut s.song.instruments.get_mut(ti)?.fx, &to)?;
                let a = at.min(c.len());
                c.insert(a, nd);
                a
            };
            if m.copy {
                return Some(());
            }
            let mut src = from.as_slice().to_vec();
            if fi == ti && src.len() > to.len() && src.starts_with(&to) {
                let k = to.len();
                if src[k] as usize >= at_real {
                    src[k] += 1;
                }
            }
            let src = DevPath::from_slice(&src)?;
            if src.depth() == 0 {
                s.song.remove_device(fi, src.index())?;
            } else {
                let c = fx::chain_at_mut(&mut s.song.instruments.get_mut(fi)?.fx, src.chain())?;
                if src.index() < c.len() {
                    c.remove(src.index());
                }
            }
            Some(())
        });
        if moved.is_some() {
            let where_ = self.with(|s| {
                s.song
                    .instruments
                    .get(ti)
                    .map(|x| chain_label(&x.fx, &to))
                    .unwrap_or_default()
            });
            self.status = format!(
                "{} {name} to {where_}{}",
                if m.copy { "Copied" } else { "Moved" },
                if !to.is_empty() && from.depth() == 0 && !m.copy {
                    " (its automation lanes stay in the main chain only)"
                } else {
                    ""
                }
            );
            self.dirty = true;
            self.dev.sel = None;
        }
    }

    /// the Add Device window (mockup 3): ChipWave effects with a line each,
    /// the Split container, plugin effects (not inside a Split, D7);
    /// search, Enter adds, Esc closes
    pub(crate) fn add_device_window(&mut self, ctx: &egui::Context) {
        let Some(mut a) = self.dev.add.clone() else {
            return;
        };
        let t = th();
        let inside = !a.chain.is_empty();
        let depth = a.chain.len() / 2;
        let (target, n_in_chain) = self.with(|s| {
            let Some(x) = s.song.instruments.get(a.inst) else {
                return (String::new(), 0);
            };
            let c = fx::chain_at(&x.fx, &a.chain).map_or(&[][..], |c| c);
            let pos = if a.at >= c.len() {
                match c.last() {
                    Some(l) => format!("at the end, after {}", l.title()),
                    None => "as the first device".into(),
                }
            } else if a.at == 0 {
                "at the start".into()
            } else {
                format!("after {}", c[a.at - 1].title())
            };
            (
                format!(
                    "to {:02X} {} › {}, {pos}",
                    a.inst + 1,
                    x.name,
                    chain_crumb(&x.fx, &a.chain)
                ),
                c.len(),
            )
        });
        let q = a.query.to_lowercase();
        let hit = |s: &str, b: &str| {
            q.is_empty() || s.to_lowercase().contains(&q) || b.to_lowercase().contains(&q)
        };
        let kinds: Vec<FxKind> = FxKind::ALL
            .into_iter()
            .filter(|k| hit(k.name(), k.blurb()))
            .collect();
        let split_ok = depth < fx::MAX_SPLIT_DEPTH;
        // listed inside a Split too, greyed (D7)
        let plugins: Vec<crate::plugin::PluginPatch> = crate::plugin::scan::cache()
            .effects()
            .into_iter()
            .map(super::fxplugview::patch_of)
            .filter(|p| hit(&p.name, &p.vendor))
            .collect();
        let split_hit = split_ok && hit("Split parallel paths", FxKind::Split.blurb());
        // typing keeps a visible device highlighted, so Enter adds what shows
        let visible = match &a.pick {
            Some(AddPick::Kind(FxKind::Split)) => split_hit,
            Some(AddPick::Kind(k)) => kinds.contains(k),
            Some(AddPick::Plugin(p)) => !inside && plugins.contains(p),
            None => false,
        };
        if !visible {
            a.pick = kinds
                .first()
                .map(|k| AddPick::Kind(*k))
                .or_else(|| split_hit.then_some(AddPick::Kind(FxKind::Split)))
                .or_else(|| {
                    (!inside)
                        .then(|| plugins.first().cloned().map(AddPick::Plugin))
                        .flatten()
                });
        }
        let mut add = false;
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && a.pick.is_some() {
            add = true;
        }
        let screen = ctx.content_rect();
        let size = egui::vec2(
            (screen.width() - 80.0).clamp(420.0, 720.0),
            (screen.height() - 120.0).clamp(300.0, 470.0),
        );
        egui::Window::new("Add Device")
            .id(egui::Id::unique("chains_add_device"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(size)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("ADD DEVICE").strong().size(15.0));
                    ui.add(
                        egui::Label::new(egui::RichText::new(&target).color(t.accent)).truncate(),
                    );
                });
                let te = ui.add(
                    egui::TextEdit::singleline(&mut a.query)
                        .hint_text("Search devices · name or what it does")
                        .desired_width(f32::INFINITY),
                );
                if a.focus {
                    te.request_focus();
                    a.focus = false;
                }
                ui.add_space(4.0);
                let body_h = size.y - 110.0;
                ui.columns(2, |cols| {
                    let ui = &mut cols[0];
                    ui.label(egui::RichText::new("CHIPWAVE EFFECTS").small().strong().color(t.ink2));
                    egui::ScrollArea::vertical()
                        .id_salt("add_dev_fx")
                        .max_height(body_h)
                        .show(ui, |ui| {
                            for k in &kinds {
                                let on = a.pick == Some(AddPick::Kind(*k));
                                let r = ui
                                    .horizontal(|ui| {
                                        let b = ui.add_sized(
                                            egui::vec2(140.0, 18.0),
                                            egui::Button::selectable(on, k.name()),
                                        );
                                        ui.label(egui::RichText::new(k.blurb()).small().color(t.ink2));
                                        b
                                    })
                                    .inner;
                                if r.clicked() {
                                    a.pick = Some(AddPick::Kind(*k));
                                }
                                if r.double_clicked() {
                                    a.pick = Some(AddPick::Kind(*k));
                                    add = true;
                                }
                            }
                            if kinds.is_empty() {
                                ui.label(egui::RichText::new("nothing matches").color(t.ink2));
                            }
                        });
                    let ui = &mut cols[1];
                    ui.label(egui::RichText::new("CONTAINERS").small().strong().color(t.ink2));
                    if hit("Split parallel paths", FxKind::Split.blurb()) {
                        let on = a.pick == Some(AddPick::Kind(FxKind::Split));
                        let r = ui
                            .add_enabled(
                                split_ok,
                                egui::Button::selectable(on, "Split · parallel paths"),
                            )
                            .on_hover_text(
                                "the sound goes through 2-4 paths at once and is mixed back; an empty path is the dry sound",
                            )
                            .on_disabled_hover_text("one Split can sit inside another, not deeper");
                        if r.clicked() {
                            a.pick = Some(AddPick::Kind(FxKind::Split));
                        }
                        if r.double_clicked() {
                            a.pick = Some(AddPick::Kind(FxKind::Split));
                            add = true;
                        }
                        ui.label(
                            egui::RichText::new(
                                "2 paths at -6 dB to start, so the sound is unchanged until you add devices",
                            )
                            .small()
                            .color(t.ink2),
                        );
                    }
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("PLUGIN EFFECTS (CLAP / VST3)").small().strong().color(t.ink2),
                    );
                    if inside {
                        ui.label(
                            egui::RichText::new(
                                "Not inside a Split yet: plugin effects go in the main chain or a layer's chain.",
                            )
                            .small()
                            .color(t.ink2),
                        );
                    }
                    if plugins.is_empty() {
                        ui.label(
                            egui::RichText::new(
                                "No effect plugins found (Settings › Plugins › Find plugins…)",
                            )
                            .small()
                            .color(t.ink2),
                        );
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("add_dev_plug")
                            .max_height((body_h * 0.5).max(60.0))
                            .show(ui, |ui| {
                                for p in &plugins {
                                    let on = a.pick == Some(AddPick::Plugin(p.clone()));
                                    let label = if p.format == "vst3" {
                                        format!("{} (VST3)", p.name)
                                    } else {
                                        p.name.clone()
                                    };
                                    let r = ui
                                        .add_enabled(!inside, egui::Button::selectable(on, label))
                                        .on_hover_text(&p.vendor)
                                        .on_disabled_hover_text(
                                            "plugin effects can't go inside a Split yet",
                                        );
                                    if r.clicked() {
                                        a.pick = Some(AddPick::Plugin(p.clone()));
                                    }
                                    if r.double_clicked() {
                                        a.pick = Some(AddPick::Plugin(p.clone()));
                                        add = true;
                                    }
                                }
                            });
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("ALSO").small().strong().color(t.ink2));
                    ui.label(
                        egui::RichText::new(
                            "· drag any effect from Browser › Effects onto a row\n· Enter adds the highlighted device, Esc closes",
                        )
                        .small()
                        .color(t.ink2),
                    );
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let what = match &a.pick {
                        Some(AddPick::Kind(k)) => k.name().to_string(),
                        Some(AddPick::Plugin(p)) => p.name.clone(),
                        None => "nothing picked".into(),
                    };
                    ui.label(egui::RichText::new(what).strong().color(t.accent));
                    ui.label(
                        egui::RichText::new(format!("→ position {} of {}", a.at.min(n_in_chain) + 1, n_in_chain + 1))
                            .color(t.ink2),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(a.pick.is_some(), egui::Button::new("Add  (Enter)"))
                            .clicked()
                        {
                            add = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            });
        if add {
            let at = a.at;
            match a.pick.clone() {
                Some(AddPick::Kind(k)) => {
                    let d = if k == FxKind::Split {
                        Device::split()
                    } else {
                        Device::new(k)
                    };
                    self.apply_fx_ops(a.inst, vec![wrap(&a.chain, FxOp::Insert(at, Box::new(d)))]);
                    let idx = at.min(n_in_chain);
                    let sel = DevPath::from_slice(&[a.chain.clone(), vec![idx as u8]].concat());
                    self.dev.sel = sel.map(|p| Sel::Dev(a.inst, p));
                }
                Some(AddPick::Plugin(p)) if !inside => {
                    self.add_plugin_fx_at((a.inst + 1) as u8, p, at);
                    let idx = at.min(n_in_chain);
                    self.dev.sel = Some(Sel::Dev(a.inst, DevPath::top(idx)));
                }
                _ => {}
            }
            self.dev.add = None;
            return;
        }
        self.dev.add = if close { None } else { Some(a) };
    }

    /// "Signal flow": the order the sound goes through, in words
    fn chains_flow(&mut self, ui: &mut egui::Ui, root: usize) {
        let t = th();
        let line = self.with(|s| flow_text(&s.song, root));
        ui.label(
            egui::RichText::new("SIGNAL FLOW")
                .small()
                .strong()
                .color(t.ink2),
        );
        ui.label(egui::RichText::new(line).color(t.ink));
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(
                "Select a row on the left to see its controls here. Every change is one undo step (Ctrl+Z).",
            )
            .small()
            .color(t.ink2),
        );
    }
}

/// "07 Supersaw Pad → Filter → Chorus (bypassed) → track fader, pan, sends"
pub(crate) fn flow_text(song: &Song, root: usize) -> String {
    let Some(inst) = song.instruments.get(root) else {
        return String::new();
    };
    let mut parts = vec![format!("{:02X} {}", root + 1, inst.name)];
    if let InstrumentKind::Rack(r) = &inst.kind {
        let ls: Vec<String> = r
            .layers
            .iter()
            .enumerate()
            .map(|(k, l)| {
                let mut v = vec![song
                    .instruments
                    .get(l.slot)
                    .map(|x| x.name.clone())
                    .unwrap_or_default()];
                if let Some(x) = song.instruments.get(l.slot) {
                    v.extend(x.fx.iter().map(dev_flow));
                }
                format!(
                    "{}: {}{}",
                    super::rackops::layer_letter(k),
                    v.join(" › "),
                    if l.on { "" } else { " (muted)" }
                )
            })
            .collect();
        parts[0] = format!(
            "{} [{}]",
            parts[0],
            if ls.is_empty() {
                "no layers".to_string()
            } else {
                ls.join(" | ")
            }
        );
    }
    for d in &inst.fx {
        parts.push(dev_flow(d));
    }
    parts.push("track fader, pan, sends".into());
    parts.join("  →  ")
}

fn dev_flow(d: &Device) -> String {
    let mut s = d.title();
    if d.kind == FxKind::Split {
        let paths: Vec<String> = d
            .paths
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let inner: Vec<String> = p.devices.iter().map(dev_flow).collect();
                format!(
                    "{}: {}",
                    super::fxview::path_letter(k),
                    if inner.is_empty() {
                        "dry".to_string()
                    } else {
                        inner.join(" › ")
                    }
                )
            })
            .collect();
        s = format!("Split [{}]", paths.join(" | "));
    }
    if !d.enabled {
        s.push_str(" (bypassed)");
    }
    s
}

/// a place in a chain: instrument, chain prefix, index (usize::MAX = the end)
pub(crate) type ChainAt = (usize, Vec<u8>, usize);

/// how many layers rack `r` has (0 for anything else)
pub(crate) fn layer_count(song: &Song, r: usize) -> usize {
    match &song.instruments.get(r).map(|i| &i.kind) {
        Some(InstrumentKind::Rack(x)) => x.layers.len(),
        _ => 0,
    }
}

/// "Synth · -3.0 dB · keys C-3..B-5 · +12 st"
fn layer_summary(l: &crate::model::Layer, tag: &str) -> String {
    let mut v = vec![tag.to_string()];
    if l.level_db != 0.0 {
        v.push(format!("{:+.1} dB", l.level_db));
    }
    if l.pan != 0.0 {
        v.push(format!("pan {:+.2}", l.pan));
    }
    if l.key_lo > 0 || l.key_hi < 127 {
        v.push(format!(
            "keys {}..{}",
            crate::model::note_name(l.key_lo.max(1)),
            crate::model::note_name(l.key_hi.max(1))
        ));
    }
    if l.transpose != 0 {
        v.push(format!("{:+} st", l.transpose));
    }
    v.join(" · ")
}

/// a device row dropped somewhere in the tree
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TreeDrop {
    pub(crate) from: (usize, DevPath),
    /// instrument, chain prefix, index (usize::MAX = the end)
    pub(crate) to: (usize, Vec<u8>, usize),
    pub(crate) copy: bool,
}

/// "Path B", "Effects", ... a chain's short name for buttons
fn chain_label(fx: &[Device], prefix: &[u8]) -> String {
    match prefix {
        [] => "the main chain".into(),
        _ => {
            let c = chain_crumb(fx, prefix);
            c.rsplit(" › ")
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" › ")
        }
    }
}

/// every chain of instrument `inst` that device `d` (at `at`) may move to:
/// the main chain and each Split path, minus its own chain and anything
/// inside itself; plugin effects stay out of Splits (D7)
pub(crate) fn move_targets(
    song: &Song,
    inst: usize,
    at: DevPath,
    d: &Device,
) -> Vec<(String, ChainAt)> {
    let Some(i) = song.instruments.get(inst) else {
        return vec![];
    };
    let mut chains: Vec<Vec<u8>> = vec![vec![]];
    fn walk(c: &[Device], pre: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) {
        for (k, x) in c.iter().enumerate() {
            if x.kind != FxKind::Split {
                continue;
            }
            for (p, path) in x.paths.iter().enumerate() {
                pre.push(k as u8);
                pre.push(p as u8);
                out.push(pre.clone());
                walk(&path.devices, pre, out);
                pre.pop();
                pre.pop();
            }
        }
    }
    walk(&i.fx, &mut vec![], &mut chains);
    let own = at.as_slice();
    chains
        .into_iter()
        .filter(|c| c.as_slice() != at.chain())
        // not into itself
        .filter(|c| !(c.len() > own.len() && c.starts_with(own)))
        .filter(|c| c.is_empty() || !d.has_plugin())
        .filter(|c| c.len() / 2 + d.split_depth() <= fx::MAX_SPLIT_DEPTH)
        .map(|c| (chain_label(&i.fx, &c), (inst, c, usize::MAX)))
        .collect()
}

/// an op from a Split's path controls (addressed with usize::MAX) for the
/// Split at index `k`
fn op_at(k: usize, op: FxOp) -> FxOp {
    match op {
        FxOp::PathOn(_, p) => FxOp::PathOn(k, p),
        FxOp::PathLevel(_, p, v, f) => FxOp::PathLevel(k, p, v, f),
        FxOp::PathPan(_, p, v, f) => FxOp::PathPan(k, p, v, f),
        FxOp::RemovePath(_, p) => FxOp::RemovePath(k, p),
        FxOp::AddPath(_) => FxOp::AddPath(k),
        o => o,
    }
}

/// "0 dB · C" / "-4.0 dB · L 30 · muted"
fn path_summary(p: &fx::FxPath) -> String {
    let pan = if p.pan.abs() < 0.005 {
        "C".to_string()
    } else if p.pan < 0.0 {
        format!("L {:.0}", -p.pan * 100.0)
    } else {
        format!("R {:.0}", p.pan * 100.0)
    };
    let mut s = format!("{:.1} dB · {pan}", p.level_db);
    if !p.on {
        s.push_str(" · muted");
    }
    s
}

/// where a dragged device row lands: (marker y, (instrument, chain,
/// index)). Over a device row: before it (upper half) or after it; over
/// a path or Effects heading: the start of that chain.
fn drop_target(drawn: &[(Rect, RowKind)], p: Pos2) -> Option<(f32, ChainAt)> {
    // on a row, or just below the last one of a chain (the "+ Add device"
    // line): after that row
    let (r, k) = drawn
        .iter()
        .find(|(r, _)| r.y_range().contains(p.y))
        .or_else(|| {
            drawn
                .iter()
                .filter(|(r, _)| r.bottom() <= p.y && p.y - r.bottom() < 24.0)
                .max_by(|a, b| a.0.bottom().total_cmp(&b.0.bottom()))
                .filter(|(_, k)| matches!(k, RowKind::Dev(..)))
        })?;
    match k {
        RowKind::Dev(i, q) => {
            let after = p.y > r.center().y;
            Some((
                if after { r.bottom() } else { r.top() },
                (*i, q.chain().to_vec(), q.index() + after as usize),
            ))
        }
        RowKind::Path(i, sp, pi) => {
            let mut c = sp.as_slice().to_vec();
            c.push(*pi as u8);
            Some((r.bottom(), (*i, c, 0)))
        }
        RowKind::Effects(i) => Some((r.bottom(), (*i, vec![], 0))),
        _ => None,
    }
}

/// the fold triangle (painted: the UI fonts have no ▾ / ▸)
pub(crate) fn fold_arrow(p: &egui::Painter, c: Pos2, open: bool, col: Color32) {
    let pts = if open {
        vec![
            c + egui::vec2(-4.0, -2.5),
            c + egui::vec2(4.0, -2.5),
            c + egui::vec2(0.0, 3.0),
        ]
    } else {
        vec![
            c + egui::vec2(-2.5, -4.0),
            c + egui::vec2(3.0, 0.0),
            c + egui::vec2(-2.5, 4.0),
        ]
    };
    p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
}
