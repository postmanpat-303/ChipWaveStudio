//! StringWave's own panel: the Instrument tab's simple view for a StringWave
//! instrument (Advanced › still opens the full editor with its tabs). Three
//! sub-tabs fit a 1366x768 laptop (Layers / Filter & Mod / FX & Master);
//! the full view (Shift+F11 or the Full view button) gives the panel the
//! whole window and shows every section. A text-size switch (100-200 %)
//! scales only this panel. Right-click a knob or the empty panel for the
//! pie (`radial.rs`). Every control is the instrument panel's own parameter
//! (`params.rs`), so search, automation lanes, MIDI learn, modulation and
//! the Dice locks work exactly as on the cards; colours come from the theme
//! and the built-in instrument skin (`theme::instskin`).
use super::super::radial::{RIcon, Radial, RadialEv, RadialItem};
use super::*;
use crate::strings::*;

/// the sub-tabs: (name, columns of sections)
pub(in crate::app) const TABS: [(&str, &[&[usize]]); 3] = [
    ("Layers", &[&[0, 1, 2], &[3, 4, 14]]),
    ("Filter & Mod", &[&[5, 6, 15], &[7, 8, 9]]),
    ("FX & Master", &[&[10, 11, 12], &[13, 14]]),
];

/// the full view: every section in two columns
const FULL: &[&[usize]] = &[&[0, 1, 2, 3, 5, 6, 10, 14], &[4, 7, 8, 9, 15, 11, 12, 13]];

/// how many sections there are
const NSEC: usize = 16;

/// the text sizes offered (percent)
pub(in crate::app) const TEXT_SIZES: [u16; 4] = [100, 125, 150, 200];

/// a wedge's outer ring: (label, action)
type Kids = Vec<(String, PieAct)>;

#[derive(Default)]
pub(in crate::app) struct SwUi {
    /// the sub-tab shown
    pub tab: usize,
    /// the full view: the whole window, every section
    pub full: bool,
    /// the open pie
    pub pie: Option<SwPie>,
    /// the frame the panel was last drawn in (Shift+F11, stale pies)
    pub shown_at: u64,
    /// the knobs as last drawn: (param key, rect on screen) (tests, shots)
    pub knobs: Vec<(String, Rect)>,
    /// the panel's body as last drawn, on screen
    pub body: Option<Rect>,
    /// open a pie once drawn: a knob's (param key) or the panel's (None)
    /// (tests, screenshots)
    pub pie_req: Option<Option<String>>,
}

/// what a pie wedge or child does
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum PieAct {
    Reset(String),
    Lock(String),
    Lane(String),
    Learn(String),
    CancelLearn,
    Forget(u8, String),
    /// connect modulator id to the knob's target
    ModWith(u32, String),
    /// a new modulator (ModKind::ALL index) on the knob's target
    ModNew(usize, String),
    /// set a parameter (key, value)
    Set(String, f32),
    TextSize(u16),
    Full,
    Advanced,
    Nothing,
}

pub(in crate::app) struct SwPie {
    pub radial: Radial,
    /// a knob's pie (its param key), else the panel's
    pub knob: Option<String>,
    pub wedge: Vec<PieAct>,
    pub kids: Vec<Vec<PieAct>>,
    seen: u64,
}

/// a section, drawn as a strip: on the left its title, power / mute /
/// solo and a choice or picture; then its other choices (LEDs, combos);
/// then its knobs in one row
struct Sec {
    title: &'static str,
    /// the card whose picture (envelope, filter) it shows
    viz: Option<&'static str>,
    power: Option<String>,
    ms: Option<(String, String)>,
    /// a choice under the title (a layer's wave)
    side: Option<String>,
    /// LED choices in the middle column
    leds: Vec<String>,
    /// combo choices in the middle column
    ctls: Vec<String>,
    /// knobs: (macro index, caption)
    knobs: Vec<(usize, &'static str)>,
    /// the Perform hints (no knobs)
    hint: bool,
    /// the voices meter in the middle column
    meter: bool,
    /// the Movement list (what moves which knob)
    movement: bool,
}

impl Sec {
    fn new(title: &'static str, knobs: Vec<(usize, &'static str)>) -> Self {
        Sec {
            title,
            viz: None,
            power: None,
            ms: None,
            side: None,
            leds: vec![],
            ctls: vec![],
            knobs,
            hint: false,
            meter: false,
            movement: false,
        }
    }
}

fn sections() -> Vec<Sec> {
    let mut v = vec![];
    // ---- Layers ----
    for l in 0..3 {
        let b = LAYER[l];
        let mut s = Sec::new(
            ["Layer 1", "Layer 2", "Layer 3"][l],
            vec![
                (b + L_OCT, "Octave"),
                (b + L_DET, "Detune"),
                (b + L_TONE, "Tone"),
                (b + L_LVL, "Level"),
                (b + L_PAN, "Pan"),
            ],
        );
        s.power = Some(format!("sw.on{l}"));
        s.ms = Some((format!("sw.mute{l}"), format!("sw.solo{l}")));
        s.side = Some(format!("sw.wave{l}"));
        v.push(s);
    }
    let mut s = Sec::new(
        "Undertones",
        vec![
            (SUB_LVL, "Sub"),
            (DRONE_LVL, "Drone"),
            (DRONE_TONE, "Drone tone"),
        ],
    );
    s.power = Some("sw.und".into());
    s.ms = Some(("sw.undmute".into(), "sw.undsolo".into()));
    s.side = Some("sw.suboct".into());
    v.push(s);
    let mut s = Sec::new(
        "Amp",
        vec![
            (AMP_A, "Swell"),
            (AMP_D, "Decay"),
            (AMP_S, "Sustain"),
            (AMP_R, "Release"),
            (VEL, "Velocity"),
            (VOICES, "Voices"),
        ],
    );
    s.viz = Some("Amp envelope");
    v.push(s);
    // ---- Filter & Mod ----
    let mut s = Sec::new(
        "Filter",
        vec![
            (CUTOFF, "Cutoff"),
            (RESO, "Resonance"),
            (F_KEY, "Key track"),
        ],
    );
    s.viz = Some("Filter");
    s.power = Some("sw.filter".into());
    s.leds = vec!["sw.slope".into()];
    v.push(s);
    let mut s = Sec::new(
        "Filter env",
        vec![
            (F_ENV, "Amount"),
            (F_A, "Attack"),
            (F_D, "Decay"),
            (F_S, "Sustain"),
            (F_R, "Release"),
        ],
    );
    s.viz = Some("Filter envelope");
    v.push(s);
    for n in 0..2 {
        let mut s = Sec::new(
            ["LFO 1", "LFO 2"][n],
            vec![(LFO[n], "Rate"), (LFO[n] + 1, "Depth")],
        );
        s.power = Some(format!("sw.lfo{n}"));
        s.ctls = vec![format!("sw.lfow{n}"), format!("sw.lfod{n}")];
        v.push(s);
    }
    let mut s = Sec::new(
        "Mod env",
        vec![
            (ME_A, "Attack"),
            (ME_D, "Decay"),
            (ME_AMT, "Amount"),
            (WHEEL, "Wheel vib."),
        ],
    );
    s.power = Some("sw.me".into());
    s.ctls = vec!["sw.med".into()];
    v.push(s);
    // ---- FX & Master ----
    let mut s = Sec::new(
        "Chorus",
        vec![(CHO_RATE, "Rate"), (CHO_DEPTH, "Depth"), (CHO_MIX, "Mix")],
    );
    s.power = Some("sw.cho".into());
    v.push(s);
    let mut s = Sec::new(
        "Echo",
        vec![
            (ECHO_TIME, "Time"),
            (ECHO_FB, "Feedback"),
            (ECHO_TONE, "Tone"),
            (ECHO_MIX, "Mix"),
        ],
    );
    s.power = Some("sw.echo".into());
    v.push(s);
    let mut s = Sec::new(
        "Reverb",
        vec![(REV_SIZE, "Size"), (REV_DAMP, "Damping"), (REV_MIX, "Mix")],
    );
    s.power = Some("sw.rev".into());
    v.push(s);
    let mut s = Sec::new("Master", vec![(MASTER, "Master")]);
    s.meter = true;
    v.push(s);
    // the Perform hints
    let mut s = Sec::new("Perform", vec![]);
    s.hint = true;
    v.push(s);
    // what moves which knob (the LFOs, the mod env, song-level sources)
    let mut s = Sec::new("Movement", vec![]);
    s.movement = true;
    v.push(s);
    debug_assert_eq!(v.len(), NSEC);
    v
}

/// the left block (title, power / mute / solo, a choice or picture)
const LB: f32 = 104.0;
/// the middle column (LEDs, combos, the voices meter)
const MC: f32 = 136.0;
/// left / right padding inside a strip (the ember screws sit in the corners)
const PADL: f32 = 14.0;
const PADR: f32 = 10.0;
/// a strip's height: one knob cell plus a point above and below
const STRIP_H: f32 = CELL.y + 2.0;
/// the Perform strip's height
const HINT_H: f32 = 50.0;
/// between strips and columns
const GAP: f32 = 3.0;
const COL_GAP: f32 = 8.0;

fn sec_size(sc: &Sec) -> egui::Vec2 {
    if sc.hint {
        return egui::vec2(310.0, HINT_H);
    }
    if sc.movement {
        return egui::vec2(330.0, STRIP_H);
    }
    let mid = !sc.leds.is_empty() || !sc.ctls.is_empty() || sc.meter;
    let w = PADL + LB + if mid { MC + 6.0 } else { 0.0 } + sc.knobs.len() as f32 * CELL.x + PADR;
    egui::vec2(w, STRIP_H)
}

/// a page's size: columns side by side
fn page_size(secs: &[Sec], cols: &[&[usize]]) -> egui::Vec2 {
    let mut w = 0.0;
    let mut h: f32 = 0.0;
    for col in cols {
        let cw = col
            .iter()
            .map(|&s| sec_size(&secs[s]).x)
            .fold(0.0, f32::max);
        let ch: f32 = col.iter().map(|&s| sec_size(&secs[s]).y).sum::<f32>()
            + GAP * (col.len() as f32 - 1.0).max(0.0);
        w += cw + COL_GAP;
        h = h.max(ch);
    }
    egui::vec2(w - COL_GAP + 8.0, h + 4.0)
}

fn find<'a>(l: &'a Layout, key: &str) -> Option<&'a Param> {
    l.params.iter().find(|p| p.key == key)
}

/// the text size setting as a scale (0 = 100 %)
pub(in crate::app) fn text_scale(pct: u16) -> f32 {
    if pct == 0 {
        1.0
    } else {
        (pct as f32 / 100.0).clamp(1.0, 2.0)
    }
}

/// a rect drawn on `layer` as it lands on screen (the text size scales
/// this panel's layer)
fn to_screen(ctx: &egui::Context, layer: egui::LayerId, r: Rect) -> Rect {
    match ctx.layer_transform_to_global(layer) {
        Some(t) => t * r,
        None => r,
    }
}

impl App {
    /// is the StringWave panel on screen (drawn last frame)?
    pub(in crate::app) fn sw_panel_shown(&self, ctx: &egui::Context) -> bool {
        self.ip.sw.shown_at > 0 && self.ip.sw.shown_at + 1 >= ctx.cumulative_frame_nr()
    }

    /// Shift+F11 / the Full view button
    pub(in crate::app) fn toggle_sw_full(&mut self) {
        self.ip.sw.full = !self.ip.sw.full;
        self.status = if self.ip.sw.full {
            "StringWave full view: every section (Shift+F11 goes back)".into()
        } else {
            "StringWave panel back in the lower panel".into()
        };
    }

    /// StringWave's panel (the simple view)
    pub(super) fn strings_panel(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        inst: &mut Instrument,
        cx: &mut Cx,
        n: usize,
    ) {
        let InstrumentKind::Gen(g) = &inst.kind else {
            return;
        };
        if !matches!(g.engine, crate::gen::GenEngine::Strings(_)) {
            return;
        }
        let frame = ui.ctx().cumulative_frame_nr();
        self.ip.sw.shown_at = frame;
        self.ip.sw.knobs.clear();
        self.strings_header(ui, layout, inst, cx, n);
        ui.add_space(2.0);
        let rect = ui.available_rect_before_wrap();
        let rect = Rect::from_min_max(rect.min, rect.max - egui::vec2(0.0, 2.0));
        if rect.width() < 40.0 || rect.height() < 20.0 {
            return;
        }
        ui.allocate_rect(rect, Sense::hover());
        self.ip.sw.body = Some(rect);
        // the empty panel: right-click for the panel pie
        let bg = ui.interact(rect, egui::Id::unique(("sw_bg", cx.idx)), Sense::click());
        let cols: &[&[usize]] = if self.ip.sw.full {
            FULL
        } else {
            TABS[self.ip.sw.tab.min(TABS.len() - 1)].1
        };
        let scale = text_scale(self.extra.sw_text);
        let mut bg_menu = bg.secondary_clicked();
        if scale == 1.0 {
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                self.strings_body(ui, layout, cols, inst, cx);
            });
        } else {
            // the text size: this panel's own layer, scaled about its corner
            let parent = ui.layer_id();
            let local = Rect::from_min_size(Pos2::ZERO, rect.size() / scale);
            let area = egui::Area::new(egui::Id::unique(("sw_zoom", cx.idx)))
                .order(parent.order)
                .fixed_pos(Pos2::ZERO)
                .constrain(false)
                .movable(false)
                .show(ui.ctx(), |ui| {
                    ui.set_clip_rect(local);
                    // the scaled layer is on top: its own empty background
                    let zbg =
                        ui.interact(local, egui::Id::unique(("sw_zbg", cx.idx)), Sense::click());
                    bg_menu |= zbg.secondary_clicked();
                    ui.scope_builder(egui::UiBuilder::new().max_rect(local), |ui| {
                        self.strings_body(ui, layout, cols, inst, cx);
                    });
                });
            let layer = area.response.layer_id;
            ui.ctx().set_transform_layer(
                layer,
                egui::emath::TSTransform::new(rect.min.to_vec2(), scale),
            );
            ui.ctx().set_sublayer(parent, layer);
        }
        if bg_menu {
            // (on screen: the scaled layer's own positions are its own)
            let at = ui
                .ctx()
                .input(|i| i.pointer.interact_pos())
                .unwrap_or(rect.center());
            self.open_sw_pie(None, at, layout, inst, cx, ui.ctx());
        }
        if let Some(req) = self.ip.sw.pie_req.take() {
            let at = match &req {
                Some(k) => self
                    .ip
                    .sw
                    .knobs
                    .iter()
                    .find(|(kk, _)| kk == k)
                    .map(|(_, r)| r.center()),
                None => Some(Pos2::new(rect.right() - 260.0, rect.center().y)),
            };
            if let Some(at) = at {
                self.open_sw_pie(req.as_deref(), at, layout, inst, cx, ui.ctx());
            }
        }
        self.sw_pie_frame(ui.ctx(), layout, inst, cx);
    }

    /// name, badge, preset; the sub-tabs; voices, A/B, text size, full
    /// view, Advanced ›
    fn strings_header(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        inst: &mut Instrument,
        cx: &mut Cx,
        n: usize,
    ) {
        let t = th();
        let idx = cx.idx;
        let tag = cx.tag;
        let voices = self.with(|s| s.engine.inst_voices(idx));
        let max_v = find(layout, &format!("sw.{VOICES}"))
            .map(|p| (p.get)(inst).round() as usize)
            .unwrap_or(16);
        let tabs = |me: &mut Self, ui: &mut egui::Ui| {
            if me.ip.sw.full {
                ui.label(
                    egui::RichText::new("Full view · every section")
                        .small()
                        .color(t.ink2),
                );
                return;
            }
            for (i, (name, _)) in TABS.iter().enumerate() {
                if crate::ember::tab(ui, me.ip.sw.tab == i, name)
                    .on_hover_text("StringWave's sections, one group at a time")
                    .clicked()
                {
                    me.ip.sw.tab = i;
                }
            }
        };
        // narrow windows drop the name (it's on the track) and the voices
        // count (the Master strip has it)
        let w = ui.available_width();
        let (show_name, show_voices) = (w >= 1060.0, w >= 1160.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if ui.small_button("<").on_hover_text("previous instrument").clicked() {
                cx.acts
                    .push(Act::Select(if idx == 0 { n as u8 } else { idx as u8 }));
            }
            if ui.small_button(">").on_hover_text("next instrument").clicked() {
                cx.acts.push(Act::Select(((idx + 1) % n) as u8 + 1));
            }
            if show_name {
                let head = egui::RichText::new(format!("{:02X} {}", idx + 1, inst.name));
                ui.scope(|ui| {
                    ui.set_max_width(170.0);
                    ui.add(
                        egui::Label::new(if t.ember {
                            head.font(crate::theme::label_font(crate::theme::ts::HEADING)).color(t.ink)
                        } else {
                            head.strong().size(crate::theme::ts::px(crate::theme::ts::LARGE))
                        })
                        .truncate(),
                    );
                });
            }
            tag_pill(ui, tag);
            ui.add_space(4.0);
            self.preset_dropdown(ui, inst, cx);
            ui.add_space(8.0);
            tabs(self, ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                let adv = if t.ember {
                    crate::ember::button(ui, "Advanced ›", false)
                } else {
                    ui.button("Advanced ›")
                };
                if adv
                    .on_hover_text("every control of this instrument: the full editor with its tabs")
                    .clicked()
                {
                    self.ip.advanced = true;
                    self.ip.sw.full = false;
                }
                let full = self.ip.sw.full;
                if crate::ember::toggle(
                    ui,
                    full,
                    egui::RichText::new("Full").strong(),
                    "Full view",
                )
                .on_hover_text("Shift+F11: StringWave gets the whole window, every section at once")
                .clicked()
                {
                    self.toggle_sw_full();
                }
                let cur = self.extra.sw_text.max(100);
                let mut pick = cur;
                egui::ComboBox::from_id_salt(("sw_text", idx))
                    .width(56.0)
                    .selected_text(format!("Aa {cur}%"))
                    .show_ui(ui, |ui| {
                        for s in TEXT_SIZES {
                            ui.selectable_value(&mut pick, s, format!("{s} %"));
                        }
                    })
                    .response
                    .on_hover_text("text size: scales this panel only (the UI zoom is in Settings)");
                if pick != cur {
                    self.extra.sw_text = pick;
                    self.save_settings();
                }
                let on_b = self.ip.ab.as_ref().is_some_and(|a| a.on_b);
                for (b, name) in [(true, "B"), (false, "A")] {
                    let r = if t.ember {
                        crate::ember::button(ui, name, on_b == b)
                    } else {
                        ui.selectable_label(on_b == b, egui::RichText::new(name).strong())
                    };
                    let r = r.on_hover_text(
                        "A/B compare: B starts as a copy of A; edit one, switch to hear the other (Ctrl+Z undoes a switch)",
                    );
                    if r.clicked() && on_b != b {
                        cx.acts.push(Act::AbToggle);
                    }
                }
                if show_voices {
                    ui.label(
                        egui::RichText::new(format!("VOICES {voices}/{max_v}"))
                            .small()
                            .monospace()
                            .color(t.ink2),
                    )
                    .on_hover_text("notes sounding on this instrument now / its voice limit");
                }
            });
        });
    }

    /// the page's columns of strips (when they don't fit: the strips
    /// wrap, in a scroll area)
    fn strings_body(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        cols: &[&[usize]],
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let secs = sections();
        let size = page_size(&secs, cols);
        let avail = ui.available_size();
        let fits = size.x <= avail.x + 0.5 && size.y <= avail.y + 0.5;
        egui::ScrollArea::both()
            .id_salt(("sw_body", cx.idx, self.ip.sw.full, self.ip.sw.tab))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let origin = ui.cursor().min + egui::vec2(4.0, 2.0);
                let mut placed: Vec<(usize, Rect)> = vec![];
                if fits {
                    let mut x = origin.x;
                    for col in cols {
                        let cw = col
                            .iter()
                            .map(|&s| sec_size(&secs[s]).x)
                            .fold(0.0, f32::max);
                        let mut y = origin.y;
                        for &s in col.iter() {
                            let z = sec_size(&secs[s]);
                            placed.push((
                                s,
                                Rect::from_min_size(Pos2::new(x, y), egui::vec2(cw, z.y)),
                            ));
                            y += z.y + GAP;
                        }
                        x += cw + COL_GAP;
                    }
                } else {
                    // narrow (a big text size): the strips wrap
                    let ids: Vec<usize> = cols.iter().flat_map(|c| c.iter().copied()).collect();
                    let wmax = (avail.x - 8.0).max(320.0);
                    let (mut x, mut y, mut row_h) = (origin.x, origin.y, 0.0f32);
                    for s in ids {
                        let z = sec_size(&secs[s]);
                        if x > origin.x && x + z.x > origin.x + wmax {
                            x = origin.x;
                            y += row_h + GAP;
                            row_h = 0.0;
                        }
                        placed.push((s, Rect::from_min_size(Pos2::new(x, y), z)));
                        x += z.x + COL_GAP;
                        row_h = row_h.max(z.y);
                    }
                }
                let all = placed.iter().fold(Rect::NOTHING, |a, (_, r)| a.union(*r));
                ui.allocate_rect(
                    Rect::from_min_max(ui.cursor().min, all.max + egui::vec2(4.0, 2.0)),
                    Sense::hover(),
                );
                for (s, r) in placed {
                    self.sw_strip(ui, layout, &secs[s], r, inst, cx);
                }
            });
    }

    /// a compact combo for a choice (the full name for screen readers)
    fn sw_combo(
        &mut self,
        ui: &mut egui::Ui,
        p: &Param,
        inst: &mut Instrument,
        cx: &mut Cx,
        w: f32,
        caption: bool,
    ) {
        let Ctl::Choice(opts) = &p.ctl else {
            return;
        };
        let v = (p.get)(inst);
        let cur = v.round().max(0.0) as usize;
        let txt = if caption {
            format!("{}: {}", p.caption(), p.text(v))
        } else {
            p.text(v)
        };
        let cr = egui::ComboBox::from_id_salt(("swc", cx.idx, &p.key))
            .width(w)
            .selected_text(egui::RichText::new(txt).small())
            .show_ui(ui, |ui| {
                for (k, o) in opts.iter().enumerate() {
                    if ui.selectable_label(cur == k, o).clicked() && cur != k {
                        (p.set)(inst, k as f32);
                        cx.changed = true;
                    }
                }
            });
        let nm = p.name.clone();
        cr.response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &nm));
        let r = if p.tip.is_empty() {
            cr.response.on_hover_text(&p.name)
        } else {
            cr.response.on_hover_text(format!("{}: {}", p.name, p.tip))
        };
        self.param_menu(&r, p, inst, cx, None);
    }

    /// one section as a strip in `rect`
    fn sw_strip(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        sc: &Sec,
        rect: Rect,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let t = th();
        super::instcard::frame(ui, rect, "");
        let p = ui.painter().clone();
        let tpos = Pos2::new(rect.left() + PADL + 2.0, rect.top() + 11.0);
        if t.ember {
            crate::ember::title(&p, tpos, sc.title);
        } else {
            p.text(
                tpos,
                egui::Align2::LEFT_CENTER,
                sc.title.to_uppercase(),
                crate::theme::prop(crate::theme::ts::LABEL),
                t.ink,
            );
        }
        let clip = rect.intersect(ui.clip_rect());
        let child = |ui: &mut egui::Ui, r: Rect, lay: egui::Layout| {
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(lay));
            c.set_clip_rect(r.intersect(clip));
            c
        };
        if sc.movement {
            let n = self.quick_rows(inst, cx).len();
            if n > 0 {
                ui.painter().text(
                    Pos2::new(rect.right() - PADR, rect.top() + 10.0),
                    egui::Align2::RIGHT_CENTER,
                    format!("{n} active"),
                    crate::theme::prop(crate::theme::ts::SMALL),
                    th().ink2,
                );
            }
            let mut c = child(
                ui,
                Rect::from_min_max(
                    Pos2::new(rect.left() + PADL, rect.top() + 19.0),
                    rect.max - egui::vec2(PADR, 3.0),
                ),
                egui::Layout::top_down(egui::Align::Min),
            );
            c.spacing_mut().item_spacing.y = 1.0;
            // compact rows: two fit in the strip
            c.spacing_mut().interact_size.y = 16.0;
            c.spacing_mut().button_padding.y = 0.0;
            egui::ScrollArea::vertical()
                .id_salt(("sw_movement", cx.idx))
                .auto_shrink([false, false])
                .show(&mut c, |ui| self.movement_list(ui, inst, cx, false));
            return;
        }
        if sc.hint {
            let mut c = child(
                ui,
                Rect::from_min_max(
                    Pos2::new(rect.left() + PADL, rect.top() + 21.0),
                    rect.max - egui::vec2(PADR, 2.0),
                ),
                egui::Layout::top_down(egui::Align::Min),
            );
            c.spacing_mut().item_spacing.y = 1.0;
            for line in [
                "Right-click a knob: reset, lock, automate, learn, modulate",
                "CC1 vibrato · CC11 expression · CC64 sustain",
            ] {
                c.label(egui::RichText::new(line).small().color(t.ink2));
            }
            return;
        }
        let x0 = rect.left() + PADL;
        let row = |k: f32| rect.top() + 21.0 + k * 23.0;
        // row 2: power, mute, solo
        let mut items: Vec<(&str, &str)> = vec![];
        if let Some(p) = &sc.power {
            items.push((p.as_str(), "On"));
        }
        if let Some((m, s)) = &sc.ms {
            items.push((m.as_str(), "M"));
            items.push((s.as_str(), "S"));
        }
        if !items.is_empty() {
            let mut c = child(
                ui,
                Rect::from_min_size(Pos2::new(x0, row(0.0)), egui::vec2(LB, 21.0)),
                egui::Layout::left_to_right(egui::Align::Center),
            );
            c.spacing_mut().item_spacing.x = 3.0;
            for (key, label) in items {
                let Some(p) = find(layout, key) else {
                    continue;
                };
                let on = (p.get)(inst) > 0.5;
                let r = if label == "On" {
                    crate::ember::led_item(&mut c, "On", on)
                } else {
                    crate::ember::button_sized(&mut c, label, on, 22.0)
                };
                let name = p.name.clone();
                r.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, &name));
                let r = r.on_hover_text(format!("{}: {}", p.name, p.tip));
                if r.clicked() {
                    (p.set)(inst, if on { 0.0 } else { 1.0 });
                    cx.changed = true;
                }
                self.param_menu(&r, p, inst, cx, None);
            }
        }
        // row 3 (or rows 2-3): the choice or the picture
        let lower = if sc.power.is_some() || sc.ms.is_some() {
            Rect::from_min_size(Pos2::new(x0, row(1.0)), egui::vec2(LB - 6.0, 21.0))
        } else {
            Rect::from_min_max(
                Pos2::new(x0, row(0.0)),
                Pos2::new(x0 + LB - 6.0, rect.bottom() - 4.0),
            )
        };
        if let Some(key) = &sc.side {
            if let Some(p) = find(layout, key) {
                let mut c = child(ui, lower, egui::Layout::left_to_right(egui::Align::Center));
                self.sw_combo(&mut c, p, inst, cx, LB - 14.0, false);
            }
        } else if let Some(card) = sc.viz {
            if let Some(v) = layout
                .cards
                .iter()
                .find(|c| c.name == card)
                .and_then(|c| c.viz.as_ref())
            {
                knob::draw_viz(&ui.painter_at(lower.intersect(clip)), lower, &v(inst));
            }
        }
        // the middle column
        let mut x = x0 + LB;
        if !sc.leds.is_empty() || !sc.ctls.is_empty() || sc.meter {
            let mr = Rect::from_min_size(
                Pos2::new(x, rect.top() + 4.0),
                egui::vec2(MC, rect.height() - 8.0),
            );
            let mut c = child(ui, mr, egui::Layout::top_down(egui::Align::Min));
            c.spacing_mut().item_spacing.y = 3.0;
            for key in &sc.leds {
                if let Some(p) = find(layout, key) {
                    c.label(egui::RichText::new(p.caption()).small().color(t.ink2));
                    super::polyview::led_row(&mut c, p, inst, cx);
                }
            }
            for key in &sc.ctls {
                if let Some(p) = find(layout, key) {
                    self.sw_combo(&mut c, p, inst, cx, MC - 12.0, true);
                }
            }
            if sc.meter {
                let idx = cx.idx;
                let voices = self.with(|s| s.engine.inst_voices(idx));
                let max_v = find(layout, &format!("sw.{VOICES}"))
                    .map(|p| (p.get)(inst).round().max(1.0))
                    .unwrap_or(16.0);
                c.add_space(6.0);
                c.label(
                    egui::RichText::new(format!("VOICES {voices}/{}", max_v as usize))
                        .small()
                        .monospace()
                        .color(t.ink2),
                )
                .on_hover_text("notes sounding on this instrument now / its voice limit");
                let (r, _) = c.allocate_exact_size(egui::vec2(MC - 16.0, 8.0), Sense::hover());
                crate::ember::dot_meter(
                    c.painter(),
                    r,
                    (voices as f32 / max_v).clamp(0.0, 1.0),
                    None,
                    false,
                    false,
                );
            }
            x += MC + 6.0;
        }
        // the knobs, one row
        let kr = Rect::from_min_max(
            Pos2::new(x, rect.top() + 1.0),
            rect.max - egui::vec2(0.0, 1.0),
        );
        let mut c = child(ui, kr, egui::Layout::left_to_right(egui::Align::Min));
        c.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        for (k, cap) in &sc.knobs {
            let key = format!("sw.{k}");
            let Some(p) = find(layout, &key) else {
                continue;
            };
            let Some((resp, _)) = self.knob_core(&mut c, p, inst, cx, Some(cap)) else {
                continue;
            };
            let scr = to_screen(c.ctx(), resp.layer_id, resp.rect);
            self.ip.sw.knobs.push((key.clone(), scr));
            let learning = p
                .target
                .is_some_and(|tg| self.midi.learn.as_deref() == Some(tg.id().as_str()));
            if learning {
                super::super::midiview::ui_ring(c.ctx(), scr);
            }
            if resp.secondary_clicked() {
                let at = c.ctx().pointer_interact_pos().unwrap_or(scr.center());
                self.open_sw_pie(Some(&key), at, layout, inst, cx, c.ctx());
            }
        }
    }

    // ------------------------------------------------------------- the pie

    /// the quick wedge for a section: (label, wedge action, children)
    fn sw_quick(layout: &Layout, inst: &Instrument, sc: &Sec) -> Option<(String, PieAct, Kids)> {
        let tog = |key: &str, on: &str, off: &str| -> Option<(String, PieAct)> {
            let p = find(layout, key)?;
            let cur = (p.get)(inst) > 0.5;
            Some((
                if cur { off } else { on }.to_string(),
                PieAct::Set(key.into(), if cur { 0.0 } else { 1.0 }),
            ))
        };
        let mut kids = vec![];
        if let Some(pw) = &sc.power {
            kids.extend(tog(pw, "Turn on", "Turn off"));
        }
        if let Some((m, s)) = &sc.ms {
            kids.extend(tog(m, "Mute", "Unmute"));
            kids.extend(tog(s, "Solo", "Unsolo"));
        }
        for key in &sc.leds {
            if let Some(p) = find(layout, key) {
                if let Ctl::Choice(opts) = &p.ctl {
                    for (i, o) in opts.iter().enumerate() {
                        kids.push((o.clone(), PieAct::Set(key.clone(), i as f32)));
                    }
                }
            }
        }
        if kids.is_empty() {
            return None;
        }
        let first = kids[0].1.clone();
        Some((sc.title.to_string(), first, kids))
    }

    /// the FX wedge: each effect on / off
    fn sw_fx_kids(layout: &Layout, inst: &Instrument) -> Vec<(String, PieAct)> {
        [
            ("sw.cho", "Chorus"),
            ("sw.echo", "Echo"),
            ("sw.rev", "Reverb"),
        ]
        .iter()
        .filter_map(|(k, n)| {
            let p = find(layout, k)?;
            let on = (p.get)(inst) > 0.5;
            Some((
                format!("{n} {}", if on { "off" } else { "on" }),
                PieAct::Set(k.to_string(), if on { 0.0 } else { 1.0 }),
            ))
        })
        .collect()
    }

    /// open the pie for a knob (its param key) or the empty panel
    fn open_sw_pie(
        &mut self,
        knob: Option<&str>,
        at: Pos2,
        layout: &Layout,
        inst: &Instrument,
        cx: &Cx,
        ctx: &egui::Context,
    ) {
        let secs = sections();
        let mut items: Vec<RadialItem> = vec![];
        let mut wedge: Vec<PieAct> = vec![];
        let mut kids: Vec<Vec<PieAct>> = vec![];
        let mut push =
            |label: String, icon: Option<RIcon>, w: PieAct, ch: Vec<(String, PieAct)>| {
                items.push(RadialItem::new(
                    label,
                    icon,
                    ch.iter().map(|c| c.0.clone()).collect(),
                ));
                wedge.push(w);
                kids.push(ch.into_iter().map(|c| c.1).collect());
            };
        let (title, sub) = if let Some(key) = knob {
            let Some(p) = find(layout, key) else {
                return;
            };
            push(
                "Reset".into(),
                Some(RIcon::Transpose),
                PieAct::Reset(key.into()),
                vec![],
            );
            if let Some((locked, _)) = &p.lock {
                push(
                    if locked(inst) {
                        "Unlock for dice"
                    } else {
                        "Lock for dice"
                    }
                    .into(),
                    Some(RIcon::Select),
                    PieAct::Lock(key.into()),
                    vec![],
                );
            }
            if let Some(tg) = p.target {
                let id = tg.id();
                push(
                    if self.lane_led(tg).is_some() {
                        "Show automation"
                    } else {
                        "Automate"
                    }
                    .into(),
                    Some(RIcon::Edit),
                    PieAct::Lane(key.into()),
                    vec![],
                );
                let waiting = self.midi.learn.as_deref() == Some(id.as_str());
                let forget: Vec<(String, PieAct)> = self
                    .extra
                    .midi
                    .learn
                    .iter()
                    .filter(|(_, t)| *t == id)
                    .map(|(c, _)| (format!("Forget CC {c}"), PieAct::Forget(*c, id.clone())))
                    .collect();
                push(
                    if waiting {
                        "Cancel learn"
                    } else {
                        "MIDI learn"
                    }
                    .into(),
                    Some(RIcon::Play),
                    if waiting {
                        PieAct::CancelLearn
                    } else {
                        PieAct::Learn(key.into())
                    },
                    forget,
                );
                if crate::automation::modulatable(tg) {
                    let m = &cx.modm;
                    let mut ch: Vec<(String, PieAct)> = (0..m.mods.len())
                        .filter(|&k| {
                            m.mods[k].owner == cx.idx && !m.routes_to(&id).any(|(_, j)| j == k)
                        })
                        .map(|k| (m.title(k), PieAct::ModWith(m.mods[k].id, id.clone())))
                        .collect();
                    for (i, kind) in crate::modulation::ModKind::ALL.iter().enumerate() {
                        ch.push((
                            format!("New {}", kind.name()),
                            PieAct::ModNew(i, id.clone()),
                        ));
                    }
                    let first = ch
                        .iter()
                        .find(|c| matches!(c.1, PieAct::ModNew(..)))
                        .map(|c| c.1.clone())
                        .unwrap_or(PieAct::Nothing);
                    push("Modulate".into(), Some(RIcon::Generate), first, ch);
                }
            }
            let k: usize = key
                .strip_prefix("sw.")
                .and_then(|s| s.parse().ok())
                .unwrap_or(usize::MAX);
            if let Some(sc) = secs.iter().find(|s| s.knobs.iter().any(|(i, _)| *i == k)) {
                if let Some((label, w, ch)) = Self::sw_quick(layout, inst, sc) {
                    push(label, Some(RIcon::More), w, ch);
                } else if ["Chorus", "Echo", "Reverb", "Master"].contains(&sc.title) {
                    push(
                        "FX".into(),
                        Some(RIcon::More),
                        PieAct::Nothing,
                        Self::sw_fx_kids(layout, inst),
                    );
                }
            }
            let real = (p.get)(inst);
            (p.name.clone(), p.text(real))
        } else {
            for sc in secs
                .iter()
                .filter(|s| s.ms.is_some() || s.title == "Filter")
            {
                if sc.title == "Undertones" {
                    continue;
                }
                if let Some((label, w, ch)) = Self::sw_quick(layout, inst, sc) {
                    push(label, Some(RIcon::More), w, ch);
                }
            }
            push(
                "FX".into(),
                Some(RIcon::More),
                PieAct::Nothing,
                Self::sw_fx_kids(layout, inst),
            );
            let cur = self.extra.sw_text.max(100);
            let next = TEXT_SIZES
                .iter()
                .copied()
                .find(|&s| s > cur)
                .unwrap_or(TEXT_SIZES[0]);
            push(
                "Text size".into(),
                Some(RIcon::Type),
                PieAct::TextSize(next),
                TEXT_SIZES
                    .iter()
                    .map(|&s| (format!("{s} %"), PieAct::TextSize(s)))
                    .collect(),
            );
            push(
                if self.ip.sw.full {
                    "Exit full view"
                } else {
                    "Full view"
                }
                .into(),
                Some(RIcon::Bulb),
                PieAct::Full,
                vec![],
            );
            push(
                "Advanced".into(),
                Some(RIcon::Edit),
                PieAct::Advanced,
                vec![],
            );
            (STRINGS_DISPLAY_NAME.to_string(), inst.name.clone())
        };
        let mut radial = Radial::new(at, title, sub, items);
        radial.accent = crate::theme::instskin().map(|s| s.tint);
        radial.clamp_to(ctx.content_rect());
        self.ip.sw.pie = Some(SwPie {
            radial,
            knob: knob.map(String::from),
            wedge,
            kids,
            seen: ctx.cumulative_frame_nr(),
        });
    }

    /// every frame the panel is drawn: the open pie
    fn sw_pie_frame(
        &mut self,
        ctx: &egui::Context,
        layout: &Layout,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let Some(mut pie) = self.ip.sw.pie.take() else {
            return;
        };
        let fr = ctx.cumulative_frame_nr();
        if pie.seen + 1 < fr {
            // the panel wasn't drawn meanwhile: a stale pie
            return;
        }
        pie.seen = fr;
        // the knob the pie is for keeps a ring while it's open
        if let Some(r) = pie
            .knob
            .as_ref()
            .and_then(|k| self.ip.sw.knobs.iter().find(|(kk, _)| kk == k))
            .map(|(_, r)| *r)
        {
            let col = crate::theme::instskin().map_or(th().accent, |s| s.tint);
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("sw_pie_ring"),
            ))
            .rect_stroke(
                r.expand(2.0),
                6,
                Stroke::new(2.0, col),
                egui::StrokeKind::Outside,
            );
        }
        match pie.radial.show(ctx) {
            RadialEv::Cancel => {}
            RadialEv::Pick(i, c) => {
                let act = match c {
                    Some(c) => pie.kids.get(i).and_then(|k| k.get(c)).cloned(),
                    None => pie.wedge.get(i).cloned(),
                };
                if let Some(a) = act {
                    self.sw_pie_act(a, layout, inst, cx);
                }
            }
            _ => self.ip.sw.pie = Some(pie),
        }
    }

    fn sw_pie_act(&mut self, a: PieAct, layout: &Layout, inst: &mut Instrument, cx: &mut Cx) {
        let target = |key: &str| find(layout, key).and_then(|p| p.target);
        match a {
            PieAct::Reset(k) => {
                if let Some(p) = find(layout, &k) {
                    if let Ctl::Knob { def, .. } = p.ctl {
                        (p.set)(inst, def);
                        cx.changed = true;
                        self.status = format!("{} reset", p.name);
                    }
                }
            }
            PieAct::Lock(k) => {
                if let Some(p) = find(layout, &k) {
                    let Some((locked, f)) = p.lock.as_ref() else {
                        return;
                    };
                    f(inst);
                    cx.changed = true;
                    self.status = format!(
                        "{}: {}",
                        p.name,
                        if locked(inst) {
                            "locked, the dice leave it alone"
                        } else {
                            "unlocked for the dice"
                        }
                    );
                }
            }
            PieAct::Lane(k) => {
                if let Some(tg) = target(&k) {
                    cx.acts.push(Act::Lane(tg));
                }
            }
            PieAct::Learn(k) => {
                if let Some(tg) = target(&k) {
                    let id = tg.id();
                    self.status = format!(
                        "MIDI learn: move a knob or the mod wheel to control {}",
                        super::super::midiview::target_text(&id)
                    );
                    self.midi.learn = Some(id);
                }
            }
            PieAct::CancelLearn => self.midi.learn = None,
            PieAct::Forget(cc, id) => {
                self.extra
                    .midi
                    .learn
                    .retain(|(c, t)| !(*c == cc && *t == id));
                self.save_settings();
            }
            PieAct::ModWith(m, id) => {
                cx.modm
                    .connect(m, &id, super::super::modview::DEFAULT_DEPTH);
                self.ip.mod_sel = Some(m);
            }
            PieAct::ModNew(i, id) => {
                if let Some(kind) = crate::modulation::ModKind::ALL.get(i) {
                    if let Some(k) = cx.modm.add(cx.idx, *kind) {
                        let mid = cx.modm.mods[k].id;
                        cx.modm
                            .connect(mid, &id, super::super::modview::DEFAULT_DEPTH);
                        self.ip.mod_sel = Some(mid);
                    }
                }
            }
            PieAct::Set(k, v) => {
                if let Some(p) = find(layout, &k) {
                    (p.set)(inst, v);
                    cx.changed = true;
                }
            }
            PieAct::TextSize(s) => {
                self.extra.sw_text = s;
                self.save_settings();
            }
            PieAct::Full => self.toggle_sw_full(),
            PieAct::Advanced => {
                self.ip.advanced = true;
                self.ip.sw.full = false;
            }
            PieAct::Nothing => {}
        }
    }
}

/// the knob keys a sub-tab shows (tests)
#[cfg(test)]
pub(in crate::app) fn tab_knob_keys(tab: usize) -> Vec<String> {
    let secs = sections();
    TABS[tab]
        .1
        .iter()
        .flat_map(|c| c.iter())
        .flat_map(|&s| secs[s].knobs.iter().map(|(k, _)| format!("sw.{k}")))
        .collect()
}
