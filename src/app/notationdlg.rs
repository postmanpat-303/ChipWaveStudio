//! File > Export sheet music…: the parts, bars, rhythm grid and key of a
//! score, written as MusicXML and LilyPond files, and as a PDF when
//! LilyPond (a separate free program) is installed.
use super::*;
use crate::notation::{self, KeyChoice, Opts, GRIDS};
use std::path::Path;
use std::sync::mpsc::Receiver;

pub(super) struct NotationDlg {
    pub opts: Opts,
    /// the output path without its extension (each format adds its own)
    pub path: String,
    pub xml: bool,
    pub ly: bool,
    pub pdf: bool,
    /// whole song, or the bars in `opts.bars`
    pub some_bars: bool,
    /// bars in the song
    pub nbars: i64,
    /// the key Auto picks, for the label
    pub auto_key: String,
    pub lilypond: Option<PathBuf>,
    job: Option<Receiver<Result<String, String>>>,
    result: Option<Result<String, String>>,
}

/// the files written for `stem`: (MusicXML, LilyPond, PDF stem)
fn outputs(stem: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let name = stem
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir = stem.parent().map(Path::to_path_buf).unwrap_or_default();
    (
        dir.join(format!("{name}.musicxml")),
        dir.join(format!("{name}.ly")),
        dir.join(name),
    )
}

/// drop a known extension the user typed or the file picker added
fn strip_ext(p: &str) -> String {
    let t = p.trim();
    for e in [".musicxml", ".xml", ".ly", ".pdf"] {
        if t.len() > e.len() && t.to_lowercase().ends_with(e) {
            return t[..t.len() - e.len()].to_string();
        }
    }
    t.to_string()
}

impl App {
    pub(super) fn open_notation(&mut self) {
        if self.notation_dlg.is_some() {
            return;
        }
        let (opts, nbars, auto_key, title) = self.with(|s| {
            let o = notation::default_opts(&s.song, &self.extra.notation_composer);
            let t = notation::timeline_of(&s.song);
            let tracks = notation::extract(&s.song, &t);
            let k = notation::score_key(&s.song, &tracks, KeyChoice::Auto)
                .map(|k| k.name())
                .unwrap_or_else(|| "C major".into());
            (o, notation::bar_count(&s.song), k, s.song.title.clone())
        });
        let dir = if self.extra.export_dir.is_empty() {
            dialogs::music_dir()
        } else {
            PathBuf::from(&self.extra.export_dir)
        };
        let name = if title.trim().is_empty() {
            "Untitled".to_string()
        } else {
            sanitize(&title)
        };
        let lilypond = notation::lily::find(&self.extra.lilypond_path);
        let mut opts = opts;
        opts.bars = (1, nbars);
        self.notation_dlg = Some(NotationDlg {
            path: dir.join(name).display().to_string(),
            xml: true,
            ly: lilypond.is_none(),
            pdf: lilypond.is_some(),
            some_bars: false,
            nbars,
            auto_key,
            lilypond,
            opts,
            job: None,
            result: None,
        });
    }

    pub(super) fn start_notation(&mut self) {
        let song = self.with(|s| s.song.clone());
        let Some(d) = &mut self.notation_dlg else {
            return;
        };
        if d.job.is_some() {
            return;
        }
        let stem = PathBuf::from(strip_ext(&d.path));
        if stem.file_name().is_none() {
            d.result = Some(Err("choose where to save".into()));
            return;
        }
        let pdf = d.pdf && d.lilypond.is_some();
        if !d.xml && !d.ly && !pdf {
            d.result = Some(Err("tick at least one format".into()));
            return;
        }
        if !d
            .opts
            .parts
            .iter()
            .any(|p| p.on && (d.opts.drums || !p.drums))
        {
            d.result = Some(Err("tick at least one part".into()));
            return;
        }
        if let Some(dir) = stem.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                d.result = Some(Err(format!("can't create {}: {e}", dir.display())));
                return;
            }
            self.extra.export_dir = dir.display().to_string();
        }
        // the composer typed is remembered for next time (blank = none)
        self.extra.notation_composer = d.opts.composer.trim().to_string();
        let mut o = d.opts.clone();
        if !d.some_bars {
            o.bars = (1, 0);
        }
        let sc = notation::build(&song, &o);
        let (xml, ly, pdf_stem) = outputs(&stem);
        let (want_xml, want_ly) = (d.xml, d.ly);
        let exe = if pdf { d.lilypond.clone() } else { None };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = (|| -> Result<String, String> {
                let mut made = vec![];
                if want_xml {
                    std::fs::write(&xml, notation::to_musicxml(&sc))
                        .map_err(|e| format!("can't write {}: {e}", xml.display()))?;
                    made.push(xml.display().to_string());
                }
                let text = notation::to_ly(&sc);
                if want_ly {
                    std::fs::write(&ly, &text)
                        .map_err(|e| format!("can't write {}: {e}", ly.display()))?;
                    made.push(ly.display().to_string());
                }
                if let Some(exe) = exe {
                    // the .ly LilyPond reads: the one saved, else a temporary one
                    let src = if want_ly {
                        ly.clone()
                    } else {
                        let t = std::env::temp_dir()
                            .join(format!("chipwave-score-{}.ly", std::process::id()));
                        std::fs::write(&t, &text).map_err(|e| format!("can't write {e}"))?;
                        t
                    };
                    let r = notation::lily::run(&exe, &src, &pdf_stem);
                    if !want_ly {
                        let _ = std::fs::remove_file(&src);
                    }
                    let (pdf, log) = r?;
                    made.push(pdf.display().to_string());
                    let bad = notation::lily::problems(&log);
                    if !bad.is_empty() {
                        return Ok(format!(
                            "Saved {} (LilyPond warned: {})",
                            made.join(", "),
                            bad[0]
                        ));
                    }
                }
                Ok(format!("Saved {}", made.join(", ")))
            })();
            let _ = tx.send(r);
        });
        d.result = None;
        d.job = Some(rx);
        self.save_settings();
    }

    /// Esc closes, Enter exports; true = consumed (the dialog is open)
    pub(super) fn notation_key(&mut self, key: Key) -> bool {
        let Some(d) = &mut self.notation_dlg else {
            return false;
        };
        let running = d.job.is_some();
        match key {
            Key::Escape if !running => self.notation_dlg = None,
            Key::Enter if !running => self.start_notation(),
            _ => {}
        }
        true
    }

    pub(super) fn notation_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.notation_dlg.take() else {
            return;
        };
        if let Some(rx) = &d.job {
            if let Ok(r) = rx.try_recv() {
                if let Ok(m) = &r {
                    self.status = m.clone();
                }
                d.result = Some(r);
                d.job = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        let running = d.job.is_some();
        let mut open = true;
        let (mut close_btn, mut go, mut browse, mut find_ly, mut remerge) =
            (false, false, false, false, false);
        let t = th();
        // fits a 1366x768 screen: at most 720 x 640
        egui::Window::new("Export sheet music")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(680.0);
                ui.add_enabled_ui(!running, |ui| {
                    egui::Grid::new("notation_exp")
                        .num_columns(2)
                        .spacing([14.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Format").strong());
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut d.xml, "MusicXML")
                                    .on_hover_text("opens in MuseScore, Finale, Sibelius, Dorico and most notation apps");
                                ui.checkbox(&mut d.ly, "LilyPond .ly")
                                    .on_hover_text("text you can edit and engrave with LilyPond or Frescobaldi");
                                ui.add_enabled(d.lilypond.is_some(), egui::Checkbox::new(&mut d.pdf, "PDF"))
                                    .on_hover_text("engraved by LilyPond")
                                    .on_disabled_hover_text("PDF needs LilyPond (free, lilypond.org)");
                            });
                            ui.end_row();
                            ui.label("");
                            ui.horizontal(|ui| {
                                match &d.lilypond {
                                    Some(p) => {
                                        ui.label(
                                            egui::RichText::new(format!("LilyPond: {}", p.display()))
                                                .small()
                                                .color(t.ink2),
                                        );
                                    }
                                    None => {
                                        ui.label(
                                            egui::RichText::new(
                                                "PDF needs LilyPond, a free program from lilypond.org (not found)",
                                            )
                                            .small()
                                            .color(t.brick),
                                        );
                                    }
                                }
                                if ui
                                    .small_button("Find LilyPond…")
                                    .on_hover_text("choose lilypond.exe or its folder")
                                    .clicked()
                                {
                                    find_ly = true;
                                }
                            });
                            ui.end_row();
                            ui.label("Bars");
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut d.some_bars, false, "Whole song");
                                ui.selectable_value(&mut d.some_bars, true, "Bars");
                                ui.add_enabled_ui(d.some_bars, |ui| {
                                    let n = d.nbars.max(1);
                                    ui.add(egui::DragValue::new(&mut d.opts.bars.0).range(1..=n));
                                    ui.label("to");
                                    let lo = d.opts.bars.0;
                                    ui.add(egui::DragValue::new(&mut d.opts.bars.1).range(lo..=n));
                                });
                                ui.label(
                                    egui::RichText::new(format!("of {}", d.nbars)).color(t.ink2),
                                );
                            });
                            ui.end_row();
                            ui.label("Rhythm");
                            ui.horizontal(|ui| {
                                let cur = GRIDS
                                    .iter()
                                    .find(|g| g.0 == d.opts.grid)
                                    .map_or("1/16", |g| g.1);
                                egui::ComboBox::from_id_salt("notation_grid")
                                    .selected_text(format!("Grid {cur}"))
                                    .show_ui(ui, |ui| {
                                        for (g, l) in GRIDS {
                                            ui.selectable_value(&mut d.opts.grid, g, format!("Grid {l}"));
                                        }
                                    })
                                    .response
                                    .on_hover_text("notes snap to this grid (the shortest note written)");
                                if ui
                                    .checkbox(&mut d.opts.merge, "One staff per instrument")
                                    .on_hover_text("tracks that play the same instrument share a staff of chords")
                                    .changed()
                                {
                                    remerge = true;
                                }
                                ui.checkbox(&mut d.opts.drums, "Include drums")
                                    .on_hover_text("drum tracks on a drum staff");
                            });
                            ui.end_row();
                            ui.label("Key");
                            ui.horizontal(|ui| {
                                use crate::chords::{Key as K, Scale};
                                let shown = match d.opts.key {
                                    KeyChoice::Auto => format!("From the song ({})", d.auto_key),
                                    KeyChoice::Key(k) => k.name(),
                                    KeyChoice::None => "No key signature".into(),
                                };
                                egui::ComboBox::from_id_salt("notation_key")
                                    .selected_text(shown)
                                    .height(300.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut d.opts.key,
                                            KeyChoice::Auto,
                                            format!("From the song ({})", d.auto_key),
                                        );
                                        ui.selectable_value(&mut d.opts.key, KeyChoice::None, "No key signature");
                                        ui.separator();
                                        for scale in [Scale::Major, Scale::Minor] {
                                            for root in 0..12u8 {
                                                let k = K::new(root, scale);
                                                ui.selectable_value(&mut d.opts.key, KeyChoice::Key(k), k.name());
                                            }
                                        }
                                    })
                                    .response
                                    .on_hover_text(
                                        "the key signature: the song's key (Song info, chord helper), \
                                         else the key its notes fit best",
                                    );
                            });
                            ui.end_row();
                            ui.label("Title");
                            ui.add(egui::TextEdit::singleline(&mut d.opts.title).desired_width(420.0));
                            ui.end_row();
                            ui.label("Composer");
                            ui.add(
                                egui::TextEdit::singleline(&mut d.opts.composer)
                                    .desired_width(420.0)
                                    .hint_text("blank = no composer line"),
                            )
                            .on_hover_text("remembered for next time");
                            ui.end_row();
                        });
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Parts").strong());
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(660.0);
                        egui::ScrollArea::vertical()
                            .id_salt("notation_parts")
                            .max_height(210.0)
                            .show(ui, |ui| {
                                if d.opts.parts.is_empty() {
                                    ui.label(egui::RichText::new("No notes in this song yet").color(t.ink2));
                                }
                                egui::Grid::new("notation_parts_g")
                                    .num_columns(3)
                                    .spacing([16.0, 4.0])
                                    .show(ui, |ui| {
                                        for p in d.opts.parts.iter_mut() {
                                            let off = p.drums && !d.opts.drums;
                                            ui.add_enabled(!off, egui::Checkbox::new(&mut p.on, &p.name))
                                                .on_disabled_hover_text("Include drums is off");
                                            let what = if p.drums {
                                                "drum staff"
                                            } else if p.tracks.len() > 1 {
                                                "chords from several tracks"
                                            } else {
                                                "staff (clef from its range)"
                                            };
                                            ui.label(egui::RichText::new(what).color(t.ink2));
                                            let mut note = format!("{} notes", p.notes);
                                            if p.sfx {
                                                note.push_str(" · sound effects");
                                            }
                                            if p.muted {
                                                note.push_str(" · muted");
                                            }
                                            ui.label(egui::RichText::new(note).color(t.ink2));
                                            ui.end_row();
                                        }
                                    });
                            });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Save to");
                        ui.add(egui::TextEdit::singleline(&mut d.path).desired_width(480.0))
                            .on_hover_text("each format adds its own extension");
                        if ui.button("Browse…").clicked() {
                            browse = true;
                        }
                    });
                });
                ui.add_space(6.0);
                match &d.result {
                    _ if running => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Writing the score…");
                        });
                    }
                    Some(Ok(m)) => {
                        ui.label(egui::RichText::new(m).color(t.accent));
                    }
                    Some(Err(e)) => {
                        ui.label(egui::RichText::new(e).color(t.brick));
                    }
                    None => {
                        ui.label(
                            egui::RichText::new(
                                "Notes are tidied to the grid; overlapping notes are cut at the next one.",
                            )
                            .color(t.ink2),
                        );
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!running, |ui| {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("  Export  ").strong())
                                    .fill(t.accent.gamma_multiply(0.6)),
                            )
                            .on_hover_text("Enter")
                            .clicked()
                        {
                            go = true;
                        }
                        if ui.button("Close").on_hover_text("Esc").clicked() {
                            close_btn = true;
                        }
                    });
                });
            });
        if remerge {
            let merge = d.opts.merge;
            let old = d.opts.parts.clone();
            d.opts.parts = self.with(|s| notation::reparts(&s.song, merge, &old));
        }
        if find_ly {
            let mut fd = rfd::FileDialog::new();
            if cfg!(windows) {
                fd = fd.add_filter("LilyPond", &["exe"]);
            }
            if let Some(p) = fd.pick_file() {
                match notation::lily::find(&p.display().to_string()) {
                    Some(exe) if p.is_file() || exe.starts_with(&p) => {
                        self.extra.lilypond_path = exe.display().to_string();
                        d.lilypond = Some(exe);
                        d.pdf = true;
                        d.result = None;
                        self.save_settings();
                    }
                    _ => {
                        d.result = Some(Err(format!("{} is not LilyPond", p.display())));
                    }
                }
            }
        }
        if browse {
            let cur = PathBuf::from(strip_ext(&d.path));
            let mut fd = rfd::FileDialog::new();
            if let Some(dir) = cur.parent() {
                fd = fd.set_directory(dir);
            }
            if let Some(n) = cur.file_name() {
                fd = fd.set_file_name(n.to_string_lossy());
            }
            if let Some(p) = fd.save_file() {
                d.path = strip_ext(&p.display().to_string());
            }
        }
        if (open && !close_btn) || running {
            self.notation_dlg = Some(d);
        }
        if go {
            self.start_notation();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names_share_the_stem() {
        let (x, l, p) = outputs(Path::new("/m/Song"));
        assert_eq!(x, Path::new("/m/Song.musicxml"));
        assert_eq!(l, Path::new("/m/Song.ly"));
        assert_eq!(p, Path::new("/m/Song"));
        assert_eq!(strip_ext("/m/Song.musicxml"), "/m/Song");
        assert_eq!(strip_ext(" /m/Song.PDF "), "/m/Song");
        assert_eq!(strip_ext("/m/Song.v2"), "/m/Song.v2");
    }
}
