//! Follow actions in the Clip Arranger (docs/ROUND_ROBIN.md): the
//! "Follow action…" popover of a launcher clip (after N loops, chance %
//! for one action, else another, a preview of where the track goes) and
//! Capture to timeline, which writes that run into the arrangement.
use super::*;
use crate::timeline::{self as tl, follow::Act, follow::Follow};

/// the Follow action popover on launcher clip `id`
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FollowPop {
    pub id: u32,
    pub at: Pos2,
    /// Capture to timeline: how many bars
    pub bars: u16,
}

impl App {
    /// open the popover on launcher clip `id` near `at`
    pub(super) fn follow_open(&mut self, id: u32, at: Pos2) {
        self.pr.follow_pop = Some(FollowPop { id, at, bars: 16 });
    }

    /// set (or clear) launcher clip `id`'s follow action: one undo step
    pub(super) fn follow_set(&mut self, id: u32, f: Option<Follow>) {
        let cur = self.with(|s| Some(s.song.timeline.as_deref()?.launch_clip(id)?.follow));
        if cur.is_some_and(|c| c != f) {
            self.snapshot();
            self.with(|s| {
                let t = s.song.timeline.as_deref_mut()?;
                let l = t.launcher.iter_mut().find(|l| l.clip.id == id)?;
                l.follow = f;
                Some(())
            });
            self.dirty = true;
            self.status = match f {
                Some(f) => format!("Follow action: {}", f.describe()),
                None => "Follow action off: the clip loops until stopped".into(),
            };
        }
    }

    /// Capture to timeline: what launching scene row `scene` plays for
    /// `bars` bars, placed after the arrangement's last clip
    pub(super) fn follow_capture(&mut self, scene: usize, bars: u16) {
        self.snapshot();
        let ids = self.with(|s| {
            let t = s.song.timeline.as_deref_mut()?;
            let bar = t.sig_at(0).bar_ticks().max(1);
            let end = t.clips.iter().map(|c| c.end()).max().unwrap_or(0);
            let at = (end + bar - 1) / bar * bar;
            let ids = tl::follow::capture(t, scene, at, bars.max(1) as tl::Tick * bar);
            (!ids.is_empty()).then_some((ids, at / bar))
        });
        match ids {
            Some((ids, b)) => {
                self.dirty = true;
                self.status = format!(
                    "Captured {bars} bars of scene {} to the timeline from bar {} ({} clips, \
                     linked; Ctrl+Z undoes)",
                    scene + 1,
                    b + 1,
                    ids.len()
                );
                self.tlv.sels = ids;
                self.tlv.sel = None;
            }
            None => self.status = "Capture to timeline: this scene has no note clips".into(),
        }
    }

    /// the Follow action popover
    pub(super) fn follow_popover(&mut self, ctx: &egui::Context) {
        let Some(mut pop) = self.pr.follow_pop.clone() else {
            return;
        };
        let t = th();
        let Some((cur, scene, plan, names)) = self.with(|s| {
            let tlv = s.song.timeline.as_deref()?;
            let l = tlv.launch_clip(pop.id)?;
            let names: Vec<(u32, String)> = tlv
                .launcher
                .iter()
                .filter(|x| x.clip.track == l.clip.track)
                .map(|x| {
                    let n = tlv
                        .scenes
                        .get(x.scene as usize)
                        .map(|s| s.name.clone())
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| format!("Scene {}", x.scene + 1));
                    (x.clip.id, n)
                })
                .collect();
            Some((l.follow, l.scene as usize, tl::launch_plan(tlv), names))
        }) else {
            self.pr.follow_pop = None;
            return;
        };
        let mut on = cur.is_some();
        let mut f = cur.unwrap_or_default();
        let mut close = false;
        let mut capture = false;
        let w = 320.0;
        let at = Pos2::new(
            pop.at.x.min(ctx.content_rect().right() - w - 20.0).max(4.0),
            pop.at.y.min(ctx.content_rect().bottom() - 330.0).max(4.0),
        );
        let area = egui::Area::new(egui::Id::unique("follow_pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_width(w);
                    ui.label(egui::RichText::new("Follow action").strong().color(t.ink));
                    ui.label(
                        egui::RichText::new("When its loops are up, this clip moves on by itself.")
                            .color(t.ink2),
                    );
                    ui.checkbox(&mut on, "On").on_hover_text(
                        "Off: the clip loops until you stop it (the usual). On: after the loops \
                         below, the track plays another clip or stops",
                    );
                    ui.add_enabled_ui(on, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("After");
                            ui.add(egui::DragValue::new(&mut f.after).range(1..=64))
                                .on_hover_text("how many times the clip plays first");
                            ui.label(if f.after == 1 { "loop:" } else { "loops:" });
                        });
                        let act_combo = |ui: &mut egui::Ui, id: &str, a: &mut Act| {
                            egui::ComboBox::from_id_salt(id)
                                .selected_text(a.label())
                                .width(80.0)
                                .show_ui(ui, |ui| {
                                    for x in Act::ALL {
                                        ui.selectable_value(a, x, x.label()).on_hover_text(x.tip());
                                    }
                                })
                                .response
                                .on_hover_text(a.tip());
                        };
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::DragValue::new(&mut f.chance)
                                    .range(0..=100)
                                    .suffix(" %"),
                            )
                            .on_hover_text(
                                "how often the first action is taken (the rest of the time: the \
                                 second). Decided from the clip's seed, so it is the same every \
                                 play and export",
                            );
                            act_combo(ui, "follow_a", &mut f.a);
                            ui.label("else");
                            act_combo(ui, "follow_b", &mut f.b);
                        });
                        ui.label(egui::RichText::new(f.describe()).color(t.ink2));
                        // where the track goes: the same run the engine plays
                        let mut p2 = plan.clone();
                        if let Some(x) = p2.iter_mut().find(|x| x.id == pop.id) {
                            x.follow = Some(f);
                        }
                        let bar = p2
                            .iter()
                            .find(|x| x.id == pop.id)
                            .map_or(1, |x| x.loop_len.max(1));
                        let run = tl::follow::run(&p2, pop.id, 64 * bar);
                        let name = |id: u32| {
                            names
                                .iter()
                                .find(|n| n.0 == id)
                                .map_or("?".to_string(), |n| n.1.clone())
                        };
                        let mut line: Vec<String> = run
                            .iter()
                            .take(6)
                            .map(|(id, d)| {
                                let len = p2
                                    .iter()
                                    .find(|x| x.id == *id)
                                    .map_or(1, |x| x.loop_len.max(1));
                                format!("{} ×{}", name(*id), d / len)
                            })
                            .collect();
                        let total: tl::Tick = run.iter().map(|r| r.1).sum();
                        if run.len() > 6 {
                            line.push("…".into());
                        } else if total < 64 * bar {
                            line.push("stop".into());
                        }
                        ui.label(egui::RichText::new("From here").color(t.ink2));
                        ui.label(egui::RichText::new(line.join(" › ")).color(t.ink))
                            .on_hover_text(
                                "what this track plays when you launch this clip: each clip and \
                                 how many loops, worked out from the seeds",
                            );
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .button("Capture to timeline")
                            .on_hover_text(format!(
                                "write what launching scene {} plays into the arrangement, after \
                                 its last clip: every track's clips and follow actions, as linked \
                                 clips with the same seeds (so it sounds the same)",
                                scene + 1
                            ))
                            .clicked()
                        {
                            capture = true;
                        }
                        ui.add(egui::DragValue::new(&mut pop.bars).range(1..=256))
                            .on_hover_text("how many bars to capture");
                        ui.label("bars");
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Done").clicked() {
                            close = true;
                        }
                    });
                });
            });
        self.follow_set(pop.id, on.then_some(f));
        if capture {
            self.follow_capture(scene, pop.bars);
        }
        let outside = ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
        let opened_now = ctx
            .input(|i| i.pointer.press_origin())
            .is_some_and(|p| (p - pop.at).length() < 40.0);
        let popup_open = egui::Popup::is_any_open(ctx);
        if close || capture || (outside && !opened_now && !popup_open) {
            self.pr.follow_pop = None;
        } else {
            self.pr.follow_pop = Some(pop);
        }
    }
}
