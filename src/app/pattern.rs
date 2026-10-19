//! The pattern (block) editor view.
use super::*;

impl App {
    pub(super) fn pattern_view(&mut self, ui: &mut egui::Ui) {
        let (song_snapshot, play_pat, play_row, playing) = self.with(|s| {
            (
                s.song.patterns.get(self.pattern).cloned(),
                s.engine.pattern,
                s.engine.row,
                s.engine.playing,
            )
        });
        let (channels, names, npat, mixer) = self.with(|s| {
            (
                s.song.channels,
                s.song.channel_names.clone(),
                s.song.patterns.len(),
                s.song.mixer.clone(),
            )
        });
        if playing && self.follow {
            self.pattern = play_pat.min(npat - 1);
            self.row = play_row;
        }
        let Some(pat) = song_snapshot else { return };
        // one line: block controls and transpose on the left, channels / chords /
        // full height on the right, the order list in between (clipped when it
        // does not fit). Narrower windows move the right group to a second
        // line, then transpose too (a third line only in very narrow windows).
        const RIGHT_W: f32 = 430.0; // the right-hand group
        const TRANSPOSE_W: f32 = 270.0; // separator, "Transpose", -12 -1 +1 +12
        let (mut wrap, mut tr_later) = (false, false);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Block").strong());
            if ui.button("<").clicked() && self.pattern > 0 {
                self.pattern -= 1;
            }
            ui.label(
                egui::RichText::new(format!("{:02} {}", self.pattern, pat.name))
                    .monospace()
                    .color(th().value)
                    .strong(),
            );
            if ui.button(">").clicked() && self.pattern + 1 < npat {
                self.pattern += 1;
            }
            self.block_len_controls(ui, pat.rows);
            if ui
                .button("+ New block")
                .on_hover_text("add an empty block at the end of the order")
                .clicked()
            {
                self.add_block();
            }
            ui.separator();
            let w = ui.available_width();
            if w >= TRANSPOSE_W {
                self.transpose_bar(ui);
                ui.separator();
            } else {
                tr_later = true;
            }
            if !tr_later && ui.available_width() >= RIGHT_W {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.block_bar_right(ui, channels);
                });
            } else {
                wrap = true;
            }
        });
        if tr_later && ui.available_width() < TRANSPOSE_W + RIGHT_W {
            ui.horizontal(|ui| self.transpose_bar(ui));
            tr_later = false;
        }
        if wrap {
            ui.horizontal(|ui| {
                if tr_later {
                    self.transpose_bar(ui);
                    ui.separator();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.block_bar_right(ui, channels);
                });
            });
        }

        let full = ui.available_rect_before_wrap();
        let font = FontId::monospace(14.0);
        let cw = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
        let chan_w = cw * 15.0 + 10.0; // "C-4 01 40 0C20" + gap
        let x0_of = |r: Rect| r.left() + 10.0 + cw * 3.0 + 8.0;
        // horizontal channel scroll: keep the cursor channel in view; a scroll
        // bar appears under the grid when the channels don't all fit
        let max_ch_vis = (((full.right() - x0_of(full)) / chan_w) as usize).max(1);
        let scroll = channels > max_ch_vis;
        let bar_h = if scroll { 16.0 } else { 0.0 };
        let avail = Rect::from_min_max(full.min, Pos2::new(full.right(), full.bottom() - bar_h));
        let resp = ui.allocate_rect(avail, Sense::click_and_drag());
        let max_first = channels.saturating_sub(max_ch_vis);
        let mut first_ch = self.ch_view.min(max_first);
        self.ch_bar = Rect::NOTHING;
        if scroll {
            let bar = Rect::from_min_max(Pos2::new(x0_of(full), avail.bottom() + 3.0), full.max);
            let br = ui.allocate_rect(bar, Sense::click_and_drag());
            self.ch_bar = bar;
            if br.is_pointer_button_down_on() {
                if let Some(p) = br.interact_pointer_pos() {
                    let f = ((p.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                    let want = (f * channels as f32 - max_ch_vis as f32 / 2.0).round();
                    first_ch = (want.max(0.0) as usize).min(max_first);
                    // the cursor follows the view
                    self.ch = self.ch.clamp(first_ch, first_ch + max_ch_vis - 1);
                }
            }
        }
        if self.ch < first_ch {
            first_ch = self.ch;
        } else if self.ch >= first_ch + max_ch_vis {
            first_ch = self.ch + 1 - max_ch_vis;
        }
        self.ch_view = first_ch;
        if scroll {
            let bar = Rect::from_min_max(Pos2::new(x0_of(full), avail.bottom() + 3.0), full.max);
            let p = ui.painter_at(bar);
            p.rect_filled(bar, 4, th().crt_grid);
            let w = bar.width() / channels as f32;
            let thumb = Rect::from_min_size(
                Pos2::new(bar.left() + first_ch as f32 * w, bar.top()),
                egui::vec2(w * max_ch_vis as f32, bar.height()),
            );
            p.rect_filled(thumb, 4, th().slate);
            p.text(
                bar.center(),
                egui::Align2::CENTER_CENTER,
                format!(
                    "channels {}–{} of {}",
                    first_ch + 1,
                    (first_ch + max_ch_vis).min(channels),
                    channels
                ),
                FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
        let sel = self.sel_rect();
        let painter = ui.painter_at(avail);
        theme::crt(&painter, avail);
        let rh = 19.0;
        let x0 = x0_of(avail);
        let header_y = avail.top() + 6.0;
        let top = header_y + 26.0;
        let visible = (((avail.bottom() - top) / rh) as usize).max(1);
        let start = (self.row as isize - visible as isize / 2)
            .clamp(0, (pat.rows as isize - visible as isize).max(0)) as usize;

        // column backgrounds under everything row-based: every other channel
        // (by channel index, so the stagger stays put while scrolling) gets
        // a theme-derived tint, a 1 px divider between columns, and a faint
        // accent wash on the cursor's channel
        let alt = self.extra.alt_columns;
        let tint = theme::shade(&th(), 0.05);
        let line = theme::shade(&th(), 0.12);
        let rows_vis = visible.min(pat.rows.saturating_sub(start));
        let grid_bot = (top + rows_vis as f32 * rh).min(avail.bottom() - 2.0);
        for (vi, ch) in (first_ch..channels).enumerate() {
            let x = x0 + vi as f32 * chan_w - 5.0;
            if x > avail.right() {
                break;
            }
            let col = Rect::from_min_max(
                Pos2::new(x, top),
                Pos2::new((x + chan_w).min(avail.right() - 2.0), grid_bot),
            );
            if alt && ch % 2 == 1 {
                painter.rect_filled(col, 0, tint);
            }
            if ch == self.ch {
                painter.rect_filled(col, 0, th().accent.gamma_multiply(0.07));
            }
            if alt && vi > 0 {
                painter.vline(x.round() + 0.5, top..=grid_bot, Stroke::new(1.0, line));
            }
        }
        for (vi, ch) in (first_ch..channels).enumerate() {
            let x = x0 + vi as f32 * chan_w;
            if x > avail.right() {
                break;
            }
            let hr = Rect::from_min_size(Pos2::new(x, header_y), egui::vec2(chan_w - 8.0, 22.0));
            painter.rect_filled(
                hr,
                6,
                if !mixer.audible(ch) {
                    hex(0x4A525E)
                } else {
                    th().slate
                },
            );
            if alt && ch % 2 == 1 {
                painter.rect_filled(hr, 6, tint);
            }
            painter.text(
                hr.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("{} {}", ch + 1, names.get(ch).cloned().unwrap_or_default()),
                FontId::proportional(13.0),
                Color32::WHITE,
            );
            if ch == self.ch {
                // current channel: an accent underline on its header
                painter.hline(
                    hr.x_range().shrink(4.0),
                    hr.bottom() + 1.0,
                    Stroke::new(2.0, th().accent),
                );
            }
            painter.text(
                hr.right_center() - egui::vec2(6.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                if mixer.channels[ch].solo {
                    "SOLO"
                } else if mixer.channels[ch].mute {
                    "MUTED"
                } else {
                    ""
                },
                FontId::proportional(11.0),
                hex(0xFFB4AE),
            );
        }
        for vi in 0..visible {
            let r = start + vi;
            if r >= pat.rows {
                break;
            }
            let y = top + vi as f32 * rh;
            let row_rect = Rect::from_min_size(
                Pos2::new(avail.left() + 4.0, y),
                egui::vec2(avail.width() - 8.0, rh),
            );
            if r.is_multiple_of(16) {
                painter.rect_filled(row_rect, 0, th().crt_grid.gamma_multiply(0.9));
            } else if r.is_multiple_of(4) {
                painter.rect_filled(row_rect, 0, th().crt_grid.gamma_multiply(0.55));
            }
            if playing && play_pat == self.pattern && r == play_row {
                painter.rect_stroke(
                    row_rect,
                    3,
                    Stroke::new(1.5, th().play),
                    egui::StrokeKind::Inside,
                );
            }
            if r == self.row {
                if theme::polished() {
                    // a soft cursor row: a gentle band with faint edges
                    let t = th();
                    painter.rect_filled(row_rect, 3, t.slate.gamma_multiply(0.6));
                    painter.hline(
                        row_rect.x_range().shrink(3.0),
                        row_rect.top() + 0.5,
                        Stroke::new(1.0, t.accent.gamma_multiply(0.35)),
                    );
                    painter.hline(
                        row_rect.x_range().shrink(3.0),
                        row_rect.bottom() - 0.5,
                        Stroke::new(1.0, t.accent.gamma_multiply(0.35)),
                    );
                } else {
                    painter.rect_filled(row_rect, 4, th().slate.gamma_multiply(0.7));
                }
            }
            if let Some((r0, r1, c0, c1)) = sel {
                if (r0..=r1).contains(&r) {
                    for (vi_c, ch) in (first_ch..channels).enumerate() {
                        if (c0..=c1).contains(&ch) {
                            let x = x0 + vi_c as f32 * chan_w - 3.0;
                            let sr =
                                Rect::from_min_size(Pos2::new(x, y), egui::vec2(chan_w - 4.0, rh));
                            painter.rect_filled(sr, 0, th().sel);
                            let st = Stroke::new(1.5, th().accent);
                            if r == r0 {
                                painter.hline(sr.x_range(), sr.top(), st);
                            }
                            if r == r1 {
                                painter.hline(sr.x_range(), sr.bottom(), st);
                            }
                            if ch == c0 {
                                painter.vline(sr.left(), sr.y_range(), st);
                            }
                            if ch == c1 {
                                painter.vline(sr.right(), sr.y_range(), st);
                            }
                        }
                    }
                }
            }
            painter.text(
                Pos2::new(avail.left() + 10.0, y + 2.0),
                egui::Align2::LEFT_TOP,
                self.row_text(r, pat.rows),
                font.clone(),
                if r.is_multiple_of(4) {
                    th().fx
                } else {
                    hex(0x7FA6D9)
                },
            );
            for (vi_c, ch) in (first_ch..channels).enumerate() {
                let x = x0 + vi_c as f32 * chan_w;
                if x > avail.right() {
                    break;
                }
                let c = pat.cells[r][ch];
                let parts: [(String, Color32, usize); 4] = [
                    (
                        note_name(c.note),
                        if c.note == 0 { th().empty } else { th().note },
                        0,
                    ),
                    (
                        if c.instr == 0 {
                            "..".into()
                        } else {
                            format!("{:02X}", c.instr)
                        },
                        if c.instr == 0 { th().empty } else { th().instr },
                        4,
                    ),
                    (
                        if c.vol == 0 {
                            "..".into()
                        } else {
                            format!("{:02X}", c.vol - 1)
                        },
                        if c.vol == 0 { th().empty } else { th().vol },
                        7,
                    ),
                    (
                        if c.fx == 0 && c.fxp == 0 {
                            "....".into()
                        } else {
                            format!("{:02X}{:02X}", c.fx, c.fxp)
                        },
                        if c.fx == 0 && c.fxp == 0 {
                            th().empty
                        } else {
                            th().fx
                        },
                        10,
                    ),
                ];
                // muted / not-soloed channels: dimmed cells
                let dim = !mixer.audible(ch);
                for (t, col, off) in parts {
                    painter.text(
                        Pos2::new(x + off as f32 * cw, y + 2.0),
                        egui::Align2::LEFT_TOP,
                        t,
                        font.clone(),
                        if dim { col.gamma_multiply(0.45) } else { col },
                    );
                }
                if r == self.row && ch == self.ch {
                    let (off, w) = match self.col {
                        0 => (0.0, 3.0),
                        1 => (4.0, 1.0),
                        2 => (5.0, 1.0),
                        3 => (7.0, 1.0),
                        4 => (8.0, 1.0),
                        k => (10.0 + (k - 5) as f32, 1.0),
                    };
                    let cr = Rect::from_min_size(
                        Pos2::new(x + off * cw - 1.0, y),
                        egui::vec2(w * cw + 2.0, rh),
                    );
                    painter.rect_stroke(
                        cr,
                        3,
                        Stroke::new(2.0, if self.edit { th().fx } else { Color32::WHITE }),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
        let cell_at = |p: Pos2| -> Pos {
            let vr = ((p.y - top) / rh).floor().max(0.0);
            let vc = ((p.x - x0) / chan_w).floor().max(0.0);
            (
                (start + vr as usize).min(pat.rows - 1),
                (first_ch + vc as usize).min(channels - 1),
            )
        };
        if resp.clicked() || resp.drag_started() {
            self.arr_focus = false;
        }
        // mouse wheel: 1 row per notch (Shift = 16 rows); line and pixel deltas
        if resp.hovered() {
            let ev = ui.input(|i| i.events.clone());
            let n = input::take_notches(&mut self.wheel_pat, input::wheel_lines(&ev));
            if n != 0 {
                let per = if ui.input(|i| i.modifiers.shift) {
                    16
                } else {
                    1
                };
                let last = pat.rows as isize - 1;
                self.row = (self.row as isize - n as isize * per).clamp(0, last) as usize;
                self.sel = None;
            }
        }
        if self.view == View::Pattern && self.arr_docked() && !self.arr_focus {
            focus_ring(&ui.painter_at(avail.expand(6.0)), avail.expand(2.0));
        }
        if resp.drag_started() {
            if let Some(o) = ui.input(|i| i.pointer.press_origin()) {
                self.drag_anchor = Some(cell_at(o));
            }
        }
        if resp.dragged() {
            if let (Some(a), Some(p)) = (self.drag_anchor, resp.interact_pointer_pos()) {
                let c = cell_at(p);
                self.row = c.0;
                self.ch = c.1;
                self.sel = Some((a, c));
                // auto-scroll when dragging past the edges
                if p.y > avail.bottom() - rh {
                    self.row = (self.row + 1).min(pat.rows - 1);
                    self.sel = Some((a, (self.row, self.ch)));
                }
            }
        }
        if resp.drag_stopped() {
            self.drag_anchor = None;
        }
        if resp.clicked() {
            self.sel = None;
            if let Some(p) = resp.interact_pointer_pos() {
                let vr = ((p.y - top) / rh).floor();
                if vr >= 0.0 {
                    self.row = (start + vr as usize).min(pat.rows - 1);
                }
                let vc = ((p.x - x0) / chan_w).floor();
                if vc >= 0.0 {
                    self.ch = (first_ch + vc as usize).min(channels - 1);
                    let within = (p.x - x0 - vc * chan_w) / cw;
                    self.col = match within as usize {
                        0..=3 => 0,
                        4 => 1,
                        5 | 6 => 2,
                        7 => 3,
                        8 | 9 => 4,
                        10 => 5,
                        11 => 6,
                        12 => 7,
                        _ => 8,
                    };
                }
            }
            ui.memory_mut(|m| m.surrender_focus(m.focused().unwrap_or(egui::Id::NULL)));
        }
    }

    // ---------------------------------------------------------------- arranger
}

impl App {
    /// Block bar, right side (drawn right to left): Full height, Chords…,
    /// Channels, -last / +cur and the order list (clipped)
    fn block_bar_right(&mut self, ui: &mut egui::Ui, channels: usize) {
        let full = !self.lower_visible();
        if ui
            .selectable_label(full, "Full height")
            .on_hover_text(if full {
                "show the lower panel again (F11)"
            } else {
                "Full-height pattern: hide the lower panel (F11)"
            })
            .clicked()
        {
            self.toggle_full_height();
        }
        ui.separator();
        if let Some(k) = self.scale_lock_key() {
            ui.label(
                egui::RichText::new(format!("lock {}", k.name()))
                    .small()
                    .color(th().accent),
            )
            .on_hover_text("scale lock: typed and MIDI notes snap to this key (Chord helper)");
        }
        if ui
            .button("Chords…")
            .on_hover_text(
                "chord helper: song key, chord pads, progressions into this block, scale lock",
            )
            .clicked()
        {
            self.chords.open = !self.chords.open;
        }
        ui.separator();
        let mut ch = channels;
        if ui
            .add(egui::DragValue::new(&mut ch).range(1..=MAX_CHANNELS))
            .on_hover_text(super::CHANNELS_TIP)
            .changed()
        {
            self.set_channels(ch);
        }
        ui.label("Channels");
        ui.separator();
        if ui.small_button("-last").clicked() {
            self.with(|s| {
                if s.song.order.len() > 1 {
                    s.song.order.pop();
                }
            });
            self.dirty = true;
        }
        if ui
            .small_button("+cur")
            .on_hover_text("append current block to the order list")
            .clicked()
        {
            let p = self.pattern;
            self.with(|s| s.song.order.push(p));
            self.dirty = true;
        }
        let order = self.with(|s| s.song.order.clone());
        let txt = format!(
            "Order: {}",
            order
                .iter()
                .map(|o| format!("{o:02}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&txt).monospace()).truncate())
                .on_hover_text(txt.clone());
        });
    }
}
