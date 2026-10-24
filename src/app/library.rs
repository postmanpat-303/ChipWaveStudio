//! Instrument library window (0.2.3.9): the song's instruments, the user
//! library folder (.cwi files, sub-folders = categories) and the factory
//! sounds, with search, audition, add, replace-a-slot and save/import/export.
use super::*;
use crate::instfile::{self, Entry};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Library {
    pub open: bool,
    pub query: String,
    /// "" = all kinds, else "synth" / "sample" / "voice"
    pub kind: &'static str,
    pub user: Vec<Entry>,
    pub factory: Vec<Entry>,
    /// folder `user` was scanned from
    pub scanned: Option<std::path::PathBuf>,
    /// stop the audition preview at this time
    pub audition_until: Option<Instant>,
}

/// what a click in the window asked for (applied after drawing)
enum Act {
    Audition(Box<Instrument>),
    AuditionSlot(usize),
    Add(Entry),
    Replace(Entry),
    Select(u8),
    Move(usize, usize),
    SaveSlot(u8),
    Rescan,
    OpenFolder,
    Import,
    Export(u8),
    Swap(u8, u8, bool),
}

impl App {
    pub(super) fn open_library(&mut self) {
        self.library.open = true;
        self.library.scanned = None; // rescan on open
    }

    fn rescan_library(&mut self) {
        let dir = instfile::library_dir(&self.extra.library_dir);
        self.library.user = dir.as_deref().map(instfile::scan).unwrap_or_default();
        self.library.scanned = dir;
        if self.library.factory.is_empty() {
            self.library.factory = instfile::factory_entries();
        }
    }

    pub(super) fn stop_audition(&mut self) {
        if self.library.audition_until.take().is_some() {
            self.with(|s| {
                if s.preview.is_some() {
                    s.preview = None;
                    s.engine.stop();
                }
            });
        }
    }

    pub(super) fn audition(&mut self, inst: &Instrument) {
        self.with(|s| {
            let song = instfile::audition_song(inst, &s.song);
            s.engine.stop();
            s.engine.start(&song, 0, 0, false, 0);
            s.preview = Some(song);
        });
        self.library.audition_until = Some(Instant::now() + Duration::from_millis(2600));
        self.status = format!("Auditioning {}", inst.name);
    }

    /// add a library / factory instrument as a new slot (undoable)
    pub(super) fn add_library_instrument(&mut self, inst: Instrument) {
        let name = inst.name.clone();
        self.snapshot();
        self.undo_hold += 1;
        match self.with(|s| s.song.add_instrument(inst)) {
            Some(i) => self.adopt_new_instrument((i + 1) as u8, &name),
            None => self.status = "All 255 instrument slots are used".into(),
        }
        self.undo_hold -= 1;
    }

    /// A just-imported / added instrument in slot `slot` becomes THE active
    /// one (Del (postmanpat) 24 Oct 2026: imports didn't switch to it).
    /// Setting `instr` alone was the bug: on the Timeline the current
    /// instrument is the selected track's (`track_instrument`, the one rule
    /// the Rack, typing, the piano roll and the next track click follow),
    /// so the new slot played nowhere, an empty track's Rack stayed empty
    /// and the next click on a track switched back. Like the track pie's
    /// Add and a Browser load: an empty selected track plays it, else a new
    /// track below the selected one does; that track is lit and the Rack
    /// shows its compact card. Outside the Timeline (tracker songs) the
    /// slot is selected for the instrument column, as before. The caller
    /// holds the undo step (snapshot + `undo_hold`).
    pub(super) fn adopt_new_instrument(&mut self, slot: u8, name: &str) {
        self.instr = slot;
        self.dirty = true;
        let tl = self.view == View::Timeline && self.with(|s| s.song.timeline.is_some());
        let nch = self.with(|s| s.song.channels);
        if !tl || (nch >= MAX_CHANNELS && !self.track_needs_instrument(self.ch)) {
            self.status = if tl {
                format!("Added instrument {slot:02X} {name} (all 32 tracks are used: drop it onto a track to play it)")
            } else {
                format!("Added instrument {slot:02X} {name}")
            };
            return;
        }
        use super::browserview::Act;
        let act = if self.tlv.song_sel.is_some() || self.ch >= nch {
            // Song / Master lit: a new track at the end
            Act::NewTrack(nch)
        } else if self.track_needs_instrument(self.ch) {
            Act::Track(self.ch)
        } else {
            Act::NewTrack(self.ch + 1)
        };
        self.br_assign_slot(slot, name, act);
        self.set_lower(4);
    }

    /// replace slot `slot`'s sound, keeping every note that uses it
    pub(super) fn replace_instrument_sound(&mut self, slot: u8, mut inst: Instrument) {
        let idx = (slot as usize).wrapping_sub(1);
        // a Devices drop onto the instrument card keeps the track's effects
        if let Some((s, fx)) = self.dev.keep_fx.take() {
            if s == slot && inst.fx.is_empty() {
                inst.fx = fx;
            }
        }
        self.snapshot();
        let name = inst.name.clone();
        let ok = self.with(|s| match s.song.instruments.get_mut(idx) {
            Some(x) => {
                *x = inst;
                true
            }
            None => false,
        });
        if ok {
            self.dirty = true;
            self.status = format!("Instrument {slot:02X} is now {name} (its notes are kept)");
        }
    }

    pub(super) fn move_instrument(&mut self, from: usize, to: usize) {
        self.snapshot();
        let map = self.with(|s| {
            let m = s.song.move_instrument(from, to)?;
            let m2 = m.clone();
            s.engine.remap_instruments(&move |i| m2.get(i).copied());
            Some(m)
        });
        if let Some(m) = map {
            let cur = (self.instr as usize).wrapping_sub(1);
            if let Some(&n) = m.get(cur) {
                self.instr = (n + 1) as u8;
            }
            self.dirty = true;
            self.status = format!("Moved instrument {:02X} to {:02X}", from + 1, to + 1);
        }
    }

    pub(super) fn save_instrument_to_library(&mut self, slot: u8) {
        self.capture_plugins();
        let Some(inst) = self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .cloned()
        }) else {
            return;
        };
        let Some(dir) = instfile::library_dir(&self.extra.library_dir) else {
            self.status = "No library folder (set one in Settings > Files & Safety)".into();
            return;
        };
        let r = std::fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|_| {
                let p = instfile::free_path(&dir, &inst.name);
                instfile::save_cwi(&inst, &p).map(|_| p)
            });
        self.status = match r {
            Ok(p) => format!("Saved {} to the library: {}", inst.name, p.display()),
            Err(e) => format!("Could not save to the library: {e}"),
        };
        self.library.scanned = None;
    }

    pub(super) fn export_instrument(&mut self, slot: u8) {
        self.capture_plugins();
        let Some(inst) = self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .cloned()
        }) else {
            return;
        };
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("ChipWave instrument", &[instfile::EXT])
            .set_file_name(instfile::file_name(&inst.name))
            .save_file()
        {
            self.status = match instfile::save_cwi(&inst, &p) {
                Ok(()) => format!("Exported {} to {}", inst.name, p.display()),
                Err(e) => format!("Export failed: {e}"),
            };
        }
    }

    /// load a .cwi file as a new instrument (File menu, drag and drop)
    pub(super) fn import_cwi(&mut self, p: &std::path::Path) {
        match instfile::load_cwi(p) {
            Ok(i) => self.add_library_instrument(i),
            Err(e) => self.status = format!("{}: {e}", p.display()),
        }
    }

    pub(super) fn swap_instrument_notes(&mut self, from: u8, to: u8, block_only: bool) {
        self.snapshot();
        let pat = self.pattern;
        let n = self.with(|s| {
            s.song
                .replace_instrument_uses(from, to, block_only.then_some(pat))
        });
        self.dirty |= n > 0;
        self.status = format!(
            "{n} note(s) {} switched from instrument {from:02X} to {to:02X} (Ctrl+Z undoes)",
            if block_only {
                "in this block"
            } else {
                "in the song"
            }
        );
    }

    pub(super) fn library_window(&mut self, ctx: &egui::Context) {
        if self
            .library
            .audition_until
            .is_some_and(|t| Instant::now() >= t)
        {
            self.stop_audition();
        }
        if self.library.audition_until.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if !self.library.open {
            self.stop_audition();
            return;
        }
        if self.library.scanned.is_none() {
            self.rescan_library();
        }
        let song_list: Vec<(String, &'static str, usize)> = self.with(|s| {
            s.song
                .instruments
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    (
                        x.name.clone(),
                        instfile::kind_name(&x.kind),
                        s.song.instrument_uses(i as u8 + 1),
                    )
                })
                .collect()
        });
        let cur = self.instr;
        let mut acts: Vec<Act> = vec![];
        let mut open = true;
        let folder = self
            .library
            .scanned
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(no folder)".into());
        egui::Window::new("Instrument library")
            .open(&mut open)
            .default_width(760.0)
            .default_height(520.0)
            .resizable(true)
            .show(ctx, |ui| {
                let lib = &mut self.library;
                ui.horizontal_wrapped(|ui| {
                    ui.label("Search");
                    ui.add(
                        egui::TextEdit::singleline(&mut lib.query)
                            .hint_text("name, kind or category")
                            .desired_width(200.0),
                    );
                    for (k, n) in [("", "all"), ("synth", "synth"), ("sample", "sample"), ("voice", "voice")] {
                        ui.selectable_value(&mut lib.kind, k, n);
                    }
                    ui.separator();
                    if ui.button("Rescan").clicked() {
                        acts.push(Act::Rescan);
                    }
                    if ui.button("Open folder").on_hover_text(&folder).clicked() {
                        acts.push(Act::OpenFolder);
                    }
                    if ui.button("Import .cwi…").clicked() {
                        acts.push(Act::Import);
                    }
                });
                let q = lib.query.clone();
                let kind = lib.kind;
                let shown = |e: &Entry| (kind.is_empty() || e.kind == kind) && instfile::matches(e, &q);
                let target = if (cur as usize).wrapping_sub(1) < song_list.len() {
                    Some(cur)
                } else {
                    None
                };
                ui.columns(3, |cols| {
                    // ---- in this song
                    cols[0].label(egui::RichText::new("In this song").strong());
                    egui::ScrollArea::vertical().id_salt("lib_song").max_height(380.0).show(&mut cols[0], |ui| {
                        for (i, (n, k, uses)) in song_list.iter().enumerate() {
                            let e = Entry {
                                name: n.clone(),
                                kind: k,
                                category: "song".into(),
                                source: instfile::Source::Builtin(String::new()),
                            };
                            if !shown(&e) {
                                continue;
                            }
                            let slot = (i + 1) as u8;
                            ui.horizontal(|ui| {
                                if ui.small_button("›").on_hover_text("audition").clicked() {
                                    acts.push(Act::AuditionSlot(i));
                                }
                                let r = ui.selectable_label(cur == slot, format!("{slot:02X} {n}"));
                                if r.clicked() {
                                    acts.push(Act::Select(slot));
                                }
                                r.on_hover_text(format!("{k}, used by {uses} note(s)")).context_menu(|ui| {
                                    if ui.button("Move up").clicked() && i > 0 {
                                        acts.push(Act::Move(i, i - 1));
                                        ui.close();
                                    }
                                    if ui.button("Move down").clicked() && i + 1 < song_list.len() {
                                        acts.push(Act::Move(i, i + 1));
                                        ui.close();
                                    }
                                    if ui.button("Save to library").clicked() {
                                        acts.push(Act::SaveSlot(slot));
                                        ui.close();
                                    }
                                    if ui.button("Export .cwi…").clicked() {
                                        acts.push(Act::Export(slot));
                                        ui.close();
                                    }
                                    if let Some(t) = target.filter(|&t| t != slot) {
                                        if ui.button(format!("Use instead of {t:02X} (whole song)")).clicked() {
                                            acts.push(Act::Swap(t, slot, false));
                                            ui.close();
                                        }
                                        if ui.button(format!("Use instead of {t:02X} (this block)")).clicked() {
                                            acts.push(Act::Swap(t, slot, true));
                                            ui.close();
                                        }
                                    }
                                });
                                ui.label(egui::RichText::new(format!("{uses}")).small().color(th().ink2));
                            });
                        }
                    });
                    // ---- library + factory
                    for (col, title, list) in [(1usize, "My library", &lib.user), (2, "Factory", &lib.factory)] {
                        let ui = &mut cols[col];
                        ui.label(egui::RichText::new(title).strong());
                        egui::ScrollArea::vertical().id_salt(("lib", col)).max_height(380.0).show(ui, |ui| {
                            if list.is_empty() {
                                ui.label(
                                    egui::RichText::new("empty: right-click a song instrument > Save to library")
                                        .small()
                                        .color(th().ink2),
                                );
                            }
                            let mut last_cat = None;
                            for e in list.iter().filter(|e| shown(e)) {
                                if last_cat != Some(&e.category) && !e.category.is_empty() {
                                    ui.label(egui::RichText::new(&e.category).small().color(th().ink2));
                                }
                                last_cat = Some(&e.category);
                                ui.horizontal(|ui| {
                                    if ui.small_button("›").on_hover_text("audition").clicked() {
                                        if let Ok(i) = instfile::instantiate(e) {
                                            acts.push(Act::Audition(Box::new(i)));
                                        }
                                    }
                                    if ui.small_button("+").on_hover_text("add to the song as a new instrument").clicked() {
                                        acts.push(Act::Add(e.clone()));
                                    }
                                    let r = ui.label(format!("{} ({})", e.name, e.kind));
                                    if let Some(t) = target {
                                        r.context_menu(|ui| {
                                            if ui
                                                .button(format!("Replace sound of {t:02X} (keep its notes)"))
                                                .clicked()
                                            {
                                                acts.push(Act::Replace(e.clone()));
                                                ui.close();
                                            }
                                        });
                                    }
                                });
                            }
                        });
                    }
                });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if let Some(t) = target {
                        let n = &song_list[t as usize - 1].0;
                        if ui.button(format!("Save {t:02X} {n} to library")).clicked() {
                            acts.push(Act::SaveSlot(t));
                        }
                        if ui.button("Export .cwi…").clicked() {
                            acts.push(Act::Export(t));
                        }
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "› audition · + add · right-click: replace / move / swap · folder: {folder}"
                        ))
                        .small()
                        .color(th().ink2),
                    );
                });
            });
        self.library.open = open;
        for a in acts {
            match a {
                Act::Audition(i) => self.audition(&i),
                Act::AuditionSlot(i) => {
                    if let Some(x) = self.with(|s| s.song.instruments.get(i).cloned()) {
                        self.audition(&x);
                    }
                }
                Act::Add(e) => match instfile::instantiate(&e) {
                    Ok(i) => self.add_library_instrument(i),
                    Err(err) => self.status = format!("{}: {err}", e.name),
                },
                Act::Replace(e) => match instfile::instantiate(&e) {
                    Ok(i) => self.replace_instrument_sound(cur, i),
                    Err(err) => self.status = format!("{}: {err}", e.name),
                },
                Act::Select(s) => self.instr = s,
                Act::Move(a, b) => self.move_instrument(a, b),
                Act::SaveSlot(s) => self.save_instrument_to_library(s),
                Act::Rescan => self.library.scanned = None,
                Act::OpenFolder => {
                    if let Some(p) = instfile::library_dir(&self.extra.library_dir) {
                        let _ = std::fs::create_dir_all(&p);
                        super::settingsview::open_folder(&p);
                    }
                }
                Act::Import => {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("ChipWave instrument", &[instfile::EXT])
                        .pick_file()
                    {
                        self.import_cwi(&p);
                    }
                }
                Act::Export(s) => self.export_instrument(s),
                Act::Swap(from, to, b) => self.swap_instrument_notes(from, to, b),
            }
        }
    }
}
