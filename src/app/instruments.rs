//! Left side panel: instrument list, sample import button and the patch editor.
use super::*;

impl App {
    /// open the inline rename field of an instrument
    pub(super) fn start_rename(&mut self, slot: u8, name: String) {
        self.renaming = Some((slot, name));
        self.rename_focus = false;
    }

    /// close the rename field: `Some(name)` commits it (one undo step; empty or
    /// unchanged names keep the old one), `None` cancels
    pub(super) fn finish_rename(&mut self, slot: u8, name: Option<String>) {
        if self.renaming.as_ref().map(|r| r.0) != Some(slot) {
            return;
        }
        self.renaming = None;
        self.rename_focus = false;
        let Some(name) = name else {
            self.status = format!("Rename of instrument {slot:02X} cancelled");
            return;
        };
        let name = name.trim().to_string();
        let i = slot as usize - 1;
        let old = self.with(|s| s.song.instruments.get(i).map(|x| x.name.clone()));
        if name.is_empty() || old.as_deref() == Some(name.as_str()) || old.is_none() {
            return;
        }
        self.snapshot();
        self.with(|s| {
            if let Some(x) = s.song.instruments.get_mut(i) {
                x.name = name.clone();
                if let InstrumentKind::Synth(p) = &mut x.kind {
                    p.name = name.clone();
                }
            }
        });
        self.dirty = true;
        self.status = format!("Renamed instrument {slot:02X} to {name}");
    }

    pub(super) fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
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
                            InstrumentKind::Gen(_) => "  (gen)",
                            InstrumentKind::Plugin(_) => "  (plugin)",
                            InstrumentKind::Rack(_) => "  (rack)",
                            InstrumentKind::Synth(_) => "",
                        },
                    )
                })
                .collect()
        });
        // list order: each rack followed by its layers (indented)
        let order: Vec<(usize, Option<usize>)> = self.with(|s| {
            let n = s.song.instruments.len();
            let mut placed = vec![false; n];
            let mut v = vec![];
            for i in 0..n {
                if placed[i] {
                    continue;
                }
                let owner = s.song.rack_of(i);
                if owner.is_some_and(|r| r != i && r < n) {
                    // listed under its rack
                    continue;
                }
                placed[i] = true;
                v.push((i, None));
                if let InstrumentKind::Rack(r) = &s.song.instruments[i].kind {
                    for l in &r.layers {
                        if l.slot < n && !placed[l.slot] {
                            placed[l.slot] = true;
                            v.push((l.slot, Some(i)));
                        }
                    }
                }
            }
            // anything a damaged rack left out still shows
            for (i, p) in placed.iter().enumerate() {
                if !p {
                    v.push((i, None));
                }
            }
            v
        });
        let tags: Vec<&'static str> = self.with(|s| {
            s.song
                .instruments
                .iter()
                .map(|i| super::params::type_tag(&i.kind))
                .collect()
        });
        // header: title, count and (for long lists) the search box on one row
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Instruments").strong().size(15.0));
            ui.label(
                egui::RichText::new(format!("{}", names.len()))
                    .small()
                    .color(th().ink2),
            );
            if names.len() > 6 || !self.instr_query.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    if !self.instr_query.is_empty() && ui.small_button("×").clicked() {
                        self.instr_query.clear();
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.instr_query)
                            .hint_text("search")
                            .text_color(th().note)
                            .desired_width(ui.available_width().clamp(60.0, 130.0)),
                    )
                    .on_hover_text(
                        "filter the list by name or kind (synth, sample, voice, gen, plugin)",
                    );
                });
            }
        });
        self.instrument_toolbar(ui, names.len());
        let q = self.instr_query.to_lowercase();
        // (slot, new name); None = Esc: cancel
        let mut commit_rename: Option<(u8, Option<String>)> = None;
        let mut rename_drawn = false;
        let renaming_before = self.renaming.as_ref().map(|r| r.0);
        // compact rows; the list grows with the panel and only scrolls when
        // it has to, with a thin bar in its own gutter (never over the names)
        let row_h = (ui.text_style_height(&egui::TextStyle::Monospace) + 4.0).max(17.0);
        let visible = names
            .iter()
            .zip(&tags)
            .filter(|((n, tag), tt)| {
                let kind = if tag.is_empty() { "synth" } else { tag.trim() };
                let tt = tt.to_lowercase();
                q.is_empty()
                    || q.split_whitespace()
                        .all(|w| n.to_lowercase().contains(w) || kind.contains(w) || tt.contains(w))
            })
            .count();
        let panel_h = ui.clip_rect().height();
        let max_h = (panel_h * 0.45).max(row_h * 8.0);
        if names.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(
                        "No instruments yet: + adds one, or drop .wav / .cwi files on the window",
                    )
                    .small()
                    .color(th().ink2),
                );
            });
        }
        let mut empty_menu = false;
        ui.scope(|ui| {
            let sc = &mut ui.spacing_mut().scroll;
            sc.floating = false;
            sc.bar_width = 5.0;
            sc.bar_inner_margin = 3.0;
            sc.bar_outer_margin = 0.0;
            sc.handle_min_length = 16.0;
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical()
                .id_salt("inst")
                .max_height(max_h.min(visible as f32 * row_h + 2.0).max(row_h))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for &(i, rack_of) in &order {
                        let (n, tag) = &names[i];
                        let slot = (i + 1) as u8;
                        let kind = if tag.is_empty() { "synth" } else { tag.trim() };
                        let ttag = tags.get(i).copied().unwrap_or("");
                        let ttl = ttag.to_lowercase();
                        if !q.is_empty()
                            && !q.split_whitespace().all(|w| {
                                n.to_lowercase().contains(w) || kind.contains(w) || ttl.contains(w)
                            })
                        {
                            continue;
                        }
                        if let Some((rs, txt)) = &mut self.renaming {
                            if *rs == slot {
                                rename_drawn = true;
                                ui.horizontal(|ui| {
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(format!("{slot:02X}")).monospace(),
                                    );
                                    let r = ui.add(
                                        egui::TextEdit::singleline(txt)
                                            .id(egui::Id::unique(("inst_rename", slot)))
                                            .desired_width(ui.available_width() - 8.0)
                                            .char_limit(64),
                                    );
                                    // focus the field once; after that, losing focus
                                    // (a click anywhere else, Tab, a view switch) commits
                                    let had_focus = self.rename_focus;
                                    if !had_focus {
                                        r.request_focus();
                                        self.rename_focus = true;
                                        self.rename_view = self.view;
                                    }
                                    let enter = ui.input(|i| i.key_pressed(Key::Enter));
                                    let esc = ui.input(|i| i.key_pressed(Key::Escape));
                                    if esc {
                                        commit_rename = Some((slot, None));
                                    } else if enter
                                        || (had_focus && !r.has_focus())
                                        || self.view != self.rename_view
                                    {
                                        commit_rename = Some((slot, Some(txt.clone())));
                                    }
                                });
                                continue;
                            }
                        }
                        // rack layers sit indented under their rack
                        let (shown, label) = match rack_of {
                            Some(r) => (
                                format!("    · {n}"),
                                format!("{:02X}  {}{} (layer of {:02X})", slot, n, tag, r + 1),
                            ),
                            None => (n.clone(), format!("{:02X}  {}{}", slot, n, tag)),
                        };
                        let r = instrument_row(
                            ui,
                            row_h,
                            slot,
                            &shown,
                            ttag,
                            self.instr == slot,
                            &label,
                        );
                        self.br
                            .slot_rects
                            .push((slot, r.rect.intersect(ui.clip_rect())));
                        if r.clicked() {
                            self.instr = slot;
                            // the lower panel follows (3.6): its editor
                            if self.extra.follow_select && self.view == View::Timeline {
                                self.show_selection_panel();
                            }
                        }
                        if r.double_clicked() {
                            self.start_rename(slot, n.clone());
                        }
                        // a newly selected instrument (duplicate, Up/Down, MIDI...) scrolls into view
                        if self.instr == slot && self.instr_shown != slot {
                            r.scroll_to_me(None);
                            self.instr_shown = slot;
                        }
                        let r = r.on_hover_text(format!(
                            "{slot:02X} {n} ({ttag})\ndouble-click: rename · right-click: more"
                        ));
                        r.context_menu(|ui| {
                            ui.menu_button("Add effect", |ui| {
                                for k in crate::fx::FxKind::ALL {
                                    if ui.button(k.name()).clicked() {
                                        self.add_effect(slot, k);
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                if let Some(pp) = self.plugin_fx_menu(ui) {
                                    self.add_plugin_fx(slot, pp);
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
                            // a timeline song: its effects in the lower panel's
                            // Devices tab, under the timeline (the FX view stays
                            // in View › FX)
                            let tl_song = self.with(|s| s.song.timeline.is_some());
                            let (lab, tip) = if tl_song {
                                ("Show effects (Nested Device Chains)", "this instrument's effect chain in the lower panel, under the timeline")
                            } else {
                                ("Open in FX view", "this instrument's effect chain in the FX view")
                            };
                            if ui.button(lab).on_hover_text(tip).clicked() {
                                self.instr = slot;
                                if tl_song {
                                    self.view = View::Timeline;
                                    self.set_lower(4);
                                } else {
                                    self.view = View::Fx;
                                }
                                ui.close();
                            }
                            ui.separator();
                            if ui
                                .button("Rename")
                                .on_hover_text("or double-click the name")
                                .clicked()
                            {
                                self.start_rename(slot, n.clone());
                                ui.close();
                            }
                            if ui
                                .add_enabled(i > 0, egui::Button::new("Move up"))
                                .clicked()
                            {
                                self.move_instrument(i, i - 1);
                                ui.close();
                            }
                            if ui
                                .add_enabled(i + 1 < names.len(), egui::Button::new("Move down"))
                                .clicked()
                            {
                                self.move_instrument(i, i + 1);
                                ui.close();
                            }
                            ui.menu_button("Use this instead of", |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        "notes of the instrument you pick switch to this one",
                                    )
                                    .small()
                                    .color(th().ink2),
                                );
                                for (j, (m, _)) in names.iter().enumerate() {
                                    let other = (j + 1) as u8;
                                    if other == slot {
                                        continue;
                                    }
                                    ui.horizontal(|ui| {
                                        if ui
                                            .button(format!("{other:02X} {m}"))
                                            .on_hover_text("whole song")
                                            .clicked()
                                        {
                                            self.swap_instrument_notes(other, slot, false);
                                            ui.close();
                                        }
                                        if ui.small_button("this block").clicked() {
                                            self.swap_instrument_notes(other, slot, true);
                                            ui.close();
                                        }
                                    });
                                }
                            });
                            ui.separator();
                            if ui.button("Save to library").clicked() {
                                self.save_instrument_to_library(slot);
                                ui.close();
                            }
                            if ui.button("Export .cwi…").clicked() {
                                self.export_instrument(slot);
                                ui.close();
                            }
                            if ui.button("Replace sound from library…").clicked() {
                                self.instr = slot;
                                self.open_library();
                                ui.close();
                            }
                            ui.separator();
                            let no_rack = self.with(|s| {
                                super::rackops::no_rack_reason(&s.song, slot as usize - 1)
                            });
                            if ui
                                .add_enabled(
                                    no_rack.is_none(),
                                    egui::Button::new("Make a rack from this instrument"),
                                )
                                .on_hover_text(
                                    "this sound becomes layer A of a rack in the same slot (it sounds the same)",
                                )
                                .on_disabled_hover_text(no_rack.unwrap_or(""))
                                .clicked()
                            {
                                self.make_rack(slot as usize - 1);
                                self.set_lower(4);
                                ui.close();
                            }
                            if ui.button("Save Custom Rack…").clicked() {
                                let idx = slot as usize - 1;
                                let root = self.with(|s| s.song.rack_of(idx)).unwrap_or(idx);
                                self.open_save_rack(root);
                                ui.close();
                            }
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

                            ui.separator();
                            self.list_actions_menu(ui);
                        });
                    }
                });
            // an empty (or fully filtered) list: right-click offers the list actions
            if names.is_empty() || visible == 0 {
                empty_menu = true;
            }
        });
        if empty_menu {
            let r = ui.allocate_response(
                egui::vec2(ui.available_width(), row_h * 2.0),
                Sense::click(),
            );
            r.context_menu(|ui| self.list_actions_menu(ui));
        }
        // the field wasn't drawn (filtered out, panel hidden): keep what was typed
        if commit_rename.is_none() && !rename_drawn {
            if let Some((slot, txt)) = &self.renaming {
                if renaming_before == Some(*slot) {
                    commit_rename = Some((*slot, Some(txt.clone())));
                }
            }
        }
        if let Some((slot, name)) = commit_rename {
            self.finish_rename(slot, name);
        }
        ui.add_space(2.0);
        ui.separator();
        if !self.extra.classic_editor {
            self.instrument_summary(ui);
            return;
        }
        // patch editor for the selected synth instrument
        let idx = self.instr as usize;
        let mut changed = false;
        let Some((name, mut volume, mut transpose, mut patch, mut voice, mut gen, plug)) = self
            .with(|s| {
                s.song.instruments.get(idx.wrapping_sub(1)).map(|i| {
                    let (patch, voice, gen) = match &i.kind {
                        InstrumentKind::Synth(p) => (Some(p.clone()), None, None),
                        InstrumentKind::Voice(v) => (Some(v.base.clone()), Some(v.clone()), None),
                        InstrumentKind::Gen(g) => (None, None, Some(g.clone())),
                        _ => (None, None, None),
                    };
                    let plug = match &i.kind {
                        InstrumentKind::Plugin(p) => Some(p.clone()),
                        _ => None,
                    };
                    (
                        i.name.clone(),
                        i.volume,
                        i.transpose,
                        patch,
                        voice,
                        gen,
                        plug,
                    )
                })
            })
        else {
            return;
        };
        let mut bake = false;
        let mut open_filter_lanes = false;
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
        if let Some(pp) = &plug {
            self.plugin_panel(ui, idx.wrapping_sub(1), pp);
        }
        if let Some(g) = &mut gen {
            let o = super::genview::gen_panel(ui, g, &mut self.gen_ui, idx.wrapping_sub(1));
            changed |= o.changed;
            bake = o.bake;
        }
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
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("fmode")
                            .selected_text(super::filtercard::mode_name(p.filter))
                            .show_ui(ui, |ui| {
                                for m in [FilterMode::Lp, FilterMode::Hp, FilterMode::Bp] {
                                    changed |= ui
                                        .selectable_value(
                                            &mut p.filter,
                                            m,
                                            super::filtercard::mode_name(m),
                                        )
                                        .changed();
                                }
                            });
                        ui.label(egui::RichText::new("Cmd 38xx").small().weak())
                            .on_hover_text(
                                "38xx cutoff · 39xx resonance · 3Axx/3Bxx cutoff slides",
                            );
                        if ui
                            .small_button("A")
                            .on_hover_text("open Cutoff and Reso lanes on the track")
                            .clicked()
                        {
                            open_filter_lanes = true;
                        }
                    });
                    ui.end_row();
                    ui.label("   Model");
                    ui.horizontal(|ui| {
                        changed |= super::genview::filter_model_combo(ui, "fmodel", p);
                    });
                    ui.end_row();
                    ui.label(format!("   {}", super::filtercard::LABEL_CUTOFF));
                    let r =
                        ui.add(egui::Slider::new(&mut p.cutoff, 20.0..=18000.0).logarithmic(true));
                    changed |= r.changed();
                    learn.push((r, crate::automation::Target::Cutoff(idx.wrapping_sub(1))));
                    ui.end_row();
                    ui.label(format!("   {}", super::filtercard::LABEL_RESO));
                    let r = ui.add(egui::Slider::new(&mut p.resonance, 0.0..=0.98));
                    changed |= r.changed();
                    learn.push((r, crate::automation::Target::Resonance(idx.wrapping_sub(1))));
                    ui.end_row();
                    ui.label(format!("   {}", super::filtercard::LABEL_ENV));
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
                    ui.label(format!("   {}", super::filtercard::LABEL_DRIVE));
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
                    ui.label("   Glide (303)");
                    ui.horizontal(|ui| {
                        let mut ms = p.glide * 1000.0;
                        if ui
                            .add(
                                egui::Slider::new(&mut ms, 1.0..=1000.0)
                                    .logarithmic(true)
                                    .suffix(" ms"),
                            )
                            .on_hover_text(
                                "glide time: how long a slide to the next note takes \
                                 (effect 3C01, or every held note with Legato on)",
                            )
                            .changed()
                        {
                            p.glide = ms / 1000.0;
                            changed = true;
                        }
                        changed |= ui
                            .checkbox(&mut p.legato, "Legato")
                            .on_hover_text(
                                "a new note while the last one still sounds glides to it \
                                 without restarting the envelopes (put a note-off or 3C00 \
                                 between notes to retrigger)",
                            )
                            .changed();
                    });
                    ui.end_row();
                    ui.label("   Accent");
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::Slider::new(&mut p.accent, 0.0..=1.0))
                            .on_hover_text(
                                "accented notes (effect 3D or high velocity) are louder, \
                                 with a more open filter and a shorter decay",
                            )
                            .changed();
                        let mut v = (p.accent_vel * 64.0).round() as u32;
                        if ui
                            .add(
                                egui::DragValue::new(&mut v)
                                    .range(0..=64)
                                    .hexadecimal(2, false, true)
                                    .prefix("at vol ≥ "),
                            )
                            .on_hover_text(
                                "notes with a volume-column value at or above this are \
                                 accented (00 = off: only effect 3D accents)",
                            )
                            .changed()
                        {
                            p.accent_vel = v as f32 / 64.0;
                            changed = true;
                        }
                    });
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
                    if let (Some(g), InstrumentKind::Gen(dst)) = (gen, &mut inst.kind) {
                        *dst = g;
                    }
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
        if open_filter_lanes {
            let i = idx.wrapping_sub(1);
            self.add_lane(crate::automation::Target::Cutoff(i));
            self.add_lane(crate::automation::Target::Resonance(i));
        }
        if bake {
            self.bake_gen(idx.wrapping_sub(1));
        }
    }
}

/// devices per instrument chain (same limit as the FX view)
pub(super) const MAX_DEVICES: usize = crate::fx::MAX_DEVICES;

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
        // copy plugin effects with their current settings
        self.plug.state_dirty = true;
        self.capture_plugins();
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
            // pasted plugin devices are new instances (own uid)
            i.fx.extend(clip.into_iter().take(n).map(|mut d| {
                d.renew_uid();
                d
            }));
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

#[derive(Clone, Copy)]
enum Glyph {
    Import,
    Duplicate,
    Delete,
    Up,
    Down,
    Library,
}

/// tiny vector icons for the instrument toolbar (no icon font needed)
fn paint_glyph(p: &egui::Painter, r: Rect, g: Glyph, col: Color32) {
    let c = r.center();
    let s = Stroke::new(1.3, col);
    let v = |x: f32, y: f32| c + egui::vec2(x, y);
    match g {
        Glyph::Import => {
            // waveform with a down arrow: "bring in a sample"
            let pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let x = -6.0 + i as f32 * 1.2;
                    let a = [0.0, 2.0, -3.0, 4.0, -4.0, 3.0, -2.0, 1.5, -1.0, 0.5, 0.0][i];
                    v(x, 2.0 + a * 0.6)
                })
                .collect();
            p.add(egui::Shape::line(pts, s));
            p.line_segment([v(0.0, -6.0), v(0.0, -1.5)], s);
            p.add(egui::Shape::convex_polygon(
                vec![v(-2.5, -3.0), v(2.5, -3.0), v(0.0, -0.5)],
                col,
                Stroke::NONE,
            ));
        }
        Glyph::Duplicate => {
            p.rect_stroke(
                Rect::from_min_size(v(-5.0, -5.0), egui::vec2(7.0, 7.0)),
                1.0,
                s,
                egui::StrokeKind::Middle,
            );
            p.rect_filled(
                Rect::from_min_size(v(-2.0, -2.0), egui::vec2(7.0, 7.0)),
                1.0,
                col,
            );
        }
        Glyph::Delete => {
            p.line_segment([v(-5.0, -4.0), v(5.0, -4.0)], s);
            p.line_segment([v(-1.5, -5.5), v(1.5, -5.5)], s);
            p.add(egui::Shape::closed_line(
                vec![v(-3.8, -3.0), v(3.8, -3.0), v(3.0, 5.5), v(-3.0, 5.5)],
                s,
            ));
            p.line_segment([v(-1.2, -1.0), v(-1.0, 3.8)], s);
            p.line_segment([v(1.2, -1.0), v(1.0, 3.8)], s);
        }
        Glyph::Up | Glyph::Down => {
            let d = if matches!(g, Glyph::Up) { 1.0 } else { -1.0 };
            p.add(egui::Shape::convex_polygon(
                vec![v(0.0, -5.0 * d), v(4.5, 0.0), v(-4.5, 0.0)],
                col,
                Stroke::NONE,
            ));
            p.line_segment([v(0.0, -0.5 * d), v(0.0, 5.0 * d)], Stroke::new(2.0, col));
        }
        Glyph::Library => {
            for (k, h) in [(0, 10.0), (1, 9.0), (2, 11.0)] {
                let x = -5.5 + k as f32 * 3.6;
                p.rect_stroke(
                    Rect::from_min_max(v(x, 5.5 - h), v(x + 2.6, 5.5)),
                    0.5,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
            p.line_segment([v(5.5, 5.5), v(7.0, -3.5)], s);
        }
    }
}

/// One compact list row: kind tag, slot number, name (clipped with an
/// ellipsis, never under the scroll bar). `label` is the accessible name.
fn instrument_row(
    ui: &mut egui::Ui,
    h: f32,
    slot: u8,
    name: &str,
    kind: &str,
    selected: bool,
    label: &str,
) -> egui::Response {
    let w = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
    let lbl = label.to_string();
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListBoxOption, true, selected, &lbl));
    if ui.is_rect_visible(rect) {
        let t = th();
        let p = ui.painter_at(rect);
        let body = rect.shrink2(egui::vec2(4.0, 1.0));
        if selected {
            p.rect_filled(body, 4.0, t.sel);
        } else if r.hovered() {
            p.rect_filled(body, 4.0, t.ink2.gamma_multiply(0.15));
        }
        // kind tag
        let tag = Rect::from_min_size(
            Pos2::new(body.left() + 3.0, body.top() + 3.0),
            egui::vec2(3.0, body.height() - 6.0),
        );
        p.rect_filled(tag, 1.0, super::instpanel::tag_colour(kind));
        let font = egui::TextStyle::Monospace.resolve(ui.style());
        let x0 = tag.right() + 6.0;
        p.text(
            Pos2::new(x0, body.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{slot:02X}"),
            font.clone(),
            t.ink2,
        );
        let nx = x0 + font.size * 1.9;
        // type tag on the right (Drum, Analog, FM-4, Voice, CLAP, VST3...)
        let tf = egui::FontId::proportional((font.size * 0.82).max(9.0));
        let tg = ui.fonts_mut(|f| {
            f.layout_no_wrap(
                kind.to_string(),
                tf,
                super::instpanel::tag_text_colour(kind),
            )
        });
        let tag_w = if body.width() > 150.0 {
            tg.size().x + 8.0
        } else {
            0.0
        };
        if tag_w > 0.0 {
            p.galley(
                Pos2::new(
                    body.right() - 4.0 - tg.size().x,
                    body.center().y - tg.size().y / 2.0,
                ),
                tg,
                t.ink2,
            );
        }
        let avail = body.right() - 4.0 - tag_w - nx;
        let col = if selected { t.ink } else { t.value };
        let mut job = egui::text::LayoutJob::simple_singleline(name.to_string(), font, col);
        job.wrap = egui::text::TextWrapping::truncate_at_width(avail.max(8.0));
        let g = ui.fonts_mut(|f| f.layout_job(job));
        p.galley(Pos2::new(nx, body.center().y - g.size().y / 2.0), g, col);
    }
    r
}

impl App {
    /// one row of small icon buttons over the list (with tooltips)
    fn instrument_toolbar(&mut self, ui: &mut egui::Ui, n: usize) {
        let has = (self.instr as usize).wrapping_sub(1) < n;
        let i = (self.instr as usize).wrapping_sub(1);
        // a small painted-icon button whose accessible name is the action
        fn ib(ui: &mut egui::Ui, on: bool, icon: Glyph, name: &str, tip: &str) -> egui::Response {
            let r = ui
                .add_enabled(
                    on,
                    egui::Button::new("")
                        .small()
                        .min_size(egui::vec2(22.0, 18.0)),
                )
                .on_hover_text(tip)
                .on_disabled_hover_text(tip);
            let col = if on {
                ui.visuals().widgets.inactive.fg_stroke.color
            } else {
                th().ink2.gamma_multiply(0.5)
            };
            paint_glyph(ui.painter(), r.rect, icon, col);
            let nm = name.to_string();
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, on, &nm));
            r
        }
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.spacing_mut().item_spacing.x = 3.0;
            let r = ui
                .menu_button(egui::RichText::new("+").size(14.0).strong(), |ui| {
                    self.new_instrument_menu(ui)
                })
                .response;
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "New…"));
            r.on_hover_text("New instrument: Synth, Generator, Plugin, Voice");
            if ib(ui, true, Glyph::Import, "Import WAV…", "Import WAV… as a sample instrument (or drop .wav / .cwi files on the window)").clicked() {
                self.import_wav_dialog();
            }
            ui.add_space(4.0);
            if ib(ui, has, Glyph::Duplicate, "Duplicate", "Duplicate the selected instrument (with its FX chain)").clicked() {
                self.duplicate_instrument(self.instr);
            }
            if ib(ui, has, Glyph::Delete, "Delete…", "Delete the selected instrument; its notes are removed and later instruments move up one slot (Ctrl+Z undoes)").clicked() {
                self.request_delete_instrument(self.instr);
            }
            if ib(ui, has && i > 0, Glyph::Up, "Up", "Move the selected instrument up one slot (notes follow)").clicked() {
                self.move_instrument(i, i - 1);
            }
            if ib(ui, has && i + 1 < n, Glyph::Down, "Down", "Move the selected instrument down one slot (notes follow)").clicked() {
                self.move_instrument(i, i + 1);
            }
            ui.add_space(4.0);
            if ib(ui, true, Glyph::Library, "Library…", "Instrument library: browse, audition, add and save instruments (.cwi)").clicked() {
                self.open_library();
            }
        });
        ui.add_space(2.0);
    }

    fn import_wav_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("WAV", &["wav"])
            .pick_file()
        {
            self.import_sample(p);
        }
    }

    /// list-wide actions (right-click menu of rows and of the empty list)
    fn list_actions_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("New instrument", |ui| self.new_instrument_menu(ui));
        if ui.button("Import WAV…").clicked() {
            ui.close();
            self.import_wav_dialog();
        }
        if ui.button("Instrument library…").clicked() {
            ui.close();
            self.open_library();
        }
    }
}

impl App {
    /// under the list: what the selected instrument is, and where to edit it
    fn instrument_summary(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let slot = self.instr;
        let info = self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .map(|i| {
                    (
                        i.name.clone(),
                        super::params::type_tag(&i.kind),
                        i.volume,
                        i.transpose,
                        i.fx.len(),
                        s.song.instrument_uses(slot),
                    )
                })
        });
        let Some((name, tag, vol, tr, nfx, uses)) = info else {
            return;
        };
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add(
                egui::Label::new(egui::RichText::new(format!("{slot:02X} {name}")).strong())
                    .truncate(),
            );
            super::instpanel::tag_pill(ui, tag);
        });
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(format!(
                    "vol {:.0} % · {:+} st · {} FX · in {} block{}",
                    vol * 100.0,
                    tr,
                    nfx,
                    uses,
                    if uses == 1 { "" } else { "s" }
                ))
                .small()
                .color(t.ink2),
            );
        });
        ui.add_space(4.0);
    }
}
