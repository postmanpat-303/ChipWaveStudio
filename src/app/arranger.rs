//! Arranger: clip selection, insert/duplicate/copy/paste/delete/move, and the arranger view.
use super::*;

impl App {
    pub(super) fn arr_focused(&self) -> bool {
        self.view == View::Arranger
            || (self.view == View::Pattern && self.arr_docked() && self.arr_focus)
    }

    pub(super) fn order_len(&self) -> usize {
        self.with(|s| s.song.order.len())
    }

    /// selected song-order indices, sorted (never empty)
    pub(super) fn arr_sel_list(&self) -> Vec<usize> {
        let len = self.order_len();
        let mut v: Vec<usize> = self
            .arr_multi
            .iter()
            .copied()
            .filter(|&i| i < len)
            .collect();
        if v.is_empty() {
            v.push(self.arr_sel.min(len - 1));
        }
        v
    }

    pub(super) fn arr_select(&mut self, idx: impl IntoIterator<Item = usize>, anchor: usize) {
        let len = self.order_len();
        self.arr_multi = idx.into_iter().filter(|&i| i < len).collect();
        self.arr_sel = anchor.min(len - 1);
        self.arr_multi.insert(self.arr_sel);
        self.pattern = self.with(|s| s.song.order[self.arr_sel]);
        self.row = self.row.min(self.rows() - 1);
    }

    /// Move the play marker to (song-order entry, row): the pattern editor
    /// shows that block and row, and running song playback jumps there. The
    /// arranger's clip selection is left alone.
    pub(super) fn seek_to(&mut self, t: (usize, usize)) {
        let Some(b) = self.with(|s| s.song.order.get(t.0).copied()) else {
            return;
        };
        self.cue = t;
        self.pattern = b;
        self.row = t.1.min(self.rows().saturating_sub(1));
        self.with(|s| {
            let Shared {
                song,
                engine,
                preview,
            } = s;
            if engine.playing && preview.is_none() {
                engine.seek(song, t.0, t.1);
            }
        });
        let rows = self.rows();
        self.status = format!(
            "Play marker: entry {} · block {b:02} · row {}",
            t.0 + 1,
            self.row_text(t.1, rows)
        );
    }

    pub(super) fn arr_flash_now(&mut self, idx: Vec<usize>, ctx: &egui::Context) {
        self.arr_flash = Some((idx, ctx.input(|i| i.time)));
    }

    /// insert block ids after the last selected entry (optionally as new copies)
    pub(super) fn arr_insert_blocks(
        &mut self,
        blocks: &[usize],
        unique: bool,
        ctx: &egui::Context,
    ) -> usize {
        let npat = self.with(|s| s.song.patterns.len());
        let blocks: Vec<usize> = blocks.iter().copied().filter(|&b| b < npat).collect();
        if blocks.is_empty() {
            return 0;
        }
        self.snapshot();
        let at = self.arr_sel_list().last().copied().unwrap_or(0) + 1;
        self.with(|s| {
            let song = &mut s.song;
            let at = at.min(song.order.len());
            for (k, &b) in blocks.iter().enumerate() {
                let b = if unique {
                    let mut p = song.patterns[b].clone();
                    p.name = format!("{} copy", p.name);
                    song.patterns.push(p);
                    song.patterns.len() - 1
                } else {
                    b
                };
                song.order.insert(at + k, b);
            }
        });
        let range: Vec<usize> = (at..at + blocks.len()).collect();
        self.arr_select(range.clone(), at + blocks.len() - 1);
        self.arr_flash_now(range, ctx);
        blocks.len()
    }

    pub(super) fn arr_insert(&mut self, ctx: &egui::Context) {
        self.snapshot();
        let n = self.with(|s| {
            let n = s.song.patterns.len();
            let ch = s.song.channels;
            s.song
                .patterns
                .push(Pattern::new(&format!("Block {n:02}"), ROWS, ch));
            n
        });
        // the snapshot above already covers this: one undo step for both parts
        self.undo_hold += 1;
        self.arr_insert_blocks(&[n], false, ctx);
        self.undo_hold -= 1;
        self.status = format!("Inserted new empty block {n:02} at #{}", self.arr_sel + 1);
    }

    pub(super) fn arr_duplicate(&mut self, unique: bool, ctx: &egui::Context) {
        let order = self.with(|s| s.song.order.clone());
        let blocks: Vec<usize> = self.arr_sel_list().iter().map(|&i| order[i]).collect();
        let n = self.arr_insert_blocks(&blocks, unique, ctx);
        self.status = format!(
            "{} {n} clip(s) -> #{}",
            if unique {
                "Duplicated as new block copies:"
            } else {
                "Duplicated (same blocks):"
            },
            self.arr_sel + 2 - n
        );
    }

    pub(super) fn arr_copy(&mut self, cut: bool, ctx: &egui::Context) {
        let order = self.with(|s| s.song.order.clone());
        let idx = self.arr_sel_list();
        self.arr_clip = idx.iter().map(|&i| order[i]).collect();
        // system clipboard text so Ctrl+V delivers Event::Paste on Windows
        ctx.copy_text(input::arr_clip_to_text(&self.arr_clip));
        let names: Vec<String> = self.arr_clip.iter().map(|b| format!("{b:02}")).collect();
        if cut {
            if idx.len() >= order.len() {
                self.status = format!(
                    "Copied {} clip(s) [{}] - can't cut every clip, the song needs one entry",
                    idx.len(),
                    names.join(" ")
                );
                return;
            }
            self.arr_remove(&idx);
            self.status = format!("Cut {} clip(s) [{}]", idx.len(), names.join(" "));
        } else {
            self.arr_flash_now(idx.clone(), ctx);
            self.status = format!("Copied {} clip(s) [{}]", idx.len(), names.join(" "));
        }
    }

    pub(super) fn arr_paste(&mut self, text: Option<String>, unique: bool, ctx: &egui::Context) {
        if let Some(c) = text.as_deref().and_then(input::arr_clip_from_text) {
            self.arr_clip = c;
        }
        if self.arr_clip.is_empty() {
            self.status = "Order list clipboard is empty (select clips, Ctrl+C)".into();
            return;
        }
        let clip = self.arr_clip.clone();
        let n = self.arr_insert_blocks(&clip, unique, ctx);
        self.status = if n == 0 {
            "Nothing pasted (blocks not in this song)".into()
        } else {
            format!(
                "Pasted {n} clip(s){} at #{}",
                if unique { " as new block copies" } else { "" },
                self.arr_sel + 2 - n
            )
        };
    }

    pub(super) fn arr_remove(&mut self, idx: &[usize]) {
        self.snapshot();
        let first = idx.first().copied().unwrap_or(0);
        self.with(|s| {
            for &i in idx.iter().rev() {
                if i < s.song.order.len() {
                    s.song.order.remove(i);
                }
            }
        });
        let len = self.order_len();
        let a = first.min(len - 1);
        self.arr_select([a], a);
    }

    pub(super) fn arr_delete(&mut self) {
        let idx = self.arr_sel_list();
        if idx.len() >= self.order_len() {
            self.status = "The song needs at least one entry".into();
            return;
        }
        self.arr_remove(&idx);
        self.status = format!("Removed {} clip(s) (blocks stay in the pool)", idx.len());
    }

    pub(super) fn arr_move(&mut self, from: usize, to: usize) {
        if from == to || to == from + 1 {
            return;
        }
        self.snapshot();
        let to = self.with(|s| {
            let b = s.song.order.remove(from);
            let to = if to > from { to - 1 } else { to };
            let to = to.min(s.song.order.len());
            s.song.order.insert(to, b);
            to
        });
        self.arr_select([to], to);
        self.status = "Moved clip".into();
    }

    pub(super) fn arr_zoom(&mut self, f: f32) {
        self.with(|s| s.song.arranger_zoom = (s.song.arranger_zoom * f).clamp(0.25, 16.0));
    }

    pub(super) fn arranger_action(&mut self, a: input::ArrAction, ctx: &egui::Context) {
        self.arranger_action_text(a, None, ctx)
    }

    pub(super) fn arranger_action_text(
        &mut self,
        a: input::ArrAction,
        text: Option<String>,
        ctx: &egui::Context,
    ) {
        use input::ArrAction::*;
        let len = self.order_len();
        let cur = self.arr_sel.min(len - 1);
        let ends = self.arr_sel_list();
        match a {
            Prev => self.arr_select([cur.saturating_sub(1)], cur.saturating_sub(1)),
            Next => self.arr_select([(cur + 1).min(len - 1)], (cur + 1).min(len - 1)),
            ExtendPrev | ExtendNext => {
                let (lo, hi) = (ends[0], *ends.last().unwrap());
                let (lo, hi) = if a == ExtendPrev {
                    (lo.saturating_sub(1), hi)
                } else {
                    (lo, (hi + 1).min(len - 1))
                };
                self.arr_select(lo..=hi, if a == ExtendPrev { lo } else { hi });
            }
            SelectAll => {
                self.arr_select(0..len, cur);
                self.status = format!("Selected all {len} clips");
            }
            Delete => self.arr_delete(),
            Insert => self.arr_insert(ctx),
            Duplicate => self.arr_duplicate(false, ctx),
            DuplicateUnique => self.arr_duplicate(true, ctx),
            ZoomIn => self.arr_zoom(1.25),
            ZoomOut => self.arr_zoom(0.8),
            Copy => self.arr_copy(false, ctx),
            Cut => self.arr_copy(true, ctx),
            Paste => self.arr_paste(text, false, ctx),
            PasteUnique => self.arr_paste(text, true, ctx),
            Role(r) => self.set_role(r),
        }
        if matches!(a, Prev | Next) {
            self.row = 0;
        }
    }

    #[allow(clippy::type_complexity)]
    pub(super) fn arranger_view(&mut self, ui: &mut egui::Ui) {
        if self.with(|s| s.song.timeline.is_some()) {
            // timeline mode: the order list does not play; say so
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(
                        "This song is in timeline mode: the Timeline (Ctrl+T) decides what plays, \
                         not this order list.",
                    )
                    .color(th().ochre),
                );
                if ui.small_button("Open the Timeline").clicked() {
                    self.view = View::Timeline;
                }
            });
        }
        let (order, blocks, zoom, playing, loop_pat, opos, prow, ppat) = self.with(|s| {
            #[allow(clippy::type_complexity)]
            let blocks: Vec<(
                String,
                usize,
                Vec<(usize, usize)>,
                Option<crate::arrange::Role>,
                bool,
            )> = s
                .song
                .patterns
                .iter()
                .map(|p| {
                    let mut ticks = vec![];
                    for (r, row) in p.cells.iter().enumerate() {
                        for (c, cell) in row.iter().enumerate().take(s.song.channels) {
                            if cell.note > 0 && cell.note != NOTE_OFF {
                                ticks.push((r, c));
                            }
                        }
                    }
                    (p.name.clone(), p.rows, ticks, p.role, p.derived.is_some())
                })
                .collect();
            (
                s.song.order.clone(),
                blocks,
                s.song.arranger_zoom,
                s.engine.playing,
                s.engine.loop_pattern,
                s.engine.order_pos,
                s.engine.row,
                s.engine.pattern,
            )
        });
        let channels = self.with(|s| s.song.channels).max(1);
        self.arr_sel = self.arr_sel.min(order.len() - 1);
        if self.arr_focused() {
            focus_ring(ui.painter(), ui.max_rect().expand(3.0));
        }
        ui.horizontal_wrapped(|ui| {
            if self.arr_focused() {
                ui.label(
                    egui::RichText::new(" ORDER LIST ")
                        .strong()
                        .monospace()
                        .color(Color32::from_rgb(0x1A, 0x1C, 0x1F))
                        .background_color(th().focus_ring),
                )
                .on_hover_text("keyboard focus: Ctrl+X/C/V, Del, arrows act on clips");
            } else {
                ui.label(egui::RichText::new("Order list").strong().color(th().ink2))
                    .on_hover_text("click to give the Order list keyboard focus");
            }
            ui.label(egui::RichText::new("zoom").color(th().ink2));
            if ui.small_button("-").clicked() {
                self.arr_zoom(0.8);
            }
            if ui.small_button("+").clicked() {
                self.arr_zoom(1.25);
            }
            ui.separator();
            if ui.button("Insert block").on_hover_text("Ins").clicked() {
                self.arranger_action(input::ArrAction::Insert, &ui.ctx().clone());
            }
            if ui.button("Duplicate").on_hover_text("Ctrl+D").clicked() {
                self.arranger_action(input::ArrAction::Duplicate, &ui.ctx().clone());
            }
            if ui
                .button("Copy as new")
                .on_hover_text("Ctrl+Shift+D: unique copy of the block")
                .clicked()
            {
                self.arranger_action(input::ArrAction::DuplicateUnique, &ui.ctx().clone());
            }
            if ui.button("Delete").on_hover_text("Del").clicked() {
                self.arr_delete();
            }
            ui.separator();
            for (t, h, a) in [
                ("Cut", "Ctrl+X", input::ArrAction::Cut),
                ("Copy", "Ctrl+C", input::ArrAction::Copy),
                (
                    "Paste",
                    "Ctrl+V (Ctrl+Shift+V = as new blocks)",
                    input::ArrAction::Paste,
                ),
            ] {
                if ui.small_button(t).on_hover_text(h).clicked() {
                    self.arr_focus = true;
                    self.arranger_action(a, &ui.ctx().clone());
                }
            }
            ui.checkbox(&mut self.follow, "Follow");
            ui.label(
                egui::RichText::new(format!(
                    "{} entries · drag clips to reorder · click = open in editor · click the ruler = move the play marker",
                    order.len()
                ))
                .small()
                .color(th().ink2),
            );
        });
        self.automation_bar(ui);
        // clip x positions (in rows)
        let mut starts = Vec::with_capacity(order.len() + 1);
        let mut acc = 0usize;
        for &b in &order {
            starts.push(acc);
            acc += blocks.get(b).map(|x| x.1).unwrap_or(ROWS);
        }
        starts.push(acc);
        let pad = 8.0;
        let avail_h = ui.available_height().max(80.0);
        let follow = self.follow && playing;
        let (lanes, song_pos, fxk) = self.with(|s| {
            (
                s.song.automation.clone(),
                s.engine.song_pos,
                s.song
                    .instruments
                    .iter()
                    .map(|i| crate::automation::Units {
                        fx: i.fx.iter().map(|d| d.kind).collect(),
                        gen: crate::automation::Units::gen_of(&i.kind),
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let show_auto = self.show_auto;
        let n_lanes = if show_auto { lanes.len() } else { 0 };
        let lanes_h = n_lanes as f32 * (LANE_H + 4.0) + if n_lanes > 0 { 4.0 } else { 0.0 };
        let clip_h = (avail_h - 14.0 - lanes_h).max(70.0);
        // mouse wheel: scroll horizontally; Ctrl+wheel zooms around the pointer
        let hover_x = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| self.arr_area.contains(*p))
            .map(|p| p.x);
        if let Some(px) = hover_x {
            let ev = ui.input(|i| i.events.clone());
            let lines = input::wheel_lines(&ev);
            if lines != 0.0 {
                let ctrl = ui.input(|i| i.modifiers.command);
                if ctrl {
                    let n = input::take_notches(&mut self.wheel_arr, lines);
                    if n != 0 {
                        let z0 = zoom;
                        self.arr_zoom(1.15f32.powi(n));
                        let z1 = self.with(|s| s.song.arranger_zoom);
                        let rel = px - self.arr_area.left();
                        let off = ((self.arr_scroll + rel - pad) * z1 / z0 + pad - rel).max(0.0);
                        self.arr_scroll_set = Some(off);
                    }
                } else {
                    // smooth: pixels follow the wheel directly (40 px per line)
                    let off = (self.arr_scroll - lines * 40.0).max(0.0);
                    self.arr_scroll_set = Some(off);
                }
            }
        }
        let zoom = self.with(|s| s.song.arranger_zoom);
        let ghost_rows = self
            .fill
            .as_ref()
            .and_then(|f| f.prop.as_ref())
            .map(|p| p.rows);
        let total_w = acc.max(ghost_rows.unwrap_or(0)) as f32 * zoom + pad * 2.0;
        let mut lane_ops: Vec<LaneOp> = vec![];
        let mut sa = egui::ScrollArea::both()
            .id_salt("arranger_scroll")
            .auto_shrink([false, false])
            .scroll_source(egui::containers::scroll_area::ScrollSource {
                mouse_wheel: false,
                ..Default::default()
            });
        if let Some(off) = self.arr_scroll_set.take() {
            sa = sa.horizontal_scroll_offset(off);
        }
        let out = sa.show(ui, |ui| {
            let w = total_w.max(ui.available_width());
            let (rect, bg) = ui.allocate_exact_size(egui::vec2(w, clip_h), Sense::click());
            if bg.clicked() || bg.secondary_clicked() {
                self.arr_focus = true;
            }
            let painter = ui.painter_at(rect);
            theme::crt(&painter, rect);
            let ruler_h = 16.0;
            let full_lane = Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + ruler_h + 4.0),
                Pos2::new(rect.right(), rect.bottom() - 6.0),
            );
            // with a Fill-to-length proposal: real clips on top, ghost clips below
            let (lane, ghost) = if ghost_rows.is_some() {
                let h = full_lane.height();
                (
                    Rect::from_min_max(
                        full_lane.min,
                        Pos2::new(full_lane.right(), full_lane.top() + h * 0.5),
                    ),
                    Some(Rect::from_min_max(
                        Pos2::new(full_lane.left(), full_lane.top() + h * 0.5 + 14.0),
                        full_lane.max,
                    )),
                )
            } else {
                (full_lane, None)
            };
            if let Some(g) = ghost {
                let names: Vec<String> = blocks.iter().map(|b| b.0.clone()).collect();
                self.ghost_lane(&painter, g, rect.left() + pad, zoom, &names);
            }
            let xof = |rows: usize| rect.left() + pad + rows as f32 * zoom;
            // ruler: a tick every 16 rows, labels every 64
            let step = if zoom < 1.0 { 64 } else { 16 };
            let mut r = 0;
            while r <= acc {
                let x = xof(r);
                let major = r % 64 == 0;
                painter.vline(
                    x,
                    rect.top() + if major { 2.0 } else { 9.0 }..=rect.top() + ruler_h,
                    Stroke::new(1.0, th().ink2.gamma_multiply(0.6)),
                );
                if major && zoom * 64.0 > 28.0 {
                    painter.text(
                        Pos2::new(x + 3.0, rect.top() + 1.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", r / 16 + 1),
                        FontId::monospace(10.0),
                        th().ink2,
                    );
                }
                r += step;
            }
            // ruler: click = move the play marker there, drag = scrub,
            // Alt = exact row (no bar snap). Clip selection is untouched.
            let ruler = Rect::from_min_max(
                rect.min,
                Pos2::new(rect.right(), rect.top() + ruler_h + 3.0),
            );
            self.arr_ruler = ruler;
            self.arr_x0 = xof(0);
            let rresp = ui
                .interact(ruler, ui.scope_id().with("ruler"), Sense::click_and_drag())
                .on_hover_text(
                    "click: move the play marker here (jumps while playing) · drag: scrub · Alt: exact row",
                );
            if rresp.hovered() || rresp.dragged() {
                painter.rect_filled(ruler, 2, th().ink2.gamma_multiply(0.12));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if rresp.drag_started() {
                self.ruler_drag = true;
            }
            if !rresp.dragged() && !rresp.drag_started() {
                self.ruler_drag = false;
            }
            if rresp.clicked() || rresp.dragged() || rresp.drag_started() {
                if let Some(p) = rresp.interact_pointer_pos() {
                    let lens: Vec<usize> = starts.windows(2).map(|w| w[1] - w[0]).collect();
                    let abs = ((p.x - xof(0)) / zoom).max(0.0) as usize;
                    let alt = ui.input(|i| i.modifiers.alt);
                    let t = ruler_target(&lens, abs, !alt);
                    if rresp.clicked() || rresp.drag_started() || t != self.cue {
                        self.seek_to(t);
                    }
                    self.arr_focus = true;
                }
            }
            let pointer = ui.input(|i| i.pointer.interact_pos());
            let mods = ui.input(|i| i.modifiers);
            let selected = self.arr_sel_list();
            let mut menu_act: Option<input::ArrAction> = None;
            let mut bake = false;
            let released = ui.input(|i| i.pointer.any_released());
            for (i, &b) in order.iter().enumerate() {
                let cr = Rect::from_min_max(
                    Pos2::new(xof(starts[i]) + 1.0, lane.top()),
                    Pos2::new(xof(starts[i + 1]) - 1.0, lane.bottom()),
                );
                let resp =
                    ui.interact(cr, ui.scope_id().with(("clip", i)), Sense::click_and_drag());
                let col = hex(CLIP_COLOURS[b % CLIP_COLOURS.len()]);
                let col = if resp.hovered() {
                    col.gamma_multiply(1.15)
                } else {
                    col
                };
                let is_sel = selected.contains(&i);
                let col = if is_sel { col.gamma_multiply(1.3) } else { col };
                let pol = theme::polished();
                let crad: u8 = if pol { 3 } else { 6 };
                if pol {
                    if is_sel {
                        theme::glow(&painter, cr, crad, th().selection_outline);
                    }
                    theme::block(&painter, cr, col);
                } else {
                    painter.rect_filled(cr, 6, col);
                }
                if let Some((fi, t0)) = &self.arr_flash {
                    let age = ui.input(|inp| inp.time) - t0;
                    if fi.contains(&i) && age < 0.8 {
                        painter.rect_filled(
                            cr,
                            crad,
                            Color32::from_white_alpha((90.0 * (1.0 - age / 0.8)) as u8),
                        );
                        ui.ctx().request_repaint();
                    }
                }
                if is_sel && pol {
                    // soft glow (above) + a crisp 2 px outline
                    painter.rect_stroke(
                        cr,
                        crad,
                        Stroke::new(2.0, th().selection_outline),
                        egui::StrokeKind::Inside,
                    );
                } else if is_sel {
                    // inner shadow, then a thick light outline
                    painter.rect_stroke(
                        cr.shrink(3.0),
                        4,
                        Stroke::new(2.0, Color32::from_black_alpha(90)),
                        egui::StrokeKind::Inside,
                    );
                    painter.rect_stroke(
                        cr,
                        6,
                        Stroke::new(3.0, th().selection_outline),
                        egui::StrokeKind::Inside,
                    );
                }
                // mini pattern preview: note ticks per channel
                if let Some((name, rows, ticks, role, der)) = blocks.get(b) {
                    let body = Rect::from_min_max(
                        Pos2::new(cr.left() + 4.0, cr.top() + 20.0),
                        Pos2::new(cr.right() - 4.0, cr.bottom() - 4.0),
                    );
                    let lh = (body.height() / channels as f32).max(1.0);
                    let tick = Color32::from_white_alpha(110);
                    for &(r, c) in ticks {
                        let x = body.left() + r as f32 / *rows as f32 * body.width();
                        let y = body.top() + c as f32 * lh + lh * 0.5;
                        painter.hline(
                            x..=x + (zoom * 0.8).max(1.0),
                            y,
                            Stroke::new((lh * 0.5).clamp(1.0, 3.0), tick),
                        );
                    }
                    let tp = ui.painter_at(cr.shrink(2.0));
                    tp.text(
                        cr.left_top() + egui::vec2(6.0, 3.0),
                        egui::Align2::LEFT_TOP,
                        format!("{b:02} {name}"),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                    // role badge (muted palette), top-right
                    if let Some(r) = role {
                        let txt = if cr.width() > 70.0 {
                            r.name()
                        } else {
                            r.short()
                        };
                        let w = txt.len() as f32 * 6.5 + 8.0;
                        let br = Rect::from_min_size(
                            Pos2::new(cr.right() - w - 4.0, cr.top() + 3.0),
                            egui::vec2(w, 14.0),
                        );
                        if br.left() > cr.left() + 4.0 {
                            tp.rect_filled(br, 7, fillview::role_col(*r));
                            tp.text(
                                br.center(),
                                egui::Align2::CENTER_CENTER,
                                txt,
                                FontId::proportional(10.0),
                                Color32::from_rgb(0x16, 0x18, 0x1b),
                            );
                        }
                    }
                    if *der {
                        tp.text(
                            cr.left_bottom() + egui::vec2(6.0, -3.0),
                            egui::Align2::LEFT_BOTTOM,
                            "derived'",
                            FontId::proportional(10.0),
                            Color32::from_white_alpha(200),
                        );
                    }
                }
                if resp.clicked() {
                    self.arr_focus = true;
                    if mods.shift {
                        let a = self.arr_sel.min(order.len() - 1);
                        self.arr_multi = (a.min(i)..=a.max(i)).collect();
                    } else if mods.command {
                        if !self.arr_multi.remove(&i) {
                            self.arr_multi.insert(i);
                            self.arr_sel = i;
                        }
                    } else {
                        self.arr_select([i], i);
                        self.row = 0;
                    }
                    self.pattern = b;
                    self.sel = None;
                }
                if resp.secondary_clicked() {
                    self.arr_focus = true;
                    if !selected.contains(&i) {
                        self.arr_select([i], i);
                    }
                }
                let resp = resp.on_hover_text(format!(
                    "#{} · block {:02} · {} rows",
                    i + 1,
                    b,
                    blocks.get(b).map(|x| x.1).unwrap_or(0)
                ));
                resp.context_menu(|ui| {
                    use input::ArrAction as A;
                    for (t, a) in [
                        ("Cut            Ctrl+X", A::Cut),
                        ("Copy           Ctrl+C", A::Copy),
                        ("Paste          Ctrl+V", A::Paste),
                        ("Paste as new   Ctrl+Shift+V", A::PasteUnique),
                        ("Duplicate      Ctrl+D", A::Duplicate),
                        ("Delete         Del", A::Delete),
                        ("Insert block   Ins", A::Insert),
                    ] {
                        if ui.button(egui::RichText::new(t).monospace()).clicked() {
                            menu_act = Some(a);
                            ui.close();
                        }
                    }
                    ui.separator();
                    ui.menu_button("Role", |ui| {
                        for (k, r) in crate::arrange::ROLES.iter().enumerate() {
                            let cur = blocks.get(b).and_then(|x| x.3) == Some(*r);
                            if ui
                                .selectable_label(
                                    cur,
                                    egui::RichText::new(format!("{}  {}", k + 1, r.name()))
                                        .monospace()
                                        .color(fillview::role_col(*r)),
                                )
                                .clicked()
                            {
                                menu_act = Some(A::Role(Some(*r)));
                                ui.close();
                            }
                        }
                        if ui
                            .button(egui::RichText::new("0  none").monospace())
                            .clicked()
                        {
                            menu_act = Some(A::Role(None));
                            ui.close();
                        }
                    });
                    if blocks.get(b).is_some_and(|x| x.4)
                        && ui
                            .button("Bake derived block")
                            .on_hover_text(
                                "make it an ordinary block that no longer follows its source",
                            )
                            .clicked()
                    {
                        bake = true;
                        ui.close();
                    }
                });
                if resp.double_clicked() {
                    self.view = View::Pattern;
                }
                if resp.drag_started() {
                    self.arr_focus = true;
                    self.arr_drag = Some(i);
                    self.arr_select([i], i);
                }
            }
            if let Some(a) = menu_act {
                self.arranger_action(a, &ui.ctx().clone());
            }
            if bake {
                self.bake_selected();
            }
            // drag to reorder: insertion marker + drop
            if let (Some(from), Some(p)) = (self.arr_drag, pointer) {
                let to = (0..order.len())
                    .find(|&i| p.x < (xof(starts[i]) + xof(starts[i + 1])) * 0.5)
                    .unwrap_or(order.len());
                let x = xof(starts[to]);
                painter.vline(x, lane.y_range(), Stroke::new(3.0, th().accent));
                if released {
                    self.arr_drag = None;
                    self.arr_move(from, to);
                }
            } else if released {
                self.arr_drag = None;
            }
            // play marker: a triangle on the ruler and a line across the clips
            // (dimmer while playing, where it is the next Play song start)
            let cue = self.cue;
            if cue.0 < order.len() {
                let x = xof(starts[cue.0] + cue.1);
                let col = if playing {
                    th().accent.gamma_multiply(0.5)
                } else {
                    th().accent
                };
                painter.vline(x, lane.y_range(), Stroke::new(if playing { 1.0 } else { 2.0 }, col));
                marker(&painter, x, rect.top() + ruler_h + 3.0, col);
            }
            // playhead
            if playing {
                let idx = if loop_pat {
                    if order.get(self.arr_sel) == Some(&ppat) {
                        Some(self.arr_sel)
                    } else {
                        order.iter().position(|&b| b == ppat)
                    }
                } else {
                    Some(opos.min(order.len() - 1))
                };
                if let Some(i) = idx {
                    let x = xof(starts[i] + prow);
                    painter.vline(x, rect.y_range(), Stroke::new(2.0, th().play));
                    marker(&painter, x, rect.top() + ruler_h + 3.0, th().play);
                    if follow && !self.ruler_drag {
                        ui.scroll_to_rect(
                            Rect::from_center_size(
                                Pos2::new(x, rect.center().y),
                                egui::vec2(zoom * 64.0, 10.0),
                            ),
                            None,
                        );
                    }
                }
            }
            // ------------------------------------------------ automation lanes
            if show_auto && n_lanes > 0 {
                let (lrect, _) = ui.allocate_exact_size(egui::vec2(w, lanes_h), Sense::hover());
                let shift = ui.input(|i| i.modifiers.shift);
                let clip_x = ui.clip_rect().x_range();
                let to_x = |pos: f32| rect.left() + pad + pos * zoom;
                let to_pos = |x: f32| ((x - rect.left() - pad) / zoom).max(0.0);
                let song_snap = self.with(|s| s.song.clone());
                for (k, lane) in lanes.iter().enumerate() {
                    let lr = Rect::from_min_size(
                        Pos2::new(lrect.left(), lrect.top() + 4.0 + k as f32 * (LANE_H + 4.0)),
                        egui::vec2(w, LANE_H),
                    );
                    let body = lr.shrink2(egui::vec2(0.0, 6.0));
                    let to_y = |n: f32| body.bottom() - n.clamp(0.0, 1.0) * body.height();
                    let to_n = |y: f32| ((body.bottom() - y) / body.height()).clamp(0.0, 1.0);
                    let p = ui.painter_at(lr);
                    p.rect_filled(lr, 6, th().crt_bot);
                    for f in [0.25, 0.5, 0.75] {
                        p.hline(lr.x_range(), to_y(f), Stroke::new(1.0, th().crt_grid));
                    }
                    let mut r = 0;
                    while r <= acc {
                        p.vline(
                            to_x(r as f32),
                            lr.y_range(),
                            Stroke::new(1.0, th().crt_grid),
                        );
                        r += 16;
                    }
                    let target = crate::automation::Target::parse(&lane.target)
                        .or_else(|| crate::automation::Target::parse_in(&lane.target, &song_snap));
                    let col = target
                        .map(|tg| hex(tg.family_in(&song_snap).colour()))
                        .unwrap_or_else(|| hex(LANE_COLOURS[k % LANE_COLOURS.len()]));
                    // curve, sampled every 2 px across the visible range
                    let (x0, x1) = (clip_x.min.max(lr.left()), clip_x.max.min(lr.right()));
                    let mut pts = vec![];
                    let mut x = x0;
                    while x <= x1 {
                        if let Some(n) = crate::automation::value_at(&lane.points, to_pos(x)) {
                            pts.push(Pos2::new(x, to_y(n)));
                        }
                        x += 2.0;
                    }
                    if pts.len() > 1 {
                        // soft glow, then the line itself (muted, not neon)
                        for (wd, a) in [(7.0, 0.07), (4.0, 0.16)] {
                            p.add(egui::Shape::line(
                                pts.clone(),
                                Stroke::new(wd, col.gamma_multiply(a)),
                            ));
                        }
                        p.add(egui::Shape::line(
                            pts,
                            Stroke::new(1.8, col.gamma_multiply(0.9)),
                        ));
                    }
                    for (i, pt) in lane.points.iter().enumerate() {
                        let c = Pos2::new(to_x(pt.pos), to_y(pt.value));
                        let sel = self.auto_sel == Some((k, i));
                        let r = if sel {
                            crate::automation::POINT_R_HOT
                        } else {
                            crate::automation::POINT_R
                        };
                        p.circle_filled(c, r, col);
                        if sel {
                            p.circle_stroke(
                                c,
                                r + 2.0,
                                Stroke::new(1.5, th().selection_outline),
                            );
                        }
                    }
                    // playhead (stopped: the play marker)
                    if playing {
                        p.vline(to_x(song_pos), lr.y_range(), Stroke::new(1.5, th().play));
                    } else if cue.0 < order.len() {
                        let x = to_x((starts[cue.0] + cue.1) as f32);
                        p.vline(x, lr.y_range(), Stroke::new(1.0, th().accent));
                    }
                    let resp =
                        ui.interact(lr, ui.scope_id().with(("lane", k)), Sense::click_and_drag());
                    let hit = |pp: Pos2| {
                        lane.points.iter().position(|pt| {
                            Pos2::new(to_x(pt.pos), to_y(pt.value)).distance(pp)
                                < crate::automation::POINT_HIT
                        })
                    };
                    // label: name and value (at the playhead, or under the mouse)
                    let readout = if let Some(hp) = resp.hover_pos() {
                        let pos = to_pos(hp.x);
                        format!(
                            "line {:.0} · {}",
                            pos,
                            target
                                .map(|t| t.format_in(to_n(hp.y), &fxk))
                                .unwrap_or_default()
                        )
                    } else {
                        let pos = if playing { song_pos } else { 0.0 };
                        crate::automation::value_at(&lane.points, pos)
                            .and_then(|n| target.map(|t| t.format_in(n, &fxk)))
                            .unwrap_or_default()
                    };
                    let lp = Pos2::new(x0 + 8.0, lr.top() + 4.0);
                    p.text(
                        lp,
                        egui::Align2::LEFT_TOP,
                        format!(
                            "{}{}   {}",
                            if self.auto_lane == k { "> " } else { "" },
                            target
                                .map(|tg| tg.header_label(&song_snap))
                                .unwrap_or_else(|| lane.label.clone()),
                            readout
                        ),
                        FontId::proportional(11.5),
                        col.gamma_multiply(1.1),
                    );
                    if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
                        self.auto_lane = k;
                        self.arr_focus = true;
                    }
                    let snap = |pos: f32| if shift { pos } else { pos.round() };
                    if resp.double_clicked() {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            if let Some(i) = hit(pp) {
                                lane_ops.push(LaneOp::Delete(k, i));
                            } else {
                                lane_ops.push(LaneOp::Add(k, snap(to_pos(pp.x)), to_n(pp.y)));
                            }
                        }
                    } else if resp.clicked() {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            if let Some(i) = hit(pp) {
                                self.auto_sel = Some((k, i));
                            }
                        }
                    }
                    if resp.secondary_clicked() {
                        if let Some(i) = resp.interact_pointer_pos().and_then(hit) {
                            lane_ops.push(LaneOp::Delete(k, i));
                        }
                    }
                    if resp.drag_started() {
                        let o = ui.input(|i| i.pointer.press_origin());
                        if let Some(i) = o.and_then(hit) {
                            self.auto_drag = Some((k, i));
                            self.auto_sel = Some((k, i));
                            lane_ops.push(LaneOp::Snapshot);
                        } else if self.auto_pencil {
                            self.auto_pencil_lane = Some(k);
                            lane_ops.push(LaneOp::Snapshot);
                        }
                    }
                    if resp.dragged() {
                        if let Some(pp) = resp.interact_pointer_pos() {
                            if let Some((dk, i)) = self.auto_drag {
                                if dk == k {
                                    lane_ops.push(LaneOp::Move(
                                        k,
                                        i,
                                        snap(to_pos(pp.x)),
                                        to_n(pp.y),
                                    ));
                                }
                            } else if self.auto_pencil_lane == Some(k) {
                                lane_ops.push(LaneOp::Pencil(k, to_pos(pp.x).round(), to_n(pp.y)));
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        self.auto_drag = None;
                        self.auto_pencil_lane = None;
                    }
                }
            }
        });
        self.arr_scroll = out.state.clamped_offset().x;
        self.arr_area = out.inner_rect;
        self.apply_lane_ops(lane_ops);
    }
}

/// Ruler click -> (song-order entry, row). `lens` = rows of each order entry,
/// `abs` = clicked row on the whole timeline. With `snap` the row goes back to
/// the start of its bar (16 rows, the ruler ticks), but never before the start
/// of the block it lands in. Past the end: the last block's last bar / row.
pub(crate) fn ruler_target(lens: &[usize], abs: usize, snap: bool) -> (usize, usize) {
    let total: usize = lens.iter().sum();
    if total == 0 {
        return (0, 0);
    }
    let abs = abs.min(total - 1);
    let mut start = 0;
    for (i, &n) in lens.iter().enumerate() {
        if abs < start + n {
            let t = if snap {
                (abs / 16 * 16).max(start)
            } else {
                abs
            };
            return (i, t - start);
        }
        start += n;
    }
    (lens.len() - 1, 0)
}

/// down-pointing play-marker triangle whose tip sits at (x, tip_y)
fn marker(p: &egui::Painter, x: f32, tip_y: f32, col: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(x - 5.0, tip_y - 8.0),
            Pos2::new(x + 5.0, tip_y - 8.0),
            Pos2::new(x, tip_y),
        ],
        col,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod ruler_tests {
    use super::ruler_target;

    #[test]
    fn bar_snap_maps_to_order_entry_and_row() {
        let lens = [64, 64, 32, 64];
        assert_eq!(ruler_target(&lens, 0, true), (0, 0));
        assert_eq!(ruler_target(&lens, 37, true), (0, 32));
        assert_eq!(ruler_target(&lens, 37, false), (0, 37));
        assert_eq!(ruler_target(&lens, 64, true), (1, 0));
        assert_eq!(ruler_target(&lens, 100, true), (1, 32));
        assert_eq!(ruler_target(&lens, 130, true), (2, 0));
        assert_eq!(ruler_target(&lens, 150, true), (2, 16));
        assert_eq!(ruler_target(&lens, 175, true), (3, 0));
        assert_eq!(ruler_target(&lens, 180, true), (3, 16));
        // past the end: last bar of the last block
        assert_eq!(ruler_target(&lens, 10_000, true), (3, 48));
        assert_eq!(ruler_target(&lens, 10_000, false), (3, 63));
    }

    #[test]
    fn odd_block_lengths_never_snap_into_the_previous_block() {
        // block 1 starts at row 24, between the bar lines at 16 and 32
        let lens = [24, 40];
        assert_eq!(ruler_target(&lens, 20, true), (0, 16));
        assert_eq!(ruler_target(&lens, 26, true), (1, 0));
        assert_eq!(ruler_target(&lens, 33, true), (1, 8));
        assert_eq!(ruler_target(&[], 5, true), (0, 0));
    }
}
