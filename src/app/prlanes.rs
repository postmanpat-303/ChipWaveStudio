//! The piano roll's lower lane (docs/ROUND_ROBIN.md): Velocity, Conditions
//! (the "Play this note…" step conditions) and Chance, switched from the
//! lane's label. Note clips only; Tracker blocks keep the velocity lane.
use super::pianoview::{sel_note_colours, RNote, Source, Target};
use super::*;
use crate::timeline::{self as tl, Cond, Tick};

/// what the lane under the notes shows
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lane {
    #[default]
    Vel,
    Cond,
    Chance,
}

impl Lane {
    pub(crate) const ALL: [Lane; 3] = [Lane::Vel, Lane::Cond, Lane::Chance];
    pub(crate) fn short(self) -> &'static str {
        match self {
            Lane::Vel => "Vel",
            Lane::Cond => "Cond",
            Lane::Chance => "Chance",
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Lane::Vel => "Velocity",
            Lane::Cond => "Conditions",
            Lane::Chance => "Chance",
        }
    }
    pub(crate) fn tip(self) -> &'static str {
        match self {
            Lane::Vel => "How hard each note is played. Drag a bar up or down.",
            Lane::Cond => {
                "When each note plays as the clip loops: every pass, 1 time in 4, only the \
                 first pass, only in a fill… Click a note's chip to choose."
            }
            Lane::Chance => {
                "How likely each note is to play on a pass (100 % = always). The dice are the \
                 same every time you play or export, so a take you like stays."
            }
        }
    }
}

/// the "Play this note…" popover: the notes it edits and where it opened
#[derive(Clone, Debug)]
pub(crate) struct CondPop {
    pub(crate) target: Target,
    pub(crate) ids: Vec<usize>,
    pub(crate) at: Pos2,
}

/// the choices of the popover, in order
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    Always,
    Sometimes,
    Every,
    First,
    NotFirst,
    Fill,
    NotFill,
}

impl Choice {
    pub(crate) const ALL: [Choice; 7] = [
        Choice::Always,
        Choice::Sometimes,
        Choice::Every,
        Choice::First,
        Choice::NotFirst,
        Choice::Fill,
        Choice::NotFill,
    ];
    pub(crate) fn label(self) -> &'static str {
        match self {
            Choice::Always => "Always",
            Choice::Sometimes => "Sometimes",
            Choice::Every => "Every",
            Choice::First => "First pass only",
            Choice::NotFirst => "Not on the first pass",
            Choice::Fill => "Only in a fill",
            Choice::NotFill => "Not in a fill",
        }
    }
    pub(crate) fn tip(self) -> &'static str {
        match self {
            Choice::Always => "The note plays every time the clip goes round.",
            Choice::Sometimes => {
                "The note plays only some of the time, by the % below. The same passes play \
                 every time you press Play."
            }
            Choice::Every => {
                "The note plays on one pass out of every few: 2 of 4 = passes 2, 6, 10…"
            }
            Choice::First => "The note plays the first time round only: a pickup or an intro hit.",
            Choice::NotFirst => "The note sits out the first time round, then joins in.",
            Choice::Fill => {
                "The note plays only in a fill: the clip's last pass, a pass that ends where a \
                 song marker starts the next section, or while Fill is on."
            }
            Choice::NotFill => "The note drops out in fills, making room for the fill notes.",
        }
    }
    pub(crate) fn of(n: &tl::Note) -> Choice {
        match n.cond {
            Cond::Always if n.chance > 0 => Choice::Sometimes,
            Cond::Always => Choice::Always,
            Cond::Every { .. } => Choice::Every,
            Cond::First => Choice::First,
            Cond::NotFirst => Choice::NotFirst,
            Cond::Fill => Choice::Fill,
            Cond::NotFill => Choice::NotFill,
        }
    }
    /// set note `n` to this choice (Sometimes and Always set the chance;
    /// the others keep it, so a condition can also be "sometimes")
    pub(crate) fn apply(self, n: &mut tl::Note, every: (u8, u8)) {
        match self {
            Choice::Always => {
                n.cond = Cond::Always;
                n.chance = 0;
            }
            Choice::Sometimes => {
                n.cond = Cond::Always;
                if n.chance == 0 {
                    n.chance = 50;
                }
            }
            Choice::Every => {
                n.cond = Cond::Every {
                    a: every.0,
                    b: every.1,
                }
            }
            Choice::First => n.cond = Cond::First,
            Choice::NotFirst => n.cond = Cond::NotFirst,
            Choice::Fill => n.cond = Cond::Fill,
            Choice::NotFill => n.cond = Cond::NotFill,
        }
    }
}

/// a note's chip in the Conditions lane: "2:4", "1st", "50 %", "Fill 50 %";
/// empty when it always plays
pub(crate) fn chip(cond: Cond, chance: u8) -> String {
    let c = cond.chip();
    match (c.is_empty(), chance) {
        (true, 0) => String::new(),
        (true, p) => format!("{p} %"),
        (false, 0) => c,
        (false, p) => format!("{c} {p} %"),
    }
}

/// the note plays on some passes only (striped in the roll)
pub(crate) fn conditional(n: &RNote) -> bool {
    n.cond != Cond::Always || n.chance > 0
}

/// diagonal stripes over a conditional note's rect
pub(crate) fn stripes(p: &egui::Painter, r: Rect, c: Color32) {
    let p = p.with_clip_rect(r.intersect(p.clip_rect()));
    let step = 6.0;
    let mut x = r.left() - r.height();
    while x < r.right() {
        p.line_segment(
            [Pos2::new(x, r.bottom()), Pos2::new(x + r.height(), r.top())],
            Stroke::new(1.5, c),
        );
        x += step;
    }
}

impl App {
    /// the lane label (left of the lane): click for the lane menu
    pub(super) fn pr_lane_label(&mut self, ui: &mut egui::Ui, r: Rect, target: Target) {
        let t = th();
        let p = ui.painter_at(r);
        let notes_target = matches!(target, Target::Notes { .. });
        if !notes_target {
            self.pr.lane = Lane::Vel;
        }
        let lane = self.pr.lane;
        let resp = ui.interact(
            r,
            egui::Id::unique("pr_lane_label"),
            if notes_target {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let hot = notes_target && resp.hovered();
        if hot {
            p.rect_filled(r.shrink(2.0), 3.0, t.sel.gamma_multiply(0.25));
        }
        p.text(
            Pos2::new(
                r.left() + 4.0,
                r.center().y - if notes_target { 6.0 } else { 0.0 },
            ),
            egui::Align2::LEFT_CENTER,
            lane.short(),
            crate::theme::prop(crate::theme::ts::TINY),
            if lane == Lane::Vel { t.ink2 } else { t.accent },
        );
        if notes_target {
            // a small "choose" triangle
            let c = Pos2::new(r.left() + 10.0, r.center().y + 8.0);
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + egui::vec2(-4.0, -2.0),
                    c + egui::vec2(4.0, -2.0),
                    c + egui::vec2(0.0, 3.0),
                ],
                if hot { t.ink } else { t.ink2 },
                Stroke::NONE,
            ));
            let tip = format!(
                "{}: {}\nClick to show Velocity, Conditions or Chance here.",
                lane.name(),
                lane.tip()
            );
            let resp = resp.clone().on_hover_text(tip);
            if resp.clicked() {
                self.pr.lane_menu = Some(r.left_top());
            }
        }
        if let Some(at) = self.pr.lane_menu {
            let mut close = false;
            let area = egui::Area::new(egui::Id::unique("pr_lane_menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(at - egui::vec2(0.0, 112.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::menu(ui.style()).show(ui, |ui| {
                        ui.set_min_width(150.0);
                        ui.label(egui::RichText::new("Show in the lane").color(t.ink2));
                        for l in Lane::ALL {
                            if ui
                                .selectable_label(self.pr.lane == l, l.name())
                                .on_hover_text(l.tip())
                                .clicked()
                            {
                                self.pr.lane = l;
                                close = true;
                            }
                        }
                    });
                });
            let outside = ui.input(|i| i.pointer.any_pressed())
                && !area.response.contains_pointer()
                && !resp.contains_pointer();
            if close || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.pr.lane_menu = None;
            }
        }
    }

    /// draw the Conditions or Chance lane (false: the velocity lane draws)
    pub(super) fn pr_lane_draw(
        &mut self,
        ui: &egui::Ui,
        vel_r: Rect,
        notes: &[RNote],
        base: Color32,
    ) -> bool {
        let t = th();
        let p = ui.painter_at(vel_r);
        match self.pr.lane {
            Lane::Vel => false,
            Lane::Chance => {
                // 100 % guide
                p.hline(
                    vel_r.x_range(),
                    vel_r.top() + 2.0,
                    Stroke::new(0.5, t.ink2.gamma_multiply(0.5)),
                );
                for n in notes {
                    let x = self.pr.x_of(n.start);
                    if x < vel_r.left() - 2.0 || x > vel_r.right() {
                        continue;
                    }
                    let v = if n.chance == 0 {
                        1.0
                    } else {
                        n.chance as f32 / 100.0
                    };
                    let h = (vel_r.height() - 4.0) * v;
                    let sel = self.pr.sel.contains(&n.id);
                    let c = if sel {
                        sel_note_colours(&t, 1.0).0
                    } else if n.chance == 0 {
                        base.gamma_multiply(0.5)
                    } else {
                        base
                    };
                    let top = vel_r.bottom() - 2.0 - h;
                    p.vline(x + 1.0, top..=vel_r.bottom() - 2.0, Stroke::new(2.0, c));
                    p.circle_filled(Pos2::new(x + 1.0, top), 2.5, c);
                }
                true
            }
            Lane::Cond => {
                // one chip per start time (the selected notes there first)
                let mut last_x = f32::NEG_INFINITY;
                for (start, ids) in groups(notes, &self.pr.sel) {
                    let x = self.pr.x_of(start);
                    if x < vel_r.left() - 40.0 || x > vel_r.right() {
                        continue;
                    }
                    let first = notes.iter().find(|n| n.id == ids[0]).copied();
                    let Some(n) = first else { continue };
                    let mixed = ids.iter().any(|id| {
                        notes
                            .iter()
                            .find(|m| m.id == *id)
                            .is_some_and(|m| m.cond != n.cond || m.chance != n.chance)
                    });
                    let sel = ids.iter().any(|id| self.pr.sel.contains(id));
                    let txt = if mixed {
                        "mixed".to_string()
                    } else {
                        chip(n.cond, n.chance)
                    };
                    let col = if sel {
                        sel_note_colours(&t, 1.0).0
                    } else {
                        base
                    };
                    if txt.is_empty() {
                        // always plays: a quiet dot
                        p.circle_stroke(
                            Pos2::new(x + 4.0, vel_r.center().y),
                            3.0,
                            Stroke::new(1.0, col.gamma_multiply(0.7)),
                        );
                        continue;
                    }
                    let font = crate::theme::prop(crate::theme::ts::TINY);
                    let g = p.layout_no_wrap(txt, font, t.ink);
                    // stagger chips that would overlap the one before
                    let y = if x < last_x {
                        vel_r.top() + 24.0
                    } else {
                        vel_r.top() + 5.0
                    };
                    let r = Rect::from_min_size(
                        Pos2::new(x, y),
                        egui::vec2(g.size().x + 8.0, g.size().y + 2.0),
                    );
                    last_x = r.right() + 2.0;
                    p.rect_filled(r, 4.0, col.gamma_multiply(if sel { 0.9 } else { 0.55 }));
                    p.rect_stroke(r, 4.0, Stroke::new(1.0, col), egui::StrokeKind::Inside);
                    p.galley(r.min + egui::vec2(4.0, 1.0), g, t.ink);
                }
                true
            }
        }
    }

    /// the lane's mouse (false: the velocity lane handles it)
    pub(super) fn pr_lane_input(
        &mut self,
        resp: &egui::Response,
        vel_r: Rect,
        notes: &[RNote],
        target: Target,
        src: &Source,
    ) -> bool {
        match self.pr.lane {
            Lane::Vel => false,
            Lane::Chance => {
                if resp.is_pointer_button_down_on() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let v = ((vel_r.bottom() - 2.0 - pos.y) / (vel_r.height() - 4.0))
                            .clamp(0.0, 1.0);
                        // 5 % steps; the top is "always"
                        let pc = ((v * 20.0).round() * 5.0) as u8;
                        let chance = if pc >= 100 { 0 } else { pc.max(1) };
                        let ids = near(&self.pr, notes, pos.x);
                        if !ids.is_empty() {
                            let mut s2 = self.pr_read(target).unwrap_or(src.clone());
                            if let Source::Notes(d) = &mut s2 {
                                for id in &ids {
                                    if let Some(n) = d.notes.get_mut(*id) {
                                        n.chance = chance;
                                    }
                                }
                            }
                            self.pr_write(target, s2);
                            self.status = if chance == 0 {
                                "Chance 100 % (always plays)".into()
                            } else {
                                format!("Chance {chance} % (the same passes play every time)")
                            };
                        }
                    }
                }
                true
            }
            Lane::Cond => {
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let mut ids = near(&self.pr, notes, pos.x);
                        if ids.is_empty() {
                            // on a chip (they run right of their note)
                            ids = groups(notes, &self.pr.sel)
                                .into_iter()
                                .rev()
                                .find(|(st, _)| {
                                    let x = self.pr.x_of(*st);
                                    x <= pos.x && pos.x - x <= 48.0
                                })
                                .map(|(_, ids)| ids)
                                .unwrap_or_default();
                        }
                        if !ids.is_empty() {
                            self.pr.cond_pop = Some(CondPop {
                                target,
                                ids,
                                at: Pos2::new(pos.x, vel_r.top()),
                            });
                        }
                    }
                }
                true
            }
        }
    }

    /// set the live Fill (Conditions lane / popover): live playback treats
    /// every pass as a fill while it is on
    pub(super) fn pr_set_fill(&mut self, on: bool) {
        self.pr.fill = on;
        self.with(|s| {
            s.engine.fill_live = on;
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.status = if on {
            "Fill on: notes set to \"Only in a fill\" play now, \"Not in a fill\" notes rest".into()
        } else {
            "Fill off".into()
        };
    }

    /// apply `f` to the popover's notes (one undo step)
    fn pr_cond_edit(&mut self, pop: &CondPop, f: impl Fn(&mut tl::Note)) {
        let Some(mut s2) = self.pr_read(pop.target) else {
            return;
        };
        self.snapshot();
        if let Source::Notes(d) = &mut s2 {
            for id in &pop.ids {
                if let Some(n) = d.notes.get_mut(*id) {
                    f(n);
                }
            }
        }
        self.pr_write(pop.target, s2);
    }

    /// the "Play this note…" popover
    pub(super) fn pr_cond_popover(&mut self, ctx: &egui::Context) {
        let Some(pop) = self.pr.cond_pop.clone() else {
            return;
        };
        let t = th();
        let Some(Source::Notes(d)) = self.pr_read(pop.target) else {
            self.pr.cond_pop = None;
            return;
        };
        let Some(n0) = pop.ids.iter().find_map(|i| d.notes.get(*i)).cloned() else {
            self.pr.cond_pop = None;
            return;
        };
        let cur = Choice::of(&n0);
        let (mut a, mut b) = match n0.cond {
            Cond::Every { a, b } => (a, b),
            _ => (1, 4),
        };
        let mut chance = if n0.chance == 0 { 100 } else { n0.chance };
        let mut pick: Option<Choice> = None;
        let mut every_changed = false;
        let mut chance_changed = false;
        let mut fill_click = false;
        let mut close = false;
        let h = 330.0;
        let at = Pos2::new(
            pop.at.x.min(ctx.content_rect().right() - 300.0),
            (pop.at.y - h).max(4.0),
        );
        let area = egui::Area::new(egui::Id::unique("pr_cond_pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_width(270.0);
                    let title = if pop.ids.len() > 1 {
                        format!("Play these {} notes…", pop.ids.len())
                    } else {
                        "Play this note…".to_string()
                    };
                    ui.label(egui::RichText::new(title).strong().color(t.ink));
                    for c in Choice::ALL {
                        ui.horizontal(|ui| {
                            if ui
                                .radio(cur == c, c.label())
                                .on_hover_text(c.tip())
                                .clicked()
                            {
                                pick = Some(c);
                            }
                            if c == Choice::Every {
                                let r1 = ui
                                    .add(egui::DragValue::new(&mut a).range(1..=b.max(1)))
                                    .on_hover_text("which pass");
                                ui.label("of");
                                let r2 = ui
                                    .add(egui::DragValue::new(&mut b).range(2..=16))
                                    .on_hover_text("out of how many passes");
                                ui.label("passes");
                                if r1.changed() || r2.changed() {
                                    a = a.min(b);
                                    every_changed = true;
                                }
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Chance");
                        let r = ui
                            .add(egui::Slider::new(&mut chance, 1..=100).suffix(" %"))
                            .on_hover_text(
                                "How often the note plays on the passes above. 100 % = always. \
                                 The same passes play every time (export too).",
                            );
                        chance_changed = r.changed();
                    });
                    // pass preview: 8 passes, the 8th shown as a fill
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "The first 8 passes (pass 8 is the clip's last: a fill)",
                        )
                        .color(t.ink2),
                    );
                    let (r, _) = ui.allocate_exact_size(egui::vec2(260.0, 30.0), Sense::hover());
                    let p = ui.painter_at(r);
                    for k in 0..8u32 {
                        let c = Pos2::new(r.left() + 16.0 + k as f32 * 32.0, r.top() + 9.0);
                        let fill = k == 7 || self.pr.fill;
                        let plays = n0.cond.plays(k, fill);
                        let col = if plays { t.accent } else { t.dot_off };
                        if plays && n0.chance > 0 {
                            p.circle_stroke(c, 6.0, Stroke::new(2.0, col));
                            p.circle_filled(c, 2.5, col);
                        } else if plays {
                            p.circle_filled(c, 6.0, col);
                        } else {
                            p.circle_stroke(c, 6.0, Stroke::new(1.0, col));
                        }
                        p.text(
                            Pos2::new(c.x, r.bottom() - 6.0),
                            egui::Align2::CENTER_CENTER,
                            if k == 7 {
                                "8 F".to_string()
                            } else {
                                (k + 1).to_string()
                            },
                            crate::theme::prop(crate::theme::ts::TINY),
                            t.ink2,
                        );
                    }
                    ui.horizontal(|ui| {
                        let lbl = if self.pr.fill { "Fill: ON" } else { "Fill" };
                        if ui
                            .selectable_label(self.pr.fill, lbl)
                            .on_hover_text(
                                "Turn Fill on while playing to hear the fill notes now (every \
                                 pass counts as a fill). Exports never use it.",
                            )
                            .clicked()
                        {
                            fill_click = true;
                        }
                        if ui.button("Done").clicked() {
                            close = true;
                        }
                    });
                });
            });
        if let Some(c) = pick {
            self.pr_cond_edit(&pop, |n| c.apply(n, (a, b)));
        } else if every_changed {
            self.pr_cond_edit(&pop, |n| n.cond = Cond::Every { a, b });
        } else if chance_changed {
            let ch = if chance >= 100 { 0 } else { chance };
            self.pr_cond_edit(&pop, |n| n.chance = ch);
        }
        if fill_click {
            let on = !self.pr.fill;
            self.pr_set_fill(on);
        }
        let outside = ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
        // the click that opened it lands in the lane, not "outside"
        let in_lane = ctx
            .input(|i| i.pointer.press_origin())
            .is_some_and(|p| self.pr.vel_rect.contains(p));
        if close || (outside && !in_lane) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pr.cond_pop = None;
        }
    }
}

/// notes grouped by start, each group's ids with the selected ones first
fn groups(notes: &[RNote], sel: &[usize]) -> Vec<(Tick, Vec<usize>)> {
    let mut v: Vec<&RNote> = notes.iter().collect();
    v.sort_by_key(|n| (n.start, !sel.contains(&n.id), n.id));
    let mut out: Vec<(Tick, Vec<usize>)> = vec![];
    for n in v {
        match out.last_mut() {
            Some((s, ids)) if *s == n.start => ids.push(n.id),
            _ => out.push((n.start, vec![n.id])),
        }
    }
    out
}

/// the notes at screen x (as the velocity lane: the selected ones where
/// some are selected)
fn near(pr: &super::pianoview::PianoUi, notes: &[RNote], x: f32) -> Vec<usize> {
    let at: Vec<&RNote> = notes
        .iter()
        .filter(|n| (pr.x_of(n.start) + 1.0 - x).abs() <= 6.0)
        .collect();
    let sel: Vec<usize> = at
        .iter()
        .filter(|n| pr.sel.contains(&n.id))
        .map(|n| n.id)
        .collect();
    if sel.is_empty() {
        at.iter().map(|n| n.id).collect()
    } else {
        sel
    }
}
