//! One drop system for clips dragged onto the Timeline (docs/unified/
//! PLAN.md Stage 4a): where the clip would land (track and snapped start,
//! from the toolbar's snap setting; Shift: rows), what it would cover, a
//! preview of all that before the release (a ghost clip on the target row,
//! a snap line through the ruler, a card that says what will happen), and
//! no silent overwrite: a drop over clips asks Replace / Place after it /
//! Keep both / Cancel. A target that can't take the clip (the Song /
//! Master row, an audio track for notes) offers a new track instead.
//! The Clip Arranger's slot drag and the Browser's drum patterns use it;
//! the model side is
//! `timeline::{overlaps, clear_range, free_start, place_copy}`.
use super::timeline::TlOp;
use super::*;
use crate::timeline::{self as tl, Tick, PPQ};

/// what is being dragged
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DropSrc {
    /// a Clip Arranger slot of a Tracker-clip song: column `col` of `block`
    Block { block: usize, col: usize },
    /// a clip (a Clip Arranger slot of a note / audio clip song): a copy of it
    Clip(u32),
    /// a Browser drum pattern (`drumpat::PATTERNS` index): a new note clip
    Pattern(u16),
}

/// what a drop does to the clips in its way
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fit {
    /// nothing is in the way
    Free,
    /// clear the span first (clips across its edges keep their outside parts)
    Replace,
    /// the first free start after the clips in the way
    After,
    /// keep both: note and audio clips play together
    Layer,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DropPlan {
    /// the track it lands on (with `new_track`: the index the new one gets)
    pub track: usize,
    pub new_track: bool,
    pub start: Tick,
    pub len: Tick,
    /// clips on the target track it would cover
    pub overlaps: Vec<u32>,
    /// Keep both is possible (no Tracker clip involved: those hide each other)
    pub layer_ok: bool,
    /// why it can't land here (None: it can)
    pub bad: Option<String>,
    /// the snap grid used (ticks) and its name
    pub grid: Tick,
    pub snap: &'static str,
}

/// a drop over clips, waiting for the answer
#[derive(Clone, Debug)]
pub(crate) struct DropAsk {
    pub src: DropSrc,
    pub plan: DropPlan,
    pub unique: bool,
}

/// what the dragged thing is, as the plan and the card need it
pub(crate) struct SrcInfo {
    pub len: Tick,
    pub name: String,
    /// a Tracker clip (block column)
    pub block: bool,
    pub audio: bool,
    /// a Browser drum pattern (its notes are new, never linked)
    pub pattern: bool,
    /// the track it comes from (its instrument plays it on a new track)
    pub from_track: usize,
}

fn snap_name(grid: Tick, bar: Tick) -> &'static str {
    match grid {
        g if g == tl::ROW => "1/16",
        g if g == PPQ => "Beat",
        g if g == bar => "Bar",
        g if g == bar * 4 => "4 bars",
        _ => "grid",
    }
}

/// "2 bars", "1 bar 2 beats", "3 beats"
pub(crate) fn len_text(len: Tick, bar: Tick) -> String {
    let bars = len / bar.max(1);
    let beats = (len % bar.max(1)) / PPQ;
    let pl = |n: Tick, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    match (bars, beats) {
        (0, 0) => "less than a beat".into(),
        (0, b) => pl(b, "beat"),
        (b, 0) => pl(b, "bar"),
        (a, b) => format!("{} {}", pl(a, "bar"), pl(b, "beat")),
    }
}

impl App {
    pub(crate) fn drop_src_info(&self, src: DropSrc) -> Option<SrcInfo> {
        self.with(|s| {
            let song = &s.song;
            match src {
                DropSrc::Block { block, col } => {
                    let p = song.patterns.get(block)?;
                    Some(SrcInfo {
                        len: tl::rows_to_ticks(p.rows.max(1)),
                        name: format!("{block:02} {} · column {}", p.name, col + 1),
                        block: true,
                        audio: false,
                        pattern: false,
                        from_track: col,
                    })
                }
                DropSrc::Clip(id) => {
                    let t = song.timeline.as_deref()?;
                    let c = t.any_clip(id)?;
                    let name = c
                        .name
                        .clone()
                        .or_else(|| {
                            c.notes()
                                .and_then(|d| t.notes.get(d))
                                .map(|d| d.name.clone())
                        })
                        .or_else(|| {
                            c.audio()
                                .and_then(|a| t.audio.get(a))
                                .map(|a| a.name.clone())
                        })
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| "Clip".into());
                    Some(SrcInfo {
                        len: c.len,
                        name,
                        block: c.block().is_some(),
                        audio: c.audio().is_some(),
                        pattern: false,
                        from_track: c.track as usize,
                    })
                }
                DropSrc::Pattern(i) => {
                    let p = crate::drumpat::PATTERNS.get(i as usize)?;
                    Some(SrcInfo {
                        len: p.len(),
                        name: format!("{} ({})", p.name, p.kit),
                        block: false,
                        audio: false,
                        pattern: true,
                        from_track: 0,
                    })
                }
            }
        })
    }

    /// where `src` dropped at `p` would land (None: not over the Timeline)
    pub(crate) fn drop_plan(&self, p: Pos2, src: DropSrc, fine: bool) -> Option<DropPlan> {
        let info = self.drop_src_info(src)?;
        let (target, _) = self.tlv.drop_at(p)?;
        let over_lanes = self.tlv.lanes.x_range().contains(p.x);
        let (nch, bar) = self.with(|s| {
            (
                s.song.channels,
                s.song
                    .timeline
                    .as_deref()
                    .map_or(4 * PPQ, |t| t.sig_at(0).bar_ticks().max(1)),
            )
        });
        let grid = if fine { tl::ROW } else { self.tlv.snap_grid() };
        let raw = if over_lanes {
            self.tlv.tick_at(p.x)
        } else {
            // over the headers: the visible start
            self.tlv.tick_at(self.tlv.lanes.left())
        };
        let start = (raw / grid * grid).max(0);
        let mut plan = DropPlan {
            track: 0,
            new_track: false,
            start,
            len: info.len,
            overlaps: vec![],
            layer_ok: !info.block,
            bad: None,
            grid,
            snap: if fine {
                "off (Shift)"
            } else {
                snap_name(grid, bar)
            },
        };
        let new_track = |plan: &mut DropPlan, why: Option<&str>| {
            plan.new_track = true;
            plan.track = nch;
            if nch >= MAX_CHANNELS {
                plan.bad = Some(format!(
                    "{}all 32 tracks are used",
                    why.map(|w| format!("{w}; ")).unwrap_or_default()
                ));
            }
        };
        match target {
            Ok(tr) if tr >= nch => {
                // the Song / Master row plays no clips
                new_track(&mut plan, Some("the Song / Master row plays no clips"));
            }
            Ok(tr) => {
                let kind = self.with(|s| {
                    s.song
                        .timeline
                        .as_deref()
                        .and_then(|t| t.tracks.get(tr))
                        .map(|t| t.kind)
                        .unwrap_or_default()
                });
                if kind == tl::TrackKind::Audio && !info.audio {
                    new_track(&mut plan, Some("an audio track plays audio clips only"));
                } else {
                    plan.track = tr;
                }
            }
            Err(_) => new_track(&mut plan, None),
        }
        if !plan.new_track {
            let skip: Vec<u32> = vec![];
            let (ov, layer_ok) = self.with(|s| {
                let Some(t) = s.song.timeline.as_deref() else {
                    return (vec![], false);
                };
                let ov = tl::overlaps(t, plan.track, plan.start, plan.start + plan.len, &skip);
                let any_block = t
                    .clips
                    .iter()
                    .any(|c| ov.contains(&c.id) && c.block().is_some());
                (ov, !any_block)
            });
            plan.overlaps = ov;
            plan.layer_ok &= layer_ok;
        }
        Some(plan)
    }

    /// the drop preview: target row, ghost clip, snap line and the card at
    /// the pointer. `unique`: Alt is held (an independent copy)
    pub(crate) fn drop_paint(
        &self,
        ctx: &egui::Context,
        src: DropSrc,
        plan: Option<&DropPlan>,
        pos: Option<Pos2>,
        unique: bool,
    ) {
        let t = th();
        let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::NULL.with("drop_plan"));
        let p = ctx.layer_painter(layer);
        let info = self.drop_src_info(src);
        let bar = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .map_or(4 * PPQ, |x| x.sig_at(0).bar_ticks().max(1))
        });
        let bar_of = |at: Tick| {
            self.with(|s| {
                s.song.timeline.as_deref().map_or(String::new(), |x| {
                    let b = x.bar_pos(at);
                    if b.beat <= 1 && b.tick == 0 {
                        format!("bar {}", b.bar)
                    } else {
                        format!("bar {}.{}", b.bar, b.beat)
                    }
                })
            })
        };
        let warn = t.brick;
        let lanes = self.tlv.lanes;
        if let Some(pl) = plan.filter(|pl| pl.bad.is_none()) {
            let hot = if pl.overlaps.is_empty() {
                t.accent
            } else {
                warn
            };
            let x0 = self.tlv.x_of(pl.start);
            let x1 = self.tlv.x_of(pl.start + pl.len);
            let band = if pl.new_track {
                let y = self.tlv.tracks_bottom();
                p.line_segment(
                    [
                        Pos2::new(self.tlv.heads.left(), y),
                        Pos2::new(lanes.right(), y),
                    ],
                    Stroke::new(2.5, t.accent),
                );
                Some(Rect::from_min_max(
                    Pos2::new(x0, y + 4.0),
                    Pos2::new(x1, y + 4.0 + self.tlv.track_h_pts() - 6.0),
                ))
            } else {
                self.tlv.track_rects(pl.track).map(|(_, row)| {
                    p.rect_filled(row, 0.0, hot.gamma_multiply(0.10));
                    Rect::from_min_max(
                        Pos2::new(x0, row.top() + 2.0),
                        Pos2::new(x1, row.bottom() - 2.0),
                    )
                })
            };
            if let Some(g) = band {
                let g = g.intersect(lanes.expand2(egui::vec2(0.0, self.tlv.track_h_pts())));
                if g.is_positive() {
                    p.rect_filled(g, 3.0, hot.gamma_multiply(0.32));
                    p.rect_stroke(g, 3.0, Stroke::new(2.0, hot), egui::StrokeKind::Inside);
                    if let Some(i) = &info {
                        p.with_clip_rect(g.shrink(3.0)).text(
                            g.left_top() + egui::vec2(5.0, 3.0),
                            egui::Align2::LEFT_TOP,
                            &i.name,
                            crate::theme::prop(crate::theme::ts::LABEL),
                            t.ink,
                        );
                    }
                }
            }
            // the clips in the way
            for id in &pl.overlaps {
                if let Some(r) = self.tlv.clip_rect(*id) {
                    p.rect_stroke(
                        r.intersect(lanes),
                        3.0,
                        Stroke::new(2.0, warn),
                        egui::StrokeKind::Outside,
                    );
                }
            }
            // snap line through ruler and lanes, with the bar on the ruler
            if lanes.x_range().contains(x0) {
                let top = self.tlv.ruler.top();
                p.add(egui::Shape::dashed_line(
                    &[Pos2::new(x0, top), Pos2::new(x0, lanes.bottom())],
                    Stroke::new(1.5, hot),
                    5.0,
                    3.0,
                ));
                let lab = format!("{} · snap {}", bar_of(pl.start), pl.snap);
                let gal = p.layout_no_wrap(
                    lab,
                    crate::theme::prop(crate::theme::ts::SMALL),
                    Color32::from_rgb(0x16, 0x18, 0x1B),
                );
                let r = Rect::from_min_size(Pos2::new(x0, top), gal.size() + egui::vec2(10.0, 4.0));
                p.rect_filled(r, 3.0, hot);
                p.galley(r.min + egui::vec2(5.0, 2.0), gal, t.ink);
            }
        }
        // the card at the pointer
        let Some(q) = pos else { return };
        let mut lines: Vec<(String, Color32, bool)> = vec![];
        if let Some(i) = &info {
            lines.push((
                format!("{}  ·  {}", i.name, len_text(i.len, bar)),
                t.ink,
                true,
            ));
        }
        match plan {
            None => lines.push(("drop it on a track in the Timeline".into(), t.ink2, false)),
            Some(pl) if pl.bad.is_some() => {
                lines.push((
                    format!("Can't drop here: {}", pl.bad.clone().unwrap_or_default()),
                    warn,
                    true,
                ));
            }
            Some(pl) => {
                let tname = self.with(|s| {
                    s.song
                        .channel_names
                        .get(pl.track)
                        .cloned()
                        .unwrap_or_default()
                });
                lines.push((
                    if pl.new_track {
                        format!(
                            "→ a new track ({:02}) at {} (snap {})",
                            pl.track + 1,
                            bar_of(pl.start),
                            pl.snap
                        )
                    } else {
                        format!(
                            "→ Track {:02} {tname} at {} (snap {})",
                            pl.track + 1,
                            bar_of(pl.start),
                            pl.snap
                        )
                    },
                    t.accent,
                    true,
                ));
                let from = info.as_ref().map_or(0, |i| i.from_track);
                let slot = if pl.new_track {
                    self.track_slot(from)
                } else {
                    self.track_slot(pl.track)
                };
                let kit = |slot: u8| {
                    self.with(|s| {
                        s.song
                            .instruments
                            .get((slot as usize).wrapping_sub(1))
                            .is_some_and(super::drumpat::is_kit)
                    })
                };
                let pattern = match src {
                    DropSrc::Pattern(i) => crate::drumpat::PATTERNS.get(i as usize),
                    _ => None,
                };
                if let Some(p) = pattern {
                    lines.push((
                        if !pl.new_track && kit(slot) {
                            format!("plays {slot:02X}, the track's kit")
                        } else if pl.new_track {
                            format!("plays a new {} Drum Kit on the new track", p.kit)
                        } else {
                            format!("plays a {} Drum Kit (the track's instrument stays)", p.kit)
                        },
                        t.ink2,
                        false,
                    ));
                } else if let Some(n) = self.with(|s| {
                    s.song
                        .instruments
                        .get((slot as usize).wrapping_sub(1))
                        .map(|i| i.name.clone())
                }) {
                    lines.push((
                        format!(
                            "plays {slot:02X} {n}{}",
                            if pl.new_track {
                                " (on the new track)"
                            } else {
                                ", the track's instrument"
                            }
                        ),
                        t.ink2,
                        false,
                    ));
                }
                let what = if info.as_ref().is_some_and(|i| i.block) {
                    "block"
                } else {
                    "notes"
                };
                lines.push((
                    if info.as_ref().is_some_and(|i| i.pattern) {
                        "A new note clip with the pattern's notes".to_string()
                    } else if unique {
                        format!("Independent copy: its own {what} (release Alt for a linked copy)")
                    } else {
                        format!(
                            "Linked: editing its {what} changes both · hold Alt: independent copy"
                        )
                    },
                    t.ink2,
                    false,
                ));
                if !pl.overlaps.is_empty() {
                    lines.push((self.overlap_text(pl), warn, true));
                }
            }
        }
        lines.push(("Shift: no snap · Esc cancels".into(), t.ink2, false));
        let gals: Vec<_> = lines
            .into_iter()
            .map(|(s, c, b)| {
                let f = crate::theme::prop(if b { 12.0 } else { 11.0 });
                (p.layout_no_wrap(s, f, c), c)
            })
            .collect();
        let w = gals.iter().map(|(g, _)| g.size().x).fold(0.0, f32::max) + 16.0;
        let h = gals.iter().map(|(g, _)| g.size().y + 2.0).sum::<f32>() + 8.0;
        let screen = ctx.content_rect();
        let mut r = Rect::from_min_size(q + egui::vec2(16.0, 12.0), egui::vec2(w, h));
        if r.right() > screen.right() - 4.0 {
            r = r.translate(egui::vec2(screen.right() - 4.0 - r.right(), 0.0));
        }
        if r.bottom() > screen.bottom() - 4.0 {
            r = r.translate(egui::vec2(0.0, -(h + 30.0)));
        }
        let edge = if plan.is_some_and(|pl| pl.bad.is_some() || !pl.overlaps.is_empty()) {
            warn
        } else {
            t.accent
        };
        p.rect_filled(r, 4.0, t.cart_top);
        p.rect_stroke(r, 4.0, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
        let mut y = r.top() + 4.0;
        for (g, c) in gals {
            let gh = g.size().y;
            p.galley(Pos2::new(r.left() + 8.0, y), g, c);
            y += gh + 2.0;
        }
    }

    /// "Overlaps 05 Breakdown (bars 41-49) and 1 more: release to choose"
    pub(crate) fn overlap_text(&self, pl: &DropPlan) -> String {
        self.with(|s| {
            let Some(t) = s.song.timeline.as_deref() else {
                return String::new();
            };
            let first = pl
                .overlaps
                .first()
                .and_then(|id| t.clips.iter().find(|c| c.id == *id));
            let Some(c) = first else {
                return String::new();
            };
            let name = c
                .name
                .clone()
                .or_else(|| {
                    c.block()
                        .and_then(|(b, _)| s.song.patterns.get(b))
                        .map(|p| format!("{:02} {}", c.block().unwrap().0, p.name))
                })
                .or_else(|| {
                    c.notes()
                        .and_then(|d| t.notes.get(d))
                        .map(|d| d.name.clone())
                })
                .or_else(|| {
                    c.audio()
                        .and_then(|a| t.audio.get(a))
                        .map(|a| a.name.clone())
                })
                .unwrap_or_else(|| "a clip".into());
            let (a, b) = (
                t.bar_pos(c.start).bar,
                t.bar_pos((c.end() - 1).max(c.start)).bar,
            );
            let more = pl.overlaps.len() - 1;
            format!(
                "Overlaps {name} (bar{} {a}{}){}: release to choose",
                if a == b { "" } else { "s" },
                if a == b {
                    String::new()
                } else {
                    format!("-{b}")
                },
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            )
        })
    }

    /// the mouse was let go over `plan`
    pub(crate) fn drop_release(&mut self, src: DropSrc, plan: Option<DropPlan>, unique: bool) {
        let Some(plan) = plan else {
            self.status = "Dropped outside the Timeline: nothing placed".into();
            return;
        };
        if let Some(why) = &plan.bad {
            self.status = format!("Not placed: {why}");
            return;
        }
        if plan.overlaps.is_empty() || plan.new_track {
            self.drop_apply(src, &plan, unique, Fit::Free);
        } else {
            self.drop_ask = Some(DropAsk { src, plan, unique });
        }
    }

    /// place it (one undo step, with a new track when the plan says so)
    pub(crate) fn drop_apply(&mut self, src: DropSrc, plan: &DropPlan, unique: bool, fit: Fit) {
        let Some(info) = self.drop_src_info(src) else {
            return;
        };
        if let DropSrc::Pattern(i) = src {
            self.drop_pattern(i as usize, plan, fit);
            return;
        }
        self.snapshot();
        self.undo_hold += 1;
        let mut track = plan.track;
        if plan.new_track {
            let n = self.with(|s| s.song.channels);
            let slot = self.track_slot(info.from_track);
            let name = self.with(|s| {
                s.song
                    .channel_names
                    .get(info.from_track)
                    .cloned()
                    .unwrap_or_default()
            });
            let name = if name.is_empty() {
                format!("Track {}", n + 1)
            } else {
                name
            };
            if !self.track_insert(n, &name) {
                self.undo_hold -= 1;
                return;
            }
            self.with(|s| {
                if let Some(t) = s.song.timeline.as_deref_mut() {
                    let colour = t.tracks.get(info.from_track).and_then(|x| x.colour);
                    if let Some(x) = t.tracks.get_mut(n) {
                        x.instr = slot;
                        x.colour = colour;
                    }
                }
            });
            track = n;
        }
        self.tl_apply(TlOp::Place {
            src,
            track: track as u8,
            at: plan.start,
            unique,
            fit,
            grid: plan.grid,
        });
        self.undo_hold -= 1;
        self.ch = track;
        self.dirty = true;
    }

    /// a Browser drum pattern where the plan says (its own undo step; a new
    /// track gets the pattern's Drum Kit)
    fn drop_pattern(&mut self, i: usize, plan: &DropPlan, fit: Fit) {
        let track = if plan.new_track {
            u8::MAX
        } else {
            plan.track as u8
        };
        let at = match (fit, self.with(|s| s.song.timeline.as_deref().cloned())) {
            (Fit::After, Some(t)) => {
                tl::free_start(&t, plan.track, plan.start, plan.len, plan.grid)
            }
            _ => plan.start,
        };
        self.snapshot();
        self.undo_hold += 1;
        self.pat_place_at(i, track, at, false);
        if fit == Fit::Replace {
            if let Some(id) = self.tlv.sel {
                let n = self.with(|s| {
                    let n = s.song.timeline.as_deref_mut().map_or(0, |t| {
                        tl::clear_range(t, plan.track, at, at + plan.len, &[id])
                    });
                    let crate::audio::Shared { song, engine, .. } = s;
                    engine.ensure_plan(song);
                    n
                });
                self.status.push_str(&format!(
                    ", replacing the part of {n} clip{} under it",
                    if n == 1 { "" } else { "s" }
                ));
            }
        } else if fit == Fit::Layer {
            self.status
                .push_str(", on top of the clips there (they play together)");
        }
        self.undo_hold -= 1;
    }

    /// the overlap question (Replace / Place after it / Keep both / Cancel)
    pub(crate) fn drop_ask_window(&mut self, ctx: &egui::Context) {
        let Some(ask) = self.drop_ask.clone() else {
            return;
        };
        let t = th();
        let msg = self
            .overlap_text(&ask.plan)
            .replace(": release to choose", "");
        let after = self.with(|s| {
            s.song.timeline.as_deref().map(|x| {
                let at = tl::free_start(
                    x,
                    ask.plan.track,
                    ask.plan.start,
                    ask.plan.len,
                    ask.plan.grid,
                );
                x.bar_pos(at).bar
            })
        });
        let mut pick: Option<Option<Fit>> = None;
        egui::Window::new("Place the clip over others?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -60.0])
            .show(ctx, |ui| {
                ui.label(format!("{msg} on track {:02}.", ask.plan.track + 1));
                ui.label(
                    egui::RichText::new("Nothing changes until you choose; Ctrl+Z undoes the result.")
                        .small()
                        .color(t.ink2),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .button("Replace")
                        .on_hover_text("the clips in the way lose the part under the new clip (the parts outside it stay)")
                        .clicked()
                    {
                        pick = Some(Some(Fit::Replace));
                    }
                    let lab = after.map_or("Place after it".to_string(), |b| format!("Place after it (bar {b})"));
                    if ui
                        .button(lab)
                        .on_hover_text("the first free place on this track after the clips in the way")
                        .clicked()
                    {
                        pick = Some(Some(Fit::After));
                    }
                    if ask.plan.layer_ok
                        && ui
                            .button("Keep both")
                            .on_hover_text("note and audio clips on one track play together")
                            .clicked()
                    {
                        pick = Some(Some(Fit::Layer));
                    }
                    if ui.button("Cancel").clicked() {
                        pick = Some(None);
                    }
                });
            });
        match pick {
            Some(Some(fit)) => {
                self.drop_ask = None;
                self.drop_apply(ask.src, &ask.plan, ask.unique, fit);
            }
            Some(None) => {
                self.drop_ask = None;
                self.status = "Not placed (cancelled)".into();
            }
            None => {}
        }
    }
}
