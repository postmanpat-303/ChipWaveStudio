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
        self.capture_plugins();
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

    /// File > New song (Ctrl+N): a clean project. Timeline mode (Settings >
    /// Look > New songs, Del's default) starts with no tracks and no
    /// instruments, only Song / Master; tracker mode with four empty
    /// channels and no instruments. Tempo and metre are the defaults.
    pub(super) fn new_song(&mut self) {
        let tl = self.extra.new_in_timeline || !self.tracker_on();
        self.with(|s| {
            s.engine.stop();
            s.song = if tl { Song::clean() } else { Song::empty() };
            let Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.path = None;
        self.instr = 1;
        self.reset_cursor();
        if tl {
            // Timeline on top, the piano roll below
            self.default_layout();
            self.tlv.start_new_song();
            self.status =
                "New song · drag an instrument from the Browser onto the timeline, or + Add track"
                    .into();
        } else {
            self.status = "New song (no instruments yet: add one from the Browser)".into();
        }
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
            let old = s.song.channels;
            s.song.channels = ch;
            s.song.channel_names.resize(ch, "Ch".into());
            if let Some(t) = s.song.timeline.as_deref_mut() {
                // the Song / Master track (after the channels) stays last
                let tail = t.tracks.split_off(old.min(t.tracks.len()));
                t.tracks.resize_with(ch, Default::default);
                t.tracks.extend(
                    tail.into_iter()
                        .filter(|x| x.kind == crate::timeline::TrackKind::Master),
                );
            }
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

    /// Settings › Open tracker songs in timeline mode (on by default): a
    /// just-loaded tracker-mode song switches to timeline mode with the same
    /// lossless conversion as the "Switch to timeline mode?" dialog. Call it
    /// before `reset_cursor`, so the converted song is the undo base and not
    /// an edit. Screenshots keep the song as it is. True: it was converted.
    pub(super) fn convert_on_open(&mut self) -> bool {
        if self.shot.is_some() {
            return false;
        }
        let on = self.extra.open_in_timeline || !self.tracker_on();
        self.with(|s| {
            let done = convert_on_open(&mut s.song, on);
            if done {
                let Shared { song, engine, .. } = s;
                engine.ensure_plan(song);
            }
            done
        })
    }

    /// replace the song with the built-in demo (unsaved, no file path)
    pub(super) fn load_demo(&mut self, i: usize) {
        let song = fileio::demo_song_n(i);
        let title = song.title.clone();
        self.with(|s| {
            s.engine.stop();
            s.song = song;
        });
        self.path = None;
        let conv = self.convert_on_open();
        self.reset_cursor();
        self.dirty = false;
        if conv {
            self.default_layout();
        }
        self.status = format!("Loaded the demo song \"{title}\" (Save As to keep your changes)");
        if conv {
            self.status = format!("{} · {OPENED_IN_TIMELINE}", self.status);
        }
    }

    /// open a .cwt by path (File > Recent, drag and drop)
    pub(super) fn open_path(&mut self, p: PathBuf) {
        // a single-file text song (demos/*.cwt.toml) opens unsaved: Save asks
        // for a .cwt name instead of overwriting the text file
        let text = fileio::is_text_song(&p);
        let loaded = if text {
            fileio::load_text_song(&p).map(|s| (s, Vec::new()))
        } else {
            fileio::load_cwt_warn(&p)
        };
        match loaded {
            Ok((song, warnings)) => {
                self.with(|s| {
                    s.engine.stop();
                    s.song = song;
                });
                self.path = if text { None } else { Some(p.clone()) };
                let conv = self.convert_on_open();
                self.reset_cursor();
                // timeline-mode songs open in the default layout; tracker-
                // mode songs keep the view (Ctrl+6 is one key away)
                if self.with(|s| s.song.timeline.is_some()) {
                    self.default_layout();
                }
                self.add_recent(&p);
                self.status = if warnings.is_empty() {
                    format!("Opened {}", p.display())
                } else {
                    format!("Opened {} with {} warning(s)", p.display(), warnings.len())
                };
                if conv {
                    self.status = format!("{} · {OPENED_IN_TIMELINE}", self.status);
                }
                self.load_warnings = warnings;
            }
            Err(e) if fileio::is_too_new(&e) => {
                self.status = "Open failed: the song needs a newer ChipWave Studio".into();
                self.too_new = Some(e);
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    /// a song from a newer ChipWave Studio was refused: say why, offer the update
    pub(super) fn too_new_window(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.too_new.clone() else {
            return;
        };
        let (mut ok, mut check) = (false, false);
        egui::Window::new("Song needs a newer ChipWave Studio")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(msg);
                ui.add_space(6.0);
                ui.label("Your current song is unchanged.");
                ui.horizontal(|ui| {
                    check = ui.button("Check for updates").clicked();
                    ok = ui.button("OK").clicked();
                });
            });
        if check {
            self.too_new = None;
            self.check_updates(true);
        }
        if ok || ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) {
            self.too_new = None;
        }
    }

    /// periodic autosave + session heartbeat (called every frame, cheap)
    pub(super) fn tick_safety(&mut self) {
        let every = std::time::Duration::from_secs(self.extra.autosave_mins as u64 * 60);
        if self.session.is_some()
            && self.extra.autosave_mins > 0
            && self.dirty
            && self.last_autosave.elapsed() >= every
        {
            // the recovery copy includes the plugins' current settings
            self.capture_plugins();
        }
        let Some(sess) = &self.session else { return };
        if self.last_heartbeat.elapsed() >= crate::recovery::HEARTBEAT_EVERY {
            self.last_heartbeat = std::time::Instant::now();
            sess.heartbeat();
        }
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
        let mut d = rfd::FileDialog::new()
            .add_filter("ChipWave song", &["cwt"])
            .add_filter("ChipWave text song / demo (.cwt.toml)", &["toml"]);
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
        self.cue = (0, 0);
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

/// the note after a tracker-mode song was opened in timeline mode
pub(crate) const OPENED_IN_TIMELINE: &str =
    "Opened in timeline mode. File > Export tracker-compatible copy for 0.2.4.0 and older.";

/// Switch a just-opened tracker-mode song to timeline mode when `on`
/// (Settings › Open tracker songs in timeline mode); the conversion of the
/// "Switch to timeline mode?" dialog, which plays bit-identically. True:
/// converted (timeline songs and `on` = false are left alone).
pub(crate) fn convert_on_open(song: &mut Song, on: bool) -> bool {
    if !on || song.timeline.is_some() {
        return false;
    }
    crate::timeline::enter_timeline_mode(song);
    true
}
