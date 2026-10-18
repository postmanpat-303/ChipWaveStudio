//! Automation lanes: lane edit operations, adding lanes and the automation bar.
use super::*;

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
                        lane.points.push(Point { pos, value, shape });
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
                            lane.points.push(Point { pos, value, shape });
                            lane.sort();
                        }
                    });
                }
            }
            self.dirty = true;
        }
    }

    pub(super) fn add_lane(&mut self, t: crate::automation::Target) {
        use crate::automation::{Lane, Target};
        let label = self.with(|s| {
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
                                dev.kind.name(),
                                crate::fx::params(dev.kind)
                                    .get(p)
                                    .map(|x| x.name)
                                    .unwrap_or("")
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
                Target::Send(c, r) => {
                    format!(
                        "Ch {} {} - Send {}",
                        c + 1,
                        cname(c),
                        if r == 0 { "A" } else { "B" }
                    )
                }
                _ => t.param_name().to_string(),
            }
        });
        self.snapshot();
        let n = self.with(|s| {
            // start flat at the current value
            let cur = match t {
                Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) => {
                    match s.song.instruments.get(i).and_then(|x| x.kind.patch()) {
                        Some(p) => t.to_norm(match t {
                            Target::Cutoff(_) => p.cutoff,
                            Target::Resonance(_) => p.resonance,
                            _ => p.drive,
                        }),
                        _ => 0.5,
                    }
                }
                Target::Level(_) => t.to_norm(1.0),
                Target::ChanVolume(c) => t.to_norm(s.song.mixer.channels[c].volume),
                Target::ChanPan(c) => t.to_norm(s.song.mixer.channels[c].pan),
                Target::Master => t.to_norm(s.song.mixer.master),
                Target::Tempo => t.to_norm(s.song.bpm),
                Target::Fx(i, d, p) => s
                    .song
                    .instruments
                    .get(i)
                    .and_then(|x| x.fx.get(d))
                    .and_then(|dev| dev.params.get(p).copied())
                    .unwrap_or(0.5),
                Target::Ret(r, p) => s
                    .song
                    .mixer
                    .returns
                    .get(r)
                    .and_then(|x| x.device.params.get(p).copied())
                    .unwrap_or(0.5),
                Target::Send(c, r) => s
                    .song
                    .mixer
                    .channels
                    .get(c)
                    .map(|m| m.send[r])
                    .unwrap_or(0.0),
            };
            let mut lane = Lane::new(t, label.clone());
            lane.points.push(crate::automation::Point {
                pos: 0.0,
                value: cur,
                shape: crate::automation::Shape::Linear,
            });
            s.song.automation.push(lane);
            s.song.automation.len() - 1
        });
        self.auto_lane = n;
        self.show_auto = true;
        self.status = format!("Added lane '{label}' (click to add points, Pencil to draw)");
    }

    pub(super) fn automation_bar(&mut self, ui: &mut egui::Ui) {
        use crate::automation::{Shape, Target};
        let fxdevs = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| i.fx.iter().map(|d| d.kind).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        });
        let (ninst, synths, nch, nl) = self.with(|s| {
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
        let _ = ninst;
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
                ui.set_max_height(320.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, (n, synth)) in synths.iter().enumerate() {
                        let mut ts = vec![Target::Level(i)];
                        if *synth {
                            ts = vec![
                                Target::Cutoff(i),
                                Target::Resonance(i),
                                Target::Drive(i),
                                Target::Level(i),
                            ];
                        }
                        for t in ts {
                            if ui
                                .button(format!("{:02X} {n} - {}", i + 1, t.param_name()))
                                .clicked()
                            {
                                add = Some(t);
                                ui.close();
                            }
                        }
                        for (d, k) in fxdevs.get(i).into_iter().flatten().enumerate() {
                            ui.menu_button(
                                format!("{:02X} {n} - FX {} {}", i + 1, d + 1, k.name()),
                                |ui| {
                                    for (p, info) in crate::fx::params(*k).iter().enumerate() {
                                        if ui.button(info.name).clicked() {
                                            add = Some(Target::Fx(i, d, p));
                                            ui.close();
                                        }
                                    }
                                },
                            );
                        }
                    }
                    ui.separator();
                    for c in 0..nch {
                        for t in [Target::ChanVolume(c), Target::ChanPan(c)] {
                            if ui
                                .button(format!("Mixer ch {} - {}", c + 1, t.param_name()))
                                .clicked()
                            {
                                add = Some(t);
                                ui.close();
                            }
                        }
                    }
                    for c in 0..nch {
                        for r in 0..2 {
                            let t = Target::Send(c, r);
                            if ui
                                .button(format!(
                                    "Mixer ch {} - Send {}",
                                    c + 1,
                                    if r == 0 { "A" } else { "B" }
                                ))
                                .clicked()
                            {
                                add = Some(t);
                                ui.close();
                            }
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
                                        add = Some(Target::Ret(r, p));
                                        ui.close();
                                    }
                                }
                            },
                        );
                    }
                    for t in [Target::Master, Target::Tempo] {
                        if ui.button(t.param_name()).clicked() {
                            add = Some(t);
                            ui.close();
                        }
                    }
                });
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
