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
    if changed {
        vp.fix();
    }
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
