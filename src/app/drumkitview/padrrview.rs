//! The Drum Kit editor's Round Robin tab (docs/ROUND_ROBIN.md, part B): the
//! selected pad's extra sounds, which one plays when, the Variation amount
//! with its per-part ranges, the seed, and a "Next 8 hits" row that shows
//! exactly what the next hits will play.
use super::*;
use crate::drumkit::padrr::{self, PadRr, DRIFT_UI, MAX_SOUNDS};
use crate::timeline::rr::Order;

const VARIATION: MacroDef = mix(
    "Variation",
    0.0,
    1.0,
    false,
    "%",
    0.3,
    "how much each hit drifts from the last: 0 = every hit exactly the same, 100 % = the full ranges below",
);

/// the colour of sound `i` in the sounds list and the Next 8 hits row
fn sound_col(i: usize) -> Color32 {
    let t = th();
    [t.accent, t.ochre, t.sage, t.brick][i % 4]
}

impl App {
    /// the selected pad's Round Robin; compact = the side panel
    pub(super) fn dk_rr(&mut self, ui: &mut egui::Ui, kit: &mut DrumKitP, compact: bool) {
        let sel = self.dk.sel;
        let n_pads = kit.pads.len();
        let Some(pad) = kit.pads.get_mut(sel) else {
            return;
        };
        let mut audition = false;
        let rr_sel = &mut self.dk.rr_sel;
        let mut body = |ui: &mut egui::Ui| {
            let mut on = pad.rr.is_some();
            ui.horizontal_wrapped(|ui| {
                if ui
                    .checkbox(&mut on, "On")
                    .on_hover_text(
                        "each hit of this pad takes the next of up to 4 sounds, with a little human \
                         drift, so repeated hits stop sounding like a machine gun. Off: every hit is the same",
                    )
                    .changed()
                {
                    pad.rr = on.then(PadRr::default);
                    *rr_sel = 0;
                }
                if !on {
                    ui.label(
                        egui::RichText::new("off: this pad plays the same sound every hit")
                            .color(th().ink2),
                    );
                }
            });
            let Some(r) = pad.rr.as_mut() else {
                return;
            };
            *rr_sel = (*rr_sel).min(r.alts.len());
            ui.horizontal_wrapped(|ui| {
                ui.label("Next 8 hits").on_hover_text(
                    "the sounds the next 8 hits of this pad play, from the start of the song",
                );
                let hits = padrr::preview(r, 0, 8);
                for (k, s) in hits.iter().enumerate() {
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(24.0, 20.0), egui::Sense::hover());
                    let c = sound_col(*s as usize);
                    ui.painter().rect_filled(rect, 4.0, c.gamma_multiply(0.35));
                    ui.painter().rect_stroke(
                        rect,
                        4.0,
                        egui::Stroke::new(1.0, c),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{}", s + 1),
                        crate::theme::prop(ts::BODY),
                        th().ink,
                    );
                    let m = padrr::hit_mod(r, k as u32);
                    resp.on_hover_text(format!(
                        "hit {}: sound {}, {:+.2} st, {:+.1} dB, {:+.0} % decay, {:.1} ms late",
                        k + 1,
                        s + 1,
                        m.semis,
                        20.0 * m.gain.max(1e-6).log10(),
                        (m.decay - 1.0) * 100.0,
                        m.delay * 1000.0
                    ));
                }
            });
            // sounds
            ui.horizontal_wrapped(|ui| {
                ui.label("Sounds");
                for i in 0..r.sounds() {
                    let d = if i == 0 { &pad.drum } else { &r.alts[i - 1] };
                    let model = d
                        .kind
                        .models()
                        .get(d.model as usize)
                        .copied()
                        .unwrap_or("?");
                    let txt = egui::RichText::new(format!("{} {model}", i + 1)).color(sound_col(i));
                    let tip = if i == 0 {
                        "the pad's own sound (edit it on the Sound tab)".to_string()
                    } else {
                        format!("sound {}: click to edit it below", i + 1)
                    };
                    if ui
                        .selectable_label(*rr_sel == i, txt)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        *rr_sel = i;
                    }
                }
                if r.sounds() < MAX_SOUNDS
                    && ui
                        .button("+ Sound")
                        .on_hover_text(
                            "adds another sound to take turns with: a copy of the last one with \
                             the next model, ready to tweak",
                        )
                        .clicked()
                {
                    let last = r.alts.last().unwrap_or(&pad.drum).clone();
                    let n = last.kind.models().len().max(1) as u8;
                    let mut d = DrumP::new(last.kind, (last.model + 1) % n, &[]);
                    d.macros = last.macros;
                    d.level = last.level;
                    r.alts.push(d);
                    *rr_sel = r.alts.len();
                    audition = true;
                }
                if *rr_sel > 0
                    && ui
                        .button("Remove")
                        .on_hover_text("removes the selected sound from the rotation")
                        .clicked()
                {
                    r.alts.remove(*rr_sel - 1);
                    *rr_sel -= 1;
                }
            });
            if *rr_sel > 0 {
                let d = &mut r.alts[*rr_sel - 1];
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Sound {}", *rr_sel + 1))
                            .color(sound_col(*rr_sel)),
                    );
                    let models = d.kind.models();
                    let before = d.model;
                    egui::ComboBox::from_id_salt(("dk_rr_model", sel, *rr_sel))
                        .width(100.0)
                        .selected_text(*models.get(d.model as usize).unwrap_or(&"?"))
                        .show_ui(ui, |ui| {
                            for (i, m) in models.iter().enumerate() {
                                ui.selectable_value(&mut d.model, i as u8, *m);
                            }
                        });
                    audition |= d.model != before;
                });
                ui.horizontal_wrapped(|ui| {
                    for (i, def) in d.kind.macros().iter().enumerate() {
                        let mut lock = false;
                        super::super::genview::knob(
                            ui,
                            &mut d.macros[i],
                            def,
                            &mut lock,
                            ("dkrr", sel, *rr_sel * 8 + i),
                        );
                    }
                });
            }
            ui.add_space(4.0);
            // order
            ui.horizontal_wrapped(|ui| {
                ui.label("Order")
                    .on_hover_text("which sound plays on each hit");
                for o in Order::ALL {
                    let on = r.order.same_kind(o);
                    if ui.radio(on, o.label()).on_hover_text(o.tip()).clicked() && !on {
                        r.order = o;
                    }
                }
                if let Order::EveryN { n, which } = &mut r.order {
                    ui.label("every");
                    ui.add(egui::DragValue::new(n).range(2..=16))
                        .on_hover_text("how many hits apart the other sound comes");
                    ui.label("hits plays sound");
                    let mut w = *which + 1;
                    if ui
                        .add(egui::DragValue::new(&mut w).range(2..=MAX_SOUNDS as u8))
                        .on_hover_text("the sound that takes that hit")
                        .changed()
                    {
                        *which = w - 1;
                    }
                }
            });
            // variation and its ranges
            ui.horizontal_wrapped(|ui| {
                mix_knob(ui, &mut r.variation, &VARIATION, &format!("rrv{sel}"));
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Ranges at full Variation")
                            .small()
                            .color(th().ink2),
                    );
                    egui::Grid::new(("dk_rr_drift", sel))
                        .num_columns(if compact { 2 } else { 6 })
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            for (i, (name, unit, tip)) in DRIFT_UI.iter().enumerate() {
                                let mut v = r.drift.get(i);
                                let max = padrr::Drift::MAX[i];
                                let pct = *unit == "%";
                                let mut shown = if pct { v * 100.0 } else { v };
                                let resp = ui
                                    .add(
                                        egui::DragValue::new(&mut shown)
                                            .range(0.0..=if pct { max * 100.0 } else { max })
                                            .speed(max / 200.0 * if pct { 100.0 } else { 1.0 })
                                            .max_decimals(2)
                                            .prefix(format!("{name} "))
                                            .suffix(if unit.is_empty() {
                                                String::new()
                                            } else {
                                                format!(" {unit}")
                                            }),
                                    )
                                    .on_hover_text(*tip);
                                if resp.changed() {
                                    v = if pct { shown / 100.0 } else { shown };
                                    r.drift.set(i, v);
                                }
                                if compact && i % 2 == 1 || !compact && i == 5 {
                                    ui.end_row();
                                }
                            }
                        });
                });
            });
            // seed and the preview
            ui.horizontal_wrapped(|ui| {
                ui.label("Seed")
                    .on_hover_text("the same seed always plays the same hits; New picks another");
                ui.add(egui::DragValue::new(&mut r.seed).hexadecimal(8, false, true));
                if ui
                    .button("New")
                    .on_hover_text("another seed: a different run of sounds and drift")
                    .clicked()
                {
                    r.seed = crate::timeline::ClipRng::new(r.seed, 1).next_u32();
                }
            });
        };
        if compact {
            ui.label(
                egui::RichText::new(format!("Pad {} of {n_pads}", sel + 1))
                    .small()
                    .color(th().ink2),
            );
            body(ui);
        } else {
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(egui::vec2(w, 196.0), egui::Sense::hover());
            card(ui, r, &format!("Round Robin · pad {} of {n_pads}", sel + 1));
            let inner = egui::Rect::from_min_max(
                r.min + egui::vec2(16.0, 26.0),
                r.max - egui::vec2(16.0, 10.0),
            );
            area(ui, inner, body);
        }
        if audition {
            let k = kit.clone();
            self.dk_audition(&k, sel, self.ch, 1.0);
        }
    }
}
