//! The simple instrument card (cleanup C): a header (prev / next, name, type,
//! Preset: … dropdown, A/B, Advanced ›), a MACROS row of 8 big knobs with a
//! caption saying what each drives, then Sound / Filter / Envelope sections
//! with one graph and the few knobs that change the sound right now
//! (Pulse width only on Pulse, F.Decay only with an Env amount, Glide only
//! with Legato, an oscillator at level 0 shows just its level). Advanced ›
//! opens the full editor (the cards and tabs of params.rs).
//!
//! Macros store nothing: a macro knob shows its first target's position and
//! turning it moves every target by the same amount (scaled), so songs don't
//! change and old songs play bit-identically. Built-in engines that already
//! have 8 macro parameters (Drum, Kit, Chip FX, FM-4, Grid) use those; the
//! others get a curated set per type.
use super::*;
use crate::fx::FxKind;

/// a visibility rule on the instrument's current values
type Cond = fn(&Layout, &Instrument) -> bool;

/// what a macro moves: an instrument parameter or an effect's parameter
#[derive(Clone, Copy, Debug)]
pub(crate) enum Tg {
    P(&'static str, f32),
    Fx(FxKind, &'static str, f32),
}

pub(crate) struct Mac {
    pub name: &'static str,
    pub caption: &'static str,
    pub targets: &'static [Tg],
}

pub(crate) enum Macros {
    None,
    Curated(&'static [Mac]),
    /// the engine's own macro parameters (keys)
    Params(&'static [&'static str]),
    /// a plugin's 8 assignable macros
    Plugin,
}

pub(crate) struct Sec {
    pub title: &'static str,
    /// card of params.rs whose picture this section shows
    pub viz: Option<&'static str>,
    /// choices / toggles in the section's header row
    pub head: &'static [&'static str],
    /// multi-value widgets (kit pieces, sample info, plugin status)
    pub customs: &'static [&'static str],
    pub knobs: &'static [(&'static str, Option<Cond>)],
    /// the section is switched off: only its header row shows
    pub off: Option<Cond>,
}

pub(crate) struct Spec {
    pub macros: Macros,
    pub secs: &'static [Sec],
}

fn val(l: &Layout, i: &Instrument, key: &str) -> f32 {
    l.find(key).map_or(0.0, |p| (p.get)(i))
}
fn txt(l: &Layout, i: &Instrument, key: &str) -> String {
    l.find(key).map_or(String::new(), |p| p.text((p.get)(i)))
}

const fn s(
    title: &'static str,
    viz: Option<&'static str>,
    head: &'static [&'static str],
    knobs: &'static [(&'static str, Option<Cond>)],
) -> Sec {
    Sec {
        title,
        viz,
        head,
        customs: &[],
        knobs,
        off: None,
    }
}

const CHORUS: Mac = Mac {
    name: "Chorus",
    caption: "Chorus mix",
    targets: &[Tg::Fx(FxKind::Chorus, "mix", 1.0)],
};
const SPACE: Mac = Mac {
    name: "Space",
    caption: "Reverb mix",
    targets: &[Tg::Fx(FxKind::Reverb, "mix", 1.0)],
};
const fn m(name: &'static str, caption: &'static str, targets: &'static [Tg]) -> Mac {
    Mac {
        name,
        caption,
        targets,
    }
}

const SYNTH_MACROS: &[Mac] = &[
    m(
        "Bright",
        "Cutoff + Res",
        &[Tg::P("cutoff", 1.0), Tg::P("resonance", 0.3)],
    ),
    m(
        "Width",
        "Detune + Stereo",
        &[Tg::P("detune_cents", 1.0), Tg::P("width", 1.0)],
    ),
    m("Attack", "Amp attack", &[Tg::P("amp.a", 1.0)]),
    m("Release", "Amp release", &[Tg::P("amp.r", 1.0)]),
    m("Motion", "LFO > Cutoff", &[Tg::P("lfo.cutoff", 1.0)]),
    CHORUS,
    m("Drive", "Filter drive", &[Tg::P("drive", 1.0)]),
    SPACE,
];
const VOICE_MACROS: &[Mac] = &[
    m(
        "Bright",
        "Cutoff + Res",
        &[Tg::P("cutoff", 1.0), Tg::P("resonance", 0.3)],
    ),
    m("Formant", "Formant shift", &[Tg::P("shift", 1.0)]),
    m("Breath", "Breath noise", &[Tg::P("breath", 1.0)]),
    m("Attack", "Amp attack", &[Tg::P("amp.a", 1.0)]),
    m("Release", "Amp release", &[Tg::P("amp.r", 1.0)]),
    m("Babble", "Vowel babble", &[Tg::P("babble", 1.0)]),
    CHORUS,
    SPACE,
];
const ANALOG_MACROS: &[Mac] = &[
    m(
        "Bright",
        "Cutoff + Res",
        &[Tg::P("an.0", 1.0), Tg::P("an.1", 0.3)],
    ),
    m(
        "Width",
        "Detune + Width",
        &[Tg::P("an.4", 1.0), Tg::P("an.24", 1.0)],
    ),
    m("Attack", "Amp attack", &[Tg::P("an.18", 1.0)]),
    m("Release", "Amp release", &[Tg::P("an.21", 1.0)]),
    m("Env", "Filter env amount", &[Tg::P("an.2", 1.0)]),
    CHORUS,
    m("Drive", "Filter drive", &[Tg::P("an.6", 1.0)]),
    SPACE,
];
const WAVE_MACROS: &[Mac] = &[
    m(
        "Bright",
        "Cutoff + Res",
        &[Tg::P("wv.2", 1.0), Tg::P("wv.3", 0.3)],
    ),
    m(
        "Width",
        "Detune + Spread",
        &[Tg::P("wv.5", 1.0), Tg::P("wv.6", 1.0)],
    ),
    m("Attack", "Amp attack", &[Tg::P("wv.21", 1.0)]),
    m("Release", "Amp release", &[Tg::P("wv.24", 1.0)]),
    m(
        "Morph",
        "Table position A + B",
        &[Tg::P("wv.0", 1.0), Tg::P("wv.1", 1.0)],
    ),
    m("Sub", "Sub oscillator", &[Tg::P("wv.14", 1.0)]),
    CHORUS,
    SPACE,
];

fn unison(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "unison") > 1.0
}
fn pulse(l: &Layout, i: &Instrument) -> bool {
    txt(l, i, "osc1") == "Pulse"
}
fn env_amt(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "env_amount").abs() > 1e-4
}
fn legato(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "legato") > 0.5
}
fn gen_legato(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "gen.legato") > 0.5
}
fn gen_filter_off(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "gen.filter_on") < 0.5
}

const PATCH_FILTER: Sec = s(
    "Filter",
    Some("Filter"),
    &["filter", "ladder_12db"],
    &[
        ("cutoff", None),
        ("resonance", None),
        ("env_amount", None),
        ("fenv.d", Some(env_amt)),
    ],
);
const PATCH_ENV: Sec = s(
    "Envelope",
    Some("Envelope"),
    &["legato"],
    &[
        ("amp.a", None),
        ("amp.d", None),
        ("amp.s", None),
        ("amp.r", None),
        ("glide", Some(legato)),
    ],
);
const GEN_FILTER: Sec = Sec {
    off: Some(gen_filter_off),
    ..s(
        "Filter",
        Some("Filter"),
        &["gen.filter_on", "filter"],
        &[("cutoff", None), ("resonance", None)],
    )
};

const SYNTH: Spec = Spec {
    macros: Macros::Curated(SYNTH_MACROS),
    secs: &[
        s(
            "Sound",
            Some("Oscillator"),
            &["osc1"],
            &[
                ("osc1_oct", None),
                ("pulse_width", Some(pulse)),
                ("unison", None),
                ("detune_cents", Some(unison)),
                ("width", Some(unison)),
            ],
        ),
        PATCH_FILTER,
        PATCH_ENV,
    ],
};
const VOICE: Spec = Spec {
    macros: Macros::Curated(VOICE_MACROS),
    secs: &[
        s(
            "Sound",
            None,
            &["throat", "register"],
            &[
                ("shift", None),
                ("breath", None),
                ("tension", None),
                ("osc1_oct", None),
            ],
        ),
        PATCH_FILTER,
        PATCH_ENV,
    ],
};
fn osc2_on(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "an.9") > 0.0
}
fn osc3_on(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "an.10") > 0.0
}
const ANALOG: Spec = Spec {
    macros: Macros::Curated(ANALOG_MACROS),
    secs: &[
        s(
            "Sound",
            Some("Oscillators"),
            &["an.wave0"],
            &[
                ("an.9", None),
                ("an.12", Some(osc2_on)),
                ("an.10", None),
                ("an.13", Some(osc3_on)),
                ("an.11", None),
            ],
        ),
        s(
            "Filter",
            Some("Filter"),
            &["an.mode", "an.pole4"],
            &[
                ("an.0", None),
                ("an.1", None),
                ("an.2", None),
                ("an.22", None),
            ],
        ),
        s(
            "Envelope",
            Some("Amp envelope"),
            &["gen.legato"],
            &[
                ("an.18", None),
                ("an.19", None),
                ("an.20", None),
                ("an.21", None),
                ("gen.glide", Some(gen_legato)),
            ],
        ),
    ],
};
fn oscb_on(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "wv.7") > 0.0
}
fn wv_filter_off(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "wv.filter_on") < 0.5
}
fn wv_unison(l: &Layout, i: &Instrument) -> bool {
    val(l, i, "wv.13") > 1.0
}
const WAVE: Spec = Spec {
    macros: Macros::Curated(WAVE_MACROS),
    secs: &[
        s(
            "Sound",
            Some("Osc A"),
            &["wv.table0"],
            &[
                ("wv.0", None),
                ("wv.7", None),
                ("wv.1", Some(oscb_on)),
                ("wv.11", Some(oscb_on)),
                ("wv.13", None),
                ("wv.5", Some(wv_unison)),
            ],
        ),
        Sec {
            off: Some(wv_filter_off),
            ..s(
                "Filter",
                Some("Filter"),
                &["wv.filter_on", "wv.mode"],
                &[("wv.2", None), ("wv.3", None), ("wv.4", None)],
            )
        },
        s(
            "Envelope",
            Some("Amp envelope"),
            &["gen.legato"],
            &[
                ("wv.21", None),
                ("wv.22", None),
                ("wv.23", None),
                ("wv.24", None),
                ("gen.glide", Some(gen_legato)),
            ],
        ),
    ],
};
const DRUM_MACROS: &[&str] = &[
    "drum.m0", "drum.m1", "drum.m2", "drum.m3", "drum.m4", "drum.m5", "drum.m6", "drum.m7",
];
const DRUM: Spec = Spec {
    macros: Macros::Params(DRUM_MACROS),
    secs: &[
        s(
            "Sound",
            None,
            &["drum.kind", "drum.model", "drum.follow_note"],
            &[("drum.level", None), ("gen.release", None)],
        ),
        GEN_FILTER,
    ],
};
const KIT: Spec = Spec {
    macros: Macros::Params(DRUM_MACROS),
    secs: &[
        Sec {
            customs: &["kit.piece"],
            ..s(
                "Sound",
                None,
                &["drum.kind", "drum.model"],
                &[("drum.level", None), ("gen.release", None)],
            )
        },
        GEN_FILTER,
    ],
};
const CHIPFX: Spec = Spec {
    macros: Macros::Params(&[
        "sfx.0", "sfx.2", "sfx.4", "sfx.10", "sfx.11", "sfx.14", "sfx.18", "sfx.20",
    ]),
    secs: &[
        Sec {
            customs: &["chipfx.category"],
            ..s(
                "Sound",
                None,
                &["chipfx.wave", "chipfx.follow_note"],
                &[("chipfx.level", None), ("gen.release", None)],
            )
        },
        GEN_FILTER,
    ],
};
const FM: Spec = Spec {
    macros: Macros::Params(&[
        "fm.0", "fm.1", "fm.2", "fm.3", "fm.4", "fm.5", "fm.6", "fm.7",
    ]),
    secs: &[s(
        "Sound",
        Some("Algorithm"),
        &["fm.algo"],
        &[("fm.level", None)],
    )],
};
const NOISE: Spec = Spec {
    macros: Macros::None,
    secs: &[
        s(
            "Sound",
            None,
            &["nz.color"],
            &[
                ("nz.8", None),
                ("nz.14", None),
                ("nz.6", None),
                ("nz.7", None),
            ],
        ),
        s(
            "Filter",
            Some("Filter"),
            &["nz.mode", "nz.ladder"],
            &[("nz.1", None), ("nz.2", None), ("nz.3", None)],
        ),
        s(
            "Envelope",
            None,
            &["nz.hold"],
            &[
                ("nz.0", None),
                ("nz.4", None),
                ("nz.5", None),
                ("nz.11", None),
            ],
        ),
    ],
};
const GRID: Spec = Spec {
    macros: Macros::Params(&[
        "gd.0", "gd.1", "gd.2", "gd.3", "gd.4", "gd.5", "gd.6", "gd.7",
    ]),
    secs: &[],
};
const SAMPLE: Spec = Spec {
    macros: Macros::None,
    secs: &[Sec {
        customs: &["sample.info"],
        ..s(
            "Sound",
            Some("Sample"),
            &[],
            &[("volume", None), ("transpose", None)],
        )
    }],
};
const PLUGIN: Spec = Spec {
    macros: Macros::Plugin,
    secs: &[Sec {
        customs: &["plugin.status"],
        ..s("Sound", None, &[], &[("volume", None), ("transpose", None)])
    }],
};

/// the simple card of an instrument type (its `type_tag`)
pub(crate) fn spec(tag: &str) -> &'static Spec {
    match tag {
        "Synth" | "Moog" => &SYNTH,
        "Voice" => &VOICE,
        "Analog" => &ANALOG,
        "Wave" => &WAVE,
        "Drum" => &DRUM,
        "Kit" => &KIT,
        "Chip FX" => &CHIPFX,
        "FM-4" => &FM,
        "Noise" => &NOISE,
        "Grid" => &GRID,
        "Sample" => &SAMPLE,
        _ => &PLUGIN,
    }
}

/// a section's knobs that change the sound now: enabled, and its rule holds
pub(crate) fn shown_knobs<'a>(l: &'a Layout, i: &Instrument, sec: &Sec) -> Vec<&'a Param> {
    sec.knobs
        .iter()
        .filter(|(_, c)| c.is_none_or(|f| f(l, i)))
        .filter_map(|(k, _)| l.find(k))
        .filter(|p| p.is_on(i) && matches!(p.ctl, Ctl::Knob { .. }))
        .collect()
}

/// one macro as drawn: name, caption, targets
pub(crate) struct MacroRt {
    pub name: String,
    pub caption: String,
    pub targets: Vec<Tg>,
}

pub(crate) fn macros(l: &Layout, sp: &Spec) -> Vec<MacroRt> {
    match &sp.macros {
        Macros::Curated(ms) => ms
            .iter()
            .map(|m| MacroRt {
                name: m.name.into(),
                caption: m.caption.into(),
                targets: m.targets.to_vec(),
            })
            .collect(),
        Macros::Params(keys) => keys
            .iter()
            .filter_map(|k| l.find(k))
            .map(|p| MacroRt {
                name: p.name.clone(),
                caption: short_tip(&p.tip),
                targets: vec![Tg::P(leak_key(&p.key), 1.0)],
            })
            .collect(),
        _ => vec![],
    }
}

/// the engine macro keys are &'static in the spec already; find it there
fn leak_key(k: &str) -> &'static str {
    for sp in [&DRUM, &CHIPFX, &FM, &GRID] {
        if let Macros::Params(ks) = &sp.macros {
            if let Some(s) = ks.iter().find(|x| **x == k) {
                return s;
            }
        }
    }
    ""
}

/// a tooltip's first words as a caption ("what it drives")
fn short_tip(t: &str) -> String {
    let first = t.split([';', '.', '(', ',']).next().unwrap_or("").trim();
    if first.chars().count() <= 22 {
        first.to_string()
    } else {
        let w: Vec<&str> = first.split_whitespace().collect();
        let mut out = String::new();
        for x in w {
            if out.chars().count() + x.chars().count() + 1 > 21 {
                break;
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(x);
        }
        out + "…"
    }
}

/// a macro target's knob position, if it exists on this instrument
pub(crate) fn tg_norm(l: &Layout, i: &Instrument, t: Tg) -> Option<f32> {
    match t {
        Tg::P(k, _) => {
            let p = l.find(k)?;
            if !p.is_on(i) {
                return None;
            }
            let Ctl::Knob { lo, hi, log, .. } = p.ctl else {
                return None;
            };
            Some(params::to_norm((p.get)(i), lo, hi, log))
        }
        Tg::Fx(kind, key, _) => {
            let k = crate::fx::Device::param_index(kind, key)?;
            i.fx.iter().find(|d| d.kind == kind).map(|d| d.params[k])
        }
    }
}

fn tg_set(l: &Layout, i: &mut Instrument, t: Tg, n: f32) {
    let n = n.clamp(0.0, 1.0);
    match t {
        Tg::P(k, _) => {
            let Some(p) = l.find(k) else { return };
            let Ctl::Knob {
                lo, hi, log, step, ..
            } = p.ctl
            else {
                return;
            };
            let mut v = params::from_norm(n, lo, hi, log);
            if step > 0.0 {
                v = ((v - lo) / step).round() * step + lo;
            }
            (p.set)(i, v);
        }
        Tg::Fx(kind, key, _) => {
            let Some(k) = crate::fx::Device::param_index(kind, key) else {
                return;
            };
            if let Some(d) = i.fx.iter_mut().find(|d| d.kind == kind) {
                d.params[k] = n;
            }
        }
    }
}

fn tg_scale(t: Tg) -> f32 {
    match t {
        Tg::P(_, s) | Tg::Fx(_, _, s) => s,
    }
}

/// turn macro `m` so its first available target sits at `n`: every target
/// moves by the same change (times its scale)
pub(crate) fn macro_turn(l: &Layout, i: &mut Instrument, m: &MacroRt, n: f32) {
    let Some((t0, n0)) = m
        .targets
        .iter()
        .find_map(|t| tg_norm(l, i, *t).map(|v| (*t, v)))
    else {
        return;
    };
    let d = (n - n0) / tg_scale(t0).max(1e-6);
    for t in &m.targets {
        if let Some(v) = tg_norm(l, i, *t) {
            tg_set(l, i, *t, v + d * tg_scale(*t));
        }
    }
}

/// the macro's text: its first target's value
fn macro_text(l: &Layout, i: &Instrument, m: &MacroRt) -> Option<(f32, String, f32)> {
    for t in &m.targets {
        let Some(n) = tg_norm(l, i, *t) else { continue };
        return Some(match *t {
            Tg::P(k, _) => {
                let p = l.find(k)?;
                let def = match p.ctl {
                    Ctl::Knob {
                        lo, hi, log, def, ..
                    } => params::to_norm(def, lo, hi, log),
                    _ => 0.0,
                };
                (n, p.text((p.get)(i)), def)
            }
            Tg::Fx(kind, key, _) => {
                let k = crate::fx::Device::param_index(kind, key)?;
                let pi = &crate::fx::params(kind)[k];
                (n, pi.format(n), pi.to_norm(pi.def))
            }
        });
    }
    None
}

/// the effect a macro would add when the instrument has none of its targets
fn macro_missing_fx(m: &MacroRt) -> Option<FxKind> {
    m.targets.iter().find_map(|t| match t {
        Tg::Fx(k, _, _) => Some(*k),
        _ => None,
    })
}

const MACRO_H: f32 = 100.0;
const SEC_HEAD: f32 = 26.0;
const GAP: f32 = 8.0;

/// a big macro knob: name over it, value and caption under it
#[allow(clippy::too_many_arguments)]
fn big_knob(
    ui: &mut egui::Ui,
    id: egui::Id,
    v: &mut f32,
    name: &str,
    value: &str,
    caption: &str,
    def: f32,
    enabled: bool,
) -> (egui::Response, bool) {
    let t = th();
    let (cell, _) = ui.allocate_exact_size(egui::vec2(96.0, MACRO_H - 8.0), Sense::hover());
    let r = if t.ember { 25.0 } else { 21.0 };
    let c = Pos2::new(cell.center().x, cell.top() + 18.0 + r);
    let krect = Rect::from_center_size(c, egui::vec2(r * 2.0 + 8.0, r * 2.0 + 8.0));
    let resp = ui.interact(
        krect,
        id,
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::click()
        },
    );
    let label = format!("{name} macro");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, enabled, &label));
    let mut changed = false;
    if enabled {
        if resp.double_clicked() {
            *v = def;
            changed = true;
        } else if resp.dragged() {
            let fine = if ui.input(|i| i.modifiers.ctrl || i.modifiers.shift) {
                0.1
            } else {
                1.0
            };
            let d = resp.drag_delta();
            let nid = id.with("pos");
            let n0 = if resp.drag_started() {
                *v
            } else {
                ui.data(|m| m.get_temp::<f32>(nid)).unwrap_or(*v)
            };
            let n = (n0 + (d.x - d.y) * 0.004 * fine).clamp(0.0, 1.0);
            ui.data_mut(|m| m.insert_temp(nid, n));
            if (n - *v).abs() > 1e-6 {
                *v = n;
                changed = true;
            }
        }
    }
    let p = ui.painter();
    let dim = if enabled { 1.0 } else { 0.4 };
    if t.ember {
        // pic-1 knob: LED arc, silver ring, glossy well with the readout
        crate::ember::knob_label(p, Pos2::new(cell.center().x, cell.top() + 6.0), name, 12.5);
        let bip = (def - 0.5).abs() < 0.01;
        let mut kp = p.clone();
        kp.multiply_opacity(if enabled { 1.0 } else { 0.55 });
        crate::ember::knob_big(
            &kp,
            c,
            r + 8.0,
            *v,
            bip,
            &compact(value),
            enabled && (*v - if bip { 0.5 } else { 0.0 }).abs() > 0.002,
        );
        if resp.hovered() && enabled {
            p.circle_stroke(c, r + 9.0, Stroke::new(1.0, t.ink2.gamma_multiply(0.4)));
        }
        let cap =
            super::super::pattern::fit(p, caption, &egui::FontId::proportional(10.0), cell.width());
        p.text(
            Pos2::new(cell.center().x, c.y + r + 17.0),
            egui::Align2::CENTER_CENTER,
            cap,
            egui::FontId::proportional(10.0),
            crate::ember::INK3,
        );
        return (resp, changed);
    }
    p.text(
        Pos2::new(cell.center().x, cell.top() + 6.0),
        egui::Align2::CENTER_CENTER,
        name.to_uppercase(),
        egui::FontId::proportional(11.0),
        t.ink.gamma_multiply(dim),
    );
    // track and value arc over 270°
    let a0 = std::f32::consts::PI * 0.75;
    let span = std::f32::consts::PI * 1.5;
    let arc = |from: f32, to: f32, rad: f32| -> Vec<Pos2> {
        let n = ((to - from).abs() * 18.0).ceil().max(2.0) as usize;
        (0..=n)
            .map(|k| {
                let a = a0 + span * (from + (to - from) * k as f32 / n as f32);
                c + egui::vec2(a.cos(), a.sin()) * rad
            })
            .collect()
    };
    p.circle_filled(c, r + 1.0, t.cart_bot);
    p.add(egui::Shape::line(
        arc(0.0, 1.0, r + 5.0),
        Stroke::new(3.0, t.ink2.gamma_multiply(0.22)),
    ));
    // bipolar (default in the middle): light from 12 o'clock
    let from = if (def - 0.5).abs() < 0.01 { 0.5 } else { 0.0 };
    if enabled && (*v - from).abs() > 0.002 {
        p.add(egui::Shape::line(
            arc(from.min(*v), from.max(*v), r + 5.0),
            Stroke::new(3.0, t.accent),
        ));
    }
    p.circle_stroke(
        c,
        r,
        Stroke::new(
            1.2,
            if resp.hovered() && enabled {
                t.accent
            } else {
                t.ink2.gamma_multiply(0.6 * dim)
            },
        ),
    );
    let a = a0 + span * *v;
    p.line_segment(
        [
            c + egui::vec2(a.cos(), a.sin()) * (r * 0.35),
            c + egui::vec2(a.cos(), a.sin()) * (r - 3.0),
        ],
        Stroke::new(2.4, t.ink.gamma_multiply(dim)),
    );
    p.text(
        Pos2::new(cell.center().x, c.y + r + 13.0),
        egui::Align2::CENTER_CENTER,
        value,
        egui::FontId::monospace(10.5),
        if enabled {
            t.value
        } else {
            t.ink2.gamma_multiply(0.6)
        },
    );
    let cap =
        super::super::pattern::fit(p, caption, &egui::FontId::proportional(10.0), cell.width());
    p.text(
        Pos2::new(cell.center().x, c.y + r + 27.0),
        egui::Align2::CENTER_CENTER,
        cap,
        egui::FontId::proportional(10.0),
        t.ink2.gamma_multiply(0.85 * dim),
    );
    (resp, changed)
}

/// the short readout inside a pic-1 knob: "3.20 kHz" -> "3.2k",
/// "250 ms" -> "250", "+0.0 dB" -> "+0.0"
pub(crate) fn compact(v: &str) -> String {
    let mut it = v.split_whitespace();
    let Some(num) = it.next() else {
        return String::new();
    };
    let unit = it.next().unwrap_or("");
    let k = if unit.starts_with('k') && !num.ends_with('k') {
        "k"
    } else {
        ""
    };
    let mut n = num.to_string();
    // too long: fewer decimals
    while n.chars().count() + k.len() > 5 && n.contains('.') {
        n.pop();
        if n.ends_with('.') {
            n.pop();
            break;
        }
    }
    if n.chars().count() > 5 {
        n = n.chars().take(5).collect();
    }
    n + k
}

/// a section frame: rounded panel and an uppercase title; returns the rect
fn frame(ui: &egui::Ui, r: Rect, title: &str) {
    let t = th();
    let p = ui.painter();
    if t.ember {
        crate::ember::card(p, r);
        crate::ember::title(
            p,
            Pos2::new(r.left() + 16.0, r.top() + SEC_HEAD * 0.5),
            title,
        );
        return;
    }
    p.rect_filled(r, 6.0, t.cart_bot);
    p.rect_stroke(
        r,
        6.0,
        Stroke::new(1.0, t.ink2.gamma_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
    p.text(
        Pos2::new(r.left() + 10.0, r.top() + SEC_HEAD * 0.5),
        egui::Align2::LEFT_CENTER,
        title.to_uppercase(),
        egui::FontId::proportional(11.5),
        t.ink,
    );
}

/// draw `f` in a child ui at `r` without moving the parent's cursor
fn inside<R>(
    ui: &mut egui::Ui,
    r: Rect,
    lay: egui::Layout,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(lay));
    c.set_clip_rect(r.intersect(ui.clip_rect()));
    f(&mut c)
}

/// "punchy-psy-kick" -> "Punchy psy kick"
fn pretty(s: &str) -> String {
    let s = s.replace(['-', '_'], " ");
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// the presets of this instrument's type: (group, name, tip, sound)
pub(crate) fn preset_list(inst: &Instrument) -> Vec<(String, String, String, InstrumentKind)> {
    match &inst.kind {
        InstrumentKind::Synth(p) => crate::presets::FACTORY
            .iter()
            .filter_map(|(name, _)| {
                let np = crate::presets::factory(name)?;
                let mut q = p.clone();
                q.copy_sound_from(&np);
                let (g, short) = name.split_once('/').unwrap_or(("", name));
                let (g, short) = (pretty(g), pretty(short));
                Some((g, short, String::new(), InstrumentKind::Synth(q)))
            })
            .collect(),
        InstrumentKind::Voice(v) => crate::voice::presets()
            .into_iter()
            .map(|(name, tip, mut np)| {
                np.base.name = v.base.name.clone();
                (
                    String::new(),
                    name.to_string(),
                    tip.to_string(),
                    InstrumentKind::Voice(np),
                )
            })
            .collect(),
        InstrumentKind::Gen(g) => {
            let d = std::mem::discriminant(&g.engine);
            crate::gen::presets()
                .into_iter()
                .filter(|(_, _, np)| std::mem::discriminant(&np.engine) == d)
                .map(|(name, tip, mut np)| {
                    np.base.name = g.base.name.clone();
                    (
                        String::new(),
                        name.to_string(),
                        tip.to_string(),
                        InstrumentKind::Gen(np),
                    )
                })
                .collect()
        }
        _ => vec![],
    }
}

impl App {
    /// the preset this instrument's sound is (cached per sound), or None
    fn current_preset(&mut self, idx: usize, inst: &Instrument) -> Option<String> {
        if let Some((i, k, name)) = &self.ip.preset_seen {
            if *i == idx && *k == inst.kind {
                return name.clone();
            }
        }
        let name = if matches!(
            inst.kind,
            InstrumentKind::Synth(_) | InstrumentKind::Voice(_) | InstrumentKind::Gen(_)
        ) {
            preset_list(inst)
                .into_iter()
                .find(|(_, _, _, k)| *k == inst.kind)
                .map(|(_, n, _, _)| n)
        } else {
            None
        };
        self.ip.preset_seen = Some((idx, inst.kind.clone(), name.clone()));
        name
    }

    /// the simple card (see the module doc); edits go through `cx` like the
    /// full editor's
    pub(super) fn simple_card(
        &mut self,
        ui: &mut egui::Ui,
        l: &Layout,
        inst: &mut Instrument,
        cx: &mut Cx,
        n: usize,
    ) {
        let t = th();
        cx.no_locks = true;
        let idx = cx.idx;
        let tag = cx.tag;
        let sp = spec(tag);
        // ---- header ----
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if ui.small_button("<").on_hover_text("previous instrument").clicked() {
                cx.acts
                    .push(Act::Select(if idx == 0 { n as u8 } else { idx as u8 }));
            }
            if ui.small_button(">").on_hover_text("next instrument").clicked() {
                cx.acts.push(Act::Select(((idx + 1) % n) as u8 + 1));
            }
            let head = egui::RichText::new(format!("{:02X} {}", idx + 1, inst.name));
            ui.label(if t.ember {
                head.font(crate::theme::label_font(17.0)).color(t.ink)
            } else {
                head.strong().size(15.0)
            });
            tag_pill(ui, tag);
            ui.add_space(6.0);
            self.preset_dropdown(ui, inst, cx);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                let adv = if t.ember {
                    crate::ember::button(ui, "Advanced ›", false)
                } else {
                    ui.button("Advanced ›")
                };
                if adv
                    .on_hover_text("every control of this instrument: the full editor with its tabs")
                    .clicked()
                {
                    self.ip.advanced = true;
                }
                if !matches!(inst.kind, InstrumentKind::Plugin(_)) {
                    let on_b = self.ip.ab.as_ref().is_some_and(|a| a.on_b);
                    for (b, name) in [(true, "B"), (false, "A")] {
                        let r = if t.ember {
                            crate::ember::button(ui, name, on_b == b)
                        } else {
                            ui.selectable_label(on_b == b, egui::RichText::new(name).strong())
                        };
                        let r = r.on_hover_text(
                                "A/B compare: B starts as a copy of A; edit one, switch to hear the other (Ctrl+Z undoes a switch)",
                            );
                        if r.clicked() && on_b != b {
                            cx.acts.push(Act::AbToggle);
                        }
                    }
                }
            });
        });
        ui.add_space(4.0);
        let area = ui.available_rect_before_wrap();
        let area = Rect::from_min_max(
            area.min + egui::vec2(4.0, 0.0),
            area.max - egui::vec2(4.0, 4.0),
        );
        ui.allocate_rect(area, Sense::hover());
        let ms = macros(l, sp);
        let plugin_macros = matches!(sp.macros, Macros::Plugin);
        let has_macros = !ms.is_empty() || plugin_macros;
        let mut y = area.top();
        // ---- macros ----
        if has_macros {
            let mh = if plugin_macros {
                MACRO_H + 10.0
            } else {
                MACRO_H
            };
            let r = Rect::from_min_size(Pos2::new(area.left(), y), egui::vec2(area.width(), mh));
            frame(ui, r, "Macros");
            let body = Rect::from_min_max(
                Pos2::new(r.left() + 70.0, r.top() + 4.0),
                r.max - egui::vec2(8.0, 2.0),
            );
            if plugin_macros {
                if let InstrumentKind::Plugin(pp) = &inst.kind {
                    let pp = pp.clone();
                    inside(
                        ui,
                        body,
                        egui::Layout::left_to_right(egui::Align::Min),
                        |ui| self.plugin_macro_knobs(ui, idx, &pp),
                    );
                }
            } else {
                let w = body.width() / ms.len() as f32;
                for (k, m) in ms.iter().enumerate() {
                    let cr = Rect::from_min_size(
                        Pos2::new(body.left() + w * k as f32 + (w - 96.0) * 0.5, body.top()),
                        egui::vec2(96.0, body.height()),
                    );
                    let cur = macro_text(l, inst, m);
                    let (mut v, text, def) = cur.clone().unwrap_or((0.0, "–".into(), 0.0));
                    let (resp, changed) = inside(
                        ui,
                        cr,
                        egui::Layout::left_to_right(egui::Align::Min),
                        |ui| {
                            big_knob(
                                ui,
                                egui::Id::unique(("macro", idx, k)),
                                &mut v,
                                &m.name,
                                &text,
                                &m.caption,
                                def,
                                cur.is_some(),
                            )
                        },
                    );
                    let missing = if cur.is_none() {
                        macro_missing_fx(m)
                    } else {
                        None
                    };
                    let resp = match missing {
                        Some(fk) => resp.on_hover_text(format!(
                            "{}: drives the {}'s mix; this instrument has none. Click to add one.",
                            m.name,
                            fk.name()
                        )),
                        None => resp.on_hover_text(format!(
                            "{} drives {} · drag · Ctrl-drag fine · double-click default",
                            m.name,
                            m.caption.to_lowercase()
                        )),
                    };
                    if let Some(fk) = missing {
                        if resp.clicked() {
                            cx.acts.push(Act::AddFx(fk));
                        }
                    }
                    if changed {
                        macro_turn(l, inst, m, v);
                        cx.changed = true;
                    }
                }
            }
            y = r.bottom() + GAP;
        }
        // ---- sections ----
        if sp.secs.is_empty() {
            let r = Rect::from_min_max(Pos2::new(area.left(), y), area.max);
            if r.height() > 40.0 {
                frame(ui, r, "Patch");
                let b = Rect::from_center_size(r.center(), egui::vec2(160.0, 26.0));
                if ui
                    .put(b, egui::Button::new("Edit the patch ›"))
                    .on_hover_text("this sound is a Grid patch: open it in the full editor")
                    .clicked()
                {
                    self.ip.advanced = true;
                    self.ip.tab = Tab::Grid;
                }
            }
            return;
        }
        let offs: Vec<bool> = sp
            .secs
            .iter()
            .map(|s| s.off.is_some_and(|f| f(l, inst)))
            .collect();
        // no graph to show: the sections are as tall as their controls
        let graphs = sp.secs.iter().enumerate().any(|(k, s)| {
            !offs[k]
                && s.viz
                    .is_some_and(|n| l.cards.iter().any(|c| c.name == n && c.viz.is_some()))
        });
        let mut row = Rect::from_min_max(Pos2::new(area.left(), y), area.max);
        if !graphs {
            let ch = sp
                .secs
                .iter()
                .map(|s| {
                    s.customs
                        .iter()
                        .filter_map(|c| l.find(c).and_then(|p| p.custom()))
                        .map(|c| custom_size(c, row.height()).1)
                        .sum::<f32>()
                })
                .fold(0.0, f32::max);
            let h = SEC_HEAD + 8.0 + ch + 8.0 + CELL.y + 8.0;
            row.set_height(row.height().min(h));
        }
        let shown: Vec<Vec<&Param>> = sp.secs.iter().map(|s| shown_knobs(l, inst, s)).collect();
        // widths: by knob count (at least 3 knobs wide), the rest shared out
        let want: Vec<f32> = sp
            .secs
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let nk = if offs[k] { 2 } else { shown[k].len().max(3) };
                let cw = s
                    .customs
                    .iter()
                    .filter_map(|c| l.find(c).and_then(|p| p.custom()))
                    .map(|c| custom_size(c, row.height()).0)
                    .fold(0.0, f32::max);
                (nk as f32 * CELL.x + 24.0).max(cw + 20.0).max(220.0)
            })
            .collect();
        let total: f32 = want.iter().sum::<f32>() + GAP * (want.len() - 1) as f32;
        let extra = ((row.width() - total) / want.len() as f32).max(0.0);
        let mut x = row.left();
        for (k, sec) in sp.secs.iter().enumerate() {
            let w = want[k] + extra;
            let r = Rect::from_min_size(Pos2::new(x, row.top()), egui::vec2(w, row.height()));
            x += w + GAP;
            frame(ui, r, sec.title);
            // header controls on the right of the title
            let head = Rect::from_min_max(
                Pos2::new(r.left() + 90.0, r.top() + 2.0),
                Pos2::new(r.right() - 6.0, r.top() + SEC_HEAD),
            );
            let heads: Vec<&Param> = sec
                .head
                .iter()
                .filter_map(|k| l.find(k))
                .filter(|p| p.is_on(inst))
                .collect();
            inside(
                ui,
                head,
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for p in heads.iter().rev() {
                        self.head_ctl(ui, p, inst, cx);
                    }
                },
            );
            let body = Rect::from_min_max(
                Pos2::new(r.left() + 8.0, r.top() + SEC_HEAD + 2.0),
                r.max - egui::vec2(8.0, 6.0),
            );
            if offs[k] {
                ui.painter().text(
                    body.left_top() + egui::vec2(2.0, 8.0),
                    egui::Align2::LEFT_TOP,
                    format!("{} off", sec.title),
                    egui::FontId::proportional(11.5),
                    t.ink2,
                );
                continue;
            }
            let customs: Vec<Custom> = sec
                .customs
                .iter()
                .filter_map(|c| l.find(c).and_then(|p| p.custom()))
                .collect();
            let ch: f32 = customs
                .iter()
                .map(|c| custom_size(*c, body.height()).1)
                .sum();
            let knobs = &shown[k];
            let kh = if knobs.is_empty() { 0.0 } else { CELL.y };
            let viz = sec
                .viz
                .and_then(|n| l.cards.iter().find(|c| c.name == n && c.tab == Tab::Main))
                .and_then(|c| c.viz.as_ref());
            let gh = (body.height() - kh - ch - 8.0).clamp(0.0, 220.0);
            let mut yy = body.top();
            if let Some(v) = viz {
                if gh >= 28.0 {
                    let g = Rect::from_min_size(body.min, egui::vec2(body.width(), gh));
                    let p = ui.painter_at(g);
                    super::super::devcards::well(&p, g);
                    knob::draw_viz(&p, g.shrink(3.0), &v(inst));
                    yy += gh + 6.0;
                }
            }
            if !customs.is_empty() {
                let cr = Rect::from_min_max(
                    Pos2::new(body.left(), yy),
                    Pos2::new(body.right(), yy + ch),
                );
                inside(ui, cr, egui::Layout::top_down(egui::Align::Min), |ui| {
                    for c in &customs {
                        self.custom_ui(ui, *c, inst, cx, body.height());
                    }
                });
            }
            if !knobs.is_empty() {
                let kr =
                    Rect::from_min_max(Pos2::new(body.left(), body.bottom() - CELL.y), body.max);
                let pad = ((kr.width() - knobs.len() as f32 * CELL.x) / 2.0).max(0.0);
                inside(
                    ui,
                    kr,
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_space(pad);
                        for p in knobs {
                            self.knob_ui(ui, p, inst, cx);
                        }
                    },
                );
            }
        }
    }

    /// a choice or toggle in a section header: a compact dropdown / checkbox
    fn head_ctl(&mut self, ui: &mut egui::Ui, p: &Param, inst: &mut Instrument, cx: &mut Cx) {
        let t = th();
        let v = (p.get)(inst);
        match &p.ctl {
            Ctl::Choice(opts) => {
                let cur = v.round().max(0.0) as usize;
                let r = egui::ComboBox::from_id_salt(("sc_head", cx.idx, &p.key))
                    .selected_text(p.text(v))
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for (k, o) in opts.iter().enumerate() {
                            if o == "other" {
                                continue;
                            }
                            if ui.selectable_label(cur == k, o).clicked() && cur != k {
                                (p.set)(inst, k as f32);
                                cx.changed = true;
                            }
                        }
                    })
                    .response;
                let r = r.on_hover_text(if p.tip.is_empty() { &p.name } else { &p.tip });
                self.param_menu(&r, p, inst, cx, None);
                ui.label(egui::RichText::new(&p.name).small().color(t.ink2));
            }
            Ctl::Toggle => {
                let mut b = v > 0.5;
                let r = ui.checkbox(&mut b, egui::RichText::new(&p.name).small());
                let r = if p.tip.is_empty() {
                    r
                } else {
                    r.on_hover_text(&p.tip)
                };
                if r.changed() {
                    (p.set)(inst, if b { 1.0 } else { 0.0 });
                    cx.changed = true;
                }
                self.param_menu(&r, p, inst, cx, None);
            }
            _ => {}
        }
    }

    /// "Preset: name ▾": the presets of this type, then Save / Library
    fn preset_dropdown(&mut self, ui: &mut egui::Ui, inst: &Instrument, cx: &mut Cx) {
        let cur = self.current_preset(cx.idx, inst);
        let label = format!("Preset: {}", cur.as_deref().unwrap_or("custom"));
        let list = preset_list(inst);
        let r = ui.menu_button(label, |ui| {
            egui::ScrollArea::vertical()
                .max_height(340.0)
                .show(ui, |ui| {
                    let mut groups: Vec<&str> = vec![];
                    for (g, ..) in &list {
                        if !groups.contains(&g.as_str()) {
                            groups.push(g);
                        }
                    }
                    for g in groups {
                        if !g.is_empty() {
                            ui.label(egui::RichText::new(g).small().strong());
                        }
                        for (pg, name, tip, k) in &list {
                            if pg != g {
                                continue;
                            }
                            let on = cur.as_deref() == Some(name.as_str());
                            let b = ui.selectable_label(on, name);
                            let b = if tip.is_empty() {
                                b
                            } else {
                                b.on_hover_text(tip)
                            };
                            if b.clicked() {
                                cx.acts.push(Act::Kind(Box::new(k.clone())));
                                ui.close();
                            }
                        }
                    }
                });
            if !list.is_empty() {
                ui.separator();
            }
            if ui.button("Save to library…").clicked() {
                cx.acts.push(Act::SaveLib);
                ui.close();
            }
            if ui
                .button("Library…")
                .on_hover_text("replace the sound from the instrument library")
                .clicked()
            {
                cx.acts.push(Act::Library);
                ui.close();
            }
        });
        r.response.on_hover_text(
            "load a preset of this type (keeps the name and effects), save or open the library",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::params::{build, tests::all_types};

    /// every key the simple card names exists for its type, every section
    /// shows 1-6 knobs, macros are 8 (or none)
    #[test]
    fn every_type_has_a_simple_card_built_from_real_parameters() {
        for (tag, i) in all_types() {
            let l = build(&i, 0, 0);
            let sp = spec(tag);
            for sec in sp.secs {
                for k in sec
                    .head
                    .iter()
                    .chain(sec.customs)
                    .chain(sec.knobs.iter().map(|(k, _)| k))
                {
                    assert!(l.find(k).is_some(), "{tag}: {} has no {k}", sec.title);
                }
                if let Some(v) = sec.viz {
                    assert!(
                        l.cards.iter().any(|c| c.name == v && c.viz.is_some()),
                        "{tag}: no picture card {v}"
                    );
                }
                let n = shown_knobs(&l, &i, sec).len();
                assert!(n <= 6, "{tag}: {} shows {n}", sec.title);
            }
            let ms = macros(&l, sp);
            assert!(ms.is_empty() || ms.len() == 8, "{tag}: {} macros", ms.len());
            for m in &ms {
                for t in &m.targets {
                    if let Tg::P(k, _) = t {
                        assert!(l.find(k).is_some(), "{tag}: macro {} has no {k}", m.name);
                    }
                }
            }
        }
    }

    fn synth() -> Instrument {
        all_types()
            .into_iter()
            .find(|(t, _)| *t == "Synth")
            .unwrap()
            .1
    }

    fn keys(l: &Layout, i: &Instrument, title: &str) -> Vec<String> {
        let sec = SYNTH.secs.iter().find(|s| s.title == title).unwrap();
        shown_knobs(l, i, sec)
            .iter()
            .map(|p| p.key.clone())
            .collect()
    }

    #[test]
    fn only_controls_that_change_the_sound_show() {
        let mut i = synth();
        let l = build(&i, 0, 0);
        // Saw, one voice: no Pulse width, Detune or Stereo
        assert_eq!(keys(&l, &i, "Sound"), ["osc1_oct", "unison"]);
        // Env amt 0: no F.Decay; Legato off: no Glide
        assert_eq!(
            keys(&l, &i, "Filter"),
            ["cutoff", "resonance", "env_amount"]
        );
        assert!(!keys(&l, &i, "Envelope").contains(&"glide".to_string()));
        (l.find("osc1").unwrap().set)(&mut i, 2.0); // Pulse
        (l.find("unison").unwrap().set)(&mut i, 3.0);
        (l.find("env_amount").unwrap().set)(&mut i, 1.5);
        (l.find("legato").unwrap().set)(&mut i, 1.0);
        let l = build(&i, 0, 0);
        assert_eq!(
            keys(&l, &i, "Sound"),
            ["osc1_oct", "pulse_width", "unison", "detune_cents", "width"]
        );
        assert!(keys(&l, &i, "Filter").contains(&"fenv.d".to_string()));
        assert!(keys(&l, &i, "Envelope").contains(&"glide".to_string()));
    }

    #[test]
    fn a_macro_moves_all_its_targets_by_the_same_amount() {
        let mut i = synth();
        let l = build(&i, 0, 0);
        let ms = macros(&l, &SYNTH);
        let bright = ms.iter().find(|m| m.name == "Bright").unwrap();
        let c0 = tg_norm(&l, &i, Tg::P("cutoff", 1.0)).unwrap();
        let r0 = tg_norm(&l, &i, Tg::P("resonance", 1.0)).unwrap();
        macro_turn(&l, &mut i, bright, c0 - 0.2);
        let c1 = tg_norm(&l, &i, Tg::P("cutoff", 1.0)).unwrap();
        let r1 = tg_norm(&l, &i, Tg::P("resonance", 1.0)).unwrap();
        assert!((c1 - (c0 - 0.2)).abs() < 1e-3, "{c0} {c1}");
        assert!((r1 - (r0 - 0.06)).abs() < 1e-3, "{r0} {r1}");
        // Space drives the reverb's mix; without a reverb it does nothing
        let space = ms.iter().find(|m| m.name == "Space").unwrap();
        let before = i.clone();
        macro_turn(&l, &mut i, space, 0.9);
        assert_eq!(i, before);
        i.fx.push(crate::fx::Device::new(FxKind::Reverb));
        macro_turn(&l, &mut i, space, 0.9);
        let k = crate::fx::Device::param_index(FxKind::Reverb, "mix").unwrap();
        assert!((i.fx.last().unwrap().params[k] - 0.9).abs() < 1e-6);
    }

    #[test]
    fn the_preset_dropdown_names_a_loaded_factory_sound() {
        let i = synth();
        let list = preset_list(&i);
        assert!(list.len() >= 4);
        assert!(list.iter().all(|x| !x.1.contains('-')));
        let (_, name, _, k) = list[3].clone();
        let mut j = i.clone();
        j.kind = k;
        assert_eq!(
            preset_list(&j)
                .into_iter()
                .find(|x| x.3 == j.kind)
                .map(|x| x.1),
            Some(name)
        );
    }
}
