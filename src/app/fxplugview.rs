//! Plugin effects in the app (Unreleased): the "Plugin effect" entries of
//! the add-device menus, the plugin device body in the FX view (status,
//! editor, parameters, automation) and the per-frame sync of the engine's
//! effect rack. The engine side lives in src/plugin/fxrack.rs.
use super::instruments::MAX_DEVICES;
use super::*;
use crate::fx::FxKind;
use crate::plugin::{fxrack, scan, ParamInfo, PluginPatch};

/// a scanned CLAP effect as a fresh device patch
pub(super) fn patch_of(f: scan::Found) -> PluginPatch {
    PluginPatch {
        format: f.format,
        id: f.desc.id,
        name: f.desc.name,
        vendor: f.desc.vendor,
        path: f.path,
        state: String::new(),
        params: vec![],
        macros: vec![],
    }
}

impl App {
    /// "Plugin effect ›" submenu (every add-device menu); the chosen effect
    pub(super) fn plugin_fx_menu(&mut self, ui: &mut egui::Ui) -> Option<PluginPatch> {
        let mut pick = None;
        let mut rescan = false;
        ui.menu_button(FxKind::Plugin.name(), |ui| {
            let list = scan::cache().effects();
            if list.is_empty() {
                ui.label(
                    egui::RichText::new(if self.plug.scan.is_some() {
                        "Scanning…"
                    } else {
                        "No CLAP effects found yet"
                    })
                    .color(th().ink2),
                );
            }
            ui.set_max_height(360.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for f in list {
                    let tip = format!(
                        "{} · {} {}\n{}",
                        f.desc.vendor,
                        f.format.to_uppercase(),
                        f.desc.version,
                        f.path
                    );
                    if ui.button(&f.desc.name).on_hover_text(tip).clicked() {
                        pick = Some(patch_of(f));
                        ui.close();
                    }
                }
            });
            ui.separator();
            if ui
                .add_enabled(
                    self.plug.scan.is_none(),
                    egui::Button::new("Scan for plugins"),
                )
                .on_hover_text("look in the plugin folders again (Settings > Plugins)")
                .clicked()
            {
                rescan = true;
                ui.close();
            }
        });
        if rescan {
            self.start_plugin_scan(false);
        }
        pick
    }

    /// add a plugin effect device to instrument `slot` (1-based) and load it
    pub(super) fn add_plugin_fx(&mut self, slot: u8, p: PluginPatch) {
        let idx = (slot as usize).wrapping_sub(1);
        self.snapshot();
        let name = p.name.clone();
        let ok = self.with(|s| {
            let i = s.song.instruments.get_mut(idx)?;
            if i.fx.len() >= MAX_DEVICES {
                return Some(false);
            }
            i.fx.push(crate::fx::Device::plugin(p));
            Some(true)
        });
        self.status = match ok {
            Some(true) => format!("Added {name} to instrument {slot:02X}"),
            Some(false) => "Chain is full (16 devices)".into(),
            None => String::new(),
        };
        self.dirty = true;
        self.sync_fx_plugins();
    }

    pub(super) fn sync_fx_plugins(&mut self) {
        let extra = (self.instr as usize).checked_sub(1).map(|i| (i, self.ch));
        fxrack::sync(&self.shared, extra);
    }

    /// every frame the song uses plugin effects (from plugin_frame): load /
    /// drop effect instances, run main-thread callbacks
    pub(super) fn fx_plugin_frame(&mut self) {
        if self.plug.tick % 15 == 1 {
            self.sync_fx_plugins();
        }
        let dirty = self.with(|s| s.engine.fx_plugins.poll_main());
        if !dirty.is_empty() {
            self.plug.state_dirty = true;
            self.dirty = true;
        }
    }

    /// parameter list of a plugin device's running instance
    pub(super) fn fx_plugin_params(&self, owner: usize, uid: u32) -> Vec<ParamInfo> {
        self.with(|s| {
            let r = &s.engine.fx_plugins;
            r.primary(owner, uid)
                .map(|k| r.slots[k].inst.params())
                .unwrap_or_default()
        })
    }

    /// the FX view body of a plugin device `d` of instrument `owner`
    pub(super) fn plugin_fx_body(
        &mut self,
        ui: &mut egui::Ui,
        owner: usize,
        d: usize,
        dev: &crate::fx::Device,
    ) {
        let Some(p) = dev.plugin.as_deref() else {
            return;
        };
        let uid = dev.uid;
        let (loaded, err, has_ed, ed_open, lat) = self.with(|s| {
            let r = &s.engine.fx_plugins;
            let pr = r.primary(owner, uid).map(|k| &r.slots[k]);
            (
                pr.is_some(),
                r.error(owner, uid).map(str::to_string),
                pr.is_some_and(|x| x.inst.has_editor()),
                pr.is_some_and(|x| x.inst.editor_open()),
                pr.map(|x| x.latency).unwrap_or(0),
            )
        });
        let t = th();
        ui.label(
            egui::RichText::new(format!(
                "{} · {}",
                if p.vendor.is_empty() {
                    "plugin"
                } else {
                    &p.vendor
                },
                p.format.to_uppercase()
            ))
            .small()
            .color(t.ink2),
        );
        let sr = self.with(|s| s.engine.sr).max(1.0);
        let (txt, col) = match (&err, loaded) {
            (Some(e), _) => (format!("{e} · bypassed, settings kept"), t.brick),
            (None, true) => (
                format!(
                    "loaded · latency {lat} smp ({:.1} ms, compensated)",
                    lat as f32 * 1000.0 / sr
                ),
                t.ink2,
            ),
            (None, false) => ("loads when a note plays it".into(), t.ink2),
        };
        ui.add(egui::Label::new(egui::RichText::new(txt).small().color(col)).wrap());
        let mut editor: Option<bool> = None;
        let mut reload = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    has_ed,
                    egui::Button::new(if ed_open { "Close editor" } else { "Editor…" }),
                )
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
                let k = s.engine.fx_plugins.primary(owner, uid)?;
                let inst = &mut s.engine.fx_plugins.slots[k].inst;
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
        }
        if reload {
            self.capture_plugins();
            let gone = self.with(|s| {
                let r = &mut s.engine.fx_plugins;
                r.retry();
                let drop: Vec<usize> = r
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| x.owner == owner && x.uid == uid)
                    .map(|(k, _)| k)
                    .collect();
                r.take(&drop)
            });
            drop(gone);
            self.sync_fx_plugins();
        }
        if !loaded {
            return;
        }
        // parameters: slider each, right-click = automation lane
        let params = self.fx_plugin_params(owner, uid);
        // a modulated parameter shows its own value (the live one is a dot)
        let tid = |id: u32| crate::automation::Target::Fx(owner, d, id as usize).id();
        let vals: Vec<(f64, String)> = self.with(|s| {
            let r = &s.engine.fx_plugins;
            let Some(k) = r.primary(owner, uid) else {
                return vec![];
            };
            let inst = &r.slots[k].inst;
            let m = &s.song.modulation;
            params
                .iter()
                .take(200)
                .map(|x| {
                    let modded = m.routes_to(&tid(x.id)).next().is_some();
                    let v = match modded {
                        true => r.base_value(k, x.id),
                        false => None,
                    }
                    .or_else(|| inst.value(x.id))
                    .unwrap_or(x.default);
                    (v, inst.value_text(x.id, v).unwrap_or_default())
                })
                .collect()
        });
        let w = ui.available_width();
        let mut set: Vec<(u32, f64)> = vec![];
        let mut lane: Option<u32> = None;
        egui::ScrollArea::vertical()
            .id_salt(("plugfx", owner, uid))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().slider_width = w - 4.0;
                ui.spacing_mut().item_spacing.y = 1.0;
                for (info, (v, txt)) in params.iter().zip(&vals) {
                    let (row, _) = ui.allocate_exact_size(egui::vec2(w, 15.0), Sense::hover());
                    let pr = ui.painter();
                    pr.text(
                        row.left_center(),
                        egui::Align2::LEFT_CENTER,
                        info.label(),
                        FontId::proportional(11.0),
                        t.ink2,
                    );
                    pr.text(
                        row.right_center(),
                        egui::Align2::RIGHT_CENTER,
                        txt,
                        FontId::monospace(11.0),
                        t.value,
                    );
                    let mut x = *v;
                    let mut s = egui::Slider::new(&mut x, info.min..=info.max.max(info.min + 1e-9))
                        .show_value(false)
                        .trailing_fill(true);
                    if info.stepped {
                        s = s.step_by(1.0);
                    }
                    let r = ui.add_enabled(!info.readonly, s);
                    if r.double_clicked() {
                        set.push((info.id, info.default));
                    } else if r.changed() {
                        set.push((info.id, x));
                    }
                    if !info.readonly {
                        let span = (info.max - info.min).max(1e-9);
                        let n = ((*v - info.min) / span).clamp(0.0, 1.0) as f32;
                        let mut l = false;
                        self.mod_slider(ui, &r, &tid(info.id), n, Some(&mut l));
                        if l {
                            lane = Some(info.id);
                        }
                    }
                }
            });
        if !set.is_empty() {
            self.with(|s| {
                for (id, v) in &set {
                    s.engine.fx_plugins.set_param(owner, uid, *id, *v);
                }
            });
            self.plug.state_dirty = true;
            self.dirty = true;
        }
        if let Some(id) = lane {
            self.add_lane(crate::automation::Target::Fx(owner, d, id as usize));
        }
    }
}
