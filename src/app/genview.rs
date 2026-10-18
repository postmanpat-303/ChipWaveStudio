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
    let is_fx = matches!(g.engine, GenEngine::ChipFx(_));
    let own = g.engine.own_env();
    st.sel = st.sel.min(7);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new(if is_kit {
                "GENERATOR · DRUM KIT"
            } else if is_fx {
                "GENERATOR · CHIP FX"
            } else if matches!(g.engine, GenEngine::Analog(_)) {
                "GENERATOR · ANALOG"
            } else if matches!(g.engine, GenEngine::Fm(_)) {
                "GENERATOR · FM-4"
            } else if matches!(g.engine, GenEngine::Noise(_)) {
                "GENERATOR · NOISE"
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
    if is_fx {
        chipfx_buttons(ui, g, st, &mut out);
    }
    if own {
        out.changed |= g2_choices(ui, g);
    }
    // kind + model of the shown drum
    if let Some(d) = g.drum_mut(sel) {
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
    if is_fx {
        chipfx_knobs(ui, g, &mut out);
    }
    if own {
        out.changed |= g2_knobs(ui, g);
    }
    // the eight macro knobs
    if let Some(d) = g.drum_mut(sel) {
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
    if !own {
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
            egui::RichText::new(if is_fx {
                "43xy: knob x (1-8) = y (0-F), read at note start"
            } else {
                "43xy: macro x (1-8) = y (0-F)"
            })
            .small()
            .color(th().ink2),
        );
    });
    out
}

/// a lock bit in a u64 mask, toggled by the knob's "lock"
fn knob64(
    ui: &mut egui::Ui,
    v: &mut f32,
    def: &crate::gen::MacroDef,
    locks: &mut u64,
    bit: usize,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> bool {
    let mut l = *locks & (1 << bit) != 0;
    let ch = knob(ui, v, def, &mut l, id);
    let was = *locks & (1 << bit) != 0;
    if l != was {
        *locks ^= 1 << bit;
    }
    ch || l != was
}

/// knobs `idx` in rows of `per`
fn knob_rows(
    ui: &mut egui::Ui,
    p: &mut [f32],
    defs: &[crate::gen::MacroDef],
    locks: &mut u64,
    idx: &[usize],
    per: usize,
    tag: &str,
) -> bool {
    let mut ch = false;
    for r in idx.chunks(per) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            for &i in r {
                ch |= knob64(ui, &mut p[i], &defs[i], locks, i, (tag, i));
            }
        });
    }
    ch
}

/// a lockable discrete choice ("lock" toggles bit `bit` of `locks`)
fn choice_lock_ui(ui: &mut egui::Ui, locks: &mut u64, k: u32) -> bool {
    let b = crate::gen2::choice_lock(k);
    if ui
        .selectable_label(*locks & b != 0, "lock")
        .on_hover_text("keep this choice on Dice")
        .clicked()
    {
        *locks ^= b;
        return true;
    }
    false
}

fn mode_combo(ui: &mut egui::Ui, id: &str, m: &mut FilterMode) -> bool {
    let mut ch = false;
    egui::ComboBox::from_id_salt(id)
        .width(46.0)
        .selected_text(format!("{m:?}"))
        .show_ui(ui, |ui| {
            for x in [FilterMode::Lp, FilterMode::Bp, FilterMode::Hp] {
                ch |= ui.selectable_value(m, x, format!("{x:?}")).changed();
            }
        });
    ch
}

/// stage-2 generators: waveforms / algorithm / colour and filter choices
fn g2_choices(ui: &mut egui::Ui, g: &mut GenPatch) -> bool {
    use crate::gen2::{AWave, Color, ALGOS};
    let mut ch = false;
    let (glide, legato) = (&mut g.base.glide, &mut g.base.legato);
    match &mut g.engine {
        GenEngine::Analog(a) => {
            for k in 0..3 {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(format!("Osc {}", k + 1));
                    for w in AWave::ALL {
                        ch |= ui.selectable_value(&mut a.waves[k], w, w.name()).changed();
                    }
                    ch |= choice_lock_ui(ui, &mut a.locks, k as u32);
                });
            }
            ui.horizontal_wrapped(|ui| {
                ui.add_space(14.0);
                ch |= ui
                    .checkbox(&mut a.sync, "Sync 2>1")
                    .on_hover_text("hard-sync oscillator 2 to oscillator 1")
                    .changed();
                ch |= ui
                    .checkbox(&mut a.vintage, "Vintage")
                    .on_hover_text("saturated, 2x oversampled ladder (else clean)")
                    .changed();
                ch |= ui.checkbox(&mut a.pole4, "24 dB").changed();
                ch |= mode_combo(ui, "an_mode", &mut a.mode);
            });
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label("Glide");
                let mut ms = *glide * 1000.0;
                if ui
                    .add(egui::Slider::new(&mut ms, 0.0..=1000.0).suffix(" ms"))
                    .on_hover_text("portamento time (with Legato, or 3C in the pattern)")
                    .changed()
                {
                    *glide = ms / 1000.0;
                    ch = true;
                }
                ch |= ui
                    .checkbox(legato, "Legato")
                    .on_hover_text("overlapping notes glide instead of retriggering (mono)")
                    .changed();
            });
        }
        GenEngine::Fm(f) => {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label("Algorithm");
                egui::ComboBox::from_id_salt("fm_algo")
                    .width(130.0)
                    .selected_text(ALGOS[f.algo.min(7) as usize].2)
                    .show_ui(ui, |ui| {
                        for (i, a) in ALGOS.iter().enumerate() {
                            ch |= ui.selectable_value(&mut f.algo, i as u8, a.2).changed();
                        }
                    });
                ch |= choice_lock_ui(ui, &mut f.locks, 0);
            });
        }
        GenEngine::Noise(n) => {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                for c in Color::ALL {
                    ch |= ui.selectable_value(&mut n.color, c, c.name()).changed();
                }
                ch |= choice_lock_ui(ui, &mut n.locks, 0);
            });
            ui.horizontal_wrapped(|ui| {
                ui.add_space(14.0);
                ch |= ui
                    .checkbox(&mut n.ladder, "Ladder")
                    .on_hover_text("Moog ladder (else the SVF)")
                    .changed();
                ch |= mode_combo(ui, "nz_mode", &mut n.mode);
                ch |= ui
                    .checkbox(&mut n.hold, "Hold")
                    .on_hover_text(
                        "stay at the peak while the note is held (wind, vinyl); else one-shot",
                    )
                    .changed();
                ch |= ui
                    .checkbox(&mut n.key_track, "Key track")
                    .on_hover_text("the note moves the cutoff (C-4 = as set)")
                    .changed();
            });
        }
        _ => {}
    }
    ch
}

/// stage-2 generators: the eight macros, level, and every other knob
fn g2_knobs(ui: &mut egui::Ui, g: &mut GenPatch) -> bool {
    use crate::gen2::{fk, AN, ANALOG, FM, FN, NN, NOISE};
    let mut ch = false;
    let macros: Vec<usize> = (0..8).collect();
    let (p, defs, locks, level, n, tag): (
        &mut [f32],
        &[crate::gen::MacroDef],
        &mut u64,
        &mut f32,
        usize,
        &str,
    ) = match &mut g.engine {
        GenEngine::Analog(a) => (&mut a.p, &ANALOG, &mut a.locks, &mut a.level, AN, "an"),
        GenEngine::Fm(f) => (&mut f.p, &FM, &mut f.locks, &mut f.level, FN, "fm"),
        GenEngine::Noise(x) => (&mut x.p, &NOISE, &mut x.locks, &mut x.level, NN, "nz"),
        _ => return false,
    };
    ch |= knob_rows(ui, p, defs, locks, &macros, 4, tag);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label("Level");
        ch |= ui.add(egui::Slider::new(level, 0.0..=4.0)).changed();
    });
    egui::CollapsingHeader::new(if tag == "fm" {
        "Operators"
    } else {
        "More knobs"
    })
    .id_salt((tag, "more"))
    .show(ui, |ui| {
        if tag == "fm" {
            for o in 0..4 {
                ui.label(
                    egui::RichText::new(format!("Operator {}", o + 1))
                        .small()
                        .strong(),
                );
                let idx: Vec<usize> = (0..6).map(|j| fk(o, j)).collect();
                ch |= knob_rows(ui, p, defs, locks, &idx, 3, tag);
            }
        } else {
            let idx: Vec<usize> = (8..n).collect();
            ch |= knob_rows(ui, p, defs, locks, &idx, 4, tag);
        }
    });
    ch
}

/// Chip FX: category buttons, Randomize, waveform
fn chipfx_buttons(ui: &mut egui::Ui, g: &mut GenPatch, st: &mut GenUi, out: &mut GenOut) {
    use crate::chipfx::{Category, Wave, WAVE_LOCK};
    let before = g.clone();
    let seed = new_seed(g.seed);
    let mut act: Option<Option<Category>> = None;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        for c in Category::ALL {
            if ui
                .button(c.name())
                .on_hover_text(format!(
                    "a new {} sound (sfxr recipe; locked knobs stay)",
                    c.name()
                ))
                .clicked()
            {
                act = Some(Some(c));
            }
        }
        if ui
            .button("Randomize")
            .on_hover_text("sfxr's Randomize: any sound at all (locked knobs stay)")
            .clicked()
        {
            act = Some(None);
        }
    });
    if let Some(a) = act {
        g.seed = seed;
        if let GenEngine::ChipFx(s) = &mut g.engine {
            match a {
                Some(c) => s.category(c, seed),
                None => s.randomize(seed),
            }
        }
        st.record(&before, g);
        out.changed = true;
    }
    let GenEngine::ChipFx(s) = &mut g.engine else {
        return;
    };
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label("Wave");
        egui::ComboBox::from_id_salt("sfx_wave")
            .width(80.0)
            .selected_text(s.wave.name())
            .show_ui(ui, |ui| {
                for w in Wave::ALL {
                    out.changed |= ui.selectable_value(&mut s.wave, w, w.name()).changed();
                }
            });
        let mut l = s.locks & WAVE_LOCK != 0;
        if ui
            .selectable_label(l, "lock")
            .on_hover_text("keep the wave on rolls")
            .clicked()
        {
            l = !l;
            s.locks ^= WAVE_LOCK;
            out.changed = true;
        }
        let _ = l;
        out.changed |= ui
            .checkbox(&mut s.follow_note, "Follow note")
            .on_hover_text("C-4 plays the sound as designed; other notes transpose it")
            .changed();
    });
}

/// Chip FX: the eight macro knobs (43xy targets) and every sfxr parameter
fn chipfx_knobs(ui: &mut egui::Ui, g: &mut GenPatch, out: &mut GenOut) {
    use crate::chipfx::{MACROS, N, PARAMS};
    let GenEngine::ChipFx(s) = &mut g.engine else {
        return;
    };
    fn one(ui: &mut egui::Ui, i: usize, s: &mut crate::chipfx::SfxP) -> bool {
        let def = &PARAMS[i];
        let mut v = def.unmap(s.p[i]);
        let mut locked = s.locks & (1 << i) != 0;
        let ch = knob(ui, &mut v, def, &mut locked, ("sfx", i));
        if ch {
            s.p[i] = def.map(v);
        }
        let was = s.locks & (1 << i) != 0;
        if locked != was {
            s.locks ^= 1 << i;
        }
        ch || locked != was
    }
    for row in 0..2 {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            for &i in &MACROS[row * 4..row * 4 + 4] {
                out.changed |= one(ui, i, s);
            }
        });
    }
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label("Level");
        out.changed |= ui.add(egui::Slider::new(&mut s.level, 0.0..=1.5)).changed();
    });
    egui::CollapsingHeader::new("All sfxr parameters")
        .id_salt("sfx_all")
        .show(ui, |ui| {
            let rest: Vec<usize> = (0..N).filter(|i| !MACROS.contains(i)).collect();
            for r in rest.chunks(4) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for &i in r {
                        out.changed |= one(ui, i, s);
                    }
                });
            }
        });
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
        for (menu, tip, pick) in [
            (
                "Generator (analog)",
                "Moog-style: 3 drifting oscillators, ladder, 2 envelopes, glide",
                0u8,
            ),
            ("Generator (FM-4)", "4-operator FM, 8 algorithms", 1),
            (
                "Generator (noise)",
                "risers, downlifters, impacts, wind, vinyl",
                2,
            ),
        ] {
            ui.menu_button(menu, |ui| {
                ui.label(egui::RichText::new(tip).small().color(th().ink2));
                for (name, tip, mut p) in crate::gen::presets() {
                    let ok = match pick {
                        0 => matches!(p.engine, GenEngine::Analog(_)),
                        1 => matches!(p.engine, GenEngine::Fm(_)),
                        _ => matches!(p.engine, GenEngine::Noise(_)),
                    };
                    if !ok {
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
        ui.menu_button("Generator (chip FX)", |ui| {
            ui.label(
                egui::RichText::new("sfxr-style game sounds: categories, Randomize, Mutate")
                    .small()
                    .color(th().ink2),
            );
            for (name, tip, mut p) in crate::gen::presets() {
                if !matches!(p.engine, crate::gen::GenEngine::ChipFx(_)) {
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
}
