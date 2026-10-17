//! Mixer view and level meters.
use super::*;

impl App {
    pub(super) fn meter(ui: &mut egui::Ui, level: f32, h: f32) {
        let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, h), Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, 3, th().crt_bot);
        let db = if level > 1e-5 {
            20.0 * level.log10()
        } else {
            -90.0
        };
        let frac = ((db + 60.0) / 66.0).clamp(0.0, 1.0);
        if frac > 0.0 {
            let top = r.bottom() - frac * r.height();
            let col = if db > 0.0 {
                hex(0xD9605A)
            } else if db > -6.0 {
                hex(0xE8C35A)
            } else {
                hex(0x5CC07A)
            };
            p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), top), r.max), 3, col);
        }
        // 0 dB line
        let y0 = r.bottom() - 60.0 / 66.0 * r.height();
        p.hline(r.x_range(), y0, Stroke::new(1.0, th().ink2));
    }

    #[allow(clippy::needless_range_loop)]
    pub(super) fn mixer_view(&mut self, ui: &mut egui::Ui) {
        let (mut mix, names, nch, cpk, plr) = self.with(|s| {
            (
                s.song.mixer.clone(),
                s.song.channel_names.clone(),
                s.song.channels,
                s.engine.chan_peak,
                s.engine.peak_lr,
            )
        });
        let before = mix.clone();
        // (response, target) of controls that get a MIDI learn menu
        let learn =
            std::cell::RefCell::new(Vec::<(egui::Response, crate::automation::Target)>::new());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Mixer").strong());
            ui.separator();
            ui.label("Sidechain key")
                .on_hover_text("notes on this channel duck every channel whose Duck is above 0 %");
            egui::ComboBox::from_id_salt("duck_key")
                .width(110.0)
                .selected_text(format!(
                    "{} {}",
                    mix.duck_key + 1,
                    names.get(mix.duck_key).cloned().unwrap_or_default()
                ))
                .show_ui(ui, |ui| {
                    for k in 0..nch.min(MAX_CHANNELS) {
                        ui.selectable_value(
                            &mut mix.duck_key,
                            k,
                            format!("{} {}", k + 1, names.get(k).cloned().unwrap_or_default()),
                        );
                    }
                });
            let mut ms = mix.duck_release * 1000.0;
            if ui
                .add(
                    egui::DragValue::new(&mut ms)
                        .range(20.0..=1000.0)
                        .speed(2.0)
                        .prefix("release ")
                        .suffix(" ms"),
                )
                .on_hover_text("how long a ducked channel takes to come back")
                .changed()
            {
                mix.duck_release = ms / 1000.0;
            }
            ui.separator();
            ui.label(
                egui::RichText::new(
                    "fader 0 dB = double-click · pan C = double-click · M mute · S solo (F9 / F10)",
                )
                .small()
                .color(th().ink2),
            );
        });
        ui.add_space(6.0);
        let fader_h = (ui.available_height() - 150.0).clamp(80.0, 320.0);
        egui::ScrollArea::horizontal()
            .id_salt("mixer_scroll")
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let strip =
                        |ui: &mut egui::Ui,
                         title: String,
                         spine: Color32,
                         vol: &mut f32,
                         levels: &[f32],
                         target: Option<crate::automation::Target>,
                         extra: &mut dyn FnMut(&mut egui::Ui)| {
                            egui::Frame::new()
                                .fill(th().cart_top)
                                .stroke(Stroke::new(1.0, spine))
                                .corner_radius(8)
                                .inner_margin(8)
                                .show(ui, |ui| {
                                    ui.set_width(92.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new(title).strong().color(th().ink),
                                        );
                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().slider_width = fader_h;
                                            ui.add_space(8.0);
                                            // sliders only sense drags: add clicks
                                            // so double-click resets work
                                            let r = ui
                                                .add(
                                                    egui::Slider::new(vol, 0.0..=2.0)
                                                        .vertical()
                                                        .trailing_fill(true)
                                                        .show_value(false),
                                                )
                                                .interact(Sense::click());
                                            if r.double_clicked() {
                                                *vol = 1.0;
                                            }
                                            if let Some(t) = target {
                                                learn.borrow_mut().push((r, t));
                                            }
                                            for l in levels {
                                                Self::meter(ui, *l, fader_h);
                                            }
                                        });
                                        ui.label(
                                            egui::RichText::new(db_text(*vol))
                                                .monospace()
                                                .small()
                                                .color(th().value),
                                        );
                                        extra(ui);
                                    });
                                });
                        };
                    for ch in 0..nch.min(MAX_CHANNELS) {
                        let c = &mut mix.channels[ch];
                        let mut vol = c.volume;
                        let (mut pan, mut mute, mut solo) = (c.pan, c.mute, c.solo);
                        let mut send = c.send;
                        let mut duck = c.duck;
                        let is_key = mix.duck_key == ch;
                        strip(
                            ui,
                            format!("{} {}", ch + 1, names.get(ch).cloned().unwrap_or_default()),
                            th().slate,
                            &mut vol,
                            &[cpk[ch]],
                            Some(crate::automation::Target::ChanVolume(ch)),
                            &mut |ui| {
                                ui.spacing_mut().slider_width = 76.0;
                                let r = ui
                                    .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false))
                                    .interact(Sense::click());
                                if r.double_clicked() {
                                    pan = 0.0;
                                }
                                learn
                                    .borrow_mut()
                                    .push((r, crate::automation::Target::ChanPan(ch)));
                                let pt = if pan.abs() < 0.01 {
                                    "C".to_string()
                                } else if pan < 0.0 {
                                    format!("L{:.0}", -pan * 100.0)
                                } else {
                                    format!("R{:.0}", pan * 100.0)
                                };
                                ui.label(egui::RichText::new(pt).monospace().small());
                                ui.horizontal(|ui| {
                                    ui.add_space(14.0);
                                    ui.toggle_value(&mut mute, egui::RichText::new(" M ").strong())
                                        .on_hover_text("mute (F9)");
                                    ui.toggle_value(&mut solo, egui::RichText::new(" S ").strong())
                                        .on_hover_text("solo (F10)");
                                });
                                ui.spacing_mut().slider_width = 56.0;
                                for (b, nm) in ["A", "B"].iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(*nm)
                                                .small()
                                                .monospace()
                                                .color(th().ink2),
                                        );
                                        let r = ui
                                            .add(
                                                egui::Slider::new(&mut send[b], 0.0..=1.0)
                                                    .show_value(false),
                                            )
                                            .interact(Sense::click())
                                            .on_hover_text(format!(
                                                "send {} ({}): {:.0} %",
                                                nm,
                                                if b == 0 { "delay" } else { "reverb" },
                                                send[b] * 100.0
                                            ));
                                        if r.double_clicked() {
                                            send[b] = 0.0;
                                        }
                                        learn
                                            .borrow_mut()
                                            .push((r, crate::automation::Target::Send(ch, b)));
                                    });
                                }
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("Duck")
                                            .small()
                                            .monospace()
                                            .color(th().ink2),
                                    );
                                    let mut pct = duck * 100.0;
                                    let r = ui
                                        .add_enabled(
                                            !is_key,
                                            egui::DragValue::new(&mut pct)
                                                .range(0.0..=100.0)
                                                .speed(0.5)
                                                .suffix(" %"),
                                        )
                                        .on_hover_text(
                                            "sidechain ducking: how far this channel dips on each \
                                             note of the key channel (the 'pump'); double-click = 0",
                                        )
                                        .on_disabled_hover_text("this is the sidechain key channel");
                                    if r.changed() {
                                        duck = pct / 100.0;
                                    }
                                    if r.double_clicked() {
                                        duck = 0.0;
                                    }
                                });
                            },
                        );
                        let c = &mut mix.channels[ch];
                        c.duck = duck;
                        c.send = send;
                        c.volume = vol;
                        c.pan = pan;
                        c.mute = mute;
                        c.solo = solo;
                    }
                    ui.add_space(12.0);
                    // return buses (delay / reverb), 100 % wet
                    for b in 0..mix.returns.len() {
                        let mut lvl = mix.returns[b].level;
                        let mut dev = mix.returns[b].device.clone();
                        strip(
                            ui,
                            format!("Ret {}", mix.returns[b].name),
                            fxview::strip_fill(dev.kind, dev.enabled),
                            &mut lvl,
                            &[],
                            None,
                            &mut |ui| {
                                ui.spacing_mut().slider_width = 76.0;
                                let keys: &[&str] = if b == 0 {
                                    &["sync", "feedback", "lp"]
                                } else {
                                    &["size", "damping", "predelay"]
                                };
                                for key in keys {
                                    let Some(k) = crate::fx::Device::param_index(dev.kind, key)
                                    else {
                                        continue;
                                    };
                                    let info = &crate::fx::params(dev.kind)[k];
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} {}",
                                            info.name,
                                            info.format(dev.params[k])
                                        ))
                                        .small()
                                        .color(th().ink2),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut dev.params[k], 0.0..=1.0)
                                            .show_value(false),
                                    );
                                }
                                ui.toggle_value(&mut dev.enabled, "On");
                            },
                        );
                        mix.returns[b].level = lvl;
                        mix.returns[b].device = dev;
                    }
                    ui.add_space(12.0);
                    let mut master = mix.master;
                    strip(
                        ui,
                        "Master".into(),
                        th().accent,
                        &mut master,
                        &plr,
                        Some(crate::automation::Target::Master),
                        &mut |ui| {
                            ui.label(egui::RichText::new("soft limiter").small().color(th().ink2));
                        },
                    );
                    mix.master = master;
                });
            });
        if mix != before {
            self.with(|s| s.song.mixer = mix);
            self.dirty = true;
        }
        for (r, t) in learn.into_inner() {
            self.learn_menu(&r, t);
        }
    }
}
