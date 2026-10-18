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

    /// Master limiter controls and the loudness readout (src/master.rs).
    pub(super) fn master_panel(
        ui: &mut egui::Ui,
        lim: &mut crate::master::LimiterSettings,
        loud: &crate::master::Readout,
    ) {
        let fmt = |v: f32, unit: &str| {
            if v.is_finite() && v > -70.0 {
                format!("{v:.1} {unit}")
            } else {
                format!("–– {unit}")
            }
        };
        egui::Frame::new()
            .fill(th().cart_top)
            .stroke(Stroke::new(1.0, th().accent))
            .corner_radius(8)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.set_width(220.0);
                ui.spacing_mut().slider_width = 100.0;
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Limiter & loudness")
                            .strong()
                            .color(th().ink),
                    );
                    ui.checkbox(&mut lim.enabled, "True-peak limiter")
                        .on_hover_text(
                            "lookahead brickwall limiter on the master and in every export: \
                         the output never goes over the ceiling, even between samples \
                         (4x oversampled true peak). Off = the classic soft clipper.",
                        );
                    ui.add_enabled_ui(lim.enabled, |ui| {
                        egui::Grid::new("limiter_grid")
                            .num_columns(2)
                            .show(ui, |ui| {
                                ui.label("Gain");
                                ui.add(
                                    egui::Slider::new(&mut lim.gain_db, 0.0..=24.0)
                                        .suffix(" dB")
                                        .fixed_decimals(1),
                                )
                                .on_hover_text(
                                    "drives the mix into the limiter: louder, more limiting",
                                );
                                ui.end_row();
                                ui.label("Ceiling");
                                ui.add(
                                    egui::Slider::new(&mut lim.ceiling_db, -12.0..=0.0)
                                        .suffix(" dBTP")
                                        .fixed_decimals(1),
                                )
                                .on_hover_text(
                                    "maximum true peak; −1 dBTP is safe for MP3 and streaming",
                                );
                                ui.end_row();
                                ui.label("Release");
                                ui.add(
                                    egui::Slider::new(&mut lim.release_ms, 10.0..=1000.0)
                                        .logarithmic(true)
                                        .suffix(" ms")
                                        .fixed_decimals(0),
                                )
                                .on_hover_text("how fast the gain comes back after a peak");
                                ui.end_row();
                            });
                        let gr = loud.gain_red;
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("GR").small().color(th().ink2));
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(150.0, 8.0), Sense::hover());
                            ui.painter().rect_filled(r, 3, th().crt_bot);
                            let f = (-gr / 12.0).clamp(0.0, 1.0);
                            if f > 0.0 {
                                let mut g = r;
                                g.set_width(r.width() * f);
                                ui.painter().rect_filled(g, 3, hex(0xE8C35A));
                            }
                            ui.label(
                                egui::RichText::new(format!("{gr:.1}"))
                                    .monospace()
                                    .small()
                                    .color(th().value),
                            );
                        })
                        .response
                        .on_hover_text("gain reduction right now (dB)");
                    });
                    ui.separator();
                    egui::Grid::new("loud_grid").num_columns(2).show(ui, |ui| {
                        for (name, v, unit, tip) in [
                            (
                                "Momentary",
                                loud.momentary,
                                "LUFS",
                                "loudness of the last 0.4 s (ITU-R BS.1770)",
                            ),
                            (
                                "Short-term",
                                loud.short_term,
                                "LUFS",
                                "loudness of the last 3 s",
                            ),
                            (
                                "Integrated",
                                loud.integrated,
                                "LUFS",
                                "gated loudness since Play was pressed",
                            ),
                            (
                                "True peak",
                                loud.true_peak,
                                "dBTP",
                                "highest true peak since Play was pressed",
                            ),
                        ] {
                            ui.label(egui::RichText::new(name).small().color(th().ink2))
                                .on_hover_text(tip);
                            ui.label(
                                egui::RichText::new(fmt(v, unit))
                                    .monospace()
                                    .color(th().value),
                            );
                            ui.end_row();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Target").small().color(th().ink2));
                        let cur = crate::master::TARGETS
                            .iter()
                            .find(|t| t.0 == lim.target_lufs)
                            .map(|t| t.1)
                            .unwrap_or("custom");
                        egui::ComboBox::from_id_salt("lufs_target")
                            .width(150.0)
                            .selected_text(cur)
                            .show_ui(ui, |ui| {
                                for (v, n) in crate::master::TARGETS {
                                    ui.selectable_value(&mut lim.target_lufs, v, n);
                                }
                            });
                    });
                    let g = crate::master::guidance(loud.integrated, lim.target_lufs);
                    if !g.is_empty() {
                        ui.label(egui::RichText::new(g).small().color(th().ink2));
                    }
                });
            });
    }

    #[allow(clippy::needless_range_loop)]
    pub(super) fn mixer_view(&mut self, ui: &mut egui::Ui) {
        let (mut mix, names, nch, cpk, plr, loud) = self.with(|s| {
            (
                s.song.mixer.clone(),
                s.song.channel_names.clone(),
                s.song.channels,
                s.engine.chan_peak,
                s.engine.peak_lr,
                s.engine.loud,
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
        // master section pinned on the right, channels + returns scroll.
        // Its real width is measured each frame (egui memory), so the scroll
        // area gets every pixel left of it minus a clear gap; before 0.2.4.x a
        // fixed 400 px guess left the last return strip cut off right next to
        // Master on wide screens, looking hidden behind it.
        let master_id = ui.make_persistent_id("mixer_master_w");
        let master_w: f32 = ui
            .ctx()
            .data(|d| d.get_temp(master_id))
            .unwrap_or(MIXER_MASTER_W_GUESS);
        let nret = mix.returns.len();
        let strip = |ui: &mut egui::Ui,
                     title: String,
                     spine: Color32,
                     vol: &mut f32,
                     levels: &[f32],
                     target: Option<crate::automation::Target>,
                     extra: &mut dyn FnMut(&mut egui::Ui)|
         -> Rect {
            egui::Frame::new()
                .fill(th().cart_top)
                .stroke(Stroke::new(1.0, spine))
                .corner_radius(8)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.set_width(92.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new(title).strong().color(th().ink));
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
                })
                .response
                .rect
        };
        ui.horizontal_top(|ui| {
        // right edges of the strips (from the content's left), last frame
        let edges_id = ui.make_persistent_id("mixer_strip_edges");
        let last_edges: Vec<f32> = ui.ctx().data(|d| d.get_temp(edges_id)).unwrap_or_default();
        let room = mixer_scroll_width(ui.available_width(), master_w);
        let view_w = mixer_view_width(room, &last_edges);
        let mut edges: Vec<f32> = Vec::with_capacity(nch + nret);
        let mut area = egui::ScrollArea::horizontal()
            .id_salt("mixer_scroll")
            .max_width(view_w);
        if let Some(x) = self.mix_scroll.take() {
            area = area.horizontal_scroll_offset(x);
        }
        let scrolled = area.show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let x0 = ui.cursor().left();
                    let mut edge = |r: Rect| edges.push(r.right() - x0);
                    for ch in 0..nch.min(MAX_CHANNELS) {
                        let c = &mut mix.channels[ch];
                        let mut vol = c.volume;
                        let (mut pan, mut mute, mut solo) = (c.pan, c.mute, c.solo);
                        let mut send = c.send;
                        let mut duck = c.duck;
                        let is_key = mix.duck_key == ch;
                        edge(strip(
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
                        ));
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
                        edge(strip(
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
                                    &["size", "damping", "predelay", "lowcut", "highcut"]
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
                        ));
                        mix.returns[b].level = lvl;
                        mix.returns[b].device = dev;
                    }
                });
            });
        // our own bar under the strips (egui's floats in only on hover):
        // shows which channels are in view; click or drag to scroll
        let (inner, content, off) = (
            scrolled.inner_rect,
            scrolled.content_size.x,
            scrolled.state.clamped_offset().x,
        );
        // right under the strips (the area itself reaches the panel bottom)
        let bar_y = (inner.top() + scrolled.content_size.y + 8.0).min(inner.bottom() - 20.0);
        self.mix_bar = egui::Rect::NOTHING;
        // screenshot harness: CHIPWAVE_SHOT_MIXSCROLL=end scrolls to the last strip
        let end_id = egui::Id::unique("mixer_scroll_to_end");
        if ui.ctx().data(|d| d.get_temp::<bool>(end_id)).unwrap_or(false)
            && content > inner.width() + 1.0
            && edges == last_edges
        {
            ui.ctx().data_mut(|d| d.remove::<bool>(end_id));
            self.mix_scroll = Some(content - inner.width());
            ui.ctx().request_repaint();
        }
        if content > inner.width() + 1.0 {
            let bar = egui::Rect::from_min_size(
                egui::pos2(inner.left(), bar_y),
                egui::vec2(inner.width(), 16.0),
            );
            self.mix_bar = bar;
            let r = ui.interact(bar, ui.make_persistent_id("mixer_bar"), Sense::click_and_drag());
            let w = bar.width();
            if r.is_pointer_button_down_on() {
                if let Some(p) = r.interact_pointer_pos() {
                    let f = ((p.x - bar.left()) / w).clamp(0.0, 1.0);
                    let x = (f * content - inner.width() / 2.0).clamp(0.0, content - inner.width());
                    self.mix_scroll = Some(x);
                    ui.ctx().request_repaint();
                }
            }
            let p = ui.painter_at(bar);
            p.rect_filled(bar, 4, th().crt_grid);
            let thumb = egui::Rect::from_min_size(
                egui::pos2(bar.left() + off / content * w, bar.top()),
                egui::vec2(w * inner.width() / content, bar.height()),
            );
            p.rect_filled(thumb, 4, th().slate);
            let (a, b) = mixer_visible_strips(&edges, off, inner.width());
            p.text(
                bar.center(),
                egui::Align2::CENTER_CENTER,
                format!(
                    "{} · drag / Shift+wheel to scroll",
                    mixer_bar_label(a, b, nch, nret)
                ),
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
        if edges != last_edges {
            ui.ctx().data_mut(|d| d.insert_temp(edges_id, edges));
            ui.ctx().request_repaint();
        }
        // when the strips scroll, Master stays pinned on the right: the room
        // left over by showing only whole strips goes before the divider
        if content > inner.width() + 1.0 {
            ui.add_space((room - inner.width()).max(0.0));
        }
        // a clear gap with a divider between the scrolling strips and Master
        ui.add_space(MIXER_GAP / 2.0 - ui.spacing().item_spacing.x);
        let (div, _) = ui.allocate_exact_size(
            egui::vec2(1.0, (inner.height()).max(40.0)),
            Sense::hover(),
        );
        ui.painter().vline(
            div.center().x,
            div.top() + 4.0..=div.top() + scrolled.content_size.y.min(div.height()),
            Stroke::new(1.0, th().crt_grid),
        );
        ui.add_space(MIXER_GAP / 2.0 - ui.spacing().item_spacing.x);
        let master_start = ui.cursor().left();
                    let mut master = mix.master;
                    strip(
                        ui,
                        "Master".into(),
                        th().accent,
                        &mut master,
                        &plr,
                        Some(crate::automation::Target::Master),
                        &mut |ui| {
                            let t = if mix.limiter.enabled {
                                format!("limit {:.1} dBTP", mix.limiter.ceiling_db)
                            } else {
                                "soft clip".into()
                            };
                            ui.label(egui::RichText::new(t).small().color(th().ink2));
                        },
                    );
                    mix.master = master;
                    ui.add_space(8.0);
                    Self::master_panel(ui, &mut mix.limiter, &loud);
                    // measured width of the master section, for the next frame
                    let used = ui.min_rect().right() - master_start;
                    if (used - master_w).abs() > 0.5 {
                        ui.ctx().data_mut(|d| d.insert_temp(master_id, used));
                        ui.ctx().request_repaint();
                    }
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

/// first-frame guess for the master section (Master strip + limiter panel)
const MIXER_MASTER_W_GUESS: f32 = 372.0;
/// gap (with a divider line) between the scrolling strips and Master
pub(super) const MIXER_GAP: f32 = 24.0;

/// width for the strip scroll area: everything left of the master section and
/// its gap (never under 200 px, so very narrow windows still show a strip)
pub(super) fn mixer_scroll_width(avail: f32, master_w: f32) -> f32 {
    (avail - master_w - MIXER_GAP).max(200.0)
}

/// visible width of the strip area. `edges` are the strips' right edges
/// (from the left of the strip row, last frame). If they all fit it is just
/// `room`; otherwise it ends at the last strip that fits whole, so the right
/// edge never slices a strip in half (which looked like Ret B hiding behind
/// Master). Never under one strip.
pub(super) fn mixer_view_width(room: f32, edges: &[f32]) -> f32 {
    match edges.last() {
        Some(&end) if end > room + 0.5 => edges
            .iter()
            .copied()
            .filter(|&e| e <= room + 0.5)
            .fold(edges[0].min(room), f32::max),
        _ => room,
    }
}

/// 1-based (first, last) strips wholly in view; `edges` are the strips'
/// right edges from the left of the strip row, `off` the scroll offset
pub(super) fn mixer_visible_strips(edges: &[f32], off: f32, width: f32) -> (usize, usize) {
    let mut left = 0.0;
    let (mut a, mut b) = (0, 0);
    for (i, &e) in edges.iter().enumerate() {
        if left >= off - 1.0 && e <= off + width + 1.0 {
            if a == 0 {
                a = i + 1;
            }
            b = i + 1;
        }
        left = e;
    }
    if a == 0 {
        // nothing whole in view (a tiny window): the strip under the left edge
        let i = edges.iter().position(|&e| e > off).unwrap_or(0);
        (i + 1, i + 1)
    } else {
        (a, b)
    }
}

/// "channels 3–8 of 8 + Ret A" style text for the strip bar: strips `a..=b`
/// (1-based) of `nch` channels followed by `nret` return strips
pub(super) fn mixer_bar_label(a: usize, b: usize, nch: usize, nret: usize) -> String {
    let ret = |i: usize| format!("Ret {}", (b'A' + i as u8) as char);
    let mut s = if a <= nch {
        format!("channels {}–{} of {}", a, b.min(nch), nch)
    } else {
        format!("{} channels", nch)
    };
    if b > nch {
        let r0 = a.saturating_sub(nch + 1);
        let r1 = (b - nch - 1).min(nret.saturating_sub(1));
        if r0 == r1 {
            s += &format!(" + {}", ret(r1));
        } else {
            s += &format!(" + {}–{}", ret(r0), (b'A' + r1 as u8) as char);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_label_names_channels_and_returns() {
        assert_eq!(mixer_bar_label(1, 6, 8, 2), "channels 1–6 of 8");
        assert_eq!(mixer_bar_label(3, 9, 8, 2), "channels 3–8 of 8 + Ret A");
        assert_eq!(mixer_bar_label(4, 10, 8, 2), "channels 4–8 of 8 + Ret A–B");
        assert_eq!(mixer_bar_label(10, 10, 8, 2), "8 channels + Ret B");
    }

    #[test]
    fn view_width_shows_whole_strips_only_when_scrolling() {
        let edges = [110.0, 228.0, 346.0, 476.0, 620.0];
        // everything fits (or nothing measured yet): all the room
        assert_eq!(mixer_view_width(700.0, &edges), 700.0);
        assert_eq!(mixer_view_width(300.0, &[]), 300.0);
        // overflow: up to the last whole strip, even with uneven widths
        assert_eq!(mixer_view_width(600.0, &edges), 476.0);
        assert_eq!(mixer_view_width(476.0, &edges), 476.0);
        assert_eq!(mixer_view_width(475.0, &edges), 346.0);
        // tiny windows still show (at most) one strip
        assert_eq!(mixer_view_width(100.0, &edges), 100.0);
    }

    #[test]
    fn visible_strips_counts_whole_strips() {
        let edges = [110.0, 228.0, 346.0, 476.0, 620.0];
        assert_eq!(mixer_visible_strips(&edges, 0.0, 346.0), (1, 3));
        assert_eq!(mixer_visible_strips(&edges, 110.0, 366.0), (2, 4));
        assert_eq!(mixer_visible_strips(&edges, 300.0, 320.0), (4, 5));
        assert_eq!(mixer_visible_strips(&edges, 0.0, 50.0), (1, 1));
    }

    #[test]
    fn scroll_width_leaves_master_and_gap() {
        assert_eq!(
            mixer_scroll_width(1500.0, 372.0),
            1500.0 - 372.0 - MIXER_GAP
        );
        assert_eq!(mixer_scroll_width(300.0, 372.0), 200.0);
    }
}
