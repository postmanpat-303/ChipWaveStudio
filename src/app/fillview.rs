//! Song > Fill to length… dialog, ghost clips in the arranger, block roles (docs/AUTO_ARRANGE.md).
use super::*;
use crate::arrange::{self, Proposal, Role, Style};
use std::collections::BTreeSet;

pub(super) struct FillState {
    styles: Vec<Style>,
    style_errs: Vec<String>,
    style: usize,
    target: String,
    snap: usize,
    variation: f32,
    pub(super) seed: u64,
    energy: Vec<[f32; 2]>,
    pub(super) prop: Option<Proposal>,
    err: Option<String>,
    pub(super) sel: usize,
    pub(super) locks: BTreeSet<usize>,
    drag: Option<usize>,
    pub(super) previewing: bool,
    /// dialog hidden (H) so the ghost clips are visible; keys still work
    hidden: bool,
}

pub(super) fn role_col(r: Role) -> Color32 {
    let [a, b, c] = r.colour();
    Color32::from_rgb(a, b, c)
}

fn styles_dir() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join("styles"))
}

fn new_seed(s: u64) -> u64 {
    let mut r = arrange::Rng::new(s.wrapping_add(0x5EED));
    r.next_u64() % 1_000_000
}

impl App {
    pub(super) fn open_fill(&mut self) {
        let (styles, errs) = arrange::load_styles(styles_dir().as_deref());
        let style = styles
            .iter()
            .position(|s| s.name.starts_with("Pop"))
            .unwrap_or(0);
        let target = styles
            .get(style)
            .map(|s| s.default_length.clone())
            .unwrap_or("3:30".into());
        let energy = styles
            .get(style)
            .map(|s| s.energy.clone())
            .unwrap_or_default();
        self.fill = Some(FillState {
            styles,
            style_errs: errs,
            style,
            target,
            snap: 4,
            variation: 0.5,
            seed: 1,
            energy,
            prop: None,
            err: None,
            sel: 0,
            locks: BTreeSet::new(),
            drag: None,
            previewing: false,
            hidden: false,
        });
        self.fill_generate();
    }

    fn fill_target_bars(&self) -> Option<usize> {
        let f = self.fill.as_ref()?;
        let secs = arrange::parse_mmss(&f.target)?;
        Some(self.with(|s| arrange::target_bars(&s.song, secs, f.snap)))
    }

    pub(super) fn fill_generate(&mut self) {
        let Some(tb) = self.fill_target_bars() else {
            if let Some(f) = &mut self.fill {
                f.err = Some("Target: type a length like 3:30".into());
            }
            return;
        };
        let song = self.with(|s| s.song.clone());
        let Some(f) = &mut self.fill else { return };
        let Some(style) = f.styles.get(f.style) else {
            f.err = Some("No style templates found".into());
            return;
        };
        // locks: matched by start row in the previous proposal
        let mut locks = vec![];
        if let Some(p) = &f.prop {
            let mut t = 0;
            for (i, s) in p.sections.iter().enumerate() {
                if f.locks.contains(&i) {
                    locks.push((t, s.clone()));
                }
                t += s.rows;
            }
        }
        let params = arrange::Params {
            style,
            target_bars: tb,
            energy: &f.energy,
            variation: f.variation,
            seed: f.seed,
            locks,
        };
        match arrange::generate(&song, &params) {
            Ok(p) => {
                // remap lock indices onto the new plan
                f.locks = p
                    .sections
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.locked)
                    .map(|(i, _)| i)
                    .collect();
                f.sel = f.sel.min(p.sections.len().saturating_sub(1));
                f.prop = Some(p);
                f.err = None;
            }
            Err(e) => {
                f.prop = None;
                f.err = Some(e);
            }
        }
        if self.fill.as_ref().is_some_and(|f| f.previewing) {
            self.fill_preview(true);
        }
    }

    pub(super) fn fill_reroll(&mut self) {
        if let Some(f) = &mut self.fill {
            f.seed = new_seed(f.seed);
        }
        self.fill_generate();
    }

    pub(super) fn fill_toggle_lock(&mut self) {
        if let Some(f) = &mut self.fill {
            if f.prop.is_some() && !f.locks.remove(&f.sel) {
                f.locks.insert(f.sel);
            }
            if let Some(p) = &mut f.prop {
                for (i, s) in p.sections.iter_mut().enumerate() {
                    s.locked = f.locks.contains(&i);
                }
            }
        }
    }

    /// start (or restart, `on` = true) / stop the proposal preview
    pub(super) fn fill_preview(&mut self, on: bool) {
        let Some(f) = &mut self.fill else { return };
        let song = match (&f.prop, on) {
            (Some(p), true) => {
                let seed = f.seed;
                let p = p.clone();
                Some(self.with(|s| arrange::preview_song(&s.song, &p, seed)))
            }
            _ => None,
        };
        let Some(f) = &mut self.fill else { return };
        f.previewing = song.is_some();
        self.with(|s| {
            s.engine.stop();
            s.preview = song;
            if let Some(p) = &s.preview {
                s.engine.start(p, 0, 0, false, 0);
            }
        });
        self.status = if on {
            "Previewing the proposal (P = stop)".into()
        } else {
            "Preview stopped".into()
        };
    }

    fn fill_stop_preview(&mut self) {
        if self.fill.as_ref().is_some_and(|f| f.previewing) {
            self.fill_preview(false);
        }
    }

    pub(super) fn fill_accept(&mut self) {
        self.fill_stop_preview();
        let Some(f) = &self.fill else { return };
        let Some(p) = f.prop.clone() else { return };
        let seed = f.seed;
        self.snapshot();
        let secs = self.with(|s| {
            arrange::apply(&mut s.song, &p, seed);
            p.seconds(&s.song)
        });
        self.fill = None;
        self.arr_sel = 0;
        self.arr_multi.clear();
        self.pattern = self.with(|s| s.song.order[0]);
        self.row = 0;
        self.status = format!(
            "Arrangement accepted: {} ({:.0} bars, {} sections). Ctrl+Z undoes.",
            arrange::mmss(secs),
            p.bars(),
            p.sections.len()
        );
    }

    pub(super) fn fill_discard(&mut self) {
        self.fill_stop_preview();
        self.fill = None;
        self.status = "Proposal discarded".into();
    }

    /// keys while the dialog is open; true = handled
    pub(super) fn fill_key(&mut self, key: Key, m: egui::Modifiers) -> bool {
        if self.fill.is_none() {
            return false;
        }
        if m.command {
            if key == Key::Z {
                self.fill_discard();
                return true;
            }
            return false;
        }
        let n = self
            .fill
            .as_ref()
            .and_then(|f| f.prop.as_ref())
            .map(|p| p.sections.len())
            .unwrap_or(0);
        match key {
            Key::G => self.fill_generate(),
            Key::R => self.fill_reroll(),
            Key::L => self.fill_toggle_lock(),
            Key::P => {
                let on = !self.fill.as_ref().is_some_and(|f| f.previewing);
                self.fill_preview(on);
            }
            Key::Enter => self.fill_accept(),
            Key::H => {
                if let Some(f) = &mut self.fill {
                    f.hidden = !f.hidden;
                    self.status = if f.hidden {
                        "Fill to length: dialog hidden (H shows it; Enter accepts, Esc discards)"
                            .into()
                    } else {
                        String::new()
                    };
                }
            }
            Key::Escape => self.fill_discard(),
            Key::ArrowLeft | Key::ArrowRight => {
                if let Some(f) = &mut self.fill {
                    if n > 0 {
                        f.sel = if key == Key::ArrowLeft {
                            f.sel.saturating_sub(1)
                        } else {
                            (f.sel + 1).min(n - 1)
                        };
                    }
                }
            }
            Key::ArrowUp | Key::ArrowDown => {
                if let Some(f) = &mut self.fill {
                    let d = if key == Key::ArrowUp { 0.05 } else { -0.05 };
                    f.variation = (f.variation + d).clamp(0.0, 1.0);
                }
                self.fill_generate();
            }
            Key::S => {
                if let Some(f) = &mut self.fill {
                    f.style = (f.style + 1) % f.styles.len().max(1);
                    if let Some(s) = f.styles.get(f.style) {
                        f.energy = s.energy.clone();
                        f.target = s.default_length.clone();
                    }
                }
                self.fill_generate();
            }
            Key::Space
            | Key::F1
            | Key::F2
            | Key::F3
            | Key::F4
            | Key::F5
            | Key::F6
            | Key::F7
            | Key::F8 => return false,
            _ => {}
        }
        true
    }

    /// set (or clear) the role of the blocks of the selected arranger clips
    pub(super) fn set_role(&mut self, r: Option<Role>) {
        let idx = self.arr_sel_list();
        let blocks: BTreeSet<usize> = self.with(|s| {
            idx.iter()
                .filter_map(|&i| s.song.order.get(i).copied())
                .collect()
        });
        if blocks.is_empty() {
            return;
        }
        self.snapshot();
        self.with(|s| {
            for &b in &blocks {
                if let Some(p) = s.song.patterns.get_mut(b) {
                    p.role = r;
                }
            }
        });
        let ids: Vec<String> = blocks.iter().map(|b| format!("{b:02}")).collect();
        self.status = match r {
            Some(r) => format!("Block {} tagged {}", ids.join(", "), r.name()),
            None => format!("Block {}: role cleared", ids.join(", ")),
        };
        if self
            .fill
            .as_ref()
            .is_some_and(|f| f.prop.is_some() || f.err.is_some())
        {
            self.fill_generate();
        }
    }

    pub(super) fn bake_selected(&mut self) {
        let idx = self.arr_sel_list();
        self.snapshot();
        let n = self.with(|s| {
            let mut n = 0;
            for i in idx {
                if let Some(&b) = s.song.order.get(i) {
                    let p = &mut s.song.patterns[b];
                    if p.derived.take().is_some() {
                        p.cache.clear();
                        n += 1;
                    }
                }
            }
            n
        });
        self.status = format!("Baked {n} derived block(s): now ordinary blocks");
    }

    /// once per frame: derived blocks follow their source; preview ends with playback
    pub(super) fn tick_derived(&mut self) {
        let (baked, stopped) = self.with(|s| {
            let b = if s.song.patterns.iter().any(|p| p.derived.is_some()) {
                arrange::refresh_derived(&mut s.song)
            } else {
                0
            };
            let stopped = s.preview.is_some() && !s.engine.playing;
            if stopped {
                s.preview = None;
            }
            (b, stopped)
        });
        if baked > 0 {
            self.status = "Edited a derived block: it is now an ordinary (baked) block".into();
        }
        if stopped {
            if let Some(f) = &mut self.fill {
                f.previewing = false;
            }
        }
    }

    /// ghost clips of the proposal, drawn in the arranger's lower lane
    pub(super) fn ghost_lane(
        &self,
        painter: &egui::Painter,
        lane: Rect,
        x0: f32,
        zoom: f32,
        names: &[String],
    ) {
        let Some(f) = &self.fill else { return };
        let Some(p) = &f.prop else { return };
        painter.text(
            Pos2::new(lane.left() + 6.0, lane.top() - 1.0),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "PROPOSAL · {} · {:.0} bars · Enter = accept, Esc = discard",
                arrange::mmss(self.with(|s| p.seconds(&s.song))),
                p.bars()
            ),
            FontId::monospace(10.0),
            th().ink2,
        );
        let mut t = 0usize;
        for (i, s) in p.sections.iter().enumerate() {
            let r = Rect::from_min_max(
                Pos2::new(x0 + t as f32 * zoom + 1.0, lane.top() + 2.0),
                Pos2::new(x0 + (t + s.rows) as f32 * zoom - 1.0, lane.bottom()),
            );
            t += s.rows;
            let col = role_col(s.role);
            painter.rect_filled(r, 5, col.gamma_multiply(0.28));
            let st = Stroke::new(
                if i == f.sel { 2.0 } else { 1.2 },
                if i == f.sel { th().focus_ring } else { col },
            );
            let pts = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            for w in pts.windows(2) {
                painter.extend(egui::Shape::dashed_line(&[w[0], w[1]], st, 5.0, 3.0));
            }
            let tp = painter.with_clip_rect(r.shrink(2.0));
            let derived = s
                .ops
                .iter()
                .any(|o| !matches!(o, arrange::VarOp::TakeRows(_)))
                || s.pad;
            let name = names.get(s.block).cloned().unwrap_or_default();
            tp.text(
                r.left_top() + egui::vec2(5.0, 3.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "{}{} {}{}",
                    if s.locked { "[L] " } else { "" },
                    s.role.short(),
                    name,
                    if derived { "'" } else { "" }
                ),
                FontId::proportional(11.0),
                th().ink,
            );
            let ops: Vec<String> = s
                .ops
                .iter()
                .filter(|o| !matches!(o, arrange::VarOp::TakeRows(_)))
                .map(|o| o.label())
                .collect();
            let sub = if s.pad {
                format!("pad fill {} bar", s.rows / p.rows_per_bar.max(1))
            } else if ops.is_empty() {
                format!("{} bars", s.rows / p.rows_per_bar.max(1))
            } else {
                format!("derived · {}", ops.join(", "))
            };
            tp.text(
                r.left_top() + egui::vec2(5.0, 17.0),
                egui::Align2::LEFT_TOP,
                sub,
                FontId::proportional(10.0),
                th().ink2,
            );
        }
    }

    pub(super) fn fill_dialog(&mut self, ctx: &egui::Context) {
        if self.fill.as_ref().is_some_and(|f| f.hidden) {
            return;
        }
        let Some(mut f) = self.fill.take() else {
            return;
        };
        let song_info = self.with(|s| {
            (
                s.song.bpm,
                s.song.lines_per_beat,
                s.song
                    .patterns
                    .iter()
                    .map(|p| (p.name.clone(), p.role, p.derived.is_some()))
                    .collect::<Vec<_>>(),
            )
        });
        let tagged = song_info.2.iter().filter(|x| x.1.is_some() && !x.2).count();
        let tb = arrange::parse_mmss(&f.target)
            .map(|secs| self.with(|s| arrange::target_bars(&s.song, secs, f.snap)));
        let result_secs = f.prop.as_ref().map(|p| self.with(|s| p.seconds(&s.song)));
        let mut regen = false;
        let mut act: Option<&str> = None;
        let mut open = true;
        egui::Window::new("Fill to length")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.content_rect().right_top() + egui::vec2(-640.0, 60.0))
            .show(ctx, |ui| {
                ui.set_width(600.0);
                egui::Grid::new("fill_grid")
                    .num_columns(2)
                    .spacing([12.0, 7.0])
                    .show(ui, |ui| {
                        ui.label("Target length");
                        ui.horizontal(|ui| {
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut f.target)
                                    .desired_width(56.0)
                                    .hint_text("3:30"),
                            );
                            if r.lost_focus()
                                || (r.changed() && arrange::parse_mmss(&f.target).is_some())
                            {
                                regen = true;
                            }
                            ui.label("snap");
                            for s in [1usize, 4, 8, 16] {
                                if ui.selectable_label(f.snap == s, format!("{s}")).clicked() {
                                    f.snap = s;
                                    regen = true;
                                }
                            }
                            ui.label(egui::RichText::new("bars").color(th().ink2));
                        });
                        ui.end_row();
                        ui.label("Result");
                        let txt = match (result_secs, &f.prop, tb) {
                            (Some(secs), Some(p), Some(tb)) => {
                                let tsecs = arrange::parse_mmss(&f.target).unwrap_or(0.0);
                                format!(
                                    "result: {} ({:.0} bars) · target {} → {} bars{}",
                                    arrange::mmss(secs),
                                    p.bars(),
                                    f.target.trim(),
                                    tb,
                                    if (secs - tsecs).abs() >= 0.05 {
                                        format!(" · {:+.1} s", secs - tsecs)
                                    } else {
                                        String::new()
                                    }
                                )
                            }
                            _ => "result: –".into(),
                        };
                        ui.label(egui::RichText::new(txt).monospace().color(th().value));
                        ui.end_row();
                        ui.label("Style");
                        ui.horizontal(|ui| {
                            let cur = f
                                .styles
                                .get(f.style)
                                .map(|s| s.name.clone())
                                .unwrap_or_default();
                            egui::ComboBox::from_id_salt("fill_style")
                                .selected_text(cur)
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    for (i, s) in f.styles.iter().enumerate() {
                                        if ui.selectable_label(f.style == i, &s.name).clicked()
                                            && f.style != i
                                        {
                                            f.style = i;
                                            f.energy = s.energy.clone();
                                            f.target = s.default_length.clone();
                                            regen = true;
                                        }
                                    }
                                });
                            if ui
                                .button("Edit styles…")
                                .on_hover_text("opens the folder with the style TOML files")
                                .clicked()
                            {
                                act = Some("styles");
                            }
                            ui.label(egui::RichText::new("S = next").small().color(th().ink2));
                        });
                        ui.end_row();
                        ui.label("Variation");
                        ui.horizontal(|ui| {
                            if knob(ui, &mut f.variation) {
                                regen = true;
                            }
                            ui.label(
                                egui::RichText::new(format!("{:.0} %", f.variation * 100.0))
                                    .monospace()
                                    .strong(),
                            );
                            ui.label(egui::RichText::new("Up/Down").small().color(th().ink2));
                            ui.add_space(18.0);
                            ui.label("Seed");
                            if ui
                                .add(egui::DragValue::new(&mut f.seed).range(0..=999_999))
                                .changed()
                            {
                                regen = true;
                            }
                            if ui.button("Reroll").on_hover_text("R: new seed").clicked() {
                                act = Some("reroll");
                            }
                        });
                        ui.end_row();
                    });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Energy curve").strong());
                    ui.label(
                        egui::RichText::new(
                            "drag points · double-click adds · right-click removes",
                        )
                        .small()
                        .color(th().ink2),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Reset curve").clicked() {
                            if let Some(s) = f.styles.get(f.style) {
                                f.energy = s.energy.clone();
                                regen = true;
                            }
                        }
                    });
                });
                if energy_editor(ui, &mut f) {
                    regen = true;
                }
                ui.add_space(4.0);
                // sections strip (fixed height)
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Sections").strong());
                    ui.label(
                        egui::RichText::new(
                            "Left/Right select · L lock · P preview · H hide dialog",
                        )
                        .small()
                        .color(th().ink2),
                    );
                });
                let (srect, _) = ui.allocate_exact_size(egui::vec2(600.0, 40.0), Sense::hover());
                let p = ui.painter_at(srect);
                p.rect_filled(srect, 4, th().crt_bot);
                if let Some(prop) = &f.prop {
                    let tot = prop.rows.max(1) as f32;
                    let mut x = srect.left() + 2.0;
                    for (i, s) in prop.sections.iter().enumerate() {
                        let w = (s.rows as f32 / tot * (srect.width() - 4.0)).max(3.0);
                        let r = Rect::from_min_size(
                            Pos2::new(x, srect.top() + 3.0),
                            egui::vec2(w - 1.0, srect.height() - 6.0),
                        );
                        x += w;
                        let col = role_col(s.role);
                        p.rect_filled(r, 3, col.gamma_multiply(if s.locked { 0.9 } else { 0.55 }));
                        if i == f.sel {
                            p.rect_stroke(
                                r,
                                3,
                                Stroke::new(2.0, th().focus_ring),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let varied = s
                            .ops
                            .iter()
                            .any(|o| !matches!(o, arrange::VarOp::TakeRows(_)));
                        let lbl = format!(
                            "{}{}{}",
                            if s.locked { "L " } else { "" },
                            s.role.short(),
                            if varied || s.pad { "'" } else { "" }
                        );
                        if w > 15.0 {
                            let fs = if w > 30.0 { 10.0 } else { 8.0 };
                            p.with_clip_rect(r).text(
                                r.center(),
                                egui::Align2::CENTER_CENTER,
                                lbl,
                                FontId::monospace(fs),
                                th().ink,
                            );
                        }
                        let resp =
                            ui.interact(r, ui.make_persistent_id(("fsec", i)), Sense::click());
                        if resp.clicked() {
                            f.sel = i;
                        }
                        if resp.double_clicked() {
                            f.sel = i;
                            act = Some("lock");
                        }
                        let name = song_info
                            .2
                            .get(s.block)
                            .map(|x| x.0.clone())
                            .unwrap_or_default();
                        let ops: Vec<String> = s.ops.iter().map(|o| o.label()).collect();
                        resp.on_hover_text(format!(
                            "{} · {} · {} bars{}",
                            s.role.name(),
                            name,
                            s.rows / prop.rows_per_bar.max(1),
                            if ops.is_empty() {
                                String::new()
                            } else {
                                format!("\n{}", ops.join(", "))
                            }
                        ));
                    }
                } else {
                    p.text(
                        srect.center(),
                        egui::Align2::CENTER_CENTER,
                        "no proposal yet: press Generate (G)",
                        FontId::proportional(12.0),
                        th().ink2,
                    );
                }
                // selected section + notes: fixed two lines
                let (nrect, _) = ui.allocate_exact_size(egui::vec2(600.0, 34.0), Sense::hover());
                let np = ui.painter_at(nrect);
                let line1 = match &f.prop {
                    Some(prop) => prop
                        .sections
                        .get(f.sel)
                        .map(|s| {
                            let ops: Vec<String> = s
                                .ops
                                .iter()
                                .filter(|o| !matches!(o, arrange::VarOp::TakeRows(_)))
                                .map(|o| o.label())
                                .collect();
                            format!(
                                "#{} {} · {} · {} bars{}{}",
                                f.sel + 1,
                                s.role.name(),
                                song_info.2.get(s.block).map(|x| x.0.as_str()).unwrap_or(""),
                                s.rows / prop.rows_per_bar.max(1),
                                if ops.is_empty() {
                                    String::new()
                                } else {
                                    format!(" · {}", ops.join(", "))
                                },
                                if s.locked { " · locked" } else { "" }
                            )
                        })
                        .unwrap_or_default(),
                    None => String::new(),
                };
                np.text(
                    nrect.left_top(),
                    egui::Align2::LEFT_TOP,
                    line1,
                    FontId::proportional(12.0),
                    th().ink,
                );
                let mut notes: Vec<String> = vec![];
                if let Some(e) = &f.err {
                    notes.push(e.clone());
                }
                if let Some(p) = &f.prop {
                    notes.extend(p.notes.iter().cloned());
                }
                notes.extend(f.style_errs.iter().cloned());
                if tagged == 0 {
                    notes.push(
                        "Tag blocks first: arranger, keys 1–7 (Intro … Fill) or right-click > Role"
                            .into(),
                    );
                }
                let warn = f.err.is_some() || tagged == 0;
                np.with_clip_rect(nrect).text(
                    nrect.left_top() + egui::vec2(0.0, 17.0),
                    egui::Align2::LEFT_TOP,
                    if notes.is_empty() {
                        format!("{tagged} tagged block(s) · seed {}", f.seed)
                    } else {
                        notes.join(" · ")
                    },
                    FontId::proportional(11.0),
                    if warn { th().brick } else { th().ink2 },
                );
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new(" Generate ").strong(),
                        ))
                        .on_hover_text("G")
                        .clicked()
                    {
                        act = Some("gen");
                    }
                    let pv = if f.previewing {
                        "Stop preview"
                    } else {
                        "Preview"
                    };
                    if ui
                        .add_enabled(f.prop.is_some(), egui::Button::new(pv))
                        .on_hover_text("P")
                        .clicked()
                    {
                        act = Some("preview");
                    }
                    let lk = if f.locks.contains(&f.sel) {
                        "Unlock"
                    } else {
                        "Lock section"
                    };
                    if ui
                        .add_enabled(f.prop.is_some(), egui::Button::new(lk))
                        .on_hover_text("L: keep this section on reroll")
                        .clicked()
                    {
                        act = Some("lock");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Discard").on_hover_text("Esc").clicked() {
                            act = Some("discard");
                        }
                        if ui
                            .add_enabled(
                                f.prop.is_some(),
                                egui::Button::new(egui::RichText::new("  Accept  ").strong())
                                    .fill(th().accent.gamma_multiply(0.6)),
                            )
                            .on_hover_text("Enter: replace the song order (one undo step)")
                            .clicked()
                        {
                            act = Some("accept");
                        }
                    });
                });
            });
        self.fill = Some(f);
        if !open {
            act = Some("discard");
        }
        if regen {
            self.fill_generate();
        }
        match act {
            Some("gen") => self.fill_generate(),
            Some("reroll") => self.fill_reroll(),
            Some("lock") => self.fill_toggle_lock(),
            Some("preview") => {
                let on = !self.fill.as_ref().is_some_and(|f| f.previewing);
                self.fill_preview(on);
            }
            Some("accept") => self.fill_accept(),
            Some("discard") => self.fill_discard(),
            Some("styles") => {
                if let Some(d) = styles_dir() {
                    let _ = std::fs::create_dir_all(&d);
                    #[cfg(windows)]
                    let _ = std::process::Command::new("explorer").arg(&d).spawn();
                    #[cfg(not(windows))]
                    let _ = std::process::Command::new("xdg-open").arg(&d).spawn();
                    self.status = format!("Styles: {} (reopen the dialog to reload)", d.display());
                }
            }
            _ => {}
        }
    }
}

/// rotary knob, drag up/down; returns true when changed (on release, to keep generation cheap)
fn knob(ui: &mut egui::Ui, v: &mut f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), Sense::click_and_drag());
    let mut changed = false;
    if resp.dragged() {
        let d = -resp.drag_delta().y * 0.005 + resp.drag_delta().x * 0.005;
        if d != 0.0 {
            *v = (*v + d).clamp(0.0, 1.0);
        }
    }
    if resp.drag_stopped() {
        changed = true;
    }
    if resp.double_clicked() {
        *v = 0.5;
        changed = true;
    }
    let p = ui.painter();
    let c = r.center();
    let rad = 14.0;
    p.circle_filled(c, rad, th().crt_bot);
    p.circle_stroke(c, rad, Stroke::new(1.0, th().ink2.gamma_multiply(0.6)));
    let a0 = std::f32::consts::PI * 0.75;
    let span = std::f32::consts::PI * 1.5;
    let arc = |t: f32| c + egui::vec2((a0 + span * t).cos(), (a0 + span * t).sin()) * (rad - 3.0);
    let pts: Vec<Pos2> = (0..=24).map(|i| arc(i as f32 / 24.0 * *v)).collect();
    if pts.len() > 1 {
        p.add(egui::Shape::line(pts, Stroke::new(3.0, th().accent)));
    }
    p.line_segment([c, arc(*v)], Stroke::new(2.0, th().ink));
    resp.on_hover_text("Variation: drag up/down (double-click = 50 %)");
    changed
}

/// CRT energy curve with draggable square handles; returns true when edited
fn energy_editor(ui: &mut egui::Ui, f: &mut FillState) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(600.0, 110.0), Sense::click_and_drag());
    let p = ui.painter_at(r);
    theme::crt(&p, r);
    let body = r.shrink2(egui::vec2(10.0, 10.0));
    let to_s = |q: [f32; 2]| {
        Pos2::new(
            body.left() + q[0] * body.width(),
            body.bottom() - q[1] * body.height(),
        )
    };
    let to_q = |s: Pos2| {
        [
            ((s.x - body.left()) / body.width()).clamp(0.0, 1.0),
            ((body.bottom() - s.y) / body.height()).clamp(0.0, 1.0),
        ]
    };
    for g in [0.25, 0.5, 0.75] {
        p.hline(
            body.x_range(),
            body.bottom() - g * body.height(),
            Stroke::new(1.0, th().crt_grid),
        );
        p.vline(
            body.left() + g * body.width(),
            body.y_range(),
            Stroke::new(1.0, th().crt_grid),
        );
    }
    // section energies of the proposal as faint bars
    if let Some(prop) = &f.prop {
        let tot = prop.rows.max(1) as f32;
        let mut t = 0.0;
        for s in &prop.sections {
            let x0 = body.left() + t / tot * body.width();
            t += s.rows as f32;
            let x1 = body.left() + t / tot * body.width();
            p.vline(
                x0,
                body.y_range(),
                Stroke::new(1.0, role_col(s.role).gamma_multiply(0.5)),
            );
            if x1 - x0 > 18.0 {
                p.text(
                    Pos2::new(x0 + 2.0, body.top()),
                    egui::Align2::LEFT_TOP,
                    s.role.short(),
                    FontId::monospace(8.0),
                    role_col(s.role).gamma_multiply(0.9),
                );
            }
        }
    }
    f.energy.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let pts: Vec<Pos2> = f.energy.iter().map(|q| to_s(*q)).collect();
    let col = th().accent;
    for (w, a) in [(6.0, 0.12), (2.0, 1.0)] {
        p.add(egui::Shape::line(
            pts.clone(),
            Stroke::new(w, col.gamma_multiply(a)),
        ));
    }
    let mut changed = false;
    let pos = resp.interact_pointer_pos().or(resp.hover_pos());
    let near = |pos: Pos2, pts: &[Pos2]| {
        pts.iter()
            .enumerate()
            .map(|(i, q)| (i, q.distance(pos)))
            .filter(|x| x.1 < 10.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    };
    if resp.drag_started() {
        f.drag = pos.and_then(|q| near(q, &pts));
    }
    if let (Some(i), Some(q)) = (f.drag, pos) {
        if resp.dragged() {
            let mut v = to_q(q);
            let n = f.energy.len();
            if i == 0 {
                v[0] = 0.0;
            } else if i + 1 == n {
                v[0] = 1.0;
            } else {
                v[0] = v[0].clamp(f.energy[i - 1][0] + 0.01, f.energy[i + 1][0] - 0.01);
            }
            f.energy[i] = v;
        }
    }
    if resp.drag_stopped() && f.drag.is_some() {
        f.drag = None;
        changed = true;
    }
    if resp.double_clicked() {
        if let Some(q) = pos {
            f.energy.push(to_q(q));
            changed = true;
        }
    }
    if resp.secondary_clicked() {
        if let Some(i) = pos.and_then(|q| near(q, &pts)) {
            if f.energy.len() > 2 && i != 0 && i + 1 != f.energy.len() {
                f.energy.remove(i);
                changed = true;
            }
        }
    }
    for (i, q) in pts.iter().enumerate() {
        let hot = f.drag == Some(i) || pos.is_some_and(|ps| ps.distance(*q) < 10.0);
        let rr = Rect::from_center_size(*q, egui::vec2(9.0, 9.0));
        p.rect_filled(rr, 1, if hot { th().ink } else { col });
        p.rect_stroke(
            rr,
            1,
            Stroke::new(1.0, Color32::from_black_alpha(160)),
            egui::StrokeKind::Outside,
        );
    }
    changed
}
