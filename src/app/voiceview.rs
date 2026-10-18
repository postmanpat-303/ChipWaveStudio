//! Voice (glitch) instrument editor panel and "New instrument" menu entries.
use super::*;
use crate::voice::{Register, VoicePatch, VoiceSource, Vowel};

/// beat lengths offered for step / stutter / chop (beats, label as note value)
const LENS: [(f32, &str); 8] = [
    (0.0, "off"),
    (1.0 / 16.0, "1/64"),
    (0.125, "1/32"),
    (0.25, "1/16"),
    (0.5, "1/8"),
    (1.0, "1/4"),
    (2.0, "1/2"),
    (4.0, "1 bar"),
];

fn len_combo(ui: &mut egui::Ui, id: &str, v: &mut f32, max: f32, tip: &str) -> bool {
    let cur = LENS
        .iter()
        .find(|l| (l.0 - *v).abs() < 1e-4)
        .map(|l| l.1.to_string())
        .unwrap_or_else(|| format!("{:.3} beat", v));
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .width(70.0)
        .selected_text(cur)
        .show_ui(ui, |ui| {
            for (b, n) in LENS.iter().filter(|l| l.0 <= max) {
                changed |= ui.selectable_value(v, *b, *n).changed();
            }
        })
        .response
        .on_hover_text(tip);
    changed
}

/// Edit the voice-specific part of a Voice patch; true when changed.
pub(super) fn voice_panel(ui: &mut egui::Ui, vp: &mut VoicePatch) -> bool {
    let t = th();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new("VOICE (glitch)")
                .strong()
                .color(t.accent),
        );
        egui::ComboBox::from_id_salt("voice_preset")
            .width(120.0)
            .selected_text("Load preset…")
            .show_ui(ui, |ui| {
                for (name, tip, p) in crate::voice::presets() {
                    if ui.button(name).on_hover_text(tip).clicked() {
                        let keep = std::mem::take(&mut vp.base.name);
                        *vp = p;
                        vp.base.name = keep;
                        changed = true;
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("replace every voice, envelope, filter and LFO setting with a preset");
    });
    // vowel palette: click adds to the sequence
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        ui.label(egui::RichText::new("Add").small().color(t.ink2));
        for v in Vowel::ALL {
            let b = egui::Button::new(
                egui::RichText::new(v.name())
                    .monospace()
                    .strong()
                    .color(if t.light { Color32::WHITE } else { t.value }),
            )
            .fill(t.slate)
            .min_size(egui::vec2(30.0, 20.0));
            if ui
                .add_enabled(vp.vowels.len() < 8, b)
                .on_hover_text(format!("add {} to the vowel sequence (max 8)", v.hint()))
                .clicked()
            {
                vp.vowels.push(v);
                changed = true;
            }
        }
    });
    // the sequence: click a chip to remove it
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        ui.label(egui::RichText::new("Seq").small().color(t.ink2));
        let mut remove = None;
        for (i, v) in vp.vowels.iter().enumerate() {
            let b = egui::Button::new(
                egui::RichText::new(v.name())
                    .monospace()
                    .strong()
                    .color(if t.light { Color32::WHITE } else { t.crt_bot }),
            )
            .fill(t.accent)
            .min_size(egui::vec2(30.0, 20.0));
            if ui
                .add(b)
                .on_hover_text(format!(
                    "step {}: {} - click to remove (31{:02X} jumps here)",
                    i + 1,
                    v.hint(),
                    i
                ))
                .clicked()
            {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            if vp.vowels.len() > 1 {
                vp.vowels.remove(i);
                changed = true;
            }
        }
    });
    // tract model
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(egui::RichText::new("Tract").small().color(t.ink2));
        changed |= ui
            .selectable_value(&mut vp.throat, false, "Formant")
            .on_hover_text("five formant filters (the classic ChipWave voice)")
            .changed();
        changed |= ui
            .selectable_value(&mut vp.throat, true, "Throat")
            .on_hover_text(
                "a simulated vocal tract (tube with tongue, lips and nose), inspired by \
                 Pink Trombone by Neil Thapen: rounder, more physical, more mouth noise",
            )
            .changed();
    });
    // words: each note speaks the next word (3Exx picks one)
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        ui.label(egui::RichText::new("Words").small().color(t.ink2))
            .on_hover_text(
                "each note speaks the next word; 3Exx speaks word xx. Letters: vowels a e i o u er ae, \
                 p b t d k g, s z sh f v h, m n ng; . = pause. Empty = vowels only.",
            );
        let mut remove = None;
        for (i, w) in vp.words.iter_mut().enumerate() {
            let r = ui.add(
                egui::TextEdit::singleline(w)
                    .desired_width(56.0)
                    .char_limit(48)
                    .hint_text("ka"),
            );
            r.clone().on_hover_text(format!(
                "word {:02X} (3E{:02X}): {}",
                i,
                i,
                crate::speech::describe(w)
            ));
            changed |= r.changed();
            if r.secondary_clicked() {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            vp.words.remove(i);
            changed = true;
        }
        if vp.words.len() < crate::speech::MAX_WORDS
            && ui
                .small_button("+ word")
                .on_hover_text("add a word (right-click a word to remove it)")
                .clicked()
        {
            let next = ["ka", "ta", "da", "ma", "sha", "pa", "na", "ga"];
            vp.words.push(next[vp.words.len() % next.len()].to_string());
            changed = true;
        }
        if !vp.words.is_empty() && ui.small_button("clear").clicked() {
            vp.words.clear();
            changed = true;
        }
    });
    egui::Grid::new("voice_grid")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("   Register");
            egui::ComboBox::from_id_salt("voice_reg")
                .selected_text(format!("{:?}", vp.register))
                .show_ui(ui, |ui| {
                    for r in Register::ALL {
                        changed |= ui
                            .selectable_value(&mut vp.register, r, format!("{r:?}"))
                            .changed();
                    }
                })
                .response
                .on_hover_text(
                    "formant table: Bass / Tenor / Alto / Soprano, or Demon \
                     (bass formants scaled down: a huge dark throat)",
                );
            ui.end_row();
            ui.label("   Source");
            egui::ComboBox::from_id_salt("voice_src")
                .selected_text(format!("{:?}", vp.source))
                .show_ui(ui, |ui| {
                    for s in VoiceSource::ALL {
                        changed |= ui
                            .selectable_value(&mut vp.source, s, format!("{s:?}"))
                            .changed();
                    }
                })
                .response
                .on_hover_text(
                    "Glottal = vocal-cord pulse (most voice-like), Saw = brighter, \
                     Pulse = robot / chip speech (uses Pulse width), Whisper = noise only",
                );
            ui.end_row();
            ui.label("   Step");
            changed |= len_combo(
                ui,
                "voice_step",
                &mut vp.step,
                4.0,
                "move to the next vowel of the sequence every … (tempo-synced); off = hold the first",
            );
            ui.end_row();
            ui.label("   Speech rate");
            changed |= ui
                .add(egui::Slider::new(&mut vp.rate, 0.25..=3.0).logarithmic(true).suffix(" x"))
                .on_hover_text("how fast words are spoken (consonant and vowel lengths, tongue speed)")
                .changed();
            ui.end_row();
            ui.label("   Morph");
            changed |= ui
                .add(egui::Slider::new(&mut vp.morph_ms, 0.0..=1000.0).suffix(" ms").logarithmic(true))
                .on_hover_text("glide time between vowels (0 = hard cut)")
                .changed();
            ui.end_row();
            ui.label("   Babble");
            changed |= ui
                .add(egui::Slider::new(&mut vp.babble, 0.0..=1.0))
                .on_hover_text("chance that a step jumps to a random vowel instead (needs Step)")
                .changed();
            ui.end_row();
            ui.label("   Formant shift");
            changed |= ui
                .add(egui::Slider::new(&mut vp.shift, 0.5..=2.0).logarithmic(true).suffix(" x"))
                .on_hover_text("moves all formants up/down without changing the pitch (< 1 = bigger, darker)")
                .changed();
            ui.end_row();
            ui.label("   Sharpness");
            changed |= ui
                .add(egui::Slider::new(&mut vp.sharp, 0.5..=3.0))
                .on_hover_text("narrower formants = whistly, robotic; wider = softer")
                .changed();
            ui.end_row();
            ui.label("   Breath");
            changed |= ui
                .add(egui::Slider::new(&mut vp.breath, 0.0..=1.0))
                .on_hover_text("aspiration noise (35xx sets it per row)")
                .changed();
            ui.end_row();
            ui.label("   Tension");
            changed |= ui
                .add(egui::Slider::new(&mut vp.tension, 0.0..=1.0))
                .on_hover_text("0 = soft, breathy pulse .. 1 = pressed, buzzy (Glottal source)")
                .changed();
            ui.end_row();
            ui.label("   Jitter");
            changed |= ui
                .add(egui::Slider::new(&mut vp.jitter, 0.0..=1.0))
                .on_hover_text("random pitch / level wobble per vocal-cord cycle (human, rough)")
                .changed();
            ui.end_row();
            ui.label("   LFO > vowel");
            changed |= ui
                .add(egui::Slider::new(&mut vp.lfo_vowel, -4.0..=4.0).suffix(" steps"))
                .on_hover_text("the patch LFO sweeps through the vowel sequence (set the LFO below)")
                .changed();
            ui.end_row();
            ui.label("   LFO > shift");
            changed |= ui
                .add(egui::Slider::new(&mut vp.lfo_shift, -12.0..=12.0).suffix(" st"))
                .on_hover_text("the patch LFO moves the formant shift (wah / throat wobble)")
                .changed();
            ui.end_row();
            ui.label("   Stutter");
            ui.horizontal(|ui| {
                changed |= len_combo(
                    ui,
                    "voice_stut",
                    &mut vp.stutter_len,
                    2.0,
                    "every note: capture this slice, then repeat it (beat-repeat); 33xy does it per row",
                );
                let mut n = vp.stutter_repeats as u32;
                if ui
                    .add(egui::DragValue::new(&mut n).range(0..=16).suffix(" x"))
                    .on_hover_text("repeats after the captured slice")
                    .changed()
                {
                    vp.stutter_repeats = n as u8;
                    changed = true;
                }
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut vp.stutter_pitch)
                            .range(-12.0..=12.0)
                            .speed(0.1)
                            .suffix(" st"),
                    )
                    .on_hover_text("pitch step per repeat (tape-style: up = faster, down = slower)")
                    .changed();
            });
            ui.end_row();
            ui.label("   Chop");
            ui.horizontal(|ui| {
                changed |= len_combo(
                    ui,
                    "voice_chop",
                    &mut vp.chop_rate,
                    2.0,
                    "tempo-synced gate: the voice opens and closes every … (37xy per row)",
                );
                changed |= ui
                    .add(egui::Slider::new(&mut vp.chop_duty, 0.05..=1.0).text("open"))
                    .on_hover_text("how much of each chop cycle is open")
                    .changed();
            });
            ui.end_row();
        });
    changed |= transform_panel(ui, &mut vp.transform);
    if changed {
        vp.fix();
    }
    changed
}

fn slider(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f32,
    r: std::ops::RangeInclusive<f32>,
    tip: &str,
) -> bool {
    ui.label(format!("   {label}"));
    let c = ui.add(egui::Slider::new(v, r)).on_hover_text(tip).changed();
    ui.end_row();
    c
}

fn slider_u(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f32,
    r: std::ops::RangeInclusive<f32>,
    suffix: &str,
    log: bool,
    tip: &str,
) -> bool {
    ui.label(format!("   {label}"));
    let c = ui
        .add(egui::Slider::new(v, r).suffix(suffix).logarithmic(log))
        .on_hover_text(tip)
        .changed();
    ui.end_row();
    c
}

// set by `--screenshot --view transform`: scroll the panel to the Transform
thread_local! {
    pub(super) static TF_SCROLL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The Transform section (0.2.3.11): blend, order and the three stages.
fn transform_panel(ui: &mut egui::Ui, tf: &mut crate::transform::Transform) -> bool {
    use crate::transform::{Order, Ratios, SubMode};
    let t = th();
    let mut changed = false;
    ui.add_space(6.0);
    // --screenshot --view transform scrolls the panel here once
    if TF_SCROLL.with(|f| f.replace(false)) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
    }
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label(egui::RichText::new("TRANSFORM").strong().color(t.accent))
            .on_hover_text(
                "metallic resonance, digital distortion and a sub-bass layer after the voice. \
                 Blend 0 = off. Pattern: 3Fxx blend, 40xx resonance, 41xx digital, 42xx sub \
                 (00 = instrument, 01-FF = 0-100 %)",
            );
    });
    egui::Grid::new("tf_grid")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            changed |= slider(
                ui,
                "Blend",
                &mut tf.blend,
                0.0..=1.0,
                "dry voice (0) .. fully transformed (1); 3Fxx per row",
            );
            ui.label("   Order");
            ui.horizontal(|ui| {
                changed |= ui
                    .selectable_value(&mut tf.order, Order::DistRes, "Dist > Res")
                    .on_hover_text("distortion first, the resonators ring on the grit")
                    .changed();
                changed |= ui
                    .selectable_value(&mut tf.order, Order::ResDist, "Res > Dist")
                    .on_hover_text("resonators first, then the ringing gets crushed")
                    .changed();
            });
            ui.end_row();
        });
    egui::CollapsingHeader::new("Metallic resonance")
        .id_salt("tf_res")
        .default_open(true)
        .show(ui, |ui| {
            let r = &mut tf.res;
            egui::Grid::new("tf_res_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                changed |= slider(ui, "Amount", &mut r.amount, 0.0..=1.0, "resonator mix (40xx per row)");
                ui.label("   Ratios");
                egui::ComboBox::from_id_salt("tf_ratios")
                    .selected_text(r.ratios.name())
                    .show_ui(ui, |ui| {
                        for x in Ratios::ALL {
                            changed |= ui.selectable_value(&mut r.ratios, x, x.name()).changed();
                        }
                    })
                    .response
                    .on_hover_text(
                        "mode ratios: harmonic = robotic comb, metal = 808 cymbal, bell = church bell, \
                         bar = struck bar (glassy), odd = hollow square",
                    );
                ui.end_row();
                ui.label("   Pitch");
                ui.horizontal(|ui| {
                    changed |= ui.selectable_value(&mut r.keyed, true, "Note").on_hover_text("tuned to the note played").changed();
                    changed |= ui.selectable_value(&mut r.keyed, false, "Fixed").on_hover_text("always the fixed pitch below").changed();
                });
                ui.end_row();
                if r.keyed {
                    changed |= slider_u(ui, "Tune", &mut r.tune, -24.0..=24.0, " st", false, "semitones from the note");
                } else {
                    changed |= slider_u(ui, "Fixed pitch", &mut r.hz, 30.0..=4000.0, " Hz", true, "resonator base pitch");
                }
                changed |= slider_u(ui, "Decay", &mut r.decay_ms, 20.0..=4000.0, " ms", true, "ring time (feedback): long = singing metal");
                changed |= slider(ui, "Damping", &mut r.damp, 0.0..=1.0, "higher modes die faster and the ring gets darker");
                changed |= slider(ui, "Spread", &mut r.spread, 0.0..=1.0, "detunes the modes apart, left vs right (stereo shimmer)");
            });
        });
    egui::CollapsingHeader::new("Digital distortion")
        .id_salt("tf_dig")
        .default_open(true)
        .show(ui, |ui| {
            let d = &mut tf.dig;
            egui::Grid::new("tf_dig_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    changed |= slider(
                        ui,
                        "Amount",
                        &mut d.amount,
                        0.0..=1.0,
                        "distortion mix (41xx per row)",
                    );
                    changed |= slider_u(
                        ui,
                        "Drive",
                        &mut d.drive_db,
                        0.0..=36.0,
                        " dB",
                        false,
                        "gain into the clipper / folder",
                    );
                    ui.label("   Shape");
                    ui.horizontal(|ui| {
                        changed |= ui
                            .selectable_value(&mut d.fold, false, "Clip")
                            .on_hover_text("hard clip")
                            .changed();
                        changed |= ui
                            .selectable_value(&mut d.fold, true, "Fold")
                            .on_hover_text("wavefold (in phase with the input)")
                            .changed();
                    });
                    ui.end_row();
                    changed |= slider_u(
                        ui,
                        "Bits",
                        &mut d.bits,
                        1.0..=16.0,
                        " bit",
                        false,
                        "bit depth (16 = off)",
                    );
                    changed |= slider_u(
                        ui,
                        "Rate",
                        &mut d.rate,
                        300.0..=48000.0,
                        " Hz",
                        true,
                        "sample-rate reduction",
                    );
                    ui.label("   Aliasing");
                    changed |= ui
                        .checkbox(&mut d.alias, "raw (aliasing)")
                        .on_hover_text(
                            "on: plain sample-and-hold, metallic aliasing; off: filtered, cleaner",
                        )
                        .changed();
                    ui.end_row();
                    changed |= slider(
                        ui,
                        "Ring mod",
                        &mut d.ring,
                        0.0..=1.0,
                        "ring modulator depth (robot / bell tone)",
                    );
                    changed |= slider_u(
                        ui,
                        "Ring ratio",
                        &mut d.ring_ratio,
                        0.25..=8.0,
                        " x",
                        true,
                        "ring frequency = note x ratio",
                    );
                    changed |= slider_u(
                        ui,
                        "Pre HP",
                        &mut d.hp,
                        20.0..=2000.0,
                        " Hz",
                        true,
                        "high-pass before the distortion (less mud)",
                    );
                    changed |= slider_u(
                        ui,
                        "Post LP",
                        &mut d.lp,
                        1000.0..=20000.0,
                        " Hz",
                        true,
                        "low-pass after it (tames fizz)",
                    );
                });
        });
    egui::CollapsingHeader::new("Sub bass")
        .id_salt("tf_sub")
        .default_open(true)
        .show(ui, |ui| {
            let b = &mut tf.sub;
            egui::Grid::new("tf_sub_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                changed |= slider(ui, "Level", &mut b.amount, 0.0..=1.0, "sub layer level (42xx per row)");
                ui.label("   Mode");
                egui::ComboBox::from_id_salt("tf_submode")
                    .selected_text(b.mode.name())
                    .show_ui(ui, |ui| {
                        for x in SubMode::ALL {
                            changed |= ui.selectable_value(&mut b.mode, x, x.name()).changed();
                        }
                    })
                    .response
                    .on_hover_text("sine = follows the note pitch; divider = octave divider on the voice itself");
                ui.end_row();
                ui.label("   Octave");
                ui.horizontal(|ui| {
                    changed |= ui.selectable_value(&mut b.octave, 1, "-1").changed();
                    changed |= ui.selectable_value(&mut b.octave, 2, "-2").changed();
                });
                ui.end_row();
                changed |= slider(ui, "Drive", &mut b.drive, 0.0..=1.0, "sub saturation (harmonics, not level)");
                changed |= slider(ui, "Growl", &mut b.growl, 0.0..=1.0, "level of the saturation harmonics above the crossover");
                changed |= slider_u(ui, "Crossover", &mut b.xover, 60.0..=250.0, " Hz", true, "below this the sub is clean and everything is mono");
                changed |= slider_u(ui, "Formant drop", &mut b.formant_drop, 0.0..=12.0, " st", false, "lowers the formants with the blend: a monster voice");
            });
        });
    changed
}

impl App {
    /// Append a new instrument and select it.
    pub(super) fn add_instrument(&mut self, name: &str, kind: InstrumentKind) {
        self.snapshot();
        let n = self.with(|s| {
            if s.song.instruments.len() >= 255 {
                return None;
            }
            s.song.instruments.push(Instrument {
                name: name.into(),
                volume: 0.8,
                transpose: 0,
                fx: vec![],
                kind,
            });
            Some(s.song.instruments.len())
        });
        match n {
            Some(n) => {
                self.instr = n as u8;
                self.dirty = true;
                self.status = format!("New instrument {n:02X} {name}");
            }
            None => self.status = "All 255 instrument slots are used".into(),
        }
    }

    /// "New instrument" menu: Synth (init) and Voice (glitch) presets.
    pub(super) fn new_instrument_menu(&mut self, ui: &mut egui::Ui) {
        if ui
            .button("Synth (init)")
            .on_hover_text("a plain saw synth to start from")
            .clicked()
        {
            self.add_instrument("Synth", InstrumentKind::Synth(Default::default()));
            ui.close();
        }
        self.new_generator_menu(ui);
        self.new_plugin_menu_item(ui);
        ui.menu_button("Voice (glitch)", |ui| {
            ui.label(
                egui::RichText::new("synthetic vowels: ahh eee ooh err")
                    .small()
                    .color(th().ink2),
            );
            for (name, tip, mut p) in crate::voice::presets() {
                if ui.button(name).on_hover_text(tip).clicked() {
                    p.base.name = name.into();
                    self.add_instrument(name, InstrumentKind::Voice(p));
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .button("Init voice")
                .on_hover_text("plain bass \"ahh\"")
                .clicked()
            {
                self.add_instrument("Voice", InstrumentKind::Voice(VoicePatch::default()));
                ui.close();
            }
        });
    }
}
