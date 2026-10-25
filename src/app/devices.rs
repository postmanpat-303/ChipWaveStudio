//! Devices (0.2.4, requested by Del (postmanpat) inspired by other DAWs): a lower-panel
//! tab next to the piano roll that shows the selected track's sound chain
//! left to right: its instrument (the lower-panel instrument editor in a
//! card), then every insert effect as a card with its key controls inline
//! (EQ with its curve, compressor transfer curve, filter response, delay
//! taps, reverb decay; plugin effects with their thumbnail and editor
//! button). Each card has bypass, collapse, remove and a title to drag it to
//! a new place; the `+` card at the end opens Browser › Effects. Effects
//! dragged from the Browser drop in between with an insertion marker.
//!
//! The chain is the instrument's existing `fx` list (no song format change);
//! edits go through `apply_fx_ops`, which snapshots for undo and keeps
//! automation lanes on their device. The engine thread rebuilds only the
//! devices that changed (`fx::ChainState::sync`), and the audio callback
//! never sees any of it (it only pops the ring buffer, see audio.rs).
//! Original ChipWave code.
use super::devcards::{self, open_key, CARD_MIN_W, CARD_W, GAP, MORE_W, ROW_H};
use super::fxview::{strip_fill, FxOp};
use super::knob::{self, KnobArgs, CELL};
use super::rackview::{Pick, TabHit, TAB_W};
use super::*;
use crate::fx::{self, Device, FxKind};

/// header height of a card
const HEAD: f32 = 24.0;
/// a collapsed card's width
const SLIM: f32 = 30.0;

/// Devices state that is not part of the song
pub(crate) struct DevUi {
    /// screen rects as last drawn: the whole row, the instrument card and
    /// each device card (for Browser drops and the insertion marker)
    pub(crate) row: Rect,
    pub(crate) inst: Rect,
    pub(crate) cards: Vec<Rect>,
    /// the device card being dragged to a new place
    pub(crate) drag: Option<usize>,
    /// the instrument card is folded to a slim strip
    pub(crate) inst_folded: bool,
    /// the track the view last followed (usize::MAX = none yet)
    pub(crate) follow_ch: usize,
    /// a Browser instrument dropped on the instrument card: keep the
    /// slot's FX chain when its sound is replaced
    pub(crate) keep_fx: Option<(u8, Vec<Device>)>,
    /// screenshot: hold a Browser drag over device card N
    pub(crate) shot_at: Option<usize>,
    /// cards with their "…" open, as (instrument, device uid); remembered
    /// while ChipWave runs
    pub(crate) open: std::collections::HashSet<(usize, u64)>,
    /// this frame's card width (every card the same; narrower when many)
    pub(crate) card_w: f32,
    /// Nested Device Chains: the selected row
    pub(crate) sel: Option<super::chains::Sel>,
    /// ... the device row being dragged
    pub(crate) tree_drag: Option<super::chains::Sel>,
    /// ... the rows as last drawn
    pub(crate) tree_rows: Vec<(Rect, super::chains::RowKind)>,
    /// `cards` are tree rows (top to bottom), not the card row
    pub(crate) vertical: bool,
    /// drawing a device inside a Split: no automate / modulate handles
    /// (D8) and no meter
    pub(crate) no_auto: bool,
    /// the panel was opened by selecting a track header or an instrument
    /// (a clip click then shows the clip, as the Instrument tab does)
    pub(crate) by_select: bool,
    /// a device row dropped in the tree (handled after the frame's ops)
    pub(crate) tree_drop: Option<super::chains::TreeDrop>,
    /// the Add Device window is open for this chain
    pub(crate) add: Option<super::chains::AddAt>,
    /// the Save Custom Rack dialog is open
    pub(crate) save_rack: Option<super::rackui::SaveRack>,
    /// My Racks should be listed again (a rack was saved or changed)
    pub(crate) racks_dirty: bool,
    pub(crate) my_racks: super::rackui::MyRacks,
    /// the Rack: the selected card
    pub(crate) pick: super::rackview::Pick,
    /// ... was the last panel clicked (Delete, Ctrl+D and 0 act on it)
    pub(crate) focus: bool,
    /// the chain editor (tree and detail) instead of the Rack row; opened
    /// from a card's ⋮ › Open in the chain editor
    pub(crate) tree: bool,
    /// drawing a Rack card: its side tab has power, remove and "…", so
    /// the card's header leaves them out
    pub(crate) tabbed: bool,
    /// an Arp switched off, per instrument: its settings for the next
    /// switch-on (while ChipWave runs)
    pub(crate) arp_stash: std::collections::HashMap<usize, crate::arp::Arp>,
    /// the Arp grid shows steps 17-32
    pub(crate) arp_page: u8,
    /// the Arp card (or its slim "+ ARP" strip) this frame, and its grid
    /// rows (numbers, velocity, gate, accent, rest/tie, ratchet, transpose)
    pub(crate) arp_card: Rect,
    pub(crate) arp_rows: [Rect; 7],
}

impl Default for DevUi {
    fn default() -> Self {
        Self {
            row: Rect::NOTHING,
            inst: Rect::NOTHING,
            cards: vec![],
            drag: None,
            inst_folded: false,
            follow_ch: usize::MAX,
            keep_fx: None,
            shot_at: None,
            open: Default::default(),
            card_w: CARD_W,
            sel: None,
            tree_drag: None,
            tree_rows: vec![],
            vertical: false,
            no_auto: false,
            by_select: false,
            tree_drop: None,
            add: None,
            save_rack: None,
            racks_dirty: true,
            my_racks: Default::default(),
            pick: Default::default(),
            focus: false,
            tree: false,
            tabbed: false,
            arp_stash: Default::default(),
            arp_page: 0,
            arp_card: Rect::NOTHING,
            arp_rows: [Rect::NOTHING; 7],
        }
    }
}

/// where in the chain a drop at x lands (0..=n): before the first card whose
/// centre is right of x
pub(crate) fn insert_index(cards: &[Rect], x: f32) -> usize {
    cards
        .iter()
        .position(|r| x < r.center().x)
        .unwrap_or(cards.len())
}

/// the marker's x for insertion index `i`
pub(crate) fn marker_x(inst: Rect, cards: &[Rect], i: usize) -> f32 {
    if let Some(r) = cards.get(i) {
        r.left() - 5.0
    } else if let Some(r) = cards.last() {
        r.right() + 5.0
    } else {
        inst.right() + 9.0
    }
}

fn hz_x(r: Rect, f: f32) -> f32 {
    let t = (f / 20.0).max(1.0).ln() / (20000.0f32 / 20.0).ln();
    r.left() + t.clamp(0.0, 1.0) * r.width()
}

fn x_hz(r: Rect, x: f32) -> f32 {
    let t = ((x - r.left()) / r.width()).clamp(0.0, 1.0);
    20.0 * (1000.0f32).powf(t)
}

/// graph background with a few frequency lines
fn graph_bg(p: &egui::Painter, r: Rect, freq_grid: bool) {
    let t = th();
    if t.ember {
        crate::ember::well(p, r, 4);
        let g = Stroke::new(1.0, crate::ember::GRID2);
        if freq_grid {
            for f in [100.0, 1000.0, 10000.0] {
                p.vline(hz_x(r, f), r.y_range().shrink(1.0), g);
            }
        }
        p.hline(r.x_range().shrink(1.0), r.center().y, g);
        return;
    }
    p.rect_filled(
        r,
        4,
        Color32::from_black_alpha(if t.light { 22 } else { 70 }),
    );
    let g = Stroke::new(1.0, t.ink2.gamma_multiply(0.18));
    if freq_grid {
        for f in [100.0, 1000.0, 10000.0] {
            p.vline(hz_x(r, f), r.y_range(), g);
        }
    }
    p.hline(r.x_range(), r.center().y, g);
}

/// the 0 dB-centred dB -> y for the EQ / filter graphs
fn db_y(r: Rect, db: f32, range: f32) -> f32 {
    r.center().y - (db / range).clamp(-1.0, 1.0) * (r.height() * 0.5 - 3.0)
}

/// a curve over the log frequency axis
fn freq_curve(p: &egui::Painter, r: Rect, col: Color32, fill: bool, f: impl Fn(f32) -> f32) {
    let n = 96;
    let pts: Vec<Pos2> = (0..=n)
        .map(|k| {
            let x = r.left() + r.width() * k as f32 / n as f32;
            Pos2::new(x, f(x_hz(r, x)))
        })
        .collect();
    if th().ember && col == th().accent {
        if fill {
            crate::ember::fill_under(p, &pts, r.bottom() - 1.0, col, 0.28);
        }
        crate::ember::glow_line(p, &pts, 1.8, th().acc_hot);
        return;
    }
    if fill {
        let y0 = r.center().y;
        for w in pts.windows(2) {
            p.add(egui::Shape::convex_polygon(
                vec![w[0], w[1], Pos2::new(w[1].x, y0), Pos2::new(w[0].x, y0)],
                col.gamma_multiply(0.18),
                Stroke::NONE,
            ));
        }
    }
    p.add(egui::Shape::line(pts, Stroke::new(1.8, col)));
}

/// magnitude in dB of ChipWave's Filter device (state-variable, k = 2 - 2 res)
fn svf_db(mode: usize, fc: f32, res: f32, f: f32) -> f32 {
    let k = (2.0 - 2.0 * res).max(0.05);
    let w = f / fc.max(1.0);
    let den = ((1.0 - w * w).powi(2) + (k * w).powi(2)).sqrt().max(1e-6);
    let m = match mode {
        0 => 1.0 / den,
        1 => w * w / den,
        _ => w / den,
    };
    20.0 * m.max(1e-6).log10()
}

impl App {
    /// the Devices tab is showing in the lower panel
    pub(crate) fn devices_visible(&self) -> bool {
        matches!(self.view, View::Pattern | View::Timeline)
            && self.lower_visible()
            && self.extra.lower_dev
    }

    /// the track / instrument the Effects tab and the Devices row add to
    pub(crate) fn devices_target_label(&self) -> String {
        let idx = (self.instr as usize).wrapping_sub(1);
        let (inst, track) = self.with(|s| {
            let inst = s.song.instruments.get(idx).map(|i| i.name.clone());
            let track = s.song.channel_names.get(self.ch).cloned();
            (inst, track)
        });
        match inst {
            None => "no instrument yet (pick a track)".into(),
            Some(i) => match track {
                Some(t) if !t.trim().is_empty() && !t.eq_ignore_ascii_case(&i) => {
                    format!("{t} · {:02X} {i}", self.instr)
                }
                _ => format!("{:02X} {i}", self.instr),
            },
        }
    }

    /// follow the selected track: its instrument becomes the current one
    fn devices_follow(&mut self) {
        if self.dev.follow_ch == self.ch {
            return;
        }
        self.dev.follow_ch = self.ch;
        let s = self.track_slot(self.ch);
        let n = self.with(|x| x.song.instruments.len());
        if s > 0 && s as usize <= n {
            self.instr = s;
        }
    }

    /// Browser › Effects, from the `+` card
    fn devices_open_effects(&mut self) {
        if !self.extra.browser_open {
            self.toggle_browser();
        }
        self.br.filter.tab = crate::browser::Tab::Effects;
        self.br.filter.category = None;
        self.br.filter.creator = None;
        self.br.filter.tags.clear();
        self.status = format!(
            "Browser › Effects: double-click adds to {}, or drag into the Devices row",
            self.devices_target_label()
        );
    }

    /// the lower panel's Devices tab
    pub(super) fn devices_view(&mut self, ui: &mut egui::Ui) {
        self.devices_follow();
        if self.dev.tree {
            self.chains_view(ui);
            return;
        }
        self.dev.vertical = false;
        // an Empty track: an empty Rack, not the last-selected instrument
        if self.chooser_wanted() {
            self.empty_rack_hint(ui);
            return;
        }
        let t = th();
        let n = self.with(|s| s.song.instruments.len());
        if n > 0 && (self.instr == 0 || self.instr as usize > n) {
            self.instr = 1;
        }
        let idx = (self.instr as usize).wrapping_sub(1);
        let chain = self.with(|s| s.song.instruments.get(idx).map(|i| i.fx.clone()));
        // a rack's layer: the way back to its rack
        self.rack_back_bar(ui, idx);
        let Some(chain) = chain else {
            self.dev.row = ui.max_rect();
            self.dev.inst = Rect::NOTHING;
            self.dev.cards.clear();
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("This track has no instrument yet").strong());
                ui.label(
                    egui::RichText::new(
                        "Drag one in from the Browser, or pick a track that plays one",
                    )
                    .color(t.ink2),
                );
                ui.menu_button("New instrument", |ui| self.new_instrument_menu(ui));
            });
            return;
        };
        // the hint line under the row
        let foot = 18.0;
        let h = (ui.available_height() - 4.0 - foot).max(120.0);
        let press = ui.input(|i| {
            if i.pointer.any_pressed() {
                i.pointer.press_origin()
            } else {
                None
            }
        });
        let popup = egui::Popup::is_any_open(ui.ctx());
        let builtin = self
            .with(|s| {
                s.song
                    .instruments
                    .get(idx)
                    .map(|i| crate::theme::is_builtin(&i.kind))
            })
            .unwrap_or(false);
        let mut ops: Vec<FxOp> = vec![];
        let mut cards: Vec<Rect> = vec![];
        let mut inst_rect = Rect::NOTHING;
        let mut open_effects = false;
        let pointer = ui.ctx().pointer_latest_pos();
        let released = ui.input(|i| i.pointer.any_released());
        let row_top = ui.cursor().top();
        // one width for every card: the most that fits, between the two limits
        {
            let open = chain
                .iter()
                .enumerate()
                .filter(|(i, d)| {
                    !d.collapsed && self.dev.open.contains(&(idx, open_key(&chain, *i)))
                })
                .count();
            let slim = chain.iter().filter(|d| d.collapsed).count() + self.dev.inst_folded as usize;
            // the instrument card has its own width (its face's columns)
            let iw = if self.dev.inst_folded {
                0.0
            } else {
                super::rackview::inst_card_w(self, idx)
            };
            let full = chain.len() - chain.iter().filter(|d| d.collapsed).count();
            let aw = super::arpcard::arp_card_w(self, idx);
            let fixed = 16.0
                + iw
                + aw
                + if aw > 0.0 { GAP } else { 0.0 }
                + plus_w(chain.is_empty())
                + GAP * (chain.len() + 1) as f32
                + SLIM * slim as f32
                + MORE_W * open as f32
                + TAB_W * (chain.len() + 1) as f32;
            self.dev.card_w = if full == 0 {
                CARD_W
            } else {
                ((ui.available_width() - fixed) / full as f32)
                    .clamp(CARD_MIN_W, CARD_W)
                    .floor()
            };
        }
        // where an empty chain's drop box ends (the row's visible right edge)
        let row_right = ui.max_rect().right();
        egui::ScrollArea::horizontal()
            .id_salt("devices_row")
            .max_height(h + 4.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(4.0);
                    // the Arp (or Note repeat) plays the notes into the
                    // instrument, so it comes first
                    if self.devices_arp_card(ui, h, idx).is_some() {
                        chevron(ui, h);
                    }
                    self.dev.tabbed = true;
                    let (tab, _) = ui.allocate_exact_size(egui::vec2(TAB_W, h), Sense::hover());
                    let r = self.devices_inst_card(ui, h, idx);
                    let folded = self.dev.inst_folded;
                    if self.rack_tab(ui, tab, r, Pick::Inst, true, folded, builtin, idx, &mut ops)
                        == Some(TabHit::Collapse)
                    {
                        self.dev.inst_folded = !folded;
                    }
                    inst_rect = tab.union(r);
                    chevron(ui, h);
                    for (i, d) in chain.iter().enumerate() {
                        let (tab, _) = ui.allocate_exact_size(egui::vec2(TAB_W, h), Sense::hover());
                        let r = self.devices_card(
                            ui,
                            idx,
                            i,
                            d,
                            open_key(&chain, i),
                            h,
                            chain.len(),
                            &mut ops,
                        );
                        let pick = Pick::Fx(i);
                        match self.rack_tab(
                            ui,
                            tab,
                            r,
                            pick,
                            d.enabled,
                            d.collapsed,
                            false,
                            idx,
                            &mut ops,
                        ) {
                            Some(TabHit::Power) => ops.push(FxOp::Toggle(i)),
                            Some(TabHit::Remove) => ops.push(FxOp::Remove(i)),
                            Some(TabHit::Collapse) => ops.push(FxOp::Collapse(i)),
                            None => {}
                        }
                        cards.push(tab.union(r));
                        chevron(ui, h);
                    }
                    self.dev.tabbed = false;
                    let fill = (row_right - ui.cursor().left() - 12.0).max(plus_w(true));
                    if plus_card(ui, h, chain.is_empty(), fill) {
                        open_effects = true;
                    }
                    ui.add_space(12.0);
                });
                // drag a card's title: insertion marker, then the move
                if let (Some(from), Some(p)) = (self.dev.drag, pointer) {
                    let to = insert_index(&cards, p.x);
                    let x = marker_x(inst_rect, &cards, to);
                    paint_marker(ui.painter(), x, inst_rect.y_range());
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    if released {
                        self.dev.drag = None;
                        ops.push(FxOp::Move(from, to));
                    }
                } else if released {
                    self.dev.drag = None;
                }
            });
        self.dev.row = Rect::from_min_max(
            Pos2::new(ui.max_rect().left(), row_top),
            ui.max_rect().right_bottom(),
        );
        let root = self.chains_root().unwrap_or(idx);
        let flow = self.with(|s| super::chains::flow_text(&s.song, root));
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "Click a card to select it · Delete removes it (Ctrl+Z undoes) · \
                         Ctrl+D duplicates · 0 bypasses · drag a title to reorder · \
                         drop from the Browser on +",
                    )
                    .size(crate::theme::ts::px(crate::theme::ts::SMALL))
                    .color(t.ink2),
                )
                .truncate(),
            )
            .on_hover_text(format!("Signal flow: {flow}"));
        });
        // the last panel clicked has the keys; a click picks a card
        if let Some(p) = press.filter(|_| !popup) {
            self.dev.focus = self.dev.row.contains(p);
            if inst_rect.contains(p) {
                self.dev.pick = Pick::Inst;
            } else if let Some(k) = cards.iter().position(|r| r.contains(p)) {
                self.dev.pick = Pick::Fx(k);
            }
        }
        if let Pick::Fx(k) = self.dev.pick {
            if k >= chain.len() {
                self.dev.pick = chain.len().checked_sub(1).map_or(Pick::Inst, Pick::Fx);
            }
        }
        self.dev.inst = inst_rect;
        self.dev.cards = cards;
        if open_effects {
            self.devices_open_effects();
        }
        self.grid_fx_window(ui.ctx(), idx, &chain, &mut ops);
        self.apply_fx_ops(idx, ops);
    }

    /// the instrument card, as compact as the effects: its picture, four key
    /// knobs and Advanced › for the full editor (the Instrument tab)
    pub(super) fn devices_inst_card(&mut self, ui: &mut egui::Ui, h: f32, idx: usize) -> Rect {
        let builtin = self
            .with(|s| {
                s.song
                    .instruments
                    .get(idx)
                    .map(|i| crate::theme::is_builtin(&i.kind))
            })
            .unwrap_or(false);
        // a built-in paints in the built-in skin; plugins and racks stay
        // neutral, like the effect cards
        let _scope = crate::theme::SkinScope::enter(crate::theme::instskin().filter(|_| builtin));
        self.devices_inst_card_body(ui, h, idx)
    }

    fn devices_inst_card_body(&mut self, ui: &mut egui::Ui, h: f32, idx: usize) -> Rect {
        let t = th();
        let skin = crate::theme::active_skin();
        let (name, tag) = self.with(|s| {
            s.song
                .instruments
                .get(idx)
                .map(|i| (i.name.clone(), super::params::type_tag(&i.kind)))
                .unwrap_or_default()
        });
        let w = if self.dev.inst_folded {
            SLIM
        } else if self.dev.tabbed {
            super::rackview::inst_card_w(self, idx)
        } else {
            self.dev.card_w
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
        let p = ui.painter_at(rect.expand(2.0));
        let accent = t.accent;
        if t.ember {
            crate::ember::card(&p, rect);
        } else {
            p.rect_filled(rect, 8, t.cart_top);
            p.rect_stroke(
                rect,
                8,
                Stroke::new(1.0, accent.gamma_multiply(0.55)),
                egui::StrokeKind::Inside,
            );
            if let Some(s) = skin {
                s.panel(&p, rect, 8.0);
            }
        }
        if let Some(s) = skin {
            s.stripe(&p, rect, 8.0);
        }
        let id = ui.make_persistent_id("dev_inst");
        if self.dev.inst_folded {
            let r = ui
                .interact(rect, id.with("unfold"), Sense::click())
                .on_hover_text(format!("{name}: click to show the instrument"));
            vertical_name(&p, rect, &name.to_uppercase(), t.ink);
            if r.clicked() {
                self.dev.inst_folded = false;
            }
            return rect;
        }
        // header: number and name, type, fold
        let head = Rect::from_min_size(rect.min, egui::vec2(w, HEAD));
        if t.ember {
            p.hline(
                (head.left() + 8.0)..=(head.right() - 8.0),
                head.bottom(),
                Stroke::new(1.0, crate::ember::LINE),
            );
        } else {
            p.rect_filled(
                head,
                egui::CornerRadius {
                    nw: 8,
                    ne: 8,
                    sw: 0,
                    se: 0,
                },
                skin.map_or(accent, |s| s.tint).gamma_multiply(0.22),
            );
        }
        // a Rack card folds from its side tab
        let fold = Rect::from_center_size(
            Pos2::new(head.right() - 14.0, head.center().y),
            egui::vec2(18.0, 18.0),
        )
        .translate(egui::vec2(if self.dev.tabbed { 22.0 } else { 0.0 }, 0.0));
        let tag_g = p.layout_no_wrap(
            tag.to_string(),
            crate::theme::prop(crate::theme::ts::SMALL),
            super::instpanel::tag_text_colour(tag),
        );
        let tag_r = Rect::from_min_size(
            Pos2::new(
                fold.left() - 8.0 - tag_g.size().x - 12.0,
                head.center().y - 8.0,
            ),
            egui::vec2(tag_g.size().x + 12.0, 16.0),
        );
        if let (Some(s), false) = (skin, tag.is_empty()) {
            let g = s.badge_galley(&p, tag);
            let r = Rect::from_min_max(
                Pos2::new(tag_r.right() - g.size().x - 14.0, tag_r.top()),
                tag_r.max + egui::vec2(0.0, 1.0),
            );
            s.paint_badge(&p, r, g);
        } else if !tag.is_empty() && t.ember {
            p.rect_filled(tag_r, 3, Color32::from_black_alpha(60));
            p.rect_stroke(
                tag_r,
                3,
                Stroke::new(1.0, t.line2),
                egui::StrokeKind::Inside,
            );
            p.text(
                tag_r.center(),
                egui::Align2::CENTER_CENTER,
                tag.to_uppercase(),
                crate::theme::label_font(crate::theme::ts::SMALL),
                t.acc_hot,
            );
        } else if !tag.is_empty() {
            p.rect_filled(tag_r, 8, super::instpanel::tag_colour(tag));
            p.galley(tag_r.center() - tag_g.size() * 0.5, tag_g, t.ink);
        }
        let name_r = Rect::from_min_max(
            head.min + egui::vec2(10.0, 0.0),
            Pos2::new(tag_r.left() - 6.0, head.bottom()),
        );
        let nf = if t.ember {
            crate::theme::label_font(crate::theme::ts::LARGE)
        } else {
            crate::theme::prop(crate::theme::ts::COMPACT)
        };
        let label =
            super::pattern::fit(&p, &format!("{:02X} {name}", idx + 1), &nf, name_r.width());
        p.text(
            name_r.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            nf,
            t.ink,
        );
        if !self.dev.tabbed {
            let fr = ui
                .interact(fold, id.with("fold"), Sense::click())
                .on_hover_text("fold the instrument card");
            chevron_icon(
                &p,
                fold.center(),
                false,
                if fr.hovered() { accent } else { t.ink2 },
            );
            if fr.clicked() {
                self.dev.inst_folded = true;
            }
        }
        ui.interact(name_r, id.with("name"), Sense::hover())
            .on_hover_text(format!(
                "{name}: the track's instrument · drop one from the Browser here to replace its sound \\
                 (its effects stay) · Advanced › opens every control"
            ));
        let main = Rect::from_min_max(
            Pos2::new(rect.left() + 8.0, head.bottom() + 6.0),
            rect.max - egui::vec2(8.0, 6.0),
        );
        let foot_h = 22.0;
        if self.dev.tabbed {
            self.rack_inst_body(ui, &p, main, foot_h, idx, tag);
            return rect;
        }
        let gh = (main.height() - CELL.y - foot_h - 8.0).clamp(0.0, 240.0);
        let g = Rect::from_min_size(main.min, egui::vec2(main.width(), gh));
        if gh >= 24.0 {
            match self.inst_card_viz(idx) {
                Some(v) => {
                    devcards::well(&p, g);
                    knob::draw_viz(&p, g.shrink(2.0), &v);
                }
                None => {
                    devcards::well(&p, g);
                    p.text(
                        g.center(),
                        egui::Align2::CENTER_CENTER,
                        tag,
                        crate::theme::prop(crate::theme::ts::LARGE),
                        t.ink2,
                    );
                }
            }
        }
        let kr = Rect::from_min_size(
            Pos2::new(main.left(), g.bottom() + 6.0),
            egui::vec2(main.width(), CELL.y),
        );
        inside(ui, egui::UiBuilder::new().max_rect(kr), |ui| {
            ui.set_clip_rect(kr.intersect(ui.clip_rect()));
            if self.inst_key_knobs(ui, idx, devcards::knobs_fit(kr.width()).min(4)) == 0 {
                let rack = self.with(|s| {
                    matches!(
                        s.song.instruments.get(idx).map(|i| &i.kind),
                        Some(InstrumentKind::Rack(_))
                    )
                });
                ui.label(
                    egui::RichText::new(if rack {
                        "layers: see the tree"
                    } else {
                        "its controls are in Advanced ›"
                    })
                    .color(t.ink2),
                );
            }
        });
        let b = Rect::from_min_max(
            Pos2::new(main.right() - 92.0, main.bottom() - foot_h),
            main.max,
        );
        if ui
            .put(b, egui::Button::new("Advanced ›").small())
            .on_hover_text("every control of this instrument (the Instrument tab's full editor)")
            .clicked()
        {
            self.instr = (idx + 1) as u8;
            self.ip.advanced = true;
            self.set_lower(1);
        }
        rect
    }

    /// one effect card: the same width for every kind, a header (power,
    /// name to drag, "…", remove), one graph, its 2-4 key knobs; "…" adds a
    /// column with the other parameters, presets and reset
    #[allow(clippy::too_many_arguments)]
    pub(super) fn devices_card(
        &mut self,
        ui: &mut egui::Ui,
        owner: usize,
        i: usize,
        d: &Device,
        key: u64,
        h: f32,
        n: usize,
        ops: &mut Vec<FxOp>,
    ) -> Rect {
        let t = th();
        let cw = self.dev.card_w;
        let open = !d.collapsed && self.dev.open.contains(&(owner, key));
        let w = if d.collapsed {
            SLIM
        } else if open {
            cw + MORE_W
        } else {
            cw
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
        let p = ui.painter_at(rect.expand(2.0));
        let tint = strip_fill(d.kind, d.enabled);
        let dragging = self.dev.drag == Some(i);
        if t.ember {
            crate::ember::card(&p, rect);
            if dragging {
                p.rect_stroke(
                    rect,
                    6,
                    Stroke::new(2.0, t.accent),
                    egui::StrokeKind::Inside,
                );
            }
        } else {
            p.rect_filled(rect, 8, t.cart_top);
            p.rect_stroke(
                rect,
                8,
                Stroke::new(
                    if dragging { 2.0 } else { 1.0 },
                    if dragging {
                        t.accent
                    } else {
                        tint.gamma_multiply(0.9)
                    },
                ),
                egui::StrokeKind::Inside,
            );
        }
        let id = ui.make_persistent_id(("dev_card", owner, i));
        let title = d.title();
        // a Rack card: power, remove and the menu live on its side tab
        let tabbed = self.dev.tabbed;
        let menu = |ui: &mut egui::Ui, ops: &mut Vec<FxOp>| {
            ui.label(egui::RichText::new(&title).strong());
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
            let pre = fx::presets(d.kind);
            if !pre.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("Presets").small());
                for (k, (pn, _)) in pre.iter().enumerate() {
                    if ui.button(*pn).clicked() {
                        ops.push(FxOp::Preset(i, k));
                        ui.close();
                    }
                }
            }
        };
        if d.collapsed {
            if !t.ember {
                p.rect_filled(rect.shrink(1.0), 7, tint.gamma_multiply(0.55));
            }
            let pw = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.top() + 14.0),
                egui::vec2(20.0, 20.0),
            );
            if !tabbed && power(ui, &p, pw, id.with("pw"), d.enabled) {
                ops.push(FxOp::Toggle(i));
            }
            let top = if tabbed {
                rect.top() + 8.0
            } else {
                pw.bottom() + 4.0
            };
            let body = Rect::from_min_max(Pos2::new(rect.left(), top), rect.max);
            vertical_name(&p, body, &title.to_uppercase(), t.ink);
            let r = ui
                .interact(body, id.with("open"), Sense::click_and_drag())
                .on_hover_text(format!("{title}: click to expand, drag to move"));
            if r.clicked() {
                ops.push(FxOp::Collapse(i));
            }
            if r.drag_started() {
                self.dev.drag = Some(i);
            }
            if tabbed {
                r.context_menu(|ui| self.rack_menu(ui, owner, Pick::Fx(i), ops));
            } else {
                r.context_menu(|ui| menu(ui, ops));
            }
            return rect;
        }
        // header: power · name (drag) · … · remove
        let head = Rect::from_min_size(rect.min, egui::vec2(w, HEAD));
        if t.ember {
            p.hline(
                (head.left() + 8.0)..=(head.right() - 8.0),
                head.bottom(),
                Stroke::new(1.0, crate::ember::LINE),
            );
        } else {
            p.rect_filled(
                head,
                egui::CornerRadius {
                    nw: 8,
                    ne: 8,
                    sw: 0,
                    se: 0,
                },
                tint.gamma_multiply(0.8),
            );
        }
        let pw = Rect::from_center_size(
            Pos2::new(head.left() + 14.0, head.center().y),
            egui::vec2(20.0, 20.0),
        );
        if !tabbed && power(ui, &p, pw, id.with("pw"), d.enabled) {
            ops.push(FxOp::Toggle(i));
        }
        let x_r = Rect::from_center_size(
            Pos2::new(head.right() - 13.0, head.center().y),
            egui::vec2(18.0, 18.0),
        );
        let more_r = Rect::from_center_size(
            Pos2::new(x_r.left() - 14.0, head.center().y),
            egui::vec2(22.0, 18.0),
        );
        let name_r = if tabbed {
            Rect::from_min_max(
                Pos2::new(head.left() + 10.0, head.top()),
                Pos2::new(head.right() - 8.0, head.bottom()),
            )
        } else {
            Rect::from_min_max(
                Pos2::new(pw.right() + 4.0, head.top()),
                Pos2::new(more_r.left() - 2.0, head.bottom()),
            )
        };
        let ink = if d.enabled { t.ink } else { t.ink2 };
        p.with_clip_rect(name_r).text(
            name_r.left_center(),
            egui::Align2::LEFT_CENTER,
            if d.enabled {
                title.clone()
            } else {
                format!("{title} (bypassed)")
            },
            if t.ember {
                crate::theme::label_font(crate::theme::ts::LARGE)
            } else {
                crate::theme::prop(crate::theme::ts::COMPACT)
            },
            ink,
        );
        let nr = ui
            .interact(name_r, id.with("name"), Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text(format!(
                "{title}: drag to move · double-click to collapse · right-click for more\n\
                 knobs: drag · Ctrl-drag fine · double-click default · right-click to automate or modulate"
            ));
        if nr.drag_started() {
            self.dev.drag = Some(i);
        }
        if nr.double_clicked() {
            ops.push(FxOp::Collapse(i));
        }
        if tabbed {
            nr.context_menu(|ui| self.rack_menu(ui, owner, Pick::Fx(i), ops));
        }
        if !tabbed {
            nr.context_menu(|ui| menu(ui, ops));
            let mr = ui
                .interact(more_r, id.with("more"), Sense::click())
                .on_hover_text(if open {
                    "hide the other parameters"
                } else {
                    "show the other parameters, presets and reset"
                });
            if open || mr.hovered() {
                p.rect_filled(
                    more_r,
                    4,
                    t.ink.gamma_multiply(if open { 0.16 } else { 0.10 }),
                );
            }
            p.text(
                more_r.center(),
                egui::Align2::CENTER_CENTER,
                "…",
                crate::theme::prop(crate::theme::ts::LARGE),
                if open || mr.hovered() { t.accent } else { ink },
            );
            if mr.clicked() {
                let k = (owner, key);
                if !self.dev.open.remove(&k) {
                    self.dev.open.insert(k);
                }
            }
            let xr = ui
                .interact(x_r, id.with("x"), Sense::click())
                .on_hover_text("remove (Ctrl+Z brings it back)");
            let xc = if xr.hovered() { t.brick } else { ink };
            let c = x_r.center();
            for s in [-1.0, 1.0] {
                p.line_segment(
                    [c + egui::vec2(-4.0, -4.0 * s), c + egui::vec2(4.0, 4.0 * s)],
                    Stroke::new(1.6, xc),
                );
            }
            if xr.clicked() {
                ops.push(FxOp::Remove(i));
            }
        }
        // the main column: graph, key knobs, a line about what's under "…"
        let main = Rect::from_min_max(
            Pos2::new(rect.left() + 8.0, head.bottom() + 6.0),
            Pos2::new(rect.left() + cw - 8.0, rect.bottom() - 6.0),
        );
        let foot_h = 16.0;
        let gh = (main.height() - CELL.y - foot_h - 8.0).clamp(0.0, 240.0);
        let g = Rect::from_min_size(main.min, egui::vec2(main.width(), gh));
        let mut keys = devcards::key_params(d);
        keys.truncate(devcards::knobs_fit(main.width()));
        let lay = egui::Layout::top_down(egui::Align::LEFT);
        inside(
            ui,
            egui::UiBuilder::new().max_rect(main).layout(lay),
            |ui| {
                ui.set_clip_rect(main.intersect(ui.clip_rect()));
                if !d.enabled {
                    ui.multiply_opacity(0.55);
                }
                if gh >= 24.0 {
                    self.devices_graph(ui, owner, i, d, g, ops);
                }
                if d.kind == FxKind::Plugin {
                    return;
                }
                let kr = Rect::from_min_size(
                    Pos2::new(main.left(), g.bottom() + 6.0),
                    egui::vec2(main.width(), CELL.y),
                );
                inside(ui, egui::UiBuilder::new().max_rect(kr), |ui| {
                    let pad = ((kr.width() - keys.len() as f32 * CELL.x) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        self.devices_knobs(ui, owner, i, d, &keys, ops);
                    });
                });
            },
        );
        let rest = devcards::more_params(d).len();
        if tabbed && (rest > 0 || d.kind == FxKind::Plugin) {
            // the footer opens and closes the other parameters
            let txt = if open {
                "‹ fewer".to_string()
            } else if d.kind == FxKind::Plugin {
                "parameters ›".to_string()
            } else {
                format!("+{rest} more ›")
            };
            let fr = Rect::from_min_size(
                Pos2::new(main.left(), main.bottom() - 15.0),
                egui::vec2(90.0, 15.0),
            );
            let r = ui
                .interact(fr, id.with("foot"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(if open {
                    "hide the other parameters"
                } else {
                    "show every parameter, presets and reset"
                });
            // screen readers (and the tests) find it by its text
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &txt));
            p.text(
                fr.left_bottom(),
                egui::Align2::LEFT_BOTTOM,
                txt,
                crate::theme::prop(crate::theme::ts::SMALL),
                if r.hovered() { t.accent } else { t.ink2 },
            );
            if r.clicked() {
                let k = (owner, key);
                if !self.dev.open.remove(&k) {
                    self.dev.open.insert(k);
                }
            }
        } else if !open && (rest > 0 || d.kind == FxKind::Plugin) {
            p.text(
                Pos2::new(main.left(), main.bottom()),
                egui::Align2::LEFT_BOTTOM,
                if d.kind == FxKind::Plugin {
                    "parameters under …".to_string()
                } else {
                    format!("+{rest} more under …")
                },
                crate::theme::prop(crate::theme::ts::SMALL),
                t.ink2.gamma_multiply(0.8),
            );
        }
        if open {
            let more = Rect::from_min_max(
                Pos2::new(rect.left() + cw + 4.0, head.bottom() + 6.0),
                rect.max - egui::vec2(8.0, 6.0),
            );
            p.vline(
                more.left() - 6.0,
                egui::Rangef::new(more.top(), more.bottom()),
                Stroke::new(1.0, t.border.gamma_multiply(0.7)),
            );
            inside(
                ui,
                egui::UiBuilder::new().max_rect(more).layout(lay),
                |ui| {
                    ui.set_clip_rect(more.intersect(ui.clip_rect()));
                    if !d.enabled {
                        ui.multiply_opacity(0.55);
                    }
                    self.devices_more(ui, owner, i, d, ops);
                },
            );
        }
        rect
    }

    /// the card's one graph
    fn devices_graph(
        &mut self,
        ui: &mut egui::Ui,
        owner: usize,
        i: usize,
        d: &Device,
        g: Rect,
        ops: &mut Vec<FxOp>,
    ) {
        let t = th();
        let p = ui.painter().clone();
        match d.kind {
            FxKind::Eq => self.devices_eq(ui, owner, i, d, g, ops),
            FxKind::Filter => {
                graph_bg(&p, g, true);
                let (mode, fc, res) = (d.real(0) as usize, d.real(1), d.real(2));
                freq_curve(&p, g, t.accent, true, |f| {
                    db_y(g, svf_db(mode, fc, res, f), 24.0)
                });
            }
            FxKind::Compressor => {
                comp_graph(
                    &p,
                    Rect::from_min_max(g.min, Pos2::new(g.right() - 14.0, g.bottom())),
                    d,
                );
                let gr = if self.dev.no_auto {
                    0.0
                } else {
                    self.with(|s| s.engine.comp_gr_db(owner, i))
                };
                devcards::gr_meter(&p, g, gr);
                if gr > 0.05 {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(40));
                }
                ui.interact(g, ui.make_persistent_id(("gr", owner, i)), Sense::hover())
                    .on_hover_text(format!(
                        "gain reduction {gr:.1} dB (the meter on the right)"
                    ));
            }
            FxKind::Delay => {
                let bpm = self.with(|s| s.song.bpm);
                delay_graph(&p, g, d, bpm);
            }
            FxKind::Reverb => reverb_graph(&p, g, d),
            FxKind::Chorus => devcards::chorus_graph(&p, g, d),
            FxKind::Drive => devcards::drive_graph(&p, g, d),
            FxKind::Bitcrusher => devcards::steps_graph(&p, g, d.real(0)),
            FxKind::Digital => devcards::steps_graph(&p, g, d.real(3)),
            FxKind::Phaser => devcards::phaser_graph(&p, g, d),
            FxKind::FreqShift => devcards::shift_graph(&p, g, d),
            FxKind::Resonator => devcards::ring_graph(&p, g, d),
            FxKind::Sub => devcards::sub_graph(&p, g),
            FxKind::Grid => {
                let nm = d.grid.as_ref().map_or(0, |gp| gp.modules.len());
                devcards::grid_graph(&p, g, nm);
                let b = Rect::from_center_size(g.center(), egui::vec2(120.0, 22.0));
                if ui
                    .put(b, egui::Button::new(format!("Edit patch… ({nm})")))
                    .on_hover_text("open the Grid FX patch editor")
                    .clicked()
                {
                    self.ip.grid_fx = Some((owner, i));
                }
            }
            FxKind::Plugin => self.devices_plugin_face(ui, owner, d, g),
            FxKind::Split => split_graph(&p, g, d),
        }
    }

    /// the "…" column: every other parameter as a row, then presets / reset
    pub(super) fn devices_more(
        &mut self,
        ui: &mut egui::Ui,
        owner: usize,
        i: usize,
        d: &Device,
        ops: &mut Vec<FxOp>,
    ) {
        let t = th();
        if d.kind == FxKind::Plugin {
            egui::ScrollArea::vertical()
                .id_salt(("dev_more", owner, i))
                .auto_shrink([false, false])
                .show(ui, |ui| self.plugin_fx_body(ui, owner, i, d));
            return;
        }
        let info = fx::params(d.kind);
        let rows = devcards::more_params(d);
        let list_h = (ui.available_height() - 28.0).max(ROW_H);
        egui::ScrollArea::vertical()
            .id_salt(("dev_more", owner, i))
            .max_height(list_h)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for k in rows {
                    let pi = &info[k];
                    let v0 = d.params.get(k).copied().unwrap_or(0.0);
                    if !pi.choices.is_empty() {
                        let cur = d.real(k) as usize;
                        ui.horizontal(|ui| {
                            ui.set_height(ROW_H);
                            ui.add_sized(
                                egui::vec2(76.0, ROW_H),
                                egui::Label::new(
                                    egui::RichText::new(pi.name)
                                        .size(crate::theme::ts::px(crate::theme::ts::LABEL))
                                        .color(t.ink),
                                )
                                .truncate(),
                            );
                            egui::ComboBox::from_id_salt(("dev_ch", owner, i, k))
                                .selected_text(pi.choices.get(cur).copied().unwrap_or(""))
                                .width((ui.available_width() - 4.0).max(60.0))
                                .show_ui(ui, |ui| {
                                    for (ci, cn) in pi.choices.iter().enumerate() {
                                        if ui.selectable_label(ci == cur, *cn).clicked()
                                            && ci != cur
                                        {
                                            ops.push(FxOp::Param(
                                                i,
                                                k,
                                                pi.to_norm(ci as f32),
                                                true,
                                            ));
                                        }
                                    }
                                });
                        });
                        continue;
                    }
                    let (resp, out) = devcards::param_row(
                        ui,
                        ui.make_persistent_id(("dev_row", owner, i, k)),
                        pi.name,
                        v0,
                        &pi.format(v0),
                        pi.to_norm(pi.def),
                    );
                    if !self.dev.no_auto {
                        let tid = crate::automation::Target::Fx(owner, i, k);
                        let mut lane = false;
                        self.mod_slider(ui, &resp, &tid.id(), v0, Some(&mut lane));
                        if lane {
                            self.add_lane(tid);
                        }
                    }
                    if let Some((v, first)) = out {
                        ops.push(FxOp::Param(i, k, v, first));
                    }
                }
            });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
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
            if ui
                .button("Reset")
                .on_hover_text("every parameter back to its default (one undo step)")
                .clicked()
            {
                for (k, pi) in info.iter().enumerate() {
                    ops.push(FxOp::Param(i, k, pi.to_norm(pi.def), k == 0));
                }
            }
        });
    }

    /// knobs for parameters `keys` of device `i`, wrapped
    fn devices_knobs(
        &mut self,
        ui: &mut egui::Ui,
        owner: usize,
        i: usize,
        d: &Device,
        keys: &[usize],
        ops: &mut Vec<FxOp>,
    ) {
        let info = fx::params(d.kind);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            for &k in keys {
                let Some(pi) = info.get(k) else {
                    continue;
                };
                let v0 = d.params.get(k).copied().unwrap_or(0.0);
                let mut v = v0;
                let out = knob::knob(
                    ui,
                    ui.make_persistent_id(("dev_knob", owner, i, k)),
                    &mut v,
                    KnobArgs {
                        caption: None,
                        name: pi.name,
                        value_text: pi.format(v0),
                        default: pi.to_norm(pi.def),
                        steps: 0.0,
                        enabled: true,
                        highlight: false,
                        badge: None,
                        locked: None,
                        tip: "",
                        rings: &[],
                        live: None,
                        auto_led: None,
                        mod_dot: None,
                    },
                );
                if !self.dev.no_auto {
                    let tid = crate::automation::Target::Fx(owner, i, k);
                    let mut lane = false;
                    self.mod_slider(ui, &out.resp, &tid.id(), v0, Some(&mut lane));
                    if lane {
                        self.add_lane(tid);
                    }
                }
                if out.changed {
                    let first = out.resp.drag_started() || !out.resp.dragged();
                    ops.push(FxOp::Param(i, k, v, first));
                }
            }
        });
    }

    /// EQ: the response curve with a draggable dot per band (frequency and
    /// gain; double-click a dot for 0 dB). The gains are the card's knobs.
    fn devices_eq(
        &mut self,
        ui: &mut egui::Ui,
        owner: usize,
        i: usize,
        d: &Device,
        r: Rect,
        ops: &mut Vec<FxOp>,
    ) {
        let t = th();
        let sr = self.with(|s| s.engine.sr).max(8000.0);
        {
            let p = ui.painter();
            graph_bg(p, r, true);
            // the four bands' coefficients once, then |H| per point
            let real: Vec<f32> = (0..fx::EQ.len()).map(|k| d.real(k)).collect();
            let bands = fx::eq_bands(&real, sr);
            freq_curve(p, r, t.accent, true, |f| {
                let m: f32 = bands.iter().map(|b| b.mag(f, sr)).product();
                db_y(r, 20.0 * m.max(1e-6).log10(), 18.0)
            });
        }
        // (freq param, gain param) per band
        const BANDS: [(usize, usize, &str); 4] = [
            (0, 1, "Low"),
            (2, 3, "Mid 1"),
            (5, 6, "Mid 2"),
            (8, 9, "High"),
        ];
        const COLS: [u32; 4] = [0xE0A060, 0x7FD08A, 0x6CB6E8, 0xC88AE0];
        for (b, (fi, gi, name)) in BANDS.iter().enumerate() {
            let c = Pos2::new(hz_x(r, d.real(*fi)), db_y(r, d.real(*gi), 18.0));
            let resp = ui
                .interact(
                    Rect::from_center_size(c, egui::vec2(16.0, 16.0)),
                    ui.make_persistent_id(("eq_dot", owner, i, b)),
                    Sense::click_and_drag(),
                )
                .on_hover_text(format!(
                    "{name}: {} · {} (drag; double-click = 0 dB)",
                    fx::EQ[*fi].format(d.params[*fi]),
                    fx::EQ[*gi].format(d.params[*gi])
                ));
            let col = hex(COLS[b]);
            let hot = resp.hovered() || resp.dragged();
            let p = ui.painter();
            if t.ember {
                // Slate: numbered rings, the touched one with a halo
                if hot {
                    crate::ember::glow_dot(p, c, 7.0, t.accent);
                }
                p.circle_filled(c, 6.0, hex(0x141619));
                p.circle_stroke(
                    c,
                    6.0,
                    Stroke::new(1.4, if hot { t.acc_hot } else { t.ink2 }),
                );
                p.text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", b + 1),
                    crate::theme::label_font(crate::theme::ts::TINY),
                    if hot { t.acc_hot } else { t.ink },
                );
            } else {
                p.circle_filled(c, if hot { 6.0 } else { 4.5 }, col);
                if hot {
                    p.circle_stroke(c, 8.0, Stroke::new(1.2, col));
                }
            }
            if resp.double_clicked() {
                ops.push(FxOp::Param(i, *gi, fx::EQ[*gi].to_norm(0.0), true));
            } else if resp.dragged() {
                if let Some(q) = resp.interact_pointer_pos() {
                    let first = resp.drag_started();
                    let f = x_hz(r, q.x);
                    let g = -((q.y - r.center().y) / (r.height() * 0.5 - 3.0)) * 18.0;
                    ops.push(FxOp::Param(i, *fi, fx::EQ[*fi].to_norm(f), first));
                    ops.push(FxOp::Param(i, *gi, fx::EQ[*gi].to_norm(g), false));
                }
            }
        }
    }

    /// a plugin effect's face: its editor thumbnail (or a card with its
    /// name) and an Open button for its own editor
    fn devices_plugin_face(&mut self, ui: &mut egui::Ui, owner: usize, d: &Device, g: Rect) {
        let t = th();
        let Some(pp) = d.plugin.as_deref() else {
            return;
        };
        let found = self.br.items.iter().find_map(|it| match &it.src {
            crate::browser::Src::Plugin(f) if f.desc.id == pp.id && f.format == pp.format => {
                Some(f.clone())
            }
            _ => None,
        });
        let tex = found.and_then(|f| self.br_thumb(ui.ctx(), &f));
        let main = Rect::from_min_max(g.min, Pos2::new(g.right(), ui.max_rect().bottom() - 46.0));
        let p = ui.painter();
        if let Some(tex) = tex {
            let s = tex.size_vec2();
            let scale = (main.width() / s.x).min(main.height() / s.y).min(1.0);
            let r = Rect::from_center_size(main.center(), s * scale);
            p.image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            devcards::well(p, main);
            p.text(
                main.center(),
                egui::Align2::CENTER_CENTER,
                &pp.name,
                crate::theme::prop(crate::theme::ts::LARGE),
                t.ink2,
            );
        }
        let b = Rect::from_center_size(
            Pos2::new(main.center().x, main.bottom() + 18.0),
            egui::vec2(110.0, 24.0),
        );
        let r = ui
            .put(b, egui::Button::new("Open"))
            .on_hover_text(format!("open {}'s own editor", pp.name));
        if r.clicked() {
            self.devices_plugin_editor(owner, d.uid, &pp.name);
        }
    }

    /// open a plugin effect's own editor window
    fn devices_plugin_editor(&mut self, owner: usize, uid: u32, name: &str) {
        let title = format!("{name} – ChipWave");
        let r = self.with(|s| {
            let k = s.engine.fx_plugins.primary(owner, uid)?;
            Some(s.engine.fx_plugins.slots[k].inst.open_editor(&title))
        });
        match r {
            Some(Err(e)) => self.status = format!("Plugin editor: {e}"),
            None => self.status = format!("{name} loads when a note plays it"),
            _ => {}
        }
    }
}

/// compressor: input -> output level (dB) with the threshold knee
fn comp_graph(p: &egui::Painter, r: Rect, d: &Device) {
    let t = th();
    graph_bg(p, r, false);
    let (thr, ratio, makeup) = (d.real(0), d.real(1), d.real(4));
    let lo = -48.0;
    let to = |i: f32, o: f32| {
        Pos2::new(
            r.left() + (i - lo) / -lo * r.width(),
            r.bottom() - ((o - lo) / -lo).clamp(0.0, 1.0) * r.height(),
        )
    };
    p.line_segment(
        [to(lo, lo), to(0.0, 0.0)],
        Stroke::new(1.0, t.ink2.gamma_multiply(0.35)),
    );
    let pts: Vec<Pos2> = (0..=48)
        .map(|k| {
            let i = lo + k as f32;
            let o = if i > thr { thr + (i - thr) / ratio } else { i } + makeup;
            to(i, o)
        })
        .collect();
    acc_line(p, pts, 1.8);
    let tx = to(thr, thr).x;
    p.vline(
        tx,
        r.y_range(),
        Stroke::new(1.0, t.accent.gamma_multiply(0.4)),
    );
    p.text(
        r.left_top() + egui::vec2(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        format!("{:.0} dB · {:.1}:1", thr, ratio),
        crate::theme::prop(crate::theme::ts::TINY),
        t.ink2,
    );
}

/// delay: the echo taps fading with the feedback
fn delay_graph(p: &egui::Painter, r: Rect, d: &Device, bpm: f32) {
    let t = th();
    graph_bg(p, r, false);
    let sync = d.real(0) as usize;
    let beats = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0];
    let secs = if sync == 0 {
        d.real(1) / 1000.0
    } else {
        60.0 / bpm.max(20.0) * beats[(sync - 1).min(5)]
    };
    let (fb, mix, pp) = (d.real(2), d.real(6), d.real(3) >= 0.5);
    let span = 2.0f32;
    let x = |s: f32| r.left() + (s / span).min(1.0) * r.width();
    p.vline(
        x(0.0) + 2.0,
        r.y_range(),
        Stroke::new(3.0, t.ink2.gamma_multiply(0.6)),
    );
    let mut a = mix;
    let mut k = 1;
    while (k as f32) * secs < span && a > 0.02 && k < 40 {
        let h = a * (r.height() - 8.0);
        let left = pp && k % 2 == 0;
        let xx = x(k as f32 * secs);
        p.line_segment(
            [
                Pos2::new(xx, r.bottom() - 2.0),
                Pos2::new(xx, r.bottom() - 2.0 - h),
            ],
            Stroke::new(3.0, if left { t.fx } else { t.accent }),
        );
        a *= fb;
        k += 1;
    }
    p.text(
        r.right_top() + egui::vec2(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        format!(
            "{:.0} ms{}",
            secs * 1000.0,
            if pp { " · ping-pong" } else { "" }
        ),
        crate::theme::prop(crate::theme::ts::TINY),
        t.ink2,
    );
}

/// reverb: pre-delay, then the decay (length from the size)
fn reverb_graph(p: &egui::Painter, r: Rect, d: &Device) {
    let t = th();
    graph_bg(p, r, false);
    let (size, damp, pre, mix) = (d.real(0), d.real(1), d.real(2) / 1000.0, d.real(4));
    let decay = 0.4 + size * size * 5.6; // seconds, roughly
    let span = 6.0f32;
    let x = |s: f32| r.left() + (s / span).min(1.0) * r.width();
    let n = 80;
    let mut pts = vec![Pos2::new(x(0.0), r.bottom())];
    for k in 0..=n {
        let s = pre + k as f32 / n as f32 * (span - pre);
        let env = (-(s - pre) / decay * (3.0 + damp * 2.0)).exp() * mix.max(0.05);
        pts.push(Pos2::new(x(s), r.bottom() - env * (r.height() - 6.0)));
    }
    for w in pts.windows(2) {
        p.add(egui::Shape::convex_polygon(
            vec![
                w[0],
                w[1],
                Pos2::new(w[1].x, r.bottom()),
                Pos2::new(w[0].x, r.bottom()),
            ],
            t.accent.gamma_multiply(0.2),
            Stroke::NONE,
        ));
    }
    acc_line(p, pts, 1.6);
    p.text(
        r.right_top() + egui::vec2(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        format!("~{decay:.1} s"),
        crate::theme::prop(crate::theme::ts::TINY),
        t.ink2,
    );
}

/// an accent curve (Ember: acc-hot over a bloom)
/// a Split: the sound fanning out into its paths and back together
fn split_graph(p: &egui::Painter, r: Rect, d: &Device) {
    let t = th();
    devcards::well(p, r);
    let n = d.paths.len().max(1);
    let (a, b) = (
        Pos2::new(r.left() + 14.0, r.center().y),
        Pos2::new(r.right() - 14.0, r.center().y),
    );
    for (k, path) in d.paths.iter().enumerate() {
        let y = r.top() + r.height() * (k as f32 + 0.5) / n as f32;
        let col = if path.on {
            t.accent
        } else {
            t.ink2.gamma_multiply(0.5)
        };
        let l = Pos2::new(r.left() + r.width() * 0.3, y);
        let rr = Pos2::new(r.right() - r.width() * 0.3, y);
        p.line_segment([a, l], Stroke::new(1.5, col));
        p.line_segment([l, rr], Stroke::new(1.5, col));
        p.line_segment([rr, b], Stroke::new(1.5, col));
        p.text(
            Pos2::new(r.center().x, y - 2.0),
            egui::Align2::CENTER_BOTTOM,
            format!(
                "{} · {} device{}",
                super::fxview::path_letter(k),
                path.devices.len(),
                if path.devices.len() == 1 { "" } else { "s" }
            ),
            crate::theme::prop(crate::theme::ts::SMALL),
            t.ink,
        );
    }
    p.circle_filled(a, 3.0, t.ink);
    p.circle_filled(b, 3.0, t.ink);
}

fn acc_line(p: &egui::Painter, pts: Vec<Pos2>, w: f32) {
    let t = th();
    if t.ember {
        crate::ember::glow_line(p, &pts, w, t.acc_hot);
    } else {
        p.add(egui::Shape::line(pts, Stroke::new(w, t.accent)));
    }
}

/// the insertion marker: an accent bar with end caps
pub(crate) fn paint_marker(p: &egui::Painter, x: f32, y: egui::Rangef) {
    let a = th().accent;
    p.vline(x, y, Stroke::new(3.0, a));
    for (yy, s) in [(y.min, 1.0), (y.max, -1.0)] {
        p.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(x - 6.0, yy),
                Pos2::new(x + 6.0, yy),
                Pos2::new(x, yy + 7.0 * s),
            ],
            a,
            Stroke::NONE,
        ));
    }
}

/// the same marker lying down (Nested Device Chains' tree rows)
pub(crate) fn paint_marker_h(p: &egui::Painter, y: f32, x: egui::Rangef) {
    let a = th().accent;
    p.hline(x, y, Stroke::new(3.0, a));
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(x.min, y - 6.0),
            Pos2::new(x.min, y + 6.0),
            Pos2::new(x.min + 7.0, y),
        ],
        a,
        Stroke::NONE,
    ));
}

/// where a Browser effect dropped at `p` lands in the shown chain (card
/// row: by x; Nested Device Chains' tree: by y)
pub(crate) fn drop_index(vertical: bool, cards: &[Rect], p: Pos2) -> usize {
    if vertical {
        cards
            .iter()
            .position(|r| p.y < r.center().y)
            .unwrap_or(cards.len())
    } else {
        insert_index(cards, p.x)
    }
}

/// a power button; true when clicked
pub(super) fn power(ui: &egui::Ui, p: &egui::Painter, r: Rect, id: egui::Id, on: bool) -> bool {
    let t = th();
    let resp =
        ui.interact(r, id, Sense::click())
            .on_hover_text(if on { "bypass" } else { "enable" });
    let c = r.center();
    let col = if on { t.accent } else { t.ink2 };
    if resp.hovered() {
        p.circle_filled(c, 10.0, t.ink.gamma_multiply(0.12));
    }
    if t.ember {
        // Ember: the card's LED (lit = on)
        crate::ember::led(p, c, 4.0, on);
        return resp.clicked();
    }
    let rad = 5.5;
    let pts: Vec<Pos2> = (0..=20)
        .map(|k| {
            let a = -std::f32::consts::FRAC_PI_2
                + 0.6
                + k as f32 / 20.0 * (std::f32::consts::TAU - 1.2);
            Pos2::new(c.x + rad * a.cos(), c.y + rad * a.sin())
        })
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.6, col)));
    p.line_segment(
        [c - egui::vec2(0.0, 7.0), c - egui::vec2(0.0, 1.5)],
        Stroke::new(1.6, col),
    );
    resp.clicked()
}

/// a small chevron (`<` when folding, `>` when opening)
fn chevron_icon(p: &egui::Painter, c: Pos2, right: bool, col: Color32) {
    let s = if right { 1.0 } else { -1.0 };
    p.add(egui::Shape::line(
        vec![
            c + egui::vec2(-2.5 * s, -4.0),
            c + egui::vec2(2.5 * s, 0.0),
            c + egui::vec2(-2.5 * s, 4.0),
        ],
        Stroke::new(1.6, col),
    ));
}

/// the signal arrow between cards
fn chevron(ui: &mut egui::Ui, h: f32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(GAP, h), Sense::hover());
    chevron_icon(
        ui.painter(),
        r.center(),
        true,
        th().ink2.gamma_multiply(0.7),
    );
}

/// a name written bottom to top in a slim card
fn vertical_name(p: &egui::Painter, r: Rect, s: &str, col: Color32) {
    let g = p.layout_no_wrap(
        s.to_string(),
        crate::theme::prop(crate::theme::ts::COMPACT),
        col,
    );
    let pos = Pos2::new(r.center().x - g.size().y * 0.5, r.bottom() - 10.0);
    p.add(egui::epaint::TextShape::new(pos, g, col).with_angle(-std::f32::consts::FRAC_PI_2));
}

/// the `+` card at the end of the chain; true when clicked
/// the + drop slot's width
fn plus_w(empty: bool) -> f32 {
    if empty {
        200.0
    } else {
        56.0
    }
}

/// `fill`: an empty chain's box takes the rest of the row (the empty
/// track's "Drag an instrument here" look, Del (postmanpat) 23 Oct 2026)
fn plus_card(ui: &mut egui::Ui, h: f32, empty: bool, fill: f32) -> bool {
    let t = th();
    let w = if empty { fill } else { plus_w(false) };
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
    let resp = resp.on_hover_text("add an effect: opens Browser › Effects (or drag one here)");
    let p = ui.painter();
    let col = if resp.hovered() {
        t.accent
    } else {
        t.ink2.gamma_multiply(0.7)
    };
    if empty {
        // the empty Rack's drop box: a rounded outline, a title and a hint
        let r2 = r.shrink2(egui::vec2(2.0, 6.0));
        p.rect_stroke(r2, 8, Stroke::new(1.5, col), egui::StrokeKind::Inside);
        let wrap = (r2.width() - 20.0).max(60.0);
        let title = p.layout(
            "Drag an effect here".into(),
            crate::theme::prop(crate::theme::ts::HEADING),
            if resp.hovered() { t.accent } else { t.ink },
            wrap,
        );
        let hint = p.layout(
            "No effects yet: drop one from Browser › Effects, or click to open it".into(),
            crate::theme::prop(crate::theme::ts::COMPACT),
            t.ink2,
            wrap,
        );
        let tot = title.size().y + 6.0 + hint.size().y;
        let y0 = r2.center().y - tot / 2.0;
        p.galley(
            Pos2::new(r2.center().x - title.size().x / 2.0, y0),
            title,
            t.ink,
        );
        p.galley(
            Pos2::new(
                r2.center().x - hint.size().x / 2.0,
                y0 + tot - hint.size().y,
            ),
            hint,
            t.ink2,
        );
        return resp.clicked();
    }
    // dashed outline
    let r2 = r.shrink(2.0);
    for (a, b) in [
        (r2.left_top(), r2.right_top()),
        (r2.right_top(), r2.right_bottom()),
        (r2.right_bottom(), r2.left_bottom()),
        (r2.left_bottom(), r2.left_top()),
    ] {
        p.extend(egui::Shape::dashed_line(
            &[a, b],
            Stroke::new(1.2, col),
            6.0,
            4.0,
        ));
    }
    let c = r.center();
    p.line_segment(
        [c - egui::vec2(9.0, 0.0), c + egui::vec2(9.0, 0.0)],
        Stroke::new(2.2, col),
    );
    p.line_segment(
        [c - egui::vec2(0.0, 9.0), c + egui::vec2(0.0, 9.0)],
        Stroke::new(2.2, col),
    );
    resp.clicked()
}

/// draw `f` in a child ui at the builder's rect without moving the parent's
/// cursor (the card already allocated its whole rect)
fn inside<R>(ui: &mut egui::Ui, b: egui::UiBuilder, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut c = ui.new_child(b);
    f(&mut c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_index_and_marker_follow_the_card_centres() {
        let inst = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::vec2(400.0, 200.0));
        let cards = [
            Rect::from_min_size(Pos2::new(420.0, 0.0), egui::vec2(200.0, 200.0)),
            Rect::from_min_size(Pos2::new(640.0, 0.0), egui::vec2(100.0, 200.0)),
        ];
        assert_eq!(insert_index(&cards, 300.0), 0);
        assert_eq!(insert_index(&cards, 519.0), 0);
        assert_eq!(insert_index(&cards, 521.0), 1);
        assert_eq!(insert_index(&cards, 700.0), 2);
        assert_eq!(marker_x(inst, &cards, 1), 635.0);
        assert_eq!(marker_x(inst, &cards, 2), 745.0);
        assert_eq!(marker_x(inst, &[], 0), 409.0);
    }

    #[test]
    fn filter_curve_matches_the_mode() {
        // LP passes lows, HP passes highs, both -3 dB-ish at the cutoff with no resonance
        assert!(svf_db(0, 1000.0, 0.0, 50.0).abs() < 0.1);
        assert!(svf_db(0, 1000.0, 0.0, 10000.0) < -30.0);
        assert!(svf_db(1, 1000.0, 0.0, 10000.0).abs() < 0.2);
        assert!(svf_db(1, 1000.0, 0.0, 50.0) < -40.0);
        assert!(svf_db(0, 1000.0, 0.9, 1000.0) > 10.0, "resonance peak");
    }
}
