//! Plugin instruments in the app (0.2.3.16): the "Plugin instrument" browser,
//! the plugin panel (Configure / parameters / macros), Settings > Plugins and
//! the per-frame housekeeping that keeps the engine's plugin rack in step
//! with the song. Hosting itself lives in src/plugin/ (CLAP via clap-sys, VST3 via the vst3 crate).
use super::*;
use crate::plugin::{self, scan, ParamInfo, PluginPatch, NO_PARAM};
use std::sync::mpsc::Receiver;

/// background scan: progress (done, total, current file) and the result
pub(super) struct ScanJob {
    progress: Arc<Mutex<(usize, usize, String)>>,
    rx: Receiver<scan::Cache>,
}

#[derive(Default)]
pub(super) struct PluginUi {
    /// the "Add plugin instrument" window is open
    pub browser: bool,
    pub query: String,
    pub scan: Option<ScanJob>,
    pub param_query: String,
    /// parameter list of the shown instrument: (instrument, list)
    params: Option<(usize, Vec<ParamInfo>)>,
    pub tick: u32,
    /// frame counter of the StringWave instance sync
    pub sw_tick: u32,
    /// instances changed their state since the last capture
    pub state_dirty: bool,
    /// new folder typed in Settings > Plugins
    pub new_dir: String,
    pub scan_msg: String,
}

impl App {
    /// Every frame: load / drop plugin instances to match the song, run the
    /// plugins' main-thread callbacks, pick up finished scans.
    pub(super) fn plugin_frame(&mut self, ctx: &egui::Context) {
        if let Some(job) = &self.plug.scan {
            if let Ok(c) = job.rx.try_recv() {
                let n = c.instruments().len();
                let nfx = c.effects().len();
                let bad = c.entries.iter().filter(|e| e.status != "ok").count();
                scan::set_cache(c);
                self.plug.scan = None;
                self.plug.scan_msg = format!(
                    "Scan done: {n} plugin instrument(s), {} effect(s){}",
                    nfx,
                    if bad > 0 {
                        format!(", {bad} file(s) skipped (see the list)")
                    } else {
                        String::new()
                    }
                );
                self.status = self.plug.scan_msg.clone();
                // a newly found plugin may fix a missing instrument
                self.with(|s| {
                    s.engine.plugins.errors.clear();
                    s.engine.fx_plugins.retry();
                });
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        // StringWave: create / free its engines to match the song (the
        // audio thread creates a missing one itself, this keeps it off there)
        if self.with(|s| crate::strings::rack::song_has(&s.song) || !s.engine.strings.is_empty()) {
            self.plug.sw_tick = self.plug.sw_tick.wrapping_add(1);
            if self.plug.sw_tick % 15 == 1 {
                let extra = (self.instr as usize).checked_sub(1).map(|i| (i, self.ch));
                crate::strings::rack::sync(&self.shared, extra);
            }
        }
        let has_plugins = self.with(|s| {
            s.song
                .instruments
                .iter()
                .any(|i| matches!(i.kind, InstrumentKind::Plugin(_)))
                || !s.engine.plugins.is_empty()
        });
        let has_fx = self.with(|s| {
            crate::plugin::fxrack::song_has_fx(&s.song) || !s.engine.fx_plugins.is_empty()
        });
        if !has_plugins && !has_fx {
            return;
        }
        self.plug.tick = self.plug.tick.wrapping_add(1);
        if has_fx {
            self.fx_plugin_frame();
        }
        if !has_plugins {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        }
        if self.plug.tick % 15 == 1 {
            let extra = (self.instr as usize).checked_sub(1).map(|i| (i, self.ch));
            plugin::sync(&self.shared, extra);
            self.default_macros();
        }
        let polled = self.with(|s| s.engine.plugins.poll_main());
        if !polled.dirty.is_empty() {
            // a plugin changed itself (its editor): the song needs saving;
            // the state is copied into the song before save / export
            self.plug.state_dirty = true;
            self.dirty = true;
        }
        self.plugin_edits(&polled.events);
        if !polled.closed.is_empty() {
            // an editor closed: keep its edits in the song now and give the
            // other instances its full state (not only parameters)
            self.plug.state_dirty = true;
            self.capture_plugins();
        }
        // editors resize / close through main-thread callbacks
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }

    /// parameter edits made in plugin editors: a knob mapped to a macro
    /// counts as touching that macro (last-touched / follow-touch lanes)
    pub(super) fn plugin_edits(&mut self, events: &[(usize, plugin::OutEv)]) {
        use crate::automation::Target;
        let mut touched: Vec<Target> = vec![];
        self.with(|s| {
            for &(i, e) in events {
                let (plugin::OutEv::Begin(id) | plugin::OutEv::Value(id, _)) = e else {
                    continue;
                };
                let Some(InstrumentKind::Plugin(p)) = s.song.instruments.get(i).map(|x| &x.kind)
                else {
                    continue;
                };
                if let Some(k) = (0..8).find(|&k| id != NO_PARAM && p.macro_id(k) == id) {
                    let t = Target::Macro(i, k);
                    if !touched.contains(&t) {
                        touched.push(t);
                    }
                }
            }
        });
        for t in touched {
            self.touch_param(t);
        }
    }

    /// a fresh plugin instrument maps its first 8 parameters to macros 1-8
    pub(super) fn default_macros(&mut self) {
        self.with(|s| {
            let crate::audio::Shared { song, engine, .. } = s;
            let mut changed = false;
            for (i, inst) in song.instruments.iter_mut().enumerate() {
                let InstrumentKind::Plugin(p) = &mut inst.kind else {
                    continue;
                };
                if !p.macros.is_empty() {
                    continue;
                }
                let Some(si) = engine.plugins.primary(i) else {
                    continue;
                };
                let ids: Vec<u32> = engine.plugins.slots[si]
                    .inst
                    .params()
                    .iter()
                    .filter(|x| !x.readonly)
                    .take(8)
                    .map(|x| x.id)
                    .collect();
                p.macros = (0..8)
                    .map(|k| ids.get(k).copied().unwrap_or(NO_PARAM))
                    .collect();
                changed = true;
            }
            if changed {
                engine.plugins.refresh_macros(song);
            }
        });
    }

    /// Copy every plugin's current state into the song (before saving,
    /// exporting or copying an instrument). Not an undo step of its own.
    /// Copy every plugin's current state into the song (and into the
    /// other instances of the same instrument / effect). Always asks the
    /// plugins: many never report their editor edits (`mark_dirty`), so a
    /// "nothing changed" flag can't be trusted. Not an undo step; the undo
    /// history only hears about it when a state really changed.
    pub(super) fn capture_plugins(&mut self) {
        self.plug.state_dirty = false;
        let shared = self.shared.clone();
        let mut s = audio::lock(&shared);
        let crate::audio::Shared { song, engine, .. } = &mut *s;
        if engine.plugins.is_empty() && engine.fx_plugins.is_empty() {
            return;
        }
        let before = plugin_patches(song);
        let instrs: Vec<usize> = (0..song.instruments.len())
            .filter(|&i| engine.plugins.primary(i).is_some())
            .collect();
        for i in instrs {
            engine.plugins.capture(song, i);
        }
        engine.fx_plugins.capture_all(song);
        let after = plugin_patches(song);
        if after != before {
            // commit pending edits as they were, then fold the new plugin
            // states into the current step
            put_plugin_patches(song, &before);
            self.hist.boundary(song, std::time::Instant::now());
            put_plugin_patches(song, &after);
            self.hist.absorb(song);
        }
    }

    pub(super) fn start_plugin_scan(&mut self, force: bool) {
        if self.plug.scan.is_some() {
            return;
        }
        let dirs = scan::all_dirs(&self.extra.plugin_dirs);
        let old = scan::cache();
        let progress = Arc::new(Mutex::new((0, 0, String::new())));
        let (tx, rx) = std::sync::mpsc::channel();
        let p2 = progress.clone();
        std::thread::spawn(move || {
            let exe = std::env::current_exe().unwrap_or_default();
            let c = scan::scan(&exe, &dirs, &old, force, &|k, n, f| {
                if let Ok(mut g) = p2.lock() {
                    *g = (k, n, f.to_string());
                }
            });
            let _ = tx.send(c);
        });
        self.plug.scan = Some(ScanJob { progress, rx });
        self.plug.scan_msg = "Scanning plugin folders…".into();
    }

    fn scan_progress_ui(&self, ui: &mut egui::Ui) {
        if let Some(job) = &self.plug.scan {
            let (k, n, f) = job.progress.lock().map(|g| g.clone()).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Scanning {}/{}", (k + 1).min(n.max(1)), n.max(1)));
            });
            let name = std::path::Path::new(&f)
                .file_name()
                .map(|x| x.to_string_lossy().into_owned())
                .unwrap_or_default();
            ui.label(egui::RichText::new(name).small().color(th().ink2));
        } else if !self.plug.scan_msg.is_empty() {
            ui.label(
                egui::RichText::new(&self.plug.scan_msg)
                    .small()
                    .color(th().ink2),
            );
        }
    }

    /// New instrument menu entry
    pub(super) fn new_plugin_menu_item(&mut self, ui: &mut egui::Ui) {
        if ui
            .button("Plugin instrument…")
            .on_hover_text("a CLAP or VST3 instrument plugin installed on this computer")
            .clicked()
        {
            self.plug.browser = true;
            ui.close();
        }
    }

    /// "Add plugin instrument" window
    pub(super) fn plugin_browser(&mut self, ctx: &egui::Context) {
        if !self.plug.browser {
            return;
        }
        let mut open = true;
        let mut add: Option<scan::Found> = None;
        let mut rescan = false;
        let cache = scan::cache();
        if cache.entries.is_empty() && self.plug.scan.is_none() && self.plug.scan_msg.is_empty() {
            // first use: scan the standard folders
            rescan = true;
        }
        egui::Window::new("Add plugin instrument")
            .open(&mut open)
            .default_size([460.0, 420.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.add(egui::TextEdit::singleline(&mut self.plug.query).desired_width(200.0));
                    if ui
                        .add_enabled(self.plug.scan.is_none(), egui::Button::new("Rescan"))
                        .on_hover_text(
                            "look for plugins again (each file is checked in a separate \
                             process, so a broken plugin can't crash ChipWave)",
                        )
                        .clicked()
                    {
                        rescan = true;
                    }
                });
                self.scan_progress_ui(ui);
                ui.separator();
                let q = self.plug.query.to_lowercase();
                let list: Vec<scan::Found> = cache
                    .instruments()
                    .into_iter()
                    .filter(|f| {
                        q.is_empty()
                            || f.desc.name.to_lowercase().contains(&q)
                            || f.desc.vendor.to_lowercase().contains(&q)
                    })
                    .collect();
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        if list.is_empty() {
                            ui.label(if self.plug.scan.is_some() {
                            "Scanning…"
                        } else {
                            "No plugin instruments found. Install a CLAP or VST3 instrument (for example \
                             Surge XT) or add its folder in Settings > Plugins, then Rescan."
                        });
                        }
                        for f in &list {
                            ui.horizontal(|ui| {
                                if ui.button("+ Add").clicked() {
                                    add = Some(f.clone());
                                }
                                ui.label(egui::RichText::new(&f.desc.name).strong());
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · {} {}",
                                        f.desc.vendor,
                                        f.format.to_uppercase(),
                                        f.desc.version
                                    ))
                                    .small()
                                    .color(th().ink2),
                                );
                            });
                        }
                    });
                let skipped: Vec<&scan::Entry> =
                    cache.entries.iter().filter(|e| e.status != "ok").collect();
                if !skipped.is_empty() {
                    ui.collapsing(format!("{} file(s) skipped", skipped.len()), |ui| {
                        for e in skipped {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {} {}",
                                    e.status, e.path, e.message
                                ))
                                .small()
                                .color(th().ink2),
                            );
                        }
                    });
                }
            });
        if rescan {
            self.start_plugin_scan(false);
        }
        if let Some(f) = add {
            let name = f.desc.name.clone();
            self.add_instrument(
                &name,
                InstrumentKind::Plugin(PluginPatch {
                    format: f.format,
                    id: f.desc.id,
                    name: f.desc.name,
                    vendor: f.desc.vendor,
                    path: f.path,
                    state: String::new(),
                    params: vec![],
                    macros: vec![],
                }),
            );
            // load it right away (the cursor channel plays it)
            let extra = (self.instr as usize).checked_sub(1).map(|i| (i, self.ch));
            plugin::sync(&self.shared, extra);
            self.default_macros();
            open = false;
        }
        self.plug.browser = open;
    }

    /// the side-panel editor of a plugin instrument (instrument `i`, 0-based)
    pub(super) fn plugin_panel(&mut self, ui: &mut egui::Ui, i: usize, p: &PluginPatch) {
        if !self.plugin_status_ui(ui, i, p) {
            return;
        }
        let params = self.plugin_param_infos(i);
        self.plugin_macro_assign(ui, i, p, &params);
        self.plugin_param_list(ui, i, p, &params);
    }

    /// plugin name, load status, Configure… / Close editor, Reload; true when loaded
    pub(super) fn plugin_status_ui(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        p: &PluginPatch,
    ) -> bool {
        let (loaded, n, err, has_ed, ed_open) = self.with(|s| {
            let r = &s.engine.plugins;
            let n = r.slots.iter().filter(|x| x.instr == i).count();
            let pr = r.primary(i).map(|k| &r.slots[k].inst);
            (
                pr.is_some(),
                n,
                r.error(i).map(str::to_string),
                pr.is_some_and(|x| x.has_editor()),
                pr.is_some_and(|x| x.editor_open()),
            )
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} · {} · {}",
                    p.name,
                    if p.vendor.is_empty() {
                        "plugin"
                    } else {
                        &p.vendor
                    },
                    p.format.to_uppercase()
                ))
                .color(th().value),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let (txt, col) = match (&err, loaded) {
                (Some(e), _) => (e.clone(), th().brick),
                (None, true) => (
                    format!("loaded · {n} instance(s) (one per channel it plays on)"),
                    th().ink2,
                ),
                (None, false) => ("not playing yet: put a note in a block".into(), th().ink2),
            };
            ui.add(egui::Label::new(egui::RichText::new(txt).small().color(col)).wrap());
        });
        let mut reload = false;
        let mut editor: Option<bool> = None;
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let label = if ed_open {
                "Close editor"
            } else {
                "Configure…"
            };
            if ui
                .add_enabled(has_ed, egui::Button::new(label))
                .on_hover_text("open the plugin's own window")
                .on_disabled_hover_text(if loaded {
                    "this plugin has no editor window here: use the parameters below"
                } else {
                    "the plugin is not loaded"
                })
                .clicked()
            {
                editor = Some(!ed_open);
            }
            if ui
                .button("Reload")
                .on_hover_text("load the plugin again (after installing it or a crash)")
                .clicked()
            {
                reload = true;
            }
        });
        if let Some(open) = editor {
            let title = format!("{} – ChipWave", p.name);
            let r = self.with(|s| {
                let k = s.engine.plugins.primary(i)?;
                let inst = &mut s.engine.plugins.slots[k].inst;
                Some(if open {
                    inst.open_editor(&title)
                } else {
                    inst.close_editor();
                    Ok(())
                })
            });
            if let Some(Err(e)) = r {
                self.status = format!("Plugin editor: {e}");
            }
            if !open {
                // closing keeps the editor's edits and syncs the other instances
                self.plug.state_dirty = true;
                self.capture_plugins();
            }
        }
        if reload {
            self.capture_plugins();
            let gone = self.with(|s| {
                s.engine.plugins.retry(i);
                let drop: Vec<usize> = s
                    .engine
                    .plugins
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| x.instr == i)
                    .map(|(k, _)| k)
                    .collect();
                s.engine.plugins.take(&drop)
            });
            drop(gone);
            scan::set_cache(scan::cache()); // keep the cache; resolve again
            let extra = (self.instr as usize).checked_sub(1).map(|i| (i, self.ch));
            plugin::sync(&self.shared, extra);
            self.plug.params = None;
        }
        loaded
    }

    /// the loaded plugin's parameter list (refreshed now and then: plugins may rename them)
    pub(super) fn plugin_param_infos(&mut self, i: usize) -> Vec<crate::plugin::ParamInfo> {
        if self.plug.params.as_ref().is_none_or(|x| x.0 != i) || self.plug.tick.is_multiple_of(120)
        {
            let list = self.with(|s| {
                let k = s.engine.plugins.primary(i)?;
                Some(s.engine.plugins.slots[k].inst.params())
            });
            self.plug.params = list.map(|l| (i, l));
        }
        self.plug
            .params
            .as_ref()
            .map(|x| x.1.clone())
            .unwrap_or_default()
    }

    /// macros 1-8: which parameter each one drives
    pub(super) fn plugin_macro_assign(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        p: &PluginPatch,
        params: &[crate::plugin::ParamInfo],
    ) {
        // macros 1-8 (43xy, automation lanes, MIDI learn)
        let mut macros = p.macros.clone();
        macros.resize(8, NO_PARAM);
        let mut mchanged = false;
        ui.collapsing("Macros 1-8 (43xy · automation · MIDI learn)", |ui| {
            egui::Grid::new("plug_macros")
                .num_columns(2)
                .show(ui, |ui| {
                    for (k, m) in macros.iter_mut().enumerate() {
                        ui.label(format!("   {}", k + 1));
                        let cur = params
                            .iter()
                            .find(|x| x.id == *m)
                            .map(|x| x.name.clone())
                            .unwrap_or_else(|| "(none)".into());
                        egui::ComboBox::from_id_salt(("plug_macro", k))
                            .selected_text(cur)
                            .width(200.0)
                            .height(300.0)
                            .show_ui(ui, |ui| {
                                mchanged |= ui.selectable_value(m, NO_PARAM, "(none)").changed();
                                for x in params.iter().filter(|x| !x.readonly) {
                                    mchanged |= ui.selectable_value(m, x.id, x.label()).changed();
                                }
                            });
                        ui.end_row();
                    }
                });
        });
        if mchanged {
            self.snapshot();
            self.with(|s| {
                if let Some(InstrumentKind::Plugin(q)) =
                    s.song.instruments.get_mut(i).map(|x| &mut x.kind)
                {
                    q.macros = macros.clone();
                }
                let crate::audio::Shared { song, engine, .. } = s;
                engine.plugins.refresh_macros(song);
            });
        }
    }

    /// every parameter as a slider (searchable; right-click puts one on a macro)
    pub(super) fn plugin_param_list(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        p: &PluginPatch,
        params: &[crate::plugin::ParamInfo],
    ) {
        let mut macros = p.macros.clone();
        macros.resize(8, NO_PARAM);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(format!("Parameters ({})", params.len()));
            ui.add(
                egui::TextEdit::singleline(&mut self.plug.param_query)
                    .hint_text("search")
                    .desired_width(120.0),
            );
        });
        let q = self.plug.param_query.to_lowercase();
        let vals: Vec<(u32, f64, String)> = self.with(|s| {
            let Some(k) = s.engine.plugins.primary(i) else {
                return vec![];
            };
            let inst = &s.engine.plugins.slots[k].inst;
            params
                .iter()
                .filter(|x| q.is_empty() || x.label().to_lowercase().contains(&q))
                .take(300)
                .map(|x| {
                    let v = inst.value(x.id).unwrap_or(x.default);
                    let t = inst.value_text(x.id, v).unwrap_or_default();
                    (x.id, v, t)
                })
                .collect()
        });
        let mut set: Vec<(u32, f64)> = vec![];
        let mut map: Option<(usize, u32)> = None;
        let lw = (ui.available_width() * 0.42).clamp(90.0, 170.0);
        for (id, v, t) in &vals {
            let Some(info) = params.iter().find(|x| x.id == *id) else {
                continue;
            };
            let mut name = info.label();
            if let Some(k) = macros.iter().position(|m| m == id) {
                name = format!("[M{}] {name}", k + 1);
            }
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(lw, 16.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(lw);
                        ui.add(egui::Label::new(egui::RichText::new(&name).small()).truncate())
                            .on_hover_text(&name);
                    },
                );
                let mut x = *v;
                let mut s = egui::Slider::new(&mut x, info.min..=info.max.max(info.min + 1e-9))
                    .show_value(false);
                if info.stepped {
                    s = s.step_by(1.0);
                }
                ui.spacing_mut().slider_width = (ui.available_width() - 84.0).clamp(40.0, 130.0);
                let r = ui.add_enabled(!info.readonly, s);
                ui.add(
                    egui::Label::new(egui::RichText::new(t).small().monospace().color(th().value))
                        .truncate(),
                );
                if r.changed() {
                    set.push((*id, x));
                }
                r.context_menu(|ui| {
                    for k in 0..8 {
                        if ui.button(format!("Put on macro {}", k + 1)).clicked() {
                            map = Some((k, *id));
                            ui.close();
                        }
                    }
                });
            });
        }
        if !set.is_empty() {
            self.with(|s| {
                for (id, v) in &set {
                    s.engine.plugins.set_param(i, *id, *v);
                }
            });
            self.plug.state_dirty = true;
            self.dirty = true;
        }
        if let Some((k, id)) = map {
            self.snapshot();
            self.with(|s| {
                if let Some(InstrumentKind::Plugin(q)) =
                    s.song.instruments.get_mut(i).map(|x| &mut x.kind)
                {
                    q.macros.resize(8, NO_PARAM);
                    q.macros[k] = id;
                }
                let crate::audio::Shared { song, engine, .. } = s;
                engine.plugins.refresh_macros(song);
            });
        }
    }

    pub(super) fn plugins_summary(&self) -> (Vec<String>, Option<String>) {
        let c = scan::cache();
        let n = c.instruments().len();
        let bad = c.entries.iter().filter(|e| e.status != "ok").count();
        (
            vec![
                "Plugin instruments".into(),
                if c.entries.is_empty() {
                    "not scanned yet".into()
                } else {
                    format!("{n} found · {bad} skipped")
                },
                format!("{} extra folder(s)", self.extra.plugin_dirs.len()),
            ],
            None,
        )
    }

    pub(super) fn plugin_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("ChipWave looks for CLAP and VST3 instrument plugins in the standard folders and in your own folders below.");
        ui.label(
            egui::RichText::new(
                "Each file is tested in a separate process; plugins that crash or hang are skipped and listed.",
            )
            .small()
            .color(th().ink2),
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Standard folders").strong());
        for d in scan::default_dirs() {
            ui.label(
                egui::RichText::new(d.display().to_string())
                    .monospace()
                    .small(),
            );
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Your folders").strong());
        let mut remove = None;
        for (k, d) in self.extra.plugin_dirs.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.small_button("✕").on_hover_text("remove").clicked() {
                    remove = Some(k);
                }
                ui.label(egui::RichText::new(d).monospace().small());
            });
        }
        let mut changed = false;
        if let Some(k) = remove {
            self.extra.plugin_dirs.remove(k);
            changed = true;
        }
        ui.horizontal(|ui| {
            if ui.button("Add folder…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    self.extra.plugin_dirs.push(p.display().to_string());
                    changed = true;
                }
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.plug.new_dir)
                    .hint_text("or type a path")
                    .desired_width(240.0),
            );
            if ui.button("Add").clicked() && !self.plug.new_dir.trim().is_empty() {
                self.extra
                    .plugin_dirs
                    .push(self.plug.new_dir.trim().to_string());
                self.plug.new_dir.clear();
                changed = true;
            }
        });
        if changed {
            self.save_settings();
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.plug.scan.is_none(), egui::Button::new("Rescan"))
                .on_hover_text("check new and changed plugin files")
                .clicked()
            {
                self.start_plugin_scan(false);
            }
            if ui
                .add_enabled(self.plug.scan.is_none(), egui::Button::new("Full rescan"))
                .on_hover_text("check every plugin file again, also the skipped ones")
                .clicked()
            {
                self.start_plugin_scan(true);
            }
            if ui
                .button("Find plugins…")
                .on_hover_text("search the standard folders (and optionally the whole drive) for VST3 / CLAP plugin folders, or pick them yourself")
                .clicked()
            {
                self.open_plugin_setup(false);
            }
        });
        self.scan_progress_ui(ui);
        let c = scan::cache();
        for e in &c.entries {
            let col = if e.status == "ok" {
                th().value
            } else {
                th().brick
            };
            ui.label(
                egui::RichText::new(format!(
                    "{} {} · {}{}",
                    e.format.to_uppercase(),
                    e.status,
                    e.path,
                    if e.message.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", e.message)
                    }
                ))
                .small()
                .monospace()
                .color(col),
            );
        }
    }
}

/// every plugin patch in the song, in a fixed order: instrument plugins,
/// then plugin devices in each instrument's chain
fn plugin_patches(song: &Song) -> Vec<PluginPatch> {
    let mut v = vec![];
    for inst in &song.instruments {
        if let InstrumentKind::Plugin(p) = &inst.kind {
            v.push(p.clone());
        }
        v.extend(inst.fx.iter().filter_map(|d| d.plugin.as_deref().cloned()));
    }
    v
}

/// write back what [`plugin_patches`] returned (same song shape)
fn put_plugin_patches(song: &mut Song, list: &[PluginPatch]) {
    let mut it = list.iter();
    for inst in &mut song.instruments {
        if let InstrumentKind::Plugin(p) = &mut inst.kind {
            if let Some(x) = it.next() {
                p.clone_from(x);
            }
        }
        for d in inst.fx.iter_mut() {
            if let Some(p) = d.plugin.as_deref_mut() {
                if let Some(x) = it.next() {
                    p.clone_from(x);
                }
            }
        }
    }
}
