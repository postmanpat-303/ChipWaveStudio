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
use crate::fx::{self, DevPath, Device};

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
    chain_rows(song, root, &mut out, 1, closed);
    out
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
    for (i, _) in inst.fx.iter().enumerate() {
        out.push(Row {
            depth: depth + 1,
            kind: RowKind::Dev(idx, DevPath::top(i)),
            key: String::new(),
            open: true,
        });
    }
}

/// the device a selection points at
pub(crate) fn sel_device(song: &Song, sel: Sel) -> Option<&Device> {
    match sel {
        Sel::Dev(i, p) => fx::dev_at(&song.instruments.get(i)?.fx, &p),
        Sel::Inst(_) => None,
    }
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
        Sel::Dev(i, p) => {
            let d = sel_device(song, sel).map(|d| d.title()).unwrap_or_default();
            format!(
                "{} › Effects › {d}  (device {} of {})",
                name(i),
                p.index() + 1,
                fx::chain_at(&song.instruments[i].fx, p.chain()).map_or(0, |c| c.len())
            )
        }
    }
}

impl App {
    /// the instrument the tree shows: the current one
    pub(crate) fn chains_root(&self) -> Option<usize> {
        let n = self.with(|s| s.song.instruments.len());
        let i = (self.instr as usize).wrapping_sub(1);
        (i < n).then_some(i)
    }

    /// a selection that no longer exists (undo, removed device, another
    /// instrument) falls back to the root instrument
    fn chains_fix_sel(&mut self, root: usize) {
        let ok = match self.dev.sel {
            Some(Sel::Inst(i)) => i == root,
            Some(s @ Sel::Dev(i, _)) => {
                i == root && self.with(|x| sel_device(&x.song, s).is_some())
            }
            None => false,
        };
        if !ok {
            self.dev.sel = Some(Sel::Inst(root));
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
        let mut drawn: Vec<(Rect, RowKind)> = vec![];
        let pointer = ui.ctx().pointer_latest_pos();
        let released = ui.input(|i| i.pointer.any_released());
        ui.painter()
            .rect_filled(ui.max_rect(), 4.0, t.ink2.gamma_multiply(0.05));
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("CHAINS").small().strong().color(t.ink2));
            if !slim {
                ui.label(
                    egui::RichText::new("drag rows to reorder · right-click for more")
                        .small()
                        .color(t.ink2),
                );
            }
        });
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
                    // guide line for nesting
                    if row.depth > 0 {
                        p.vline(
                            x0 - INDENT * 0.5,
                            r.y_range(),
                            Stroke::new(1.0, t.border.gamma_multiply(0.5)),
                        );
                    }
                    let mut x = x0;
                    // fold arrow (text, so it reads as a control)
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
                            if !slim {
                                p.text(
                                    Pos2::new(g.right() + 8.0, r.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    tag,
                                    FontId::proportional(11.0),
                                    t.ink2,
                                );
                            }
                        }
                        RowKind::Effects(i) => {
                            let g = p.text(
                                Pos2::new(x, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                "Effects",
                                font.clone(),
                                t.ink,
                            );
                            if !slim {
                                p.text(
                                    Pos2::new(g.right() + 8.0, r.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    "in order, after the instrument",
                                    FontId::proportional(11.0),
                                    t.ink2,
                                );
                            }
                            let _ = i;
                        }
                        RowKind::Dev(i, path) => {
                            let d =
                                self.with(|s| sel_device(&s.song, Sel::Dev(*i, *path)).cloned());
                            if let Some(d) = d {
                                // power: a dot you click (bypass)
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
                                if dr.clicked() {
                                    ops.push((*i, FxOp::Toggle(path.index())));
                                }
                                let label = format!("{}  {}", path.index() + 1, d.title());
                                let g = p.text(
                                    Pos2::new(x + 14.0, r.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    label,
                                    font.clone(),
                                    if d.enabled { t.ink } else { t.ink2 },
                                );
                                if !d.enabled {
                                    p.text(
                                        Pos2::new(g.right() + 8.0, r.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        "bypassed",
                                        FontId::proportional(11.0),
                                        t.brick,
                                    );
                                }
                                if resp.drag_started() {
                                    self.dev.tree_drag = Some(Sel::Dev(*i, *path));
                                }
                                resp.context_menu(|ui| {
                                    ui.label(egui::RichText::new(d.title()).strong());
                                    let k = path.index();
                                    for (txt, op) in [
                                        (
                                            if d.enabled { "Bypass" } else { "Enable" },
                                            FxOp::Toggle(k),
                                        ),
                                        ("Duplicate", FxOp::Dup(k)),
                                        ("Move up", FxOp::Move(k, k.saturating_sub(1))),
                                        ("Move down", FxOp::Move(k, k + 2)),
                                        ("Remove", FxOp::Remove(k)),
                                    ] {
                                        if ui.button(txt).clicked() {
                                            ops.push((*i, op));
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        }
                        RowKind::Note(s) => {
                            p.text(
                                Pos2::new(x, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                s,
                                FontId::proportional(11.0),
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
        // drag a device row: a marker between rows, then the move
        if let Some(Sel::Dev(i, from)) = self.dev.tree_drag {
            let target = pointer.and_then(|p| drop_slot(&drawn, i, from, p));
            if let Some((y, to)) = target {
                let r = ui.max_rect();
                super::devices::paint_marker_h(ui.painter(), y, r.x_range().shrink(8.0));
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                if released {
                    ops.push((i, FxOp::Move(from.index(), to)));
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
            Sel::Inst(i) => {
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(crumb).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Open full editor  Ctrl+I")
                            .on_hover_text("every control of this instrument (the Instrument tab)")
                            .clicked()
                        {
                            self.instr = (i + 1) as u8;
                            self.set_lower(1);
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
            Sel::Dev(i, path) => {
                let Some(d) = self.with(|s| sel_device(&s.song, sel).cloned()) else {
                    return;
                };
                let k = path.index();
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(egui::RichText::new(crumb).strong()).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Remove")
                            .on_hover_text("remove (Ctrl+Z brings it back)")
                            .clicked()
                        {
                            ops.push((i, FxOp::Remove(k)));
                        }
                        if ui
                            .button("Duplicate")
                            .on_hover_text("a copy right after it")
                            .clicked()
                        {
                            ops.push((i, FxOp::Dup(k)));
                        }
                        let b = if d.enabled { "Bypass" } else { "Enable" };
                        if ui
                            .button(b)
                            .on_hover_text("bypass: the sound passes it unchanged")
                            .clicked()
                        {
                            ops.push((i, FxOp::Toggle(k)));
                        }
                    });
                });
                let h = (ui.available_height() - 6.0).max(120.0);
                let n = self.with(|s| {
                    fx::chain_at(&s.song.instruments[i].fx, path.chain()).map_or(0, |c| c.len())
                });
                let mut local: Vec<FxOp> = vec![];
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0);
                    let keep = self.dev.card_w;
                    self.dev.card_w = super::devcards::CARD_W.min(ui.available_width() * 0.45);
                    let mut card = d.clone();
                    card.collapsed = false;
                    let key = super::devcards::open_key(
                        &self.with(|s| {
                            fx::chain_at(&s.song.instruments[i].fx, path.chain())
                                .map(|c| c.to_vec())
                                .unwrap_or_default()
                        }),
                        k,
                    );
                    // the card without its own "…" column: All parameters is beside it
                    self.dev.open.remove(&(i, key));
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
                            ui.label(
                                egui::RichText::new("ALL PARAMETERS")
                                    .small()
                                    .strong()
                                    .color(t.ink2),
                            );
                            self.devices_more(ui, i, k, &card, &mut local);
                        });
                    });
                });
                if self.dev.drag.is_some() && ui.input(|x| x.pointer.any_released()) {
                    self.dev.drag = None;
                }
                for op in local {
                    // (the card's own collapse means nothing here)
                    if matches!(op, FxOp::Collapse(_)) {
                        continue;
                    }
                    ops.push((i, op));
                }
            }
        }
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
    for d in &inst.fx {
        parts.push(dev_flow(d));
    }
    parts.push("track fader, pan, sends".into());
    parts.join("  →  ")
}

fn dev_flow(d: &Device) -> String {
    let mut s = d.title();
    if !d.enabled {
        s.push_str(" (bypassed)");
    }
    s
}

/// where a dragged device row lands: (marker y, index in its chain) for
/// rows of the same chain as `from`
fn drop_slot(
    drawn: &[(Rect, RowKind)],
    inst: usize,
    from: DevPath,
    p: Pos2,
) -> Option<(f32, usize)> {
    let same: Vec<(Rect, usize)> = drawn
        .iter()
        .filter_map(|(r, k)| match k {
            RowKind::Dev(i, q) if *i == inst && q.chain() == from.chain() => Some((*r, q.index())),
            _ => None,
        })
        .collect();
    let first = same.first()?;
    let last = same.last()?;
    if p.y < first.0.top() - ROW || p.y > last.0.bottom() + ROW {
        return None;
    }
    let to = same
        .iter()
        .find(|(r, _)| p.y < r.center().y)
        .map(|(_, k)| *k)
        .unwrap_or(last.1 + 1);
    let y = same
        .iter()
        .find(|(_, k)| *k == to)
        .map(|(r, _)| r.top())
        .unwrap_or(last.0.bottom());
    Some((y, to))
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
