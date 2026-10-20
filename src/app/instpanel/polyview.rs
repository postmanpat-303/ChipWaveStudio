//! The poly synth's own editor tab (Instrument panel › the synth's tab, named
//! `poly::POLY_DISPLAY_NAME`). Wide panels show every section at once
//! (layout A); narrow ones switch to one section at a time behind a row of
//! section tabs (layout B). Every knob and choice is the instrument panel's
//! own parameter (`params.rs`), so search, automation lanes, MIDI learn and
//! drag-to-modulate work here exactly as on the cards.
use super::*;
use crate::gen::GenEngine;
use crate::poly::*;

#[derive(Default)]
pub(in crate::app) struct PolyUi {
    /// layout B: the section shown
    pub sec: usize,
    /// the patch as it was when the instrument was opened or a preset
    /// loaded: (instrument, patch, name) for Revert
    pub loaded: Option<(usize, crate::gen::GenPatch)>,
    /// the last layout drawn was the wide one (tests, screenshots)
    pub wide: bool,
}

/// below this width the editor switches to layout B
pub(in crate::app) const WIDE_MIN: f32 = 1300.0;

/// sections: (title, card whose picture it shows, LED choices, other
/// choices, knobs, knobs per row)
struct Sec {
    title: &'static str,
    viz: Option<&'static str>,
    leds: &'static [&'static str],
    ctls: &'static [&'static str],
    knobs: Vec<usize>,
    cols: usize,
}

fn osc_knobs(o: usize) -> Vec<usize> {
    [
        SHAPE, PW, OCT, PITCH, SUB, SUB_PW, SYNC, VOICES, DETUNE, WIDTH, OPAN,
    ]
    .iter()
    .map(|k| OSC[o] + k)
    .collect()
}

fn sections() -> Vec<Sec> {
    vec![
        Sec {
            title: "Osc 1",
            viz: Some("Osc 1"),
            leds: &[],
            ctls: &["ps.reset0"],
            knobs: osc_knobs(0),
            cols: 6,
        },
        Sec {
            title: "Osc 2",
            viz: Some("Osc 2"),
            leds: &[],
            ctls: &["ps.reset1"],
            knobs: osc_knobs(1),
            cols: 6,
        },
        Sec {
            title: "Blend & noise",
            viz: None,
            leds: &["ps.blend"],
            ctls: &[],
            knobs: vec![BLEND, NOISE, COLOUR],
            cols: 3,
        },
        Sec {
            title: "Filter",
            viz: Some("Filter"),
            leds: &["ps.filter"],
            ctls: &[],
            knobs: vec![CUTOFF, RES, ENV_AMT, KEYTRK, VEL_CUT, FM_AMT, FM_FREQ],
            cols: 4,
        },
        Sec {
            title: "Drive & shaper",
            viz: None,
            leds: &["ps.shaper"],
            ctls: &[],
            knobs: vec![HP_CUT, HP_RES, DRIVE, FEEDBACK, SHAPER, SHAPER_MIX],
            cols: 3,
        },
        Sec {
            title: "Filter envelope",
            viz: Some("Filter envelope"),
            leds: &[],
            ctls: &[],
            knobs: (FEG..FEG + 4).collect(),
            cols: 4,
        },
        Sec {
            title: "Amp envelope",
            viz: Some("Amp envelope"),
            leds: &[],
            ctls: &[],
            knobs: (AEG..AEG + 4).collect(),
            cols: 4,
        },
        Sec {
            title: "Voice",
            viz: None,
            leds: &["ps.mode"],
            ctls: &["ps.polyphony"],
            knobs: vec![GLIDE, VEL_AMP, GAIN, PAN],
            cols: 4,
        },
    ]
}

/// layout B's section tabs: (name, sections shown)
const TABS: [(&str, &[usize]); 6] = [
    ("Osc", &[0, 1]),
    ("Mix", &[2, 4]),
    ("Filter", &[3]),
    ("Envelopes", &[5, 6]),
    ("Voice", &[7]),
    ("Mod", &[8]),
];

fn find<'a>(l: &'a Layout, key: &str) -> Option<&'a Param> {
    l.params.iter().find(|p| p.key == key)
}

impl App {
    /// the poly synth editor (its own tab)
    pub(super) fn poly_panel(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let InstrumentKind::Gen(g) = &inst.kind else {
            return;
        };
        if !matches!(g.engine, GenEngine::Poly(_)) {
            return;
        }
        if self.ip.poly.loaded.as_ref().map(|l| l.0) != Some(cx.idx) {
            self.ip.poly.loaded = Some((cx.idx, g.clone()));
        }
        self.poly_header(ui, inst, cx);
        let wide = ui.available_width() >= WIDE_MIN;
        self.ip.poly.wide = wide;
        let secs = sections();
        egui::ScrollArea::vertical()
            .id_salt(("poly_body", cx.idx))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if wide {
                    // layout A: oscillators, mix, filter on top; drive,
                    // envelopes, voice and the routes below
                    for row in [&[0usize, 1, 2, 3][..], &[4, 5, 6, 7][..], &[8][..]] {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            for &s in row {
                                self.poly_section(ui, layout, &secs, s, inst, cx);
                            }
                        });
                        ui.add_space(6.0);
                    }
                } else {
                    // layout B: one group at a time
                    ui.horizontal(|ui| {
                        for (i, (name, _)) in TABS.iter().enumerate() {
                            if crate::ember::tab(ui, self.ip.poly.sec == i, name).clicked() {
                                self.ip.poly.sec = i;
                            }
                        }
                    });
                    ui.add_space(4.0);
                    let sec = self.ip.poly.sec.min(TABS.len() - 1);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        for &s in TABS[sec].1 {
                            self.poly_section(ui, layout, &secs, s, inst, cx);
                        }
                    });
                }
            });
    }

    /// title, presets, Init / Revert, Voices and Load
    fn poly_header(&mut self, ui: &mut egui::Ui, inst: &mut Instrument, cx: &mut Cx) {
        let t = th();
        let idx = cx.idx;
        let InstrumentKind::Gen(g) = &mut inst.kind else {
            return;
        };
        let voices_per_note = match &g.engine {
            GenEngine::Poly(x) => x.osc_voices(),
            _ => 1,
        };
        let mut load: Option<crate::gen::GenPatch> = None;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(POLY_DISPLAY_NAME.to_uppercase())
                    .strong()
                    .color(t.accent),
            );
            ui.add_space(10.0);
            egui::ComboBox::from_id_salt(("poly_preset", idx))
                .width(170.0)
                .selected_text(if g.base.name.is_empty() {
                    "Presets".to_string()
                } else {
                    g.base.name.clone()
                })
                .show_ui(ui, |ui| {
                    for (name, tip, p) in crate::gen::presets() {
                        if !matches!(p.engine, GenEngine::Poly(_)) {
                            continue;
                        }
                        if ui.button(name).on_hover_text(tip).clicked() {
                            let mut p = p;
                            p.base.name = name.into();
                            load = Some(p);
                            ui.close();
                        }
                    }
                });
            if crate::ember::button(ui, "Init", false)
                .on_hover_text("start from one plain saw")
                .clicked()
            {
                let mut p =
                    crate::gen::preset(concat!(crate::poly_name!(), " Init")).unwrap_or_default();
                p.base.name = concat!(crate::poly_name!(), " Init").into();
                load = Some(p);
            }
            let can_revert = self
                .ip
                .poly
                .loaded
                .as_ref()
                .is_some_and(|(i, p)| *i == idx && p != &*g);
            if ui
                .add_enabled_ui(can_revert, |ui| crate::ember::button(ui, "Revert", false))
                .inner
                .on_hover_text("back to the patch as it was loaded (undo still works)")
                .clicked()
            {
                if let Some((_, p)) = &self.ip.poly.loaded {
                    *g = p.clone();
                    cx.changed = true;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let load_pct = self.audio.as_ref().map(|a| a.load_pct());
                let voices = self.with(|s| s.engine.inst_voices(idx));
                ui.label(
                    egui::RichText::new(match load_pct {
                        Some(l) => format!("LOAD {l:.0} %"),
                        None => "LOAD –".into(),
                    })
                    .small()
                    .monospace()
                    .color(t.ink2),
                )
                .on_hover_text("audio engine load, % of real time (all tracks)");
                ui.label(
                    egui::RichText::new(format!(
                        "VOICES {voices}  ·  {voices_per_note} OSC/NOTE"
                    ))
                    .small()
                    .monospace()
                    .color(t.ink2),
                )
                .on_hover_text(
                    "notes sounding on this instrument now, and oscillator voices per note (unison of both oscillators)",
                );
            });
        });
        if let Some(p) = load {
            *g = p.clone();
            g.sync_poly();
            self.ip.poly.loaded = Some((idx, g.clone()));
            cx.changed = true;
        }
    }

    /// one section: a panel with its picture, LED choices, choices, knobs
    fn poly_section(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        secs: &[Sec],
        s: usize,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let t = th();
        let (title, w) = if s == 8 {
            ("Mod routes", 4.0 * CTL_W + 24.0)
        } else {
            let sc = &secs[s];
            (sc.title, sc.cols as f32 * CELL.x + 16.0)
        };
        egui::Frame::new()
            .fill(t.cart_bot)
            .stroke(Stroke::new(1.0, t.ink2.gamma_multiply(0.35)))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(w - 12.0);
                ui.set_max_width(w - 12.0);
                ui.label(
                    egui::RichText::new(title.to_uppercase())
                        .size(11.5)
                        .strong()
                        .color(t.ink),
                );
                if s == 8 {
                    self.poly_routes(ui, layout, inst, cx);
                    return;
                }
                let sc = &secs[s];
                if let Some(card) = sc.viz {
                    if let Some(c) = layout.cards.iter().find(|c| c.name == card) {
                        if let Some(v) = &c.viz {
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(w - 12.0, 46.0), Sense::hover());
                            knob::draw_viz(&ui.painter_at(r), r, &v(inst));
                        }
                    }
                }
                for key in sc.leds {
                    if let Some(p) = find(layout, key) {
                        led_row(ui, p, inst, cx);
                    }
                }
                for key in sc.ctls {
                    if let Some(p) = find(layout, key) {
                        ui.horizontal(|ui| self.ctl_ui(ui, p, inst, cx));
                    }
                }
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                for row in sc.knobs.chunks(sc.cols) {
                    ui.horizontal(|ui| {
                        for k in row {
                            if let Some(p) = find(layout, &format!("ps.{k}")) {
                                self.knob_ui(ui, p, inst, cx);
                            }
                        }
                    });
                }
            });
    }

    /// the eight routes (two rows of four): source, destination, amount
    fn poly_routes(
        &mut self,
        ui: &mut egui::Ui,
        layout: &Layout,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        for half in [0..4, 4..MOD_SLOTS] {
            ui.horizontal_top(|ui| {
                for s in half.clone() {
                    ui.vertical(|ui| {
                        ui.set_width(CTL_W);
                        for key in [format!("ps.src{s}"), format!("ps.dst{s}")] {
                            if let Some(p) = find(layout, &key) {
                                self.ctl_ui(ui, p, inst, cx);
                            }
                        }
                        if let Some(p) = find(layout, &format!("ps.{}", MOD_AMT + s)) {
                            self.knob_ui(ui, p, inst, cx);
                        }
                    });
                }
            });
        }
    }
}

/// a choice as a row of LEDs (blend operator, filter mode ...)
fn led_row(ui: &mut egui::Ui, p: &Param, inst: &mut Instrument, cx: &mut Cx) {
    let Ctl::Choice(opts) = &p.ctl else {
        return;
    };
    let cur = (p.get)(inst).round() as usize;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, o) in opts.iter().enumerate() {
            let r = crate::ember::led_item(ui, o, i == cur).on_hover_text(&p.tip);
            if r.clicked() && i != cur {
                (p.set)(inst, i as f32);
                cx.changed = true;
            }
        }
    });
}
