//! Menu bar + grouped toolbar (0.1.9 redesign) and the recovery dialog.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Ic {
    Play,
    PlaySong,
    Stop,
    Edit,
}

/// Painted icon (no emoji fonts needed).
fn paint_icon(p: &egui::Painter, r: Rect, ic: Ic, col: Color32) {
    let c = r.center();
    match ic {
        Ic::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + egui::vec2(-4.0, -6.0),
                    c + egui::vec2(6.0, 0.0),
                    c + egui::vec2(-4.0, 6.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Ic::PlaySong => {
            // play triangle + "list" bars = whole song
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + egui::vec2(-7.0, -6.0),
                    c + egui::vec2(2.0, 0.0),
                    c + egui::vec2(-7.0, 6.0),
                ],
                col,
                Stroke::NONE,
            ));
            for dy in [-5.0, 0.0, 5.0] {
                p.line_segment(
                    [c + egui::vec2(4.0, dy), c + egui::vec2(8.0, dy)],
                    Stroke::new(1.8, col),
                );
            }
        }
        Ic::Stop => {
            p.rect_filled(Rect::from_center_size(c, egui::vec2(10.0, 10.0)), 1, col);
        }
        Ic::Edit => {
            p.circle_filled(c, 5.0, col);
        }
    }
}

/// Toolbar button: painted icon + text label.
fn tool_button(
    ui: &mut egui::Ui,
    ic: Ic,
    label: &str,
    on: bool,
    icon_col: Color32,
) -> egui::Response {
    tool_button_w(ui, ic, label, on, icon_col, 0.0)
}

/// [`tool_button_w`]; `icon_only` hides the label (the LCD toolbar) but
/// keeps it as the button's name
fn tool_button_opt(
    ui: &mut egui::Ui,
    ic: Ic,
    label: &str,
    on: bool,
    icon_col: Color32,
    min_w: f32,
    icon_only: bool,
) -> egui::Response {
    let shown = if icon_only { "" } else { label };
    let min_w = if icon_only { 0.0 } else { min_w };
    tool_button_named(ui, shown, label, on, min_w, |p, ir, v_text| {
        let ic_col = if on && th().light { v_text } else { icon_col };
        paint_icon(p, ir, ic, ic_col);
    })
}

/// [`tool_button`] at least `min_w` wide (a toggle whose label changes
/// keeps its size)
fn tool_button_w(
    ui: &mut egui::Ui,
    ic: Ic,
    label: &str,
    on: bool,
    icon_col: Color32,
    min_w: f32,
) -> egui::Response {
    tool_button_with(ui, label, on, min_w, |p, ir, v_text| {
        // light themes: active buttons are filled, so draw the icon in the text colour
        let ic_col = if on && th().light { v_text } else { icon_col };
        paint_icon(p, ir, ic, ic_col);
    })
}

/// a toolbar button with its icon painted by `icon` (painter, the 18x18
/// icon rect, the button's text colour)
fn tool_button_with(
    ui: &mut egui::Ui,
    label: &str,
    on: bool,
    min_w: f32,
    icon: impl FnOnce(&egui::Painter, Rect, Color32),
) -> egui::Response {
    tool_button_named(ui, label, label, on, min_w, icon)
}

/// `tool_button_with` showing `label` (may be empty: icon only) while
/// screen readers and the UI tests see `name`
fn tool_button_named(
    ui: &mut egui::Ui,
    label: &str,
    name: &str,
    on: bool,
    min_w: f32,
    icon: impl FnOnce(&egui::Painter, Rect, Color32),
) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let text_w = if label.is_empty() {
        -6.0
    } else {
        galley.size().x
    };
    let size = egui::vec2(
        (10.0 + 18.0 + 6.0 + text_w + 10.0).max(min_w),
        (galley.size().y + 8.0).max(26.0),
    );
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    // screen readers (and the UI tests) see its name
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), name));
    if ui.is_rect_visible(r) {
        let v = ui.style().interact_selectable(&resp, on);
        let p = ui.painter();
        p.rect(r, 5, v.weak_bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
        theme::raise(p, r, 5.0, resp.is_pointer_button_down_on() || on);
        let ir = Rect::from_center_size(
            Pos2::new(r.left() + 19.0, r.center().y),
            egui::vec2(18.0, 18.0),
        );
        // the text follows the face (Amiga's pale selected fill under the
        // white selection text was 1.5:1)
        let col = if theme::contrast(v.text_color(), v.weak_bg_fill) < 4.5 {
            theme::on(v.weak_bg_fill)
        } else {
            v.text_color()
        };
        icon(p, ir, col);
        p.galley(
            Pos2::new(ir.right() + 6.0, r.center().y - galley.size().y / 2.0),
            galley,
            col,
        );
    }
    resp
}

/// segmented control; returns the clicked index
fn segmented(ui: &mut egui::Ui, items: &[(&str, &str)], cur: usize) -> Option<usize> {
    let mut out = None;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let gs: Vec<_> = items
        .iter()
        .map(|(t, _)| {
            ui.painter()
                .layout_no_wrap(t.to_string(), font.clone(), Color32::PLACEHOLDER)
        })
        .collect();
    let w: f32 = gs.iter().map(|g| g.size().x + 22.0).sum();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, 26.0), Sense::hover());
    let t = th();
    ui.painter().rect(
        r,
        6,
        Color32::from_black_alpha(90),
        Stroke::new(1.0, Color32::from_white_alpha(40)),
        egui::StrokeKind::Inside,
    );
    let mut x = r.left();
    for (i, g) in gs.into_iter().enumerate() {
        let sw = g.size().x + 22.0;
        let sr = Rect::from_min_size(Pos2::new(x, r.top()), egui::vec2(sw, 26.0)).shrink(2.0);
        let resp = ui
            .interact(
                sr,
                ui.make_persistent_id(("seg", items[i].0)),
                Sense::click(),
            )
            .on_hover_text(items[i].1);
        let label = items[i].0;
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, i == cur, label));
        let sel = i == cur;
        let on_fill = if t.light { Color32::WHITE } else { t.accent };
        if sel {
            ui.painter().rect_filled(sr, 4, on_fill);
            theme::raise(ui.painter(), sr, 4.0, false);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(sr, 4, Color32::from_white_alpha(30));
        }
        let col = if sel {
            if t.light {
                t.hdr_bot
            } else {
                t.crt_bot
            }
        } else {
            hdr_text()
        };
        ui.painter().galley(sr.center() - g.size() / 2.0, g, col);
        if i + 1 < items.len() && !sel && i + 1 != cur {
            ui.painter().vline(
                sr.right() + 2.0,
                sr.y_range().shrink(4.0),
                Stroke::new(1.0, Color32::from_white_alpha(45)),
            );
        }
        if resp.clicked() {
            out = Some(i);
        }
        x += sw;
    }
    out
}

/// a toolbar group (calmer header: no visible caption; it's the
/// group's hover text)
/// the top bar's drop marker (every theme and LCD colour)
const DROP_ORANGE: Color32 = Color32::from_rgb(0xFF, 0x8C, 0x1A);

fn group(ui: &mut egui::Ui, caption: &str, add: impl FnOnce(&mut egui::Ui)) {
    let ember = th().ember;
    egui::Frame::new()
        .fill(if ember {
            hex(0x17191C)
        } else {
            Color32::from_black_alpha(if th().light { 40 } else { 70 })
        })
        .stroke(if ember {
            Stroke::new(1.0, th().edge)
        } else {
            Stroke::new(1.0, Color32::from_white_alpha(18))
        })
        .corner_radius(if ember { 5 } else { 7 })
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                add(ui);
            });
        })
        .response
        .on_hover_text(caption);
}

/// `group` when `on` (tracker-only groups show in the Tracker view only)
fn group_if(on: bool, ui: &mut egui::Ui, caption: &str, add: impl FnOnce(&mut egui::Ui)) {
    if on {
        group(ui, caption, add);
    }
}

/// the audio status hover text: device, latency, underruns, load
pub(super) fn audio_tip(a: &audio::Audio) -> String {
    format!(
        "audio: {} @ {} Hz · buffer {}\nlatency queued ahead of the device \
         (grows after underruns, up to {:.0} ms) and buffer underruns\n\
         engine load {:.0} % of real time{}",
        a.device_name,
        a.sample_rate,
        a.buffer,
        audio::MAX_TARGET as f32 * 1000.0 / a.sample_rate as f32,
        a.load_pct(),
        a.advice().map(|s| format!("\n{s}")).unwrap_or_default()
    )
}

pub(super) fn hdr_text() -> Color32 {
    if th().light {
        Color32::WHITE
    } else {
        th().ink
    }
}

pub(super) fn hdr_dim() -> Color32 {
    if th().light {
        // opaque: 4.6:1 or more on the whole header gradient
        hex(0xDCE4F0)
    } else {
        th().ink2
    }
}

fn item(ui: &mut egui::Ui, text: &str, sc: &str) -> bool {
    ui.add(egui::Button::new(text).shortcut_text(sc)).clicked()
}

pub(super) const VIEWS: [(View, &str, &str); 6] = [
    (View::Pattern, "Tracker", "the Tracker (Ctrl+1)"),
    (
        View::Arranger,
        "Order list",
        "the tracker song's blocks in order (Ctrl+2)",
    ),
    (
        View::Timeline,
        "Timeline",
        "tracks and clips over time, like a DAW (Ctrl+T)",
    ),
    (
        View::Mixer,
        "Mixer",
        "channel faders, sends, returns (Ctrl+3)",
    ),
    (View::Fx, "FX", "instrument FX chain (Ctrl+4)"),
    (View::Sample, "Sample", "sample editor (Ctrl+5)"),
];

/// the default shortcut of a view (View menu)
pub(super) fn view_shortcut(v: View) -> &'static str {
    match v {
        View::Pattern => "Ctrl+1",
        View::Arranger => "Ctrl+2",
        View::Timeline => "Ctrl+T",
        View::Mixer => "Ctrl+3",
        View::Fx => "Ctrl+4",
        View::Sample => "Ctrl+5",
        View::Settings => "Ctrl+,",
    }
}

/// donation link on Help > About (opens in the system browser)
pub(super) const DONATE_URL: &str = "https://buymeacoffee.com/postmanpat";
pub(super) const PAYPAL_URL: &str = "https://paypal.me/derekh100";
pub(super) const YOUTUBE_URL: &str = "https://www.youtube.com/@whatgoesaround1551";

// The About credits live in `crate::credits::CREDITS` (one structured list).

impl App {
    pub(super) fn synth_key(&mut self, ctx: &egui::Context, key: Key, modifiers: egui::Modifiers) {
        self.pending.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });
        ctx.request_repaint();
    }

    pub(super) fn do_undo(&mut self, redo: bool) {
        if self.sample_undo_here() {
            self.sample_undo(redo);
        } else {
            self.undo(redo);
        }
    }

    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let cmd = egui::Modifiers::COMMAND;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.add_space(6.0);
            if th().ember {
                // Slate: CHIP in ink, WAVE in the accent (label-font capitals)
                let mut job = egui::text::LayoutJob::default();
                let f = theme::label_font(crate::theme::ts::HEADING);
                job.append("CHIP", 0.0, egui::TextFormat::simple(f.clone(), th().ink));
                job.append("WAVE", 0.0, egui::TextFormat::simple(f, th().accent));
                ui.label(job).on_hover_text("ChipWave Studio");
            } else {
                ui.label(
                    egui::RichText::new("ChipWave")
                        .strong()
                        .size(crate::theme::ts::px(crate::theme::ts::HEADING))
                        .color(hdr_text()),
                );
            }
            ui.add_space(6.0);
            ui.menu_button("File", |ui| {
                if item(ui, "New", "Ctrl+N") {
                    self.synth_key(&ctx, Key::N, cmd);
                }
                if item(ui, "Open…", "Ctrl+O") {
                    self.request(crate::prompt::Pending::Open, &ctx);
                }
                ui.menu_button("Demo songs", |ui| {
                    for (i, (name, about, _)) in fileio::DEMOS.iter().enumerate() {
                        if ui.button(*name).on_hover_text(*about).clicked() {
                            self.request(crate::prompt::Pending::Demo(i), &ctx);
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("open a built-in demo song (unsaved; Save As keeps a copy)");
                ui.menu_button("Recent", |ui| self.recent_menu(ui));
                ui.separator();
                if item(ui, "Save", "Ctrl+S") {
                    self.save(false);
                }
                if item(ui, "Save As…", "Ctrl+Shift+S") {
                    self.save(true);
                }
                ui.separator();
                if item(ui, "Export…  (WAV / MP3)", "Ctrl+Shift+E") {
                    self.open_export();
                }
                if item(ui, "Export MP3…", "") {
                    self.open_export();
                    if let Some(d) = &mut self.export_dlg {
                        d.opts.format = crate::export::Format::Mp3;
                    }
                }
                if item(ui, "Quick export WAV…", "Ctrl+E") {
                    self.export_wav();
                }
                let tl_mode = self.with(|s| s.song.timeline.is_some());
                if ui
                    .add_enabled(tl_mode, egui::Button::new("Export tracker-compatible copy…"))
                    .on_hover_text(
                        "save a tracker-mode copy of this timeline-mode song (blocks + order list) \
                         that ChipWave Studio 0.2.4.0 and older can open; it plays the same",
                    )
                    .on_disabled_hover_text("the song is in tracker mode already: Save is compatible")
                    .clicked()
                {
                    self.export_tracker_copy();
                    ui.close();
                }
                ui.separator();
                if item(ui, "Instrument library…", "") {
                    self.open_library();
                    ui.close();
                }
                if item(ui, "Import instrument (.cwi)…", "") {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("ChipWave instrument", &[crate::instfile::EXT])
                        .pick_file()
                    {
                        self.import_cwi(&p);
                    }
                }
                if item(ui, "Export instrument (.cwi)…", "") {
                    self.export_instrument(self.instr);
                }
                ui.separator();
                if item(ui, "Save controller map…", "") {
                    self.save_controller_map();
                }
                if item(ui, "Load controller map…", "") {
                    self.load_controller_map();
                }
                ui.separator();
                if item(ui, "Quit", "Ctrl+Q") {
                    self.request(crate::prompt::Pending::Quit, &ctx);
                }
            });
            ui.menu_button("Edit", |ui| {
                // the sample editor has its own history
                let main = !self.sample_undo_here();
                let info = format!(
                    "{} undo step(s) kept, {:.1} MB",
                    self.hist.len(),
                    self.hist.bytes() as f64 / 1e6
                );
                // a named step reads "Undo: Move clip" (app/action.rs); the
                // sample editor's own history stays plain
                let (undo_t, redo_t) = if main {
                    (self.undo_text(false), self.undo_text(true))
                } else {
                    ("Undo".to_string(), "Redo".to_string())
                };
                if ui
                    .add_enabled(
                        !main || self.hist.can_undo(),
                        egui::Button::new(undo_t).shortcut_text("Ctrl+Z"),
                    )
                    .on_hover_text(&info)
                    .clicked()
                {
                    self.do_undo(false);
                }
                if ui
                    .add_enabled(
                        !main || self.hist.can_redo(),
                        egui::Button::new(redo_t).shortcut_text("Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.do_undo(true);
                }
                ui.separator();
                let has_instr =
                    self.with(|s| (self.instr as usize).wrapping_sub(1) < s.song.instruments.len());
                if ui
                    .add_enabled(has_instr, egui::Button::new("Duplicate instrument"))
                    .clicked()
                {
                    self.duplicate_instrument(self.instr);
                }
                if ui
                    .add_enabled(has_instr, egui::Button::new("Delete instrument…"))
                    .on_hover_text("asks first when notes use it; Ctrl+Z undoes")
                    .clicked()
                {
                    self.request_delete_instrument(self.instr);
                }
                ui.separator();
                if self.view == View::Timeline && !self.pr_active() {
                    // the Timeline's clips (the piano roll keeps its notes'
                    // Cut / Copy / Paste while it has the keyboard)
                    let sel = !self.tlv.sels.is_empty();
                    let full = !self.tlv.clipboard.is_empty();
                    let clip_item = |ui: &mut egui::Ui, on: bool, label: &str, sc: &str| {
                        ui.add_enabled(on, egui::Button::new(label).shortcut_text(sc))
                            .clicked()
                    };
                    if clip_item(ui, sel, "Cut clips", "Ctrl+X") {
                        self.tl_copy(true);
                    }
                    if clip_item(ui, sel, "Copy clips", "Ctrl+C") {
                        self.tl_copy(false);
                    }
                    let paste = ui
                        .add_enabled(full, egui::Button::new("Paste clips").shortcut_text("Ctrl+V"))
                        .on_hover_text("at the play marker on the selected track, on the snap grid")
                        .on_disabled_hover_text("Nothing copied");
                    if paste.clicked() {
                        self.tl_paste(None, false);
                    }
                    let linked = ui
                        .add_enabled(full, egui::Button::new("Paste linked").shortcut_text("Ctrl+Shift+V"))
                        .on_hover_text("copies that play the same notes and blocks (edit one, all change)")
                        .on_disabled_hover_text("Nothing copied");
                    if linked.clicked() {
                        self.tl_paste(None, true);
                    }
                    if clip_item(ui, sel, "Duplicate clips", "Ctrl+D") {
                        self.tl_apply(super::timeline::TlOp::Duplicate);
                    }
                    if clip_item(ui, sel, "Delete clips", "Del") {
                        self.tl_apply(super::timeline::TlOp::Delete);
                    }
                    if item(ui, "Select all", "Ctrl+A") {
                        self.synth_key(&ctx, Key::A, cmd);
                    }
                } else {
                if item(ui, "Cut", "Ctrl+X") {
                    self.pending.push(egui::Event::Cut);
                }
                if item(ui, "Copy", "Ctrl+C") {
                    self.pending.push(egui::Event::Copy);
                }
                if item(ui, "Paste", "Ctrl+V") {
                    self.synth_key(&ctx, Key::V, cmd);
                }
                if item(ui, "Select all", "Ctrl+A") {
                    self.synth_key(&ctx, Key::A, cmd);
                }
                }
                ui.menu_button("Transpose", |ui| self.transpose_menu(ui));
                ui.separator();
                let mut e = self.edit;
                if ui
                    .add(egui::Checkbox::new(&mut e, "Edit mode"))
                    .on_hover_text("Esc in the Tracker and Order list")
                    .changed()
                {
                    self.edit = e;
                }
            });
            ui.menu_button("View", |ui| {
                let tracker = self.tracker_on();
                for (v, n, _) in self.views_shown() {
                    let sc = view_shortcut(v);
                    if ui
                        .add(egui::Button::selectable(self.view == v, n).shortcut_text(sc))
                        .clicked()
                    {
                        self.view = v;
                    }
                }
                if ui
                    .add(egui::Button::new("Reset to default layout").shortcut_text("Ctrl+6"))
                    .on_hover_text(
                        "Timeline on top, piano roll in the lower panel at its default height \
                         (Ctrl+6); the Inspector and Browser stay as they are",
                    )
                    .clicked()
                {
                    self.reset_layout();
                    ui.close();
                }
                ui.separator();
                if tracker
                    && ui
                    .add(
                        egui::Button::selectable(self.extra.adv_edit_open, "Advanced Edit")
                            .shortcut_text("Ctrl+Shift+A"),
                    )
                    .on_hover_text("the tracker's block operations panel (docked right of the Tracker)")
                    .clicked()
                {
                    self.toggle_adv_edit();
                    ui.close();
                }
                if ui
                    .add(
                        egui::Button::selectable(self.extra.inspector_open, "Inspector")
                            .shortcut_text("I"),
                    )
                    .on_hover_text(
                        "the panel on the left that shows the selected track or clip \
                         (I; in the Tracker, where I types a note: Alt+I)",
                    )
                    .clicked()
                {
                    self.toggle_inspector();
                    ui.close();
                }
                if ui
                    .checkbox(&mut self.extra.view_bar, "View tab bar")
                    .on_hover_text("the TIMELINE / TRACKER / ... tabs and transport along the bottom")
                    .changed()
                {
                    self.save_settings();
                }
                if tracker {
                    ui.checkbox(&mut self.show_auto, "Automation lanes");
                    ui.checkbox(&mut self.dock_arranger, "Dock lower panel under the Tracker");
                }
                let full = !self.lower_visible();
                if ui
                    .add(egui::Button::selectable(full, "Full-height view").shortcut_text("F11"))
                    .on_hover_text("hide the lower panel (piano roll, devices, instrument, automation)")
                    .clicked()
                {
                    self.toggle_full_height();
                    ui.close();
                }
                if ui
                    .add(
                        egui::Button::selectable(self.extra.clip_arranger, "Clip Arranger")
                            .shortcut_text("Ctrl+8"),
                    )
                    .on_hover_text(
                        "scenes and slots left of the Timeline (off: the Instruments list there)",
                    )
                    .clicked()
                {
                    self.toggle_clip_arranger();
                    ui.close();
                }
                if ui
                    .add(
                        egui::Button::selectable(self.devices_visible(), "Rack in lower panel")
                            .shortcut_text("Ctrl+7"),
                    )
                    .on_hover_text("the selected track's instrument and effect chain, left to right")
                    .clicked()
                {
                    self.vtab_select(super::viewbar::VTab::Devices);
                    ui.close();
                }
                let inst_on = self.lower_visible() && self.extra.lower_inst;
                if ui
                    .add(
                        egui::Button::selectable(inst_on, "Instrument editor in lower panel")
                            .shortcut_text("Ctrl+I"),
                    )
                    .clicked()
                {
                    self.toggle_instrument_panel();
                    ui.close();
                }
                if ui
                    .checkbox(
                        &mut self.extra.classic_editor,
                        "Classic sidebar instrument editor",
                    )
                    .on_hover_text("the pre-0.2.3.19 slider editor under the instrument list")
                    .changed()
                {
                    self.save_settings();
                }
                ui.checkbox(&mut self.follow, "Follow playback");
                if tracker
                    && ui
                    .checkbox(&mut self.extra.alt_columns, "Alternate column shading")
                    .on_hover_text("tint every other channel column in the Tracker")
                    .changed()
                {
                    self.save_settings();
                }
                if tracker {
                ui.horizontal(|ui| {
                    ui.label("Row numbers");
                    let a = ui.selectable_value(&mut self.extra.row_hex, false, "Decimal");
                    let b = ui.selectable_value(&mut self.extra.row_hex, true, "Hex");
                    if a.changed() || b.changed() {
                        self.save_settings();
                    }
                });
                }
                ui.separator();
                if item(ui, "Settings…", "Ctrl+,") {
                    self.toggle_settings();
                }
                ui.separator();
                ui.menu_button("Theme", |ui| {
                    for opt in theme::THEMES {
                        if ui
                            .add(egui::Button::selectable(self.theme == opt, opt.name))
                            .clicked()
                        {
                            self.theme = opt;
                            self.apply_theme(ui.ctx());
                            self.save_settings();
                        }
                    }
                });
                ui.menu_button(format!("Scale ({:.0} %)", self.zoom * 100.0), |ui| {
                    if item(ui, "Larger", "Ctrl+=") {
                        self.set_zoom(self.zoom + 0.25);
                    }
                    if item(ui, "Smaller", "Ctrl+-") {
                        self.set_zoom(self.zoom - 0.25);
                    }
                    if item(ui, "100 %", "Ctrl+0") {
                        self.set_zoom(1.0);
                    }
                    ui.separator();
                    for z in [0.75, 1.0, 1.25, 1.5, 1.75, 2.0] {
                        let ok = z <= self.max_zoom + 0.001;
                        if ui
                            .add_enabled(
                                ok,
                                egui::Button::selectable(
                                    (self.zoom - z).abs() < 0.01,
                                    format!("{:.0} %", z * 100.0),
                                ),
                            )
                            .clicked()
                        {
                            self.set_zoom(z);
                        }
                    }
                });
                ui.menu_button("Audio buffer", |ui| {
                    for opt in audio::BUFFER_CHOICES {
                        let n = if opt == 0 {
                            "Auto".to_string()
                        } else {
                            format!("{opt} frames")
                        };
                        if ui
                            .add(egui::Button::selectable(self.buffer == opt, n))
                            .clicked()
                            && self.buffer != opt
                        {
                            self.buffer = opt;
                            self.save_settings();
                            self.restart_audio();
                        }
                    }
                });
            });
            ui.menu_button("Song", |ui| {
                let (mut bpm, mut speed, mut ch) =
                    self.with(|s| (s.song.bpm, s.song.speed, s.song.channels));
                ui.horizontal(|ui| {
                    ui.label("BPM");
                    let c1 = ui
                        .add(
                            egui::DragValue::new(&mut bpm)
                                .range(30.0..=300.0)
                                .speed(0.5),
                        )
                        .changed();
                    ui.label("Speed");
                    let c2 = ui
                        .add(egui::DragValue::new(&mut speed).range(1..=32))
                        .changed();
                    if c1 || c2 {
                        self.set_tempo(bpm, speed);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Channels");
                    if ui
                        .add(egui::DragValue::new(&mut ch).range(1..=MAX_CHANNELS))
                        .on_hover_text(super::CHANNELS_TIP)
                        .changed()
                    {
                        self.set_channels(ch);
                    }
                });
                ui.separator();
                if ui
                    .button("Song info…")
                    .on_hover_text("title, artist, album, year, comment (used for MP3 tags)")
                    .clicked()
                {
                    self.show_info = true;
                }
                if item(ui, "Fill to length…", "Ctrl+Shift+F") {
                    self.open_fill();
                    ui.close();
                }
                if item(ui, "Chord helper…", "") {
                    self.chords.open = true;
                    ui.close();
                }
                let tl_mode = self.with(|s| s.song.timeline.is_some());
                let tracker = self.tracker_on();
                if tl_mode
                    && tracker
                    && ui
                        .button("Back to tracker mode…")
                        .on_hover_text("turn the timeline back into blocks and an order list (asks first)")
                        .clicked()
                {
                    self.view = View::Timeline;
                    self.tlv.ask_leave = true;
                    ui.close();
                }
                if tracker {
                    ui.separator();
                    if ui.button("Add block").clicked() {
                        self.add_block();
                    }
                    ui.menu_button("Block", |ui| self.block_len_menu(ui));
                }
            });
            let help = ui.menu_button("Help", |ui| {
                if ui
                    .button("User Manual")
                    .on_hover_text("the ChipWave Studio user manual (PDF, opens in your PDF viewer)")
                    .clicked()
                {
                    self.open_manual();
                    ui.close();
                }
                if item(ui, "Keys", "F12") {
                    self.show_help = !self.show_help;
                }
                if ui
                    .button("Load demo song")
                    .on_hover_text("Lumen Drift; all demos are in File > Demo songs")
                    .clicked()
                {
                    self.request(crate::prompt::Pending::Demo(0), &ctx);
                }
                if ui
                    .button("Check for updates…")
                    .on_hover_text("ask GitHub whether a newer ChipWave is out")
                    .clicked()
                {
                    self.check_updates(true);
                    ui.close();
                }
                if ui.button("About ChipWave Studio").clicked() {
                    self.show_about = true;
                }
            });
            // the help-menu screenshot: the Help menu open
            if self
                .shot
                .as_ref()
                .is_some_and(|s| s.view == "help-menu" && s.frame > 10)
            {
                let id = egui::Popup::default_response_id(&help.response);
                if !egui::Popup::is_id_open(ui.ctx(), id) {
                    egui::Popup::open_id(ui.ctx(), id);
                }
            }
            // song title + unsaved marker on the right...
            // and, at the far right, the status cluster (Panic, Keys →
            // Notes, Chord, MIDI; it was the toolbar's right end)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.status_cluster(ui);
                ui.separator();
                self.update_badge(ui);
                let title = self.with(|s| s.song.title.clone());
                let file = self
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "not saved yet".into());
                ui.label(egui::RichText::new(file).small().color(hdr_dim()));
                ui.label(
                    egui::RichText::new(format!("{}{}", title, if self.dirty { " •" } else { "" }))
                        .strong()
                        .color(hdr_text()),
                )
                .on_hover_text(if self.dirty {
                    "unsaved changes"
                } else {
                    "saved"
                });
            });
        });
    }

    fn recent_menu(&mut self, ui: &mut egui::Ui) {
        if self.recent.is_empty() {
            ui.label(egui::RichText::new("no recent songs").color(th().ink2));
            return;
        }
        let mut open = None;
        for p in &self.recent {
            let exists = std::path::Path::new(p).exists();
            let name = std::path::Path::new(p)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| p.clone());
            let txt = if exists {
                name
            } else {
                format!("{name}  (not found)")
            };
            if ui
                .add_enabled(exists, egui::Button::new(txt))
                .on_hover_text(p)
                .clicked()
            {
                open = Some(PathBuf::from(p));
            }
        }
        ui.separator();
        if ui.button("Remove missing").clicked() {
            self.recent.retain(|p| std::path::Path::new(p).exists());
            self.save_settings();
        }
        if ui.button("Clear list").clicked() {
            self.recent.clear();
            self.save_settings();
        }
        if let Some(p) = open {
            let ctx = ui.ctx().clone();
            self.request(crate::prompt::Pending::OpenPath(p), &ctx);
        }
    }

    pub(super) fn set_tempo(&mut self, bpm: f32, speed: u32) {
        // applies immediately, also while playing (no restart)
        self.with(|s| {
            s.song.bpm = bpm;
            s.song.speed = speed;
            s.engine.set_tempo(bpm, speed);
        });
        self.dirty = true;
    }

    /// the top bar: the Display, the Transport and the Master, the middle
    /// one centred on the window and the outer ones at its edges (in the
    /// saved order; drag a panel's grip to move it), then the view's extra
    /// controls in a free gap or on a second row (app/tbpanels.rs)
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        use super::tbpanels::{self as tp, Part};
        let lcd = self.lcd_on();
        let order = tp::order_of(&self.extra.topbar);
        let full = ui.max_rect();
        let (left, right) = (full.left(), full.right());
        // the window's middle, not the panel's (they are the same today)
        let mid = ui.ctx().content_rect().center().x;
        let shown = |a: &Self, p: Part| p != Part::Master || a.master_in_toolbar();
        let mp = ui.painter().clone();
        let widths = |a: &Self, narrow: bool| {
            order.map(|p| {
                if !shown(a, p) {
                    0.0
                } else if p == Part::Display && lcd {
                    a.lcd_display_w(&mp, narrow)
                } else if p == Part::Master {
                    super::lcd::master_w(narrow)
                } else {
                    // drawn last frame (a first guess before that)
                    let w = a.tb.w[p.idx()];
                    if w > 0.0 {
                        w
                    } else {
                        240.0
                    }
                }
            })
        };
        // the compact display and Master when the full ones do not fit
        let narrow = !tp::fits(left, right, mid, widths(self, false));
        self.lcd.narrow = narrow;
        let w = widths(self, narrow);
        let x = tp::slots(left, right, mid, w);
        let h = super::lcd::lcd_h().max(self.tb.h);
        let (row, _) = ui.allocate_exact_size(egui::vec2(full.width(), h), Sense::hover());
        let mut tall = 0.0f32;
        let mut rects = [egui::Rect::NOTHING; 3];
        for (i, p) in order.into_iter().enumerate() {
            if !shown(self, p) {
                self.tb.w[p.idx()] = 0.0;
                continue;
            }
            // room to its right up to the next panel (or the edge), so a
            // panel that grew this frame is not squeezed
            let r = egui::Rect::from_min_max(
                egui::pos2(x[i], row.top()),
                egui::pos2(x[i] + w[i].max(1.0) + 200.0, row.bottom()),
            );
            let inner = ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(r)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                    match p {
                        Part::Display => self.tb_display(ui),
                        Part::Transport => self.tb_transport(ui),
                        Part::Master => self.lcd_master(ui),
                    }
                },
            );
            let got = inner.response.rect;
            rects[i] = got;
            tall = tall.max(got.height());
            let old = self.tb.w[p.idx()];
            self.tb.w[p.idx()] = got.width();
            // a measured panel changed size: lay out again before showing
            if (old - got.width()).abs() > 0.5 && !(p == Part::Display && lcd) && p != Part::Master
            {
                ui.ctx().request_discard("top bar panel width");
            }
        }
        self.tb.h = tall;
        self.tb.rects = rects;
        self.tb_grips(ui, order, row, x, w);
        // the view's extra controls (Octave, Edit / Step, View): in a free
        // gap beside the panels, else on a second row
        let we = self.tb.extras_w;
        let spot = tp::extras_x(tp::gaps(left, right, mid, w), we);
        self.tb.second_row = spot.is_none();
        let r = match spot {
            Some(ex) => {
                egui::Rect::from_min_max(egui::pos2(ex, row.top()), egui::pos2(right, row.bottom()))
            }
            None => {
                let (r2, _) = ui.allocate_exact_size(
                    egui::vec2(
                        full.width(),
                        if we > 0.0 { self.tb.h.max(24.0) } else { 0.0 },
                    ),
                    Sense::hover(),
                );
                egui::Rect::from_min_max(
                    egui::pos2(left + tp::EDGE + tp::GRIP, r2.top()),
                    egui::pos2(right, r2.bottom()),
                )
            }
        };
        let inner = ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(r)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                self.tb_extras(ui)
            },
        );
        let got = if inner.inner {
            inner.response.rect.width()
        } else {
            0.0
        };
        self.tb.extras = if inner.inner {
            inner.response.rect
        } else {
            egui::Rect::NOTHING
        };
        if (got - self.tb.extras_w).abs() > 0.5 {
            self.tb.extras_w = got;
            ui.ctx().request_discard("top bar extras width");
        }
    }

    /// the panels' grips (six dots left of a panel, shown on hover): drag
    /// one to move its panel; a ghost follows the pointer and an orange
    /// marker shows where it lands. The order saves (UI settings are not
    /// part of Undo); Settings › Look › Top bar resets it.
    fn tb_grips(
        &mut self,
        ui: &mut egui::Ui,
        order: [super::tbpanels::Part; 3],
        row: egui::Rect,
        x: [f32; 3],
        w: [f32; 3],
    ) {
        use super::tbpanels as tp;
        let t = th();
        let pointer = ui.ctx().pointer_latest_pos();
        let centres = [0, 1, 2].map(|i| x[i] + w[i] / 2.0);
        for i in 0..3 {
            if w[i] <= 0.0 {
                continue;
            }
            let part = order[i];
            let pr = self.tb.rects[i];
            let grip = egui::Rect::from_min_max(
                egui::pos2(pr.left() - tp::GRIP + 1.0, row.top()),
                egui::pos2(pr.left() - 1.0, row.bottom()),
            );
            let resp = ui.interact(grip, ui.scope_id().with(("tb_grip", i)), Sense::drag());
            let name = format!("Move the {} panel", part.name());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
            if resp.drag_started() {
                self.tb.drag = Some(i);
            }
            let dragging = self.tb.drag == Some(i);
            let near = pointer.is_some_and(|q| pr.union(grip).expand(2.0).contains(q));
            let lit = dragging || resp.hovered();
            if (near && self.tb.drag.is_none()) || dragging {
                // six dots: two columns of three
                let col = if lit { t.accent } else { hdr_dim() };
                let c = grip.center();
                for dx in [-2.5f32, 2.5] {
                    for dy in [-5.0f32, 0.0, 5.0] {
                        ui.painter().circle_filled(c + egui::vec2(dx, dy), 1.6, col);
                    }
                }
            }
            let resp = resp
                .on_hover_cursor(egui::CursorIcon::Grab)
                .on_hover_text(format!(
                    "drag to move the {} panel (Settings › Look › Top bar resets the order)",
                    part.name()
                ));
            if dragging {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                if let Some(q) = pointer {
                    let gap = tp::drop_gap(centres, q.x);
                    let new = tp::reorder(order, i, gap);
                    // the panel stays put, dimmed, while its ghost moves
                    ui.painter()
                        .rect_filled(pr.expand(2.0), 6, t.hdr_bot.gamma_multiply(0.7));
                    // kept inside the window
                    let win = ui.ctx().content_rect();
                    let half = (pr.width() / 2.0).min(win.width() / 2.0);
                    let gx = q.x.clamp(win.left() + half, win.right() - half);
                    let ghost = egui::Rect::from_center_size(
                        egui::pos2(gx, pr.center().y + 4.0),
                        pr.size(),
                    );
                    let fp = ui.ctx().layer_painter(egui::LayerId::new(
                        egui::Order::Tooltip,
                        ui.scope_id().with("tb_ghost"),
                    ));
                    // see-through, so the panels under it still show
                    fp.rect(
                        ghost,
                        6,
                        t.accent.gamma_multiply(0.18),
                        Stroke::new(1.5, t.accent),
                        egui::StrokeKind::Inside,
                    );
                    // its name on a small tag
                    let font = crate::theme::prop(crate::theme::ts::BODY);
                    let gl = fp.layout_no_wrap(part.name().to_string(), font, hdr_text());
                    let tag = egui::Rect::from_center_size(
                        ghost.center(),
                        gl.size() + egui::vec2(16.0, 6.0),
                    );
                    fp.rect_filled(tag, 4, t.hdr_bot);
                    fp.galley(tag.center() - gl.size() / 2.0, gl, hdr_text());
                    if new != order {
                        // the drop marker, in the gap the panel goes into
                        let end = |k: usize| x[k] + w[k];
                        let mx = match gap {
                            0 => x[0] - tp::GRIP - 2.0,
                            3 => end(2) + 4.0,
                            g => (end(g - 1) + x[g] - tp::GRIP) / 2.0,
                        }
                        .clamp(row.left() + 2.0, row.right() - 2.0);
                        fp.rect_filled(
                            egui::Rect::from_center_size(
                                egui::pos2(mx, row.center().y),
                                egui::vec2(3.0, row.height() + 6.0),
                            ),
                            1.5,
                            DROP_ORANGE,
                        );
                    }
                    if resp.drag_stopped() && new != order {
                        self.set_topbar(new);
                    }
                }
            }
            if resp.drag_stopped() {
                self.tb.drag = None;
            }
        }
    }

    /// set (and save) the top bar's panel order
    pub(super) fn set_topbar(&mut self, o: [super::tbpanels::Part; 3]) {
        use super::tbpanels as tp;
        self.extra.topbar = tp::keys_of(o);
        self.save_settings();
        self.status = if o == tp::DEFAULT {
            format!("Top bar reset: {}", tp::names(o))
        } else {
            format!("Top bar: {}", tp::names(o))
        };
    }

    /// the Transport panel: play block (Tracker), to the start, play / stop,
    /// record, loop, metronome, the position (classic)
    fn tb_transport(&mut self, ui: &mut egui::Ui) {
        let playing = self.with(|s| s.engine.playing);
        let t = th();
        // Play block is a tracker control (its keys work everywhere)
        let tracker = self.view == View::Pattern;
        // the LCD toolbar (0.2.10): an icon-only transport
        let lcd = self.lcd_on();
        group(ui, "TRANSPORT", |ui| {
            let loop_on = playing && self.with(|s| s.engine.loop_pattern);
            // Play block is a tracker control: shown in the Tracker view
            // (Space still loops the block everywhere)
            if tracker
                && tool_button_opt(ui, Ic::Play, "Play block", loop_on, t.accent, 0.0, lcd)
                    .on_hover_text("loop the current block (Space)")
                    .clicked()
            {
                if playing {
                    self.with(|s| s.engine.stop());
                }
                self.play(3);
            }
            // the one transport (the view bar's copy is gone): to the
            // start, play, stop, and where the playhead is
            let tl_mode = self.with(|s| s.song.timeline.is_some());
            if super::viewbar::transport_button(
                ui,
                super::viewbar::Tr::Start,
                false,
                // the Loop button's off ink (the header's text is white
                // on Amiga's white buttons)
                th().ink2.gamma_multiply(0.75),
                "play marker to the start",
            ) {
                if tl_mode {
                    self.timeline_seek(0);
                } else {
                    self.seek_to((0, 0));
                }
            }
            // one Play / Stop toggle (Space does the same): a play icon
            // while stopped, a stop icon and accent-lit while playing
            let play_label = if self.tracker_on() {
                "Play song"
            } else {
                "Play"
            };
            let (ic, label, tip) = if playing {
                (
                    Ic::Stop,
                    "Stop",
                    "Stop (Space): stop playback. The go-to-start button moves the play marker back to bar 1.",
                )
            } else if self.tracker_on() {
                (
                    Ic::PlaySong,
                    play_label,
                    "Play song (Space; Shift+Space in the Tracker): play from the play marker (click the ruler to move it; the go-to-start button moves it back to bar 1). Click again or Space to stop.",
                )
            } else {
                (
                    Ic::Play,
                    play_label,
                    "Play (Space): play from the play marker (click the ruler to move it; the go-to-start button moves it back to bar 1). Click again or Space to stop.",
                )
            };
            // as wide as its widest label, so it does not jump
            let font = egui::TextStyle::Button.resolve(ui.style());
            let min_w = [play_label, "Stop"]
                .iter()
                .map(|l| {
                    ui.painter()
                        .layout_no_wrap(l.to_string(), font.clone(), Color32::PLACEHOLDER)
                        .size()
                        .x
                })
                .fold(0.0f32, f32::max)
                + 44.0;
            if tool_button_opt(ui, ic, label, playing, t.accent, min_w, lcd)
                .on_hover_text(tip)
                .clicked()
            {
                if playing {
                    self.with(|s| s.engine.stop());
                } else {
                    self.play(0);
                }
            }
            // Record: live into the Timeline, at the time you play
            if tl_mode {
                let rec = self.recording();
                let tip = if rec {
                    "Recording - click (or R) to stop recording; playback goes on".to_string()
                } else {
                    format!(
                        "Record (R): the armed (or selected) track records what you play on the MIDI keyboard or with Keys → Notes (Ctrl+K), in time{}",
                        match self.extra.count_in_bars() {
                            0 => "",
                            1 => ", after a bar's count-in",
                            _ => ", after a 2-bar count-in",
                        }
                    )
                };
                // counting in: Record blinks (twice a second) until the take starts
                let counting = self.with(|s| s.engine.count_in_left()).is_some();
                let blink_off = counting && {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(60));
                    (ui.input(|i| i.time) * 4.0) as u64 % 2 == 1
                };
                let tip = if counting {
                    "Counting in - recording starts after the count-in; click (or R) to cancel"
                        .to_string()
                } else {
                    tip
                };
                if super::viewbar::transport_button(
                    ui,
                    super::viewbar::Tr::Record,
                    rec && !blink_off,
                    if blink_off {
                        t.brick.gamma_multiply(0.35)
                    } else if rec {
                        t.brick
                    } else {
                        t.brick.gamma_multiply(0.7)
                    },
                    &tip,
                ) {
                    self.rec_toggle();
                }
                if rec && !lcd {
                    let rec_txt = if t.ember {
                        // Slate: a lit (glowing) red tally
                        egui::RichText::new("REC")
                            .font(theme::label_font(crate::theme::ts::BODY))
                            .color(crate::ember::RED)
                    } else {
                        egui::RichText::new("REC").small().strong().color(t.brick)
                    };
                    ui.label(rec_txt)
                        .on_hover_text("recording: R or the Record button stops it");
                }
                self.loop_toggle(ui);
                self.metronome_toggle(ui);
            }
            let left = self.with(|s| s.engine.count_in_left());
            if lcd {
                // the display shows the position and the count-in
            } else if let Some(n) = left {
                // the countdown, in the playhead's place: Count-in 4, 3, 2, 1
                ui.label(
                    egui::RichText::new(format!("Count-in {n}"))
                        .monospace()
                        .strong()
                        .color(t.brick),
                )
                .on_hover_text("beats left before recording starts");
            } else {
                let (pos, _) = self.view_bar_position(playing);
                ui.label(egui::RichText::new(pos).monospace().color(if playing {
                    t.play
                } else {
                    hdr_text()
                }))
                .on_hover_text("playhead (stopped: the play marker)");
            }
        });
    }

    /// the Display panel: the LCD (tempo, position, lamps, load), or the
    /// classic toolbar's BPM and Speed
    fn tb_display(&mut self, ui: &mut egui::Ui) {
        if self.lcd_on() {
            self.lcd_display(ui);
            return;
        }
        let mut tempo_r = None;
        group(ui, "TEMPO", |ui| {
            let (mut bpm, mut speed) = self.with(|s| (s.song.bpm, s.song.speed));
            ui.label(egui::RichText::new("BPM").color(hdr_text()));
            let r = ui
                .add(
                    egui::DragValue::new(&mut bpm)
                        .range(30.0..=300.0)
                        .speed(0.5),
                )
                .on_hover_text("tempo in beats per minute (drag or type)");
            let c1 = r.changed();
            tempo_r = Some(r);
            ui.label(egui::RichText::new("Speed").color(hdr_text()));
            let c2 = ui
                .add(egui::DragValue::new(&mut speed).range(1..=32))
                .on_hover_text("ticks per row (effect 09xx sets it in the Tracker)")
                .changed();
            if c1 || c2 {
                self.set_tempo(bpm, speed);
            }
        });
        if let Some(r) = tempo_r {
            self.learn_menu(&r, crate::automation::Target::Tempo);
        }
    }

    /// the view's extra controls: Octave and Edit / Step (the Tracker), the
    /// view switch (while the view tab bar is hidden); false: none shown
    fn tb_extras(&mut self, ui: &mut egui::Ui) -> bool {
        let t = th();
        let tracker = self.view == View::Pattern;
        group_if(tracker, ui, "OCTAVE", |ui| {
            if ui
                .button(" − ")
                .on_hover_text("octave down (numpad -)")
                .clicked()
            {
                self.set_octave(self.octave as i32 - 1);
            }
            ui.label(
                egui::RichText::new(format!("{}", self.octave))
                    .monospace()
                    .strong()
                    .size(crate::theme::ts::px(crate::theme::ts::HEADING))
                    .color(if t.light { Color32::WHITE } else { t.value }),
            )
            .on_hover_text("base octave for note entry (F1-F8)");
            if ui
                .button(" + ")
                .on_hover_text("octave up (numpad +)")
                .clicked()
            {
                self.set_octave(self.octave as i32 + 1);
            }
        });
        group_if(tracker, ui, "EDIT", |ui| {
            let label = if self.edit {
                "Edit mode ON"
            } else {
                "Edit mode off"
            };
            if tool_button(
                ui,
                Ic::Edit,
                label,
                self.edit,
                if self.edit { t.brick } else { t.ink2 },
            )
            .on_hover_text("typing enters notes when on (Esc toggles)")
            .clicked()
            {
                self.edit = !self.edit;
            }
            ui.label(egui::RichText::new("Step").color(hdr_text()));
            ui.add(egui::DragValue::new(&mut self.step).range(0..=16))
                .on_hover_text("rows the cursor moves after entering a note");
        });
        group_if(!self.extra.view_bar, ui, "VIEW", |ui| {
            let views = self.views_shown();
            let cur = views
                .iter()
                .position(|v| v.0 == self.view)
                .unwrap_or(usize::MAX);
            let items: Vec<(&str, &str)> = views.iter().map(|v| (v.1, v.2)).collect();
            if let Some(i) = segmented(ui, &items, cur) {
                self.view = views[i].0;
                self.arr_focus = false;
            }
            // the Browser panel on the right (docs/BROWSER.md)
            let on = if self.extra.browser_open {
                0
            } else {
                usize::MAX
            };
            if segmented(
                ui,
                &[("Browser", "show / hide the Browser on the right (Ctrl+B)")],
                on,
            )
            .is_some()
            {
                self.toggle_browser();
            }
        });
        tracker || !self.extra.view_bar
    }

    /// the status cluster at the menu bar's right (laid out right to left):
    /// Panic, the latency (classic), Keys → Notes, the Tracker's record
    /// mode, Chord and the MIDI light
    fn status_cluster(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let lcd = self.lcd_on();
        let (lit, midi_tip) = self.midi_light(ui.input(|i| i.time));
        if lit {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(160));
        }
        ui.add_space(10.0);
        // PANIC (0.2.3.16): far right, small, never a focus target
        if ui
            .add(
                egui::Button::new(egui::RichText::new("Panic").small().color(
                    // the button face, not the header, is behind it
                    crate::theme::on2(ui.visuals().widgets.inactive.weak_bg_fill),
                ))
                .small()
                .min_size(egui::vec2(0.0, 16.0)),
            )
            .on_hover_text("Panic: stop all sound (Ctrl+.)")
            .clicked()
        {
            self.panic();
        }
        ui.add_space(6.0);
        // the LCD shows the buffer and underruns itself
        if let Some(a) = self.audio.as_ref().filter(|_| !lcd) {
            let u = a.stats.underruns.load(std::sync::atomic::Ordering::Relaxed);
            let txt = format!(
                "{:.0} ms{} · {} underruns",
                a.latency_ms(),
                if a.latency_maxed() { " (max)" } else { "" },
                u
            );
            let col = match (u > 0, t.light) {
                (true, true) => Color32::from_rgb(255, 200, 160),
                (true, false) => t.brick,
                _ => hdr_dim(),
            };
            ui.label(egui::RichText::new(txt).small().monospace().color(col))
                .on_hover_text(audio_tip(a));
        }
        // Keys → Notes (musical typing): one icon button, a drawn
        // piano keyboard, lit while on; the hover says what it does
        // with its octave, velocity and keys
        let on = self.typing.on;
        let h = ui.spacing().interact_size.y.max(18.0);
        let (ir, resp) = ui.allocate_exact_size(egui::vec2(h * 1.6, h), Sense::click());
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), on, "Keys → Notes")
        });
        self.kb_rect = ir;
        {
            let sel = ui.visuals().selection;
            let v = ui.style().interact_selectable(&resp, on);
            let p = ui.painter();
            let col = if on {
                sel.stroke.color
            } else if resp.hovered() {
                v.fg_stroke.color
            } else {
                hdr_dim()
            };
            if on {
                p.rect(
                    ir,
                    4,
                    sel.bg_fill,
                    egui::Stroke::new(1.0, sel.stroke.color.gamma_multiply(0.6)),
                    egui::StrokeKind::Inside,
                );
            } else if resp.hovered() {
                p.rect_filled(ir, 4, v.weak_bg_fill);
            }
            // the Browser's MIDI keyboard icon: five white keys and
            // three black ones, scaled to the button
            let kh = (ir.height() * 0.62).round();
            let kw = (kh * 1.75).round();
            let r = egui::Rect::from_center_size(ir.center(), egui::vec2(kw, kh));
            let r = egui::Rect::from_min_size(r.min.round(), r.size());
            let st = egui::Stroke::new(1.0, col);
            p.rect_stroke(r, 1, st, egui::StrokeKind::Inside);
            let w = r.width() / 5.0;
            for k in 1..5 {
                let x = (r.left() + w * k as f32).round() + 0.5;
                p.line_segment([egui::pos2(x, r.top()), egui::pos2(x, r.bottom())], st);
            }
            let bw = (w * 0.6).max(2.0);
            for k in [1, 2, 4] {
                let x = (r.left() + w * k as f32).round() + 0.5;
                p.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(x - bw / 2.0, r.top()),
                        egui::vec2(bw, (r.height() * 0.58).round()),
                    ),
                    0,
                    col,
                );
            }
        }
        let tip = keys_tip(on, &self.typing_label());
        let resp = resp.on_hover_ui(|ui| {
            ui.label(
                egui::RichText::new(tip.0)
                    .strong()
                    .color(ui.visuals().selection.stroke.color)
                    .background_color(ui.visuals().selection.bg_fill),
            );
            ui.label(egui::RichText::new(tip.1).small());
        });
        if resp.clicked() {
            self.typing_toggle();
        }
        let tracker_rec = self.tracker_on();
        // MIDI quick toggles (laid out right to left: shown as light, Chord, Rec)
        let m = &mut self.extra.midi;
        let rec_txt = if m.live_record {
            "Rec live"
        } else {
            "Rec step"
        };
        let armed = self.edit;
        let rec_col = if armed { t.brick } else { hdr_dim() };
        let mut toggled = false;
        // the Tracker's MIDI record mode (the Timeline has Record)
        if tracker_rec
            && ui
            .selectable_label(
                false,
                egui::RichText::new(rec_txt).small().color(rec_col),
            )
            .on_hover_text(format!(
                "MIDI record mode: {}\nclick to switch live / step\n(records only when edit mode is on: {})",
                if m.live_record {
                    "LIVE - while the song plays, notes go to the playing row"
                } else {
                    "STEP - notes always go to the cursor, which moves by Step"
                },
                if armed { "on" } else { "off" }
            ))
            .clicked()
        {
            m.live_record = !m.live_record;
            toggled = true;
        }
        if ui
            .selectable_label(
                m.chord_spread,
                egui::RichText::new("Chord").small().color(if m.chord_spread {
                    crate::theme::on(ui.visuals().selection.bg_fill)
                } else {
                    hdr_dim()
                }),
            )
            .on_hover_text(format!(
                "MIDI chord spread: {}\nwhen on, the notes of a chord go into the cursor channel and the channels to its right",
                if m.chord_spread { "on" } else { "off" }
            ))
            .clicked()
        {
            m.chord_spread = !m.chord_spread;
            toggled = true;
        }
        if toggled {
            self.save_settings();
            self.midi_apply(false);
        }
        // MIDI activity light
        let connected = !self.midi.hub.connected().is_empty();
        let (r, resp) = ui.allocate_exact_size(egui::vec2(46.0, 18.0), Sense::click());
        let dot = r.left_center() + egui::vec2(7.0, 0.0);
        let col = if lit {
            hex(0x6FE08A)
        } else if connected {
            hex(0x2F6B3E)
        } else {
            hdr_dim().gamma_multiply(0.5)
        };
        ui.painter().circle_filled(dot, 5.0, col);
        ui.painter().text(
            dot + egui::vec2(9.0, 0.0),
            egui::Align2::LEFT_CENTER,
            "MIDI",
            crate::theme::prop(crate::theme::ts::SMALL),
            hdr_dim(),
        );
        let meter = self.midi_press_meter();
        // the hover card is custom UI, so name the light for screen
        // readers as its plain tip did
        let name = format!("{midi_tip}\nclick: MIDI settings");
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
        if resp
            .on_hover_ui(|ui| {
                ui.label(&midi_tip);
                // the live pressure meter (aftertouch keyboards)
                if let Some((v, txt)) = &meter {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(50));
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(90.0, 10.0), Sense::hover());
                        let vis = ui.visuals();
                        ui.painter().rect_filled(r, 2.0, vis.extreme_bg_color);
                        let mut f = r;
                        f.set_width(r.width() * v.clamp(0.0, 1.0));
                        ui.painter().rect_filled(f, 2.0, vis.selection.bg_fill);
                        ui.painter().rect_stroke(
                            r,
                            2.0,
                            vis.widgets.noninteractive.bg_stroke,
                            egui::StrokeKind::Inside,
                        );
                        ui.label(txt);
                    });
                }
                ui.label("click: MIDI settings");
            })
            .clicked()
        {
            self.view = View::Settings;
            self.set_card = 4;
            self.set_open = Some(4);
        }
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.about_window(ctx);
        self.manual_window(ctx);
        if self.orphans.is_empty() {
            return;
        }
        let mut act: Option<(usize, bool)> = None; // (index, recover?)
        let mut later = false;
        // Discard deletes the copy for good: it asks once on that row first
        let confirm_id = egui::Id::unique("orph_discard_confirm");
        let mut confirm: Option<usize> = ctx.data(|d| d.get_temp(confirm_id));
        egui::Window::new("Recover unsaved work?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("ChipWave did not close cleanly last time. These copies were found:");
                ui.add_space(6.0);
                // fixed list area: discarding a row must not resize the window
                ui.set_width(620.0);
                egui::ScrollArea::vertical()
                    .id_salt("orph_scroll")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        ui.set_min_height(180.0);
                        ui.set_min_width(600.0);
                        egui::Grid::new("orph")
                            .striped(true)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                for (i, o) in self.orphans.iter().enumerate() {
                                    let ago =
                                        o.modified.elapsed().map(|d| d.as_secs()).unwrap_or(0);
                                    let when = if ago < 3600 {
                                        format!("{} min ago", ago / 60)
                                    } else if ago < 86400 {
                                        format!("{} h ago", ago / 3600)
                                    } else {
                                        format!("{} days ago", ago / 86400)
                                    };
                                    let title = if o.info.title.is_empty() {
                                        "Untitled"
                                    } else {
                                        &o.info.title
                                    };
                                    ui.label(egui::RichText::new(title).strong());
                                    ui.label(when);
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}{}",
                                            o.source,
                                            if o.crashed { " · crashed" } else { "" }
                                        ))
                                        .color(th().ink2),
                                    );
                                    if ui
                                .button("Recover")
                                .on_hover_text(
                                    "open as Untitled (recovered); the original file is untouched",
                                )
                                .clicked()
                            {
                                act = Some((i, true));
                            }
                                    if confirm == Some(i) {
                                        ui.horizontal(|ui| {
                                            if ui
                                                .button("Delete it")
                                                .on_hover_text("delete this recovery copy for good")
                                                .clicked()
                                            {
                                                act = Some((i, false));
                                            }
                                            if ui.button("Keep").clicked() {
                                                confirm = None;
                                            }
                                        });
                                    } else if ui
                                        .button("Discard")
                                        .on_hover_text("delete this recovery copy (asks first)")
                                        .clicked()
                                    {
                                        confirm = Some(i);
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                ui.add_space(6.0);
                if ui
                    .button("Later")
                    .on_hover_text(
                        "decide later: until ChipWave closes normally (then the copies \
                         move to the recovery-archive folder and are not offered again)",
                    )
                    .clicked()
                {
                    later = true;
                }
            });
        if act.is_some() {
            confirm = None;
        }
        ctx.data_mut(|d| match confirm {
            Some(i) => {
                d.insert_temp(confirm_id, i);
            }
            None => d.remove::<usize>(confirm_id),
        });
        if let Some((i, rec)) = act {
            let o = self.orphans.remove(i);
            if rec {
                match fileio::load_cwt_warn(&o.file) {
                    Ok((mut song, warnings)) => {
                        // like File > Open: what could not be loaded is listed
                        self.load_warnings = warnings;
                        song.title = format!(
                            "{} (recovered)",
                            if song.title.is_empty() {
                                "Untitled"
                            } else {
                                &song.title
                            }
                        );
                        self.with(|s| {
                            s.engine.stop();
                            s.song = song;
                        });
                        self.path = None;
                        let conv = self.convert_on_open();
                        self.reset_cursor();
                        // not on disk anywhere: undo never makes it "saved"
                        self.hist.forget_saved();
                        self.dirty = true;
                        self.status = "Recovered - choose File > Save As to keep it".into();
                        if conv {
                            self.status =
                                format!("{} · {}", self.status, super::file::OPENED_IN_TIMELINE);
                        }
                        crate::recovery::discard(&o);
                    }
                    Err(e) => {
                        self.status =
                            format!("Recovery failed: {e} (copy kept in {})", o.dir.display())
                    }
                }
            } else {
                crate::recovery::discard(&o);
            }
        }
        if later {
            let dirs = self.orphans.drain(..).map(|o| o.dir);
            self.orphans_later.extend(dirs);
        }
    }
}

impl App {
    /// the transport's Loop toggle (Del, 25 Oct 2026: "we need a playloop
    /// symbol"): the Timeline's loop, the same switch as L, the LCD's LOOP
    /// pill and the Timeline toolbar's Loop; accent-lit while on
    fn loop_toggle(&mut self, ui: &mut egui::Ui) {
        let (on, range) = self.with(|s| {
            s.song.timeline.as_deref().map_or((false, None), |t| {
                (
                    t.loop_on && t.loop_range.is_some(),
                    t.loop_range
                        .map(|(a, b)| (t.bar_pos(a).bar, t.bar_pos((b - 1).max(a)).bar)),
                )
            })
        });
        let t = th();
        let tip = match range {
            Some((a, b)) if a == b => format!(
                "Loop (L): {} · bar {a}",
                if on { "on" } else { "off" }
            ),
            Some((a, b)) => format!(
                "Loop (L): {} · bars {a}–{b}",
                if on { "on" } else { "off" }
            ),
            None => "Loop (L): no loop region yet - Shift-drag the ruler, or select clips and press Ctrl+L"
                .to_string(),
        };
        let col = if on {
            t.accent
        } else {
            // dimmed as the metronome's off symbol (the header's text is
            // white on Amiga's white buttons)
            t.ink2.gamma_multiply(0.75)
        };
        if super::viewbar::transport_button(ui, super::viewbar::Tr::Loop, on, col, &tip) {
            self.tl_apply(super::timeline::TlOp::ToggleLoop);
        }
    }

    /// the toolbar's Metronome toggle: click on every beat while playing;
    /// right-click (or the arrow beside it) for the record count-in
    fn metronome_toggle(&mut self, ui: &mut egui::Ui) {
        let m = self.extra.metronome;
        // the animated symbol: swings with the transport's beat (the
        // count-in's own beats while counting in), flashes on the downbeat
        let (beat, per_bar, count) = self.with(|s| {
            let e = &s.engine;
            if let Some(n) = e.count_in_left() {
                let (b, per) = e.count_in_beat().unwrap_or((0.0, 4));
                return (Some(b), per, Some(n));
            }
            let pos = e.tl_ticks().zip(s.song.timeline.as_deref()).map(|(tk, t)| {
                let sig = t.sig_at(tk as i64);
                let bp = t.bar_pos(tk as i64);
                let bt = sig.beat_ticks().max(1) as f64;
                let frac = (bp.tick as f64 + tk.fract()) / bt;
                ((bp.beat - 1) as f64 + frac) as f32
            });
            let per = s.song.timeline.as_deref().map_or(4, |t| {
                t.sig_at(e.tl_ticks().unwrap_or(0.0) as i64).num.max(1) as u32
            });
            (pos, per, None)
        });
        let look = super::metroicon::MetroLook {
            on: m,
            beat,
            per_bar,
            count,
        };
        if (m && beat.is_some()) || count.is_some() {
            ui.ctx().request_repaint();
        }
        let shown = if self.lcd_on() { "" } else { "Metronome" };
        let r = tool_button_named(ui, shown, "Metronome", m, 0.0, |p, ir, _| {
            super::metroicon::paint_metronome(p, ir, look)
        })
        .on_hover_text(format!(
            "Metronome: click on every beat while playing ({})\nRight-click (or the arrow next to it): count-in before recording",
            if m { "on" } else { "off" }
        ));
        if r.clicked() {
            self.extra.metronome = !m;
            self.save_settings();
        }
        r.context_menu(|ui| self.count_in_menu(ui));
        ui.add_space(-ui.spacing().item_spacing.x + 2.0);
        // the options caret (painted: the small fonts lack a down-arrow glyph)
        let (cr, mr) = ui.allocate_exact_size(egui::vec2(16.0, 26.0), Sense::click());
        mr.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Metronome options")
        });
        if ui.is_rect_visible(cr) {
            let v = ui.style().interact(&mr);
            let p = ui.painter();
            p.rect(cr, 4, v.weak_bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
            let c = cr.center();
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + egui::vec2(-4.0, -2.0),
                    c + egui::vec2(4.0, -2.0),
                    c + egui::vec2(0.0, 3.0),
                ],
                v.text_color(),
                Stroke::NONE,
            ));
        }
        let mr = mr.on_hover_text("Metronome options: count-in before recording");
        egui::Popup::menu(&mr).show(|ui| self.count_in_menu(ui));
        // the record-metro-menu screenshot: the menu open
        if self
            .shot
            .as_ref()
            .is_some_and(|s| s.view == "record-metro-menu" && s.frame > 10)
        {
            let id = egui::Popup::default_response_id(&mr);
            if !egui::Popup::is_id_open(ui.ctx(), id) {
                egui::Popup::open_id(ui.ctx(), id);
            }
        }
    }

    /// Count-in before recording: Off / 1 bar / 2 bars
    pub(super) fn count_in_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Count-in before recording").strong());
        let cur = self.extra.count_in_bars();
        for (n, name) in [(0u8, "Off"), (1, "1 bar"), (2, "2 bars")] {
            if ui.radio(cur == n, name).clicked() {
                self.extra.set_count_in_bars(n);
                self.save_settings();
                ui.close();
            }
        }
        ui.separator();
        ui.label(
            egui::RichText::new("Clicks during the count-in even with the metronome off")
                .small()
                .weak(),
        );
    }
}

/// the Keys → Notes button's hover: (highlighted line, the keys)
pub(super) fn keys_tip(on: bool, label: &str) -> (String, String) {
    (
        format!(
            "Computer keys play notes · {label} · {} — click or Ctrl+K",
            if on { "on" } else { "off" }
        ),
        "A–; white keys, W E T Y U O P black · Z / X octave, C / V velocity (while on)".into(),
    )
}
