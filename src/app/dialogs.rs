//! Modal dialogs: unsaved-changes prompt, export (WAV / MP3), song info.
use super::*;
use crate::export::{self, Format, Mp3Mode, Opts, Scope};
use crate::prompt::{self, Choice, Pending};
use std::sync::mpsc::Receiver;

type Job = (Arc<export::Progress>, Receiver<Result<String, String>>);

pub(super) struct ExportDlg {
    pub opts: Opts,
    pub path: String,
    pub job: Option<Job>,
    result: Option<Result<String, String>>,
}

fn music_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join("Music").join("ChipWave")
}

fn ext(f: Format) -> &'static str {
    match f {
        Format::Wav => "wav",
        Format::Mp3 => "mp3",
    }
}

impl App {
    pub(super) fn apply_theme(&self, ctx: &egui::Context) {
        theme::set_polished(!self.extra.flat_ui);
        theme::apply(ctx, theme::with_accent(self.theme, &self.extra.accent));
    }

    pub(super) fn toggle_settings(&mut self) {
        self.view = if self.view == View::Settings {
            self.home_view()
        } else {
            View::Settings
        };
        self.rebinding = None;
    }

    pub(super) fn request(&mut self, p: Pending, ctx: &egui::Context) {
        match prompt::request(self.dirty, p) {
            prompt::Step::Run(p) => self.run_pending(p, ctx),
            prompt::Step::Ask(p) => self.prompt = Some(p),
        }
    }

    fn run_pending(&mut self, p: Pending, ctx: &egui::Context) {
        match p {
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Pending::New => self.new_song(),
            Pending::Open => self.open(),
            Pending::OpenPath(p) => self.open_path(p),
            Pending::Demo(i) => self.load_demo(i),
        }
    }

    fn answer(&mut self, c: Choice, ctx: &egui::Context) {
        let Some(p) = self.prompt.take() else { return };
        let saved = if c == Choice::Save {
            self.save_and_wait()
        } else {
            false
        };
        match prompt::resolve(c, saved) {
            prompt::Outcome::Run { discarded } => {
                if discarded {
                    self.discarded = true;
                    self.dirty = false;
                }
                self.run_pending(p, ctx);
            }
            prompt::Outcome::Stay => {
                if c == Choice::Save {
                    self.status = "Not saved - still open".into();
                }
            }
        }
    }

    pub(super) fn open_export(&mut self) {
        if self.export_dlg.is_some() {
            return;
        }
        let title = self.with(|s| s.song.title.clone());
        let dir = if self.extra.export_dir.is_empty() {
            music_dir()
        } else {
            PathBuf::from(&self.extra.export_dir)
        };
        let opts = Opts::default();
        let path = dir.join(format!("{}.{}", sanitize(&title), ext(opts.format)));
        self.export_dlg = Some(ExportDlg {
            opts,
            path: path.display().to_string(),
            job: None,
            result: None,
        });
    }

    fn selection_range(&self) -> (usize, usize) {
        if self.arr_multi.is_empty() {
            (self.arr_sel, self.arr_sel)
        } else {
            let a = *self.arr_multi.iter().min().unwrap();
            let b = *self.arr_multi.iter().max().unwrap();
            (a, b)
        }
    }

    pub(super) fn start_export(&mut self) {
        self.capture_plugins();
        let song = self.with(|s| s.song.clone());
        let Some(d) = &mut self.export_dlg else {
            return;
        };
        if d.job.is_some() {
            return;
        }
        let p = PathBuf::from(d.path.trim());
        if p.as_os_str().is_empty() {
            d.result = Some(Err("choose an output file".into()));
            return;
        }
        if let Some(dir) = p.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                d.result = Some(Err(format!("can't create {}: {e}", dir.display())));
                return;
            }
            self.extra.export_dir = dir.display().to_string();
        }
        d.result = None;
        d.job = Some(export::spawn(song, d.opts.clone(), p));
        self.save_settings();
    }

    /// keys for dialogs / the settings screen; true = consumed
    pub(super) fn modal_key(&mut self, key: Key, m: egui::Modifiers, ctx: &egui::Context) -> bool {
        if key == Key::Escape && self.ca.drag.is_some() {
            // a slot drag in progress: Esc drops nothing
            self.ca.drag = None;
            self.status = "Drag cancelled".into();
            return true;
        }
        if self.drop_ask.is_some() {
            // the clip-drop question: Esc cancels, nothing edits behind it
            if key == Key::Escape {
                self.drop_ask = None;
                self.status = "Not placed (cancelled)".into();
            }
            return true;
        }
        if self.prompt.is_some() {
            match key {
                Key::Enter => self.answer(Choice::Save, ctx),
                Key::N | Key::D => self.answer(Choice::DontSave, ctx),
                Key::Escape => self.answer(Choice::Cancel, ctx),
                _ => {}
            }
            return true;
        }
        if let Some(d) = &mut self.export_dlg {
            let running = d.job.is_some();
            match key {
                Key::Escape if running => {
                    if let Some((p, _)) = &d.job {
                        p.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                Key::Escape => self.export_dlg = None,
                Key::Enter if !running => self.start_export(),
                Key::ArrowLeft | Key::ArrowRight if !running => {
                    d.opts.format = if d.opts.format == Format::Wav {
                        Format::Mp3
                    } else {
                        Format::Wav
                    };
                    let np = PathBuf::from(&d.path).with_extension(ext(d.opts.format));
                    d.path = np.display().to_string();
                }
                _ => {}
            }
            return true;
        }
        if self.fill_key(key, m) {
            return true;
        }
        if self.show_info && key == Key::Escape {
            self.show_info = false;
            return true;
        }
        if self.view == View::Settings && !m.command {
            let n = settingsview::CARDS.len();
            match key {
                Key::ArrowRight => self.set_card = (self.set_card + 1) % n,
                Key::ArrowLeft => self.set_card = (self.set_card + n - 1) % n,
                Key::ArrowDown => self.set_card = (self.set_card + 3).min(n - 1),
                Key::ArrowUp => self.set_card = self.set_card.saturating_sub(3),
                Key::Enter | Key::Space => {
                    self.set_open = if self.set_open == Some(self.set_card) {
                        None
                    } else {
                        Some(self.set_card)
                    };
                }
                Key::Escape => self.view = self.home_view(),
                _ => return false,
            }
            return true;
        }
        false
    }

    pub(super) fn modal_dialogs(&mut self, ctx: &egui::Context) {
        // ---------------------------------------------------- unsaved prompt
        if let Some(p) = &self.prompt {
            let (title, lead) = prompt_text(
                p,
                &self.with(|s| s.song.title.clone()),
                self.path.as_deref(),
            );
            let t = th();
            let mut choice = None;
            egui::Window::new("Save changes")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .fixed_size(egui::vec2(PROMPT_W, 0.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .frame(
                    egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(20)),
                )
                .show(ctx, |ui| {
                    ui.set_width(PROMPT_W);
                    if t.ember {
                        ui.label(
                            egui::RichText::new(title)
                                .font(theme::label_font(19.0))
                                .color(t.ink),
                        );
                    } else {
                        ui.label(egui::RichText::new(title).heading().strong().color(t.ink));
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(lead).color(t.ink2));
                    ui.add_space(18.0);
                    // three buttons of one width, right-aligned in the
                    // reading order Save · Don't save · Cancel
                    let bw = egui::vec2(PROMPT_BTN_W, 30.0);
                    let gap = 10.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2(PROMPT_W, bw.y),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            ui.add_space((PROMPT_W - 3.0 * bw.x - 2.0 * gap).max(0.0));
                            // Slate: the outlined buttons, Save lit as the default
                            let btn = |ui: &mut egui::Ui, text: &str, primary: bool| {
                                if t.ember {
                                    crate::ember::button_ex(ui, text, false, primary, bw)
                                } else if primary {
                                    ui.add(
                                        egui::Button::new(egui::RichText::new(text).strong())
                                            .min_size(bw)
                                            .fill(t.accent.gamma_multiply(0.6)),
                                    )
                                } else {
                                    ui.add(egui::Button::new(text).min_size(bw))
                                }
                            };
                            if btn(ui, "Save", true)
                                .on_hover_text("save, then go on (Enter)")
                                .clicked()
                            {
                                choice = Some(Choice::Save);
                            }
                            if btn(ui, "Don't save", false)
                                .on_hover_text("go on and lose the changes (N)")
                                .clicked()
                            {
                                choice = Some(Choice::DontSave);
                            }
                            if btn(ui, "Cancel", false)
                                .on_hover_text("stay with this song (Esc)")
                                .clicked()
                            {
                                choice = Some(Choice::Cancel);
                            }
                        },
                    );
                });
            if let Some(c) = choice {
                self.answer(c, ctx);
            }
        }
        // ---------------------------------------------------- song info
        if self.show_info {
            let mut open = true;
            let (mut title, mut info) = self.with(|s| (s.song.title.clone(), s.song.info.clone()));
            let (t0, i0) = (title.clone(), info.clone());
            egui::Window::new("Song info").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
                egui::Grid::new("info").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label("Title");
                    ui.add(egui::TextEdit::singleline(&mut title).desired_width(260.0));
                    ui.end_row();
                    ui.label("Artist");
                    ui.add(egui::TextEdit::singleline(&mut info.artist).desired_width(260.0));
                    ui.end_row();
                    ui.label("Album");
                    ui.add(egui::TextEdit::singleline(&mut info.album).desired_width(260.0));
                    ui.end_row();
                    ui.label("Year");
                    ui.add(egui::TextEdit::singleline(&mut info.year).desired_width(80.0));
                    ui.end_row();
                    ui.label("Comment");
                    // fixed-height box: long comments scroll instead of growing the window
                    egui::ScrollArea::vertical().id_salt("info_comment").max_height(64.0).show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(&mut info.comment).desired_rows(3).desired_width(260.0));
                    });
                    ui.end_row();
                });
                ui.label(egui::RichText::new("Saved in the song; used for MP3 tags (the comment gets \"Made with ChipWave\").").small().color(th().ink2));
            });
            if title != t0 || info != i0 {
                self.with(|s| {
                    s.song.title = title;
                    s.song.info = info;
                });
                self.dirty = true;
            }
            self.show_info = open;
        }
        // ---------------------------------------------------- export
        let Some(mut d) = self.export_dlg.take() else {
            return;
        };
        if let Some((_, rx)) = &d.job {
            if let Ok(r) = rx.try_recv() {
                d.result = Some(match r {
                    Err(e) if e == export::CANCELLED => Err("Export cancelled".into()),
                    r => r,
                });
                d.job = None;
                if let Some(Ok(m)) = &d.result {
                    self.status = m.clone();
                }
            } else {
                ctx.request_repaint();
            }
        }
        let (title, info, npat, cur_pat) = self.with(|s| {
            (
                s.song.title.clone(),
                s.song.info.clone(),
                s.song.patterns.len(),
                self.pattern,
            )
        });
        let _ = npat;
        let ml = self.with(|s| s.song.mixer.limiter);
        let (sa, sb) = self.selection_range();
        let (tl_mode, tl_loop) = self.with(|s| {
            let t = s.song.timeline.as_deref();
            (t.is_some(), t.is_some_and(|t| t.loop_range.is_some()))
        });
        let mut open = true;
        let mut close_btn = false;
        let mut go = false;
        let mut browse = false;
        let mut edit_info = false;
        let running = d.job.is_some();
        egui::Window::new("Export audio")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(640.0);
                ui.add_enabled_ui(!running, |ui| {
                    egui::Grid::new("exp")
                        .num_columns(2)
                        .spacing([14.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Format").strong());
                            ui.horizontal(|ui| {
                                let before = d.opts.format;
                                ui.selectable_value(&mut d.opts.format, Format::Wav, "  WAV  ")
                                    .on_hover_text("32-bit float, lossless (Left/Right)");
                                ui.selectable_value(&mut d.opts.format, Format::Mp3, "  MP3  ")
                                    .on_hover_text("LAME encoder with ID3 tags (Left/Right)");
                                if before != d.opts.format {
                                    d.path = PathBuf::from(&d.path)
                                        .with_extension(ext(d.opts.format))
                                        .display()
                                        .to_string();
                                }
                            });
                            ui.end_row();
                            {
                                let is_mp3 = d.opts.format == Format::Mp3;
                                ui.add_enabled(is_mp3, egui::Label::new("MP3 mode"));
                                ui.add_enabled_ui(is_mp3, |ui| {
                                    ui.vertical(|ui| {
                                        ui.set_min_height(48.0);
                                        ui.horizontal(|ui| {
                                            let vbr = matches!(d.opts.mp3, Mp3Mode::Vbr(_));
                                            if ui
                                                .selectable_label(vbr, "VBR")
                                                .on_hover_text(
                                                    "variable bitrate, best quality per size",
                                                )
                                                .clicked()
                                                && !vbr
                                            {
                                                d.opts.mp3 = Mp3Mode::Vbr(2);
                                            }
                                            if ui
                                                .selectable_label(!vbr, "CBR")
                                                .on_hover_text("constant bitrate")
                                                .clicked()
                                                && vbr
                                            {
                                                d.opts.mp3 = Mp3Mode::Cbr(320);
                                            }
                                        });
                                        match &mut d.opts.mp3 {
                                            Mp3Mode::Cbr(k) => {
                                                ui.horizontal(|ui| {
                                                    for b in [128u16, 192, 256, 320] {
                                                        ui.selectable_value(
                                                            k,
                                                            b,
                                                            format!("{b} kbps"),
                                                        );
                                                    }
                                                });
                                            }
                                            Mp3Mode::Vbr(q) => {
                                                ui.horizontal(|ui| {
                                                    ui.add(egui::Slider::new(q, 0..=9).prefix("V"));
                                                    let kb = [
                                                        245, 225, 190, 175, 165, 130, 115, 100, 85,
                                                        65,
                                                    ]
                                                        [(*q).min(9) as usize];
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "≈ {kb} kbps · V0 best"
                                                        ))
                                                        .small()
                                                        .color(th().ink2),
                                                    );
                                                });
                                            }
                                        }
                                    })
                                })
                                .response
                                .on_disabled_hover_text("MP3 only: switch Format to MP3");
                                ui.end_row();
                            }
                            ui.label("Sample rate");
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut d.opts.sample_rate, 44100, "44.1 kHz");
                                ui.selectable_value(&mut d.opts.sample_rate, 48000, "48 kHz");
                            });
                            ui.end_row();
                            ui.label("Range");
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut d.opts.scope, Scope::Song, "Whole song");
                                ui.selectable_value(
                                    &mut d.opts.scope,
                                    Scope::Block(cur_pat),
                                    format!("Block {cur_pat:02}"),
                                )
                                .on_hover_text("the block open in the Tracker, once");
                                if tl_mode {
                                    ui.add_enabled(
                                        tl_loop,
                                        egui::Button::selectable(
                                            d.opts.scope == Scope::Loop,
                                            "Loop region",
                                        ),
                                    )
                                    .on_hover_text("the timeline's loop region, once")
                                    .on_disabled_hover_text(
                                        "no loop region: Shift-drag the timeline ruler to set one",
                                    )
                                    .clicked()
                                    .then(|| d.opts.scope = Scope::Loop);
                                } else {
                                    ui.selectable_value(
                                        &mut d.opts.scope,
                                        Scope::Range(sa, sb),
                                        format!("Order list selection {}–{}", sa + 1, sb + 1),
                                    )
                                    .on_hover_text("the clips selected in the Order list");
                                }
                            });
                            ui.end_row();
                            ui.label("Level");
                            ui.horizontal(|ui| {
                                if ml.enabled {
                                    ui.checkbox(
                                        &mut d.opts.normalise,
                                        format!("Normalise to {:.1} dBTP", ml.ceiling_db),
                                    )
                                    .on_hover_text(
                                        "scales the whole export so its true peak sits exactly \
                                         at the master limiter ceiling",
                                    );
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Master limiter on ({:.1} dBTP, +{:.1} dB gain)",
                                            ml.ceiling_db, ml.gain_db
                                        ))
                                        .small()
                                        .color(th().ink2),
                                    )
                                    .on_hover_text(
                                        "the export goes through the same true-peak limiter \
                                         as playback (Mixer > Limiter & loudness)",
                                    );
                                } else {
                                    ui.checkbox(&mut d.opts.normalise, "Normalise to −0.3 dBFS");
                                    ui.checkbox(&mut d.opts.limiter, "Soft limiter")
                                        .on_hover_text(
                                            "soft limiter, ceiling −0.3 dBFS (the master \
                                             limiter is bypassed in the mixer)",
                                        );
                                }
                            });
                            ui.end_row();
                            ui.label("Tail");
                            ui.add(
                                egui::Slider::new(&mut d.opts.tail_secs, 0.0..=10.0)
                                    .suffix(" s")
                                    .step_by(0.5),
                            )
                            .on_hover_text(
                                "extra time after the end for releases, delay and reverb",
                            );
                            ui.end_row();
                            let is_mp3 = d.opts.format == Format::Mp3;
                            ui.add_enabled(is_mp3, egui::Label::new("Tags"));
                            ui.add_enabled_ui(is_mp3, |ui| {
                                ui.horizontal(|ui| {
                                    let a = if info.artist.is_empty() {
                                        "no artist".to_string()
                                    } else {
                                        info.artist.clone()
                                    };
                                    ui.label(
                                        egui::RichText::new(format!("{title} · {a}"))
                                            .color(th().value),
                                    );
                                    if ui.small_button("Edit song info…").clicked() {
                                        edit_info = true;
                                    }
                                })
                            })
                            .response
                            .on_disabled_hover_text("ID3 tags are written to MP3 only");
                            ui.end_row();
                            ui.label("Save to");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut d.path).desired_width(360.0),
                                );
                                if ui.button("Browse…").clicked() {
                                    browse = true;
                                }
                            });
                            ui.end_row();
                        });
                });
                ui.add_space(6.0);
                let slot = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(slot, 44.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_size(egui::vec2(slot, 44.0));
                        if let Some((p, _)) = &d.job {
                            let f = p.get();
                            ui.add(
                                egui::ProgressBar::new(f)
                                    .show_percentage()
                                    .text(if f < 0.8 {
                                        format!("Rendering… {:.0} %", f * 100.0)
                                    } else {
                                        format!("Encoding… {:.0} %", f * 100.0)
                                    }),
                            );
                        }
                        match &d.result {
                            Some(Ok(m)) => {
                                ui.label(egui::RichText::new(m).color(th().accent));
                            }
                            Some(Err(e)) => {
                                ui.label(egui::RichText::new(e).color(th().brick));
                            }
                            None if d.job.is_none() => {
                                ui.label(egui::RichText::new("Ready").color(th().ink2));
                            }
                            None => {}
                        }
                    },
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if running {
                        if ui.button("Cancel").on_hover_text("Esc").clicked() {
                            if let Some((p, _)) = &d.job {
                                p.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    } else {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("  Export  ").strong())
                                    .fill(th().accent.gamma_multiply(0.6)),
                            )
                            .on_hover_text("Enter")
                            .clicked()
                        {
                            go = true;
                        }
                        if ui.button("Close").on_hover_text("Esc").clicked() {
                            close_btn = true;
                        }
                    }
                });
            });
        if browse {
            let f = d.opts.format;
            let cur = PathBuf::from(&d.path);
            let mut fd = rfd::FileDialog::new().add_filter(
                if f == Format::Wav {
                    "WAV audio"
                } else {
                    "MP3 audio"
                },
                &[ext(f)],
            );
            if let Some(dir) = cur.parent() {
                fd = fd.set_directory(dir);
            }
            if let Some(n) = cur.file_name() {
                fd = fd.set_file_name(n.to_string_lossy());
            }
            if let Some(p) = fd.save_file() {
                d.path = p.with_extension(ext(f)).display().to_string();
            }
        }
        if edit_info {
            self.show_info = true;
        }
        if (open && !close_btn) || running {
            self.export_dlg = Some(d);
        }
        if go {
            self.start_export();
        }
    }
}

/// the save prompt's width and its buttons' width
const PROMPT_W: f32 = 400.0;
const PROMPT_BTN_W: f32 = 108.0;

/// The save prompt's title and its one line of explanation:
/// `Save changes to "Song"?` - the song's title, else its file name, else
/// Untitled.
pub(crate) fn prompt_text(
    p: &Pending,
    title: &str,
    path: Option<&std::path::Path>,
) -> (String, String) {
    let name = if !title.trim().is_empty() {
        title.trim().to_string()
    } else {
        path.and_then(|p| p.file_stem())
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".into())
    };
    let lead = match p {
        Pending::Quit => "If you quit without saving, your changes are lost.",
        Pending::New => "If you start a new song without saving, your changes are lost.",
        Pending::Open | Pending::OpenPath(_) | Pending::Demo(_) => {
            "If you open another song without saving, your changes are lost."
        }
    };
    (
        format!("Save changes to \u{201C}{name}\u{201D}?"),
        lead.into(),
    )
}
