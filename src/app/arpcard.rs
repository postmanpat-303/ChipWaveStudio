//! The Arp card (0.2.16), asked for by Del (postmanpat): the first card in
//! a track's Rack row, before its instrument. Mode, Octaves, Rate, Gate,
//! Swing and Step velocity knobs, Latch and the Random seed on the left; the
//! 16-step grid (all 32 at Length 32) on the right (velocity, gate, accent, rest / tie, ratchet,
//! transpose), with the playing step lit while the song plays and a
//! plain-words readout on top. A Drum Kit gets the same card as Note repeat.
//!
//! arp2: a Chance row in the grid (seeded per-step probability) and a
//! "More" fold-out on the right for the advanced settings: how the octaves
//! go, Retrigger / Free-run, Scale lock (the song key), Humanise, the
//! Euclid fill and the chip fast-chord rates.
//!
//! Switching it off sets the instrument's `arp` to None (nothing is saved,
//! the engine takes its old path) and keeps the settings here for the next
//! switch-on while ChipWave runs. Every edit is a song change, so undo has
//! one step per click (a drag: one step on release). Original ChipWave code.
use super::knob::{self, KnobArgs, CELL};
use super::*;
use crate::arp::{self, Arp, Mode, StepKind, MAX_STEPS};

/// one fold-out edit, applied after the panel is drawn
type ArpEdit = Box<dyn FnOnce(&mut Arp)>;

/// header height
const HEAD: f32 = 24.0;
/// the readout line
const READ_H: f32 = 20.0;
/// the folded / "add" strip
pub(super) const SLIM: f32 = 30.0;
/// the knob block: three columns of knobs
const KNOBS_W: f32 = 3.0 * CELL.x;
/// the grid's row names
pub(super) const LABEL_W: f32 = 56.0;
/// one step column
pub(super) const COL_W: f32 = 22.0;
/// steps shown up to Length 16 (Length 32 widens the grid to all 32)
const PAGE: usize = 16;
const PAD: f32 = 8.0;
/// the More fold-out's width
pub(super) const MORE_W: f32 = 236.0;
/// the chances a Chance cell's click steps through
const CHANCES: [f32; 5] = [1.0, 0.75, 0.5, 0.25, 0.1];

/// the Mode and Rate knobs' last positions
const MODE_MAX: f32 = (Mode::ALL.len() - 1) as f32;
const RATE_MAX: f32 = (arp::RATES.len() - 1) as f32;

/// the transpose values a click steps through
const TRANSPOSE: [i8; 7] = [0, 12, 7, 5, -5, -12, 24];

/// what the card can do with an instrument: (has an Arp, is a drum kit,
/// shows the Arp at all)
fn arp_state(app: &App, idx: usize) -> Option<(Option<Arp>, bool)> {
    app.with(|s| {
        if s.song.rack_of(idx).is_some() {
            return None;
        }
        let i = s.song.instruments.get(idx)?;
        Some((i.arp.clone(), crate::drumkit::kit_of(i).is_some()))
    })
}

/// the card's width in the Rack row (0: none)
pub(super) fn arp_card_w(app: &App, idx: usize, ui: &egui::Ui) -> f32 {
    match arp_state(app, idx) {
        None => 0.0,
        Some((None, _)) => SLIM,
        Some((Some(a), kit)) => card_w(&a, head_w(ui, &a, kit)),
    }
}

/// the step columns the grid shows: 16, or all 32 at Length 32 (the card
/// grows, so every step is on screen at once)
pub(super) fn grid_cols(a: &Arp) -> usize {
    if a.len as usize > PAGE {
        MAX_STEPS
    } else {
        PAGE
    }
}

/// header button padding and spacing (as `arp_head` sets them)
const HEAD_PAD: f32 = 5.0;
const HEAD_GAP: f32 = 4.0;

fn small(s: &str) -> egui::RichText {
    egui::RichText::new(s).size(crate::theme::ts::SMALL)
}

fn title(kit: bool) -> egui::RichText {
    egui::RichText::new(arp_title(kit).to_uppercase())
        .font(crate::theme::label_font(crate::theme::ts::LABEL))
}

/// the width the header's controls need side by side, measured from their
/// text (hdrfix: the Simple card was narrower than its header, so Preset
/// and More painted over Simple / Steps)
fn head_w(ui: &egui::Ui, a: &Arp, kit: bool) -> f32 {
    use super::fit::{button_w, text_w};
    let b = |s: &str| button_w(ui, small(s), HEAD_PAD) + HEAD_GAP;
    let mut w = 20.0 + HEAD_GAP + text_w(ui, title(kit)) + HEAD_GAP + 4.0;
    w += b("Simple") + b("Steps");
    if a.show_steps {
        w += 6.0 + text_w(ui, small("Length")) + HEAD_GAP;
        w += ["8", "16", "32"].iter().map(|n| b(n)).sum::<f32>();
    }
    // a clear gap, then Preset ▾ and More (its wider label)
    w += 8.0 + b("Preset ▾") + b("More ▸").max(b("More ◂"));
    // the header sits 6 pt inside the card on each side
    w + 12.0
}

fn card_w(a: &Arp, head: f32) -> f32 {
    let base = if a.show_steps {
        PAD + KNOBS_W + PAD + LABEL_W + COL_W * grid_cols(a) as f32 + PAD
    } else {
        PAD + KNOBS_W + PAD
    };
    // never narrower than the header's controls
    let base = base.max(head.ceil());
    base + if a.show_more { MORE_W + PAD } else { 0.0 }
}

/// the name the card goes by
pub(crate) fn arp_title(kit: bool) -> &'static str {
    if kit {
        "Note repeat"
    } else {
        "Arp"
    }
}

/// the step the Arp is on at song tick `t` (none while stopped)
fn playing_step(a: &Arp, t: Option<f64>) -> Option<usize> {
    let t = t?;
    let (len, ..) = a.eff(&arp::Mods::default());
    let k = (t / len).floor() as i64;
    Some(k.rem_euclid(a.len.clamp(1, MAX_STEPS as u8) as i64) as usize)
}

fn sub(ui: &mut egui::Ui, r: Rect, f: impl FnOnce(&mut egui::Ui)) {
    let mut c = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    f(&mut c);
}

impl App {
    /// switch the Arp on (the stashed settings, else a starting pattern)
    /// or off (kept for the next switch-on)
    pub(crate) fn arp_set_on(&mut self, idx: usize, on: bool) {
        let stash = self.dev.arp_stash.remove(&idx);
        let old = self.with(|s| {
            let i = s.song.instruments.get_mut(idx)?;
            if on {
                if i.arp.is_none() {
                    i.arp = Some(stash.unwrap_or_default());
                }
                None
            } else {
                i.arp.take()
            }
        });
        if let Some(a) = old {
            self.dev.arp_stash.insert(idx, a);
        }
        let kit = arp_state(self, idx).is_some_and(|x| x.1);
        self.status = format!(
            "{} {}",
            arp_title(kit),
            if on {
                "on: held notes play one step at a time (Ctrl+Z undoes)"
            } else {
                "off: notes play as written"
            }
        );
    }

    fn arp_edit(&mut self, idx: usize, f: impl FnOnce(&mut Arp)) {
        self.with(|s| {
            if let Some(a) = s.song.instruments.get_mut(idx).and_then(|i| i.arp.as_mut()) {
                if a.steps.len() < MAX_STEPS {
                    a.steps.resize(MAX_STEPS, Default::default());
                }
                f(a);
            }
        });
    }

    /// the Arp's place in the Rack row: the card, or a slim "+ Arp" strip
    /// (None: this instrument has none, e.g. a Rack's layer)
    pub(super) fn devices_arp_card(
        &mut self,
        ui: &mut egui::Ui,
        h: f32,
        idx: usize,
    ) -> Option<Rect> {
        let (arp, kit) = arp_state(self, idx)?;
        let t = th();
        let Some(a) = arp else {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(SLIM, h), Sense::hover());
            let resp = ui
                .interact(
                    rect,
                    ui.make_persistent_id(("arp_add", idx)),
                    Sense::click(),
                )
                .on_hover_text(if kit {
                    "Add Note repeat: held pads repeat at a rate, with ratchets"
                } else {
                    "Add an Arp: held notes play one at a time, in time with the song"
                });
            let p = ui.painter_at(rect);
            let hot = resp.hovered();
            p.rect_filled(
                rect,
                6,
                if hot {
                    t.cart_top
                } else {
                    t.cart_top.gamma_multiply(0.6)
                },
            );
            p.rect_stroke(
                rect,
                6,
                Stroke::new(1.0, if hot { t.accent } else { t.line2 }),
                egui::StrokeKind::Inside,
            );
            let label = if kit { "+ NOTE REPEAT" } else { "+ ARP" };
            let g = p.layout_no_wrap(
                label.into(),
                crate::theme::label_font(crate::theme::ts::SMALL),
                if hot { t.accent } else { t.ink2 },
            );
            let w = g.size();
            p.add(
                egui::epaint::TextShape::new(
                    Pos2::new(rect.center().x - w.y / 2.0, rect.top() + 14.0 + w.x),
                    g,
                    t.ink2,
                )
                .with_angle(-std::f32::consts::FRAC_PI_2),
            );
            self.dev.arp_card = rect;
            self.dev.arp_rows = [Rect::NOTHING; 8];
            self.dev.arp_more = Rect::NOTHING;
            if resp.clicked() {
                self.arp_set_on(idx, true);
            }
            return Some(rect);
        };
        let w = card_w(&a, head_w(ui, &a, kit));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
        self.dev.arp_card = rect;
        self.dev.arp_rows = [Rect::NOTHING; 8];
        self.dev.arp_more = Rect::NOTHING;
        let p = ui.painter_at(rect.expand(2.0));
        p.rect_filled(rect, 8, t.cart_top);
        p.rect_stroke(
            rect,
            8,
            Stroke::new(1.0, t.accent.gamma_multiply(0.8)),
            egui::StrokeKind::Inside,
        );
        let head = Rect::from_min_size(rect.min, egui::vec2(w, HEAD));
        p.rect_filled(
            head,
            egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: 0,
                se: 0,
            },
            t.accent.gamma_multiply(0.22),
        );
        self.arp_head(ui, head.shrink2(egui::vec2(6.0, 1.0)), idx, &a, kit);
        // the readout, in plain words
        let rr = Rect::from_min_size(
            Pos2::new(rect.left() + PAD, head.bottom() + 2.0),
            egui::vec2(w - 2.0 * PAD, READ_H - 2.0),
        );
        p.rect_filled(rr, 4, t.accent.gamma_multiply(0.14));
        let words = a.describe(kit);
        crate::theme::text_fit(
            &p,
            Pos2::new(rr.left() + 6.0, rr.center().y),
            egui::Align2::LEFT_CENTER,
            &words,
            crate::theme::prop(crate::theme::ts::SMALL),
            t.ink,
            rr.width() - 12.0,
        );
        ui.interact(
            rr,
            ui.make_persistent_id(("arp_words", idx)),
            Sense::hover(),
        )
        .on_hover_text(&words);
        let body = Rect::from_min_max(
            Pos2::new(rect.left() + PAD, rr.bottom() + 4.0),
            Pos2::new(rect.right() - PAD, rect.bottom() - 4.0),
        );
        let kr = Rect::from_min_size(body.min, egui::vec2(KNOBS_W, body.height()));
        self.arp_knobs(ui, kr, idx, &a, kit);
        let mut body = body;
        if a.show_more {
            let mr = Rect::from_min_max(Pos2::new(body.right() - MORE_W, body.top()), body.max);
            body.max.x = mr.left() - PAD;
            p.line_segment(
                [
                    Pos2::new(mr.left() - PAD / 2.0, mr.top()),
                    Pos2::new(mr.left() - PAD / 2.0, mr.bottom()),
                ],
                Stroke::new(1.0, t.line2),
            );
            self.dev.arp_more = mr;
            self.arp_more(ui, mr, idx, &a, kit);
        }
        if a.show_steps {
            let gr = Rect::from_min_max(Pos2::new(kr.right() + PAD, body.top()), body.max);
            let now = self.with(|s| super::timeline::tl_head(&s.engine));
            self.arp_grid(ui, gr, idx, &a, kit, playing_step(&a, now));
        }
        Some(rect)
    }

    fn arp_head(&mut self, ui: &mut egui::Ui, r: Rect, idx: usize, a: &Arp, kit: bool) {
        let t = th();
        sub(ui, r, |ui| {
            ui.spacing_mut().item_spacing.x = HEAD_GAP;
            ui.spacing_mut().button_padding = egui::vec2(HEAD_PAD, 1.0);
            let (pr, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::hover());
            let id = ui.make_persistent_id(("arp_power", idx));
            if super::devices::power(ui, &ui.painter_at(pr.expand(4.0)), pr, id, true) {
                self.arp_set_on(idx, false);
            }
            ui.label(title(kit).color(t.ink));
            ui.add_space(4.0);
            for (steps, name, tip) in [
                (false, "Simple", "the knobs only"),
                (true, "Steps", "the knobs and the step grid"),
            ] {
                if ui
                    .add(egui::Button::new(small(name)).selected(a.show_steps == steps))
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.arp_edit(idx, |a| a.show_steps = steps);
                }
            }
            if a.show_steps {
                ui.add_space(6.0);
                ui.label(small("Length").color(t.ink2));
                for n in [8u8, 16, 32] {
                    if ui
                        .add(egui::Button::new(small(&n.to_string())).selected(a.len == n))
                        .on_hover_text(format!("the pattern repeats every {n} steps"))
                        .clicked()
                    {
                        self.arp_edit(idx, |a| a.len = n);
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(small(if a.show_more { "More ◂" } else { "More ▸" }))
                            .selected(a.show_more),
                    )
                    .on_hover_text(
                        "the advanced settings: octave walk, retrigger, scale lock, humanise, Euclid fill, fast-chord rates",
                    )
                    .clicked()
                {
                    self.arp_edit(idx, |a| a.show_more = !a.show_more);
                }
                ui.menu_button(small("Preset ▾"), |ui| {
                    for (name, p) in arp::presets(kit) {
                        if ui.button(name).clicked() {
                            let keep = a.show_steps;
                            self.arp_edit(idx, |a| {
                                *a = p.clone();
                                a.show_steps = keep;
                            });
                            self.status = format!("{}: {name}", arp_title(kit));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("starting patterns (Ctrl+Z undoes)");
            });
        });
    }

    fn arp_knobs(&mut self, ui: &mut egui::Ui, r: Rect, idx: usize, a: &Arp, kit: bool) {
        let ri = a.rate_index();
        let mi = Mode::ALL.iter().position(|&m| m == a.mode).unwrap_or(0);
        // (name, value 0..1, text, default, steps, tip)
        let mut ks: Vec<(&str, f32, String, f32, f32, &str)> = vec![];
        if !kit {
            ks.push((
                "Mode",
                mi as f32 / MODE_MAX,
                a.mode.name().into(),
                0.0,
                MODE_MAX,
                "the order the held notes play in",
            ));
            ks.push((
                "Octaves",
                (a.octaves.clamp(1, 4) - 1) as f32 / 3.0,
                a.octaves.to_string(),
                0.0,
                3.0,
                "play the held notes over 1 to 4 octaves",
            ));
        }
        ks.push((
            "Rate",
            ri as f32 / RATE_MAX,
            arp::rate_name(a.div, a.feel),
            7.0 / RATE_MAX,
            RATE_MAX,
            "how fast, in time with the song: 1/4 to 1/128, T = triplet, . = dotted; 1/64 and up are the chip fast-chord rates",
        ));
        ks.push((
            "Gate",
            (a.gate - 0.05) / 0.95,
            format!("{}%", (a.gate * 100.0).round()),
            0.45 / 0.95,
            0.0,
            "how long each note lasts, as a share of its step",
        ));
        ks.push((
            "Swing",
            a.swing,
            format!("{}%", (a.swing * 100.0).round()),
            0.0,
            0.0,
            "pushes every second step late",
        ));
        ks.push((
            "Step vel",
            a.vel_depth,
            format!("{}%", (a.vel_depth * 100.0).round()),
            1.0,
            0.0,
            "how much the grid's velocity row counts",
        ));
        for (k, (name, v0, text, def, steps, tip)) in ks.into_iter().enumerate() {
            let cr = Rect::from_min_size(
                r.min + egui::vec2((k % 3) as f32 * CELL.x, (k / 3) as f32 * CELL.y),
                CELL,
            );
            let mut v = v0;
            let mut changed = false;
            let mut resp = None;
            sub(ui, cr, |ui| {
                let o = knob::knob(
                    ui,
                    ui.make_persistent_id(("arp_knob", idx, name)),
                    &mut v,
                    KnobArgs {
                        caption: None,
                        name,
                        value_text: text,
                        default: def,
                        steps,
                        enabled: true,
                        highlight: false,
                        badge: None,
                        locked: None,
                        tip,
                        rings: &[],
                        live: None,
                        auto_led: None,
                        mod_dot: None,
                    },
                );
                changed = o.changed;
                resp = Some(o.resp);
            });
            // Rate, Gate, Swing and Octaves are automation and modulation
            // targets: drop a modulator on the knob, right-click for a lane
            let param = match name {
                "Rate" => Some(arp::Param::Rate),
                "Gate" => Some(arp::Param::Gate),
                "Swing" => Some(arp::Param::Swing),
                "Octaves" => Some(arp::Param::Octaves),
                _ => None,
            };
            if let (Some(r), Some(p)) = (resp, param) {
                let pi = arp::Param::ALL.iter().position(|q| *q == p).unwrap_or(0);
                let t = crate::automation::Target::Arp(idx, pi);
                let mut lane = false;
                self.mod_slider(ui, &r, &t.id(), v0, Some(&mut lane));
                if lane {
                    self.add_lane(t);
                }
            }
            if changed {
                let v = v.clamp(0.0, 1.0);
                self.arp_edit(idx, |a| match name {
                    "Mode" => a.mode = Mode::ALL[(v * MODE_MAX).round() as usize],
                    "Octaves" => a.octaves = 1 + (v * 3.0).round() as u8,
                    "Rate" => a.set_rate_index((v * RATE_MAX).round() as usize),
                    "Gate" => a.gate = 0.05 + v * 0.95,
                    "Swing" => a.swing = v,
                    _ => a.vel_depth = v,
                });
            }
        }
        // Latch and the seed under the knobs
        let lr = Rect::from_min_size(
            r.min + egui::vec2(0.0, 2.0 * CELL.y + 2.0),
            egui::vec2(r.width(), 20.0),
        );
        sub(ui, lr, |ui| {
            ui.spacing_mut().item_spacing.x = HEAD_GAP;
            ui.spacing_mut().button_padding = egui::vec2(HEAD_PAD, 1.0);
            let small = |s: &str| egui::RichText::new(s).size(crate::theme::ts::SMALL);
            if ui
                .add(egui::Button::new(small("Latch")).selected(a.latch))
                .on_hover_text("keep playing after you let go, until a new chord")
                .clicked()
            {
                self.arp_edit(idx, |a| a.latch = !a.latch);
            }
            if !kit
                && ui
                    .add(egui::Button::new(small(&format!("Seed {}", a.seed))))
                    .on_hover_text(
                        "Random mode's seed: the same seed plays the same notes every time; click for a new one",
                    )
                    .clicked()
            {
                self.arp_edit(idx, |a| {
                    a.seed = a.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 1000
                        + 1;
                });
            }
        });
    }

    /// the More fold-out: the advanced settings, one line each
    fn arp_more(&mut self, ui: &mut egui::Ui, r: Rect, idx: usize, a: &Arp, kit: bool) {
        let t = th();
        let key = self.with(|s| s.song.info.key);
        let small = |s: &str| egui::RichText::new(s).size(crate::theme::ts::SMALL);
        let line_h = 22.0;
        let mut y = r.top();
        let mut line = |ui: &mut egui::Ui, f: &mut dyn FnMut(&mut egui::Ui)| {
            let lr = Rect::from_min_size(Pos2::new(r.left(), y), egui::vec2(r.width(), line_h));
            y += line_h + 2.0;
            if lr.bottom() <= r.bottom() + 0.5 {
                sub(ui, lr, |ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 1.0);
                    f(ui)
                });
            }
        };
        let name = |ui: &mut egui::Ui, s: &str| {
            let (nr, _) = ui.allocate_exact_size(egui::vec2(58.0, 18.0), Sense::hover());
            ui.painter().text(
                nr.left_center(),
                egui::Align2::LEFT_CENTER,
                s,
                crate::theme::prop(crate::theme::ts::SMALL),
                t.ink2,
            );
        };
        let mut edit: Option<ArpEdit> = None;
        if !kit {
            line(ui, &mut |ui| {
                name(ui, "Octaves go");
                for m in arp::OctMode::ALL {
                    if ui
                        .add(egui::Button::new(small(m.name())).selected(a.oct_mode == m))
                        .on_hover_text(match m {
                            arp::OctMode::Up => "each pass of the run goes an octave up",
                            arp::OctMode::Down => "each pass goes an octave down",
                            arp::OctMode::Bounce => "up the octaves and back down",
                        })
                        .clicked()
                    {
                        edit = Some(Box::new(move |a| a.oct_mode = m));
                    }
                }
            });
        }
        line(ui, &mut |ui| {
            name(ui, "Pattern");
            for (on, txt, tip) in [
                (
                    true,
                    "Retrigger",
                    "a new note starts the pattern and the run from step 1",
                ),
                (
                    false,
                    "Free-run",
                    "the pattern runs on the song grid; new notes join it where it is",
                ),
            ] {
                if ui
                    .add(egui::Button::new(small(txt)).selected(a.retrig == on))
                    .on_hover_text(tip)
                    .clicked()
                {
                    edit = Some(Box::new(move |a| a.retrig = on));
                }
            }
        });
        if !kit {
            line(ui, &mut |ui| {
                name(ui, "Scale lock");
                let words = match key {
                    Some(k) => k.name(),
                    None => "set a song key".into(),
                };
                if ui
                    .add(egui::Button::new(small(if a.scale_lock { "On" } else { "Off" })).selected(a.scale_lock))
                    .on_hover_text(
                        "keep every note and step transpose in the song's key (Song info › Key); transposes count scale steps",
                    )
                    .clicked()
                {
                    edit = Some(Box::new(|a| a.scale_lock = !a.scale_lock));
                }
                ui.label(small(&words).color(if key.is_some() { t.ink } else { t.ochre }));
            });
        }
        line(ui, &mut |ui| {
            name(ui, "Humanise");
            for v in [0.0f32, 0.1, 0.25, 0.5] {
                let on = (a.humanise - v).abs() < 0.01;
                if ui
                    .add(egui::Button::new(small(&format!("{}%", (v * 100.0) as u32))).selected(on))
                    .on_hover_text(
                        "seeded velocity variation: the same every play (the Seed changes it)",
                    )
                    .clicked()
                {
                    edit = Some(Box::new(move |a| a.humanise = v));
                }
            }
        });
        let mut eu = self.dev.arp_euclid;
        let mut fill = false;
        line(ui, &mut |ui| {
            name(ui, "Euclid");
            let mut hits = eu[0] as u32;
            let mut len = eu[1] as u32;
            let mut rot = eu[2] as u32;
            ui.add(egui::DragValue::new(&mut hits).range(1..=len.max(1)))
                .on_hover_text("hits: the steps that play");
            ui.label(small("/"));
            ui.add(egui::DragValue::new(&mut len).range(1..=MAX_STEPS as u32))
                .on_hover_text("over this many steps (the pattern's new length)");
            ui.label(small("↻"));
            ui.add(egui::DragValue::new(&mut rot).range(0..=len.saturating_sub(1)))
                .on_hover_text("turn the pattern by this many steps");
            eu = [
                hits.min(len) as u8,
                len as u8,
                rot.min(len.saturating_sub(1)) as u8,
            ];
            fill = ui
                .add(egui::Button::new(small("Fill")))
                .on_hover_text(
                    "spread the hits as evenly as they go over the steps; the rest rest (Ctrl+Z undoes)",
                )
                .clicked();
        });
        self.dev.arp_euclid = eu;
        if fill {
            let [h, l, ro] = eu;
            edit = Some(Box::new(move |a| {
                a.euclid_fill(h as usize, l as usize, ro as usize)
            }));
            self.status = format!("{}: Euclid {h}/{l} filled", arp_title(kit));
        }
        line(ui, &mut |ui| {
            name(ui, "Fast chord");
            for (div, feel) in [
                (64u8, arp::Feel::Straight),
                (64, arp::Feel::Triplet),
                (128, arp::Feel::Straight),
            ] {
                let on = a.div == div && a.feel == feel;
                if ui
                    .add(egui::Button::new(small(&arp::rate_name(div, feel))).selected(on))
                    .on_hover_text(
                        "chip fast-chord rates: the held chord flickers into one buzzy tone (1/96 at 125 BPM is one 50 Hz frame, as on the C64)",
                    )
                    .clicked()
                {
                    edit = Some(Box::new(move |a| {
                        a.div = div;
                        a.feel = feel;
                        a.gate = 1.0;
                    }));
                }
            }
        });
        if let Some(f) = edit {
            self.arp_edit(idx, f);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn arp_grid(
        &mut self,
        ui: &mut egui::Ui,
        r: Rect,
        idx: usize,
        a: &Arp,
        kit: bool,
        now: Option<usize>,
    ) {
        let t = th();
        let p = ui.painter_at(r.expand(1.0));
        let len = a.len.clamp(1, MAX_STEPS as u8) as usize;
        let cols = grid_cols(a);
        // the rows' heights: velocity and gate share what is left
        let fixed = 14.0 + 5.0 * 15.0 + 7.0 * 3.0;
        let free = (r.height() - fixed).max(40.0);
        let vel_h = (free * 0.6).floor();
        let gate_h = free - vel_h;
        let names: [(&str, f32); 8] = [
            ("", 14.0),
            ("Velocity", vel_h),
            ("Gate", gate_h),
            ("Accent", 15.0),
            ("Rest / tie", 15.0),
            ("Ratchet", 15.0),
            (if kit { "" } else { "Transpose" }, 15.0),
            ("Chance", 15.0),
        ];
        let mut y = r.top();
        let mut rows = [Rect::NOTHING; 8];
        for (k, (name, h)) in names.iter().enumerate() {
            rows[k] = Rect::from_min_size(Pos2::new(r.left(), y), egui::vec2(r.width(), *h));
            if !name.is_empty() {
                p.text(
                    Pos2::new(r.left(), y + h / 2.0),
                    egui::Align2::LEFT_CENTER,
                    *name,
                    crate::theme::prop(crate::theme::ts::TINY),
                    t.ink2,
                );
            }
            if *h > 0.0 {
                y += h + 3.0;
            }
        }
        self.dev.arp_rows = rows;
        let x0 = r.left() + LABEL_W;
        let tiny = crate::theme::prop(crate::theme::ts::TINY);
        let col = |k: usize, row: Rect| {
            Rect::from_min_size(
                Pos2::new(x0 + k as f32 * COL_W + 1.0, row.top()),
                egui::vec2(COL_W - 2.0, row.height()),
            )
        };
        let pointer = ui.ctx().pointer_latest_pos();
        let mut edits: Vec<(usize, StepEdit)> = vec![];
        for c in 0..cols {
            let k = c;
            let used = k < len;
            let s = a.step(k);
            let on = now == Some(k);
            let rest = s.kind == StepKind::Rest;
            let dim = |c: Color32| if used { c } else { c.gamma_multiply(0.25) };
            p.text(
                col(c, rows[0]).center(),
                egui::Align2::CENTER_CENTER,
                (k + 1).to_string(),
                tiny.clone(),
                if on { t.acc_hot } else { dim(t.ink2) },
            );
            // velocity and gate bars (drag to draw)
            for (row, v, which) in [(rows[1], s.vel, 0u8), (rows[2], s.gate, 1)] {
                let cr = col(c, row);
                p.rect_filled(cr, 2, t.crt_top);
                let hv = (cr.height() * v.clamp(0.0, 1.0)).max(2.0);
                let bar = Rect::from_min_max(Pos2::new(cr.left(), cr.bottom() - hv), cr.max);
                let fill = if rest {
                    t.ink2.gamma_multiply(0.3)
                } else if which == 0 && s.accent {
                    t.ochre
                } else {
                    t.accent
                };
                p.rect_filled(bar, 2, dim(fill));
                let id = ui.make_persistent_id(("arp_bar", idx, which, k));
                let resp = ui.interact(cr, id, Sense::click_and_drag());
                let resp = resp.on_hover_text(if which == 0 {
                    "velocity: click or drag across to draw"
                } else {
                    "gate (times the Gate knob): click or drag across to draw"
                });
                if (resp.is_pointer_button_down_on() || resp.dragged()) && used {
                    // a drag that started here draws across every column
                    if let Some(pp) = pointer {
                        let ck = ((pp.x - x0) / COL_W).floor();
                        if (0.0..cols as f32).contains(&ck) {
                            let kk = ck as usize;
                            let v = ((cr.bottom() - pp.y) / cr.height()).clamp(0.05, 1.0);
                            if kk < len {
                                edits.push((kk, StepEdit::Bar(which, v)));
                            }
                        }
                    }
                }
            }
            // accent
            let cr = col(c, rows[3]);
            p.rect_filled(cr, 2, t.crt_top);
            if s.accent {
                p.circle_filled(cr.center(), 4.0, dim(t.ochre));
            } else {
                p.circle_filled(cr.center(), 1.5, dim(t.ink2));
            }
            if ui
                .interact(
                    cr,
                    ui.make_persistent_id(("arp_acc", idx, k)),
                    Sense::click(),
                )
                .on_hover_text("accent: louder")
                .clicked()
                && used
            {
                edits.push((k, StepEdit::Accent));
            }
            // rest / tie
            let cr = col(c, rows[4]);
            p.rect_filled(cr, 2, t.crt_top);
            let (txt, colr) = match s.kind {
                StepKind::Play => ("", t.ink2),
                StepKind::Rest => ("rest", t.ink),
                StepKind::Tie => ("tie", t.accent),
            };
            if !txt.is_empty() {
                crate::theme::text_fit(
                    &p,
                    cr.center(),
                    egui::Align2::CENTER_CENTER,
                    txt,
                    tiny.clone(),
                    dim(colr),
                    cr.width(),
                );
            }
            if ui
                .interact(
                    cr,
                    ui.make_persistent_id(("arp_kind", idx, k)),
                    Sense::click(),
                )
                .on_hover_text("click: play → rest → tie (holds the note before on)")
                .clicked()
                && used
            {
                edits.push((k, StepEdit::Kind));
            }
            // ratchet
            let cr = col(c, rows[5]);
            p.rect_filled(cr, 2, if s.ratchet > 1 { t.acc_deep } else { t.crt_top });
            p.text(
                cr.center(),
                egui::Align2::CENTER_CENTER,
                if s.ratchet > 1 {
                    format!("×{}", s.ratchet)
                } else {
                    "1".into()
                },
                tiny.clone(),
                dim(if s.ratchet > 1 { t.ink } else { t.ink2 }),
            );
            let resp = ui
                .interact(
                    cr,
                    ui.make_persistent_id(("arp_rat", idx, k)),
                    Sense::click(),
                )
                .on_hover_text(
                    "ratchet: 1-4 quick hits in the step (click; right-click goes back)",
                );
            if used && resp.clicked() {
                edits.push((k, StepEdit::Ratchet(1)));
            }
            if used && resp.secondary_clicked() {
                edits.push((k, StepEdit::Ratchet(-1)));
            }
            // transpose (melodic only)
            if !kit {
                let cr = col(c, rows[6]);
                p.rect_filled(
                    cr,
                    2,
                    if s.transpose != 0 {
                        t.acc_deep
                    } else {
                        t.crt_top
                    },
                );
                let txt = if s.transpose > 0 {
                    format!("+{}", s.transpose)
                } else {
                    s.transpose.to_string()
                };
                p.text(
                    cr.center(),
                    egui::Align2::CENTER_CENTER,
                    txt,
                    tiny.clone(),
                    dim(if s.transpose != 0 { t.ink } else { t.ink2 }),
                );
                let resp = ui
                    .interact(cr, ui.make_persistent_id(("arp_tr", idx, k)), Sense::click())
                    .on_hover_text(
                        "transpose the step: click for 0 → +12 → +7 → +5 → -5 → -12 → +24; the wheel moves by 1",
                    );
                if used && resp.clicked() {
                    edits.push((k, StepEdit::Transpose(None)));
                }
                if used && resp.hovered() {
                    let d = ui.input(|i| i.smooth_scroll_delta.y);
                    if d.abs() > 0.5 {
                        edits.push((k, StepEdit::Transpose(Some(d.signum() as i8))));
                    }
                }
            }
            // chance (seeded: the same steps drop out every play)
            let cr = col(c, rows[7]);
            let ch = s.chance.clamp(0.0, 1.0);
            p.rect_filled(cr, 2, t.crt_top);
            if ch < 1.0 {
                let hv = (cr.height() * ch).max(2.0);
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(cr.left(), cr.bottom() - hv), cr.max),
                    2,
                    dim(t.acc_deep),
                );
            }
            crate::theme::text_fit(
                &p,
                cr.center(),
                egui::Align2::CENTER_CENTER,
                &format!("{}", (ch * 100.0).round()),
                tiny.clone(),
                dim(if ch < 1.0 { t.ink } else { t.ink2 }),
                cr.width(),
            );
            let resp = ui
                .interact(cr, ui.make_persistent_id(("arp_ch", idx, k)), Sense::click())
                .on_hover_text(
                    "chance the step plays, %: click for 100 → 75 → 50 → 25 → 10 (right-click goes back); the wheel moves by 5. Seeded, so the song plays the same every time",
                );
            if used && resp.clicked() {
                edits.push((k, StepEdit::Chance(1)));
            }
            if used && resp.secondary_clicked() {
                edits.push((k, StepEdit::Chance(-1)));
            }
            if used && resp.hovered() {
                let d = ui.input(|i| i.smooth_scroll_delta.y);
                if d.abs() > 0.5 {
                    edits.push((k, StepEdit::ChanceBy(d.signum() * 0.05)));
                }
            }
            // the playing step: a box down the column
            if on {
                let top = col(c, rows[0]).top() - 1.0;
                let bot = rows[7].bottom() + 1.0;
                p.rect_stroke(
                    Rect::from_min_max(
                        Pos2::new(x0 + c as f32 * COL_W, top),
                        Pos2::new(x0 + (c + 1) as f32 * COL_W, bot),
                    ),
                    3,
                    Stroke::new(1.5, t.acc_hot),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if !edits.is_empty() {
            self.arp_edit(idx, |a| {
                for (k, e) in edits {
                    apply(&mut a.steps[k], e);
                }
            });
        }
    }
}

/// one grid edit
#[derive(Clone, Copy, Debug)]
pub(crate) enum StepEdit {
    /// velocity (0) or gate (1) bar
    Bar(u8, f32),
    Accent,
    Kind,
    Ratchet(i8),
    /// None: the next of [`TRANSPOSE`]; Some(d): by d semitones
    Transpose(Option<i8>),
    /// the next (1) or last (-1) of [`CHANCES`]
    Chance(i8),
    ChanceBy(f32),
}

pub(crate) fn apply(s: &mut arp::Step, e: StepEdit) {
    match e {
        StepEdit::Bar(0, v) => s.vel = (v * 20.0).round() / 20.0,
        StepEdit::Bar(_, v) => s.gate = ((v * 20.0).round() / 20.0).max(0.1),
        StepEdit::Accent => s.accent = !s.accent,
        StepEdit::Kind => {
            s.kind = match s.kind {
                StepKind::Play => StepKind::Rest,
                StepKind::Rest => StepKind::Tie,
                StepKind::Tie => StepKind::Play,
            }
        }
        StepEdit::Ratchet(d) => {
            let r = s.ratchet.clamp(1, 4) as i8 - 1 + d;
            s.ratchet = (r.rem_euclid(4) + 1) as u8;
        }
        StepEdit::Transpose(None) => {
            let i = TRANSPOSE.iter().position(|&v| v == s.transpose);
            s.transpose = TRANSPOSE[i.map_or(0, |i| (i + 1) % TRANSPOSE.len())];
        }
        StepEdit::Transpose(Some(d)) => s.transpose = (s.transpose + d).clamp(-24, 24),
        StepEdit::Chance(d) => {
            let n = CHANCES.len() as i8;
            let i = CHANCES
                .iter()
                .position(|&v| (v - s.chance).abs() < 0.01)
                .map_or(if d > 0 { -1 } else { 0 }, |i| i as i8);
            s.chance = CHANCES[(i + d).rem_euclid(n) as usize];
        }
        StepEdit::ChanceBy(d) => s.chance = ((s.chance + d) * 20.0).round().clamp(1.0, 20.0) / 20.0,
    }
}

/// the ARP badge on a track header: the badge text for instrument slot
/// `idx` (None: no Arp)
pub(crate) fn badge(song: &Song, idx: usize) -> Option<&'static str> {
    let i = song.instruments.get(idx)?;
    i.arp.as_ref()?;
    Some(if crate::drumkit::kit_of(i).is_some() {
        "RPT"
    } else {
        "ARP"
    })
}
