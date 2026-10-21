//! Save Custom Rack (mockup 4) and the rack loading the Browser uses
//! (docs/racks/PLAN.md Stage 5). The file side is `crate::racks`.
use super::*;
use crate::racks::{self, RackInfo};

/// the Save Custom Rack dialog's fields
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SaveRack {
    /// the instrument saved (0-based)
    pub(crate) inst: usize,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) sound_type: String,
    /// comma separated
    pub(crate) tags: String,
    pub(crate) focus: bool,
}

/// sound types offered (free text is fine too)
pub(crate) const SOUND_TYPES: [&str; 10] = [
    "Keys", "Pad", "Lead", "Bass", "Pluck", "Drums", "FX", "Strings", "Brass", "Other",
];

impl App {
    /// open Save Custom Rack for instrument `idx`
    pub(crate) fn open_save_rack(&mut self, idx: usize) {
        let Some((name, tag)) = self.with(|s| {
            s.song
                .instruments
                .get(idx)
                .map(|i| (i.name.clone(), params::type_tag(&i.kind).to_string()))
        }) else {
            return;
        };
        if tag == "CLAP" || tag == "VST3" {
            self.status =
                "Plugin instruments can't be saved as a rack yet (they can't play as a layer)"
                    .into();
            return;
        }
        self.dev.save_rack = Some(SaveRack {
            inst: idx,
            name,
            focus: true,
            ..Default::default()
        });
    }

    /// Save (or Update when `update` is the existing file); returns the path
    pub(crate) fn save_rack_now(
        &mut self,
        f: &SaveRack,
        update: Option<std::path::PathBuf>,
    ) -> Result<std::path::PathBuf, String> {
        let dir = racks::dir().ok_or("there's no settings folder to keep racks in")?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("the Racks folder can't be made ({e})"))?;
        // plugin states as they are now
        self.plug.state_dirty = true;
        self.capture_plugins();
        let info = RackInfo {
            name: f.name.clone(),
            description: f.description.trim().to_string(),
            sound_type: f.sound_type.trim().to_string(),
            tags: f
                .tags
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .take(16)
                .collect(),
            ..Default::default()
        };
        let rack = self.with(|s| racks::from_song(&s.song, f.inst, info))?;
        let path = match update {
            Some(p) => {
                racks::update(&p, rack)?;
                p
            }
            None => {
                let p = racks::free_path(&dir, &f.name);
                racks::save(&rack, &p)?;
                p
            }
        };
        self.dev.racks_dirty = true;
        Ok(path)
    }

    /// the dialog, when open
    pub(crate) fn save_rack_window(&mut self, ctx: &egui::Context) {
        let Some(mut f) = self.dev.save_rack.clone() else {
            return;
        };
        let t = th();
        let summary = self.with(|s| save_summary(&s.song, f.inst));
        let clash = racks::dir()
            .map(|d| d.join(racks::file_name(&f.name)))
            .filter(|p| p.exists());
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut save: Option<Option<std::path::PathBuf>> = None;
        let mut desc_focus = false;
        let screen = ctx.content_rect();
        let w = (screen.width() - 80.0).clamp(380.0, 560.0);
        egui::Window::new("Save Custom Rack")
            .id(egui::Id::unique("save_custom_rack"))
            .collapsible(false)
            .resizable(false)
            .default_width(w)
            .max_height(screen.height() - 60.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(w);
                egui::Grid::new("save_rack_grid")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Name");
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut f.name)
                                .char_limit(64)
                                .desired_width(w - 120.0),
                        );
                        if f.focus {
                            r.request_focus();
                            f.focus = false;
                        }
                        ui.end_row();
                        ui.label("Description");
                        desc_focus = ui
                            .add(
                                egui::TextEdit::multiline(&mut f.description)
                                    .desired_rows(2)
                                    .char_limit(2000)
                                    .hint_text("what it sounds like, how to play it")
                                    .desired_width(w - 120.0),
                            )
                            .has_focus();
                        ui.end_row();
                        ui.label("Sound type");
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("save_rack_type")
                                .selected_text(if f.sound_type.is_empty() {
                                    "choose…"
                                } else {
                                    f.sound_type.as_str()
                                })
                                .show_ui(ui, |ui| {
                                    for s in SOUND_TYPES {
                                        ui.selectable_value(&mut f.sound_type, s.to_string(), s);
                                    }
                                });
                        });
                        ui.end_row();
                        ui.label("Tags");
                        ui.add(
                            egui::TextEdit::singleline(&mut f.tags)
                                .hint_text("bright, glassy, wide (commas between)")
                                .desired_width(w - 120.0),
                        );
                        ui.end_row();
                    });
                ui.add_space(6.0);
                ui.label(egui::RichText::new("WHAT IS SAVED").small().strong().color(t.ink2));
                for line in &summary {
                    ui.label(egui::RichText::new(line).small());
                }
                if let Some(d) = racks::dir() {
                    ui.label(
                        egui::RichText::new(format!("in {}", d.display()))
                            .small()
                            .color(t.ink2),
                    );
                }
                ui.separator();
                let named = !f.name.trim().is_empty();
                if let Some(p) = &clash {
                    ui.label(
                        egui::RichText::new(format!(
                            "A rack called \"{}\" is already in My Racks. Update it (the old one is kept as a backup) or save this as a new rack.",
                            f.name.trim()
                        ))
                        .color(t.brick),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("Update existing")
                                .on_hover_text("replace its sound; name, description and tags stay; a backup goes to Racks/.backup")
                                .clicked()
                            {
                                save = Some(Some(p.clone()));
                            }
                            if ui
                                .add_enabled(named, egui::Button::new("Save as new"))
                                .on_hover_text("a second rack with a number after the name")
                                .clicked()
                            {
                                save = Some(None);
                            }
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Enter saves, except in the description (a new line)
                            let enter =
                                ui.input(|i| i.key_pressed(egui::Key::Enter)) && !desc_focus;
                            if ui
                                .add_enabled(named, egui::Button::new("Save rack"))
                                .on_disabled_hover_text("give it a name first")
                                .clicked()
                                || (named && enter)
                            {
                                save = Some(None);
                            }
                        });
                    });
                }
            });
        if let Some(target) = save {
            match self.save_rack_now(&f, target.clone()) {
                Ok(p) => {
                    self.status = format!(
                        "{} \"{}\" in My Racks ({})",
                        if target.is_some() { "Updated" } else { "Saved" },
                        f.name.trim(),
                        p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    );
                    self.dev.save_rack = None;
                }
                Err(e) => {
                    self.status = format!("Couldn't save the rack: {e}");
                    self.dev.save_rack = Some(f);
                }
            }
            return;
        }
        self.dev.save_rack = if close { None } else { Some(f) };
    }

    /// Load the rack at `path` (D9): new slots, given to track `track`
    /// (None = only added), one undo step. Returns the rack's slot.
    pub(crate) fn load_rack(
        &mut self,
        path: &std::path::Path,
        track: Option<usize>,
    ) -> Option<usize> {
        let rack = match racks::load(path) {
            Ok(r) => r,
            Err(e) => {
                self.status = e;
                return None;
            }
        };
        self.snapshot();
        let r = self.with(|s| racks::load_into(&mut s.song, &rack, track));
        match r {
            Ok(slot) => {
                self.dirty = true;
                self.instr = (slot + 1) as u8;
                if let Some(tr) = track {
                    self.ch = tr;
                }
                let plugins = rack
                    .instruments
                    .iter()
                    .any(|i| i.fx.iter().any(|d| d.plugin.is_some()));
                if plugins {
                    let extra = Some((slot, self.ch));
                    crate::plugin::sync(&self.shared, extra);
                }
                self.status = match track {
                    Some(tr) => format!(
                        "Track {:02} now plays the rack \"{}\" ({:02X})",
                        tr + 1,
                        rack.info.name,
                        slot + 1
                    ),
                    None => format!(
                        "Added the rack \"{}\" as instrument {:02X}",
                        rack.info.name,
                        slot + 1
                    ),
                };
                Some(slot)
            }
            Err(e) => {
                self.status = format!("Couldn't load the rack: {e}");
                None
            }
        }
    }
}

/// "Rack: Glass Keys · 2 layers", "Layer A: 02 Saw (Synth) + 1 effect", ...
pub(crate) fn save_summary(song: &Song, idx: usize) -> Vec<String> {
    let Some(inst) = song.instruments.get(idx) else {
        return vec![];
    };
    let fx = |n: usize| match n {
        0 => String::new(),
        1 => " + 1 effect".into(),
        n => format!(" + {n} effects"),
    };
    let mut v = vec![];
    match &inst.kind {
        InstrumentKind::Rack(r) => {
            v.push(format!(
                "The rack {:02X} {} with {} layer{}{}",
                idx + 1,
                inst.name,
                r.layers.len(),
                if r.layers.len() == 1 { "" } else { "s" },
                fx(inst.fx.len())
            ));
            for (k, l) in r.layers.iter().enumerate() {
                if let Some(x) = song.instruments.get(l.slot) {
                    v.push(format!(
                        "  Layer {}: {:02X} {} ({}){}",
                        super::rackops::layer_letter(k),
                        l.slot + 1,
                        x.name,
                        params::type_tag(&x.kind),
                        fx(x.fx.len())
                    ));
                }
            }
        }
        _ => {
            v.push(format!(
                "{:02X} {} ({}) as a 1-layer rack{}",
                idx + 1,
                inst.name,
                params::type_tag(&inst.kind),
                fx(inst.fx.len())
            ));
        }
    }
    let refs = match crate::racks::from_song(song, idx, RackInfo::default()) {
        Ok(r) => r.plugins,
        Err(_) => vec![],
    };
    if !refs.is_empty() {
        let names: Vec<String> = refs.iter().map(|p| p.name.clone()).collect();
        v.push(format!("Plugins it needs: {}", names.join(", ")));
    }
    v.push("Every setting, the layer levels, pans and key ranges, and the effect chains with their Splits".into());
    v
}

/// My Racks state that is not part of the song
#[derive(Default)]
pub(crate) struct MyRacks {
    pub(crate) list: Vec<racks::Entry>,
    pub(crate) sel: Option<std::path::PathBuf>,
    /// renaming this file: the new name
    pub(crate) rename: Option<(std::path::PathBuf, String)>,
    /// editing description, sound type and tags
    pub(crate) edit: Option<(std::path::PathBuf, String, String, String)>,
    /// asked: move this one to Deleted?
    pub(crate) confirm_delete: Option<std::path::PathBuf>,
    /// a rack that needs plugins this computer doesn't have, and the track
    /// it was going to (Some(None) = as a new instrument)
    pub(crate) missing: Option<(std::path::PathBuf, Option<usize>, Vec<String>)>,
}

/// the plugins of `e` that aren't installed here ("Example Verb (CLAP)")
pub(crate) fn missing_plugins(e: &racks::Entry) -> Vec<String> {
    e.plugins
        .iter()
        .filter(|p| {
            let pp = crate::plugin::PluginPatch {
                format: p.format.clone(),
                id: p.id.clone(),
                name: p.name.clone(),
                vendor: p.vendor.clone(),
                path: String::new(),
                state: String::new(),
                params: vec![],
                macros: vec![],
            };
            crate::plugin::scan::resolve(&pp).is_none()
        })
        .map(|p| format!("{} ({})", p.name, p.format.to_uppercase()))
        .collect()
}

/// the preview song: the rack's short phrase on its own
pub(crate) fn preview_song(rack: &racks::Rack, base: &Song, ch: usize) -> Song {
    let mut s = crate::browser::preview_song(&rack.instruments[0], base, ch);
    s.instruments.truncate(1);
    s.instruments.extend(rack.instruments[1..].iter().cloned());
    s
}

impl App {
    fn racks_refresh(&mut self) {
        if !self.dev.racks_dirty {
            return;
        }
        self.dev.racks_dirty = false;
        self.dev.my_racks.list = racks::dir().map(|d| racks::scan(&d)).unwrap_or_default();
    }

    /// load with the missing-plugin check (`track` None = a new instrument)
    pub(crate) fn rack_load_checked(&mut self, path: &std::path::Path, track: Option<usize>) {
        let e = self
            .dev
            .my_racks
            .list
            .iter()
            .find(|e| e.path == path)
            .cloned();
        let missing = e.as_ref().map(missing_plugins).unwrap_or_default();
        if !missing.is_empty() {
            self.dev.my_racks.missing = Some((path.to_path_buf(), track, missing));
            return;
        }
        self.load_rack(path, track);
    }

    fn rack_preview(&mut self, path: &std::path::Path) {
        let rack = match racks::load(path) {
            Ok(r) => r,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        self.pat_stop();
        let ch = self.ch;
        self.with(|s| {
            let song = preview_song(&rack, &s.song, ch);
            s.engine.stop();
            s.engine.start(&song, 0, 0, false, 0);
            s.preview = Some(song);
        });
        self.br.preview_until =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(2400));
        self.status = format!("Previewing the rack \"{}\"", rack.info.name);
    }

    /// Browser › My Racks: search, preview, load, rename, duplicate,
    /// update, edit, show in folder, delete (mockup 5)
    pub(crate) fn racks_body(&mut self, ui: &mut egui::Ui) {
        let t = th();
        self.racks_refresh();
        let Some(dir) = racks::dir() else {
            ui.label(
                egui::RichText::new("There's no settings folder to keep racks in.").color(t.ink2),
            );
            return;
        };
        let q = self.br.filter.query.clone();
        let list: Vec<racks::Entry> = self
            .dev
            .my_racks
            .list
            .iter()
            .filter(|e| racks::matches(e, &q))
            .cloned()
            .collect();
        let ch = self.ch;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("MY RACKS · {}", self.dev.my_racks.list.len()))
                    .small()
                    .strong()
                    .color(t.ink2),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Show folder")
                    .on_hover_text(dir.display().to_string())
                    .clicked()
                {
                    let _ = std::fs::create_dir_all(&dir);
                    super::settingsview::open_folder(&dir);
                }
                if ui.small_button("Refresh").clicked() {
                    self.dev.racks_dirty = true;
                }
            });
        });
        if self.dev.my_racks.list.is_empty() {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    "No racks yet. In Nested Device Chains, Save Custom Rack… keeps a rack (or any instrument) here with every setting.",
                )
                .color(t.ink2),
            );
            return;
        }
        if list.is_empty() {
            ui.label(egui::RichText::new("nothing matches the search").color(t.ink2));
        }
        let mut act: Option<(std::path::PathBuf, &'static str)> = None;
        egui::ScrollArea::vertical()
            .id_salt("my_racks")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for e in &list {
                    let on = self.dev.my_racks.sel.as_ref() == Some(&e.path);
                    let missing = missing_plugins(e);
                    // the row senses clicks first, so its buttons stay on top
                    let fr = ui.scope_builder(
                        egui::UiBuilder::new()
                            .id_salt(("rack_row", &e.path))
                            .sense(egui::Sense::click()),
                        |ui| {
                            ui.style_mut().interaction.selectable_labels = false;
                            egui::Frame::new()
                                .fill(if on {
                                    t.accent.gamma_multiply(0.16)
                                } else {
                                    t.ink2.gamma_multiply(0.05)
                                })
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(8, 5))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    if let Some((rp, txt)) = &mut self.dev.my_racks.rename {
                                        if *rp == e.path {
                                            let r = ui.add(
                                                egui::TextEdit::singleline(txt).char_limit(64),
                                            );
                                            r.request_focus();
                                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                act = Some((e.path.clone(), "rename_ok"));
                                            }
                                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                                act = Some((e.path.clone(), "rename_cancel"));
                                            }
                                            return;
                                        }
                                    }
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(&e.info.name).strong());
                                        let mut bits = vec![format!(
                                            "{} layer{}",
                                            e.layers,
                                            if e.layers == 1 { "" } else { "s" }
                                        )];
                                        if !e.info.sound_type.is_empty() {
                                            bits.push(e.info.sound_type.clone());
                                        }
                                        if !e.info.tags.is_empty() {
                                            bits.push(e.info.tags.join(", "));
                                        }
                                        ui.label(
                                            egui::RichText::new(bits.join(" · "))
                                                .small()
                                                .color(t.ink2),
                                        );
                                    });
                                    if let Some(err) = &e.error {
                                        ui.label(egui::RichText::new(err).small().color(t.brick));
                                    } else if !e.info.description.is_empty() {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&e.info.description).small(),
                                            )
                                            .truncate(),
                                        );
                                    }
                                    if !missing.is_empty() {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Needs plugins not installed here: {}",
                                                missing.join(", ")
                                            ))
                                            .small()
                                            .color(t.brick),
                                        );
                                    }
                                    if on && e.error.is_none() {
                                        ui.horizontal_wrapped(|ui| {
                                            if ui
                                                .button(format!("Load onto track {:02}", ch + 1))
                                                .clicked()
                                            {
                                                act = Some((e.path.clone(), "track"));
                                            }
                                            if ui.button("Preview").clicked() {
                                                act = Some((e.path.clone(), "preview"));
                                            }
                                            if ui.button("Add as new instrument").clicked() {
                                                act = Some((e.path.clone(), "new"));
                                            }
                                        });
                                    }
                                });
                        },
                    );
                    let r = fr.response;
                    r.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::ListBoxOption,
                            true,
                            on,
                            &e.info.name,
                        )
                    });
                    if r.clicked() {
                        self.dev.my_racks.sel = Some(e.path.clone());
                    }
                    if r.double_clicked() && e.error.is_none() {
                        act = Some((e.path.clone(), "track"));
                    }
                    r.context_menu(|ui| {
                        let ok = e.error.is_none();
                        for (label, what, enabled) in [
                            (format!("Load onto track {:02}", ch + 1), "track", ok),
                            ("Add as new instrument".to_string(), "new", ok),
                            ("Preview".to_string(), "preview", ok),
                            ("Rename".to_string(), "rename", ok),
                            ("Duplicate".to_string(), "duplicate", ok),
                            ("Update from track's sound".to_string(), "update", ok),
                            ("Edit description and tags…".to_string(), "edit", ok),
                            ("Show in folder".to_string(), "folder", true),
                            ("Delete…".to_string(), "delete", true),
                        ] {
                            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                                act = Some((e.path.clone(), what));
                                ui.close();
                            }
                        }
                    });
                    ui.add_space(3.0);
                }
            });
        if let Some((p, what)) = act {
            self.rack_action(&p, what);
        }
        self.racks_dialogs(ui.ctx());
    }

    /// a My Racks action on the file at `p`
    pub(crate) fn rack_action(&mut self, p: &std::path::Path, what: &str) {
        let name = self
            .dev
            .my_racks
            .list
            .iter()
            .find(|e| e.path == p)
            .map(|e| e.info.name.clone())
            .unwrap_or_default();
        let done = |me: &mut App, r: Result<String, String>| {
            me.status = match r {
                Ok(s) => s,
                Err(e) => format!("\"{name}\": {e}"),
            };
            me.dev.racks_dirty = true;
        };
        match what {
            "track" => {
                let ch = self.ch;
                let n = self.with(|s| s.song.channels);
                self.rack_load_checked(p, (ch < n).then_some(ch));
            }
            "new" => self.rack_load_checked(p, None),
            "preview" => self.rack_preview(p),
            "rename" => self.dev.my_racks.rename = Some((p.to_path_buf(), name.clone())),
            "rename_cancel" => self.dev.my_racks.rename = None,
            "rename_ok" => {
                let new = self
                    .dev
                    .my_racks
                    .rename
                    .take()
                    .map(|x| x.1)
                    .unwrap_or_default();
                let r = racks::rename(p, &new).map(|np| {
                    self.dev.my_racks.sel = Some(np);
                    format!("Renamed \"{name}\" to \"{}\"", new.trim())
                });
                done(self, r);
            }
            "duplicate" => {
                let r = racks::duplicate(p).map(|np| {
                    self.dev.my_racks.sel = Some(np);
                    format!("Duplicated \"{name}\"")
                });
                done(self, r);
            }
            "update" => {
                let cur = (self.instr as usize).wrapping_sub(1);
                let root = self.with(|s| s.song.rack_of(cur)).unwrap_or(cur);
                self.plug.state_dirty = true;
                self.capture_plugins();
                let r = self
                    .with(|s| racks::from_song(&s.song, root, RackInfo::default()))
                    .and_then(|rack| racks::update(p, rack))
                    .map(|_| format!("Updated \"{name}\" from the current sound (the old one is in Racks/.backup)"));
                done(self, r);
            }
            "edit" => {
                if let Some(e) = self.dev.my_racks.list.iter().find(|e| e.path == p) {
                    self.dev.my_racks.edit = Some((
                        p.to_path_buf(),
                        e.info.description.clone(),
                        e.info.sound_type.clone(),
                        e.info.tags.join(", "),
                    ));
                }
            }
            "folder" => {
                if let Some(d) = p.parent() {
                    super::settingsview::open_folder(d);
                }
            }
            "delete" => self.dev.my_racks.confirm_delete = Some(p.to_path_buf()),
            "delete_ok" => {
                self.dev.my_racks.confirm_delete = None;
                let r = racks::delete(p).map(|_| {
                    format!("Moved \"{name}\" to Racks/Deleted (you can get it back from there)")
                });
                if self.dev.my_racks.sel.as_deref() == Some(p) {
                    self.dev.my_racks.sel = None;
                }
                done(self, r);
            }
            _ => {}
        }
    }

    /// Delete? / missing plugins / edit description windows
    fn racks_dialogs(&mut self, ctx: &egui::Context) {
        let t = th();
        if let Some(p) = self.dev.my_racks.confirm_delete.clone() {
            let name = p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut go = None;
            egui::Window::new("Delete rack")
                .id(egui::Id::unique("rack_delete"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Move \"{name}\" to Racks/Deleted? It leaves My Racks; you can get it back from that folder."
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            go = Some(false);
                        }
                        if ui.button("Delete").clicked() {
                            go = Some(true);
                        }
                    });
                });
            match go {
                Some(true) => self.rack_action(&p, "delete_ok"),
                Some(false) => self.dev.my_racks.confirm_delete = None,
                None => {}
            }
        }
        if let Some((p, track, missing)) = self.dev.my_racks.missing.clone() {
            let mut go = None;
            egui::Window::new("Plugins missing")
                .id(egui::Id::unique("rack_missing"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("This rack needs plugins that aren't installed here:").strong());
                    for m in &missing {
                        ui.label(egui::RichText::new(format!("· {m}")).color(t.brick));
                    }
                    ui.label(
                        egui::RichText::new(
                            "Load anyway: those effects are bypassed and keep their settings, so the rack sounds right once the plugin is installed.",
                        )
                        .small()
                        .color(t.ink2),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            go = Some(0);
                        }
                        if ui.button("Find plugins…").clicked() {
                            go = Some(1);
                        }
                        if ui.button("Load anyway").clicked() {
                            go = Some(2);
                        }
                    });
                });
            match go {
                Some(0) => self.dev.my_racks.missing = None,
                Some(1) => {
                    self.dev.my_racks.missing = None;
                    self.open_plugin_setup(false);
                }
                Some(2) => {
                    self.dev.my_racks.missing = None;
                    self.load_rack(&p, track);
                }
                _ => {}
            }
        }
        if let Some((p, mut desc, mut sound, mut tags)) = self.dev.my_racks.edit.clone() {
            let mut go = None;
            egui::Window::new("Edit description and tags")
                .id(egui::Id::unique("rack_edit"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label("Description");
                    ui.add(
                        egui::TextEdit::multiline(&mut desc)
                            .desired_rows(3)
                            .char_limit(2000),
                    );
                    ui.horizontal(|ui| {
                        ui.label("Sound type");
                        egui::ComboBox::from_id_salt("rack_edit_type")
                            .selected_text(if sound.is_empty() {
                                "choose…"
                            } else {
                                sound.as_str()
                            })
                            .show_ui(ui, |ui| {
                                for s in SOUND_TYPES {
                                    ui.selectable_value(&mut sound, s.to_string(), s);
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Tags");
                        ui.add(egui::TextEdit::singleline(&mut tags).hint_text("commas between"));
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            go = Some(false);
                        }
                        if ui.button("Save").clicked() {
                            go = Some(true);
                        }
                    });
                });
            match go {
                Some(true) => {
                    self.dev.my_racks.edit = None;
                    let r = racks::load(&p).and_then(|mut r| {
                        r.info.description = desc.trim().to_string();
                        r.info.sound_type = sound.trim().to_string();
                        r.info.tags = tags
                            .split(',')
                            .map(|x| x.trim().to_string())
                            .filter(|x| !x.is_empty())
                            .collect();
                        r.info.updated = racks::today();
                        racks::save(&r, &p)
                    });
                    self.status = match r {
                        Ok(()) => "Saved the description and tags".into(),
                        Err(e) => format!("Couldn't save: {e}"),
                    };
                    self.dev.racks_dirty = true;
                }
                Some(false) => self.dev.my_racks.edit = None,
                None => self.dev.my_racks.edit = Some((p, desc, sound, tags)),
            }
        }
    }
}
