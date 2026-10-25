//! Drag-to-modulate UI (see crate::modulation and docs/MODULATION.md): the
//! modulator chips you drag onto a knob or FX slider, the Modulators and
//! Routings cards of the instrument editor's Modulation tab, the amount
//! rings' colours and the FX-slider depth bars.
use super::App;
use crate::model::Song;
use crate::modulation::{self as md, LfoShape, ModKind, Modulation, MAX_MODS};
use crate::theme::th;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke};

/// drag-and-drop payload: a modulator id
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ModDrag(pub u32);

/// depth a dropped route starts with (a quarter of the travel)
pub(super) const DEFAULT_DEPTH: f32 = 0.25;

/// colour of modulator `k` (its chip, rings, depth bars)
pub(super) fn mod_colour(k: usize) -> Color32 {
    const P: [(u8, u8, u8); 6] = [
        (0x2E, 0xC4, 0xB6),
        (0xFF, 0x9F, 0x1C),
        (0xE0, 0x4F, 0xD0),
        (0x6C, 0x9B, 0xFF),
        (0xB5, 0xD3, 0x2A),
        (0xFF, 0x5D, 0x73),
    ];
    let (r, g, b) = P[md::colour_ix(k)];
    Color32::from_rgb(r, g, b)
}

/// one route drawn on a knob: a ring from the value to value + amount
#[derive(Clone, Copy, Debug)]
pub(super) struct Ring {
    pub route: usize,
    pub amount: f32,
    pub col: Color32,
    pub selected: bool,
}

/// the routes onto `target` as rings
pub(super) fn rings(m: &Modulation, target: &str, sel: Option<u32>) -> Vec<Ring> {
    m.routes_to(target)
        .map(|(r, k)| Ring {
            route: r,
            amount: m.routes[r].amount,
            col: mod_colour(k),
            selected: sel == Some(m.mods[k].id),
        })
        .collect()
}

/// where the modulated value is now (0..1), from the engine's live outputs
pub(super) fn live_pos(m: &Modulation, target: &str, n: f32, live: &[f32; MAX_MODS]) -> f32 {
    let d: f32 = m
        .routes_to(target)
        .filter(|(_, k)| *k < MAX_MODS)
        .map(|(r, k)| m.routes[r].amount * live[k])
        .sum();
    (n + d).clamp(0.0, 1.0)
}

/// what the chips / cards / rings ask the app to do afterwards
#[derive(Default)]
pub(super) struct ModOut {
    /// a modulator index to remove (with its routes and depth lanes)
    pub remove_mod: Option<usize>,
    /// a route index to remove
    pub remove_route: Option<usize>,
    /// add an automation lane for this route's depth
    pub lane: Option<usize>,
}

fn kind_tip(k: ModKind) -> &'static str {
    match k {
        ModKind::Lfo => "a free or tempo-synced LFO (sine, triangle, saws, square)",
        ModKind::Envelope => "an ADSR started by every note of this instrument",
        ModKind::Random => "random steps (sample & hold) with optional glide",
        ModKind::Velocity => "the velocity of this instrument's latest note",
        ModKind::Keytrack => "the pitch of this instrument's latest note (C-4 = centre)",
        ModKind::ModWheel => "the MIDI mod wheel (CC 1)",
        ModKind::Macro => "a knob of its own: one move, many parameters",
        ModKind::InstrumentLfo => "this instrument's own synth LFO (its shape, rate and sync)",
    }
}

/// the modulator strip: one chip per modulator of `owner` (drag it onto a
/// knob), then "+ Modulator". Returns true when the modulation changed.
pub(super) fn strip(
    ui: &mut egui::Ui,
    m: &mut Modulation,
    owner: usize,
    has_patch: bool,
    live: &[f32; MAX_MODS],
    sel: &mut Option<u32>,
    out: &mut ModOut,
) -> bool {
    let t = th();
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Modulators").small().color(t.ink2))
            .on_hover_text(
                "drag a modulator onto a knob or FX slider to modulate it · drag the dot on a knob's ring to set the depth · Alt-drag a knob sets the selected modulator's depth",
            );
        for k in 0..m.mods.len() {
            if m.mods[k].owner == owner {
                chip(ui, m, k, live[k.min(MAX_MODS - 1)], sel, out);
            }
        }
        ui.menu_button("+ Modulator", |ui| {
            for kind in ModKind::ALL {
                if kind == ModKind::InstrumentLfo && !has_patch {
                    continue;
                }
                if ui.button(kind.name()).on_hover_text(kind_tip(kind)).clicked() {
                    if let Some(k) = m.add(owner, kind) {
                        *sel = Some(m.mods[k].id);
                        changed = true;
                    }
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("add a modulation source to this instrument");
    });
    changed
}

fn chip(
    ui: &mut egui::Ui,
    m: &Modulation,
    k: usize,
    v: f32,
    sel: &mut Option<u32>,
    out: &mut ModOut,
) {
    let t = th();
    let mdl = &m.mods[k];
    let title = m.title(k);
    let id = mdl.id;
    let col = mod_colour(k);
    let bip = mdl.kind.bipolar(mdl.unipolar);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(100.0, 22.0), Sense::click_and_drag());
    resp.dnd_set_drag_payload(ModDrag(id));
    let selected = *sel == Some(id);
    let p = ui.painter();
    p.rect_filled(rect, 5.0, t.pill);
    p.rect_stroke(
        rect,
        5.0,
        if selected {
            Stroke::new(2.0, col)
        } else {
            Stroke::new(1.0, t.ink2.gamma_multiply(0.4))
        },
        egui::StrokeKind::Inside,
    );
    let dot = Pos2::new(rect.left() + 10.0, rect.center().y);
    p.circle_filled(dot, 4.5, col);
    p.circle_stroke(dot, 7.0, Stroke::new(1.0, col.gamma_multiply(0.7)));
    let mut job = egui::text::LayoutJob::simple_singleline(
        title.clone(),
        crate::theme::prop(crate::theme::ts::SMALL),
        t.ink,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 24.0);
    let g = ui.fonts_mut(|f| f.layout_job(job));
    p.galley(Pos2::new(rect.left() + 20.0, rect.top() + 3.0), g, t.ink);
    // live value along the bottom edge
    let (x0, x1, y) = (rect.left() + 20.0, rect.right() - 5.0, rect.bottom() - 3.5);
    let (a, b) = if bip {
        let c = (x0 + x1) / 2.0;
        (c, c + v.clamp(-1.0, 1.0) * (x1 - x0) / 2.0)
    } else {
        (x0, x0 + v.clamp(0.0, 1.0) * (x1 - x0))
    };
    p.line_segment(
        [Pos2::new(a.min(b), y), Pos2::new(a.max(b) + 0.5, y)],
        Stroke::new(2.0, col),
    );
    if resp.clicked() || resp.drag_started() {
        *sel = Some(id);
    }
    if resp.dragged() {
        // a cable from the chip to the pointer while dragging
        if let Some(pp) = ui.ctx().pointer_latest_pos() {
            let lp = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("mod_cable"),
            ));
            lp.line_segment([dot, pp], Stroke::new(2.0, col));
            lp.circle_filled(pp, 4.0, col);
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    let tl = title.clone();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &tl));
    let resp = resp.on_hover_text(format!(
        "{title}: drag onto a knob or FX slider to modulate it · click to select (Modulation tab edits it) · right-click: remove"
    ));
    resp.context_menu(|ui| {
        if ui.button("Remove modulator").clicked() {
            out.remove_mod = Some(k);
            ui.close();
        }
    });
}

/// note values for the sync menu: (beats per cycle, name)
pub(super) const SYNCS: [(f32, &str); 9] = [
    (0.0, "Free (Hz)"),
    (0.25, "1/16"),
    (0.5, "1/8"),
    (1.0, "1/4"),
    (2.0, "1/2"),
    (4.0, "1 bar"),
    (8.0, "2 bars"),
    (16.0, "4 bars"),
    (32.0, "8 bars"),
];

fn sync_name(s: f32) -> String {
    SYNCS
        .iter()
        .find(|x| (x.0 - s).abs() < 1e-4)
        .map(|x| x.1.to_string())
        .unwrap_or_else(|| format!("{s} beats"))
}

/// the selected modulator of `owner` (else its first one)
pub(super) fn selected(m: &Modulation, owner: usize, sel: Option<u32>) -> Option<usize> {
    sel.and_then(|id| m.find(id))
        .filter(|k| m.mods[*k].owner == owner)
        .or_else(|| m.mods.iter().position(|x| x.owner == owner))
}

/// the Modulators card: settings of the selected modulator
pub(super) fn editor(
    ui: &mut egui::Ui,
    m: &mut Modulation,
    owner: usize,
    live: &[f32; MAX_MODS],
    sel: &mut Option<u32>,
    out: &mut ModOut,
) -> bool {
    let t = th();
    let Some(k) = selected(m, owner, *sel) else {
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "No instrument-wide sources yet. Click the dot on any knob to add movement; its LFO or envelope then shows here.",
                )
                .small()
                .color(t.ink2),
            )
            .wrap(),
        );
        return false;
    };
    let before = m.mods[k].clone();
    let title = m.title(k);
    let col = mod_colour(k);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 5.0, col);
        ui.label(egui::RichText::new(&title).strong());
        let v = live[k.min(MAX_MODS - 1)];
        ui.label(
            egui::RichText::new(format!("{v:+.2}"))
                .monospace()
                .small()
                .color(t.value),
        );
        if ui
            .small_button("Remove")
            .on_hover_text("remove this modulator and its routes")
            .clicked()
        {
            out.remove_mod = Some(k);
        }
    });
    let x = &mut m.mods[k];
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Name").small());
        ui.add(
            egui::TextEdit::singleline(&mut x.name)
                .hint_text(x.kind.name())
                .desired_width(120.0),
        );
    });
    ui.spacing_mut().slider_width = 130.0;
    let sl =
        |ui: &mut egui::Ui, name: &str, v: &mut f32, lo: f32, hi: f32, log: bool, sfx: &str| {
            ui.label(egui::RichText::new(name).small());
            ui.add(
                egui::Slider::new(v, lo..=hi)
                    .logarithmic(log)
                    .suffix(sfx)
                    .max_decimals(3),
            );
            ui.end_row();
        };
    egui::Grid::new(("mod_ed", x.id))
        .num_columns(2)
        .spacing(egui::vec2(6.0, 2.0))
        .show(ui, |ui| match x.kind {
            ModKind::Lfo | ModKind::Random => {
                if x.kind == ModKind::Lfo {
                    ui.label(egui::RichText::new("Shape").small());
                    egui::ComboBox::from_id_salt(("mod_shape", x.id))
                        .selected_text(x.shape.name())
                        .show_ui(ui, |ui| {
                            for s in LfoShape::ALL {
                                ui.selectable_value(&mut x.shape, s, s.name());
                            }
                        });
                    ui.end_row();
                }
                ui.label(egui::RichText::new("Sync").small());
                egui::ComboBox::from_id_salt(("mod_sync", x.id))
                    .selected_text(sync_name(x.sync))
                    .show_ui(ui, |ui| {
                        for (s, n) in SYNCS {
                            ui.selectable_value(&mut x.sync, s, n);
                        }
                    });
                ui.end_row();
                if x.sync <= 0.0 {
                    sl(ui, "Rate", &mut x.rate, 0.01, 30.0, true, " Hz");
                }
                if x.kind == ModKind::Lfo {
                    sl(ui, "Phase", &mut x.phase, 0.0, 1.0, false, "");
                } else {
                    sl(ui, "Glide", &mut x.smooth, 0.0, 1.0, false, "");
                }
                ui.label("");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut x.retrig, "Retrigger")
                        .on_hover_text("restart on every note of this instrument");
                    ui.checkbox(&mut x.unipolar, "Unipolar")
                        .on_hover_text("0..1 instead of -1..1 (only pushes the value up)");
                });
                ui.end_row();
            }
            ModKind::Envelope => {
                sl(ui, "Attack", &mut x.a, 0.001, 10.0, true, " s");
                sl(ui, "Decay", &mut x.d, 0.001, 10.0, true, " s");
                sl(ui, "Sustain", &mut x.s, 0.0, 1.0, false, "");
                sl(ui, "Release", &mut x.r, 0.001, 10.0, true, " s");
            }
            ModKind::Velocity | ModKind::Keytrack | ModKind::ModWheel => {
                sl(ui, "Smooth", &mut x.smooth, 0.0, 1.0, false, " s");
            }
            ModKind::Macro => {
                sl(ui, "Value", &mut x.value, 0.0, 1.0, false, "");
            }
            ModKind::InstrumentLfo => {
                ui.label(
                    egui::RichText::new("follows the instrument's LFO card (shape, rate, sync)")
                        .small()
                        .color(t.ink2),
                );
                ui.end_row();
                ui.label("");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut x.retrig, "Retrigger");
                    ui.checkbox(&mut x.unipolar, "Unipolar");
                });
                ui.end_row();
            }
        });
    *x != before
}

/// short name of a route target (`instrument.2.cutoff` → "Bass Filter cutoff")
pub(super) fn target_label(song: &Song, target: &str) -> String {
    use crate::automation::Target;
    let Some(t) = Target::parse(target) else {
        return target.to_string();
    };
    let iname = |i: usize| {
        song.instruments
            .get(i)
            .map(|x| x.name.clone())
            .unwrap_or_else(|| format!("{:02X}", i + 1))
    };
    match t {
        Target::Cutoff(i)
        | Target::Resonance(i)
        | Target::Drive(i)
        | Target::Level(i)
        | Target::Patch(i, _) => format!("{} {}", iname(i), t.param_name()),
        Target::Fx(i, d, p) => match song.instruments.get(i).and_then(|x| x.fx.get(d)) {
            Some(dev) if dev.plugin.is_some() => format!("{} {} #{p}", iname(i), dev.title()),
            Some(dev) => format!(
                "{} {} {}",
                iname(i),
                dev.title(),
                crate::fx::params(dev.kind)
                    .get(p)
                    .map(|x| x.name)
                    .unwrap_or("?")
            ),
            None => target.to_string(),
        },
        Target::Gen(i, k) => {
            let n = match song.instruments.get(i).map(|x| &x.kind) {
                Some(crate::model::InstrumentKind::Gen(g)) => crate::gen::gen_knobs(g)
                    .into_iter()
                    .find(|x| x.0 == k)
                    .map(|x| x.1),
                _ => None,
            };
            format!(
                "{} {}",
                iname(i),
                n.unwrap_or_else(|| format!("knob {}", k + 1))
            )
        }
        Target::ChanVolume(c) => format!("Track {} volume", c + 1),
        Target::ChanPan(c) => format!("Track {} pan", c + 1),
        Target::Send(c, b) => format!("Track {} send {}", c + 1, if b == 0 { "A" } else { "B" }),
        Target::Ret(b, p) => format!("Return {} p{}", if b == 0 { "A" } else { "B" }, p + 1),
        _ => t.param_name().to_string(),
    }
}

/// "Modulate with" entries of a parameter's right-click menu: connect one
/// of `owner`'s modulators, or remove a route onto this target. True if the
/// modulation changed.
pub(super) fn menu_items(
    ui: &mut egui::Ui,
    m: &mut Modulation,
    owner: usize,
    target: &str,
    sel: &mut Option<u32>,
    out: &mut ModOut,
) -> bool {
    let mut changed = false;
    let own: Vec<usize> = (0..m.mods.len())
        .filter(|k| m.mods[*k].owner == owner)
        .collect();
    ui.menu_button("Modulate with", |ui| {
        for k in own {
            let id = m.mods[k].id;
            let on = m.routes_to(target).any(|(_, j)| j == k);
            if ui.add_enabled(!on, egui::Button::new(m.title(k))).clicked() {
                m.connect(id, target, DEFAULT_DEPTH);
                *sel = Some(id);
                changed = true;
                ui.close();
            }
        }
        ui.separator();
        ui.menu_button("New", |ui| {
            for kind in ModKind::ALL {
                if ui.button(kind.name()).clicked() {
                    if let Some(k) = m.add(owner, kind) {
                        let id = m.mods[k].id;
                        m.connect(id, target, DEFAULT_DEPTH);
                        *sel = Some(id);
                        changed = true;
                    }
                    ui.close();
                }
            }
        });
    });
    let here: Vec<(usize, usize)> = m.routes_to(target).collect();
    for (r, k) in here {
        if ui
            .button(format!(
                "Remove {} ({:+.0} %)",
                m.title(k),
                m.routes[r].amount * 100.0
            ))
            .clicked()
        {
            out.remove_route = Some(r);
            ui.close();
        }
    }
    changed
}

impl App {
    /// Apply what the modulation widgets asked for (after drawing).
    pub(super) fn mod_out(&mut self, out: ModOut) {
        if let Some(k) = out.remove_mod {
            self.snapshot();
            self.with(|s| s.song.remove_modulator(k));
            self.dirty = true;
        }
        if let Some(r) = out.remove_route {
            self.snapshot();
            self.with(|s| s.song.remove_route(r));
            self.dirty = true;
        }
        if let Some(r) = out.lane {
            self.add_lane(crate::automation::Target::ModAmount(r));
        }
    }

    /// An FX slider (`r`, base position `n`) as a modulation target: drop a
    /// modulator on it, drag a depth bar under it, see the live value.
    /// Its right-click menu holds "Add automation lane" when `lane` is given
    /// (set when picked) and the modulation entries.
    pub(super) fn mod_slider(
        &mut self,
        ui: &egui::Ui,
        r: &egui::Response,
        target: &str,
        n: f32,
        lane: Option<&mut bool>,
    ) {
        let (m, live) = self.with(|s| (s.song.modulation.clone(), s.engine.mods.out));
        let mut m2 = m.clone();
        let rect = r.rect;
        let t = th();
        if r.dnd_hover_payload::<ModDrag>().is_some() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                3.0,
                Stroke::new(2.0, t.fx),
                egui::StrokeKind::Outside,
            );
        }
        if let Some(d) = r.dnd_release_payload::<ModDrag>() {
            if m2.connect(d.0, target, DEFAULT_DEPTH).is_some() {
                self.ip.mod_sel = Some(d.0);
                self.status =
                    "Modulation routed: drag the coloured bar under the slider to set the depth"
                        .into();
            }
        }
        // depth bars: one thin bar per route under the slider
        let x = |v: f32| rect.left() + v.clamp(0.0, 1.0) * rect.width();
        let rs = rings(&m, target, self.ip.mod_sel);
        for (j, rg) in rs.iter().enumerate().take(3) {
            let y = rect.bottom() + 1.5 + j as f32 * 3.0;
            let to = n + rg.amount;
            let p = ui.painter();
            p.line_segment(
                [Pos2::new(x(n.min(to)), y), Pos2::new(x(n.max(to)), y)],
                Stroke::new(2.0, rg.col),
            );
            let hp = Pos2::new(x(to), y);
            let hr = ui.interact(
                Rect::from_center_size(hp, egui::vec2(9.0, 7.0)),
                r.id.with(("depth", j)),
                Sense::drag(),
            );
            p.circle_filled(
                hp,
                if hr.hovered() || hr.dragged() {
                    3.5
                } else {
                    2.5
                },
                rg.col,
            );
            if hr.dragged() {
                let d = hr.drag_delta().x / rect.width().max(1.0);
                let a = &mut m2.routes[rg.route].amount;
                *a = (*a + d).clamp(-1.0, 1.0);
            }
            hr.on_hover_text(format!("depth {:+.0} % (drag)", rg.amount * 100.0));
        }
        if !rs.is_empty() {
            let lv = live_pos(&m, target, n, &live);
            ui.painter()
                .circle_filled(Pos2::new(x(lv), rect.center().y), 3.0, t.ink);
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        let mut out = ModOut::default();
        let owner =
            crate::model::lane_instrument(target).unwrap_or((self.instr as usize).wrapping_sub(1));
        let mut sel = self.ip.mod_sel;
        r.context_menu(|ui| {
            if let Some(l) = lane {
                if ui.button("Add automation lane").clicked() {
                    *l = true;
                    ui.close();
                }
                ui.separator();
            }
            if menu_items(ui, &mut m2, owner, target, &mut sel, &mut out) {
                ui.close();
            }
        });
        self.ip.mod_sel = sel;
        if m2 != m {
            self.with(|s| s.song.modulation = m2);
            self.dirty = true;
        }
        self.mod_out(out);
    }
}
