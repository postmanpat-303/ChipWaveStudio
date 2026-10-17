//! Song file actions: save (background), open, new, recent files, autosave tick, quick export, sample import.
use super::*;

impl App {
    pub(super) fn save(&mut self, as_new: bool) {
        self.save_inner(as_new, false);
    }

    /// save and block until done (used by "Save" in the quit/new prompt)
    pub(super) fn save_and_wait(&mut self) -> bool {
        self.save_inner(false, true)
    }

    pub(super) fn save_inner(&mut self, as_new: bool, wait: bool) -> bool {
        if as_new || self.path.is_none() {
            let mut d = rfd::FileDialog::new()
                .add_filter("ChipWave song", &["cwt"])
                .set_file_name(format!(
                    "{}.cwt",
                    sanitize(&self.with(|s| s.song.title.clone()))
                ));
            if !self.extra.song_dir.is_empty() {
                d = d.set_directory(&self.extra.song_dir);
            }
            let Some(p) = d.save_file() else {
                return false;
            };
            self.path = Some(if p.extension().is_none() {
                p.with_extension("cwt")
            } else {
                p
            });
        }
        let Some(path) = self.path.clone() else {
            return false;
        };
        self.start_save(path, wait)
    }

    /// Save a snapshot of the song on a worker thread (the audio lock is only
    /// held for the clone). `wait` blocks the UI (not the audio) until done and
    /// returns whether the save succeeded.
    pub(super) fn start_save(&mut self, path: PathBuf, wait: bool) -> bool {
        if let Some(job) = self.save_job.take() {
            // finish the previous save first so saves never overlap
            let r = job
                .1
                .recv()
                .unwrap_or_else(|_| Err("save thread died".into()));
            self.finish_save(job.0, r);
        }
        let nb = self.extra.backups.min(10);
        let song = self.with(|s| s.song.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        let p2 = path.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::recovery::atomic_save(&song, &p2, nb));
        });
        // edits made while saving mark the song dirty again
        let was_dirty = self.dirty;
        self.dirty = false;
        self.save_was_dirty = was_dirty;
        self.status = format!("Saving {}…", path.display());
        if wait {
            let r = rx.recv().unwrap_or_else(|_| Err("save thread died".into()));
            self.finish_save(path, r)
        } else {
            self.save_job = Some((path, rx));
            false
        }
    }

    pub(super) fn load_warnings_window(&mut self, ctx: &egui::Context) {
        if self.load_warnings.is_empty() {
            return;
        }
        let mut ok = false;
        egui::Window::new("Song opened with warnings")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Some parts of this song could not be loaded:");
                for w in &self.load_warnings {
                    ui.label(format!("• {w}"));
                }
                ui.add_space(6.0);
                ui.label("Saving now would drop them. Use Save As to keep the original file.");
                ok = ui.button("OK").clicked();
            });
        if ok || ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) {
            self.load_warnings.clear();
        }
    }

    /// poll the background save (called every frame)
    pub(super) fn poll_save(&mut self) {
        let Some((_, rx)) = &self.save_job else {
            return;
        };
        let r = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(_) => Err("save thread died".into()),
        };
        if let Some((path, _)) = self.save_job.take() {
            self.finish_save(path, r);
        }
    }

    pub(super) fn finish_save(&mut self, path: PathBuf, r: Result<(), String>) -> bool {
        match r {
            Ok(()) => {
                self.add_recent(&path);
                self.status = format!("Saved {}", path.display());
                true
            }
            Err(e) => {
                self.dirty |= self.save_was_dirty;
                self.status = format!("Save failed: {e} (old file kept; try Save As)");
                false
            }
        }
    }

    pub(super) fn new_song(&mut self) {
        let ch = self.with(|s| s.song.channels);
        self.with(|s| {
            s.engine.stop();
            let ins = s.song.instruments.clone();
            s.song = Song::empty();
            s.song.channels = ch;
            s.song.channel_names = (1..=ch).map(|i| format!("Ch {i}")).collect();
            s.song.patterns = vec![Pattern::new("Block 00", ROWS, ch)];
            s.song.instruments = ins;
        });
        self.path = None;
        self.reset_cursor();
        self.status = "New song (instruments kept)".into();
    }

    pub(super) fn add_block(&mut self) {
        let ch = self.with(|s| s.song.channels);
        let n = self.with(|s| {
            s.song.patterns.push(Pattern::new(
                &format!("Block {:02}", s.song.patterns.len()),
                ROWS,
                ch,
            ));
            s.song.order.push(s.song.patterns.len() - 1);
            s.song.patterns.len() - 1
        });
        self.pattern = n;
        self.row = 0;
        self.dirty = true;
        self.status = format!("Added block {n:02}");
    }

    pub(super) fn set_channels(&mut self, ch: usize) {
        let ch = ch.clamp(1, MAX_CHANNELS);
        self.with(|s| {
            s.song.channels = ch;
            s.song.channel_names.resize(ch, "Ch".into());
            for p in &mut s.song.patterns {
                for r in &mut p.cells {
                    r.resize(ch, Cell::default());
                }
            }
        });
        if self.ch >= ch {
            self.ch = ch - 1;
        }
        self.dirty = true;
    }

    pub(super) fn add_recent(&mut self, p: &std::path::Path) {
        let full = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let mut txt = full.display().to_string();
        if let Some(t) = txt.strip_prefix(r"\\?\") {
            txt = t.to_string();
        }
        crate::settings::push_recent(&mut self.recent, &txt);
        self.save_settings();
        let title = self.with(|s| s.song.title.clone());
        if let Some(sess) = &self.session {
            sess.write_info(&title, Some(p));
        }
    }

    /// open a .cwt by path (File > Recent, drag and drop)
    /// replace the song with the built-in demo (unsaved, no file path)
    pub(super) fn load_demo(&mut self) {
        let song = fileio::demo_song();
        let title = song.title.clone();
        self.with(|s| {
            s.engine.stop();
            s.song = song;
        });
        self.path = None;
        self.reset_cursor();
        self.dirty = false;
        self.status = format!("Loaded the demo song \"{title}\" (Save As to keep your changes)");
    }

    pub(super) fn open_path(&mut self, p: PathBuf) {
        match fileio::load_cwt_warn(&p) {
            Ok((song, warnings)) => {
                self.with(|s| {
                    s.engine.stop();
                    s.song = song;
                });
                self.path = Some(p.clone());
                self.reset_cursor();
                self.add_recent(&p);
                self.status = if warnings.is_empty() {
                    format!("Opened {}", p.display())
                } else {
                    format!("Opened {} with {} warning(s)", p.display(), warnings.len())
                };
                self.load_warnings = warnings;
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    /// periodic autosave + session heartbeat (called every frame, cheap)
    pub(super) fn tick_safety(&mut self) {
        let Some(sess) = &self.session else { return };
        if self.last_heartbeat.elapsed() >= crate::recovery::HEARTBEAT_EVERY {
            self.last_heartbeat = std::time::Instant::now();
            sess.heartbeat();
        }
        let every = std::time::Duration::from_secs(self.extra.autosave_mins as u64 * 60);
        if self.extra.autosave_mins > 0 && self.dirty && self.last_autosave.elapsed() >= every {
            let song = self.with(|s| s.song.clone());
            // a still-running autosave means we simply try again next frame
            if sess.autosave(song) {
                self.last_autosave = std::time::Instant::now();
                self.status = "Autosaved (recovery copy)".into();
            }
        }
    }

    pub(super) fn open(&mut self) {
        let mut d = rfd::FileDialog::new().add_filter("ChipWave song", &["cwt"]);
        if !self.extra.song_dir.is_empty() {
            d = d.set_directory(&self.extra.song_dir);
        }
        let Some(p) = d.pick_file() else {
            return;
        };
        self.open_path(p);
    }

    pub(super) fn reset_cursor(&mut self) {
        self.row = 0;
        self.ch = 0;
        self.col = 0;
        self.pattern = 0;
        self.undo_reset();
        self.sel = None;
        self.arr_sel = 0;
        self.arr_multi.clear();
        self.dirty = false;
    }

    /// Quick WAV export: pick a file, then render on the export worker thread
    /// (the export dialog shows progress and can cancel).
    pub(super) fn export_wav(&mut self) {
        if self.export_dlg.as_ref().is_some_and(|d| d.job.is_some()) {
            return;
        }
        let Some(p) = rfd::FileDialog::new()
            .add_filter("WAV audio", &["wav"])
            .set_file_name("chipwave-export.wav")
            .save_file()
        else {
            return;
        };
        self.export_dlg = None;
        self.open_export();
        if let Some(d) = &mut self.export_dlg {
            d.opts.format = crate::export::Format::Wav;
            d.path = p.display().to_string();
        }
        self.start_export();
    }

    pub(super) fn import_sample(&mut self, path: PathBuf) {
        match fileio::import_wav(&path) {
            Ok(inst) => {
                let name = inst.name.clone();
                let n = self.with(|s| {
                    s.song.instruments.push(inst);
                    s.song.instruments.len()
                });
                self.instr = n as u8;
                self.dirty = true;
                self.status = format!("Imported '{name}' into slot {:02X}", n);
            }
            Err(e) => self.status = format!("Import failed: {e}"),
        }
    }
}
