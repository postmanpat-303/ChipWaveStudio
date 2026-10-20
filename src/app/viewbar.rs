//! The view tab bar along the bottom (0.2.4 "inspired" layout, Del's pick
//! F): small transport (to start, play song, stop) with the position and
//! tempo, then one tab per ChipWave view: TIMELINE · TRACKER (pattern
//! editor) · ARRANGER, and the lower-panel tabs PIANO · INST · AUTO, then
//! MIX · FX · SAMPLE. Every tab calls the same switch as the toolbar /
//! View menu / shortcuts, so the existing ways keep working. On the right:
//! the Inspector (I) and Browser (Ctrl+B) toggles. View > View tab bar
//! hides it.
use super::topbar::{hdr_dim, hdr_text};
use super::*;

pub(crate) const VIEWBAR_H: f32 = 30.0;

/// a tab of the bar
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VTab {
    Timeline,
    Tracker,
    Arranger,
    Piano,
    Devices,
    Inst,
    Auto,
    Mix,
    Fx,
    Sample,
}

/// the view tabs: the DAW views first, then ORDER and TRACKER at the far
/// right after a divider
pub(crate) const VTABS: [(VTab, &str, &str); 10] = [
    (
        VTab::Timeline,
        "TIMELINE",
        "tracks and clips over time (Ctrl+T)",
    ),
    (
        VTab::Piano,
        "PIANO",
        "the piano roll in the lower panel (double-click a clip)",
    ),
    (
        VTab::Devices,
        "DEVICES",
        "the selected track's instrument and effects in the lower panel (Ctrl+7)",
    ),
    (
        VTab::Inst,
        "INST",
        "the instrument editor in the lower panel (Ctrl+I)",
    ),
    (VTab::Auto, "AUTO", "automation lanes in the lower panel"),
    (VTab::Mix, "MIX", "the mixer (Ctrl+3)"),
    (VTab::Fx, "FX", "the instrument FX chain (Ctrl+4)"),
    (VTab::Sample, "SAMPLE", "the sample editor (Ctrl+5)"),
    (
        VTab::Arranger,
        "ORDER",
        "the Order list: the tracker song's blocks in order (Ctrl+2)",
    ),
    (VTab::Tracker, "TRACKER", "the Tracker (Ctrl+1)"),
];

#[derive(Clone, Copy)]
enum Tr {
    Start,
    Play,
    Stop,
}

fn transport_button(ui: &mut egui::Ui, ic: Tr, on: bool, col: Color32, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(26.0, 22.0), Sense::click());
    let resp = resp.on_hover_text(tip);
    let p = ui.painter();
    let v = ui.style().interact_selectable(&resp, on);
    p.rect(r, 4, v.weak_bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
    if theme::polished() {
        theme::raise(p, r, 4.0, resp.is_pointer_button_down_on() || on);
    }
    let c = r.center();
    match ic {
        Tr::Start => {
            p.rect_filled(
                Rect::from_center_size(c + egui::vec2(-5.0, 0.0), egui::vec2(2.0, 10.0)),
                0,
                col,
            );
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + egui::vec2(5.0, -5.0),
                    c + egui::vec2(5.0, 5.0),
                    c + egui::vec2(-3.0, 0.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Tr::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + egui::vec2(-4.0, -6.0),
                    c + egui::vec2(6.0, 0.0),
                    c + egui::vec2(-4.0, 6.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Tr::Stop => {
            p.rect_filled(Rect::from_center_size(c, egui::vec2(9.0, 9.0)), 1, col);
        }
    }
    resp.clicked()
}

impl App {
    /// which tab is lit now
    pub(crate) fn vtab_current(&self) -> Option<VTab> {
        let lower = matches!(self.view, View::Pattern | View::Timeline) && self.lower_visible();
        Some(match self.view {
            _ if lower && self.extra.lower_dev => VTab::Devices,
            _ if lower && self.extra.lower_inst => VTab::Inst,
            _ if lower && self.pr.open => VTab::Piano,
            View::Timeline => VTab::Timeline,
            View::Pattern => VTab::Tracker,
            View::Arranger => VTab::Arranger,
            View::Mixer => VTab::Mix,
            View::Fx => VTab::Fx,
            View::Sample => VTab::Sample,
            View::Settings => return None,
        })
    }

    /// the tab drawn lit: the view you are in (tests)
    #[allow(dead_code)]
    pub(crate) fn vtab_lit(&self) -> Option<VTab> {
        self.vtab_main().or(self.vtab_current())
    }

    /// the main view's tab: lit, with the lower panel's tab tinted under it
    fn vtab_main(&self) -> Option<VTab> {
        match self.view {
            View::Timeline => Some(VTab::Timeline),
            View::Pattern => Some(VTab::Tracker),
            _ => None,
        }
    }

    /// switch like the toolbar / menus do
    pub(crate) fn vtab_select(&mut self, tab: VTab) {
        // the lower-panel tabs live under the Timeline or the pattern editor
        let host = |s: &Self| {
            if matches!(s.view, View::Pattern | View::Timeline) {
                s.view
            } else if s.with(|x| x.song.timeline.is_some()) {
                View::Timeline
            } else {
                View::Pattern
            }
        };
        match tab {
            VTab::Timeline => self.view = View::Timeline,
            VTab::Tracker => {
                self.view = View::Pattern;
                self.arr_focus = false;
            }
            VTab::Arranger => self.view = View::Arranger,
            VTab::Mix => self.view = View::Mixer,
            VTab::Fx => self.view = View::Fx,
            VTab::Sample => self.view = View::Sample,
            VTab::Piano => {
                self.view = host(self);
                if self.view == View::Timeline && self.tlv.sel.is_some() {
                    self.timeline_open_roll();
                } else {
                    self.set_lower(3);
                }
            }
            VTab::Devices => {
                self.view = host(self);
                self.set_lower(4);
            }
            VTab::Inst => {
                self.view = host(self);
                self.show_instrument_panel();
            }
            VTab::Auto => {
                self.view = host(self);
                self.set_lower(2);
            }
        }
    }

    pub(super) fn view_bar(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let playing = self.with(|s| s.engine.playing);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(6.0);
            // ---- transport
            let tl_mode = self.with(|s| s.song.timeline.is_some());
            if transport_button(ui, Tr::Start, false, hdr_text(), "play marker to the start") {
                if tl_mode {
                    self.timeline_seek(0);
                } else {
                    self.seek_to((0, 0));
                }
            }
            if transport_button(
                ui,
                Tr::Play,
                playing,
                t.accent,
                "play the song from the play marker (Shift+Space; Space in the Timeline)",
            ) {
                if playing {
                    self.with(|s| s.engine.stop());
                }
                self.play(0);
            }
            if transport_button(
                ui,
                Tr::Stop,
                false,
                if playing { t.brick } else { hdr_dim() },
                "stop",
            ) {
                self.with(|s| s.engine.stop());
            }
            ui.add_space(6.0);
            let (pos, bpm) = self.view_bar_position(playing);
            ui.label(
                egui::RichText::new(pos)
                    .monospace()
                    .size(12.0)
                    .color(if playing { t.play } else { hdr_text() }),
            )
            .on_hover_text("playhead (stopped: the play marker)");
            ui.label(
                egui::RichText::new(format!("· {bpm:.0} BPM"))
                    .size(11.0)
                    .color(hdr_dim()),
            );
            ui.add_space(14.0);
            // ---- the default layout, one click away
            {
                let on = self.is_default_layout();
                let txt = egui::RichText::new("TIMELINE + PIANO")
                    .size(11.0)
                    .strong()
                    .color(if on { t.accent } else { hdr_text() });
                let b = egui::Button::new(txt)
                    .min_size(egui::vec2(112.0, 22.0))
                    .corner_radius(if theme::polished() { 5 } else { 2 })
                    .stroke(egui::Stroke::new(
                        1.0,
                        t.accent.gamma_multiply(if on { 0.9 } else { 0.45 }),
                    ))
                    .fill(Color32::from_black_alpha(if t.light { 20 } else { 50 }));
                if ui
                    .add(b)
                    .on_hover_text("the default layout: Timeline on top, piano roll below (Ctrl+6)")
                    .clicked()
                {
                    self.reset_layout();
                }
                ui.add_space(8.0);
            }
            // ---- view tabs
            let cur = self.vtab_current();
            let main = self.vtab_main();
            for (tab, name, tip) in VTABS {
                if tab == VTab::Arranger {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                }
                // the view you are in is lit (TIMELINE / TRACKER); the lower
                // panel's tab under it is tinted
                let (on, host_on) = match main {
                    Some(m) => (m == tab, m != tab && cur == Some(tab)),
                    None => (cur == Some(tab), false),
                };
                let txt = egui::RichText::new(name).size(11.0).strong().color(if on {
                    if t.light {
                        t.hdr_bot
                    } else {
                        Color32::from_rgb(0x16, 0x18, 0x1B)
                    }
                } else if host_on {
                    hdr_text()
                } else {
                    hdr_dim()
                });
                let mut b = egui::Button::new(txt)
                    .min_size(egui::vec2(62.0, 22.0))
                    .corner_radius(if theme::polished() { 5 } else { 2 });
                b = if on {
                    b.fill(if t.light { Color32::WHITE } else { t.accent })
                } else if host_on {
                    b.fill(t.accent.gamma_multiply(0.22))
                } else {
                    b.fill(Color32::from_black_alpha(if t.light { 30 } else { 60 }))
                };
                if ui.add(b).on_hover_text(tip).clicked() && !on {
                    self.vtab_select(tab);
                }
            }
            // ---- right: panel toggles
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                for (on, name, tip, which) in [
                    (
                        self.extra.browser_open,
                        "Browser",
                        "show / hide the Browser on the right (Ctrl+B)",
                        1,
                    ),
                    (
                        self.extra.inspector_open,
                        "Inspector",
                        "show / hide the Inspector on the left (I; Alt+I in the Tracker)",
                        0,
                    ),
                ] {
                    if ui
                        .add(
                            egui::Button::selectable(
                                on,
                                egui::RichText::new(name).size(11.0).color(if on {
                                    t.ink
                                } else {
                                    hdr_dim()
                                }),
                            )
                            .min_size(egui::vec2(0.0, 20.0)),
                        )
                        .on_hover_text(tip)
                        .clicked()
                    {
                        if which == 0 {
                            self.toggle_inspector();
                        } else {
                            self.toggle_browser();
                        }
                    }
                }
            });
        });
    }

    /// "bar 12.3 · 0:41.2" (timeline mode) or "pos 03 · row 12" (tracker)
    fn view_bar_position(&self, playing: bool) -> (String, f32) {
        let cue_tl = self.tlv.cue;
        let cue = self.cue;
        self.with(|s| {
            let bpm = s.song.bpm;
            if let Some(t) = s.song.timeline.as_deref() {
                let tick = match s.engine.tl_row().filter(|_| playing) {
                    Some(r) => r as i64 * crate::timeline::ROW,
                    None => cue_tl,
                };
                let bp = t.bar_pos(tick);
                let secs = t.seconds_at(tick, bpm);
                (
                    format!(
                        "bar {:>3}.{} · {}:{:04.1}",
                        bp.bar,
                        bp.beat,
                        (secs / 60.0) as u32,
                        secs % 60.0
                    ),
                    bpm,
                )
            } else {
                let (o, r) = if playing {
                    (s.engine.order_pos, s.engine.row)
                } else {
                    cue
                };
                (format!("pos {o:02} · row {r:02}"), bpm)
            }
        })
    }
}
