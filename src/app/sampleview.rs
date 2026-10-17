//! Sample editor: waveform (min/max peaks, zoom/scroll), selection, loop
//! points with draggable handles, edit ops, preview with playhead, undo.
use super::*;
use crate::sample::{self, LoopMode, Op, SampleLoop};

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Select(usize),
    LoopStart,
    LoopEnd,
}

struct SmpUndo {
    inst: usize,
    data: SampleData,
    lp: SampleLoop,
    base: u8,
}

pub(super) struct SampleEd {
    /// first visible frame and visible span in frames (0 = fit)
    from: f64,
    span: f64,
    sel: Option<(usize, usize)>,
    drag: Option<Drag>,
    undo: Vec<SmpUndo>,
    redo: Vec<SmpUndo>,
    gain_db: f32,
}

impl Default for SampleEd {
    fn default() -> Self {
        Self {
            from: 0.0,
            span: 0.0,
            sel: None,
            drag: None,
            undo: vec![],
            redo: vec![],
            gain_db: 3.0,
        }
    }
}

/// preview voices use the first channel
const PREVIEW_CH: usize = 0;

impl App {
    fn smp_state(&self, i: usize) -> Option<SmpUndo> {
        self.with(|s| match s.song.instruments.get(i).map(|x| &x.kind) {
            Some(InstrumentKind::Sample {
                data,
                lp,
                base_note,
                ..
            }) => Some(SmpUndo {
                inst: i,
                data: data.clone(),
                lp: *lp,
                base: *base_note,
            }),
            _ => None,
        })
    }

    fn smp_restore(&mut self, u: SmpUndo) {
        self.with(|s| {
            if let Some(InstrumentKind::Sample {
                data,
                lp,
                base_note,
                ..
            }) = s.song.instruments.get_mut(u.inst).map(|x| &mut x.kind)
            {
                *data = u.data;
                *lp = u.lp;
                *base_note = u.base;
            }
        });
    }

    fn smp_snapshot(&mut self, i: usize) {
        if let Some(st) = self.smp_state(i) {
            self.smp.undo.push(st);
            if self.smp.undo.len() > 50 {
                self.smp.undo.remove(0);
            }
            self.smp.redo.clear();
        }
    }

    pub(super) fn sample_undo(&mut self, redo: bool) {
        let from = if redo {
            self.smp.redo.pop()
        } else {
            self.smp.undo.pop()
        };
        let Some(u) = from else {
            self.status = "Nothing to undo in the sample editor".into();
            return;
        };
        if let Some(cur) = self.smp_state(u.inst) {
            if redo {
                self.smp.undo.push(cur);
            } else {
                self.smp.redo.push(cur);
            }
        }
        self.smp_restore(u);
        self.smp.sel = None;
        self.dirty = true;
        self.status = if redo {
            "Redo (sample)".into()
        } else {
            "Undo (sample)".into()
        };
    }

    fn smp_op(&mut self, i: usize, op: Op) {
        self.smp_snapshot(i);
        let sel = self.smp.sel;
        let r = self.with(|s| {
            s.engine.release(PREVIEW_CH);
            match s.song.instruments.get_mut(i).map(|x| &mut x.kind) {
                Some(InstrumentKind::Sample { data, lp, .. }) => {
                    Some(sample::apply(data, lp, sel, op))
                }
                _ => None,
            }
        });
        if let Some(ns) = r {
            self.smp.sel = ns;
            self.dirty = true;
            self.status = format!(
                "{} {}",
                op.name(),
                if sel.is_some() {
                    "selection"
                } else {
                    "whole sample"
                }
            );
        }
    }

    pub(super) fn sample_preview_toggle(&mut self) {
        let ins = self.instr;
        let idx = (ins as usize).wrapping_sub(1);
        self.with(|s| {
            if s.engine.sample_playhead(&s.song, PREVIEW_CH).is_some() {
                s.engine.release(PREVIEW_CH);
            } else if let Some(InstrumentKind::Sample { base_note, .. }) =
                s.song.instruments.get(idx).map(|x| &x.kind)
            {
                let n = *base_note;
                let Shared { song, engine, .. } = s;
                engine.preview(song, PREVIEW_CH, n, ins);
            }
        });
    }

    pub(super) fn sample_view(&mut self, ui: &mut egui::Ui) {
        let idx = (self.instr as usize).wrapping_sub(1);
        let t = th();
        let info = self.with(|s| {
            let samples: Vec<(usize, String)> = s
                .song
                .instruments
                .iter()
                .enumerate()
                .filter(|(_, i)| matches!(i.kind, InstrumentKind::Sample { .. }))
                .map(|(k, i)| (k, i.name.clone()))
                .collect();
            let cur = match s.song.instruments.get(idx) {
                Some(Instrument {
                    name,
                    kind:
                        InstrumentKind::Sample {
                            data,
                            rate,
                            lp,
                            base_note,
                            ..
                        },
                    ..
                }) => Some((name.clone(), data.len(), *rate, *lp, *base_note)),
                _ => None,
            };
            (samples, cur)
        });
        let (samples, cur) = info;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Sample editor").strong());
            let txt = cur
                .as_ref()
                .map(|c| format!("{:02X} {}", idx + 1, c.0))
                .unwrap_or_else(|| "(not a sample)".into());
            egui::ComboBox::from_id_salt("smp_inst")
                .selected_text(txt)
                .show_ui(ui, |ui| {
                    for (k, n) in &samples {
                        if ui
                            .selectable_label(*k == idx, format!("{:02X} {n}", k + 1))
                            .clicked()
                        {
                            self.instr = (*k + 1) as u8;
                            self.smp.sel = None;
                            self.smp.span = 0.0;
                        }
                    }
                });
            if ui
                .button("Import WAV…")
                .on_hover_text("load a .wav as a new sample instrument")
                .clicked()
            {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("WAV", &["wav"])
                    .pick_file()
                {
                    self.import_sample(p);
                    self.smp.sel = None;
                    self.smp.span = 0.0;
                }
            }
        });
        let Some((_, len, rate, lp, base)) = cur else {
            ui.add_space(20.0);
            ui.label("Select a sample instrument, or import a WAV (button above, or drop a .wav on the window).");
            return;
        };
        if len == 0 {
            ui.label("empty sample");
            return;
        }
        // ---------------------------------------------------------- controls
        let mut new_lp = lp;
        let mut new_base = base;
        let mut op = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Loop").color(t.ink2));
            for m in LoopMode::ALL {
                ui.selectable_value(&mut new_lp.mode, m, m.name())
                    .on_hover_text("loop playback mode (honoured by the player and export)");
            }
            if ui
                .add_enabled(
                    self.smp.sel.is_some(),
                    egui::Button::new("Loop = selection"),
                )
                .on_hover_text("set loop start/end to the selection")
                .clicked()
            {
                let (a, b) = sample::range(self.smp.sel, len);
                new_lp.start = a;
                new_lp.end = b;
                if new_lp.mode == LoopMode::Off {
                    new_lp.mode = LoopMode::Forward;
                }
            }
            ui.separator();
            ui.label(egui::RichText::new("Base note").color(t.ink2));
            ui.add(
                egui::DragValue::new(&mut new_base)
                    .range(0..=119)
                    .custom_formatter(|v, _| note_name(v as u8))
                    .speed(0.2),
            )
            .on_hover_text("the note at which the sample plays at its own pitch");
            ui.label(egui::RichText::new("Finetune").color(t.ink2));
            ui.add(
                egui::DragValue::new(&mut new_lp.finetune)
                    .range(-100..=100)
                    .suffix(" ct"),
            )
            .on_hover_text("fine pitch in cents");
            ui.separator();
            let playing = self.with(|s| s.engine.sample_playhead(&s.song, PREVIEW_CH).is_some());
            if ui
                .button(if playing { "Stop preview" } else { "Preview" })
                .on_hover_text("play the sample at its base note (Space)")
                .clicked()
            {
                self.sample_preview_toggle();
            }
        });
        ui.horizontal_wrapped(|ui| {
            let has_sel = self.smp.sel.is_some();
            for (o, tip, need_sel) in [
                (Op::Crop, "keep only the selection", true),
                (Op::Delete, "remove the selection", true),
                (
                    Op::TrimSilence,
                    "remove silence at the start and end",
                    false,
                ),
                (
                    Op::Normalise,
                    "raise to full scale (selection or all)",
                    false,
                ),
                (Op::Reverse, "reverse (selection or all)", false),
                (Op::FadeIn, "fade in (selection or all)", false),
                (Op::FadeOut, "fade out (selection or all)", false),
            ] {
                if ui
                    .add_enabled(has_sel || !need_sel, egui::Button::new(o.name()))
                    .on_hover_text(tip)
                    .clicked()
                {
                    op = Some(o);
                }
            }
            ui.separator();
            ui.add(
                egui::DragValue::new(&mut self.smp.gain_db)
                    .range(-24.0..=24.0)
                    .speed(0.1)
                    .suffix(" dB"),
            );
            if ui
                .button("Apply gain")
                .on_hover_text("change level (selection or all)")
                .clicked()
            {
                op = Some(Op::Gain(self.smp.gain_db));
            }
            ui.separator();
            if ui
                .add_enabled(!self.smp.undo.is_empty(), egui::Button::new("Undo"))
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                self.sample_undo(false);
            }
            if ui
                .add_enabled(!self.smp.redo.is_empty(), egui::Button::new("Redo"))
                .on_hover_text("Ctrl+Y")
                .clicked()
            {
                self.sample_undo(true);
            }
            ui.separator();
            if ui.button("Zoom −").clicked() {
                self.smp.span = (self.smp.span * 2.0).min(len as f64);
            }
            if ui
                .button("Zoom +")
                .on_hover_text("wheel zooms at the mouse; Shift+wheel scrolls")
                .clicked()
            {
                let s = if self.smp.span <= 0.0 {
                    len as f64
                } else {
                    self.smp.span
                };
                let c = self.smp.from + s / 2.0;
                self.smp.span = (s / 2.0).max(32.0);
                self.smp.from = c - self.smp.span / 2.0;
            }
            if ui.button("Fit").clicked() {
                self.smp.span = 0.0;
                self.smp.from = 0.0;
            }
        });
        if new_lp != lp || new_base != base {
            self.smp_snapshot(idx);
            new_lp.fix(len);
            if new_lp.mode != LoopMode::Off && new_lp.end <= new_lp.start + 1 {
                new_lp.start = 0;
                new_lp.end = len;
                new_lp.mode = if lp.mode == LoopMode::Off {
                    new_lp.mode
                } else {
                    lp.mode
                };
            }
            self.with(|s| {
                if let Some(InstrumentKind::Sample { lp, base_note, .. }) =
                    s.song.instruments.get_mut(idx).map(|x| &mut x.kind)
                {
                    *lp = new_lp;
                    *base_note = new_base;
                }
            });
            self.dirty = true;
        }
        if let Some(o) = op {
            self.smp_op(idx, o);
        }
        // ---------------------------------------------------------- waveform
        let (len, lp) = self.with(|s| match s.song.instruments.get(idx).map(|x| &x.kind) {
            Some(InstrumentKind::Sample { data, lp, .. }) => (data.len(), *lp),
            _ => (0, SampleLoop::default()),
        });
        if len == 0 {
            return;
        }
        let lenf = len as f64;
        if self.smp.span <= 0.0 || self.smp.span > lenf {
            self.smp.span = lenf;
        }
        self.smp.from = self.smp.from.clamp(0.0, (lenf - self.smp.span).max(0.0));
        let avail = ui.available_rect_before_wrap();
        let wave = Rect::from_min_max(
            avail.min + egui::vec2(0.0, 4.0),
            avail.max - egui::vec2(0.0, 40.0),
        );
        let resp = ui.allocate_rect(wave, Sense::click_and_drag());
        let p = ui.painter_at(wave);
        theme::crt(&p, wave);
        let (from, span) = (self.smp.from, self.smp.span);
        let to_x = |f: f64| wave.left() + ((f - from) / span) as f32 * wave.width();
        let to_f =
            |x: f32| (from + ((x - wave.left()) / wave.width()) as f64 * span).clamp(0.0, lenf);
        // loop region
        if lp.mode != LoopMode::Off {
            let r =
                Rect::from_x_y_ranges(to_x(lp.start as f64)..=to_x(lp.end as f64), wave.y_range())
                    .intersect(wave);
            p.rect_filled(r, 0, t.ochre.gamma_multiply(0.12));
        }
        // selection
        if let Some((a, b)) = self.smp.sel {
            let r = Rect::from_x_y_ranges(to_x(a as f64)..=to_x(b as f64), wave.y_range())
                .intersect(wave);
            p.rect_filled(r, 0, t.sel.gamma_multiply(0.35));
            p.rect_stroke(
                r,
                0,
                Stroke::new(1.0, t.selection_outline),
                egui::StrokeKind::Inside,
            );
        }
        // centre line + peaks
        let mid = wave.center().y;
        let hh = wave.height() * 0.46;
        p.hline(wave.x_range(), mid, Stroke::new(1.0, t.crt_grid));
        let cols = wave.width().max(1.0) as usize;
        let pk = self.with(|s| match s.song.instruments.get(idx).map(|x| &x.kind) {
            Some(InstrumentKind::Sample { data, .. }) => {
                sample::peaks(data, from, from + span, cols)
            }
            _ => vec![],
        });
        let wcol = t.accent.gamma_multiply(0.85);
        for (c, (lo, hi)) in pk.iter().enumerate() {
            let x = wave.left() + c as f32 + 0.5;
            let (y0, y1) = (
                mid - hi.clamp(-1.0, 1.0) * hh,
                mid - lo.clamp(-1.0, 1.0) * hh,
            );
            p.line_segment(
                [Pos2::new(x, y0), Pos2::new(x, y1.max(y0 + 1.0))],
                Stroke::new(1.0, wcol),
            );
        }
        // loop handles
        let handle = |f: usize, label: &str| {
            let x = to_x(f as f64);
            if x >= wave.left() - 1.0 && x <= wave.right() + 1.0 {
                p.vline(x, wave.y_range(), Stroke::new(2.0, t.ochre));
                let flag =
                    Rect::from_min_size(Pos2::new(x - 9.0, wave.top()), egui::vec2(18.0, 16.0));
                p.rect_filled(flag, 3, t.ochre);
                p.text(
                    flag.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    FontId::monospace(11.0),
                    t.crt_bot,
                );
            }
        };
        if lp.mode != LoopMode::Off {
            handle(lp.start, "S");
            handle(lp.end, "E");
        }
        // preview playhead
        let ph = self.with(|s| s.engine.sample_playhead(&s.song, PREVIEW_CH));
        if let Some((pi, pos)) = ph {
            if pi == idx {
                p.vline(to_x(pos), wave.y_range(), Stroke::new(1.5, t.play));
                ui.ctx().request_repaint();
            }
        }
        // ---------------------------------------------------------- mouse
        let near = |f: usize, x: f32| (to_x(f as f64) - x).abs() < 7.0;
        if let Some(hp) = resp.hover_pos() {
            if lp.mode != LoopMode::Off && (near(lp.start, hp.x) || near(lp.end, hp.x)) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            let (wheel, shift, ctrl) = ui.input(|i| {
                (
                    i.smooth_scroll_delta.y + i.smooth_scroll_delta.x,
                    i.modifiers.shift,
                    i.modifiers.command,
                )
            });
            if wheel.abs() > 0.0 {
                if shift {
                    self.smp.from -= (wheel / wave.width()) as f64 * span;
                } else {
                    let _ = ctrl;
                    let at = to_f(hp.x);
                    let k = (1.0 - wheel as f64 * 0.004).clamp(0.5, 2.0);
                    let ns = (span * k).clamp(32.0_f64.min(lenf), lenf);
                    self.smp.from = at - (at - from) * ns / span;
                    self.smp.span = ns;
                }
            }
        }
        if resp.drag_started() {
            if let Some(pp) = resp.interact_pointer_pos() {
                self.smp.drag = Some(if lp.mode != LoopMode::Off && near(lp.start, pp.x) {
                    self.smp_snapshot(idx);
                    Drag::LoopStart
                } else if lp.mode != LoopMode::Off && near(lp.end, pp.x) {
                    self.smp_snapshot(idx);
                    Drag::LoopEnd
                } else {
                    Drag::Select(to_f(pp.x) as usize)
                });
            }
        }
        if let (Some(d), Some(pp)) = (self.smp.drag, resp.interact_pointer_pos()) {
            let f = to_f(pp.x).round() as usize;
            match d {
                Drag::Select(a) => {
                    self.smp.sel = if f.abs_diff(a) > 0 {
                        Some((a.min(f), a.max(f)))
                    } else {
                        None
                    };
                }
                Drag::LoopStart | Drag::LoopEnd => {
                    self.with(|s| {
                        if let Some(InstrumentKind::Sample { lp, data, .. }) =
                            s.song.instruments.get_mut(idx).map(|x| &mut x.kind)
                        {
                            if d == Drag::LoopStart {
                                lp.start = f.min(lp.end.saturating_sub(2));
                            } else {
                                lp.end = f.max(lp.start + 2).min(data.len());
                            }
                        }
                    });
                    self.dirty = true;
                }
            }
            // auto-scroll at the edges while dragging
            if pp.x > wave.right() - 4.0 {
                self.smp.from += span * 0.02;
            } else if pp.x < wave.left() + 4.0 {
                self.smp.from -= span * 0.02;
            }
        }
        if resp.drag_stopped() {
            self.smp.drag = None;
        }
        if resp.clicked() {
            self.smp.sel = None;
        }
        // ---------------------------------------------------------- scrollbar + info
        let bar = Rect::from_min_max(
            Pos2::new(wave.left(), wave.bottom() + 6.0),
            Pos2::new(wave.right(), wave.bottom() + 16.0),
        );
        let br = ui.interact(
            bar,
            ui.make_persistent_id("smp_bar"),
            Sense::click_and_drag(),
        );
        let pb = ui.painter();
        pb.rect_filled(bar, 4, t.crt_bot);
        let thumb = Rect::from_x_y_ranges(
            (bar.left() + (from / lenf) as f32 * bar.width())
                ..=(bar.left() + ((from + span) / lenf) as f32 * bar.width())
                    .max(bar.left() + (from / lenf) as f32 * bar.width() + 8.0),
            bar.y_range(),
        );
        pb.rect_filled(
            thumb,
            4,
            if br.hovered() || br.dragged() {
                t.accent
            } else {
                t.ink2
            },
        );
        if br.dragged() {
            self.smp.from += (br.drag_delta().x / bar.width()) as f64 * lenf;
        }
        let secs = |f: usize| f as f32 / rate.max(1) as f32;
        let sel_txt = self
            .smp
            .sel
            .map(|(a, b)| format!("selection {}..{} ({:.3} s)", a, b, secs(b - a)))
            .unwrap_or_else(|| "no selection (ops apply to all)".into());
        let loop_txt = if lp.mode == LoopMode::Off {
            "loop off".to_string()
        } else {
            format!(
                "loop {} {}..{} ({:.3} s)",
                lp.mode.name(),
                lp.start,
                lp.end,
                secs(lp.end - lp.start)
            )
        };
        pb.text(
            Pos2::new(wave.left(), bar.bottom() + 12.0),
            egui::Align2::LEFT_CENTER,
            format!("{} frames · {:.3} s · {} Hz  ·  {}  ·  {}  ·  drag: select / S,E handles · wheel: zoom · Space: preview", len, secs(len), rate, sel_txt, loop_txt),
            FontId::monospace(11.5),
            t.ink2,
        );
    }
}
