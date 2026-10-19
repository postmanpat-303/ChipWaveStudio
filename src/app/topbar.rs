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
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let size = egui::vec2(10.0 + 18.0 + 6.0 + galley.size().x + 10.0, 26.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(r) {
        let v = ui.style().interact_selectable(&resp, on);
        let p = ui.painter();
        p.rect(r, 5, v.weak_bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
        theme::raise(p, r, 5.0, resp.is_pointer_button_down_on() || on);
        let ir = Rect::from_min_size(r.min + egui::vec2(10.0, 4.0), egui::vec2(18.0, 18.0));
        // light themes: active buttons are filled, so draw the icon in the text colour
        let ic_col = if on && th().light {
            v.text_color()
        } else {
            icon_col
        };
        paint_icon(p, ir, ic, ic_col);
        p.galley(
            Pos2::new(ir.right() + 6.0, r.center().y - galley.size().y / 2.0),
            galley,
            v.text_color(),
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

fn group(ui: &mut egui::Ui, caption: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::from_black_alpha(if th().light { 40 } else { 70 }))
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(18)))
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                ui.label(egui::RichText::new(caption).small().color(hdr_dim()));
                add(ui);
            });
        });
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
        Color32::from_white_alpha(190)
    } else {
        th().ink2
    }
}

fn item(ui: &mut egui::Ui, text: &str, sc: &str) -> bool {
    ui.add(egui::Button::new(text).shortcut_text(sc)).clicked()
}

pub(super) const VIEWS: [(View, &str, &str); 6] = [
    (View::Pattern, "Pattern", "pattern editor (Ctrl+1)"),
    (View::Arranger, "Arranger", "song arrangement (Ctrl+2)"),
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
        if self.view == View::Sample {
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
            ui.label(
                egui::RichText::new("ChipWave")
                    .strong()
                    .size(16.0)
                    .color(hdr_text()),
            );
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
                if item(ui, "Quit", "Ctrl+Q") {
                    self.request(crate::prompt::Pending::Quit, &ctx);
                }
            });
            ui.menu_button("Edit", |ui| {
                // the sample editor has its own history
                let main = self.view != View::Sample;
                let info = format!(
                    "{} undo step(s) kept, {:.1} MB",
                    self.hist.len(),
                    self.hist.bytes() as f64 / 1e6
                );
                if ui
                    .add_enabled(
                        !main || self.hist.can_undo(),
                        egui::Button::new("Undo").shortcut_text("Ctrl+Z"),
                    )
                    .on_hover_text(&info)
                    .clicked()
                {
                    self.do_undo(false);
                }
                if ui
                    .add_enabled(
                        !main || self.hist.can_redo(),
                        egui::Button::new("Redo").shortcut_text("Ctrl+Y"),
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
                ui.menu_button("Transpose", |ui| self.transpose_menu(ui));
                ui.separator();
                let mut e = self.edit;
                if ui
                    .add(egui::Checkbox::new(&mut e, "Edit mode"))
                    .on_hover_text("Esc")
                    .changed()
                {
                    self.edit = e;
                }
            });
            ui.menu_button("View", |ui| {
                for (v, n, _) in VIEWS.iter() {
                    let sc = view_shortcut(*v);
                    if ui
                        .add(egui::Button::selectable(self.view == *v, *n).shortcut_text(sc))
                        .clicked()
                    {
                        self.view = *v;
                    }
                }
                ui.separator();
                if ui
                    .add(
                        egui::Button::selectable(self.extra.inspector_open, "Inspector")
                            .shortcut_text("I"),
                    )
                    .on_hover_text(
                        "the panel on the left that shows the selected track or clip \
                         (I; in the pattern editor, where I types a note: Alt+I)",
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
                ui.checkbox(&mut self.show_auto, "Automation lanes");
                ui.checkbox(&mut self.dock_arranger, "Dock lower panel under pattern");
                let full = !self.lower_visible();
                if ui
                    .add(egui::Button::selectable(full, "Full-height pattern").shortcut_text("F11"))
                    .on_hover_text("hide the lower panel (arranger, automation, instrument editor)")
                    .clicked()
                {
                    self.toggle_full_height();
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
                if ui
                    .checkbox(&mut self.extra.alt_columns, "Alternate column shading")
                    .on_hover_text("tint every other channel column in the pattern editor")
                    .changed()
                {
                    self.save_settings();
                }
                ui.horizontal(|ui| {
                    ui.label("Row numbers");
                    let a = ui.selectable_value(&mut self.extra.row_hex, false, "Decimal");
                    let b = ui.selectable_value(&mut self.extra.row_hex, true, "Hex");
                    if a.changed() || b.changed() {
                        self.save_settings();
                    }
                });
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
                ui.separator();
                if ui.button("Add block").clicked() {
                    self.add_block();
                }
                ui.menu_button("Block", |ui| self.block_len_menu(ui));
            });
            ui.menu_button("Help", |ui| {
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
            // song title + unsaved marker on the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
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

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
            ui.add_space(6.0);
            let playing = self.with(|s| s.engine.playing);
            let t = th();
            group(ui, "TRANSPORT", |ui| {
                let loop_on = playing && self.with(|s| s.engine.loop_pattern);
                if tool_button(ui, Ic::Play, "Play block", loop_on, t.accent)
                    .on_hover_text("loop the current block (Space)")
                    .clicked()
                {
                    if playing {
                        self.with(|s| s.engine.stop());
                    }
                    self.play(3);
                }
                if tool_button(ui, Ic::PlaySong, "Play song", playing && !loop_on, t.accent)
                    .on_hover_text("play the song from the arranger play marker (Shift+Space; click the arranger ruler to move it)")
                    .clicked()
                {
                    if playing {
                        self.with(|s| s.engine.stop());
                    }
                    self.play(0);
                }
                if tool_button(
                    ui,
                    Ic::Stop,
                    "Stop",
                    false,
                    if playing { t.brick } else { t.ink2 },
                )
                .on_hover_text("stop playback (Space while playing)")
                .clicked()
                {
                    self.with(|s| s.engine.stop());
                }
            });
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
                    .on_hover_text("ticks per row (effect 09xx sets it in a pattern)")
                    .changed();
                if c1 || c2 {
                    self.set_tempo(bpm, speed);
                }
            });
            if let Some(r) = tempo_r {
                self.learn_menu(&r, crate::automation::Target::Tempo);
            }
            group(ui, "OCTAVE", |ui| {
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
                        .size(16.0)
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
            group(ui, "EDIT", |ui| {
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
            group(ui, "VIEW", |ui| {
                let cur = VIEWS
                    .iter()
                    .position(|v| v.0 == self.view)
                    .unwrap_or(usize::MAX);
                let items: Vec<(&str, &str)> = VIEWS.iter().map(|v| (v.1, v.2)).collect();
                if let Some(i) = segmented(ui, &items, cur) {
                    self.view = VIEWS[i].0;
                    self.arr_focus = false;
                }
                // the Browser panel on the right (docs/BROWSER.md)
                let on = if self.extra.browser_open { 0 } else { usize::MAX };
                if segmented(ui, &[("Browser", "show / hide the Browser on the right (Ctrl+B)")], on).is_some() {
                    self.toggle_browser();
                }
            });
            let (lit, midi_tip) = self.midi_light(ui.input(|i| i.time));
            if lit {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(160));
            }
            // the right-hand status cluster (latency, Rec, Chord, MIDI) goes on
            // its own row when it does not fit next to the groups (it used to
            // overlap the view tabs at ~1600 px with the latency text shown)
            let right_id = ui.scope_id().with("toolbar_right_w");
            let right_w = ui
                .ctx()
                .data(|d| d.get_temp::<f32>(right_id))
                .unwrap_or(340.0);
            if ui.available_size_before_wrap().x < right_w + 12.0 {
                ui.end_row();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let right_edge = ui.max_rect().right();
                ui.add_space(10.0);
                // PANIC (0.2.3.16): far right, small, never a focus target
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("Panic").small().color(hdr_dim()))
                            .small()
                            .min_size(egui::vec2(0.0, 16.0)),
                    )
                    .on_hover_text("Panic: stop all sound (Ctrl+.)")
                    .clicked()
                {
                    self.panic();
                }
                ui.add_space(6.0);
                if let Some(a) = &self.audio {
                    let u = a.stats.underruns.load(std::sync::atomic::Ordering::Relaxed);
                    let txt = format!("{:.0} ms · {} underruns", a.latency_ms(), u);
                    let col = match (u > 0, t.light) {
                        (true, true) => Color32::from_rgb(255, 200, 160),
                        (true, false) => t.brick,
                        _ => hdr_dim(),
                    };
                    ui.label(egui::RichText::new(txt).small().monospace().color(col))
                        .on_hover_text(format!(
                            "audio: {} @ {} Hz, latency and buffer underruns",
                            a.device_name, a.sample_rate
                        ));
                }
                // MIDI quick toggles (laid out right to left: shown as light, Chord, Rec)
                let m = &mut self.extra.midi;
                let rec_txt = if m.live_record { "Rec live" } else { "Rec step" };
                let armed = self.edit;
                let rec_col = if armed { t.brick } else { hdr_dim() };
                let mut toggled = false;
                if ui
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
                    .selectable_label(m.chord_spread, egui::RichText::new("Chord").small())
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
                    FontId::proportional(11.0),
                    hdr_dim(),
                );
                if resp
                    .on_hover_text(format!("{midi_tip}\nclick: MIDI settings"))
                    .clicked()
                {
                    self.view = View::Settings;
                    self.set_card = 4;
                    self.set_open = Some(4);
                }
                let used = right_edge - ui.min_rect().left();
                ui.ctx().data_mut(|d| d.insert_temp(right_id, used));
            });
        });
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.about_window(ctx);
        if self.orphans.is_empty() {
            return;
        }
        let mut act: Option<(usize, bool)> = None; // (index, recover?)
        let mut later = false;
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
                                    if ui
                                        .button("Discard")
                                        .on_hover_text("delete this recovery copy")
                                        .clicked()
                                    {
                                        act = Some((i, false));
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                ui.add_space(6.0);
                if ui
                    .button("Later")
                    .on_hover_text("keep the copies and ask again next start")
                    .clicked()
                {
                    later = true;
                }
            });
        if let Some((i, rec)) = act {
            let o = self.orphans.remove(i);
            if rec {
                match fileio::load_cwt(&o.file) {
                    Ok(mut song) => {
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
                        self.reset_cursor();
                        self.path = None;
                        self.dirty = true;
                        self.status = "Recovered - choose File > Save As to keep it".into();
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
            self.orphans.clear();
        }
    }
}
