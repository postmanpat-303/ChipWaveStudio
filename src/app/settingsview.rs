//! Settings screen (docs/SETTINGS.md): summary cards that expand to details.
use super::*;
use crate::keymap::{self, Action, Bind, Combo};

pub(super) const CARDS: [&str; 6] = ["Audio", "Look", "Keys", "Files & Safety", "MIDI", "Plugins"];

pub(super) fn open_folder(p: &std::path::Path) {
    let _ = std::fs::create_dir_all(p);
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd).arg(p).spawn();
}

impl App {
    pub(super) fn finish_rebind(&mut self, a: Action, c: Combo) {
        self.rebinding = None;
        if c.key == Key::Escape && !c.ctrl && !c.shift && !c.alt {
            self.bind_msg = "Rebinding cancelled".into();
            return;
        }
        self.bind_msg = match self.keymap.set(a, c) {
            Bind::Ok => {
                self.keymap.save();
                format!("{} → {}", keymap::label_of(a), c.text())
            }
            Bind::Conflict(o) => format!(
                "{} is already used by \"{}\". Pick another key, or change that action first.",
                c.text(),
                keymap::label_of(o)
            ),
        };
    }

    fn card_summary(&self, i: usize) -> (Vec<String>, Option<String>) {
        match i {
            0 => match &self.audio {
                Some(a) => {
                    let u = a.stats.underruns.load(std::sync::atomic::Ordering::Relaxed);
                    (
                        vec![
                            a.device_name.clone(),
                            format!("{} Hz · buffer {}", a.sample_rate, a.buffer),
                            format!(
                                "latency {:.0} ms · {} underruns · load {:.0} %",
                                a.latency_ms(),
                                u,
                                a.load_pct()
                            ),
                        ],
                        a.advice(),
                    )
                }
                None => (
                    vec!["no audio output".into()],
                    Some(
                        self.audio_err
                            .clone()
                            .unwrap_or_else(|| "audio stopped".into()),
                    ),
                ),
            },
            1 => (
                vec![
                    self.theme.name.to_string(),
                    format!("scale {:.0} %", self.zoom * 100.0),
                    format!(
                        "accent {}",
                        if self.extra.accent.is_empty() {
                            "theme default"
                        } else {
                            &self.extra.accent
                        }
                    ),
                ],
                None,
            ),
            2 => (
                vec![
                    "ChipWave keymap".into(),
                    format!("{} custom binding(s)", self.keymap.overrides.len()),
                    format!("base octave {}", self.octave),
                ],
                None,
            ),
            3 => (
                vec![
                    if self.extra.autosave_mins == 0 {
                        "autosave off".to_string()
                    } else {
                        format!("autosave every {} min", self.extra.autosave_mins)
                    },
                    format!("{} backup(s) on save", self.extra.backups),
                    if self.dirty {
                        "unsaved changes".into()
                    } else {
                        "all saved".into()
                    },
                ],
                (self.extra.autosave_mins == 0).then(|| "autosave is off".to_string()),
            ),
            4 => self.midi_summary(),
            _ => self.plugins_summary(),
        }
    }

    pub(super) fn settings_view(&mut self, ui: &mut egui::Ui) {
        let t = th();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Settings").strong().size(17.0));
            ui.label(
                egui::RichText::new("arrows = move · Enter = open / close · Esc or Ctrl+, = back")
                    .small()
                    .color(t.ink2),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").on_hover_text("Esc").clicked() {
                    self.view = View::Pattern;
                }
            });
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_salt("settings_scroll")
            .show(ui, |ui| {
                let w = ui.available_width();
                let cols = ((w / 300.0) as usize).clamp(1, 3);
                let cw = (w - (cols as f32 - 1.0) * 10.0) / cols as f32 - 2.0;
                for row in 0..CARDS.len().div_ceil(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        #[allow(clippy::needless_range_loop)]
                        for i in row * cols..((row + 1) * cols).min(CARDS.len()) {
                            let (lines, warn) = self.card_summary(i);
                            let focused = self.set_card == i;
                            let open = self.set_open == Some(i);
                            let placeholder = i >= CARDS.len();
                            let edge = if warn.is_some() {
                                t.brick
                            } else if focused {
                                t.focus_ring
                            } else {
                                t.ink2.gamma_multiply(0.4)
                            };
                            let r = egui::Frame::new()
                                .fill(if open {
                                    t.cart_top.gamma_multiply(1.0)
                                } else {
                                    t.cart_bot
                                })
                                .stroke(Stroke::new(if focused || open { 2.0 } else { 1.0 }, edge))
                                .corner_radius(8)
                                .inner_margin(10)
                                .show(ui, |ui| {
                                    ui.set_width(cw - 20.0);
                                    ui.set_height(92.0);
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(CARDS[i])
                                                    .strong()
                                                    .size(15.0)
                                                    .color(if placeholder {
                                                        t.ink2
                                                    } else {
                                                        t.ink
                                                    }),
                                            );
                                            ui.label(
                                                egui::RichText::new(if open {
                                                    "[open]"
                                                } else {
                                                    "details >"
                                                })
                                                .small()
                                                .color(t.ink2),
                                            );
                                        });
                                        for l in &lines {
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(l)
                                                        .color(if placeholder {
                                                            t.ink2
                                                        } else {
                                                            t.value
                                                        })
                                                        .monospace(),
                                                )
                                                .truncate(),
                                            );
                                        }
                                        match &warn {
                                            Some(w) => ui.label(
                                                egui::RichText::new(format!("! {w}"))
                                                    .small()
                                                    .color(t.brick),
                                            ),
                                            None => ui.label(
                                                egui::RichText::new(if placeholder {
                                                    "placeholder"
                                                } else {
                                                    "OK"
                                                })
                                                .small()
                                                .color(t.ink2),
                                            ),
                                        };
                                    });
                                })
                                .response;
                            let r = ui.interact(
                                r.rect,
                                ui.make_persistent_id(("card", i)),
                                Sense::click(),
                            );
                            if r.clicked() {
                                self.set_card = i;
                                self.set_open = if open { None } else { Some(i) };
                            }
                        }
                    });
                    ui.add_space(10.0);
                }
                if let Some(i) = self.set_open {
                    ui.separator();
                    ui.label(egui::RichText::new(CARDS[i]).strong().size(16.0));
                    ui.add_space(4.0);
                    match i {
                        0 => self.audio_details(ui),
                        1 => self.look_details(ui),
                        2 => self.keys_details(ui),
                        3 => self.files_details(ui),
                        4 => self.midi_details(ui),
                        _ => self.plugin_settings(ui),
                    }
                }
            });
    }

    fn audio_details(&mut self, ui: &mut egui::Ui) {
        let mut restart = false;
        egui::Grid::new("aud")
            .num_columns(2)
            .spacing([14.0, 8.0])
            .show(ui, |ui| {
                ui.label("Output device");
                let cur = if self.extra.device.is_empty() {
                    "System default".to_string()
                } else {
                    self.extra.device.clone()
                };
                egui::ComboBox::from_id_salt("dev")
                    .selected_text(cur)
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.extra.device.is_empty(), "System default")
                            .clicked()
                        {
                            self.extra.device.clear();
                            restart = true;
                        }
                        for d in audio::output_devices() {
                            if ui.selectable_label(self.extra.device == d, &d).clicked() {
                                self.extra.device = d;
                                restart = true;
                            }
                        }
                    });
                ui.end_row();
                ui.label("Sample rate");
                let rt = |r: u32| {
                    if r == 0 {
                        "Device default".to_string()
                    } else {
                        format!("{r} Hz")
                    }
                };
                egui::ComboBox::from_id_salt("rate")
                    .selected_text(rt(self.extra.rate))
                    .show_ui(ui, |ui| {
                        let mut rates = vec![0u32];
                        let dev = audio::device_rates(&self.extra.device);
                        rates.extend(if dev.is_empty() {
                            vec![44100, 48000]
                        } else {
                            dev
                        });
                        for r in rates {
                            if ui.selectable_label(self.extra.rate == r, rt(r)).clicked() {
                                self.extra.rate = r;
                                restart = true;
                            }
                        }
                    });
                ui.end_row();
                ui.label("Buffer");
                let bt = |b: u32| {
                    if b == 0 {
                        "Auto".to_string()
                    } else {
                        format!("{b} frames")
                    }
                };
                egui::ComboBox::from_id_salt("buf")
                    .selected_text(bt(self.buffer))
                    .show_ui(ui, |ui| {
                        for b in audio::BUFFER_CHOICES {
                            if ui.selectable_label(self.buffer == b, bt(b)).clicked() {
                                self.buffer = b;
                                restart = true;
                            }
                        }
                    });
                ui.end_row();
                ui.label("Status");
                match &self.audio {
                    Some(a) => {
                        let st = &a.stats;
                        ui.label(
                            egui::RichText::new(format!(
                                "{} @ {} Hz · callback {} frames · latency {:.1} ms · underruns {} · engine load {:.0} %",
                                a.device_name,
                                a.sample_rate,
                                st.callback_frames
                                    .load(std::sync::atomic::Ordering::Relaxed),
                                a.latency_ms(),
                                st.underruns.load(std::sync::atomic::Ordering::Relaxed),
                                a.load_pct()
                            ))
                            .monospace(),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new(self.audio_err.clone().unwrap_or_default())
                                .color(th().brick),
                        );
                    }
                }
                ui.end_row();
                ui.label("Test");
                ui.horizontal(|ui| {
                    if ui
                        .button("Play test tone")
                        .on_hover_text("1 s, 440 Hz, -12 dB")
                        .clicked()
                    {
                        self.with(|s| s.engine.test_tone = s.engine.sr as u32);
                    }
                    if ui.button("Restart audio").clicked() {
                        restart = true;
                    }
                });
                ui.end_row();
            });
        if restart {
            self.save_settings();
            self.restart_audio();
        }
    }

    fn look_details(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        egui::Grid::new("look")
            .num_columns(2)
            .spacing([14.0, 8.0])
            .show(ui, |ui| {
                ui.label("Theme");
                ui.horizontal(|ui| {
                    for opt in theme::THEMES {
                        if ui.selectable_label(self.theme == opt, opt.name).clicked() {
                            self.theme = opt;
                            changed = true;
                        }
                    }
                });
                ui.end_row();
                ui.label("Interface style");
                ui.horizontal(|ui| {
                    let a = ui
                        .selectable_value(&mut self.extra.flat_ui, false, "Polished")
                        .on_hover_text("soft depth: rounded clips with a gentle gradient, shadows under panels and menus, shaded knobs and meters (default)");
                    let b = ui
                        .selectable_value(&mut self.extra.flat_ui, true, "Flat")
                        .on_hover_text("the plain look: flat fills, no shadows");
                    changed |= a.changed() || b.changed();
                });
                ui.end_row();
                ui.label("Motion");
                changed |= ui
                    .checkbox(&mut self.extra.reduce_motion, "Reduce motion")
                    .on_hover_text(
                        "menus such as the track pie appear and close at once, without animation",
                    )
                    .changed();
                ui.end_row();
                ui.label("Scale");
                ui.horizontal(|ui| {
                    let mut z = self.zoom;
                    if ui
                        .add(
                            egui::Slider::new(&mut z, 0.5..=self.max_zoom.max(1.0))
                                .step_by(0.05)
                                .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                        )
                        .changed()
                    {
                        self.set_zoom(z);
                    }
                    ui.label(
                        egui::RichText::new("Ctrl+= / Ctrl+- / Ctrl+0")
                            .small()
                            .color(th().ink2),
                    );
                });
                ui.end_row();
                ui.label("Accent");
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.extra.accent.is_empty(), "Theme default")
                        .clicked()
                    {
                        self.extra.accent.clear();
                        changed = true;
                    }
                    for (n, c) in theme::ACCENTS {
                        let (r, resp) =
                            ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 9.0, hex(c));
                        if self.extra.accent == n {
                            ui.painter().circle_stroke(
                                r.center(),
                                11.0,
                                Stroke::new(2.0, th().ink),
                            );
                        }
                        if resp.on_hover_text(n).clicked() {
                            self.extra.accent = n.into();
                            changed = true;
                        }
                    }
                });
                ui.end_row();
                ui.label("Tracker");
                if ui
                    .checkbox(&mut self.extra.alt_columns, "Alternate column shading")
                    .on_hover_text(
                        "tint every other channel column, with a thin divider between columns",
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
                ui.label("New songs");
                if ui
                    .checkbox(&mut self.extra.new_in_timeline, "Open on the Timeline view")
                    .on_hover_text(
                        "File > New starts on the Timeline (tracks and clips); off: on the Tracker",
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
                ui.label("Opening songs");
                if ui
                    .checkbox(
                        &mut self.extra.open_in_timeline,
                        "Open tracker songs in timeline mode",
                    )
                    .on_hover_text(
                        "tracker-mode songs and demos switch to timeline mode when opened \
                         (they play the same; File > Export tracker-compatible copy for 0.2.4.0 \
                         and older); off: they stay in tracker mode and the first timeline edit asks",
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
                ui.label("Double-click a clip");
                ui.horizontal(|ui| {
                    let a = ui
                        .selectable_value(&mut self.extra.dblclick_roll, true, "Piano roll")
                        .on_hover_text("Timeline: double-click opens the clip in the lower panel's piano roll (Alt+double-click: the Tracker)");
                    let b = ui
                        .selectable_value(&mut self.extra.dblclick_roll, false, "Tracker")
                        .on_hover_text("Timeline: double-click opens a Tracker clip in the Tracker (Alt+double-click: the piano roll)");
                    changed |= a.changed() || b.changed();
                });
                ui.end_row();
                ui.label("Chord menus");
                ui.horizontal(|ui| {
                    let a = ui
                        .selectable_value(&mut self.extra.chord_list_menu, false, "Pie")
                        .on_hover_text("Piano roll chord lane: right-click opens a round menu you point at");
                    let b = ui
                        .selectable_value(&mut self.extra.chord_list_menu, true, "List")
                        .on_hover_text("Piano roll chord lane: right-click opens a normal list menu");
                    changed |= a.changed() || b.changed();
                });
                ui.end_row();
                ui.label("Row numbers");
                ui.horizontal(|ui| {
                    let a = ui
                        .selectable_value(&mut self.extra.row_hex, false, "Decimal")
                        .on_hover_text("00 01 … 63 (three digits for blocks over 100 rows)");
                    let b = ui
                        .selectable_value(&mut self.extra.row_hex, true, "Hex")
                        .on_hover_text("00 01 … 3F, like classic trackers");
                    changed |= a.changed() || b.changed();
                });
                ui.end_row();
            });
        if changed {
            self.apply_theme(ui.ctx());
            self.save_settings();
        }
    }

    fn keys_details(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(
                    "Click a key to change it, then press the new key (Esc cancels).",
                )
                .color(th().ink2),
            );
            if ui.button("Reset all").clicked() {
                self.keymap.reset_all();
                self.keymap.save();
                self.bind_msg = "All keys reset to defaults".into();
            }
        });
        // message row is always reserved so the table never shifts
        if self.bind_msg.is_empty() {
            ui.label(" ");
        } else {
            let warn = self.bind_msg.contains("already used");
            ui.label(egui::RichText::new(&self.bind_msg).color(if warn {
                th().brick
            } else {
                th().accent
            }));
        }
        egui::Grid::new("keys_map")
            .num_columns(3)
            .striped(true)
            .spacing([18.0, 4.0])
            .show(ui, |ui| {
                for (a, label, _) in keymap::DEFAULTS {
                    ui.label(label);
                    let listening = self.rebinding == Some(a);
                    let txt = if listening {
                        "press a key…".to_string()
                    } else {
                        self.keymap.binding(a).text()
                    };
                    let custom = self.keymap.overrides.contains_key(&a);
                    let b =
                        egui::Button::new(egui::RichText::new(txt).monospace().color(if custom {
                            th().accent
                        } else {
                            th().value
                        }))
                        .min_size(egui::vec2(150.0, 0.0));
                    if ui
                        .add(b)
                        .on_hover_text("click, then press the new key")
                        .clicked()
                    {
                        self.rebinding = Some(a);
                        self.bind_msg.clear();
                    }
                    if custom {
                        if ui.small_button("reset").clicked() {
                            self.bind_msg = match self.keymap.reset(a) {
                                Bind::Ok => {
                                    self.keymap.save();
                                    format!("{} reset", label)
                                }
                                Bind::Conflict(o) => format!(
                                    "can't reset: the default key is used by \"{}\"",
                                    keymap::label_of(o)
                                ),
                            };
                        }
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
        if let Some(p) = crate::keymap::Keymap::path() {
            ui.label(
                egui::RichText::new(format!("saved to {}", p.display()))
                    .small()
                    .color(th().ink2),
            );
        }
    }

    fn files_details(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        egui::Grid::new("files")
            .num_columns(2)
            .spacing([14.0, 8.0])
            .show(ui, |ui| {
                ui.label("Autosave");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut self.extra.autosave_mins)
                                .range(0..=30)
                                .suffix(" min"),
                        )
                        .changed();
                    ui.label(
                        egui::RichText::new("0 = off; only when there are unsaved changes")
                            .small()
                            .color(th().ink2),
                    );
                });
                ui.end_row();
                ui.label("Backups on save");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(egui::DragValue::new(&mut self.extra.backups).range(0..=10))
                        .changed();
                    ui.label(
                        egui::RichText::new("song.cwt.bak, .bak2 …")
                            .small()
                            .color(th().ink2),
                    );
                });
                ui.end_row();
                ui.label("Updates");
                ui.horizontal(|ui| {
                    changed |= ui
                        .checkbox(&mut self.extra.update_check, "Check for updates at startup")
                        .on_hover_text(
                            "quietly asks GitHub at most once a day; a newer version shows as a small badge in the menu bar",
                        )
                        .changed();
                    if ui.button("Check now").clicked() {
                        self.check_updates(true);
                    }
                    if !self.extra.update_skip.is_empty()
                        && ui
                            .small_button(format!("Un-skip {}", self.extra.update_skip))
                            .on_hover_text("remind me about the skipped version again")
                            .clicked()
                    {
                        self.extra.update_skip.clear();
                        changed = true;
                    }
                });
                ui.end_row();
                ui.label("Recovery folder");
                ui.horizontal(|ui| {
                    let p = crate::recovery::root();
                    ui.label(
                        egui::RichText::new(
                            p.as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default(),
                        )
                        .monospace()
                        .small(),
                    );
                    if ui.button("Open").clicked() {
                        if let Some(p) = p {
                            open_folder(&p);
                        }
                    }
                });
                ui.end_row();
                ui.label("Default song folder");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.extra.song_dir)
                                .hint_text("(last used)")
                                .desired_width(300.0),
                        )
                        .changed();
                    if ui.button("Browse…").clicked() {
                        if let Some(d) = rfd::FileDialog::new().pick_folder() {
                            self.extra.song_dir = d.display().to_string();
                            changed = true;
                        }
                    }
                });
                ui.end_row();
                ui.label("Instrument library");
                ui.horizontal(|ui| {
                    let def = crate::instfile::library_dir("")
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.extra.library_dir)
                                .hint_text(def)
                                .desired_width(300.0),
                        )
                        .on_hover_text("where Save to library puts .cwi instrument files; sub-folders become categories")
                        .changed();
                    if ui.button("Browse…").clicked() {
                        if let Some(d) = rfd::FileDialog::new().pick_folder() {
                            self.extra.library_dir = d.display().to_string();
                            changed = true;
                        }
                    }
                    if ui.button("Open").clicked() {
                        if let Some(p) = crate::instfile::library_dir(&self.extra.library_dir) {
                            let _ = std::fs::create_dir_all(&p);
                            open_folder(&p);
                        }
                    }
                });
                ui.end_row();
                ui.label("Recent files");
                ui.horizontal(|ui| {
                    ui.label(format!("{} song(s)", self.recent.len()));
                    if ui.button("Clear").clicked() {
                        self.recent.clear();
                        changed = true;
                    }
                });
                ui.end_row();
            });
        if changed {
            self.save_settings();
        }
    }
}
