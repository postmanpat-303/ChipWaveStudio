//! Lower panel (0.2.3.19): the Arranger | Instrument | Automation switch, the
//! draggable splitter, Full-height pattern, and the instrument editor: header
//! (prev / next, type tag, presets, A/B), parameter search (Ctrl+F), tabs and
//! collapsible cards of knobs built from params.rs. Original ChipWave code.
use super::knob::{self, KnobArgs, CELL};
use super::params::{self, Ctl, Custom, Layout, Param, Tab};
use super::*;

pub(super) mod gridcanvas;

/// lower-panel switch bar height
const BAR_H: f32 = 26.0;
/// card header height
const HEAD_H: f32 = 22.0;
/// width of one dropdown / toggle column in a card
const CTL_W: f32 = 168.0;

/// instrument editor state that is not part of the song
#[derive(Default)]
pub(super) struct InstUi {
    pub tab: Tab,
    pub query: String,
    /// Ctrl+F: focus the search box next frame
    pub focus_search: bool,
    /// the result list is open
    pub search_open: bool,
    /// scroll this card into view next frame
    pub jump: Option<String>,
    /// card to flash (name, start time)
    pub flash: Option<(String, f64)>,
    /// A/B compare of the shown instrument
    pub ab: Option<Ab>,
    /// lower_h when the splitter drag started
    drag_h0: Option<f32>,
    /// the splitter is dragged to the bottom: release hides the panel
    pub collapse_pending: bool,
    /// the selected modulator (its depth is what Alt-drag edits)
    pub mod_sel: Option<u32>,
    /// the Grid canvas (cable being dragged, add position)
    grid: gridcanvas::GridUi,
    /// the Grid FX patch window: (instrument, device) and its canvas state
    pub grid_fx: Option<(usize, usize)>,
    pub grid_fx_ui: gridcanvas::GridUi,
    /// a knob / module drag is going on in that window (one undo step)
    pub grid_fx_held: bool,
}

pub(super) struct Ab {
    inst: usize,
    /// the sound not playing now
    other: InstrumentKind,
    on_b: bool,
}

/// what the panel asks the app to do after drawing
enum Act {
    Select(u8),
    Kind(Box<InstrumentKind>),
    AbToggle,
    Bake,
    SaveLib,
    Library,
    OpenSample,
    OpenFx,
    AddFx(crate::fx::FxKind),
    AddPluginFx(crate::plugin::PluginPatch),
    Lane(crate::automation::Target),
}

/// per-frame drawing context of the cards
struct Cx {
    idx: usize,
    tag: &'static str,
    words: Vec<String>,
    changed: bool,
    /// knob the user just moved (follow last-touched)
    touch: Option<crate::automation::Target>,
    acts: Vec<Act>,
    flash: Option<(String, f32)>,
    jump: Option<String>,
    card_h: f32,
    toggle_card: Option<String>,
    /// the song's modulation (edited here, written back after the cards)
    modm: crate::modulation::Modulation,
    /// live modulator outputs from the engine
    live: [f32; crate::modulation::MAX_MODS],
    /// route target names (Routings card)
    labels: Vec<String>,
    mod_out: super::modview::ModOut,
}

fn matches(words: &[String], p: &Param) -> bool {
    if words.is_empty() {
        return false;
    }
    let hay = format!("{} {}", p.name, p.card).to_lowercase();
    words.iter().all(|w| hay.contains(w.as_str()))
}

/// card name without the kit piece ("Drum · Kick" -> "Drum")
fn card_base(n: &str) -> &str {
    n.split(" · ").next().unwrap_or(n)
}

/// colour of a type tag (the list, the header)
pub(super) fn tag_colour(tag: &str) -> Color32 {
    match tag {
        "Sample" => hex(0xE8C35A),
        "Voice" => hex(0xD98AD0),
        "CLAP" | "VST3" => hex(0x9AA8F0),
        "Drum" | "Kit" => hex(0xE08A5C),
        "Chip FX" => hex(0xD8D070),
        "Analog" => hex(0x8FD9A3),
        "FM-4" => hex(0x7FCFC0),
        "Noise" => hex(0xB4BCC8),
        "Wave" => hex(0xC79AF0),
        "Grid" => hex(0xF0A86A),
        "Moog" => hex(0x6FB0E8),
        _ => hex(0x4FC3D9),
    }
}

/// the tag colour, darkened on light themes so text stays readable
pub(super) fn tag_text_colour(tag: &str) -> Color32 {
    let c = tag_colour(tag);
    if th().light {
        Color32::from_rgb(
            (c.r() as f32 * 0.5) as u8,
            (c.g() as f32 * 0.5) as u8,
            (c.b() as f32 * 0.5) as u8,
        )
    } else {
        c
    }
}

/// a small rounded type pill
pub(super) fn tag_pill(ui: &mut egui::Ui, tag: &str) -> egui::Response {
    let font = egui::FontId::proportional(11.0);
    let g = ui.fonts_mut(|f| f.layout_no_wrap(tag.to_string(), font, Color32::BLACK));
    let size = egui::vec2(g.size().x + 12.0, 17.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::hover());
    let c = tag_colour(tag);
    ui.painter().rect_filled(r, 8.0, c.gamma_multiply(0.22));
    ui.painter().rect_stroke(
        r,
        8.0,
        Stroke::new(1.0, c.gamma_multiply(0.8)),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley_with_override_text_color(
        r.center() - g.size() / 2.0,
        g,
        tag_text_colour(tag),
    );
    resp
}

impl App {
    /// the arranger is docked under the pattern and showing
    pub(super) fn arr_docked(&self) -> bool {
        self.dock_arranger
            && !self.extra.full_height
            && !self.extra.lower_inst
            && !self.extra.lower_dev
    }

    /// the lower panel (arranger / automation / instrument) is showing
    pub(super) fn lower_visible(&self) -> bool {
        self.dock_arranger && !self.extra.full_height
    }

    /// F11: hide / show the whole lower panel (its height is kept)
    pub(super) fn toggle_full_height(&mut self) {
        if self.lower_visible() {
            self.extra.full_height = true;
            self.arr_focus = false;
            self.status = "Full-height pattern (F11 brings the lower panel back)".into();
        } else {
            self.extra.full_height = false;
            self.dock_arranger = true;
            self.status = "Lower panel shown".into();
        }
        // F11 works for the Timeline view too (full-height timeline)
        if self.view != View::Timeline {
            self.view = View::Pattern;
        }
        self.save_settings();
    }

    /// Ctrl+I: the instrument editor in the lower panel (again: back to the arranger)
    pub(super) fn toggle_instrument_panel(&mut self) {
        if matches!(self.view, View::Pattern | View::Timeline)
            && self.lower_visible()
            && self.extra.lower_inst
        {
            self.set_lower(0);
        } else {
            self.show_instrument_panel();
        }
    }

    pub(super) fn show_instrument_panel(&mut self) {
        // the Timeline view has the same lower panel
        if self.view != View::Timeline {
            self.view = View::Pattern;
        }
        self.dock_arranger = true;
        self.extra.full_height = false;
        self.extra.lower_inst = true;
        self.extra.lower_dev = false;
        self.extra.lower_chosen = true;
        self.arr_focus = false;
        self.save_settings();
    }

    /// The default workspace (Del, 19 Oct 2026): the Timeline on top and
    /// the piano roll docked in the lower panel at about 40 % of the height.
    /// Shown on start, on File > New and when a timeline-mode song opens;
    /// Ctrl+6, View > Reset to default layout and the view bar's
    /// TIMELINE + PIANO button bring it back. The Inspector and the Browser
    /// stay as they are.
    pub(super) fn default_layout(&mut self) {
        self.view = View::Timeline;
        self.dock_arranger = true;
        self.extra.full_height = false;
        self.extra.lower_inst = false;
        self.extra.lower_dev = false;
        self.extra.lower_h = 0.0;
        self.show_auto = false;
        self.arr_focus = false;
        self.pr.open = true;
        self.pr.focus = true;
    }

    /// the default layout is showing
    pub(super) fn is_default_layout(&self) -> bool {
        self.view == View::Timeline
            && self.lower_visible()
            && !self.extra.lower_inst
            && self.pr.open
    }

    /// Ctrl+6 / View > Reset to default layout (saved)
    pub(super) fn reset_layout(&mut self) {
        self.default_layout();
        self.save_settings();
        self.status = "Timeline + piano roll (the default layout; Ctrl+6)".into();
    }

    /// 0 arranger, 1 instrument, 2 automation, 3 piano roll, 4 devices
    pub(super) fn set_lower(&mut self, m: u8) {
        self.dock_arranger = true;
        self.extra.full_height = false;
        self.extra.lower_inst = m == 1;
        self.extra.lower_dev = m == 4;
        self.extra.lower_chosen = true;
        self.pr.open = m == 3;
        self.pr.focus = m == 3;
        if m == 1 || m == 3 || m == 4 {
            self.arr_focus = false;
        } else {
            self.show_auto = m == 2;
        }
        self.save_settings();
    }

    fn lower_mode(&self) -> u8 {
        if self.extra.lower_dev {
            4
        } else if self.extra.lower_inst {
            1
        } else if self.pr.open {
            3
        } else if self.show_auto {
            2
        } else {
            0
        }
    }

    /// the pattern editor over the lower panel, with the splitter between
    pub(super) fn pattern_and_lower(&mut self, ui: &mut egui::Ui, inner: Rect) {
        let t = th();
        let nl = self.with(|s| s.song.automation.len());
        let auto_h = if self.pr.open && !self.extra.lower_inst {
            // the default layout: the piano roll at about 40 %
            (inner.height() * 0.40).max(200.0)
        } else if self.extra.lower_dev {
            // Devices: the instrument card wants room for its knobs
            (inner.height() * 0.45).clamp(260.0, 400.0)
        } else if self.extra.lower_inst || self.pr.open {
            (inner.height() * 0.45).clamp(200.0, 340.0)
        } else if self.show_auto && nl > 0 {
            (inner.height() * 0.5).clamp(190.0, 120.0 + nl as f32 * 68.0 + 60.0)
        } else {
            (inner.height() * 0.3).clamp(120.0, 190.0)
        } + BAR_H;
        let max_h = (inner.height() - 150.0).max(BAR_H + 60.0);
        let min_h = (BAR_H + 96.0).min(max_h);
        let want = if self.extra.lower_h > 0.0 {
            self.extra.lower_h
        } else {
            auto_h
        };
        let h = want.clamp(min_h, max_h);
        let split = Rect::from_min_max(
            Pos2::new(inner.left(), inner.bottom() - h - 7.0),
            Pos2::new(inner.right(), inner.bottom() - h),
        );
        let top = Rect::from_min_max(inner.min, Pos2::new(inner.right(), split.top()));
        let bot = Rect::from_min_max(Pos2::new(inner.left(), split.bottom()), inner.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(top), |ui| {
            if self.view == View::Timeline {
                self.timeline_view(ui)
            } else {
                self.pattern_view(ui)
            }
        });
        // splitter: drag to resize, to the bottom to hide, double-click = automatic
        let sr = ui
            .interact(split, egui::Id::unique("lower_split"), Sense::click_and_drag())
            .on_hover_text(
                "drag to resize the lower panel · drag to the bottom to hide it (F11) · double-click: automatic height",
            );
        if sr.hovered() || sr.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if sr.drag_started() {
            self.ip.drag_h0 = Some(self.extra.lower_h);
        }
        if sr.dragged() {
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                let nh = inner.bottom() - p.y - 3.0;
                self.ip.collapse_pending = nh < min_h * 0.55;
                self.extra.lower_h = nh.clamp(min_h, max_h);
            }
        }
        if sr.drag_stopped() {
            if self.ip.collapse_pending {
                self.extra.lower_h = self.ip.drag_h0.unwrap_or(0.0);
                self.extra.full_height = true;
                self.arr_focus = false;
                self.status = "Full-height pattern (F11 brings the lower panel back)".into();
            }
            self.ip.collapse_pending = false;
            self.ip.drag_h0 = None;
            self.save_settings();
        }
        if sr.double_clicked() {
            self.extra.lower_h = 0.0;
            self.save_settings();
        }
        let hot = sr.hovered() || sr.dragged();
        let p = ui.painter();
        let col = if hot {
            t.accent
        } else {
            t.border.gamma_multiply(0.6)
        };
        p.hline(split.x_range(), split.center().y, Stroke::new(1.0, col));
        for k in -2..=2 {
            p.circle_filled(
                Pos2::new(split.center().x + k as f32 * 6.0, split.center().y),
                1.5,
                if hot { t.accent } else { t.ink2 },
            );
        }
        let bar = Rect::from_min_size(bot.min, egui::vec2(bot.width(), BAR_H));
        let body = Rect::from_min_max(Pos2::new(bot.left(), bar.bottom() + 2.0), bot.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
            self.lower_switch_bar(ui)
        });
        if self.ip.collapse_pending {
            ui.painter().text(
                body.center(),
                egui::Align2::CENTER_CENTER,
                "release to hide the lower panel (F11 brings it back)",
                egui::FontId::proportional(14.0),
                t.accent,
            );
            return;
        }
        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
            if self.extra.lower_dev {
                self.devices_view(ui)
            } else if self.extra.lower_inst {
                self.instrument_view(ui)
            } else if self.pr.open {
                self.piano_view(ui)
            } else {
                self.arranger_view(ui)
            }
        });
    }

    /// Arranger | Instrument | Automation, and Full-height pattern
    fn lower_switch_bar(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let r = ui.max_rect();
        ui.painter()
            .rect_filled(r, 4.0, t.ink2.gamma_multiply(0.08));
        ui.horizontal_centered(|ui| {
            ui.add_space(4.0);
            let cur = self.lower_mode();
            for (m, name, tip) in [
                (0u8, "Arranger", "the song's blocks over time"),
                (
                    1,
                    "Instrument",
                    "edit the selected instrument with knobs (Ctrl+I)",
                ),
                (2, "Automation", "the arranger with its automation lanes"),
                (
                    3,
                    "Piano roll",
                    "the notes of the selected clip (or the current block column) as bars: draw, move, resize",
                ),
                (
                    4,
                    "Devices",
                    "the selected track's instrument and effects, left to right (Ctrl+7)",
                ),
            ] {
                let txt = egui::RichText::new(name).strong();
                if ui
                    .selectable_label(cur == m, txt)
                    .on_hover_text(tip)
                    .clicked()
                    && cur != m
                {
                    self.set_lower(m);
                }
            }
            if self.extra.lower_inst {
                let n = self.with(|s| {
                    s.song
                        .instruments
                        .get((self.instr as usize).wrapping_sub(1))
                        .map(|i| i.name.clone())
                });
                if let Some(n) = n {
                    ui.label(
                        egui::RichText::new(format!("· {:02X} {n}", self.instr))
                            .small()
                            .color(t.ink2),
                    );
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                if ui
                    .button("Full-height pattern  F11")
                    .on_hover_text("hide the lower panel; F11 or Ctrl+I bring it back")
                    .clicked()
                {
                    self.toggle_full_height();
                }
            });
        });
    }
}

impl App {
    /// the instrument editor (lower panel, Instrument)
    pub(super) fn instrument_view(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let n = self.with(|s| s.song.instruments.len());
        if n == 0 {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("No instruments yet").strong());
                ui.menu_button("New instrument", |ui| self.new_instrument_menu(ui));
            });
            return;
        }
        if self.instr == 0 || self.instr as usize > n {
            self.instr = 1;
        }
        let idx = self.instr as usize - 1;
        let Some(mut inst) = self.with(|s| s.song.instruments.get(idx).cloned()) else {
            return;
        };
        if self.gen_ui.inst != idx {
            self.gen_ui = super::genview::GenUi {
                inst: idx,
                ..Default::default()
            };
            // a Grid opens on its patch
            if let InstrumentKind::Gen(g) = &inst.kind {
                if matches!(g.engine, crate::gen::GenEngine::Grid(_)) {
                    self.ip.tab = Tab::Grid;
                }
            }
        }
        if self.ip.ab.as_ref().is_some_and(|a| a.inst != idx) {
            self.ip.ab = None;
        }
        self.gen_ui.sel = self.gen_ui.sel.min(7);
        let orig = inst.clone();
        let layout = params::build(&inst, idx, self.gen_ui.sel);
        let tag = params::type_tag(&inst.kind);
        let now = ui.input(|i| i.time);
        let words: Vec<String> = self
            .ip
            .query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();
        let mut cx = Cx {
            idx,
            tag,
            words,
            changed: false,
            touch: None,
            acts: vec![],
            flash: None,
            jump: self.ip.jump.take(),
            card_h: 0.0,
            toggle_card: None,
            modm: Default::default(),
            live: [0.0; crate::modulation::MAX_MODS],
            labels: vec![],
            mod_out: Default::default(),
        };
        (cx.modm, cx.live, cx.labels) = self.with(|s| {
            let m = &s.song.modulation;
            (
                m.clone(),
                s.engine.mods.out,
                m.routes
                    .iter()
                    .map(|r| super::modview::target_label(&s.song, &r.target))
                    .collect(),
            )
        });
        let orig_modm = cx.modm.clone();
        if !cx.modm.routes.is_empty() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        if let Some((c, t0)) = &self.ip.flash {
            let e = (now - t0) as f32;
            if e < 1.6 {
                cx.flash = Some((c.clone(), 1.0 - e / 1.6));
                ui.ctx().request_repaint();
            } else {
                self.ip.flash = None;
            }
        }
        // ---- header ----
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if ui.small_button("<").on_hover_text("previous instrument").clicked() {
                cx.acts
                    .push(Act::Select(if idx == 0 { n as u8 } else { idx as u8 }));
            }
            if ui.small_button(">").on_hover_text("next instrument").clicked() {
                cx.acts.push(Act::Select(((idx + 1) % n) as u8 + 1));
            }
            ui.label(
                egui::RichText::new(format!("{:02X} {}", idx + 1, inst.name))
                    .strong()
                    .size(15.0),
            );
            tag_pill(ui, tag);
            self.preset_menu(ui, &inst, &mut cx);
            if ui
                .small_button("Save")
                .on_hover_text("save this instrument to the library")
                .clicked()
            {
                cx.acts.push(Act::SaveLib);
            }
            if ui
                .small_button("Library…")
                .on_hover_text("replace the sound from the instrument library")
                .clicked()
            {
                cx.acts.push(Act::Library);
            }
            if !matches!(inst.kind, InstrumentKind::Plugin(_)) {
                let on_b = self.ip.ab.as_ref().is_some_and(|a| a.on_b);
                ui.add_space(4.0);
                for (b, name) in [(false, "A"), (true, "B")] {
                    let r = ui
                        .selectable_label(on_b == b, egui::RichText::new(name).strong())
                        .on_hover_text(
                            "A/B compare: B starts as a copy of A; edit one, switch to hear the other (Ctrl+Z undoes a switch)",
                        );
                    if r.clicked() && on_b != b {
                        cx.acts.push(Act::AbToggle);
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                self.param_search(ui, &layout, &inst, now);
            });
        });
        // ---- tabs ----
        let mut tab = self.ip.tab;
        if !layout.tab_has(tab) {
            tab = Tab::Main;
        }
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            for tb in Tab::ALL {
                if !layout.tab_has(tb) {
                    continue;
                }
                let n = layout
                    .params
                    .iter()
                    .filter(|p| p.tab == tb && p.custom().is_none())
                    .count();
                let hits = layout
                    .params
                    .iter()
                    .filter(|p| p.tab == tb && matches(&cx.words, p))
                    .count();
                let mut txt = egui::RichText::new(if n > 0 {
                    format!("{}  {n}", tb.name())
                } else {
                    tb.name().to_string()
                });
                if hits > 0 {
                    txt = txt.color(t.fx);
                }
                if ui.selectable_label(tab == tb, txt).clicked() {
                    tab = tb;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(
                            "drag · Ctrl-drag fine · double-click default · right-click automate / MIDI learn",
                        )
                        .small()
                        .color(t.ink2),
                    )
                    .truncate(),
                );
            });
        });
        self.ip.tab = tab;
        // ---- modulators: drag a chip onto a knob ----
        if super::modview::strip(
            ui,
            &mut cx.modm,
            idx,
            inst.kind.patch().is_some(),
            &cx.live,
            &mut self.ip.mod_sel,
            &mut cx.mod_out,
        ) {
            // a new modulator: show it on the Modulation tab
            self.ip.tab = Tab::Modulation;
        }
        // ---- cards ----
        let avail = ui.available_height();
        cx.card_h = (avail - 14.0).max(90.0);
        let cards: Vec<&params::Card> = layout.cards.iter().filter(|c| c.tab == tab).collect();
        if tab == Tab::Grid {
            self.grid_canvas(ui, &layout, &mut inst, &mut cx);
        } else {
            egui::ScrollArea::horizontal()
                .id_salt(("inst_cards", idx, tab))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.add_space(2.0);
                        for c in cards {
                            self.card_ui(ui, &layout, c, &mut inst, &mut cx);
                        }
                    });
                });
        }
        if let Some(k) = cx.toggle_card.take() {
            if let Some(p) = self.extra.cards_closed.iter().position(|x| *x == k) {
                self.extra.cards_closed.remove(p);
            } else {
                self.extra.cards_closed.push(k);
            }
            self.save_settings();
        }
        // ---- write back, then the actions ----
        if cx.changed && inst != orig {
            if let InstrumentKind::Voice(v) = &mut inst.kind {
                v.fix();
            }
            self.with(|s| {
                if let Some(x) = s.song.instruments.get_mut(idx) {
                    x.volume = inst.volume;
                    x.transpose = inst.transpose;
                    x.fx = inst.fx.clone();
                    if !matches!(x.kind, InstrumentKind::Plugin(_)) {
                        x.kind = inst.kind.clone();
                    }
                }
            });
            self.dirty = true;
        }
        if cx.modm != orig_modm {
            let m = cx.modm.clone();
            self.with(|s| s.song.modulation = m);
            self.dirty = true;
        }
        self.mod_out(std::mem::take(&mut cx.mod_out));
        for a in cx.acts {
            match a {
                Act::Select(s) => {
                    self.instr = s;
                }
                Act::Kind(k) => {
                    self.snapshot();
                    self.with(|s| {
                        if let Some(x) = s.song.instruments.get_mut(idx) {
                            x.kind = *k;
                        }
                    });
                    self.dirty = true;
                }
                Act::AbToggle => {
                    let cur = self.with(|s| s.song.instruments.get(idx).map(|x| x.kind.clone()));
                    let Some(cur) = cur else { continue };
                    let (other, on_b) = match self.ip.ab.take() {
                        Some(a) => (a.other, !a.on_b),
                        None => (cur.clone(), true),
                    };
                    self.snapshot();
                    self.with(|s| {
                        if let Some(x) = s.song.instruments.get_mut(idx) {
                            x.kind = other;
                        }
                    });
                    self.ip.ab = Some(Ab {
                        inst: idx,
                        other: cur,
                        on_b,
                    });
                    self.status = format!(
                        "Instrument {:02X}: {}",
                        idx + 1,
                        if on_b { "B" } else { "A" }
                    );
                }
                Act::Bake => self.bake_gen(idx),
                Act::SaveLib => self.save_instrument_to_library(idx as u8 + 1),
                Act::Library => self.open_library(),
                Act::OpenSample => self.view = View::Sample,
                Act::OpenFx => self.view = View::Fx,
                Act::AddFx(k) => self.add_effect(idx as u8 + 1, k),
                Act::AddPluginFx(p) => self.add_plugin_fx(idx as u8 + 1, p),
                Act::Lane(tg) => self.add_lane(tg),
            }
        }
        if let Some(tg) = cx.touch {
            self.touch_param(tg);
        }
    }

    /// Preset… (synth factory, voice, generator presets of the same engine)
    fn preset_menu(&mut self, ui: &mut egui::Ui, inst: &Instrument, cx: &mut Cx) {
        match &inst.kind {
            InstrumentKind::Synth(p) => {
                ui.menu_button("Preset…", |ui| {
                    let mut groups: Vec<&str> = vec![];
                    for (name, _) in crate::presets::FACTORY {
                        let g = name.split('/').next().unwrap_or("");
                        if !groups.contains(&g) {
                            groups.push(g);
                        }
                    }
                    for g in groups {
                        ui.menu_button(g, |ui| {
                            for (name, _) in crate::presets::FACTORY {
                                let Some(short) = name.strip_prefix(&format!("{g}/")) else {
                                    continue;
                                };
                                if ui.button(short).clicked() {
                                    if let Some(np) = crate::presets::factory(name) {
                                        let mut q = p.clone();
                                        q.copy_sound_from(&np);
                                        cx.acts.push(Act::Kind(Box::new(InstrumentKind::Synth(q))));
                                    }
                                    ui.close();
                                }
                            }
                        });
                    }
                })
                .response
                .on_hover_text("load a factory sound (keeps the name and FX chain)");
            }
            InstrumentKind::Voice(v) => {
                ui.menu_button("Preset…", |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            for (name, tip, mut np) in crate::voice::presets() {
                                if ui.button(name).on_hover_text(tip).clicked() {
                                    np.base.name = v.base.name.clone();
                                    cx.acts.push(Act::Kind(Box::new(InstrumentKind::Voice(np))));
                                    ui.close();
                                }
                            }
                        });
                })
                .response
                .on_hover_text(
                    "replace every voice, envelope, filter and LFO setting with a preset",
                );
            }
            InstrumentKind::Gen(g) => {
                let d = std::mem::discriminant(&g.engine);
                ui.menu_button("Preset…", |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            for (name, tip, mut np) in crate::gen::presets() {
                                if std::mem::discriminant(&np.engine) != d {
                                    continue;
                                }
                                if ui.button(name).on_hover_text(tip).clicked() {
                                    np.base.name = g.base.name.clone();
                                    cx.acts.push(Act::Kind(Box::new(InstrumentKind::Gen(np))));
                                    ui.close();
                                }
                            }
                        });
                })
                .response
                .on_hover_text("load a preset of this generator type");
            }
            _ => {}
        }
    }

    /// "Search parameters… Ctrl+F": highlights matches, the list jumps to one
    fn param_search(&mut self, ui: &mut egui::Ui, l: &Layout, inst: &Instrument, now: f64) {
        let t = th();
        let _ = inst;
        if !self.ip.query.is_empty() && ui.small_button("×").on_hover_text("clear").clicked() {
            self.ip.query.clear();
            self.ip.search_open = false;
        }
        let r = ui.add(
            egui::TextEdit::singleline(&mut self.ip.query)
                .id(egui::Id::unique("param_search"))
                .hint_text("Search parameters…  Ctrl+F")
                .text_color(t.note)
                .desired_width(ui.available_width().clamp(90.0, 210.0)),
        );
        if self.ip.focus_search {
            self.ip.focus_search = false;
            r.request_focus();
            self.ip.search_open = true;
        }
        if r.changed() || r.gained_focus() {
            self.ip.search_open = true;
        }
        let words: Vec<String> = self
            .ip
            .query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();
        let hits: Vec<&Param> = l.params.iter().filter(|p| matches(&words, p)).collect();
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let esc = ui.input(|i| i.key_pressed(Key::Escape));
        if esc && (r.has_focus() || r.lost_focus()) {
            self.ip.search_open = false;
        }
        let mut jump: Option<(Tab, String)> = None;
        if enter {
            if let Some(p) = hits.first() {
                jump = Some((p.tab, p.card.clone()));
            }
        }
        if self.ip.search_open && !words.is_empty() {
            let pos = r.rect.left_bottom() + egui::vec2(0.0, 2.0);
            let w = 280.0;
            let pos = Pos2::new(pos.x.min(ui.ctx().content_rect().right() - w - 8.0), pos.y);
            egui::Area::new(egui::Id::unique("param_search_hits"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_width(w);
                        if hits.is_empty() {
                            ui.label(egui::RichText::new("no parameter matches").color(t.ink2));
                        }
                        egui::ScrollArea::vertical()
                            .max_height(260.0)
                            .show(ui, |ui| {
                                for p in &hits {
                                    let txt =
                                        format!("{}   ·  {} › {}", p.name, p.tab.name(), p.card);
                                    if ui
                                        .add(egui::Button::new(txt).frame(false))
                                        .on_hover_text(&p.tip)
                                        .clicked()
                                    {
                                        jump = Some((p.tab, p.card.clone()));
                                    }
                                }
                            });
                    });
                });
        }
        if let Some((tab, card)) = jump {
            self.ip.tab = tab;
            let key = format!("{}/{}", params::type_tag(&inst.kind), card_base(&card));
            self.extra.cards_closed.retain(|k| *k != key);
            self.ip.jump = Some(card.clone());
            self.ip.flash = Some((card, now));
            self.ip.search_open = false;
            r.surrender_focus();
        }
    }
}

/// (width, height) a custom widget wants
fn custom_size(c: Custom, card_h: f32) -> (f32, f32) {
    match c {
        Custom::Vowels => (270.0, 84.0),
        Custom::Words => (270.0, 84.0),
        Custom::KitPiece => (300.0, 64.0),
        Custom::Dice => (300.0, 96.0),
        Custom::ChipCategories => (290.0, 64.0),
        Custom::PluginStatus => (290.0, 96.0),
        Custom::PluginMacros => (4.0 * 76.0 + 8.0, card_h - HEAD_H - 10.0),
        Custom::PluginParams => (470.0, card_h - HEAD_H - 10.0),
        Custom::SampleInfo => (270.0, 84.0),
        Custom::FxChain => (210.0, 64.0),
        Custom::WaveTable(_) => (210.0, 64.0),
        Custom::GridCanvas => (210.0, 40.0),
        Custom::Modulators => (300.0, card_h - HEAD_H - 10.0),
        Custom::Routings => (330.0, card_h - HEAD_H - 10.0),
    }
}

impl App {
    fn card_ui(
        &mut self,
        ui: &mut egui::Ui,
        l: &Layout,
        c: &params::Card,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let t = th();
        let key = format!("{}/{}", cx.tag, card_base(&c.name));
        let closed = self.extra.cards_closed.contains(&key);
        let card_h = cx.card_h;
        if closed {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(26.0, card_h), Sense::click());
            let resp = resp.on_hover_text(format!("{}: click to expand", c.name));
            let lbl = c.name.clone();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &lbl));
            let p = ui.painter();
            p.rect_filled(r, 6.0, t.cart_bot);
            p.rect_stroke(
                r,
                6.0,
                Stroke::new(
                    1.0,
                    t.ink2
                        .gamma_multiply(if resp.hovered() { 0.7 } else { 0.35 }),
                ),
                egui::StrokeKind::Inside,
            );
            let g = ui.fonts_mut(|f| {
                f.layout_no_wrap(c.name.clone(), egui::FontId::proportional(12.0), t.ink)
            });
            let pos = Pos2::new(r.center().x - g.size().y / 2.0, r.top() + 12.0 + g.size().x);
            p.add(
                egui::epaint::TextShape::new(pos, g, t.ink)
                    .with_angle(-std::f32::consts::FRAC_PI_2),
            );
            if resp.clicked() {
                cx.toggle_card = Some(key);
            }
            return;
        }
        let ps: Vec<&Param> = l.card_params(c).collect();
        let knobs: Vec<&Param> = ps
            .iter()
            .copied()
            .filter(|p| matches!(p.ctl, Ctl::Knob { .. }))
            .collect();
        let ctls: Vec<&Param> = ps
            .iter()
            .copied()
            .filter(|p| matches!(p.ctl, Ctl::Choice(_) | Ctl::Toggle))
            .collect();
        let customs: Vec<Custom> = ps.iter().filter_map(|p| p.custom()).collect();
        let (mut cw, mut chh) = (0.0f32, 0.0f32);
        for k in &customs {
            let (w, h) = custom_size(*k, card_h);
            cw = cw.max(w);
            chh += h;
        }
        let viz_h = match &c.viz {
            Some(_) if card_h > 240.0 => 46.0,
            Some(_) => 30.0,
            None => 0.0,
        };
        let nk = knobs.len() as f32;
        let nc = ctls.len() as f32;
        let min_w = if nc > 0.0 {
            CTL_W + 12.0
        } else {
            CELL.x + 16.0
        };
        let mut w = min_w.max(cw + 12.0);
        let mut cols = 1.0f32;
        for _ in 0..3 {
            let per = ((w - 12.0) / CTL_W).floor().max(1.0);
            let ctl_h = (nc / per).ceil() * 25.0;
            let free = card_h - HEAD_H - viz_h - ctl_h - chh - 14.0;
            let rows = (free / CELL.y).floor().max(1.0);
            cols = (nk / rows).ceil().max(1.0);
            w = (cols * CELL.x + 14.0).max(min_w).max(cw + 12.0);
        }
        if viz_h > 0.0 {
            w = w.max(150.0);
        }
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, card_h), Sense::hover());
        if cx.jump.as_deref() == Some(c.name.as_str()) {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
            cx.jump = None;
        }
        {
            let p = ui.painter();
            p.rect_filled(rect, 6.0, t.cart_bot);
            p.rect_stroke(
                rect,
                6.0,
                Stroke::new(1.0, t.ink2.gamma_multiply(0.35)),
                egui::StrokeKind::Inside,
            );
            if let Some((f, a)) = &cx.flash {
                if *f == c.name {
                    p.rect_stroke(
                        rect.expand(1.0),
                        7.0,
                        Stroke::new(2.5, t.fx.gamma_multiply(*a)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
        // header: click the title to collapse
        let head = Rect::from_min_size(rect.min, egui::vec2(w, HEAD_H));
        let hr = ui
            .interact(
                head,
                egui::Id::unique(("card_head", cx.idx, &c.name)),
                Sense::click(),
            )
            .on_hover_text("click to collapse (remembered per instrument type)");
        {
            let p = ui.painter();
            let tri = head.left() + 10.0;
            let cy = head.center().y;
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(tri - 3.5, cy - 2.0),
                    Pos2::new(tri + 3.5, cy - 2.0),
                    Pos2::new(tri, cy + 2.5),
                ],
                t.ink2,
                Stroke::NONE,
            ));
            p.text(
                Pos2::new(head.left() + 18.0, cy),
                egui::Align2::LEFT_CENTER,
                c.name.to_uppercase(),
                egui::FontId::proportional(11.5),
                if hr.hovered() { t.accent } else { t.ink },
            );
            if !c.note.is_empty() {
                p.text(
                    Pos2::new(head.right() - 8.0, cy),
                    egui::Align2::RIGHT_CENTER,
                    &c.note,
                    egui::FontId::proportional(10.5),
                    t.ink2,
                );
            }
        }
        if hr.clicked() {
            cx.toggle_card = Some(key);
        }
        let mut y = head.bottom();
        if let Some(v) = &c.viz {
            let vr = Rect::from_min_size(
                Pos2::new(rect.left() + 6.0, y),
                egui::vec2(w - 12.0, viz_h - 4.0),
            );
            knob::draw_viz(&ui.painter_at(vr), vr, &v(inst));
            y += viz_h;
        }
        let body = Rect::from_min_max(
            Pos2::new(rect.left() + 6.0, y + 2.0),
            rect.max - egui::vec2(6.0, 4.0),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_clip_rect(body.intersect(ui.clip_rect()));
                egui::ScrollArea::vertical()
                    .id_salt(("card_body", cx.idx, &c.name))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for k in &customs {
                            self.custom_ui(ui, *k, inst, cx, card_h);
                        }
                        if !ctls.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
                                for p in &ctls {
                                    self.ctl_ui(ui, p, inst, cx);
                                }
                            });
                        }
                        if !knobs.is_empty() {
                            let per = cols.max(1.0) as usize;
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            for row in knobs.chunks(per) {
                                ui.horizontal(|ui| {
                                    for p in row {
                                        self.knob_ui(ui, p, inst, cx);
                                    }
                                });
                            }
                        }
                    });
            },
        );
    }

    fn knob_ui(&mut self, ui: &mut egui::Ui, p: &Param, inst: &mut Instrument, cx: &mut Cx) {
        let Ctl::Knob {
            lo,
            hi,
            log,
            step,
            def,
        } = p.ctl
        else {
            return;
        };
        let real = (p.get)(inst);
        let mut n = params::to_norm(real, lo, hi, log);
        let steps = if step >= 1.0 { (hi - lo) / step } else { 0.0 };
        let locked = p.lock.as_ref().map(|(f, _)| f(inst));
        let tip = if p.tip.is_empty() {
            p.name.clone()
        } else {
            p.tip.clone()
        };
        let tid = p
            .target
            .filter(|t| crate::automation::modulatable(*t))
            .map(|t| t.id());
        let rings = tid
            .as_deref()
            .map(|id| super::modview::rings(&cx.modm, id, self.ip.mod_sel))
            .unwrap_or_default();
        let live = tid
            .as_deref()
            .map(|id| super::modview::live_pos(&cx.modm, id, n, &cx.live));
        let auto_led = p.target.and_then(|tg| self.lane_led(tg));
        let out = knob::knob(
            ui,
            egui::Id::unique(("ipk", cx.idx, &p.key)),
            &mut n,
            KnobArgs {
                name: &p.name,
                value_text: p.text(real),
                default: params::to_norm(def, lo, hi, log),
                steps,
                enabled: p.is_on(inst),
                highlight: matches(&cx.words, p),
                badge: p.badge.as_deref(),
                locked,
                tip: &tip,
                rings: &rings,
                live,
                auto_led,
            },
        );
        if let Some(id) = &tid {
            self.knob_modulation(ui, &out, id, cx);
        }
        if out.changed {
            let mut v = params::from_norm(n, lo, hi, log);
            if step > 0.0 {
                v = ((v - lo) / step).round() * step + lo;
            }
            (p.set)(inst, v);
            cx.changed = true;
            if let Some(tg) = p.target {
                cx.touch = Some(tg);
            }
        }
        if out.lock_clicked {
            if let Some((_, f)) = &p.lock {
                f(inst);
                cx.changed = true;
            }
        }
        self.param_menu(&out.resp, p, inst, cx, Some(def));
    }

    /// a knob as a modulation target: drop a modulator chip on it, drag a
    /// ring's handle, Alt-drag for the selected modulator's depth
    fn knob_modulation(&mut self, ui: &egui::Ui, out: &knob::KnobOut, id: &str, cx: &mut Cx) {
        let r = &out.resp;
        if r.dnd_hover_payload::<super::modview::ModDrag>().is_some() {
            ui.painter().circle_stroke(
                r.rect.center(),
                r.rect.width() * 0.5 + 6.0,
                egui::Stroke::new(2.0, th().fx),
            );
        }
        if let Some(d) = r.dnd_release_payload::<super::modview::ModDrag>() {
            if cx
                .modm
                .connect(d.0, id, super::modview::DEFAULT_DEPTH)
                .is_some()
            {
                self.ip.mod_sel = Some(d.0);
                self.status =
                    "Modulation routed: drag the dot on the knob's ring to set the depth".into();
            }
        }
        if let Some((route, d)) = out.ring_drag {
            if let Some(rt) = cx.modm.routes.get_mut(route) {
                rt.amount = (rt.amount + d).clamp(-1.0, 1.0);
            }
        }
        if let Some(d) = out.alt_drag {
            let Some(sel) = self.ip.mod_sel.filter(|s| cx.modm.find(*s).is_some()) else {
                self.status =
                    "Alt-drag sets the selected modulator's depth: click a modulator chip first"
                        .into();
                return;
            };
            if let Some(k) = cx.modm.connect(sel, id, 0.0) {
                let a = &mut cx.modm.routes[k].amount;
                *a = (*a + d).clamp(-1.0, 1.0);
            }
        }
    }

    /// right-click: reset, add an automation lane, MIDI learn
    fn param_menu(
        &mut self,
        r: &egui::Response,
        p: &Param,
        inst: &mut Instrument,
        cx: &mut Cx,
        def: Option<f32>,
    ) {
        let r = r.interact(Sense::click());
        let learn = p.target.map(|tg| {
            let id = tg.id();
            let waiting = self.midi.learn.as_deref() == Some(id.as_str());
            (tg, id, waiting)
        });
        r.context_menu(|ui| {
            ui.label(egui::RichText::new(&p.name).strong());
            if let Some(d) = def {
                if ui
                    .button("Reset to default")
                    .on_hover_text("or double-click the knob")
                    .clicked()
                {
                    (p.set)(inst, d);
                    cx.changed = true;
                    ui.close();
                }
            }
            if let Some((tg, id, waiting)) = &learn {
                if ui
                    .button(if self.lane_led(*tg).is_some() {
                        "Show automation"
                    } else {
                        "Add automation lane"
                    })
                    .on_hover_text(
                        "open this parameter's lane under its track (timeline) or in the Automation tab",
                    )
                    .clicked()
                {
                    cx.acts.push(Act::Lane(*tg));
                    ui.close();
                }
                ui.separator();
                self.learn_items(ui, id, *waiting);
                if crate::automation::modulatable(*tg) {
                    ui.separator();
                    super::modview::menu_items(
                        ui,
                        &mut cx.modm,
                        cx.idx,
                        id,
                        &mut self.ip.mod_sel,
                        &mut cx.mod_out,
                    );
                }
            } else {
                ui.label(
                    egui::RichText::new("not automatable (sound-design setting)")
                        .small()
                        .color(th().ink2),
                );
            }
        });
        if learn.as_ref().is_some_and(|l| l.2) {
            super::midiview::ui_ring(&r.ctx, r.rect);
        }
    }

    fn ctl_ui(&mut self, ui: &mut egui::Ui, p: &Param, inst: &mut Instrument, cx: &mut Cx) {
        let t = th();
        let on = p.is_on(inst);
        let hl = matches(&cx.words, p);
        let v = (p.get)(inst);
        ui.allocate_ui(egui::vec2(CTL_W - 6.0, 22.0), |ui| {
            ui.set_width(CTL_W - 6.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(on, |ui| match &p.ctl {
                    Ctl::Choice(opts) => {
                        let mut lbl = egui::RichText::new(&p.name).small();
                        if hl {
                            lbl = lbl.color(t.fx).strong();
                        }
                        let lr = ui.add(egui::Label::new(lbl).truncate());
                        let lr = if p.tip.is_empty() {
                            lr
                        } else {
                            lr.on_hover_text(&p.tip)
                        };
                        let cur = v.round().max(0.0) as usize;
                        let lw = (CTL_W
                            - 6.0
                            - lr.rect.width()
                            - if p.lock.is_some() { 36.0 } else { 10.0 })
                        .max(60.0);
                        let cr = egui::ComboBox::from_id_salt(("ipc", cx.idx, &p.key))
                            .width(lw)
                            .selected_text(p.text(v))
                            .show_ui(ui, |ui| {
                                for (k, o) in opts.iter().enumerate() {
                                    if ui.selectable_label(cur == k, o).clicked() && cur != k {
                                        (p.set)(inst, k as f32);
                                        cx.changed = true;
                                    }
                                }
                            });
                        let nm = p.name.clone();
                        cr.response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &nm)
                        });
                        if hl {
                            ui.painter().rect_stroke(
                                cr.response.rect.expand(1.0),
                                3.0,
                                Stroke::new(1.5, t.fx),
                                egui::StrokeKind::Outside,
                            );
                        }
                        self.param_menu(&cr.response, p, inst, cx, None);
                    }
                    Ctl::Toggle => {
                        let mut b = v > 0.5;
                        let mut txt = egui::RichText::new(&p.name);
                        if hl {
                            txt = txt.color(t.fx).strong();
                        }
                        let r = ui.checkbox(&mut b, txt);
                        let r = if p.tip.is_empty() {
                            r
                        } else {
                            r.on_hover_text(&p.tip)
                        };
                        if r.changed() {
                            (p.set)(inst, if b { 1.0 } else { 0.0 });
                            cx.changed = true;
                        }
                        self.param_menu(&r, p, inst, cx, None);
                    }
                    _ => {}
                });
                if let Some((is, f)) = &p.lock {
                    let lk = is(inst);
                    if ui
                        .selectable_label(lk, egui::RichText::new("lock").small())
                        .on_hover_text("locked settings keep their value when you Dice or Mutate")
                        .clicked()
                    {
                        f(inst);
                        cx.changed = true;
                    }
                }
            });
        });
    }
}

impl App {
    fn custom_ui(
        &mut self,
        ui: &mut egui::Ui,
        k: Custom,
        inst: &mut Instrument,
        cx: &mut Cx,
        card_h: f32,
    ) {
        let t = th();
        let idx = cx.idx;
        let sel = self.gen_ui.sel;
        match (k, &mut inst.kind) {
            (Custom::Modulators, _) => {
                egui::ScrollArea::vertical()
                    .id_salt(("mod_ed_scroll", idx))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        super::modview::editor(
                            ui,
                            &mut cx.modm,
                            idx,
                            &cx.live,
                            &mut self.ip.mod_sel,
                            &mut cx.mod_out,
                        );
                    });
            }
            (Custom::Routings, _) => {
                super::modview::routings(ui, &mut cx.modm, idx, &cx.labels, &mut cx.mod_out);
            }
            (Custom::Vowels, InstrumentKind::Voice(v)) => {
                cx.changed |= super::voiceview::vowels_ui(ui, v);
            }
            (Custom::Words, InstrumentKind::Voice(v)) => {
                cx.changed |= super::voiceview::words_ui(ui, v);
            }
            (Custom::KitPiece, _) => super::genview::kit_piece_ui(ui, &mut self.gen_ui),
            (Custom::Dice, InstrumentKind::Gen(g)) => {
                let mut out = super::genview::GenOut::default();
                super::genview::dice_controls(ui, g, &mut self.gen_ui, sel, &mut out);
                cx.changed |= out.changed;
                if out.bake {
                    cx.acts.push(Act::Bake);
                }
            }
            (Custom::ChipCategories, InstrumentKind::Gen(g)) => {
                let mut out = super::genview::GenOut::default();
                super::genview::chipfx_categories(ui, g, &mut self.gen_ui, &mut out);
                cx.changed |= out.changed;
            }
            (Custom::PluginStatus, InstrumentKind::Plugin(pp)) => {
                let pp = pp.clone();
                self.plugin_status_ui(ui, idx, &pp);
            }
            (Custom::PluginMacros, InstrumentKind::Plugin(pp)) => {
                let pp = pp.clone();
                self.plugin_macro_knobs(ui, idx, &pp);
            }
            (Custom::PluginParams, InstrumentKind::Plugin(pp)) => {
                let pp = pp.clone();
                let params = self.plugin_param_infos(idx);
                if params.is_empty() {
                    ui.label(
                        egui::RichText::new("the plugin is not loaded (see Main › Plugin)")
                            .color(t.ink2),
                    );
                } else {
                    ui.set_min_height(card_h - HEAD_H - 14.0);
                    self.plugin_param_list(ui, idx, &pp, &params);
                }
            }
            (
                Custom::SampleInfo,
                InstrumentKind::Sample {
                    file,
                    rate,
                    data,
                    base_note,
                    lp,
                },
            ) => {
                let name = std::path::Path::new(file.as_str())
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "(recorded / baked)".into());
                ui.add(egui::Label::new(egui::RichText::new(name).strong()).truncate());
                ui.label(
                    egui::RichText::new(format!(
                        "{:.2} s · {} Hz · base {} · loop {:?}",
                        data.len() as f64 / (*rate).max(1) as f64,
                        rate,
                        crate::model::note_name(*base_note),
                        lp.mode
                    ))
                    .small()
                    .color(t.ink2),
                );
                if ui
                    .button("Open in Sample editor")
                    .on_hover_text("loop points, finetune, slicing and editing")
                    .clicked()
                {
                    cx.acts.push(Act::OpenSample);
                }
            }
            (Custom::WaveTable(o), InstrumentKind::Gen(g)) => {
                let o = o as usize;
                if let crate::gen::GenEngine::Wave(w) = &mut g.engine {
                    let name = w.table_name(o);
                    ui.add(egui::Label::new(egui::RichText::new(name).strong()).truncate());
                    ui.horizontal(|ui| {
                        if ui
                            .button("Import WAV…")
                            .on_hover_text(
                                "single-cycle WAV, or a wavetable WAV of 2048-sample frames \
                                 (Serum clm chunk sizes are read); saved inside the song",
                            )
                            .clicked()
                        {
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("WAV", &["wav", "WAV"])
                                .pick_file()
                            {
                                match crate::wavetable::import_wav(&p) {
                                    Ok(u) => {
                                        self.status = format!(
                                            "Imported wavetable {} ({} frames)",
                                            u.name(),
                                            u.table.frames
                                        );
                                        w.user[o] = Some(u);
                                        w.tables[o] = crate::gen_wave::USER;
                                        cx.changed = true;
                                    }
                                    Err(e) => self.status = format!("Wavetable import: {e}"),
                                }
                            }
                        }
                        if w.user[o].is_some()
                            && ui
                                .button("Clear")
                                .on_hover_text("drop the imported table (back to Basic Shapes)")
                                .clicked()
                        {
                            w.user[o] = None;
                            if w.tables[o] == crate::gen_wave::USER {
                                w.tables[o] = 0;
                            }
                            cx.changed = true;
                        }
                    });
                }
            }
            (Custom::FxChain, _) => {
                let names: Vec<String> = inst.fx.iter().map(|d| d.title()).collect();
                ui.label(
                    egui::RichText::new(if names.is_empty() {
                        "no effects".to_string()
                    } else {
                        names.join(" > ")
                    })
                    .small()
                    .color(t.ink2),
                );
                ui.horizontal(|ui| {
                    ui.menu_button("+ Add effect", |ui| {
                        for f in crate::fx::FxKind::ALL {
                            if ui.button(f.name()).clicked() {
                                cx.acts.push(Act::AddFx(f));
                                ui.close();
                            }
                        }
                        ui.separator();
                        if let Some(pp) = self.plugin_fx_menu(ui) {
                            cx.acts.push(Act::AddPluginFx(pp));
                        }
                    });
                    if ui
                        .button("Open FX view")
                        .on_hover_text("reorder, presets, sends")
                        .clicked()
                    {
                        cx.acts.push(Act::OpenFx);
                    }
                });
            }
            _ => {}
        }
    }

    /// plugin macros 1-8 as knobs (+ which parameter each drives)
    fn plugin_macro_knobs(&mut self, ui: &mut egui::Ui, i: usize, pp: &crate::plugin::PluginPatch) {
        use crate::automation::Target;
        let params = self.plugin_param_infos(i);
        if params.is_empty() {
            ui.label(egui::RichText::new("load the plugin to use its macros").color(th().ink2));
            return;
        }
        let vals: Vec<Option<f64>> = (0..8)
            .map(|k| {
                let id = pp.macro_id(k);
                self.with(|s| {
                    let si = s.engine.plugins.primary(i)?;
                    s.engine.plugins.slots[si].inst.value(id)
                })
            })
            .collect();
        let mut set: Vec<(u32, f64)> = vec![];
        for row in 0..2 {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for k in row * 4..row * 4 + 4 {
                    let id = pp.macro_id(k);
                    let info = params.iter().find(|x| x.id == id);
                    let name = info.map(|x| x.name.clone()).unwrap_or_else(|| "-".into());
                    let (mut n, txt, def) = match (info, vals[k]) {
                        (Some(x), Some(v)) => {
                            let span = (x.max - x.min).max(1e-9);
                            (
                                ((v - x.min) / span).clamp(0.0, 1.0) as f32,
                                format!("{:.0} %", (v - x.min) / span * 100.0),
                                ((x.default - x.min) / span).clamp(0.0, 1.0) as f32,
                            )
                        }
                        _ => (0.0, "unset".into(), 0.0),
                    };
                    let label = format!("{} {}", k + 1, name);
                    let tip = format!(
                        "macro {} drives {name}\n43{:X}y in the pattern, automation lanes and MIDI learn drive it",
                        k + 1,
                        k + 1
                    );
                    let out = knob::knob(
                        ui,
                        egui::Id::unique(("ipm", i, k)),
                        &mut n,
                        KnobArgs {
                            name: &label,
                            value_text: txt,
                            default: def,
                            steps: 0.0,
                            enabled: info.is_some(),
                            highlight: false,
                            badge: Some(["M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8"][k]),
                            locked: None,
                            tip: &tip,
                            rings: &[],
                            live: None,
                            auto_led: self.lane_led(Target::Macro(i, k)),
                        },
                    );
                    if out.changed {
                        if let Some(x) = info {
                            set.push((id, x.min + (x.max - x.min) * n as f64));
                        }
                        self.touch_param(Target::Macro(i, k));
                    }
                    let tg = Target::Macro(i, k);
                    let tid = tg.id();
                    let waiting = self.midi.learn.as_deref() == Some(tid.as_str());
                    let r = out.resp.interact(Sense::click());
                    r.context_menu(|ui| {
                        if ui
                            .button(if self.lane_led(tg).is_some() {
                                "Show automation"
                            } else {
                                "Add automation lane"
                            })
                            .clicked()
                        {
                            self.add_lane(tg);
                            ui.close();
                        }
                        ui.separator();
                        self.learn_items(ui, &tid, waiting);
                    });
                }
            });
        }
        if !set.is_empty() {
            self.with(|s| {
                for (id, v) in &set {
                    s.engine.plugins.set_param(i, *id, *v);
                }
            });
            self.plug.state_dirty = true;
            self.dirty = true;
        }
        self.plugin_macro_assign(ui, i, pp, &params);
    }
}
