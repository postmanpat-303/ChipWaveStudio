//! Left side panel: instrument list, sample import button and the patch editor.
use super::*;

impl App {
    pub(super) fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Instruments").strong().size(15.0));
        });
        let names: Vec<(String, bool)> = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| {
                    (
                        i.name.clone(),
                        matches!(i.kind, InstrumentKind::Sample { .. }),
                    )
                })
                .collect()
        });
        egui::ScrollArea::vertical()
            .id_salt("inst")
            .max_height(260.0)
            .show(ui, |ui| {
                for (i, (n, is_sample)) in names.iter().enumerate() {
                    let slot = (i + 1) as u8;
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        let txt = egui::RichText::new(format!(
                            "{:02X}  {}{}",
                            slot,
                            n,
                            if *is_sample { "  (sample)" } else { "" }
                        ))
                        .monospace();
                        let r = ui.selectable_label(self.instr == slot, txt);
                        if r.clicked() {
                            self.instr = slot;
                        }
                        r.context_menu(|ui| {
                            if ui.button("Duplicate instrument").clicked() {
                                self.duplicate_instrument(slot);
                                ui.close();
                            }
                            if ui.button("Delete instrument…").clicked() {
                                self.request_delete_instrument(slot);
                                ui.close();
                            }
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
        let Some((name, mut volume, mut transpose, mut patch)) = self.with(|s| {
            s.song.instruments.get(idx.wrapping_sub(1)).map(|i| {
                let patch = match &i.kind {
                    InstrumentKind::Synth(p) => Some(p.clone()),
                    _ => None,
                };
                (i.name.clone(), i.volume, i.transpose, patch)
            })
        }) else {
            return;
        };
        // edit a copy without holding the audio lock, commit only on change
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(egui::RichText::new(format!("Sound · {}", name)).strong());
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Volume");
            changed |= ui.add(egui::Slider::new(&mut volume, 0.0..=1.0)).changed();
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
        if let Some(p) = &mut patch {
            egui::Grid::new("patch")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
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
                    ui.label("   Octave");
                    changed |= ui.add(egui::Slider::new(&mut p.osc1_oct, -3..=3)).changed();
                    ui.end_row();
                    ui.label("   Pulse width");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.pulse_width, 0.05..=0.95))
                        .changed();
                    ui.end_row();
                    ui.label("   Unison");
                    changed |= ui.add(egui::Slider::new(&mut p.unison, 1..=8)).changed();
                    ui.end_row();
                    ui.label("   Detune ct");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.detune_cents, 0.0..=50.0))
                        .changed();
                    ui.end_row();
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
                    changed |= ui
                        .add(egui::Slider::new(&mut p.cutoff, 20.0..=18000.0).logarithmic(true))
                        .changed();
                    ui.end_row();
                    ui.label("   Resonance");
                    changed |= ui
                        .add(egui::Slider::new(&mut p.resonance, 0.0..=0.98))
                        .changed();
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
                    changed |= ui.add(egui::Slider::new(&mut p.drive, 0.0..=1.0)).changed();
                    ui.end_row();
                });
        }
        if changed {
            self.with(|s| {
                if let Some(inst) = s.song.instruments.get_mut(idx.wrapping_sub(1)) {
                    inst.volume = volume;
                    inst.transpose = transpose;
                    if let (Some(p), InstrumentKind::Synth(dst)) = (patch, &mut inst.kind) {
                        *dst = p;
                    }
                }
            });
            self.dirty = true;
        }
    }
}

impl App {
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
