//! The track pie's Add › Automation lane menu: the track's lanes (show /
//! hide them under the track, switch each on or off) and the parameter
//! picker for a new lane. Everything goes through the existing automation
//! ops (`AutoOp::Toggle` / `Enable` / `AddLane`, one undo step each), so a
//! lane made here is the same as one from the track's "+ lane" strip.
use super::lanes::{lane_menu_ui, LaneMenu};
use super::timeline::autolanes::AutoOp;
use super::*;

pub(crate) struct TrackAuto {
    pub(crate) track: usize,
    pub(crate) at: Pos2,
    menu: LaneMenu,
    /// frames since it opened (the click that chose it is not "outside")
    frames: u32,
}

impl App {
    /// open the menu for track `tr` at `at` (top-left, kept on screen)
    pub(crate) fn track_auto_open(&mut self, tr: usize, at: Pos2) {
        if self.with(|s| s.song.timeline.is_none()) {
            self.status = "Automation lanes on a track need timeline mode".into();
            return;
        }
        self.track_auto = Some(TrackAuto {
            track: tr,
            at,
            menu: self.lane_menu(),
            frames: 0,
        });
    }

    /// the track's lanes: (target id, label, on)
    pub(crate) fn track_lanes(&self, tr: usize) -> Vec<(String, String, bool)> {
        self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.tracks.get(tr))
                .map(|t| {
                    t.auto
                        .iter()
                        .map(|l| {
                            let label = if l.label.is_empty() {
                                l.target.clone()
                            } else {
                                l.label.clone()
                            };
                            (l.target.clone(), label, l.enabled)
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    /// the lanes under track `tr` are showing
    pub(crate) fn track_lanes_shown(&self, tr: usize) -> bool {
        self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .is_some_and(|t| tr < 64 && t.view.auto_open >> tr & 1 == 1)
        })
    }

    /// draw and handle the menu (every frame while open)
    pub(crate) fn track_auto_menu(&mut self, ctx: &egui::Context) {
        let Some(mut m) = self.track_auto.take() else {
            return;
        };
        m.frames += 1;
        let tr = m.track;
        let (n, name) = self.with(|s| {
            (
                s.song.channels,
                s.song.channel_names.get(tr).cloned().unwrap_or_default(),
            )
        });
        if tr >= n || self.with(|s| s.song.timeline.is_none()) {
            return;
        }
        let lanes = self.track_lanes(tr);
        let shown = self.track_lanes_shown(tr);
        let mut ops: Vec<AutoOp> = vec![];
        let mut close = false;
        let screen = ctx.content_rect();
        let at = Pos2::new(
            m.at.x.clamp(
                screen.left() + 4.0,
                (screen.right() - 330.0).max(screen.left() + 4.0),
            ),
            m.at.y.clamp(
                screen.top() + 4.0,
                (screen.bottom() - 440.0).max(screen.top() + 4.0),
            ),
        );
        let area = egui::Area::new(egui::Id::unique("track_auto_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(300.0);
                    ui.set_max_width(320.0);
                    ui.label(
                        egui::RichText::new(format!("AUTOMATION · {:02} {name}", tr + 1))
                            .small()
                            .color(th().ink2),
                    );
                    let what = if shown {
                        "Hide this track's lanes"
                    } else {
                        "Show this track's lanes"
                    };
                    if ui
                        .add_enabled(!lanes.is_empty(), egui::Button::new(what))
                        .on_hover_text(
                            "the lane rows under the track on the Timeline (the A button)",
                        )
                        .on_disabled_hover_text("no lanes yet: pick a parameter below")
                        .clicked()
                    {
                        ops.push(AutoOp::Toggle(tr));
                    }
                    if !lanes.is_empty() {
                        ui.label(
                            egui::RichText::new("Lanes (tick: plays)")
                                .small()
                                .color(th().ink2),
                        );
                        for (tg, label, on) in &lanes {
                            let mut v = *on;
                            if ui
                                .checkbox(&mut v, label.as_str())
                                .on_hover_text(format!("{tg}\noff: kept but not played"))
                                .changed()
                            {
                                ops.push(AutoOp::Enable(tr, tg.clone(), v));
                            }
                        }
                    }
                    ui.separator();
                    // the picker is the same submenu as the lane strip's "+ lane"
                    let mut add = None;
                    ui.menu_button("New lane: pick a parameter ›", |ui| {
                        lane_menu_ui(ui, &m.menu, true, &mut add);
                    })
                    .response
                    .on_hover_text("a lane on this track for any instrument or effect parameter");
                    if let Some(tg) = add {
                        ops.push(AutoOp::AddLane(tr, tg));
                        close = true;
                    }
                });
            });
        let clicked_out = m.frames > 2
            && ctx.input(|i| i.pointer.any_click())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| !area.response.rect.expand(6.0).contains(p))
            // a submenu of the picker is its own popup: not "outside"
            && !ctx.any_popup_open();
        // keys reach the app through `pending` (raw_input_hook): Esc is ours
        let is_esc = |e: &egui::Event| {
            matches!(
                e,
                egui::Event::Key {
                    key: egui::Key::Escape,
                    ..
                }
            )
        };
        let esc = self.pending.iter().any(is_esc);
        self.pending.retain(|e| !is_esc(e));
        for op in ops {
            self.auto_op(op);
        }
        if !(close || clicked_out || esc) {
            self.track_auto = Some(m);
        }
    }
}
