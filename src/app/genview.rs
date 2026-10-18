//! Generator instrument editor (0.2.3.12): the shared macro-knob panel with
//! Dice, Mutate, per-knob locks, a 16-roll history (< >) and Bake to sample, plus
//! the "New instrument > Generator" menu.
use super::*;
use crate::gen::{DrumKind, GenEngine, GenPatch, MacroDef, KIT_KEYS, KIT_NAMES};

/// rolls kept for < / >
pub const HISTORY: usize = 16;

/// editor state that is not part of the song
#[derive(Default)]
pub(super) struct GenUi {
    /// instrument the history belongs to (0-based)
    pub inst: usize,
    /// kit piece shown
    pub sel: usize,
    /// sounds before / after each roll; `pos` = the one playing
    pub hist: Vec<GenPatch>,
    pub pos: usize,
}

impl GenUi {
    fn record(&mut self, before: &GenPatch, after: &GenPatch) {
        if self.hist.is_empty() {
            self.hist.push(before.clone());
            self.pos = 0;
        }
        self.hist.truncate(self.pos + 1);
        self.hist.push(after.clone());
        while self.hist.len() > HISTORY + 1 {
            self.hist.remove(0);
        }
        self.pos = self.hist.len() - 1;
    }
}

/// what the panel asks the app to do
#[derive(Default)]
pub(super) struct GenOut {
    pub changed: bool,
    pub bake: bool,
}

fn new_seed(prev: u32) -> u32 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
        .unwrap_or(1);
    let mut r = crate::gen::Rng::new(t ^ prev.rotate_left(13));
    r.next() | 1
}

/// a rotary macro knob: drag up/down (or the mouse wheel); returns true when changed
pub(super) fn knob(
    ui: &mut egui::Ui,
    v: &mut f32,
    def: &MacroDef,
    locked: &mut bool,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> bool {
    let mut changed = false;
    ui.allocate_ui(egui::vec2(52.0, 104.0), |ui| {
        ui.vertical_centered(|ui| {
            ui.set_width(52.0);
            ui.label(egui::RichText::new(def.name).small().strong());
            let (rect, resp) =
                ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::click_and_drag());
            let resp = resp.on_hover_text(format!(
                "{}\ndrag up / down or scroll; double-click = middle of the dice range",
                def.tip
            ));
            if resp.dragged() {
                let d = resp.drag_delta();
                let fine = if ui.input(|i| i.modifiers.shift) {
                    0.2
                } else {
                    1.0
                };
                *v = (*v + (d.x - d.y) * 0.005 * fine).clamp(0.0, 1.0);
                changed = true;
            }
            if resp.hovered() {
                let s = ui.input(|i| i.smooth_scroll_delta.y);
                if s != 0.0 {
                    *v = (*v + s * 0.0015).clamp(0.0, 1.0);
                    changed = true;
                }
            }
            if resp.double_clicked() {
                *v = (def.dice.0 + def.dice.1) * 0.5;
                changed = true;
            }
            let p = ui.painter();
            let c = rect.center();
            let rad = 15.0;
            let a0 = std::f32::consts::PI * 0.75;
            let span = std::f32::consts::PI * 1.5;
            let pt = |a: f32, r: f32| c + egui::vec2(a.cos(), a.sin()) * r;
            p.circle_filled(c, rad, th().pill);
            let arc = |from: f32, to: f32, col: egui::Color32, w: f32| {
                let n = 24;
                let pts: Vec<egui::Pos2> = (0..=n)
                    .map(|i| {
                        pt(
                            a0 + span * (from + (to - from) * i as f32 / n as f32),
                            rad - 2.0,
                        )
                    })
                    .collect();
                p.add(egui::Shape::line(pts, egui::Stroke::new(w, col)));
            };
            arc(0.0, 1.0, th().border, 2.0);
            let col = if *locked { th().ink2 } else { th().accent };
            arc(0.0, v.clamp(0.0, 1.0), col, 3.0);
            // the dice window, as a thin inner arc
            let n = 12;
            let pts: Vec<egui::Pos2> = (0..=n)
                .map(|i| {
                    let f = def.dice.0 + (def.dice.1 - def.dice.0) * i as f32 / n as f32;
                    pt(a0 + span * f, rad - 6.0)
                })
                .collect();
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.0, th().ink2)));
            p.line_segment(
                [pt(a0 + span * *v, 4.0), pt(a0 + span * *v, rad - 3.0)],
                egui::Stroke::new(2.0, th().ink),
            );
            ui.label(
                egui::RichText::new(def.show(*v))
                    .monospace()
                    .small()
                    .color(th().value),
            );
            let _ = ui.push_id(id, |ui| {
                if ui
                    .selectable_label(*locked, egui::RichText::new("lock").small())
                    .on_hover_text("locked knobs keep their value when you Dice or Mutate")
                    .clicked()
                {
                    *locked = !*locked;
                    changed = true;
                }
            });
        })
    });
    changed
}

/// the Generator panel; `inst` = 0-based instrument index (for the history)
pub(super) fn gen_panel(
    ui: &mut egui::Ui,
    g: &mut GenPatch,
    st: &mut GenUi,
    inst: usize,
) -> GenOut {
    let mut out = GenOut::default();
    if st.inst != inst {
        *st = GenUi {
            inst,
            ..Default::default()
        };
    }
    let is_kit = matches!(g.engine, GenEngine::Kit(_));
    st.sel = st.sel.min(7);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new(if is_kit {
                "GENERATOR · DRUM KIT"
            } else {
                "GENERATOR · DRUM SYNTH"
            })
            .strong()
            .color(th().accent),
        );
    });
    if is_kit {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(14.0);
            for i in 0..8 {
                if ui
                    .selectable_label(st.sel == i, KIT_NAMES[i])
                    .on_hover_text(format!("plays on {}", KIT_KEYS[i]))
                    .clicked()
                {
                    st.sel = i;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(
                egui::RichText::new(
                    "the note picks the drum: C kick, D snare, D# clap,\n\
                     F# closed / A# open hat, E F G A toms, C# B perc",
                )
                .small()
                .color(th().ink2),
            );
        });
    }
    let sel = st.sel;
    // kind + model of the shown drum
    {
        let d = g.drum_mut(sel);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(14.0);
            for k in DrumKind::ALL {
                if ui.selectable_label(d.kind == k, k.name()).clicked() && d.kind != k {
                    let keep = (d.level, d.follow_note);
                    *d = crate::gen::DrumP::new(k, 0, &[]);
                    (d.level, d.follow_note) = keep;
                    out.changed = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Model");
            let models = d.kind.models();
            egui::ComboBox::from_id_salt("gen_model")
                .width(110.0)
                .selected_text(models[(d.model as usize).min(models.len() - 1)])
                .show_ui(ui, |ui| {
                    for (i, n) in models.iter().enumerate() {
                        out.changed |= ui.selectable_value(&mut d.model, i as u8, *n).changed();
                    }
                });
            if !is_kit {
                out.changed |= ui
                    .checkbox(&mut d.follow_note, "Follow note")
                    .on_hover_text("C-4 plays the drum as tuned; other notes transpose it")
                    .changed();
            }
        });
    }
    // Dice / Mutate / history
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        let before = g.clone();
        if ui
            .button("Dice")
            .on_hover_text("roll the unlocked knobs inside sensible ranges (seeded)")
            .clicked()
        {
            g.dice(sel, new_seed(g.seed));
            st.record(&before, g);
            out.changed = true;
        }
        if is_kit
            && ui
                .button("Dice kit")
                .on_hover_text("roll every piece of the kit")
                .clicked()
        {
            g.dice_all(new_seed(g.seed));
            st.record(&before, g);
            out.changed = true;
        }
        if ui
            .button("Mutate")
            .on_hover_text("nudge the unlocked knobs a little around the current sound")
            .clicked()
        {
            g.mutate(sel, new_seed(g.seed));
            st.record(&before, g);
            out.changed = true;
        }
        let mut pct = g.mutate * 100.0;
        if ui
            .add(
                egui::DragValue::new(&mut pct)
                    .range(1.0..=50.0)
                    .speed(0.5)
                    .prefix("±")
                    .suffix(" %"),
            )
            .on_hover_text("how far Mutate moves each knob")
            .changed()
        {
            g.mutate = pct / 100.0;
            out.changed = true;
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        let before = g.clone();
        let n = st.hist.len();
        if ui
            .add_enabled(st.pos > 0, egui::Button::new("<"))
            .on_hover_text("back to the previous roll")
            .clicked()
        {
            st.pos -= 1;
            *g = st.hist[st.pos].clone();
            out.changed = true;
        }
        if ui
            .add_enabled(st.pos + 1 < n, egui::Button::new(">"))
            .on_hover_text("forward to the next roll")
            .clicked()
        {
            st.pos += 1;
            *g = st.hist[st.pos].clone();
            out.changed = true;
        }
        ui.label(
            egui::RichText::new(if n > 0 {
                format!("roll {}/{}", st.pos, n - 1)
            } else {
                "no rolls yet".into()
            })
            .small()
            .color(th().ink2),
        );
        let mut seed = g.seed;
        if ui
            .add(
                egui::DragValue::new(&mut seed)
                    .hexadecimal(8, false, true)
                    .prefix("seed "),
            )
            .on_hover_text(
                "the Dice seed: the same seed rolls the same sound (type one to recall it)",
            )
            .changed()
        {
            g.seed = seed;
            g.dice(sel, seed);
            st.record(&before, g);
            out.changed = true;
        }
    });
    // the eight macro knobs
    {
        let d = g.drum_mut(sel);
        let defs = d.kind.macros();
        for row in 0..2 {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.spacing_mut().item_spacing.x = 2.0;
                #[allow(clippy::needless_range_loop)]
                for i in row * 4..row * 4 + 4 {
                    let mut locked = d.locks & (1 << i) != 0;
                    let ch = knob(ui, &mut d.macros[i], &defs[i], &mut locked, ("gk", sel, i));
                    if locked {
                        d.locks |= 1 << i;
                    } else {
                        d.locks &= !(1 << i);
                    }
                    out.changed |= ch;
                }
            });
        }
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Level");
            out.changed |= ui.add(egui::Slider::new(&mut d.level, 0.0..=1.5)).changed();
        });
    }
    // shared: release, optional filter (SVF / ladder), bake
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        ui.label("Release");
        let mut ms = g.base.amp.r * 1000.0;
        if ui
            .add(
                egui::Slider::new(&mut ms, 5.0..=2000.0)
                    .logarithmic(true)
                    .suffix(" ms"),
            )
            .on_hover_text("how fast a note-off (===) fades the drum")
            .changed()
        {
            g.base.amp.r = ms / 1000.0;
            out.changed = true;
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        out.changed |= ui
            .checkbox(&mut g.filter_on, "Filter")
            .on_hover_text("run the drum through a filter (pattern commands 38-3B move it)")
            .changed();
        if g.filter_on {
            out.changed |= filter_model_combo(ui, "gen_fm", &mut g.base);
            egui::ComboBox::from_id_salt("gen_fmode")
                .width(46.0)
                .selected_text(format!("{:?}", g.base.filter))
                .show_ui(ui, |ui| {
                    for m in [FilterMode::Lp, FilterMode::Bp, FilterMode::Hp] {
                        out.changed |= ui
                            .selectable_value(&mut g.base.filter, m, format!("{m:?}"))
                            .changed();
                    }
                });
        }
    });
    if g.filter_on {
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Cutoff");
            out.changed |= ui
                .add(egui::Slider::new(&mut g.base.cutoff, 20.0..=18000.0).logarithmic(true))
                .changed();
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Res");
            out.changed |= ui
                .add(egui::Slider::new(&mut g.base.resonance, 0.0..=1.0))
                .changed();
        });
    }
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        if ui
            .button("Bake to sample")
            .on_hover_text(
                "render one C-4 hit of this sound into a new Sample instrument \
                 (then chop or edit it in the sample editor)",
            )
            .clicked()
        {
            out.bake = true;
        }
        ui.label(
            egui::RichText::new("43xy: macro x (1-8) = y (0-F)")
                .small()
                .color(th().ink2),
        );
    });
    out
}

/// filter model picker (SVF / ladder clean / ladder vintage) + the ladder slope
pub(super) fn filter_model_combo(ui: &mut egui::Ui, id: &str, p: &mut crate::synth::Patch) -> bool {
    use crate::ladder::FilterModel;
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .width(118.0)
        .selected_text(p.filter_model.name())
        .show_ui(ui, |ui| {
            for m in FilterModel::ALL {
                changed |= ui
                    .selectable_value(&mut p.filter_model, m, m.name())
                    .changed();
            }
        })
        .response
        .on_hover_text(
            "SVF: the original 12 dB filter. Ladder: Moog-style 24 dB transistor ladder \
             (self-oscillates at full resonance); Vintage saturates and runs 2x oversampled",
        );
    if p.filter_model != FilterModel::Svf {
        let mut s12 = p.ladder_12db;
        if ui
            .checkbox(&mut s12, "12 dB")
            .on_hover_text("ladder slope: 12 dB/oct instead of 24")
            .changed()
        {
            p.ladder_12db = s12;
            changed = true;
        }
    }
    changed
}

impl App {
    /// Bake: render one C-4 hit of generator instrument `idx` (0-based) into a
    /// new Sample instrument (same FX chain) and select it.
    pub(super) fn bake_gen(&mut self, idx: usize) {
        let Some((name, g, fx, vol)) = self.with(|s| {
            s.song.instruments.get(idx).and_then(|i| match &i.kind {
                InstrumentKind::Gen(g) => Some((i.name.clone(), g.clone(), i.fx.clone(), i.volume)),
                _ => None,
            })
        }) else {
            return;
        };
        let sr = 48000;
        let data = crate::gen::bake(&g, crate::model::note_freq(49), sr as f32, 8.0);
        let secs = data.len() as f32 / sr as f32;
        let bname = format!("{name} (baked)");
        self.add_instrument(
            &bname,
            InstrumentKind::Sample {
                file: String::new(),
                rate: sr,
                data: data.into(),
                base_note: 49,
                lp: Default::default(),
            },
        );
        let n = self.instr as usize;
        self.with(|s| {
            if let Some(i) = s.song.instruments.get_mut(n.wrapping_sub(1)) {
                if i.name == bname {
                    i.fx = fx;
                    i.volume = vol;
                }
            }
        });
        self.status = format!("Baked {name} to sample {n:02X} ({secs:.2} s)");
    }

    /// "New instrument > Generator" menu
    pub(super) fn new_generator_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Generator (drums)", |ui| {
            ui.label(
                egui::RichText::new("drum synth: sounds from scratch, 8 knobs + Dice")
                    .small()
                    .color(th().ink2),
            );
            for (group, names) in [
                ("Kits", &["Psy Kit", "808 Kit"][..]),
                (
                    "Kicks",
                    &["Psy Zap Kick", "808 Boom", "909 Kick", "v0 Punch Kick"][..],
                ),
                ("Snares", &["808 Snare", "909 Snare", "Shell Snare"][..]),
                (
                    "Hats",
                    &["Closed Hat", "Open Hat", "Ring Hat", "Comb Hat"][..],
                ),
                ("Claps", &["808 Clap", "Tight Clap"][..]),
                ("Toms", &["808 Tom", "Conga"][..]),
                (
                    "Perc",
                    &["Cowbell", "Claves", "Rimshot", "Cymbal", "Zap Perc"][..],
                ),
            ] {
                ui.menu_button(group, |ui| {
                    for (name, tip, mut p) in crate::gen::presets() {
                        if !names.contains(&name) {
                            continue;
                        }
                        if ui.button(name).on_hover_text(tip).clicked() {
                            p.base.name = name.into();
                            self.add_instrument(name, InstrumentKind::Gen(p));
                            ui.close();
                        }
                    }
                });
            }
        });
    }
}
