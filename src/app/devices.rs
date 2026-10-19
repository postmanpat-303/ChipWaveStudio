//! Devices (0.2.4, Del's pick from the MorningDAW reference): a lower-panel
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
use super::fxview::{strip_fill, FxOp};
use super::knob::{self, KnobArgs, CELL};
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
    /// the EQ band the curve's knobs show, per device index
    pub(crate) eq_band: Vec<(usize, usize)>,
    /// a Browser instrument dropped on the instrument card: keep the
    /// slot's FX chain when its sound is replaced
    pub(crate) keep_fx: Option<(u8, Vec<Device>)>,
    /// screenshot: hold a Browser drag over device card N
    pub(crate) shot_at: Option<usize>,
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
            eq_band: vec![],
            keep_fx: None,
            shot_at: None,
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

/// width of an expanded device card
fn card_w(d: &Device) -> f32 {
    match d.kind {
        FxKind::Eq => 300.0,
        FxKind::Plugin => 250.0,
        FxKind::Compressor | FxKind::Reverb | FxKind::Delay | FxKind::Filter => 262.0,
        k => {
            // knob grid, 4 per row
            let n = fx::params(k).len();
            (n.min(4) as f32 * CELL.x + 24.0).max(200.0)
        }
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
        let t = th();
        let n = self.with(|s| s.song.instruments.len());
        if n > 0 && (self.instr == 0 || self.instr as usize > n) {
            self.instr = 1;
        }
        let idx = (self.instr as usize).wrapping_sub(1);
        let chain = self.with(|s| s.song.instruments.get(idx).map(|i| i.fx.clone()));
        // ---- header
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Devices").strong());
            ui.label(
                egui::RichText::new(format!("· {}", self.devices_target_label()))
                    .color(t.ink2),
            );
            if let Some(c) = &chain {
                ui.label(
                    egui::RichText::new(format!(
                        "· {} effect{}",
                        c.len(),
                        if c.len() == 1 { "" } else { "s" }
                    ))
                    .small()
                    .color(t.ink2),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(
                            "drag effects in from Browser › Effects · drag a title to reorder · right-click a card for more",
                        )
                        .small()
                        .color(t.ink2),
                    )
                    .truncate(),
                );
            });
        });
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
        let h = (ui.available_height() - 4.0).max(120.0);
        let mut ops: Vec<FxOp> = vec![];
        let mut cards: Vec<Rect> = vec![];
        let mut inst_rect = Rect::NOTHING;
        let mut open_effects = false;
        let pointer = ui.ctx().pointer_latest_pos();
        let released = ui.input(|i| i.pointer.any_released());
        let row_top = ui.cursor().top();
        let row_w = ui.available_width();
        egui::ScrollArea::horizontal()
            .id_salt("devices_row")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.add_space(4.0);
                    inst_rect = self.devices_inst_card(ui, h, row_w);
                    chevron(ui, h);
                    for (i, d) in chain.iter().enumerate() {
                        let r = self.devices_card(ui, idx, i, d, h, chain.len(), &mut ops);
                        cards.push(r);
                        chevron(ui, h);
                    }
                    if plus_card(ui, h, chain.is_empty()) {
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
        self.dev.inst = inst_rect;
        self.dev.cards = cards;
        if open_effects {
            self.devices_open_effects();
        }
        self.grid_fx_window(ui.ctx(), idx, &chain, &mut ops);
        self.apply_fx_ops(idx, ops);
    }

    /// the instrument card: the lower-panel instrument editor in a frame
    fn devices_inst_card(&mut self, ui: &mut egui::Ui, h: f32, row_w: f32) -> Rect {
        let t = th();
        let (name, kind) = self.with(|s| {
            s.song
                .instruments
                .get((self.instr as usize).wrapping_sub(1))
                .map(|i| (i.name.clone(), super::params::type_tag(&i.kind)))
                .unwrap_or_default()
        });
        // about 40 % of the row, so a few effect cards fit next to it
        let w = if self.dev.inst_folded {
            SLIM
        } else {
            (row_w * 0.42).clamp(380.0, 760.0)
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
        let p = ui.painter_at(rect.expand(2.0));
        let accent = t.accent;
        p.rect_filled(rect, 8, t.cart_top);
        p.rect_stroke(
            rect,
            8,
            Stroke::new(1.0, accent.gamma_multiply(0.55)),
            egui::StrokeKind::Inside,
        );
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
        // header: name, type, fold
        let head = Rect::from_min_size(rect.min, egui::vec2(w, HEAD));
        p.rect_filled(
            head,
            egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: 0,
                se: 0,
            },
            accent.gamma_multiply(0.22),
        );
        p.text(
            head.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("INSTRUMENT · {:02X} {name}", self.instr),
            FontId::proportional(12.0),
            t.ink,
        );
        let _ = kind;
        let fold = Rect::from_center_size(
            Pos2::new(head.right() - 14.0, head.center().y),
            egui::vec2(18.0, 18.0),
        );
        let fr = ui
            .interact(fold, id.with("fold"), Sense::click())
            .on_hover_text("fold the instrument card (more room for effects)");
        chevron_icon(
            &p,
            fold.center(),
            false,
            if fr.hovered() { accent } else { t.ink2 },
        );
        if fr.clicked() {
            self.dev.inst_folded = true;
        }
        let body = Rect::from_min_max(
            Pos2::new(rect.left() + 4.0, head.bottom() + 2.0),
            rect.max - egui::vec2(4.0, 4.0),
        );
        let lay = egui::Layout::top_down(egui::Align::LEFT);
        ui.scope_builder(egui::UiBuilder::new().max_rect(body).layout(lay), |ui| {
            ui.set_clip_rect(body.intersect(ui.clip_rect()));
            self.instrument_view(ui);
        });
        rect
    }

    /// one effect card
    #[allow(clippy::too_many_arguments)]
    fn devices_card(
        &mut self,
        ui: &mut egui::Ui,
        owner: usize,
        i: usize,
        d: &Device,
        h: f32,
        n: usize,
        ops: &mut Vec<FxOp>,
    ) -> Rect {
        let t = th();
        let w = if d.collapsed { SLIM } else { card_w(d) };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
        let p = ui.painter_at(rect.expand(2.0));
        let tint = strip_fill(d.kind, d.enabled);
        let dragging = self.dev.drag == Some(i);
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
        let id = ui.make_persistent_id(("dev_card", owner, i));
        let title = d.title();
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
            p.rect_filled(rect.shrink(1.0), 7, tint.gamma_multiply(0.55));
            let pw = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.top() + 14.0),
                egui::vec2(20.0, 20.0),
            );
            if power(ui, &p, pw, id.with("pw"), d.enabled) {
                ops.push(FxOp::Toggle(i));
            }
            let body = Rect::from_min_max(Pos2::new(rect.left(), pw.bottom() + 4.0), rect.max);
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
            r.context_menu(|ui| menu(ui, ops));
            return rect;
        }
        // header: power · name (drag) · collapse · remove
        let head = Rect::from_min_size(rect.min, egui::vec2(w, HEAD));
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
        let pw = Rect::from_center_size(
            Pos2::new(head.left() + 14.0, head.center().y),
            egui::vec2(20.0, 20.0),
        );
        if power(ui, &p, pw, id.with("pw"), d.enabled) {
            ops.push(FxOp::Toggle(i));
        }
        let x_r = Rect::from_center_size(
            Pos2::new(head.right() - 13.0, head.center().y),
            egui::vec2(18.0, 18.0),
        );
        let col_r = x_r.translate(egui::vec2(-20.0, 0.0));
        let name_r = Rect::from_min_max(
            Pos2::new(pw.right() + 4.0, head.top()),
            Pos2::new(col_r.left() - 2.0, head.bottom()),
        );
        let ink = if d.enabled { t.ink } else { t.ink2 };
        p.with_clip_rect(name_r).text(
            name_r.left_center(),
            egui::Align2::LEFT_CENTER,
            if d.enabled {
                title.clone()
            } else {
                format!("{title} (bypassed)")
            },
            FontId::proportional(12.5),
            ink,
        );
        let nr = ui
            .interact(name_r, id.with("name"), Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text(format!(
                "{title}: drag to move · double-click to collapse · right-click for more"
            ));
        if nr.drag_started() {
            self.dev.drag = Some(i);
        }
        if nr.double_clicked() {
            ops.push(FxOp::Collapse(i));
        }
        nr.context_menu(|ui| menu(ui, ops));
        let cr = ui
            .interact(col_r, id.with("col"), Sense::click())
            .on_hover_text("collapse");
        chevron_icon(
            &p,
            col_r.center(),
            false,
            if cr.hovered() { t.accent } else { ink },
        );
        if cr.clicked() {
            ops.push(FxOp::Collapse(i));
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
        // body
        let body = Rect::from_min_max(
            Pos2::new(rect.left() + 8.0, head.bottom() + 6.0),
            rect.max - egui::vec2(8.0, 6.0),
        );
        let lay = egui::Layout::top_down(egui::Align::LEFT);
        ui.scope_builder(egui::UiBuilder::new().max_rect(body).layout(lay), |ui| {
            ui.set_clip_rect(body.intersect(ui.clip_rect()));
            if !d.enabled {
                ui.multiply_opacity(0.55);
            }
            match d.kind {
                FxKind::Plugin => self.devices_plugin_body(ui, owner, i, d),
                FxKind::Grid => {
                    let nm = d.grid.as_ref().map_or(0, |g| g.modules.len());
                    if ui
                        .button(format!("Edit patch… ({nm} modules)"))
                        .on_hover_text("open the Grid FX patch editor")
                        .clicked()
                    {
                        self.ip.grid_fx = Some((owner, i));
                    }
                    self.devices_knobs(ui, owner, i, d, &[0, 1, 2, 3, 8], ops);
                }
                FxKind::Eq => self.devices_eq(ui, owner, i, d, ops),
                FxKind::Compressor => {
                    let r = graph(ui, 92.0);
                    comp_graph(ui.painter(), r, d);
                    self.devices_knobs(ui, owner, i, d, &[0, 1, 2, 3, 4], ops);
                }
                FxKind::Filter => {
                    let r = graph(ui, 80.0);
                    let p = ui.painter();
                    graph_bg(p, r, true);
                    let (mode, fc, res) = (d.real(0) as usize, d.real(1), d.real(2));
                    freq_curve(p, r, t.accent, true, |f| {
                        db_y(r, svf_db(mode, fc, res, f), 24.0)
                    });
                    self.devices_choice(ui, i, d, 0, ops);
                    self.devices_knobs(ui, owner, i, d, &[1, 2, 3, 4, 5], ops);
                }
                FxKind::Delay => {
                    let r = graph(ui, 70.0);
                    let bpm = self.with(|s| s.song.bpm);
                    delay_graph(ui.painter(), r, d, bpm);
                    ui.horizontal(|ui| {
                        self.devices_choice(ui, i, d, 0, ops);
                    });
                    self.devices_choice(ui, i, d, 3, ops);
                    let keys: &[usize] = if d.real(0) as usize == 0 {
                        &[1, 2, 4, 5, 6]
                    } else {
                        &[2, 4, 5, 6]
                    };
                    self.devices_knobs(ui, owner, i, d, keys, ops);
                }
                FxKind::Reverb => {
                    let r = graph(ui, 70.0);
                    reverb_graph(ui.painter(), r, d);
                    self.devices_knobs(ui, owner, i, d, &[0, 1, 2, 3, 4, 5, 6], ops);
                }
                k => {
                    let info = fx::params(k);
                    let mut knobs = vec![];
                    for (pi, x) in info.iter().enumerate() {
                        if x.choices.is_empty() {
                            knobs.push(pi);
                        } else {
                            self.devices_choice(ui, i, d, pi, ops);
                        }
                    }
                    self.devices_knobs(ui, owner, i, d, &knobs, ops);
                }
            }
        });
        rect
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
                    },
                );
                let tid = crate::automation::Target::Fx(owner, i, k);
                let mut lane = false;
                self.mod_slider(ui, &out.resp, &tid.id(), v0, Some(&mut lane));
                if lane {
                    self.add_lane(tid);
                }
                if out.changed {
                    let first = out.resp.drag_started() || !out.resp.dragged();
                    ops.push(FxOp::Param(i, k, v, first));
                }
            }
        });
    }

    /// a choice parameter as small chips
    fn devices_choice(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        d: &Device,
        k: usize,
        ops: &mut Vec<FxOp>,
    ) {
        let Some(info) = fx::params(d.kind).get(k) else {
            return;
        };
        let cur = d.real(k) as usize;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.label(egui::RichText::new(info.name).small().color(th().ink2));
            for (ci, cn) in info.choices.iter().enumerate() {
                if ui
                    .selectable_label(cur == ci, egui::RichText::new(*cn).small())
                    .clicked()
                    && cur != ci
                {
                    ops.push(FxOp::Param(i, k, info.to_norm(ci as f32), true));
                }
            }
        });
    }

    /// EQ: the curve with a draggable dot per band, then the selected band's knobs
    fn devices_eq(
        &mut self,
        ui: &mut egui::Ui,
        owner: usize,
        i: usize,
        d: &Device,
        ops: &mut Vec<FxOp>,
    ) {
        let t = th();
        let sr = self.with(|s| s.engine.sr).max(8000.0);
        let r = graph(ui, 110.0);
        let band = self
            .dev
            .eq_band
            .iter()
            .find(|(k, _)| *k == i)
            .map(|x| x.1)
            .unwrap_or(1);
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
        let mut pick = None;
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
            let on = b == band;
            let p = ui.painter();
            p.circle_filled(c, if on || resp.hovered() { 6.0 } else { 4.5 }, col);
            if on {
                p.circle_stroke(c, 8.0, Stroke::new(1.2, col));
            }
            if resp.clicked() || resp.drag_started() {
                pick = Some(b);
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
        if let Some(b) = pick {
            self.dev.eq_band.retain(|(k, _)| *k != i);
            self.dev.eq_band.push((i, b));
        }
        let band = pick.unwrap_or(band);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for (b, (_, _, name)) in BANDS.iter().enumerate() {
                let txt = egui::RichText::new(*name).small().color(hex(COLS[b]));
                if ui.selectable_label(b == band, txt).clicked() {
                    self.dev.eq_band.retain(|(k, _)| *k != i);
                    self.dev.eq_band.push((i, b));
                }
            }
        });
        let keys: &[usize] = match band {
            0 => &[0, 1],
            1 => &[2, 3, 4],
            2 => &[5, 6, 7],
            _ => &[8, 9],
        };
        self.devices_knobs(ui, owner, i, d, keys, ops);
    }

    /// a plugin effect: its editor thumbnail, then the plugin device body
    /// (status, Editor… / Reload, parameters)
    fn devices_plugin_body(&mut self, ui: &mut egui::Ui, owner: usize, i: usize, d: &Device) {
        if let Some(pp) = d.plugin.as_deref() {
            let found = self.br.items.iter().find_map(|it| match &it.src {
                crate::browser::Src::Plugin(f) if f.desc.id == pp.id && f.format == pp.format => {
                    Some(f.clone())
                }
                _ => None,
            });
            let tex = found.and_then(|f| self.br_thumb(ui.ctx(), &f));
            if let Some(tex) = tex {
                let s = tex.size_vec2();
                let w = ui.available_width();
                let scale = (w / s.x).min(96.0 / s.y).min(1.0);
                let (r, resp) = ui.allocate_exact_size(s * scale, Sense::click());
                ui.painter().image(
                    tex.id(),
                    r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                if resp
                    .on_hover_text("double-click: open the plugin's editor")
                    .double_clicked()
                {
                    self.devices_plugin_editor(owner, d.uid, &pp.name);
                }
            }
        }
        self.plugin_fx_body(ui, owner, i, d);
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

/// a graph area at the top of a card body
fn graph(ui: &mut egui::Ui, h: f32) -> Rect {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    ui.add_space(2.0);
    r
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
    p.add(egui::Shape::line(pts, Stroke::new(1.8, t.accent)));
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
        FontId::proportional(10.0),
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
        FontId::proportional(10.0),
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
    p.add(egui::Shape::line(pts, Stroke::new(1.6, t.accent)));
    p.text(
        r.right_top() + egui::vec2(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        format!("~{decay:.1} s"),
        FontId::proportional(10.0),
        t.ink2,
    );
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

/// a power button; true when clicked
fn power(ui: &egui::Ui, p: &egui::Painter, r: Rect, id: egui::Id, on: bool) -> bool {
    let t = th();
    let resp =
        ui.interact(r, id, Sense::click())
            .on_hover_text(if on { "bypass" } else { "enable" });
    let c = r.center();
    let col = if on { t.accent } else { t.ink2 };
    if resp.hovered() {
        p.circle_filled(c, 10.0, t.ink.gamma_multiply(0.12));
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
    let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, h), Sense::hover());
    chevron_icon(
        ui.painter(),
        r.center(),
        true,
        th().ink2.gamma_multiply(0.7),
    );
}

/// a name written bottom to top in a slim card
fn vertical_name(p: &egui::Painter, r: Rect, s: &str, col: Color32) {
    let g = p.layout_no_wrap(s.to_string(), FontId::proportional(12.0), col);
    let pos = Pos2::new(r.center().x - g.size().y * 0.5, r.bottom() - 10.0);
    p.add(egui::epaint::TextShape::new(pos, g, col).with_angle(-std::f32::consts::FRAC_PI_2));
}

/// the `+` card at the end of the chain; true when clicked
fn plus_card(ui: &mut egui::Ui, h: f32, empty: bool) -> bool {
    let t = th();
    let w = if empty { 200.0 } else { 70.0 };
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
    let resp = resp.on_hover_text("add an effect: opens Browser › Effects (or drag one here)");
    let p = ui.painter();
    let col = if resp.hovered() {
        t.accent
    } else {
        t.ink2.gamma_multiply(0.7)
    };
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
    let c = Pos2::new(r.center().x, r.center().y - if empty { 14.0 } else { 0.0 });
    p.line_segment(
        [c - egui::vec2(9.0, 0.0), c + egui::vec2(9.0, 0.0)],
        Stroke::new(2.2, col),
    );
    p.line_segment(
        [c - egui::vec2(0.0, 9.0), c + egui::vec2(0.0, 9.0)],
        Stroke::new(2.2, col),
    );
    if empty {
        p.text(
            Pos2::new(r.center().x, c.y + 26.0),
            egui::Align2::CENTER_CENTER,
            "No effects yet\nclick, or drag one in from Browser › Effects",
            FontId::proportional(11.0),
            col,
        );
    }
    resp.clicked()
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
