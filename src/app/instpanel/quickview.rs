//! Quick-mod in the instrument panel: the dot on every knob opens a small
//! "Modulate <param> with…" popover, and one Movement list per instrument
//! shows everything that moves (source → target · amount · ×). Where it
//! lands is `crate::quickmod`'s job (an engine's own slots first, else a
//! song-level modulator), so songs keep their format.
use super::*;
use crate::automation::Target;
use crate::modulation::LfoShape;
use crate::quickmod::{self as qm, RowId, Source, Spec};

/// the open popover
pub(in crate::app) struct Quick {
    pub inst: usize,
    /// the parameter key it belongs to ("" = opened from the Movement list)
    pub key: String,
    pub target: Option<Target>,
    pub name: String,
    pub spec: Spec,
    /// editing an existing movement instead of adding one
    pub edit: Option<RowId>,
    /// a "slots full" or other message
    pub msg: Option<String>,
    pub opened: f64,
    /// the frame it was last drawn (drawn once per frame)
    pub drawn: u64,
    /// undo snapshot taken for this edit
    pub snap: bool,
    /// Length shown in beats and bars (the engines keep seconds)
    pub len_sync: bool,
    /// the Envelope's times were read from the instrument
    pub env_init: bool,
}

/// beyond the song's route indices: a ring of an engine's own route
pub(super) const NATIVE_BASE: usize = 1 << 20;

/// the colour of an engine's own route `j` (song modulators use theirs)
pub(super) fn native_colour(j: usize) -> egui::Color32 {
    super::super::modview::mod_colour(j + 3)
}

fn shape_icon(ui: &mut egui::Ui, s: LfoShape, on: bool) -> egui::Response {
    let t = th();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(30.0, 22.0), Sense::click());
    let p = ui.painter();
    let fill = if on {
        t.accent.gamma_multiply(0.35)
    } else if resp.hovered() {
        t.ink2.gamma_multiply(0.15)
    } else {
        egui::Color32::TRANSPARENT
    };
    p.rect_filled(r, 4.0, fill);
    p.rect_stroke(
        r,
        4.0,
        Stroke::new(
            1.0,
            if on {
                t.accent
            } else {
                t.ink2.gamma_multiply(0.5)
            },
        ),
        egui::StrokeKind::Inside,
    );
    let ir = r.shrink2(egui::vec2(6.0, 6.0));
    let pts: Vec<Pos2> = (0..=24)
        .map(|i| {
            let ph = i as f32 / 24.0;
            // the square's edges as steps
            let y = s.at(ph.min(0.9999));
            Pos2::new(
                ir.left() + ph * ir.width(),
                ir.center().y - y * ir.height() * 0.5,
            )
        })
        .collect();
    p.add(egui::Shape::line(
        pts,
        Stroke::new(1.5, if on { t.ink } else { t.ink2 }),
    ));
    let nm = s.name();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, nm));
    resp.on_hover_text(nm)
}

/// the width of the control labels, so the sliders line up
const LABEL_W: f32 = 56.0;
const SLIDER_W: f32 = 150.0;

fn row_label(ui: &mut egui::Ui, text: &str) {
    ui.add_sized(
        egui::vec2(LABEL_W, 18.0),
        egui::Label::new(egui::RichText::new(text).small()),
    );
}

/// A labelled horizontal slider over `pos` 0..1: drag (Ctrl-drag for
/// fine steps), click to jump, double-click back to `reset`. `steps` > 1
/// draws that many notches (the caller snaps). The value is `text`.
fn slide(
    ui: &mut egui::Ui,
    label: &str,
    tip: &str,
    pos: &mut f32,
    reset: f32,
    steps: usize,
    text: &str,
) -> bool {
    let t = th();
    let mut changed = false;
    ui.horizontal(|ui| {
        row_label(ui, label);
        let (r, resp) = ui.allocate_exact_size(egui::vec2(SLIDER_W, 18.0), Sense::click_and_drag());
        let id = resp.id.with("acc");
        let fine = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if resp.double_clicked() {
            *pos = reset;
            changed = true;
        } else if resp.drag_started() {
            ui.data_mut(|d| d.insert_temp(id, *pos));
        } else if resp.dragged() {
            let acc: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(*pos);
            let f = if fine { 0.1 } else { 1.0 };
            let acc = (acc + resp.drag_delta().x / r.width() * f).clamp(0.0, 1.0);
            ui.data_mut(|d| d.insert_temp(id, acc));
            if (acc - *pos).abs() > 1e-6 {
                *pos = acc;
                changed = true;
            }
        } else if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                *pos = ((p.x - r.left()) / r.width()).clamp(0.0, 1.0);
                changed = true;
            }
        }
        let p = ui.painter();
        let cy = r.center().y;
        let x0 = r.left() + 6.0;
        let x1 = r.right() - 6.0;
        let hx = x0 + (x1 - x0) * pos.clamp(0.0, 1.0);
        p.line_segment(
            [Pos2::new(x0, cy), Pos2::new(x1, cy)],
            Stroke::new(4.0, t.ink2.gamma_multiply(0.3)),
        );
        p.line_segment(
            [Pos2::new(x0, cy), Pos2::new(hx, cy)],
            Stroke::new(4.0, t.accent),
        );
        if steps > 1 {
            for i in 0..steps {
                let x = x0 + (x1 - x0) * i as f32 / (steps - 1) as f32;
                p.line_segment(
                    [Pos2::new(x, cy + 4.0), Pos2::new(x, cy + 7.0)],
                    Stroke::new(1.0, t.ink2.gamma_multiply(0.6)),
                );
            }
        }
        let hot = resp.hovered() || resp.dragged();
        p.circle_filled(Pos2::new(hx, cy), if hot { 6.5 } else { 5.5 }, t.ink);
        p.circle_stroke(Pos2::new(hx, cy), 5.5, Stroke::new(1.5, t.accent));
        let lbl = format!("{label}: {text}");
        resp.widget_info(|| egui::WidgetInfo::slider(true, *pos as f64, &lbl));
        resp.on_hover_text(format!(
            "{tip}\ndrag (Ctrl for fine steps) · double-click to reset"
        ));
        ui.label(egui::RichText::new(text).strong());
    });
    changed
}

fn log_pos(v: f32, lo: f32, hi: f32) -> f32 {
    ((v.max(lo) / lo).ln() / (hi / lo).ln()).clamp(0.0, 1.0)
}

fn log_val(p: f32, lo: f32, hi: f32) -> f32 {
    lo * (hi / lo).powf(p.clamp(0.0, 1.0))
}

/// the nearest of `divs` (beats) to `b`, on a log scale
fn nearest(divs: &[f32], b: f32) -> usize {
    (0..divs.len())
        .min_by(|&i, &j| {
            let d = |k: usize| (divs[k].ln() - b.max(1e-3).ln()).abs();
            d(i).total_cmp(&d(j))
        })
        .unwrap_or(0)
}

/// Speed: tempo divisions (8 bars .. 1/16 note, slow on the left) when
/// synced, else 0.02..20 Hz; "Sync to tempo" switches, keeping the speed
fn speed_ui(ui: &mut egui::Ui, sync: &mut f32, rate: &mut f32, max_sync: f32, bpm: f32) -> bool {
    let mut changed = false;
    let divs: Vec<f32> = qm::DIVS
        .iter()
        .copied()
        .filter(|d| *d <= max_sync + 1e-4)
        .collect();
    let n = divs.len();
    let bps = bpm.max(1.0) / 60.0;
    if *sync > 0.0 && n > 1 {
        let cur = nearest(&divs, *sync);
        let mut pos = 1.0 - cur as f32 / (n - 1) as f32;
        let one = divs
            .iter()
            .position(|d| (*d - 1.0).abs() < 1e-4)
            .unwrap_or(0);
        if slide(
            ui,
            "Speed",
            "How fast it moves",
            &mut pos,
            1.0 - one as f32 / (n - 1) as f32,
            n,
            &qm::speed_text(*sync, *rate),
        ) {
            let k = ((1.0 - pos) * (n - 1) as f32).round() as usize;
            if (divs[k.min(n - 1)] - *sync).abs() > 1e-4 {
                *sync = divs[k.min(n - 1)];
                changed = true;
            }
        }
    } else {
        let mut pos = log_pos(*rate, 0.02, 20.0);
        if slide(
            ui,
            "Speed",
            "How fast it moves",
            &mut pos,
            log_pos(1.0, 0.02, 20.0),
            0,
            &qm::speed_text(0.0, *rate),
        ) {
            *rate = log_val(pos, 0.02, 20.0);
            changed = true;
        }
    }
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + ui.spacing().item_spacing.x);
        let mut on = *sync > 0.0;
        if n > 1
            && ui
                .checkbox(&mut on, egui::RichText::new("Sync to tempo").small())
                .on_hover_text("on: in beats and bars, following the song's tempo; off: in Hz")
                .changed()
        {
            if on {
                *sync = divs[nearest(&divs, bps / rate.max(0.01))];
            } else {
                *rate = (bps / sync.max(0.01)).clamp(0.02, 20.0);
                *sync = 0.0;
            }
            changed = true;
        }
    });
    changed
}

/// Attack and Length (seconds), Length in beats while `synced`
fn length_ui(
    ui: &mut egui::Ui,
    attack: Option<&mut f32>,
    len: &mut f32,
    synced: &mut bool,
    bpm: f32,
) -> bool {
    let mut changed = false;
    let spb = 60.0 / bpm.max(1.0);
    // the divisions that fit the engine's 10 s
    let divs: Vec<f32> = qm::DIVS
        .iter()
        .copied()
        .filter(|d| d * spb <= 10.0)
        .collect();
    let n = divs.len();
    if *synced && n > 1 {
        let cur = nearest(&divs, *len / spb);
        let mut pos = cur as f32 / (n - 1) as f32;
        let one = divs
            .iter()
            .position(|d| (*d - 1.0).abs() < 1e-4)
            .unwrap_or(0);
        if slide(
            ui,
            "Length",
            "How long the envelope lasts",
            &mut pos,
            one as f32 / (n - 1) as f32,
            n,
            &qm::beats_text(divs[cur]),
        ) {
            let k = (pos * (n - 1) as f32).round() as usize;
            *len = divs[k.min(n - 1)] * spb;
            changed = true;
        }
    } else {
        let mut pos = log_pos(*len, 0.005, 10.0);
        if slide(
            ui,
            "Length",
            "How long the envelope lasts",
            &mut pos,
            log_pos(0.3, 0.005, 10.0),
            0,
            &qm::secs_text(*len),
        ) {
            *len = log_val(pos, 0.005, 10.0);
            changed = true;
        }
    }
    if let Some(a) = attack {
        let mut pos = log_pos(*a, 0.001, 5.0);
        if slide(
            ui,
            "Attack",
            "How long it takes to rise at the start of a note",
            &mut pos,
            log_pos(0.01, 0.001, 5.0),
            0,
            &qm::secs_text(*a),
        ) {
            *a = log_val(pos, 0.001, 5.0);
            changed = true;
        }
    }
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + ui.spacing().item_spacing.x);
        if ui
            .checkbox(synced, egui::RichText::new("Sync to tempo").small())
            .on_hover_text("on: the length in beats and bars at the song's tempo; off: in ms / s")
            .changed()
            && *synced
            && n > 1
        {
            *len = divs[nearest(&divs, *len / spb)] * spb;
            changed = true;
        }
    });
    changed
}

fn smooth_ui(ui: &mut egui::Ui, s: &mut f32) -> bool {
    slide(
        ui,
        "Smooth",
        "How softly it glides from one random step to the next",
        s,
        0.0,
        0,
        &format!("{:.0} %", *s * 100.0),
    )
}

fn amount_ui(ui: &mut egui::Ui, a: &mut f32) -> bool {
    let mut pct = *a * 100.0;
    let r = ui
        .horizontal(|ui| {
            row_label(ui, "Amount");
            ui.spacing_mut().slider_width = SLIDER_W;
            ui.add(
                egui::Slider::new(&mut pct, -100.0..=100.0)
                    .custom_formatter(|v, _| format!("{v:+.0} %"))
                    .max_decimals(0),
            )
            .on_hover_text("How far it moves the knob (of its whole travel, ±)")
        })
        .inner;
    if r.changed() {
        *a = (pct / 100.0).clamp(-1.0, 1.0);
    }
    r.changed()
}

/// every timing control `tm` has (shape, speed, smooth, length, attack)
fn timing_ui(ui: &mut egui::Ui, tm: &mut qm::Timing, len_sync: &mut bool, bpm: f32) -> bool {
    let mut changed = false;
    if let Some(sh) = tm.shape.as_mut() {
        ui.horizontal(|ui| {
            row_label(ui, "Shape");
            for s in LfoShape::ALL {
                if shape_icon(ui, s, *sh == s).clicked() && *sh != s {
                    *sh = s;
                    changed = true;
                }
            }
        });
    }
    if let Some((sync, rate)) = tm.speed.as_mut() {
        changed |= speed_ui(ui, sync, rate, tm.max_sync, bpm);
    }
    if let Some(s) = tm.smooth.as_mut() {
        changed |= smooth_ui(ui, s);
    }
    if let Some(l) = tm.length.as_mut() {
        changed |= length_ui(ui, tm.attack.as_mut(), l, len_sync, bpm);
    }
    changed
}

/// a fixed pseudo-random step value -1..1
fn rnd(i: usize) -> f32 {
    let x = (i as u32).wrapping_mul(2_654_435_761).rotate_left(13) ^ 0x9E37_79B9;
    (x.wrapping_mul(0x85EB_CA6B) >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
}

/// The shape it moves in (two cycles; an envelope's rise and fall) and,
/// while the song plays, a dot running along it at its speed. Returns
/// whether it is animating (so the caller repaints).
fn preview_ui(ui: &mut egui::Ui, tm: &qm::Timing, sus: f32, bpm: f32, run: Option<f64>) -> bool {
    let t = th();
    let w = ui.available_width().min(LABEL_W + SLIDER_W + 110.0);
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 34.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 4.0, t.ink2.gamma_multiply(0.08));
    let ir = r.shrink2(egui::vec2(8.0, 5.0));
    let env = tm.length.is_some();
    let a = tm.attack.unwrap_or(0.01).max(0.0005);
    let d = tm.length.unwrap_or(0.3).max(0.005);
    let total = (a + d) * 1.35;
    // x 0..1 → y -1..1
    let y_at = |x: f32| -> f32 {
        if env {
            let s = x * total;
            let v = if s < a {
                s / a
            } else if s < a + d {
                let k = (s - a) / d;
                1.0 - (1.0 - sus) * (1.0 - (1.0 - k).powi(2))
            } else {
                sus
            };
            v * 2.0 - 1.0
        } else if tm.random {
            let ph = x * 8.0;
            let i = ph.floor() as usize;
            let f = ph.fract();
            let sm = tm.smooth.unwrap_or(0.0);
            if sm > 0.0 && f < sm {
                let k = f / sm;
                let k = k * k * (3.0 - 2.0 * k);
                rnd(i.wrapping_sub(1).wrapping_add(7)) * (1.0 - k) + rnd(i + 7) * k
            } else {
                rnd(i + 7)
            }
        } else {
            tm.shape
                .unwrap_or(tm.look)
                .at((x * 2.0).fract().min(0.9999))
        }
    };
    let pt = |x: f32| {
        Pos2::new(
            ir.left() + x * ir.width(),
            ir.center().y - y_at(x) * ir.height() * 0.5,
        )
    };
    let pts: Vec<Pos2> = (0..=160).map(|i| pt(i as f32 / 160.0)).collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.5, t.accent)));
    let mut anim = false;
    if let Some(now) = run {
        // the length of what is drawn, in seconds
        let span = if env {
            total
        } else {
            let (sync, rate) = tm.speed.unwrap_or((1.0, 1.0));
            let hz = if sync > 0.0 {
                bpm.max(1.0) / 60.0 / sync
            } else {
                rate
            };
            2.0 / hz.max(1e-3)
        };
        let x = ((now / span as f64).fract()) as f32;
        p.circle_filled(pt(x), 4.0, t.ink);
        p.circle_stroke(pt(x), 4.0, Stroke::new(1.0, t.accent));
        anim = true;
    }
    resp.on_hover_text(if env {
        "the envelope: rises over the attack, falls over the length"
    } else {
        "two cycles of the movement (the dot runs while the song plays)"
    });
    anim
}

/// the target part of a song route's label without the instrument's name
fn short_label(label: &str, inst: &Instrument) -> String {
    label
        .strip_prefix(&format!("{} ", inst.name))
        .unwrap_or(label)
        .to_string()
}

impl App {
    /// every movement of the instrument being edited
    pub(super) fn quick_rows(&self, inst: &Instrument, cx: &Cx) -> Vec<qm::Row> {
        let labels = &cx.labels;
        let m = &cx.modm;
        let lbl = |id: &str| {
            let r = m.routes.iter().position(|r| r.target == id);
            r.and_then(|r| labels.get(r))
                .map(|l| short_label(l, inst))
                .unwrap_or_else(|| id.to_string())
        };
        qm::rows(inst, cx.idx, m, &lbl)
    }

    /// open the popover for knob `p` (editing what already moves it, if
    /// anything does)
    pub(super) fn quick_open(&mut self, p: &Param, inst: &Instrument, cx: &Cx, now: f64) {
        let Some(tg) = p.target else {
            return;
        };
        let rows = self.quick_rows(inst, cx);
        let edit = rows.iter().find(|r| r.knobs.contains(&tg)).map(|r| r.id);
        self.quick_open_at(cx.idx, &p.key, Some(tg), &p.name, edit, now);
    }

    pub(super) fn quick_open_at(
        &mut self,
        inst: usize,
        key: &str,
        target: Option<Target>,
        name: &str,
        edit: Option<RowId>,
        now: f64,
    ) {
        self.ip.quick = Some(Quick {
            inst,
            key: key.to_string(),
            target,
            name: name.to_string(),
            spec: Spec {
                source: self.ip.quick_src.take().unwrap_or_default(),
                ..Spec::default()
            },
            edit,
            msg: None,
            opened: now,
            drawn: u64::MAX,
            snap: false,
            len_sync: false,
            env_init: false,
        });
    }

    /// draw the popover under `anchor` when it belongs to this knob / row
    pub(super) fn quick_popover(
        &mut self,
        ui: &egui::Ui,
        key: &str,
        anchor: Rect,
        inst: &mut Instrument,
        cx: &mut Cx,
    ) {
        let ctx = &ui.ctx().clone();
        // on screen (StringWave draws on a scaled, translated layer)
        let anchor = match ctx.layer_transform_to_global(ui.layer_id()) {
            Some(t) => t * anchor,
            None => anchor,
        };
        let frame = ctx.cumulative_pass_nr();
        let Some(q) = self.ip.quick.as_mut() else {
            return;
        };
        if q.inst != cx.idx || q.key != key || q.drawn == frame {
            return;
        }
        q.drawn = frame;
        let t = th();
        let rows = self.quick_rows(inst, cx);
        let (bpm, playing) = self.with(|s| (s.song.bpm, s.engine.playing));
        let calm = self.extra.reduce_motion;
        let now = ctx.input(|i| i.time);
        // the preview's dot runs while the song plays (still with reduce
        // motion)
        let run = self.ip.quick_dot.or((playing && !calm).then_some(now));
        let mut anim = false;
        let Some(mut q) = self.ip.quick.take() else {
            return;
        };
        if !q.env_init {
            q.env_init = true;
            (q.spec.attack, q.spec.length) = qm::env_start(inst);
        }
        // an envelope's level after its length (the preview)
        let sus_of =
            |inst: &Instrument, m: &crate::modulation::Modulation, row: Option<RowId>| match row {
                Some(RowId::Song(r)) => m
                    .routes
                    .get(r)
                    .and_then(|rt| m.find(rt.src))
                    .map_or(0.5, |k| m.mods[k].s),
                Some(RowId::Native(_)) => 0.0,
                None => match &inst.kind {
                    InstrumentKind::Gen(g)
                        if matches!(g.engine, crate::gen::GenEngine::Strings(_)) =>
                    {
                        0.0
                    }
                    _ => 0.5,
                },
            };
        // an edited row that is gone (removed elsewhere): add mode
        if q.edit.is_some_and(|e| !rows.iter().any(|r| r.id == e)) {
            q.edit = None;
        }
        let w = 340.0;
        let screen = ctx.content_rect();
        let id = egui::Id::unique("quickmod_pop");
        // its height as last drawn (250 before the first frame)
        let h = ctx
            .memory(|m| m.area_rect(id))
            .map_or(250.0, |r| r.height());
        let mut pos = Pos2::new(anchor.left(), anchor.bottom() + 4.0);
        if pos.y + h > screen.bottom() {
            pos.y = (anchor.top() - 4.0 - h).max(screen.top());
        }
        pos.x = pos.x.min(screen.right() - w - 16.0).max(screen.left());
        let mut close = false;
        let mut add = false;
        let mut remove = None;
        let area = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(w);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Modulate {} with…", q.name)).strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").on_hover_text("close (Esc)").clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(2.0);
                    if let Some(e) = q.edit {
                        let Some(row) = rows.iter().find(|r| r.id == e) else {
                            return;
                        };
                        let col = row
                            .modk
                            .map(super::super::modview::mod_colour)
                            .unwrap_or(t.accent);
                        ui.horizontal(|ui| {
                            let (r, _) =
                                ui.allocate_exact_size(egui::vec2(10.0, 10.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 4.5, col);
                            ui.label(format!("{} → {}", row.source, row.target));
                        });
                        if row.per_note {
                            ui.label(
                                egui::RichText::new("per note: every voice moves on its own")
                                    .small()
                                    .color(t.ink2),
                            );
                        }
                        // its speed / length / smooth, as this kind of source has
                        if let Some(mut tm) = qm::timing(inst, &cx.modm, row.id) {
                            let was = tm;
                            if timing_ui(ui, &mut tm, &mut q.len_sync, bpm) && tm != was {
                                qm::set_timing(inst, &mut cx.modm, row.id, &tm, bpm);
                                cx.changed = true;
                            }
                            if !tm.is_empty() {
                                let sus = sus_of(inst, &cx.modm, Some(row.id));
                                anim |= preview_ui(ui, &tm, sus, bpm, run);
                            }
                            let shared = match row.id {
                                RowId::Song(r) => cx
                                    .modm
                                    .routes
                                    .get(r)
                                    .map_or(0, |x| {
                                        cx.modm.routes.iter().filter(|y| y.src == x.src).count()
                                    }),
                                RowId::Native(qm::Native::Chip(_)) => rows
                                    .iter()
                                    .filter(|r| matches!(r.id, RowId::Native(qm::Native::Chip(_))))
                                    .count(),
                                _ => 0,
                            };
                            if shared > 1 && !tm.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "the speed and shape are shared by {shared} knobs"
                                    ))
                                    .small()
                                    .color(t.ink2),
                                );
                            }
                        }
                        let mut a = row.amount;
                        if amount_ui(ui, &mut a) {
                            if !q.snap {
                                q.snap = true;
                            }
                            qm::set_amount(inst, &mut cx.modm, row.id, a);
                            cx.changed = true;
                        }
                        ui.label(egui::RichText::new(row.text.clone()).small().color(t.value));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button("Remove")
                                .on_hover_text("stop this movement")
                                .clicked()
                            {
                                remove = Some(row.id);
                            }
                            if q.target.is_some()
                                && ui
                                    .button("+ Add another")
                                    .on_hover_text("move this knob with a second source too")
                                    .clicked()
                            {
                                q.edit = None;
                                q.msg = None;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Done").clicked() {
                                        close = true;
                                    }
                                },
                            );
                        });
                        return;
                    }
                    // ---- add ----
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                        for s in Source::ALL {
                            let on = q.spec.source == s;
                            let b = ui.add(
                                egui::Button::selectable(on, s.name())
                                    .min_size(egui::vec2(52.0, 22.0)),
                            );
                            if b.clicked() {
                                q.spec.source = s;
                                q.msg = None;
                            }
                        }
                    });
                    ui.add_space(2.0);
                    // where it would go (a dry run on copies)
                    let hint = q.target.map(|tg| {
                        let mut i2 = inst.clone();
                        let mut m2 = cx.modm.clone();
                        qm::add(&mut i2, cx.idx, &mut m2, tg, &q.spec, bpm)
                    });
                    // WaveForge's own filter envelope keeps its knobs' times
                    let poly_env = q.spec.source == Source::Envelope
                        && matches!(hint, Some(Ok(qm::Added { row: RowId::Native(qm::Native::Poly(_)), .. })));
                    let src = q.spec.source;
                    let mut tm = qm::Timing {
                        shape: (src == Source::Lfo).then_some(q.spec.shape),
                        speed: src.has_rate().then_some((q.spec.sync, q.spec.rate)),
                        smooth: src.has_smooth().then_some(q.spec.smooth),
                        length: (src.has_length() && !poly_env).then_some(q.spec.length),
                        attack: (src.has_length() && !poly_env).then_some(q.spec.attack),
                        random: src == Source::Random,
                        ..qm::Timing::none()
                    };
                    if timing_ui(ui, &mut tm, &mut q.len_sync, bpm) {
                        if let Some(s) = tm.shape {
                            q.spec.shape = s;
                        }
                        if let Some((sy, ra)) = tm.speed {
                            (q.spec.sync, q.spec.rate) = (sy, ra);
                        }
                        if let Some(s) = tm.smooth {
                            q.spec.smooth = s;
                        }
                        if let (Some(l), Some(a)) = (tm.length, tm.attack) {
                            q.spec.env_set |= l != q.spec.length || a != q.spec.attack;
                            (q.spec.length, q.spec.attack) = (l, a);
                        }
                    }
                    if !tm.is_empty() {
                        anim |= preview_ui(ui, &tm, sus_of(inst, &cx.modm, None), bpm, run);
                    }
                    amount_ui(ui, &mut q.spec.amount);
                    if poly_env {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "uses {}'s own filter envelope: its Attack and Decay knobs set how long it lasts",
                                    crate::poly::POLY_DISPLAY_NAME
                                ))
                                .small()
                                .color(t.ink2),
                            )
                            .wrap(),
                        );
                    }
                    let ok = matches!(hint, Some(Ok(_)));
                    match (&q.msg, &hint) {
                        (Some(m), _) | (None, Some(Err(m))) => {
                            ui.label(egui::RichText::new(m).small().color(t.brick));
                        }
                        (None, Some(Ok(a))) => {
                            let txt = match (&a.note, a.row) {
                                (Some(n), _) => n.clone(),
                                (None, RowId::Native(_)) if poly_env => String::new(),
                        (None, RowId::Native(_)) => format!(
                                    "uses {}'s own modulation",
                                    qm::engine(&inst.kind)
                                        .map(|e| e.engine())
                                        .unwrap_or("the instrument")
                                ),
                                (None, RowId::Song(_)) => String::new(),
                            };
                            if !txt.is_empty() {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(txt).small().color(t.ink2),
                                    )
                                    .wrap(),
                                );
                            }
                        }
                        _ => {}
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(ok, egui::Button::new("Add")).clicked() {
                            add = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            });
        let rect = area.response.rect;
        let outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !rect.contains(p) && !anchor.contains(p))
        });
        if anim && self.ip.quick_dot.is_none() {
            ctx.request_repaint();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || (outside && now - q.opened > 0.2) {
            close = true;
        }
        if add {
            if let Some(tg) = q.target {
                self.snapshot();
                match qm::add(inst, cx.idx, &mut cx.modm, tg, &q.spec, bpm) {
                    Ok(a) => {
                        cx.changed = true;
                        self.status = match a.note {
                            Some(n) => n,
                            None => format!(
                                "{} now moves with {}: drag its ring to change how far",
                                q.name,
                                q.spec.source.name()
                            ),
                        };
                        if let RowId::Song(r) = a.row {
                            self.ip.mod_sel = cx.modm.routes.get(r).map(|x| x.src);
                        }
                        close = true;
                    }
                    Err(e) => {
                        self.status = e.clone();
                        q.msg = Some(e);
                    }
                }
            }
        }
        if let Some(id) = remove {
            match id {
                RowId::Native(n) => {
                    self.snapshot();
                    qm::remove_native(inst, n);
                    cx.changed = true;
                }
                RowId::Song(r) => cx.mod_out.remove_route = Some(r),
            }
            self.status = format!("{}: movement removed", q.name);
            close = true;
        }
        if q.snap {
            // one undo step for the amount edit, taken before it lands
            q.snap = false;
        }
        if !close {
            self.ip.quick = Some(q);
        }
    }

    /// the Movement list of the instrument being edited (`count`: an
    /// "N active" line above the rows; StringWave's short strip puts it in
    /// its title row instead)
    pub(super) fn movement_list(
        &mut self,
        ui: &mut egui::Ui,
        inst: &mut Instrument,
        cx: &mut Cx,
        count: bool,
    ) {
        let t = th();
        let rows = self.quick_rows(inst, cx);
        let now = ui.input(|i| i.time);
        if count && !rows.is_empty() {
            ui.label(
                egui::RichText::new(format!("{} active", rows.len()))
                    .small()
                    .color(t.ink2),
            );
        }
        if rows.is_empty() {
            ui.add(
                egui::Label::new(
                    egui::RichText::new("Click the dot on any knob to add movement").color(t.ink2),
                )
                .wrap(),
            );
            return;
        }
        let mut native_j = 0;
        for row in &rows {
            let col = match row.modk {
                Some(k) => super::super::modview::mod_colour(k),
                None => {
                    native_j += 1;
                    native_colour(native_j - 1)
                }
            };
            let key = format!("move.{:?}", row.id);
            let resp = ui
                .horizontal(|ui| {
                    // the colour dot: a song-level source can still be
                    // dragged onto any knob (or FX slider)
                    let (dr, dresp) = ui.allocate_exact_size(egui::vec2(12.0, 18.0), Sense::drag());
                    ui.painter().circle_filled(dr.center(), 4.5, col);
                    if let (Some(k), true) = (row.modk, dresp.dragged()) {
                        dresp.dnd_set_drag_payload(super::super::modview::ModDrag(
                            cx.modm.mods[k].id,
                        ));
                    }
                    let lw = (ui.available_width() - 110.0).max(80.0);
                    let txt = format!(
                        "{} → {}{}",
                        row.source,
                        row.target,
                        if row.per_note { " (per note)" } else { "" }
                    );
                    let lr = ui
                        .allocate_ui_with_layout(
                            egui::vec2(lw, 18.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(lw);
                                ui.add(
                                    egui::Label::new(egui::RichText::new(&txt))
                                        .truncate()
                                        .sense(Sense::click()),
                                )
                            },
                        )
                        .inner
                        .on_hover_text(format!("{txt}  {}\nclick to edit", row.text));
                    if lr.clicked() {
                        let tg = row.knobs.first().copied();
                        self.quick_open_at(cx.idx, &key, tg, &row.target, Some(row.id), now);
                    }
                    let mut pct = row.amount * 100.0;
                    let dv = ui
                        .add(
                            egui::DragValue::new(&mut pct)
                                .range(-100.0..=100.0)
                                .speed(0.5)
                                .custom_formatter(|v, _| format!("{v:+.0} %"))
                                .custom_parser(|s| {
                                    s.trim().trim_end_matches('%').trim().parse().ok()
                                }),
                        )
                        .on_hover_text(format!("amount: {}", row.text));
                    if dv.changed() {
                        qm::set_amount(inst, &mut cx.modm, row.id, pct / 100.0);
                        cx.changed = true;
                    }
                    let x = ui.small_button("×").on_hover_text("remove this movement");
                    let nm = format!("Remove {txt}");
                    x.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &nm));
                    if x.clicked() {
                        match row.id {
                            RowId::Native(n) => {
                                self.snapshot();
                                qm::remove_native(inst, n);
                                cx.changed = true;
                            }
                            RowId::Song(r) => cx.mod_out.remove_route = Some(r),
                        }
                    }
                    if let RowId::Song(r) = row.id {
                        lr.context_menu(|ui| {
                            if ui.button("Add automation lane (amount)").clicked() {
                                cx.mod_out.lane = Some(r);
                                ui.close();
                            }
                        });
                    }
                })
                .response;
            self.quick_popover(ui, &key, resp.rect, inst, cx);
        }
    }
}
