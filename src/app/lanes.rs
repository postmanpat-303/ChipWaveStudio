//! Automation lanes: lane edit operations, adding lanes and the automation bar.
use super::*;

/// a plugin effect device in the lane menu: (instrument, device, title,
/// (param id, name))
type PlugDev = (usize, usize, String, Vec<(u32, String)>);

/// data for [`lane_menu_ui`] (gathered outside the menu closure)
pub(crate) struct LaneMenu {
    fxdevs: Vec<Vec<crate::fx::FxKind>>,
    plugdevs: Vec<PlugDev>,
    synths: Vec<(String, bool)>,
    nch: usize,
    /// song lanes
    pub(super) nl: usize,
    gens: Vec<Vec<(usize, String)>>,
    plugs: Vec<bool>,
    routes: Vec<(usize, String)>,
}

/// a new lane's first value: where the target is now (normalised)
pub(super) fn lane_start_value(s: &Shared, t: crate::automation::Target) -> f32 {
    use crate::automation::Target;
    match t {
        Target::Fx(i, d, p) => {
            let dev = s.song.instruments.get(i).and_then(|x| x.fx.get(d));
            match dev {
                // plugin parameter: its current value over its range
                Some(dev) if dev.plugin.is_some() => {
                    let r = &s.engine.fx_plugins;
                    r.primary(i, dev.uid)
                        .and_then(|k| {
                            let inst = &r.slots[k].inst;
                            let info = inst.params().into_iter().find(|x| x.id as usize == p)?;
                            let v = inst.value(info.id)?;
                            let span = (info.max - info.min).max(1e-9);
                            Some(((v - info.min) / span) as f32)
                        })
                        .unwrap_or(0.5)
                }
                _ => crate::automation::song_norm(&s.song, t).unwrap_or(0.5),
            }
        }
        Target::Macro(i, k) => crate::plugin::macro_norm(s, i, k).unwrap_or(0.5),
        Target::Send(..) => crate::automation::song_norm(&s.song, t).unwrap_or(0.0),
        _ => crate::automation::song_norm(&s.song, t).unwrap_or(0.5),
    }
}

impl App {
    pub(super) fn apply_lane_ops(&mut self, ops: Vec<LaneOp>) {
        use crate::automation::Point;
        let shape = self.auto_shape;
        for op in ops {
            match op {
                LaneOp::Snapshot => self.snapshot(),
                LaneOp::Add(k, pos, value) => {
                    self.snapshot();
                    let idx = self.with(|s| {
                        let lane = &mut s.song.automation[k];
                        lane.points.push(Point {
                            pos,
                            value,
                            shape,
                            curve: 0.0,
                        });
                        lane.sort();
                        lane.points
                            .iter()
                            .position(|p| p.pos == pos && p.value == value)
                    });
                    self.auto_sel = idx.map(|i| (k, i));
                    self.status =
                        "Added automation point (drag to move, right-click to delete)".into();
                }
                LaneOp::Delete(k, i) => {
                    self.snapshot();
                    self.with(|s| {
                        if i < s.song.automation[k].points.len() {
                            s.song.automation[k].points.remove(i);
                        }
                    });
                    self.auto_sel = None;
                    self.status = "Deleted automation point".into();
                }
                LaneOp::Move(k, i, pos, value) => {
                    self.with(|s| {
                        let pts = &mut s.song.automation[k].points;
                        if i < pts.len() {
                            let lo = if i > 0 { pts[i - 1].pos } else { 0.0 };
                            let hi = pts.get(i + 1).map(|p| p.pos).unwrap_or(f32::MAX);
                            pts[i].pos = pos.clamp(lo, hi);
                            pts[i].value = value;
                        }
                    });
                }
                LaneOp::Pencil(k, pos, value) => {
                    self.with(|s| {
                        let lane = &mut s.song.automation[k];
                        if let Some(p) = lane.points.iter_mut().find(|p| (p.pos - pos).abs() < 0.5)
                        {
                            p.value = value;
                        } else {
                            lane.points.push(Point {
                                pos,
                                value,
                                shape,
                                curve: 0.0,
                            });
                            lane.sort();
                        }
                    });
                }
            }
            self.dirty = true;
        }
    }

    pub(super) fn add_lane(&mut self, t: crate::automation::Target) {
        use crate::automation::Lane;
        let label = self.lane_label(t);
        self.snapshot();
        let n = self.with(|s| {
            // start flat at the current value
            let cur = lane_start_value(s, t);
            let mut lane = Lane::new(t, label.clone());
            lane.points.push(crate::automation::Point {
                pos: 0.0,
                value: cur,
                shape: crate::automation::Shape::Linear,
                curve: 0.0,
            });
            s.song.automation.push(lane);
            s.song.automation.len() - 1
        });
        self.auto_lane = n;
        self.show_auto = true;
        self.pr.open = false;
        // the lower panel shows the new lane (it opens on the instrument
        // editor by default, where lanes aren't drawn)
        if self.extra.lower_inst {
            self.extra.lower_inst = false;
            self.save_settings();
        }
        self.status = format!("Added lane '{label}' (click to add points, Pencil to draw)");
    }

    /// "02 Lead - Filter cutoff": a lane's label for target `t`
    pub(super) fn lane_label(&self, t: crate::automation::Target) -> String {
        use crate::automation::Target;
        // plugin effect parameter: its name from the running instance
        let plug_name = match t {
            Target::Fx(i, d, p) => self
                .with(|s| {
                    s.song
                        .instruments
                        .get(i)
                        .and_then(|x| x.fx.get(d))
                        .filter(|dev| dev.plugin.is_some())
                        .map(|dev| dev.uid)
                })
                .map(|uid| {
                    self.fx_plugin_params(i, uid)
                        .into_iter()
                        .find(|x| x.id as usize == p)
                        .map(|x| x.name)
                        .unwrap_or_else(|| format!("param {p}"))
                }),
            _ => None,
        };
        self.with(|s| {
            let iname = |i: usize| {
                s.song
                    .instruments
                    .get(i)
                    .map(|x| x.name.clone())
                    .unwrap_or_default()
            };
            let cname = |c: usize| s.song.channel_names.get(c).cloned().unwrap_or_default();
            match t {
                Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) | Target::Level(i) => {
                    format!("{} - {}", iname(i), t.param_name())
                }
                Target::ChanVolume(c) | Target::ChanPan(c) => {
                    format!("Ch {} {} - {}", c + 1, cname(c), t.param_name())
                }
                Target::Fx(i, d, p) => {
                    let pn = s
                        .song
                        .instruments
                        .get(i)
                        .and_then(|x| x.fx.get(d))
                        .map(|dev| {
                            format!(
                                "{} {}",
                                dev.title(),
                                match &plug_name {
                                    Some(n) => n.as_str(),
                                    None => crate::fx::params(dev.kind)
                                        .get(p)
                                        .map(|x| x.name)
                                        .unwrap_or(""),
                                }
                            )
                        });
                    format!("{} - {}", iname(i), pn.unwrap_or_default())
                }
                Target::Ret(r, p) => {
                    let k = if r == 0 {
                        crate::fx::FxKind::Delay
                    } else {
                        crate::fx::FxKind::Reverb
                    };
                    format!(
                        "Return {} - {}",
                        if r == 0 { "A" } else { "B" },
                        crate::fx::params(k).get(p).map(|x| x.name).unwrap_or("")
                    )
                }
                Target::Macro(i, k) => {
                    let pn = match s.song.instruments.get(i).map(|x| &x.kind) {
                        Some(InstrumentKind::Plugin(p)) => s
                            .engine
                            .plugins
                            .primary(i)
                            .and_then(|si| {
                                let id = p.macro_id(k);
                                s.engine.plugins.slots[si]
                                    .inst
                                    .params()
                                    .into_iter()
                                    .find(|x| x.id == id)
                                    .map(|x| x.name)
                            })
                            .unwrap_or_default(),
                        _ => String::new(),
                    };
                    format!("{} - Macro {} {pn}", iname(i), k + 1)
                }
                Target::Gen(i, k) => {
                    let pn = match s.song.instruments.get(i).map(|x| &x.kind) {
                        Some(InstrumentKind::Gen(g)) => crate::gen::gen_knobs(g)
                            .into_iter()
                            .find(|x| x.0 == k)
                            .map(|x| x.1)
                            .unwrap_or_else(|| format!("Knob {}", k + 1)),
                        _ => format!("Knob {}", k + 1),
                    };
                    format!("{} - {pn}", iname(i))
                }
                Target::Send(c, r) => {
                    format!(
                        "Ch {} {} - Send {}",
                        c + 1,
                        cname(c),
                        if r == 0 { "A" } else { "B" }
                    )
                }
                Target::Patch(i, _) => format!("{} - {}", iname(i), t.param_name()),
                Target::ModAmount(r) => {
                    let m = &s.song.modulation;
                    match m.routes.get(r) {
                        Some(rt) => format!(
                            "Depth {} > {}",
                            m.find(rt.src).map(|k| m.title(k)).unwrap_or_default(),
                            super::modview::target_label(&s.song, &rt.target)
                        ),
                        None => t.param_name().to_string(),
                    }
                }
                _ => t.param_name().to_string(),
            }
        })
    }
    /// what the "+ Lane" menus list (instruments, their FX and plugin
    /// parameters, generator knobs, mixer, returns, modulation depths)
    pub(super) fn lane_menu(&self) -> LaneMenu {
        let fxdevs = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| i.fx.iter().map(|d| d.kind).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        });
        // plugin effect devices: (instrument, device) -> (title, parameters)
        let plugdevs: Vec<PlugDev> = self
            .with(|s| {
                let mut v = vec![];
                for (i, inst) in s.song.instruments.iter().enumerate() {
                    for (d, dev) in inst.fx.iter().enumerate() {
                        if dev.plugin.is_some() {
                            v.push((i, d, dev.title(), dev.uid));
                        }
                    }
                }
                v
            })
            .into_iter()
            .map(|(i, d, title, uid)| {
                let ps = self
                    .fx_plugin_params(i, uid)
                    .into_iter()
                    .filter(|x| !x.readonly)
                    .map(|x| (x.id, x.label()))
                    .collect();
                (i, d, title, ps)
            })
            .collect();
        let (_ninst, synths, nch, nl) = self.with(|s| {
            (
                s.song.instruments.len(),
                s.song
                    .instruments
                    .iter()
                    .map(|i| (i.name.clone(), i.kind.patch().is_some()))
                    .collect::<Vec<_>>(),
                s.song.channels,
                s.song.automation.len(),
            )
        });
        let gens: Vec<Vec<(usize, String)>> = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| match &i.kind {
                    InstrumentKind::Gen(g) => crate::gen::gen_knobs(g),
                    _ => Vec::new(),
                })
                .collect()
        });
        let plugs: Vec<bool> = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| matches!(i.kind, InstrumentKind::Plugin(_)))
                .collect()
        });
        let routes = self.with(|s| {
            let m = &s.song.modulation;
            m.routes
                .iter()
                .enumerate()
                .map(|(r, rt)| {
                    (
                        r,
                        format!(
                            "{} > {}",
                            m.find(rt.src).map(|k| m.title(k)).unwrap_or_default(),
                            super::modview::target_label(&s.song, &rt.target)
                        ),
                    )
                })
                .collect()
        });
        LaneMenu {
            fxdevs,
            plugdevs,
            synths,
            nch,
            nl,
            gens,
            plugs,
            routes,
        }
    }

    pub(super) fn automation_bar(&mut self, ui: &mut egui::Ui) {
        use crate::automation::Shape;
        let menu = self.lane_menu();
        let nl = menu.nl;
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(
                &mut self.show_auto,
                egui::RichText::new("Automation").strong(),
            );
            if !self.show_auto {
                return;
            }
            let mut add = None;
            ui.menu_button("+ Lane", |ui| {
                lane_menu_ui(ui, &menu, false, &mut add);
            });
            if let Some(t) = add {
                self.add_lane(t);
            }
            ui.separator();
            ui.selectable_value(&mut self.auto_pencil, false, "Select")
                .on_hover_text("click = add point, drag point = move, right-click = delete");
            ui.selectable_value(&mut self.auto_pencil, true, "Pencil")
                .on_hover_text("drag on a lane to draw (one point per line)");
            ui.label("Shape");
            let mut shape = self
                .auto_sel
                .and_then(|(k, i)| {
                    self.with(|s| {
                        s.song
                            .automation
                            .get(k)
                            .and_then(|l| l.points.get(i))
                            .map(|p| p.shape)
                    })
                })
                .unwrap_or(self.auto_shape);
            let before = shape;
            egui::ComboBox::from_id_salt("auto_shape")
                .selected_text(shape.name())
                .width(70.0)
                .show_ui(ui, |ui| {
                    for sh in Shape::ALL {
                        ui.selectable_value(&mut shape, sh, sh.name());
                    }
                })
                .response
                .on_hover_text("segment shape after the selected point / for new points");
            if shape != before {
                self.auto_shape = shape;
                if let Some((k, i)) = self.auto_sel {
                    self.snapshot();
                    self.with(|s| {
                        if let Some(p) = s
                            .song
                            .automation
                            .get_mut(k)
                            .and_then(|l| l.points.get_mut(i))
                        {
                            p.shape = shape;
                        }
                    });
                }
            }
            if nl > 0 {
                let k = self.auto_lane.min(nl - 1);
                let (mut en, label) = self.with(|s| {
                    (
                        s.song.automation[k].enabled,
                        s.song.automation[k].label.clone(),
                    )
                });
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("Lane: {label}"))
                        .small()
                        .color(th().ink2),
                );
                if ui.checkbox(&mut en, "on").changed() {
                    self.with(|s| s.song.automation[k].enabled = en);
                    self.dirty = true;
                }
                if ui.small_button("Delete lane").clicked() {
                    self.snapshot();
                    self.with(|s| {
                        s.song.automation.remove(k);
                    });
                    self.auto_sel = None;
                    self.status = format!("Deleted lane '{label}'");
                }
            }
        });
    }

    // ---------------------------------------------------------------- mixer
}

/// the "+ Lane" menu contents; `track`: for a timeline track (no tempo)
pub(super) fn lane_menu_ui(
    ui: &mut egui::Ui,
    m: &LaneMenu,
    track: bool,
    add: &mut Option<crate::automation::Target>,
) {
    use crate::automation::Target;
    let LaneMenu {
        fxdevs,
        plugdevs,
        synths,
        nch,
        gens,
        plugs,
        routes,
        ..
    } = m;
    let nch = *nch;
    ui.set_max_height(320.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, (n, synth)) in synths.iter().enumerate() {
            let mut ts = vec![Target::Level(i)];
            if plugs.get(i).copied().unwrap_or(false) {
                ts.extend((0..8).map(|k| Target::Macro(i, k)));
            }
            if *synth {
                ts = vec![
                    Target::Cutoff(i),
                    Target::Resonance(i),
                    Target::Drive(i),
                    Target::Level(i),
                ];
            }
            for t in ts {
                let txt = match t {
                    Target::Macro(_, k) => {
                        format!("{:02X} {n} - Macro {}", i + 1, k + 1)
                    }
                    _ => format!("{:02X} {n} - {}", i + 1, t.param_name()),
                };
                if ui.button(txt).clicked() {
                    *add = Some(t);
                    ui.close();
                }
            }
            if let Some(knobs) = gens.get(i).filter(|g| !g.is_empty()) {
                ui.menu_button(format!("{:02X} {n} - Generator knobs", i + 1), |ui| {
                    ui.set_max_height(320.0);
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (k, name) in knobs {
                            if ui.button(name).clicked() {
                                *add = Some(Target::Gen(i, *k));
                                ui.close();
                            }
                        }
                    });
                });
            }
            for (d, k) in fxdevs.get(i).into_iter().flatten().enumerate() {
                if let Some((_, _, title, ps)) = plugdevs.iter().find(|x| x.0 == i && x.1 == d) {
                    ui.menu_button(format!("{:02X} {n} - FX {} {title}", i + 1, d + 1), |ui| {
                        if ps.is_empty() {
                            ui.label("load the plugin first (play a note)");
                        }
                        ui.set_max_height(320.0);
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (id, name) in ps {
                                if ui.button(name).clicked() {
                                    *add = Some(Target::Fx(i, d, *id as usize));
                                    ui.close();
                                }
                            }
                        });
                    });
                    continue;
                }
                ui.menu_button(
                    format!("{:02X} {n} - FX {} {}", i + 1, d + 1, k.name()),
                    |ui| {
                        for (p, info) in crate::fx::params(*k).iter().enumerate() {
                            if ui.button(info.name).clicked() {
                                *add = Some(Target::Fx(i, d, p));
                                ui.close();
                            }
                        }
                    },
                );
            }
        }
        ui.separator();
        // one submenu per channel; grouped by 8 when there are more
        let chan_menu = |ui: &mut egui::Ui, c: usize, add: &mut Option<Target>| {
            ui.menu_button(format!("Mixer ch {}", c + 1), |ui| {
                for t in [
                    Target::ChanVolume(c),
                    Target::ChanPan(c),
                    Target::Send(c, 0),
                    Target::Send(c, 1),
                ] {
                    let name = match t {
                        Target::Send(_, 0) => "Send A",
                        Target::Send(..) => "Send B",
                        _ => t.param_name(),
                    };
                    if ui.button(name).clicked() {
                        *add = Some(t);
                        ui.close();
                    }
                }
            });
        };
        if nch <= 8 {
            for c in 0..nch {
                chan_menu(ui, c, add);
            }
        } else {
            for g in (0..nch).step_by(8) {
                let e = (g + 8).min(nch);
                ui.menu_button(format!("Mixer ch {}–{}", g + 1, e), |ui| {
                    for c in g..e {
                        chan_menu(ui, c, add);
                    }
                });
            }
        }
        for (r, k) in [crate::fx::FxKind::Delay, crate::fx::FxKind::Reverb]
            .into_iter()
            .enumerate()
        {
            ui.menu_button(
                format!("Return {} ({})", if r == 0 { "A" } else { "B" }, k.name()),
                |ui| {
                    for (p, info) in crate::fx::params(k).iter().enumerate() {
                        if ui.button(info.name).clicked() {
                            *add = Some(Target::Ret(r, p));
                            ui.close();
                        }
                    }
                },
            );
        }
        if !routes.is_empty() {
            ui.menu_button("Modulation depths", |ui| {
                for (r, name) in routes {
                    if ui.button(name).clicked() {
                        *add = Some(Target::ModAmount(*r));
                        ui.close();
                    }
                }
            });
        }
        for t in [Target::Master, Target::Tempo] {
            if track && t == Target::Tempo {
                continue;
            }
            if ui.button(t.param_name()).clicked() {
                *add = Some(t);
                ui.close();
            }
        }
    });
}
