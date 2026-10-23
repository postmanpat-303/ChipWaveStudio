//! Browser panel painting and input (direction A, docs/BROWSER.md)
use super::browserview::{Act, DropAt};
use super::*;
use crate::browser::{self as br, Fmt, Item, LocFilter, Sort, Src, Tab};

fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Tab(Tab),
    Star,
    Heart,
    Play,
    Stop,
    Rescan,
}

fn star_points(c: Pos2, r: f32) -> Vec<Pos2> {
    (0..10)
        .map(|k| {
            let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
            let rr = if k % 2 == 0 { r } else { r * 0.45 };
            c + egui::vec2(a.cos() * rr, a.sin() * rr)
        })
        .collect()
}

/// small vector icons (the UI font has no symbol glyphs)
pub(super) fn icon(p: &egui::Painter, ic: Icon, c: Pos2, s: f32, col: Color32) {
    use egui::{Shape, Stroke};
    let st = Stroke::new(1.5, col);
    let v = egui::vec2;
    match ic {
        Icon::Star | Icon::Tab(Tab::Presets) => {
            // a star is concave: fan of triangles from the centre
            let pts = star_points(c, s * 0.5);
            for k in 0..10 {
                p.add(Shape::convex_polygon(
                    vec![c, pts[k], pts[(k + 1) % 10]],
                    col,
                    Stroke::NONE,
                ));
            }
        }
        Icon::Heart => {
            let r = s * 0.24;
            p.circle_filled(c + v(-r * 0.95, -r * 0.5), r, col);
            p.circle_filled(c + v(r * 0.95, -r * 0.5), r, col);
            p.add(Shape::convex_polygon(
                vec![
                    c + v(-r * 1.95, -r * 0.25),
                    c + v(r * 1.95, -r * 0.25),
                    c + v(0.0, s * 0.45),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![
                    c + v(-s * 0.3, -s * 0.38),
                    c + v(s * 0.38, 0.0),
                    c + v(-s * 0.3, s * 0.38),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_center_size(c, v(s * 0.6, s * 0.6)), 1, col);
        }
        Icon::Rescan => {
            let r = s * 0.36;
            let pts: Vec<Pos2> = (0..=20)
                .map(|k| {
                    let a = 0.6 + k as f32 / 20.0 * 4.9;
                    c + v(a.cos() * r, a.sin() * r)
                })
                .collect();
            let end = *pts.last().unwrap();
            p.add(Shape::line(pts, st));
            p.add(Shape::convex_polygon(
                vec![end + v(-3.5, -2.5), end + v(3.0, -3.5), end + v(1.0, 3.0)],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Tab(Tab::Instruments) => {
            p.circle_filled(c + v(-s * 0.15, s * 0.25), s * 0.18, col);
            p.line_segment([c + v(s * 0.02, s * 0.25), c + v(s * 0.02, -s * 0.38)], st);
            p.line_segment([c + v(s * 0.02, -s * 0.38), c + v(s * 0.3, -s * 0.18)], st);
        }
        Icon::Tab(Tab::Songs) => {
            for dx in [-0.25, 0.22] {
                p.circle_filled(c + v(s * dx, s * 0.28), s * 0.14, col);
                p.line_segment(
                    [
                        c + v(s * (dx + 0.12), s * 0.28),
                        c + v(s * (dx + 0.12), -s * 0.32),
                    ],
                    st,
                );
            }
            p.line_segment(
                [c + v(-s * 0.13, -s * 0.32), c + v(s * 0.34, -s * 0.38)],
                Stroke::new(3.0, col),
            );
        }
        Icon::Tab(Tab::Effects) => {
            // a knob with its pointer
            p.circle_stroke(c, s * 0.34, st);
            p.line_segment([c, c + v(s * 0.2, -s * 0.24)], Stroke::new(2.0, col));
            for k in 0..3 {
                let a = 2.4 + k as f32 * 0.9;
                p.circle_filled(c + v(a.cos() * s * 0.5, a.sin() * s * 0.5), 1.2, col);
            }
        }
        Icon::Tab(Tab::Plugins) => {
            p.rect_filled(
                Rect::from_center_size(c + v(0.0, s * 0.08), v(s * 0.55, s * 0.4)),
                2,
                col,
            );
            for dx in [-0.13, 0.13] {
                p.line_segment(
                    [c + v(s * dx, -s * 0.12), c + v(s * dx, -s * 0.4)],
                    Stroke::new(2.0, col),
                );
            }
            p.line_segment(
                [c + v(0.0, s * 0.28), c + v(0.0, s * 0.45)],
                Stroke::new(2.0, col),
            );
        }
        Icon::Tab(Tab::Samples) => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|k| {
                    let x = k as f32 / 24.0;
                    let env = (x * std::f32::consts::PI).sin();
                    c + v((x - 0.5) * s, -(x * 18.0).sin() * env * s * 0.4)
                })
                .collect();
            p.add(Shape::line(pts, st));
        }
        Icon::Tab(Tab::Grid) => {
            for i in 0..3 {
                for j in 0..3 {
                    let q = c + v((i as f32 - 1.0) * s * 0.3, (j as f32 - 1.0) * s * 0.3);
                    p.rect_filled(Rect::from_center_size(q, v(s * 0.2, s * 0.2)), 1, col);
                }
            }
        }
        Icon::Tab(Tab::Wavetables) => {
            for k in 0..3 {
                let off = k as f32 * s * 0.12;
                let pts: Vec<Pos2> = (0..=16)
                    .map(|i| {
                        let x = i as f32 / 16.0;
                        c + v(
                            (x - 0.5) * s * 0.8 + off,
                            -(x * std::f32::consts::TAU).sin() * s * 0.22 + off - s * 0.1,
                        )
                    })
                    .collect();
                p.add(Shape::line(
                    pts,
                    Stroke::new(1.2, col.gamma_multiply(1.0 - k as f32 * 0.25)),
                ));
            }
        }
        Icon::Tab(Tab::Patterns) => {
            for i in 0..4 {
                for j in 0..2 {
                    let q = c + v((i as f32 - 1.5) * s * 0.24, (j as f32 - 0.5) * s * 0.3);
                    let on = (i + j * 3) % 3 == 0;
                    p.rect_filled(
                        Rect::from_center_size(q, v(s * 0.17, s * 0.2)),
                        1,
                        if on { col } else { col.gamma_multiply(0.4) },
                    );
                }
            }
        }
        Icon::Tab(Tab::Files) => super::drumpat::folder_icon(p, c, s, col),
        Icon::Tab(Tab::Racks) => {
            // stacked layers
            for k in 0..3 {
                let y = (k as f32 - 1.0) * s * 0.24;
                p.rect_stroke(
                    Rect::from_center_size(c + v(0.0, y), v(s * 0.78, s * 0.18)),
                    2,
                    st,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Icon::Tab(Tab::Midi) => {
            let r = Rect::from_center_size(c, v(s * 0.8, s * 0.6));
            p.rect_stroke(r, 1, st, egui::StrokeKind::Inside);
            for k in 1..4 {
                let x = r.left() + r.width() * k as f32 / 4.0;
                p.rect_filled(
                    Rect::from_min_size(Pos2::new(x - 1.5, r.top()), v(3.0, r.height() * 0.55)),
                    0,
                    col,
                );
            }
        }
    }
}

/// an icon button (`on`: highlighted)
pub(super) fn icon_button(
    ui: &mut egui::Ui,
    ic: Icon,
    on: bool,
    enabled: bool,
    tip: &str,
) -> egui::Response {
    let t = th();
    let (r, resp) = ui.allocate_exact_size(
        egui::vec2(22.0, 20.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let p = ui.painter();
    p.rect_filled(
        r,
        4,
        if on {
            t.sel
        } else if resp.hovered() && enabled {
            well(&t, 1.5)
        } else {
            well(&t, 1.0)
        },
    );
    p.rect_stroke(
        r,
        4,
        egui::Stroke::new(1.0, if on { t.accent } else { t.border }),
        egui::StrokeKind::Inside,
    );
    let col = if !enabled {
        t.ink2.gamma_multiply(0.5)
    } else if on {
        t.ink
    } else {
        t.ink2
    };
    theme::raise(p, r, 4.0, on);
    icon(p, ic, r.center(), 12.0, col);
    resp.on_hover_text(tip)
}

enum Op {
    Select(Item),
    Load(Item),
    Act(Item, Act),
    Effect(Item),
    Preview(Item),
    StopPreview,
    Fav(String),
    Rate(String, u8),
    AddFolder,
    RemoveFolder(String),
    Rescan,
    FindPlugins,
    GetShowcase,
    PluginRescan,
    RefreshThumb(Item),
    Close,
}

/// The panel's inset "well" colour: the dark screen colour in dark themes,
/// a pale pill in light ones (so ink stays readable), shaded by `k`.
pub(super) fn well(t: &theme::Theme, k: f32) -> Color32 {
    if t.light {
        theme::tone(
            t.pill,
            if k >= 1.0 {
                (k - 1.0) * 0.5
            } else {
                (k - 1.0) * 0.12
            },
        )
    } else {
        t.crt_top.gamma_multiply(k)
    }
}

/// the format as a compact text tag (CWI / CLAP / VST3 / WAV ...): a tinted
/// pill with a coloured outline and coloured text (0.2.4 polish)
fn badge(p: &egui::Painter, r: Rect, f: Fmt) {
    let c = rgb(f.colour());
    let light = th().light;
    let ink = if light {
        theme::tone(c, -0.55)
    } else {
        theme::tone(c, 0.15)
    };
    let rad = if theme::polished() { 4 } else { 2 };
    p.rect_filled(r, rad, c.gamma_multiply(if light { 0.30 } else { 0.16 }));
    p.rect_stroke(
        r,
        rad,
        Stroke::new(1.0, c.gamma_multiply(if light { 1.0 } else { 0.85 })),
        egui::StrokeKind::Inside,
    );
    p.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        f.badge(),
        FontId::proportional(9.5),
        ink,
    );
}

/// the narrowest the Browser can be dragged; everything inside truncates,
/// wraps or scrolls to fit (wider content must never widen the panel)
pub(super) const BROWSER_MIN_W: f32 = 220.0;

impl App {
    pub(super) fn browser_side(&mut self, ui: &mut egui::Ui) {
        let w = self.extra.browser_w.clamp(BROWSER_MIN_W, 720.0);

        let resp = egui::Panel::right("browser")
            .resizable(true)
            .default_size(w)
            .size_range(BROWSER_MIN_W..=720.0)
            .frame(
                egui::Frame::NONE
                    .fill(th().cart_top)
                    .inner_margin(egui::Margin {
                        left: 6,
                        right: 6,
                        top: 4,
                        bottom: 4,
                    }),
            )
            .show(ui, |ui| {
                // the content lives in a child that does not report its size
                // to the panel: anything too wide is clipped (and truncates,
                // wraps or scrolls on its own) instead of pushing the panel
                // wider than the user dragged it, frame after frame
                let r = ui.max_rect();
                let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(*ui.layout()));
                c.set_clip_rect(ui.clip_rect());
                self.browser_panel(&mut c);
                self.br.want_w = c.min_rect().width();
                let h = c.min_rect().height().min(r.height());
                ui.allocate_rect(
                    Rect::from_min_size(r.min, egui::vec2(r.width(), h)),
                    Sense::hover(),
                );
            });
        let nw = resp.response.rect.width();
        if (nw - self.extra.browser_w).abs() > 0.5 && !ui.ctx().input(|i| i.pointer.any_down()) {
            self.extra.browser_w = nw;
            self.save_settings();
        }
    }

    fn browser_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let t = th();
        let full = ui.max_rect().expand2(egui::vec2(6.0, 4.0));
        self.br.rect = full;
        if theme::polished() {
            // the polished look: a soft gradient and a shadow along the left edge
            theme::gradient(
                ui.painter(),
                full,
                theme::tone(t.cart_top, 0.03),
                t.cart_bot,
            );
            for (k, a) in [(0.0f32, 70u8), (1.0, 40), (2.0, 18)] {
                ui.painter().vline(
                    full.left() + k,
                    full.y_range(),
                    egui::Stroke::new(1.0, Color32::from_black_alpha(a)),
                );
            }
        }
        // accent edge (the panel's spine)
        ui.painter().rect_filled(
            Rect::from_min_max(Pos2::new(full.right() - 3.0, full.top()), full.max),
            0,
            t.accent,
        );
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                self.br.focus = full.contains(p);
            }
        }
        self.br_refresh();
        let mut ops: Vec<Op> = vec![];
        let tab = self.br.filter.tab;
        // ---- header: the BROWSER chip, a scan in progress, ••• and ×
        ui.horizontal(|ui| {
            let g = egui::RichText::new("BROWSER")
                .strong()
                .size(11.0)
                .color(Color32::from_rgb(0x1a, 0x1c, 0x1f))
                .background_color(rgb(Fmt::Cwi.colour()));
            ui.label(g);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("×")
                    .on_hover_text("hide the Browser (Ctrl+B)")
                    .clicked()
                {
                    ops.push(Op::Close);
                }
                ui.menu_button("•••", |ui| self.br_menu(ui, &mut ops))
                    .response
                    .on_hover_text("rescan, remove a location, find plugins, thumbnails");
                let busy = self
                    .br
                    .scan_count()
                    .map(|n| format!("{n} files"))
                    .or_else(|| {
                        self.plug
                            .scan
                            .is_some()
                            .then(|| "scanning plugins".to_string())
                    });
                if let Some(s) = busy {
                    ui.spinner();
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(s).small().color(t.ink2))
                                .truncate(),
                        );
                    });
                }
            });
        });
        // ---- categories: two rows of three short tiles, the sixth is More
        // (it takes the name of the category picked from it)
        let tw = ((ui.available_width() - 6.0) / 3.0).floor();
        let mut more_r = Rect::NOTHING;
        for row in 0..2 {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for col in 0..3 {
                    let k = row * 3 + col;
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(tw, 28.0), Sense::click());
                    if let Some(&tb) = Tab::MAIN.get(k) {
                        if self.br_tile(ui, r, resp, Some(tb)).clicked() {
                            self.br_set_tab(tb);
                            self.br.pop = BrPop::None;
                        }
                    } else {
                        more_r = r;
                        if self.br_tile(ui, r, resp, None).clicked() {
                            self.br.pop = if self.br.pop == BrPop::More {
                                BrPop::None
                            } else {
                                BrPop::More
                            };
                        }
                    }
                }
            });
        }
        // ---- search + Filter (the tabs with their own body: search only)
        ui.add_space(3.0);
        let own = matches!(tab, Tab::Patterns | Tab::Files | Tab::Racks);
        let mut filter_r = Rect::NOTHING;
        ui.horizontal(|ui| {
            let fw = 88.0;
            let sw = if own {
                ui.available_width()
            } else {
                ui.available_width() - fw - ui.spacing().item_spacing.x
            };
            let what = match tab {
                Tab::Plugins | Tab::Midi => tab.name().to_string(),
                _ => tab.name().to_lowercase(),
            };
            let se = ui.add(
                egui::TextEdit::singleline(&mut self.br.filter.query)
                    .id_salt("br_search")
                    .hint_text(format!("Search {what}"))
                    .desired_width(sw),
            );
            if std::mem::take(&mut self.br.search_focus) {
                se.request_focus();
            }
            if !own {
                filter_r = self.br_filter_button(ui, fw);
            }
        });
        // ---- the open popover (drawn on top of everything below)
        match self.br.pop {
            BrPop::More => self.br_more_pop(&ctx, more_r),
            BrPop::Filter if !own => self.br_filter_pop(&ctx, filter_r, full, &mut ops),
            BrPop::Filter => self.br.pop = BrPop::None,
            BrPop::None => {}
        }
        // ---- Drum Patterns / Files / My Racks: their own body
        if own {
            if tab == Tab::Patterns {
                self.patterns_body(ui);
            } else if tab == Tab::Racks {
                self.racks_body(ui);
            } else {
                self.files_body(ui);
            }
            self.br_apply_ops(ops, &ctx);
            return;
        }
        // ---- the filters that are on, as removable pills
        self.br_pills(ui);
        // ---- the Effects tab: where a double-click adds, one slim line
        if tab == Tab::Effects {
            let to = self.devices_target_label();
            let shown = self.devices_visible();
            let r = ui
                .horizontal(|ui| {
                    ui.label(egui::RichText::new("Adds to").small().color(t.ink2));
                    ui.add(
                        egui::Label::new(egui::RichText::new(to).small().strong().color(t.accent))
                            .truncate()
                            .sense(Sense::click()),
                    )
                })
                .inner;
            let r = r.on_hover_text(if shown {
                "double-click an effect to add it to the end of this chain; drag it into the Rack to put it in between, or onto any track"
            } else {
                "double-click an effect to add it to the end of this chain, or drag it onto any track; click here to show the chain (Ctrl+7)"
            });
            if r.clicked() && !shown {
                self.vtab_select(super::viewbar::VTab::Devices);
            }
        }
        // ---- VST & CLAP: a plugin file with several classes / programs
        // lists its main instrument; the rest on request (a search finds them)
        if tab == Tab::Plugins {
            let extra = self
                .br
                .items
                .iter()
                .filter(|i| i.sub_program && i.tab == Tab::Plugins)
                .count();
            if extra > 0 || self.br.filter.all_programs {
                ui.checkbox(
                    &mut self.br.filter.all_programs,
                    egui::RichText::new(format!("Show all programs ({extra} more)")).small(),
                )
                .on_hover_text(
                    "some plugin files hold several programs (multi-out variants, bundled effects or MIDI tools); the list shows each file's main instrument, tick this to list them all. A search always finds them.",
                );
            }
        }
        // ---- results header
        let n = self.br.results.len();
        ui.horizontal(|ui| {
            let label = egui::RichText::new(format!("{n} result{}", if n == 1 { "" } else { "s" }))
                .small()
                .color(t.ink2);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("br_sort")
                    .selected_text(match self.br.filter.sort {
                        Sort::Name => "Sort: Name",
                        Sort::Type => "Sort: Type",
                        Sort::Rating => "Sort: Rating",
                    })
                    .width(96.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.br.filter.sort, Sort::Name, "Name");
                        ui.selectable_value(&mut self.br.filter.sort, Sort::Type, "Type");
                        ui.selectable_value(&mut self.br.filter.sort, Sort::Rating, "Rating");
                    });
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(label).truncate());
                });
            });
        });
        // ---- results (virtualised rows)
        // the preview bar (32 px) at the bottom; the list takes the rest
        let bottom_h = 34.0;
        let list_h = (ui.available_height() - bottom_h).max(60.0);
        let thumbs =
            self.extra.browser_thumbs && matches!(self.br.filter.tab, Tab::Plugins | Tab::Effects);
        let row_h = if thumbs { 40.0 } else { 32.0 };
        // prefetch thumbnails for the rows about to be drawn
        if thumbs {
            let first = (self.br.list_off / row_h).floor().max(0.0) as usize;
            let vis = ((list_h / row_h).ceil() as usize).saturating_add(3);
            let ctx = ui.ctx().clone();
            for k in first..n.min(first + vis) {
                let it = &self.br.items[self.br.results[k]];
                if let Src::Plugin(f) = &it.src {
                    let f = f.clone();
                    let _ = self.br_thumb(&ctx, &f);
                }
            }
        }
        let sel = self.br.sel.clone();
        let scroll_to = std::mem::take(&mut self.br.scroll_sel)
            .then(|| {
                sel.as_ref().and_then(|k| {
                    self.br
                        .results
                        .iter()
                        .position(|&i| &self.br.items[i].key == k)
                })
            })
            .flatten();
        let mut sa = egui::ScrollArea::vertical()
            .id_salt("br_results")
            .max_height(list_h)
            .auto_shrink([false, false]);
        if let Some(k) = scroll_to {
            // keep the selection in view (one row above the middle)
            let off = (k as f32 * (row_h + ui.spacing().item_spacing.y) - list_h / 2.0).max(0.0);
            let cur = self.br.list_off;
            if (k as f32 * row_h) < cur || (k as f32 + 1.0) * row_h > cur + list_h {
                sa = sa.vertical_scroll_offset(off);
            }
        }
        let items = &self.br.items;
        let results = &self.br.results;
        let favs = &self.extra.browser_favs;
        let ratings = &self.extra.browser_ratings;
        let cur_slot = self.instr;
        let view_tl = self.view == View::Timeline;
        let ch = self.ch;
        // snapshot so the row closure does not borrow self.br twice
        let thumb_tex: std::collections::HashMap<String, (egui::TextureId, egui::Vec2)> = self
            .br
            .thumb_tex
            .iter()
            .map(|(k, t)| (k.clone(), (t.id(), t.size_vec2())))
            .collect();
        let mut row_rects: Vec<(String, Rect)> = vec![];
        let out = sa.show_rows(ui, row_h, n, |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for k in range {
                let it = &items[results[k]];
                row_rects.push((it.key.clone(), Rect::NOTHING));
                let w = ui.available_width();
                let (r, resp) =
                    ui.allocate_exact_size(egui::vec2(w, row_h), Sense::click_and_drag());
                if let Some(last) = row_rects.last_mut() {
                    last.1 = r.intersect(ui.clip_rect());
                }
                let p = ui.painter();
                let on = sel.as_deref() == Some(it.key.as_str());
                if on {
                    p.rect_filled(r, 0, t.sel);
                } else if resp.hovered() {
                    p.rect_filled(r, 0, well(&t, 1.4));
                } else if k % 2 == 1 {
                    p.rect_filled(r, 0, well(&t, 0.6));
                }
                let skin = crate::theme::instskin()
                    .filter(|_| matches!(it.src, Src::Tool(_)));
                let c = skin.map_or(rgb(it.fmt.colour()), |s| s.tint);
                if let Some(s) = skin {
                    // a built-in instrument row: the skin's wash
                    p.rect_filled(r, 0, s.wash);
                }
                p.rect_filled(
                    Rect::from_min_size(r.min, egui::vec2(4.0, r.height())),
                    0,
                    c,
                );
                let mut thumb_id: Option<egui::TextureId> = None;
                let mut thumb_size = egui::vec2(0.0, 0.0);
                if thumbs {
                    if let Src::Plugin(f) = &it.src {
                        let k = crate::plugin::thumb::key(&f.format, &f.desc.id, &f.desc.version);
                        if let Some((tid, s)) = thumb_tex.get(&k) {
                            thumb_id = Some(*tid);
                            let side = crate::plugin::thumb::ROW;
                            let scale = (side / s.x).min(side / s.y);
                            thumb_size = *s * scale;
                        }
                    }
                }
                if let Some(tid) = thumb_id {
                    let tr = Rect::from_center_size(
                        Pos2::new(r.left() + 28.0, r.center().y),
                        thumb_size,
                    );
                    p.rect_filled(tr.expand(1.5), 3.0, t.border);
                    p.image(
                        tid,
                        tr,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else if let Src::Tool(k) = it.src {
                    // ChipWave Instruments: the instrument's glyph tile
                    let b = super::trackpie::Builtin::ALL
                        .get(k as usize)
                        .copied()
                        .unwrap_or(super::trackpie::Builtin::Synth);
                    let g = super::glyphs::glyph_for(b.glyph_tag());
                    let side = (r.height() - 8.0).clamp(16.0, 24.0);
                    super::glyphs::tile(
                        p,
                        Rect::from_center_size(
                            Pos2::new(r.left() + 30.0, r.center().y),
                            egui::vec2(side, side),
                        ),
                        g,
                        c,
                        Color32::WHITE,
                        crate::theme::polished(),
                    );
                } else {
                    badge(
                        p,
                        Rect::from_center_size(
                            Pos2::new(r.left() + 30.0, r.center().y),
                            egui::vec2(40.0, 15.0),
                        ),
                        it.fmt,
                    );
                }
                // stars and heart on the right
                let heart = Rect::from_center_size(
                    Pos2::new(r.right() - 12.0, r.center().y),
                    egui::vec2(16.0, 16.0),
                );
                let stars = Rect::from_min_max(
                    Pos2::new(heart.left() - 56.0, r.top()),
                    Pos2::new(heart.left() - 2.0, r.bottom()),
                );
                let rating = ratings.get(&it.key).copied().unwrap_or(0);
                let fav = favs.contains(&it.key);
                let hover_pos = resp.hover_pos();
                let hot_star = hover_pos
                    .filter(|q| stars.contains(*q))
                    .map(|q| (((q.x - stars.left()) / 11.0).floor() as u8 + 1).min(5));
                let gold = Color32::from_rgb(0xE0, 0xC4, 0x6C);
                let dim = t.ink2.gamma_multiply(0.4);
                if rating > 0 || resp.hovered() {
                    for s in 0..5u8 {
                        let lit = hot_star.map(|h| s < h).unwrap_or(s < rating);
                        icon(
                            p,
                            Icon::Star,
                            Pos2::new(stars.left() + 5.0 + s as f32 * 11.0, r.center().y),
                            10.0,
                            if lit { gold } else { dim },
                        );
                    }
                }
                if fav || resp.hovered() {
                    icon(
                        p,
                        Icon::Heart,
                        heart.center(),
                        13.0,
                        if fav { gold } else { dim },
                    );
                }
                let text_r = Rect::from_min_max(
                    Pos2::new(r.left() + 56.0, r.top()),
                    Pos2::new(stars.left() - 4.0, r.bottom()),
                );
                let tp = p.with_clip_rect(text_r);
                tp.text(
                    Pos2::new(text_r.left(), r.top() + 10.0),
                    egui::Align2::LEFT_CENTER,
                    &it.name,
                    FontId::proportional(12.5),
                    if on {
                        t.ink
                    } else {
                        t.ink.gamma_multiply(0.92)
                    },
                );
                tp.text(
                    Pos2::new(text_r.left(), r.top() + 23.0),
                    egui::Align2::LEFT_CENTER,
                    &it.detail,
                    FontId::proportional(10.5),
                    t.ink2,
                );
                // input
                if resp.clicked() {
                    let q = resp.interact_pointer_pos().unwrap_or(r.center());
                    if heart.expand(2.0).contains(q) {
                        ops.push(Op::Fav(it.key.clone()));
                    } else if stars.contains(q) {
                        ops.push(Op::Rate(it.key.clone(), hot_star.unwrap_or(0)));
                    } else {
                        ops.push(Op::Select(it.clone()));
                    }
                }
                if resp.double_clicked() {
                    ops.push(Op::Load(it.clone()));
                }
                if resp.drag_started() {
                    ops.push(Op::Select(it.clone()));
                    self.br.drag = Some(it.clone());
                }
                let resp = if thumbs {
                    if let Src::Plugin(f) = &it.src {
                        let k = crate::plugin::thumb::key(&f.format, &f.desc.id, &f.desc.version);
                        let tip_path = f.path.clone();
                        let tip_name = it.name.clone();
                        let tip_detail = format!("{} · {}", it.fmt.what(), it.category);
                        if let Some((tid, s)) = thumb_tex.get(&k).copied() {
                            resp.on_hover_ui(move |ui| {
                                ui.label(egui::RichText::new(&tip_name).strong());
                                ui.label(egui::RichText::new(&tip_detail).small());
                                let side = crate::plugin::thumb::PREVIEW;
                                let scale = (side / s.x).min(side / s.y).min(1.0);
                                ui.add(
                                    egui::Image::new((tid, s * scale)).maintain_aspect_ratio(true),
                                );
                                ui.label(egui::RichText::new(&tip_path).small().weak());
                            })
                        } else {
                            resp.on_hover_text(tip_path)
                        }
                    } else {
                        resp.on_hover_text(match &it.src {
                            Src::File(p) => p.display().to_string(),
                            Src::Plugin(f) => f.path.clone(),
                            _ => format!("{} · {}", it.fmt.what(), it.category),
                        })
                    }
                } else {
                    resp.on_hover_text(match &it.src {
                        Src::File(p) => p.display().to_string(),
                        Src::Plugin(f) => f.path.clone(),
                        Src::Tool(_) => format!(
                            "{} · drag onto a track (or below the tracks for a new one), or double-click for a new track",
                            it.detail
                        ),
                        _ => format!("{} · {}", it.fmt.what(), it.category),
                    })
                };
                resp.context_menu(|ui| {
                    ui.label(egui::RichText::new(&it.name).strong());
                    let inst = !it.is_effect()
                        && !it.fmt.planned()
                        && it.tab != Tab::Songs
                        && !matches!(it.src, Src::Demo(_));
                    if it.tab == Tab::Songs {
                        if ui
                            .add_enabled(!it.fmt.planned(), egui::Button::new("Open song"))
                            .clicked()
                        {
                            ops.push(Op::Load(it.clone()));
                            ui.close();
                        }
                    } else if it.is_effect() {
                        if ui
                            .button(format!("Add to instrument {cur_slot:02X}'s FX chain"))
                            .clicked()
                        {
                            ops.push(Op::Effect(it.clone()));
                            ui.close();
                        }
                    } else if inst {
                        // (ChipWave Instruments always go onto a track)
                        let tool = matches!(it.src, Src::Tool(_));
                        if !tool && ui.button("Add as a new instrument").clicked() {
                            ops.push(Op::Act(it.clone(), Act::New));
                            ui.close();
                        }
                        if cur_slot > 0
                            && !tool
                            && ui
                                .button(format!("Replace instrument {cur_slot:02X}'s sound"))
                                .clicked()
                        {
                            ops.push(Op::Act(it.clone(), Act::Replace(cur_slot)));
                            ui.close();
                        }
                        if ui
                            .button(format!("Load onto track {:02}", ch + 1))
                            .clicked()
                        {
                            ops.push(Op::Act(it.clone(), Act::Track(ch)));
                            ui.close();
                        }
                        if ui.button("New track with it").clicked() {
                            ops.push(Op::Act(it.clone(), Act::NewTrack(usize::MAX)));
                            ui.close();
                        }
                        if it.fmt.is_sample()
                            && view_tl
                            && ui
                                .button(format!(
                                    "Audio clip on track {:02} at the play marker",
                                    ch + 1
                                ))
                                .clicked()
                        {
                            ops.push(Op::Load(it.clone()));
                            ui.close();
                        }
                        if it.previewable() && ui.button("Preview (C-3)").clicked() {
                            ops.push(Op::Preview(it.clone()));
                            ui.close();
                        }
                    }
                    if matches!(it.src, Src::Plugin(_)) && ui.button("Refresh thumbnail").clicked()
                    {
                        ops.push(Op::RefreshThumb(it.clone()));
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button(if fav {
                            "Remove from favourites"
                        } else {
                            "Add to favourites"
                        })
                        .clicked()
                    {
                        ops.push(Op::Fav(it.key.clone()));
                        ui.close();
                    }
                    ui.horizontal(|ui| {
                        ui.label("Rating");
                        for s in 0..=5u8 {
                            let l = if s == 0 {
                                "none".to_string()
                            } else {
                                s.to_string()
                            };
                            if ui.selectable_label(rating == s, l).clicked() {
                                ops.push(Op::Rate(it.key.clone(), s));
                                ui.close();
                            }
                        }
                    });
                    if let Src::File(p) = &it.src {
                        if ui.button("Copy path").clicked() {
                            ui.ctx().copy_text(p.display().to_string());
                            ui.close();
                        }
                    }
                });
            }
        });
        self.br.list_off = out.state.clamped_offset().y;
        self.br.row_rects = row_rects;
        if n == 0 {
            let msg = match tab {
                Tab::Samples | Tab::Midi | Tab::Songs if self.extra.browser_dirs.is_empty() => {
                    "Nothing here yet: add a folder (Filter › Location › + Add folder…)"
                }
                Tab::Plugins => "No plugins found yet: ••• › Find plugins… searches your drive, or ••• › Get showcase instruments…",
                _ => "No matches: clear the search or a filter",
            };
            ui.add(egui::Label::new(egui::RichText::new(msg).small().color(t.ink2)).wrap());
        }
        // ---- preview bar (the colour key is its "?")
        self.br_preview_bar(ui, ch, &mut ops);
        self.br_apply_ops(ops, &ctx);
    }

    fn br_apply_ops(&mut self, ops: Vec<Op>, ctx: &egui::Context) {
        for op in ops {
            match op {
                Op::Select(it) => {
                    let again = self.br.sel.as_deref() == Some(it.key.as_str());
                    self.br.sel = Some(it.key.clone());
                    if self.extra.browser_autoplay
                        && !again
                        && it.previewable()
                        && self.br.drag.is_none()
                    {
                        self.br_preview(&it);
                    }
                }
                Op::Load(it) => self.br_load(&it, ctx),
                Op::Act(it, Act::NewTrack(_)) => {
                    let n = self.with(|s| s.song.channels);
                    if n >= crate::model::MAX_CHANNELS {
                        self.status = "All 32 tracks are used (the channel limit)".into();
                    } else {
                        self.br_instrument(&it, Act::NewTrack(n));
                    }
                }
                Op::Act(it, a) => self.br_instrument(&it, a),
                Op::Effect(it) => {
                    let slot = self.instr;
                    self.br_drop(&it, DropAt::Fx, ctx);
                    let _ = slot;
                }
                Op::Preview(it) => self.br_preview(&it),
                Op::StopPreview => self.br_stop_preview(),
                Op::Fav(k) => self.br_toggle_fav(&k),
                Op::Rate(k, s) => self.br_rate(&k, s),
                Op::AddFolder => {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title("Add a Browser location")
                        .pick_folder()
                    {
                        self.br_add_folder(p);
                    }
                }
                Op::RemoveFolder(d) => self.br_remove_folder(&d),
                Op::Rescan => self.br_rescan(),
                Op::FindPlugins => self.open_plugin_setup(false),
                Op::GetShowcase => self.open_showcase(),
                Op::PluginRescan => self.start_plugin_scan(true),
                Op::RefreshThumb(it) => {
                    if let Src::Plugin(f) = &it.src {
                        let mut g = crate::plugin::thumb::queue();
                        let q = g.get_or_insert_with(Default::default);
                        q.force_refresh(
                            &f.format,
                            &f.path,
                            &f.desc.id,
                            &f.desc.version,
                            &f.desc.name,
                            &f.desc.vendor,
                        );
                        let k = crate::plugin::thumb::key(&f.format, &f.desc.id, &f.desc.version);
                        self.br.thumb_tex.remove(&k);
                        self.br.thumb_asked.remove(&k);
                        self.status = format!("Refreshing thumbnail for {}…", f.desc.name);
                    }
                }
                Op::Close => {
                    self.br.pop = BrPop::None;
                    self.toggle_browser();
                }
            }
        }
    }

    /// the Location column: (filter, label, count in this tab)
    fn br_loc_values(&self) -> Vec<(LocFilter, String, usize)> {
        let f = &self.br.filter;
        let base = |l: &LocFilter| {
            let mut g = f.clone();
            g.loc = l.clone();
            br::filter(
                &self.br.items,
                &g,
                &self.extra.browser_favs,
                &self.extra.browser_ratings,
            )
            .len()
        };
        let mut v = vec![];
        for (l, name) in [
            (LocFilter::All, "All"),
            (LocFilter::Library, "Library"),
            (LocFilter::Song, "This song"),
            (LocFilter::Favourites, "Favourites"),
        ] {
            let n = base(&l);
            v.push((l, name.to_string(), n));
        }
        for d in &self.extra.browser_dirs {
            let l = LocFilter::Folder(d.clone());
            let name = std::path::Path::new(d)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| d.clone());
            let n = base(&l);
            v.push((l, name, n));
        }
        v
    }

    /// after the panels: a Browser drag in progress (ghost, drop hint,
    /// target highlight) and its drop
    pub(super) fn browser_drag(&mut self, ctx: &egui::Context) {
        let Some(it) = self.br.drag.clone() else {
            return;
        };
        let (pos, down, released) = if let Some(k) = self.dev.shot_at.filter(|_| self.br.shot_drop)
        {
            // screenshot: hold the drag over the left part of device card k
            let p = match self.dev.cards.get(k) {
                Some(r) => Pos2::new(r.left() + r.width() * 0.25, r.center().y),
                None => Pos2::new(self.dev.inst.right() - 40.0, self.dev.inst.center().y),
            };
            (Some(p), true, false)
        } else if self.br.shot_drop {
            let p = Pos2::new(
                self.tlv.lanes.left() + 260.0,
                self.tlv.tracks_bottom() + 40.0,
            );
            (Some(p), true, false)
        } else {
            ctx.input(|i| {
                (
                    i.pointer.latest_pos(),
                    i.pointer.primary_down(),
                    i.pointer.primary_released(),
                )
            })
        };
        let target = pos.and_then(|p| self.br_drop_at(p));
        // a drum pattern over the Timeline: the one drop system (preview,
        // snap, overlap question; docs/unified/PLAN.md Stage 4a)
        let pat = match it.src {
            Src::Pattern(i)
                if self.with(|s| s.song.timeline.is_some())
                    && matches!(target, Some(DropAt::Track(..) | DropAt::NewTrack(_))) =>
            {
                Some(super::dropplan::DropSrc::Pattern(i as u16))
            }
            _ => None,
        };
        if let Some(src) = pat {
            let fine = ctx.input(|i| i.modifiers.shift);
            let plan = pos.and_then(|q| self.drop_plan(q, src, fine));
            if released || !down {
                self.br.drag = None;
                if released {
                    self.pat_stop();
                    self.drop_release(src, plan, false);
                }
                return;
            }
            self.drop_paint(ctx, src, plan.as_ref(), pos, false);
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            ctx.request_repaint();
            return;
        }
        let hint = target.as_ref().and_then(|d| self.br_drop_hint(&it, d));
        if released || !down {
            self.br.drag = None;
            if let (Some(d), true, Some(_)) = (target, released, hint) {
                self.br_drop(&it, d, ctx);
            }
            return;
        }
        let t = th();
        let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::NULL.with("br_drag"));
        let p = ctx.layer_painter(layer);
        // the target
        match &target {
            Some(DropAt::Slot(_)) if pos.is_some_and(|q| self.dev.inst.contains(q)) => {
                p.rect_stroke(
                    self.dev.inst,
                    8,
                    egui::Stroke::new(2.5, t.accent),
                    egui::StrokeKind::Inside,
                );
            }
            Some(DropAt::Slot(s)) => {
                if let Some((_, r)) = self.br.slot_rects.iter().find(|(x, _)| x == s) {
                    p.rect_stroke(
                        *r,
                        3,
                        egui::Stroke::new(2.0, t.accent),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            Some(DropAt::Track(tr, _)) => {
                if let Some((hr, row)) = self.tlv.track_rects(*tr) {
                    p.rect_filled(row, 0, t.accent.gamma_multiply(0.12));
                    p.rect_stroke(
                        hr,
                        3,
                        egui::Stroke::new(2.0, t.accent),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            Some(DropAt::NewTrack(_)) => {
                let y = self.tlv.tracks_bottom();
                let x0 = self.tlv.heads.left();
                let x1 = self.tlv.lanes.right();
                p.line_segment(
                    [Pos2::new(x0, y), Pos2::new(x1, y)],
                    egui::Stroke::new(2.5, t.accent),
                );
                if let Some(h) = &hint {
                    // in the lanes, clear of the "+ Add track" button under the headers
                    let g = p.layout_no_wrap(h.to_string(), FontId::proportional(13.0), t.accent);
                    let at = Pos2::new(self.tlv.lanes.left() + 10.0, y + 12.0);
                    let bg = Rect::from_min_size(
                        at - egui::vec2(6.0, g.size().y / 2.0 + 3.0),
                        g.size() + egui::vec2(12.0, 6.0),
                    );
                    p.rect_filled(bg, 4, t.cart_top);
                    p.text(
                        at,
                        egui::Align2::LEFT_CENTER,
                        h,
                        FontId::proportional(13.0),
                        t.accent,
                    );
                }
            }
            Some(DropAt::Device(i)) if self.dev.vertical => {
                let y = match self.dev.cards.get(*i) {
                    Some(r) => r.top(),
                    None => self
                        .dev
                        .cards
                        .last()
                        .map_or(self.dev.inst.bottom() + super::chains::ROW, |r| r.bottom()),
                };
                super::devices::paint_marker_h(&p, y, self.dev.row.x_range().shrink(8.0));
            }
            Some(DropAt::Device(i)) => {
                let x = super::devices::marker_x(self.dev.inst, &self.dev.cards, *i);
                super::devices::paint_marker(&p, x, self.dev.inst.y_range());
            }
            Some(DropAt::Fx) | Some(DropAt::Central) if hint.is_some() => {
                p.rect_stroke(
                    self.br.central.shrink(2.0),
                    4,
                    egui::Stroke::new(2.0, t.accent.gamma_multiply(0.6)),
                    egui::StrokeKind::Inside,
                );
            }
            _ => {}
        }
        // the ghost
        if let Some(q) = pos {
            let txt = format!("{}  {}", it.fmt.badge(), it.name);
            let g = p.layout_no_wrap(txt, FontId::proportional(12.0), t.ink);
            let hint_txt = hint.clone().unwrap_or_else(|| "can't drop here".into());
            let hg = p.layout_no_wrap(
                hint_txt,
                FontId::proportional(10.5),
                if hint.is_some() { t.accent } else { t.ink2 },
            );
            let w = g.size().x.max(hg.size().x) + 16.0;
            let r = Rect::from_min_size(q + egui::vec2(14.0, 10.0), egui::vec2(w, 36.0));
            p.rect_filled(r, 4, t.cart_top);
            p.rect_stroke(
                r,
                4,
                egui::Stroke::new(1.0, rgb(it.fmt.colour())),
                egui::StrokeKind::Inside,
            );
            p.galley(r.min + egui::vec2(8.0, 4.0), g, t.ink);
            p.galley(r.min + egui::vec2(8.0, 20.0), hg, t.ink2);
        }
        ctx.set_cursor_icon(if hint.is_some() {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::NoDrop
        });
        ctx.request_repaint();
    }
}

// ------------------------------------------------------------ the top (A)

/// the Browser's open popover: the Filter or the More menu (one at a time;
/// a click outside or Esc closes it)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum BrPop {
    #[default]
    None,
    Filter,
    More,
}

/// a category's name on its tile (short where the tile is small)
fn tile_name(tb: Tab) -> &'static str {
    match tb {
        Tab::Patterns => "Patterns",
        tb => tb.name(),
    }
}

/// the tiles' tooltips
fn tab_tip(tb: Tab) -> &'static str {
    match tb {
        Tab::Instruments => "instruments: blank synths, generator presets, your .cwi library, this song's instruments",
        Tab::Effects => "effects: ChipWave's built-ins, Grid FX patches and your effect plugins; double-click adds to the selected track, or drag into the Rack or onto a track",
        Tab::Plugins => "VST3 and CLAP plugins from the plugin scan: each plugin file's main instrument, plus effect plugins",
        Tab::Samples => "WAV, FLAC, MP3 and Ogg files in your locations",
        Tab::Songs => "demo songs and .cwt songs in your locations (MOD/MED: import planned)",
        Tab::Presets => "factory presets: load one into the current instrument",
        Tab::Grid => "Grid starter patches (instruments and FX)",
        Tab::Wavetables => "built-in wavetables and imported wavetable WAVs",
        Tab::Midi => "MIDI files in your locations (import planned)",
        Tab::Patterns => "Drum Patterns: ready-made grooves by style with their own kit; preview in time with the song, drag onto the timeline",
        Tab::Files => "your drives and folders: Home, Music, Desktop, Downloads, network drives",
        Tab::Racks => "My Racks: the racks you saved (Save rack… under the Rack); double-click loads one onto the selected track",
    }
}

/// a rounded chip (tags, categories, the active-filter pills)
fn chip(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let t = th();
    let b = egui::Button::new(egui::RichText::new(text).size(11.5).color(if on {
        t.ink
    } else {
        t.ink2
    }))
    .corner_radius(9)
    .fill(if on { t.sel } else { well(&t, 1.0) })
    .stroke(egui::Stroke::new(1.0, if on { t.accent } else { t.border }));
    ui.add(b)
}

/// a small caps section heading
fn heading(ui: &mut egui::Ui, s: &str) {
    ui.label(egui::RichText::new(s).size(10.0).strong().color(th().ink2));
}

/// a tile's background (the polished look: bevel and glow), with the
/// category colour underlined when it is the open one
fn tile_bg(p: &egui::Painter, r: Rect, on: bool, hot: bool, c: Color32) {
    let t = th();
    let bg = if on {
        well(&t, 1.6)
    } else if hot {
        well(&t, 1.3)
    } else {
        well(&t, 1.0)
    };
    p.rect_filled(r, 5, bg);
    if theme::polished() {
        let mut m = egui::Mesh::default();
        theme::round_mesh(
            &mut m,
            r,
            [5.0; 4],
            theme::tone(bg, 0.07),
            theme::tone(bg, -0.10),
        );
        p.add(egui::Shape::mesh(m));
        theme::raise(p, r, 5.0, on);
        if on {
            theme::glow(p, r, 5, c.gamma_multiply(0.6));
        }
    }
    if on {
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(r.left() + 4.0, r.bottom() - 2.0),
                Pos2::new(r.right() - 4.0, r.bottom()),
            ),
            1,
            c,
        );
    }
}

/// one line of text at `pos` (left-centre), ending in "…" past `max_w`
fn text_trunc(p: &egui::Painter, pos: Pos2, s: &str, size: f32, col: Color32, max_w: f32) {
    let mut job =
        egui::text::LayoutJob::simple_singleline(s.into(), FontId::proportional(size), col);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(8.0));
    let g = p.layout_job(job);
    let h = g.size().y;
    p.galley(pos - egui::vec2(0.0, h / 2.0), g, col);
}

/// a small "v" chevron
fn chevron(p: &egui::Painter, c: Pos2, col: Color32) {
    p.add(egui::Shape::line(
        vec![
            c + egui::vec2(-3.5, -1.8),
            c + egui::vec2(0.0, 1.8),
            c + egui::vec2(3.5, -1.8),
        ],
        egui::Stroke::new(1.5, col),
    ));
}

impl App {
    /// switch category: its own category / creator / tags start cleared
    /// (search, favourites and location stay)
    pub(super) fn br_set_tab(&mut self, tb: Tab) {
        if self.br.filter.tab != tb {
            self.br.filter.tab = tb;
            self.br.filter.category = None;
            self.br.filter.creator = None;
            self.br.filter.tags.clear();
            self.br_refresh();
        }
    }

    /// the ••• menu: the panel's housekeeping (each action lives only here)
    fn br_menu(&mut self, ui: &mut egui::Ui, ops: &mut Vec<Op>) {
        if ui
            .add_enabled(
                self.br.scan_count().is_none(),
                egui::Button::new("Rescan locations"),
            )
            .on_hover_text("rescan the library and your folders (in the background)")
            .clicked()
        {
            ops.push(Op::Rescan);
            ui.close();
        }
        let dirs = self.extra.browser_dirs.clone();
        if !dirs.is_empty() {
            ui.menu_button("Remove a location", |ui| {
                for d in dirs {
                    if ui.button(format!("× {d}")).clicked() {
                        ops.push(Op::RemoveFolder(d));
                        ui.close();
                    }
                }
            });
        }
        ui.separator();
        if ui
            .button("Find plugins…")
            .on_hover_text("search your drive for VST3 and CLAP plugin folders")
            .clicked()
        {
            ops.push(Op::FindPlugins);
            ui.close();
        }
        if ui
            .button("Get showcase instruments…")
            .on_hover_text("download free, open-source instruments from their creators")
            .clicked()
        {
            ops.push(Op::GetShowcase);
            ui.close();
        }
        if ui
            .add_enabled(
                self.plug.scan.is_none(),
                egui::Button::new("Rescan plugins"),
            )
            .on_hover_text("scan your plugin folders again (in the background)")
            .clicked()
        {
            ops.push(Op::PluginRescan);
            ui.close();
        }
        ui.separator();
        ui.checkbox(&mut self.extra.browser_thumbs, "Plugin thumbnails")
            .on_hover_text("show a small screenshot of each plugin's editor (or a generated card)");
    }

    /// one category tile (`main` = None: the More tile)
    fn br_tile(
        &self,
        ui: &egui::Ui,
        r: Rect,
        resp: egui::Response,
        main: Option<Tab>,
    ) -> egui::Response {
        let t = th();
        let cur = self.br.filter.tab;
        let more = main.is_none();
        let (tb, on) = match main {
            Some(tb) => (Some(tb), tb == cur),
            None => {
                let m = Tab::MORE.contains(&cur);
                (m.then_some(cur), m)
            }
        };
        let c = tb.map(|x| rgb(x.fmt().colour())).unwrap_or(t.ink2);
        let p = ui.painter();
        tile_bg(p, r, on, resp.hovered(), c);
        // narrow panels: the label without the icon
        let mut x = r.left() + 7.0;
        if r.width() >= 92.0 {
            let ic = Pos2::new(r.left() + 15.0, r.center().y);
            match tb {
                Some(tb) => icon(p, Icon::Tab(tb), ic, 14.0, c),
                None => {
                    for dx in [-4.0, 0.0, 4.0] {
                        p.circle_filled(ic + egui::vec2(dx, 0.0), 1.6, t.ink2);
                    }
                }
            }
            x = r.left() + 27.0;
        }
        let label = tb.map(tile_name).unwrap_or("More");
        let right = if more {
            r.right() - 16.0
        } else {
            r.right() - 4.0
        };
        text_trunc(
            p,
            Pos2::new(x, r.center().y),
            label,
            12.0,
            if on { t.ink } else { t.ink2 },
            right - x,
        );
        if more {
            chevron(p, Pos2::new(r.right() - 9.0, r.center().y), t.ink2);
        }
        let tip = match (main, tb) {
            (Some(tb), _) => tab_tip(tb).to_string(),
            (None, Some(tb)) => format!("{}\n\nClick for the other categories", tab_tip(tb)),
            (None, None) => {
                "My Racks, Presets, Grid, Wavetables, MIDI, Drum Patterns, Files".to_string()
            }
        };
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, label));
        resp.on_hover_text(tip)
    }

    /// the Filter button: "Filter", or "Filter · n" when n filters are on
    fn br_filter_button(&mut self, ui: &mut egui::Ui, w: f32) -> Rect {
        let t = th();
        let n = self.br_filter_count();
        let open = self.br.pop == BrPop::Filter;
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(r, 4, if open { t.sel } else { well(&t, 1.0) });
        p.rect_stroke(
            r,
            4,
            egui::Stroke::new(1.0, if open || n > 0 { t.accent } else { t.border }),
            egui::StrokeKind::Inside,
        );
        // a funnel
        let c = Pos2::new(r.left() + 11.0, r.center().y);
        p.add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(-5.0, -4.5),
                c + egui::vec2(5.0, -4.5),
                c + egui::vec2(1.2, 0.5),
                c + egui::vec2(-1.2, 0.5),
            ],
            t.ink2,
            egui::Stroke::NONE,
        ));
        p.rect_filled(
            Rect::from_min_max(c + egui::vec2(-1.2, 0.0), c + egui::vec2(1.2, 5.0)),
            0,
            t.ink2,
        );
        let label = if n > 0 {
            format!("Filter · {n}")
        } else {
            "Filter".into()
        };
        text_trunc(
            p,
            Pos2::new(r.left() + 20.0, r.center().y),
            &label,
            12.0,
            t.ink,
            w - 36.0,
        );
        chevron(p, Pos2::new(r.right() - 9.0, r.center().y), t.ink2);
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, label.clone())
        });
        if resp
            .on_hover_text("favourites, tags, category, creator and location")
            .clicked()
        {
            self.br.pop = if open { BrPop::None } else { BrPop::Filter };
        }
        r
    }

    /// how many filters are on (the Filter button's count)
    pub(super) fn br_filter_count(&self) -> usize {
        let f = &self.br.filter;
        f.tags.len()
            + f.category.is_some() as usize
            + f.creator.is_some() as usize
            + (f.loc != LocFilter::All) as usize
    }

    /// the filters that are on, as pills under the search; × removes one
    fn br_pills(&mut self, ui: &mut egui::Ui) {
        let f = &self.br.filter;
        // 0 location (favourites or a place), 1 category, 2 creator, 3 tag
        let mut pills: Vec<(u8, String)> = vec![];
        match &f.loc {
            LocFilter::All => {}
            LocFilter::Favourites => pills.push((0, "♥ Favourites".into())),
            lf => {
                let name = self
                    .br_loc_values()
                    .into_iter()
                    .find(|l| &l.0 == lf)
                    .map(|l| l.1)
                    .unwrap_or_else(|| "Location".into());
                pills.push((0, name));
            }
        }
        if let Some(c) = &f.category {
            pills.push((
                1,
                if c.is_empty() {
                    "—".into()
                } else {
                    c.clone()
                },
            ));
        }
        if let Some(c) = &f.creator {
            pills.push((
                2,
                if c.is_empty() {
                    "—".into()
                } else {
                    c.clone()
                },
            ));
        }
        for tg in &f.tags {
            pills.push((3, tg.to_string()));
        }
        if pills.is_empty() {
            return;
        }
        let mut drop = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
            for (k, s) in &pills {
                if chip(ui, &format!("{s}  ×"), true)
                    .on_hover_text("remove this filter")
                    .clicked()
                {
                    drop = Some((*k, s.clone()));
                }
            }
        });
        if let Some((k, s)) = drop {
            let f = &mut self.br.filter;
            match k {
                0 => f.loc = LocFilter::All,
                1 => f.category = None,
                2 => f.creator = None,
                _ => f.tags.retain(|x| *x != s),
            }
            self.br_refresh();
        }
    }

    /// close `self.br.pop` on a press outside `area` and `trigger` (not
    /// while one of its own drop-downs is open)
    fn br_pop_outside(&mut self, ctx: &egui::Context, area: Rect, trigger: Rect) {
        if !ctx.input(|i| i.pointer.any_pressed()) || egui::Popup::is_any_open(ctx) {
            return;
        }
        if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
            if !area.contains(p) && !trigger.contains(p) {
                self.br.pop = BrPop::None;
            }
        }
    }

    /// the Filter popover: favourites, tags, category, creator, location
    fn br_filter_pop(&mut self, ctx: &egui::Context, at: Rect, full: Rect, ops: &mut Vec<Op>) {
        let tab = self.br.filter.tab;
        let present: Vec<&'static str> = br::TAG_NAMES
            .iter()
            .copied()
            .filter(|tg| {
                self.br
                    .items
                    .iter()
                    .any(|i| i.in_tab(tab) && i.tags.contains(tg))
            })
            .collect();
        let favs = self.extra.browser_favs.clone();
        let n_fav = self
            .br
            .items
            .iter()
            .filter(|i| i.in_tab(tab) && favs.contains(&i.key))
            .count();
        let cats = br::column_values(
            &self.br.items,
            &self.br.filter,
            &favs,
            |i| &i.category,
            true,
            false,
        );
        let crs = br::column_values(
            &self.br.items,
            &self.br.filter,
            &favs,
            |i| &i.creator,
            false,
            true,
        );
        // Favourites is the chip above, not a place
        let locs: Vec<_> = self
            .br_loc_values()
            .into_iter()
            .filter(|l| l.0 != LocFilter::Favourites)
            .collect();
        let w = full.width() - 12.0;
        let pos = Pos2::new(full.left() + 5.0, at.bottom() + 3.0);
        let max_h = (full.bottom() - 40.0 - pos.y).max(120.0);
        let mut changed = false;
        let area = egui::Area::new(egui::Id::unique("br_filter_pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(w - 14.0);
                    egui::ScrollArea::vertical()
                        .id_salt("br_filter_pop_scroll")
                        .max_height(max_h)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                            ui.horizontal(|ui| {
                                heading(ui, "SHOW");
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("Clear all")
                                            .on_hover_text("every filter off (the search stays)")
                                            .clicked()
                                        {
                                            let f = &mut self.br.filter;
                                            f.tags.clear();
                                            f.category = None;
                                            f.creator = None;
                                            f.loc = LocFilter::All;
                                            changed = true;
                                        }
                                    },
                                );
                            });
                            ui.horizontal_wrapped(|ui| {
                                let fav = self.br.filter.loc == LocFilter::Favourites;
                                if chip(ui, &format!("♥ Favourites  {n_fav}"), fav)
                                    .on_hover_text(
                                        "show only your favourites (the heart on a row adds one)",
                                    )
                                    .clicked()
                                {
                                    self.br.filter.loc = if fav {
                                        LocFilter::All
                                    } else {
                                        LocFilter::Favourites
                                    };
                                    changed = true;
                                }
                                for tg in &present {
                                    let on = self.br.filter.tags.contains(tg);
                                    if chip(ui, tg, on).clicked() {
                                        if on {
                                            self.br.filter.tags.retain(|x| x != tg);
                                        } else {
                                            self.br.filter.tags.push(tg);
                                        }
                                        changed = true;
                                    }
                                }
                            });
                            ui.add_space(2.0);
                            heading(ui, "CATEGORY");
                            ui.horizontal_wrapped(|ui| {
                                let all: usize = cats.iter().map(|c| c.1).sum();
                                if chip(
                                    ui,
                                    &format!("All  {all}"),
                                    self.br.filter.category.is_none(),
                                )
                                .clicked()
                                {
                                    self.br.filter.category = None;
                                    changed = true;
                                }
                                for (c, n) in &cats {
                                    let on = self.br.filter.category.as_deref() == Some(c.as_str());
                                    let name = if c.is_empty() { "—" } else { c.as_str() };
                                    if chip(ui, &format!("{name}  {n}"), on).clicked() {
                                        self.br.filter.category =
                                            if on { None } else { Some(c.clone()) };
                                        changed = true;
                                    }
                                }
                            });
                            ui.add_space(2.0);
                            let cw = (w - 110.0).max(80.0);
                            egui::Grid::new("br_filter_pop_grid")
                                .num_columns(2)
                                .spacing(egui::vec2(8.0, 6.0))
                                .show(ui, |ui| {
                                    heading(ui, "CREATOR");
                                    let cur = self.br.filter.creator.clone();
                                    egui::ComboBox::from_id_salt("br_creator")
                                        .selected_text(
                                            cur.clone().unwrap_or_else(|| "Anyone".into()),
                                        )
                                        .width(cw)
                                        .truncate()
                                        .show_ui(ui, |ui| {
                                            if ui.selectable_label(cur.is_none(), "Anyone").clicked()
                                            {
                                                self.br.filter.creator = None;
                                                changed = true;
                                            }
                                            for (c, n) in &crs {
                                                let name =
                                                    if c.is_empty() { "—" } else { c.as_str() };
                                                if ui
                                                    .selectable_label(
                                                        cur.as_deref() == Some(c.as_str()),
                                                        format!("{name}  ({n})"),
                                                    )
                                                    .clicked()
                                                {
                                                    self.br.filter.creator = Some(c.clone());
                                                    changed = true;
                                                }
                                            }
                                        });
                                    ui.end_row();
                                    heading(ui, "LOCATION");
                                    let cur = locs
                                        .iter()
                                        .find(|l| l.0 == self.br.filter.loc)
                                        .map(|l| l.1.clone())
                                        .unwrap_or_else(|| "All".into());
                                    egui::ComboBox::from_id_salt("br_loc")
                                        .selected_text(cur)
                                        .width(cw)
                                        .truncate()
                                        .show_ui(ui, |ui| {
                                            for (lf, name, n) in &locs {
                                                let r = ui.selectable_label(
                                                    &self.br.filter.loc == lf,
                                                    format!("{name}  ({n})"),
                                                );
                                                let r = if let LocFilter::Folder(d) = lf {
                                                    r.on_hover_text(d.as_str())
                                                } else {
                                                    r
                                                };
                                                if r.clicked() {
                                                    self.br.filter.loc = lf.clone();
                                                    changed = true;
                                                }
                                            }
                                        });
                                    ui.end_row();
                                    ui.label("");
                                    if ui
                                        .small_button("+ Add folder…")
                                        .on_hover_text(
                                            "add a folder of samples, songs, instruments… (scanned in the background); ••• removes one",
                                        )
                                        .clicked()
                                    {
                                        ops.push(Op::AddFolder);
                                    }
                                    ui.end_row();
                                });
                        });
                });
            });
        if changed {
            self.br_refresh();
        }
        self.br_pop_outside(ctx, area.response.rect, at);
    }

    /// the More menu: the other seven categories, a line about each
    fn br_more_pop(&mut self, ctx: &egui::Context, at: Rect) {
        let t = th();
        let w = 232.0;
        let pos = Pos2::new((at.right() - w).min(at.left()), at.bottom() + 3.0);
        let mut pick = None;
        let area = egui::Area::new(egui::Id::unique("br_more_pop"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(w - 14.0);
                    for tb in Tab::MORE {
                        let (r, resp) =
                            ui.allocate_exact_size(egui::vec2(w - 14.0, 34.0), Sense::click());
                        let p = ui.painter();
                        if tb == self.br.filter.tab {
                            p.rect_filled(r, 3, t.sel);
                        } else if resp.hovered() {
                            p.rect_filled(r, 3, well(&t, 1.4));
                        }
                        let c = rgb(tb.fmt().colour());
                        icon(
                            p,
                            Icon::Tab(tb),
                            Pos2::new(r.left() + 13.0, r.center().y),
                            14.0,
                            c,
                        );
                        text_trunc(
                            p,
                            Pos2::new(r.left() + 28.0, r.top() + 11.0),
                            tb.name(),
                            12.5,
                            t.ink,
                            r.width() - 32.0,
                        );
                        text_trunc(
                            p,
                            Pos2::new(r.left() + 28.0, r.top() + 25.0),
                            tb.blurb(),
                            10.5,
                            t.ink2,
                            r.width() - 32.0,
                        );
                        resp.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::accesskit::Role::Button,
                                true,
                                tb.name(),
                            )
                        });
                        if resp.on_hover_text(tab_tip(tb)).clicked() {
                            pick = Some(tb);
                        }
                    }
                });
            });
        if let Some(tb) = pick {
            self.br_set_tab(tb);
            self.br.pop = BrPop::None;
            self.br.focus = true;
        } else {
            self.br_pop_outside(ctx, area.response.rect, at);
        }
    }

    /// the preview bar along the bottom: play / stop, where it plays, Auto,
    /// and "?" with the colour key
    fn br_preview_bar(&mut self, ui: &mut egui::Ui, ch: usize, ops: &mut Vec<Op>) {
        let t = th();
        let full = self.br.rect;
        let bar = Rect::from_min_max(
            Pos2::new(full.left(), full.bottom() - 32.0),
            Pos2::new(full.right() - 3.0, full.bottom()),
        );
        ui.painter().rect_filled(bar, 0, well(&t, 0.9));
        ui.painter()
            .hline(bar.x_range(), bar.top(), egui::Stroke::new(1.0, t.border));
        let selected = self.br_selected();
        let playing = self.br.preview_active();
        let tname = self.with(|s| s.song.channel_names.get(ch).cloned().unwrap_or_default());
        let text = format!("Preview on C-3 · plays through track {:02} {tname}", ch + 1);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(bar.shrink2(egui::vec2(6.0, 4.0))),
            |ui| {
                ui.horizontal_centered(|ui| {
                    let can = selected.as_ref().is_some_and(|i| i.previewable());
                    let b = icon_button(
                        ui,
                        if playing { Icon::Stop } else { Icon::Play },
                        playing,
                        can || playing,
                        "preview the selected item (Space while the Browser has the keyboard)",
                    );
                    if b.clicked() {
                        if playing {
                            ops.push(Op::StopPreview);
                        } else if let Some(i) = &selected {
                            ops.push(Op::Preview(i.clone()));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // the colour key, on hover
                        let (r, resp) =
                            ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::hover());
                        let hot = resp.hovered();
                        ui.painter().circle_stroke(
                            r.center(),
                            8.0,
                            egui::Stroke::new(1.0, if hot { t.ink } else { t.ink2 }),
                        );
                        ui.painter().text(
                            r.center(),
                            egui::Align2::CENTER_CENTER,
                            "?",
                            FontId::proportional(11.0),
                            if hot { t.ink } else { t.ink2 },
                        );
                        resp.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::accesskit::Role::Button,
                                true,
                                "Colour key",
                            )
                        });
                        resp.on_hover_ui(|ui| {
                            ui.label(egui::RichText::new("Colour = format").strong());
                            egui::Grid::new("br_colour_key")
                                .num_columns(4)
                                .spacing(egui::vec2(6.0, 4.0))
                                .show(ui, |ui| {
                                    for (k, f) in Fmt::ALL.iter().enumerate() {
                                        let (r, _) = ui.allocate_exact_size(
                                            egui::vec2(34.0, 14.0),
                                            Sense::hover(),
                                        );
                                        badge(ui.painter(), r, *f);
                                        ui.label(f.what());
                                        if k % 2 == 1 {
                                            ui.end_row();
                                        }
                                    }
                                });
                            ui.label(egui::RichText::new("* import planned").small());
                        });
                        ui.checkbox(
                            &mut self.extra.browser_autoplay,
                            egui::RichText::new("Auto").size(11.0),
                        )
                        .on_hover_text("auto-play: preview each item when it's selected");
                        // the label gets what is left, truncated with "…"
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&text).size(11.0).color(t.ink),
                                )
                                .truncate(),
                            )
                            .on_hover_text(&text);
                        });
                    });
                });
            },
        );
    }
}
