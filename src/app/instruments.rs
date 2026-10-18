//! Left side panel: instrument list, sample import button and the patch editor.
use super::*;

impl App {
    pub(super) fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Instruments").strong().size(15.0));
        });
        let names: Vec<(String, &str)> = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| {
                    (
                        i.name.clone(),
                        match i.kind {
                            InstrumentKind::Sample { .. } => "  (sample)",
                            InstrumentKind::Voice(_) => "  (voice)",
                            InstrumentKind::Synth(_) => "",
                        },
                    )
                })
                .collect()
        });
        egui::ScrollArea::vertical()
            .id_salt("inst")
            .max_height(260.0)
            .show(ui, |ui| {
                for (i, (n, tag)) in names.iter().enumerate() {
                    let slot = (i + 1) as u8;
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        let txt =
                            egui::RichText::new(format!("{:02X}  {}{}", slot, n, tag)).monospace();
                        let r = ui.selectable_label(self.instr == slot, txt);
                        if r.clicked() {
                            self.instr = slot;
                        }
                        // a newly selected instrument (duplicate, Up/Down, MIDI...) scrolls into view
                        if self.instr == slot && self.instr_shown != slot {
                            r.scroll_to_me(None);
                            self.instr_shown = slot;
                        }
                        r.context_menu(|ui| {
                            ui.menu_button("Add effect", |ui| {
                                for k in crate::fx::FxKind::ALL {
                                    if ui.button(k.name()).clicked() {
                                        self.add_effect(slot, k);
                                        ui.close();
                                    }
                                }
                            });
                            if ui.button("Copy effects").clicked() {
                                self.copy_effects(slot);
                                ui.close();
                            }
                            let has_clip = self.fx_clip.is_some();
                            if ui
                                .add_enabled(has_clip, egui::Button::new("Paste effects"))
                                .on_hover_text(
                                    "replace this instrument's FX chain with the copied one",
                                )
                                .clicked()
                            {
                                self.paste_effects(slot, false);
                                ui.close();
                            }
                            if ui
                                .add_enabled(has_clip, egui::Button::new("Paste effects (append)"))
                                .on_hover_text("add the copied devices after this instrument's own")
                                .clicked()
                            {
                                self.paste_effects(slot, true);
                                ui.close();
                            }
                            if ui.button("Open in FX view").clicked() {
                                self.instr = slot;
                                self.view = View::Fx;
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Duplicate instrument").clicked() {
                                self.duplicate_instrument(slot);
                                ui.close();
                            }
                            if ui.button("Delete instrument…").clicked() {
                                self.request_delete_instrument(slot);
                                ui.close();
                            }
                            ui.separator();
                            ui.menu_button("New instrument", |ui| self.new_instrument_menu(ui));
                        });
                    });
                }
            });
        let has = (self.instr as usize).wrapping_sub(1) < names.len();
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            if ui
                .add_enabled(has, egui::Button::new("Duplicate").small())
                .on_hover_text("Duplicate the selected instrument (with its FX chain); also right-click an instrument or use Edit menu")
                .clicked()
            {
                self.duplicate_instrument(self.instr);
            }
            if ui
                .add_enabled(has, egui::Button::new("Delete…").small())
                .on_hover_text("Delete the selected instrument; its notes are removed and later instruments move up one slot (Ctrl+Z undoes)")
                .clicked()
            {
                self.request_delete_instrument(self.instr);
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.menu_button("New…", |ui| self.new_instrument_menu(ui))
                .response
                .on_hover_text("add a Synth or a Voice (glitch) instrument");
            if ui.button("Import WAV…").clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("WAV", &["wav"])
                    .pick_file()
                {
                    self.import_sample(p);
                }
            }
            ui.label(
                egui::RichText::new("or drop .wav files")
                    .small()
                    .color(th().ink2),
            );
        });
        ui.separator();
        // patch editor for the selected synth instrument
        let idx = self.instr as usize;
        let mut changed = false;
        let Some((name, mut volume, mut transpose, mut patch, mut voice)) = self.with(|s| {
            s.song.instruments.get(idx.wrapping_sub(1)).map(|i| {
                let (patch, voice) = match &i.kind {
                    InstrumentKind::Synth(p) => (Some(p.clone()), None),
                    InstrumentKind::Voice(v) => (Some(v.base.clone()), Some(v.clone())),
                    _ => (None, None),
                };
                (i.name.clone(), i.volume, i.transpose, patch, voice)
            })
        }) else {
            return;
        };
        let is_voice = voice.is_some();
        let mut learn: Vec<(egui::Response, crate::automation::Target)> = vec![];
        // edit a copy without holding the audio lock, commit only on change
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new(format!("Sound · {}", name)).strong());
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Volume");
            let r = ui.add(egui::Slider::new(&mut volume, 0.0..=1.0));
            changed |= r.changed();
            learn.push((r, crate::automation::Target::Level(idx.wrapping_sub(1))));
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Transpose");
            for (txt, d) in [("-12", -12), (" - ", -1)] {
                if ui.button(txt).clicked() {
                    transpose = (transpose as i32 + d).clamp(-48, 48) as i8;
                    changed = true;
                }
            }
            ui.label(
                egui::RichText::new(format!("{:+} st", transpose))
                    .monospace()
                    .color(th().value),
            );
            for (txt, d) in [(" + ", 1), ("+12", 12)] {
                if ui.button(txt).clicked() {
                    transpose = (transpose as i32 + d).clamp(-48, 48) as i8;
                    changed = true;
                }
            }
        });
        if let Some(v) = &mut voice {
            changed |= super::voiceview::voice_panel(ui, v);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("Envelope · filter · LFO")
                        .small()
                        .color(th().ink2),
                );
            });
        }
        if let Some(p) = &mut patch {
            egui::Grid::new("patch")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    if !is_voice {
                        ui.label("   Osc");
                        egui::ComboBox::from_id_salt("wave")
                            .selected_text(format!("{:?}", p.osc1))
                            .show_ui(ui, |ui| {
                                for w in [
                                    Wave::Saw,
                                    Wave::Square,
                                    Wave::Pulse,
                                    Wave::Triangle,
                                    Wave::Sine,
                                ] {
                                    changed |= ui
                                        .selectable_value(&mut p.osc1, w, format!("{w:?}"))
                                        .changed();
                                }
                            });
                        ui.end_row();
                    }
                    ui.label("   Octave");
                    changed |= ui.add(egui::Slider::new(&mut p.osc1_oct, -3..=3)).changed();
                    ui.end_row();
                    if !is_voice
                        || voice
                            .as_ref()
                            .is_some_and(|v| v.source == crate::voice::VoiceSource::Pulse)
                    {
                        ui.label("   Pulse width");
                        changed |= ui
                            .add(egui::Slider::new(&mut p.pulse_width, 0.05..=0.95))
                            .changed();
                        ui.end_row();
                    }
                    if !is_voice {
                        ui.label("   Unison");
                        changed |= ui.add(egui::Slider::new(&mut p.unison, 1..=8)).changed();
                        ui.end_row();
                        ui.label("   Detune ct");
                        changed |= ui
                            .add(egui::Slider::new(&mut p.detune_cents, 0.0..=50.0))
                            .changed();
                        ui.end_row();
                    }
                    ui.label("   Filter");
                    egui::ComboBox::from_id_salt("fmode")
                        .selected_text(format!("{:?}", p.filter))
                        .show_ui(ui, |ui| {
                            for m in [FilterMode::Lp, FilterMode::Hp, FilterMode::Bp] {
                                changed |= ui
                                    .selectable_value(&mut p.filter, m, format!("{m:?}"))
                                    .changed();
                            }
                        });
                    ui.end_row();
                    ui.label("   Cutoff Hz");
                    let r =
                        ui.add(egui::Slider::new(&mut p.cutoff, 20.0..=18000.0).logarithmic(true));
                    changed |= r.changed();
                    learn.push((r, crate::automation::Target::Cutoff(idx.wrapping_sub(1))));
                    ui.end_row();
                    ui.label("   Resonance");
                    let r = ui.add(egui::Slider::new(&mut p.resonance, 0.0..=0.98));
                    changed |= r.changed();
                    learn.push((r, crate::automation::Target::Resonance(idx.wrapping_sub(1))));
                    ui.end_row();
                    ui.label("   Env amount");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.env_amount, -1.0..=1.0))
                        .changed();
                    ui.end_row();
                    ui.label("   Attack s");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.amp.a, 0.0..=2.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Decay s");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.amp.d, 0.001..=3.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Sustain");
                    changed |= ui.add(egui::Slider::new(&mut p.amp.s, 0.0..=1.0)).changed();
                    ui.end_row();
                    ui.label("   Release s");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.amp.r, 0.001..=4.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Filter decay");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.fenv.d, 0.001..=3.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Drive");
                    let r = ui.add(egui::Slider::new(&mut p.drive, 0.0..=1.0));
                    changed |= r.changed();
                    learn.push((r, crate::automation::Target::Drive(idx.wrapping_sub(1))));
                    ui.end_row();
                    if !is_voice {
                        ui.label("   Stereo width");
                        changed |= ui
                            .add(egui::Slider::new(&mut p.width, 0.0..=1.0))
                            .on_hover_text(
                                "spreads the unison voices left/right (Unison > 1); \
                                 with Noise it makes the noise stereo (hats)",
                            )
                            .changed();
                        ui.end_row();
                        ui.label("   Metal (808)");
                        changed |= ui
                            .add(egui::Slider::new(&mut p.metal, 0.0..=1.0))
                            .on_hover_text(
                                "six square oscillators at 808 hi-hat ratios: the metallic \
                                 part of a hat or cymbal (mix with Noise, high-pass filter)",
                            )
                            .changed();
                        ui.end_row();
                    }
                    ui.label("   Velocity > cutoff");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.vel_cutoff, 0.0..=4.0).suffix(" oct"))
                        .on_hover_text("softer notes (volume column) get a darker filter")
                        .changed();
                    ui.end_row();
                    ui.label("   Start delay");
                    let mut ms = p.start_delay * 1000.0;
                    if ui
                        .add(egui::Slider::new(&mut ms, 0.0..=50.0).suffix(" ms"))
                        .on_hover_text(
                            "plays every note this much behind the grid (laid-back hats / groove)",
                        )
                        .changed()
                    {
                        p.start_delay = ms / 1000.0;
                        changed = true;
                    }
                    ui.end_row();
                    ui.label("   LFO");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("lfo_shape")
                            .width(70.0)
                            .selected_text(format!("{:?}", p.lfo.shape))
                            .show_ui(ui, |ui| {
                                for w in [Wave::Sine, Wave::Triangle, Wave::Square, Wave::Saw] {
                                    changed |= ui
                                        .selectable_value(&mut p.lfo.shape, w, format!("{w:?}"))
                                        .changed();
                                }
                            });
                        const SYNC: [(f32, &str); 7] = [
                            (0.0, "free Hz"),
                            (0.25, "1/16"),
                            (0.5, "1/8"),
                            (1.0, "1/4"),
                            (2.0, "1/2"),
                            (4.0, "1 bar"),
                            (8.0, "2 bars"),
                        ];
                        let cur = SYNC
                            .iter()
                            .find(|s| s.0 == p.lfo.sync)
                            .map(|s| s.1)
                            .unwrap_or("sync");
                        egui::ComboBox::from_id_salt("lfo_sync")
                            .width(70.0)
                            .selected_text(cur)
                            .show_ui(ui, |ui| {
                                for (v, n) in SYNC {
                                    changed |= ui.selectable_value(&mut p.lfo.sync, v, n).changed();
                                }
                            });
                        if p.lfo.sync == 0.0 {
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut p.lfo.rate)
                                        .range(0.01..=30.0)
                                        .speed(0.05)
                                        .suffix(" Hz"),
                                )
                                .changed();
                        }
                    });
                    ui.end_row();
                    ui.label("   LFO > cutoff");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.lfo.cutoff, -4.0..=4.0).suffix(" oct"))
                        .changed();
                    ui.end_row();
                    ui.label("   LFO > amp");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.lfo.amp, 0.0..=1.0))
                        .on_hover_text("1 with a Square LFO synced to 1/16 = trance gate")
                        .changed();
                    ui.end_row();
                    ui.label("   LFO > pitch");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.lfo.pitch, 0.0..=12.0).suffix(" st"))
                        .changed();
                    ui.end_row();
                });
        }
        if changed {
            self.with(|s| {
                if let Some(inst) = s.song.instruments.get_mut(idx.wrapping_sub(1)) {
                    inst.volume = volume;
                    inst.transpose = transpose;
                    match (patch, voice, &mut inst.kind) {
                        (Some(p), None, InstrumentKind::Synth(dst)) => *dst = p,
                        (Some(p), Some(mut v), InstrumentKind::Voice(dst)) => {
                            v.base = p;
                            *dst = v;
                        }
                        _ => {}
                    }
                }
            });
            self.dirty = true;
        }
        for (r, tg) in learn {
            self.learn_menu(&r, tg);
        }
    }
}

/// devices per instrument chain (same limit as the FX view)
const MAX_DEVICES: usize = 16;

impl App {
    /// Add a default `kind` device at the end of instrument `slot`'s chain.
    pub(super) fn add_effect(&mut self, slot: u8, kind: crate::fx::FxKind) {
        let idx = (slot as usize).wrapping_sub(1);
        self.snapshot();
        let r = self.with(|s| {
            let i = s.song.instruments.get_mut(idx)?;
            if i.fx.len() >= MAX_DEVICES {
                return Some(false);
            }
            i.fx.push(crate::fx::Device::new(kind));
            Some(true)
        });
        self.status = match r {
            Some(true) => format!("Added {} to instrument {slot:02X}", kind.name()),
            Some(false) => format!("Instrument {slot:02X}: chain is full ({MAX_DEVICES} devices)"),
            None => "No such instrument".into(),
        };
    }

    pub(super) fn copy_effects(&mut self, slot: u8) {
        let idx = (slot as usize).wrapping_sub(1);
        if let Some(fx) = self.with(|s| s.song.instruments.get(idx).map(|i| i.fx.clone())) {
            self.status = format!("Copied {} effect(s) from instrument {slot:02X}", fx.len());
            self.fx_clip = Some(fx);
        }
    }

    /// Paste the copied chain: replace, or append (up to the device limit).
    pub(super) fn paste_effects(&mut self, slot: u8, append: bool) {
        let Some(clip) = self.fx_clip.clone() else {
            self.status = "Nothing copied: right-click an instrument > Copy effects".into();
            return;
        };
        let idx = (slot as usize).wrapping_sub(1);
        self.snapshot();
        let n = self.with(|s| {
            let i = s.song.instruments.get_mut(idx)?;
            if !append {
                i.fx.clear();
            }
            let room = MAX_DEVICES.saturating_sub(i.fx.len());
            let n = clip.len().min(room);
            i.fx.extend(clip.into_iter().take(n));
            Some(n)
        });
        if let Some(n) = n {
            self.status = format!(
                "{} {n} effect(s) {} instrument {slot:02X}",
                if append { "Appended" } else { "Pasted" },
                if append { "to" } else { "into" }
            );
        }
    }

    /// Delete (with a confirmation when notes use it)
    pub(super) fn request_delete_instrument(&mut self, slot: u8) {
        let (exists, name, uses) = self.with(|s| {
            let i = (slot as usize).wrapping_sub(1);
            match s.song.instruments.get(i) {
                Some(x) => (true, x.name.clone(), s.song.instrument_uses(slot)),
                None => (false, String::new(), 0),
            }
        });
        if !exists {
            self.status = "No instrument selected".into();
        } else if uses > 0 {
            self.instr_delete = Some((slot, name, uses));
        } else {
            self.delete_instrument_now(slot);
        }
    }

    pub(super) fn delete_instrument_now(&mut self, slot: u8) {
        let idx = (slot as usize).wrapping_sub(1);
        self.snapshot();
        let res = self.with(|s| {
            let x = s.song.instruments.get(idx)?.name.clone();
            let n = s.song.delete_instrument(idx);
            s.engine.remap_instruments(&|i| match i.cmp(&idx) {
                std::cmp::Ordering::Less => Some(i),
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Greater => Some(i - 1),
            });
            Some((x, n, s.song.instruments.len()))
        });
        let Some((name, n, len)) = res else {
            return;
        };
        if self.instr > slot || (self.instr == slot && slot as usize > len) {
            self.instr -= 1;
        }
        self.instr = self.instr.clamp(1, len.clamp(1, 255) as u8);
        self.dirty = true;
        self.status = format!(
            "Deleted instrument {slot:02X} {name}{} (Ctrl+Z undoes)",
            if n > 0 {
                format!(", {n} note(s) removed")
            } else {
                String::new()
            }
        );
    }

    pub(super) fn duplicate_instrument(&mut self, slot: u8) {
        let idx = (slot as usize).wrapping_sub(1);
        self.snapshot();
        match self.with(|s| s.song.duplicate_instrument(idx)) {
            Some(n) => {
                self.instr = (n + 1) as u8;
                self.status = format!("Duplicated instrument {slot:02X} to {:02X}", n + 1);
            }
            None => {
                self.status =
                    "Cannot duplicate: no instrument selected or all 255 slots used".into()
            }
        }
    }

    /// confirmation for deleting an instrument that notes use
    pub(super) fn instr_delete_dialog(&mut self, ctx: &egui::Context) {
        let Some((slot, name, uses)) = self.instr_delete.clone() else {
            return;
        };
        let (mut yes, mut no) = (false, false);
        egui::Window::new("Delete instrument?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!(
                    "Instrument {slot:02X} \"{name}\" is used by {uses} note(s)."
                ));
                ui.label("Deleting it removes those notes; later instruments move up one slot.");
                ui.label("You can undo this with Ctrl+Z.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    yes = ui
                        .button(format!("Delete and remove {uses} note(s)"))
                        .clicked();
                    no = ui.button("Cancel").clicked();
                });
            });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            no = true;
        }
        if yes {
            self.instr_delete = None;
            self.delete_instrument_now(slot);
        } else if no {
            self.instr_delete = None;
        }
    }
}
