//! Clip Variations (docs/ROUND_ROBIN.md): the piano roll's take tabs
//! (A–D and +), the Variations popover (order, seed, New, a preview of the
//! first 16 passes) and the clip pie's Variations wedge. The take shown is
//! the one you edit; which take plays is decided per loop pass from the
//! clip's seed, so playback, seeking and export always agree.
use super::pianoview::{Source, Target};
use super::*;
use crate::timeline::{self as tl, rr::Order};

/// the Variations popover: on note data `data`, played with `clip_seed`
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VarPop {
    pub data: usize,
    pub clip_seed: u32,
    pub at: Pos2,
}

/// a colour per take (A teal, B blue, C orange, D violet)
pub(crate) fn take_colour(i: usize) -> Color32 {
    const C: [u32; 4] = [0x4F9A96, 0x5A86B4, 0xC0864A, 0x7A6CB4];
    let c = C[i % 4];
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// what the passes play: the take letter per pass, for `n` passes
pub(crate) fn preview(d: &tl::NoteClipData, clip_seed: u32, n: u32) -> Vec<usize> {
    match &d.var {
        Some(v) => (0..n).map(|k| v.pick(clip_seed, k)).collect(),
        None => vec![0; n as usize],
    }
}

/// a new seed after `s` (the New button; the same steps every time)
pub(crate) fn next_seed(s: u32) -> u32 {
    tl::ClipRng::new(s ^ 0x4E45_5753, 1).next_u32()
}

impl App {
    /// the seed of the clip that owns note data `data` (the roll's clip,
    /// else the launcher clip shown)
    pub(super) fn var_clip_seed(&self, clip: Option<u32>) -> u32 {
        let id = clip.or(self.pr.launch);
        self.with(|s| {
            let t = s.song.timeline.as_deref()?;
            id.and_then(|id| t.any_clip(id)).map(|c| c.seed)
        })
        .unwrap_or(0)
    }

    /// change note data `data`'s variations as one undo step
    pub(super) fn var_edit(
        &mut self,
        data: usize,
        what: &str,
        f: impl FnOnce(&mut tl::NoteClipData),
    ) {
        self.snapshot();
        self.with(|s| {
            if let Some(x) = s
                .song
                .timeline
                .as_deref_mut()
                .and_then(|t| t.notes.get_mut(data))
            {
                f(x.make_mut());
            }
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.pr.sel.clear();
        self.dirty = true;
        self.status = what.to_string();
    }

    /// add a take (a copy of the one shown) and show it
    pub(super) fn var_add(&mut self, data: usize, clip_seed: u32) {
        let full = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.notes.get(data))
                .is_some_and(|d| d.var_count() >= tl::rr::MAX_VARS)
        });
        if full {
            self.status = "Variations: four at most (A–D)".into();
            return;
        }
        let seed = next_seed(clip_seed ^ data as u32);
        self.var_edit(data, "", |d| {
            d.var_add(seed);
        });
        let n = self.var_count(data);
        self.status = format!(
            "Variation {} added: a copy of the one before, change it to taste · the clip picks a \
             take each time it loops",
            tl::rr::var_letter(n - 1)
        );
    }

    pub(super) fn var_count(&self, data: usize) -> usize {
        self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.notes.get(data))
                .map_or(1, |d| d.var_count())
        })
    }

    /// the take tabs in the roll's toolbar (note clips only)
    pub(super) fn pr_var_tabs(&mut self, ui: &mut egui::Ui, target: Target, src: &Source) {
        let (Target::Notes { data, clip }, Source::Notes(d)) = (target, src) else {
            return;
        };
        let t = th();
        let n = d.var_count();
        let cur = d.var.as_ref().map_or(0, |v| v.cur as usize);
        let seed = self.var_clip_seed(clip);
        if n > 1 {
            for i in 0..n {
                let letter = tl::rr::var_letter(i).to_string();
                let on = i == cur;
                let txt = egui::RichText::new(&letter).color(if on {
                    Color32::WHITE
                } else {
                    take_colour(i)
                });
                let b = egui::Button::new(txt)
                    .fill(if on { take_colour(i) } else { t.pill })
                    .min_size(egui::vec2(20.0, 18.0));
                let tip = format!(
                    "Variation {letter}: click to see and edit it · the clip plays one variation \
                     each time it loops (Variations… sets the order)"
                );
                if ui.add(b).on_hover_text(tip).clicked() && !on {
                    self.var_edit(data, &format!("Editing variation {letter}"), |d| {
                        d.var_select(i);
                    });
                }
            }
        }
        if n < tl::rr::MAX_VARS {
            let (lbl, tip) = if n > 1 {
                (
                    "+Var",
                    "Add variation: a copy of this one to change, so the loop can play it instead",
                )
            } else {
                (
                    "+Var",
                    "Add variation: a second take of this clip (a copy to change). Each time \
                     the clip loops it plays one take, so a loop stays fresh",
                )
            };
            if ui.small_button(lbl).on_hover_text(tip).clicked() {
                self.var_add(data, seed);
            }
        }
        if n > 1 {
            let r = ui.small_button("Order…").on_hover_text(
                "Variations: which take plays on each loop pass, the seed, and a preview",
            );
            if r.clicked() {
                self.pr.var_pop = Some(VarPop {
                    data,
                    clip_seed: seed,
                    at: r.rect.left_bottom(),
                });
            }
        }
    }

    /// the Variations popover
    pub(super) fn var_popover(&mut self, ctx: &egui::Context) {
        let Some(pop) = self.pr.var_pop.clone() else {
            return;
        };
        let t = th();
        let Some(d) = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.notes.get(pop.data))
                .map(|d| (**d).clone())
        }) else {
            self.pr.var_pop = None;
            return;
        };
        let Some(v) = d.var.clone() else {
            self.pr.var_pop = None;
            return;
        };
        let n = v.count();
        let mut order = v.order;
        let mut seed = v.seed;
        let mut pick_tab: Option<usize> = None;
        let mut add = false;
        let mut remove = false;
        let mut new_seed = false;
        let mut close = false;
        let w = 300.0;
        let at = Pos2::new(
            pop.at.x.min(ctx.content_rect().right() - w - 20.0).max(4.0),
            pop.at.y.min(ctx.content_rect().bottom() - 300.0).max(4.0),
        );
        let area = egui::Area::new(egui::Id::unique("var_pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_width(w);
                    ui.label(egui::RichText::new("Variations").strong().color(t.ink));
                    ui.label(
                        egui::RichText::new(
                            "Each time the clip loops, it plays one of these takes.",
                        )
                        .color(t.ink2),
                    );
                    ui.horizontal(|ui| {
                        for i in 0..n {
                            let on = i == v.cur as usize;
                            let l = tl::rr::var_letter(i).to_string();
                            if ui
                                .selectable_label(
                                    on,
                                    egui::RichText::new(&l).color(take_colour(i)).strong(),
                                )
                                .on_hover_text(format!("show variation {l} in the piano roll"))
                                .clicked()
                            {
                                pick_tab = Some(i);
                            }
                        }
                        if n < tl::rr::MAX_VARS
                            && ui
                                .button("Add variation")
                                .on_hover_text("a copy of the one shown, to change")
                                .clicked()
                        {
                            add = true;
                        }
                    });
                    ui.label("Order");
                    ui.horizontal_wrapped(|ui| {
                        for o in Order::ALL {
                            let on = order.same_kind(o);
                            if ui.radio(on, o.label()).on_hover_text(o.tip()).clicked() && !on {
                                order = match o {
                                    Order::EveryN { n: e, .. } => Order::EveryN {
                                        n: e,
                                        which: (n - 1).min(1) as u8,
                                    },
                                    o => o,
                                };
                            }
                        }
                    });
                    if let Order::EveryN {
                        n: mut every,
                        mut which,
                    } = order
                    {
                        ui.horizontal(|ui| {
                            ui.label("Every");
                            let r1 = ui
                                .add(egui::DragValue::new(&mut every).range(2..=16))
                                .on_hover_text("how many passes apart");
                            ui.label("passes play");
                            let mut w1 = which + 1;
                            let r2 = ui
                                .add(
                                    egui::DragValue::new(&mut w1)
                                        .range(2..=n as u8)
                                        .custom_formatter(|x, _| {
                                            tl::rr::var_letter((x as usize).saturating_sub(1))
                                                .to_string()
                                        }),
                                )
                                .on_hover_text("which take plays then (A plays the rest)");
                            if r1.changed() || r2.changed() {
                                which = w1.saturating_sub(1);
                                order = Order::EveryN { n: every, which };
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Seed");
                        ui.add(egui::DragValue::new(&mut seed).speed(1.0))
                            .on_hover_text(
                            "The random orders come from this number: the same seed always plays \
                             the same takes (exports too)",
                        );
                        if ui
                            .button("New")
                            .on_hover_text("a new seed: another random order")
                            .clicked()
                        {
                            new_seed = true;
                        }
                    });
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("The first 16 passes").color(t.ink2));
                    let mut dv = d.clone();
                    if let Some(x) = dv.var.as_mut() {
                        x.order = order;
                        x.seed = seed;
                    }
                    let seq = preview(&dv, pop.clip_seed, 16);
                    let (r, _) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::hover());
                    let p = ui.painter_at(r);
                    let cw = w / 16.0;
                    for (k, i) in seq.iter().enumerate() {
                        let cell = Rect::from_min_size(
                            Pos2::new(r.left() + k as f32 * cw, r.top()),
                            egui::vec2(cw - 2.0, r.height()),
                        );
                        p.rect_filled(cell, 3.0, take_colour(*i));
                        p.text(
                            cell.center(),
                            egui::Align2::CENTER_CENTER,
                            tl::rr::var_letter(*i),
                            crate::theme::prop(crate::theme::ts::TINY),
                            Color32::WHITE,
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .button(format!("Remove {}", tl::rr::var_letter(v.cur as usize)))
                            .on_hover_text("delete the take shown (one left turns Variations off)")
                            .clicked()
                        {
                            remove = true;
                        }
                        if ui.button("Done").clicked() {
                            close = true;
                        }
                    });
                });
            });
        let data = pop.data;
        if let Some(i) = pick_tab {
            self.var_edit(
                data,
                &format!("Editing variation {}", tl::rr::var_letter(i)),
                |d| {
                    d.var_select(i);
                },
            );
        } else if add {
            self.var_add(data, pop.clip_seed);
        } else if remove {
            let l = tl::rr::var_letter(v.cur as usize);
            self.var_edit(data, &format!("Variation {l} removed"), |d| {
                d.var_remove(d.var.as_ref().map_or(0, |v| v.cur as usize));
            });
        } else if new_seed || seed != v.seed || order != v.order {
            let s2 = if new_seed { next_seed(seed) } else { seed };
            self.var_edit(data, &format!("Variations: {}", order.label()), |d| {
                if let Some(x) = d.var.as_mut() {
                    x.order = order;
                    x.seed = s2;
                }
            });
        }
        let outside = ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
        let opened_now = ctx
            .input(|i| i.pointer.press_origin())
            .is_some_and(|p| (p - pop.at).length() < 40.0);
        if close || (outside && !opened_now) || self.var_count(data) <= 1 {
            self.pr.var_pop = None;
        }
    }
}
