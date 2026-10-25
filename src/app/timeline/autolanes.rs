//! Track and clip automation lanes in the Timeline view (M5).
//!
//! Each track unfolds (the "A" button in its header) into one lane row per
//! automated parameter, under its clips. A row draws the track lane over
//! the whole song and, inside every clip that has its own lane for the same
//! parameter, the clip lane in a box: the innermost lane wins, so that is
//! what plays there. Click adds a point, drag moves it (Shift: off the
//! grid), Ctrl-drag draws, Alt-drag on a segment bends it, double-click or
//! right-click deletes; the right-click menu also adds or removes a clip's
//! own lane. Positions: track lanes in rows from the song start, clip lanes
//! in rows of the clip's source (so they move, trim and loop with it).
use super::*;
use crate::automation::{
    value_at, ClipMode, Lane, Point, Shape, Target, Units, WriteMode, POINT_R, POINT_R_HOT, SCALE_W,
};

/// height of one lane row and of the "+ lane" strip under the lanes
pub(crate) const AUTO_H: f32 = 56.0;
pub(crate) const ADD_H: f32 = 20.0;
/// point grab radius in pixels (design A)
const HIT: f32 = crate::automation::POINT_HIT;

/// the lane a gesture edits
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LaneRef {
    /// track, target id
    Track(usize, String),
    /// clip id, target id
    Clip(u32, String),
}

impl LaneRef {
    pub(crate) fn target(&self) -> &str {
        match self {
            LaneRef::Track(_, t) | LaneRef::Clip(_, t) => t,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AutoDragKind {
    /// move point `i`
    Point(usize),
    /// pencil
    Draw,
    /// bend segment `i` (curve at the start, value rising?)
    Bend(usize, f32, bool),
    /// line tool: start pos/value stored; end follows mouse
    Line(f32, f32),
    /// shape stamp: start pos, depth from vertical drag
    Stamp(f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AutoDrag {
    pub(crate) lane: LaneRef,
    pub(crate) kind: AutoDragKind,
    pub(crate) grab_y: f32,
    /// (track, lane row) the drag started in
    pub(crate) row: (usize, usize),
}

/// right-click spot in a lane row
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AutoMenu {
    pub(crate) lane: LaneRef,
    pub(crate) track: usize,
    pub(crate) point: Option<usize>,
    /// the clip under the mouse and whether it has a lane for this target
    pub(crate) clip: Option<(u32, bool)>,
    pub(crate) has_track_lane: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AutoOp {
    /// fold / unfold track `tr`
    Toggle(usize),
    /// toggle follow-last-touched
    FollowTouch(bool),
    /// show the last-touched knob's lane
    ShowLastTouch,
    AddLane(usize, Target),
    /// move song lane `k` onto track `tr` (pin)
    Pin(usize, usize),
    /// remove the track lane and the clips' lanes of this target on `tr`
    Remove(usize, String),
    Enable(usize, String, bool),
    Clear(LaneRef),
    AddClipLane(u32, String),
    RemoveClipLane(u32, String),
    /// add a point (and keep dragging it when the bool is set)
    Add(LaneRef, f32, f32, Option<(f32, (usize, usize))>),
    Move(LaneRef, usize, f32, f32),
    Delete(LaneRef, usize),
    Shape(LaneRef, usize, Shape),
    Curve(LaneRef, usize, f32),
    /// pencil: set the value at `pos` (points within `w` rows are replaced)
    Pencil(LaneRef, f32, f32, f32),
    /// line tool: two endpoints (pos0,v0,pos1,v1) replace the span
    Line(LaneRef, f32, f32, f32, f32),
    /// shape stamp over [pos0,pos1] with depth and rate (cycles)
    Stamp(LaneRef, f32, f32, f32, f32, StampShape),
    /// track write mode
    SetWriteMode(usize, WriteMode),
    /// clip lane combine mode
    SetClipMode(u32, String, ClipMode),
    Snapshot,
    StartDrag(AutoDrag),
    EndDrag,
    Menu(AutoMenu),
}

/// shape-stamp waveforms (design A §3.2)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum StampShape {
    Sine,
    Tri,
    Saw,
    Square,
    Random,
}

impl StampShape {
    #[allow(dead_code)]
    pub(crate) const ALL: [StampShape; 5] = [
        StampShape::Sine,
        StampShape::Tri,
        StampShape::Saw,
        StampShape::Square,
        StampShape::Random,
    ];
    pub(crate) fn name(self) -> &'static str {
        match self {
            StampShape::Sine => "Sine",
            StampShape::Tri => "Triangle",
            StampShape::Saw => "Saw",
            StampShape::Square => "Square",
            StampShape::Random => "Random",
        }
    }
    fn at(self, t: f32, seed: u32) -> f32 {
        let t = t.fract();
        match self {
            StampShape::Sine => (t * std::f32::consts::TAU).sin() * 0.5 + 0.5,
            StampShape::Tri => {
                if t < 0.5 {
                    t * 2.0
                } else {
                    2.0 - t * 2.0
                }
            }
            StampShape::Saw => t,
            StampShape::Square => {
                if t < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            StampShape::Random => {
                let mut x = seed.wrapping_add((t * 1000.0) as u32).wrapping_mul(0x9E37);
                x ^= x >> 16;
                x as f32 / u32::MAX as f32
            }
        }
    }
}

/// the gap above the Song / Master track: "+ Add track" sits in it, with a
/// divider, so Master reads as its own thing at the bottom (display only)
pub(crate) const MASTER_GAP: f32 = 34.0;

/// Vertical layout of the track area: each track's clip row, then (when
/// unfolded) its lane rows and the "+ lane" strip. The Song / Master track
/// (always the last one) comes after a gap of `MASTER_GAP`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Layout {
    /// content y of each track's top; one more entry: the total height
    pub(crate) top: Vec<f32>,
    /// the default clip row height (tracks without their own)
    pub(crate) clip_h: f32,
    /// each track's clip row height (its own, or `clip_h`)
    pub(crate) hs: Vec<f32>,
    /// lane targets shown under each track (empty and folded: none)
    pub(crate) lanes: Vec<Vec<String>>,
    pub(crate) open: Vec<bool>,
    /// the Song / Master track's index (the gap is above it)
    pub(crate) master: Option<usize>,
    /// content y where the mixer tracks end (before the gap)
    pub(crate) body_end: f32,
    /// Song / Master is docked: a fixed footer at the bottom of the view
    /// (its `top` follows the scroll; mixer tracks scroll under it)
    pub(crate) docked: bool,
}

/// the targets of track `tr`'s lanes: its own, then clip-only ones
pub(crate) fn track_targets(t: &tl::Timeline, tr: usize) -> Vec<String> {
    let mut v: Vec<String> = t
        .tracks
        .get(tr)
        .map(|x| x.auto.iter().map(|l| l.target.clone()).collect())
        .unwrap_or_default();
    for c in t.clips.iter().filter(|c| c.track as usize == tr) {
        for l in &c.auto {
            if !v.contains(&l.target) {
                v.push(l.target.clone());
            }
        }
    }
    v
}

impl Layout {
    /// `own[tr]`: track `tr`'s own clip row height (0 or missing: `clip_h`)
    pub(crate) fn build(t: &tl::Timeline, mode: bool, clip_h: f32, own: &[f32]) -> Self {
        let n = t.tracks.len();
        let hs: Vec<f32> = (0..n)
            .map(|tr| match own.get(tr).copied().unwrap_or(0.0) {
                h if h > 0.0 => h,
                _ => clip_h,
            })
            .collect();
        let mut top = Vec::with_capacity(n + 1);
        let mut lanes = Vec::with_capacity(n);
        let mut open = Vec::with_capacity(n);
        let mut y = 0.0;
        let master = (n > 0 && t.tracks[n - 1].kind == tl::TrackKind::Master).then_some(n - 1);
        for (tr, &h) in hs.iter().enumerate() {
            if Some(tr) == master {
                y += MASTER_GAP;
            }
            top.push(y);
            let o = mode && tr < 64 && t.view.auto_open >> tr & 1 == 1;
            let l = if o { track_targets(t, tr) } else { vec![] };
            y += h;
            if o {
                y += l.len() as f32 * AUTO_H + ADD_H;
            }
            lanes.push(l);
            open.push(o);
        }
        top.push(y);
        let body_end = master.map_or(y, |m| top[m] - MASTER_GAP);
        Layout {
            top,
            clip_h,
            hs,
            lanes,
            open,
            master,
            body_end,
            docked: false,
        }
    }
    /// the Song / Master row's height (clip row and any open lanes)
    pub(crate) fn master_h(&self) -> f32 {
        self.master.map_or(0.0, |m| self.top[m + 1] - self.top[m])
    }
    /// would Master dock in a view `view_h` tall (it needs room to spare)
    pub(crate) fn can_dock(&self, view_h: f32) -> bool {
        self.master.is_some() && self.master_h() <= view_h * 0.6
    }
    /// height the docked footer takes from the scrolling tracks (with the gap)
    pub(crate) fn dock_h(&self, view_h: f32) -> f32 {
        if self.can_dock(view_h) {
            self.master_h() + MASTER_GAP
        } else {
            0.0
        }
    }
    /// dock Song / Master at the bottom of a view `view_h` tall scrolled to
    /// `sy` (display only: the song keeps Master last)
    pub(crate) fn dock(&mut self, sy: f32, view_h: f32) {
        let Some(m) = self.master.filter(|_| self.can_dock(view_h)) else {
            return;
        };
        let h = self.master_h();
        let y = sy + view_h - h;
        self.top[m] = y;
        self.top[m + 1] = y + h;
        self.docked = true;
    }
    /// content y is in the gap above the Master track
    pub(crate) fn in_gap(&self, y: f32) -> bool {
        self.master.is_some_and(|m| {
            let g0 = if self.docked {
                self.body_end
            } else {
                self.top[m] - MASTER_GAP
            };
            y >= g0 && y < self.top[m]
        })
    }
    pub(crate) fn total(&self) -> f32 {
        self.top.last().copied().unwrap_or(0.0)
    }
    pub(crate) fn tracks(&self) -> usize {
        self.top.len().saturating_sub(1)
    }
    /// the track whose rows hold content y (clamped to the tracks)
    pub(crate) fn track_at(&self, y: f32) -> usize {
        let n = self.tracks();
        if n == 0 {
            return 0;
        }
        if let Some(m) = self.master.filter(|_| self.docked) {
            // the mixer tracks scroll under the docked footer
            if y >= self.top[m] {
                return m;
            }
            return self.top[..m].partition_point(|&t| t <= y).saturating_sub(1);
        }
        self.top[..n].partition_point(|&t| t <= y).saturating_sub(1)
    }
    /// what is at content y: (track, None = the clip row, Some(k) = lane
    /// row k, Some(lanes.len()) = the "+ lane" strip)
    pub(crate) fn part_at(&self, y: f32) -> Option<(usize, Option<usize>)> {
        if y < 0.0 || y >= self.total() || self.in_gap(y) {
            return None;
        }
        let tr = self.track_at(y);
        let dy = y - self.top[tr];
        let h = self.h(tr);
        if dy < h {
            return Some((tr, None));
        }
        let k = ((dy - h) / AUTO_H).floor().max(0.0) as usize;
        Some((tr, Some(k.min(self.lanes[tr].len()))))
    }
    /// content y of lane row `k` of track `tr` (k = len: the strip)
    pub(crate) fn lane_top(&self, tr: usize, k: usize) -> f32 {
        self.top[tr] + self.h(tr) + k as f32 * AUTO_H
    }
    /// track `tr`'s clip row height
    pub(crate) fn h(&self, tr: usize) -> f32 {
        self.hs.get(tr).copied().unwrap_or(self.clip_h)
    }
}

pub(crate) fn y_of(r: Rect, v: f32) -> f32 {
    r.bottom() - 3.0 - v.clamp(0.0, 1.0) * (r.height() - 6.0)
}

pub(crate) fn v_of(r: Rect, y: f32) -> f32 {
    ((r.bottom() - 3.0 - y) / (r.height() - 6.0).max(1.0)).clamp(0.0, 1.0)
}

/// source position of clip `c` at timeline tick `t` in rows (fractional)
pub(crate) fn clip_pos(c: &tl::Clip, t: f64) -> f32 {
    let mut s = c.offset as f64 + (t - c.start as f64);
    if let Some(l) = c.loop_len.filter(|l| *l > 0) {
        s = s.rem_euclid(l as f64);
    }
    (s / ROW as f64) as f32
}

fn track_lane<'a>(t: &'a tl::Timeline, tr: usize, tgt: &str) -> Option<&'a Lane> {
    t.tracks.get(tr)?.auto.iter().find(|l| l.target == tgt)
}

fn clip_lane<'a>(c: &'a tl::Clip, tgt: &str) -> Option<&'a Lane> {
    c.auto.iter().find(|l| l.target == tgt)
}

/// the clip on `tr` with its own lane for `tgt` that plays at tick `t`
/// (the later one where clips overlap)
fn clip_with_lane<'a>(c: &'a Cache, tr: usize, tgt: &str, t: Tick) -> Option<&'a tl::Clip> {
    c.idx
        .visible(&c.tl, tr, t, t + 1)
        .map(|i| &c.tl.clips[i])
        .filter(|cl| !cl.muted && clip_lane(cl, tgt).is_some())
        .max_by_key(|cl| (cl.start, cl.id))
}

/// screen positions of a lane's points: (point index, x) in the visible
/// span (clip lanes: every loop pass inside the clip)
fn point_xs(
    lane: &Lane,
    clip: Option<&tl::Clip>,
    x_of: &dyn Fn(f64) -> f32,
    v0: Tick,
    v1: Tick,
) -> Vec<(usize, f32, Tick)> {
    let mut v = vec![];
    match clip {
        None => {
            for (i, p) in lane.points.iter().enumerate() {
                let t = (p.pos as f64 * ROW as f64) as Tick;
                if t >= v0 - ROW && t <= v1 + ROW {
                    v.push((i, x_of(t as f64), t));
                }
            }
        }
        Some(c) => {
            let l = c.loop_len.filter(|l| *l > 0).unwrap_or(Tick::MAX / 4);
            let p0 = c.pass_at(c.start.max(v0));
            let p1 = c.pass_at((c.end() - 1).min(v1));
            for pass in p0..=p1.min(p0 + 4000) {
                let base = c.start - c.offset + pass as Tick * l;
                for (i, p) in lane.points.iter().enumerate() {
                    let s = (p.pos as f64 * ROW as f64) as Tick;
                    if c.loop_len.is_some() && s >= l {
                        continue;
                    }
                    let t = base + s;
                    if t >= c.start && t < c.end() && t >= v0 - ROW && t <= v1 + ROW {
                        v.push((i, x_of(t as f64), t));
                    }
                }
            }
        }
    }
    v
}

/// segment of `lane` that holds position `pos` (index of its first point)
fn segment_at(lane: &Lane, pos: f32) -> Option<usize> {
    let k = lane.points.partition_point(|p| p.pos <= pos);
    (k > 0 && k < lane.points.len()).then(|| k - 1)
}

/// everything the lane painter needs from the view
pub(crate) struct Geo<'a> {
    pub(crate) lanes: Rect,
    pub(crate) heads: Rect,
    pub(crate) sy: f32,
    pub(crate) x_of: &'a dyn Fn(f64) -> f32,
    pub(crate) tick_of: &'a dyn Fn(f32) -> f64,
    pub(crate) v0: Tick,
    pub(crate) v1: Tick,
    /// the play marker / playhead (lane headers show the value there)
    pub(crate) at: f64,
    pub(crate) mode: bool,
}

/// lane label: Device > Parameter from the target (design A); stored label
/// is kept in the file but not shown unless the target can't be parsed.
fn label_of(song: &crate::model::Song, c: &Cache, tr: usize, tgt: &str) -> String {
    if let Some(t) = Target::parse(tgt).or_else(|| Target::parse_in(tgt, song)) {
        return t.header_label(song);
    }
    track_lane(&c.tl, tr, tgt)
        .map(|l| l.label.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| tgt.to_string())
}

fn lane_colour(song: &crate::model::Song, tgt: &str, fallback: Color32) -> Color32 {
    Target::parse(tgt)
        .or_else(|| Target::parse_in(tgt, song))
        .map(|t| {
            let rgb = t.family_in(song).colour();
            Color32::from_rgb(
                ((rgb >> 16) & 0xff) as u8,
                ((rgb >> 8) & 0xff) as u8,
                (rgb & 0xff) as u8,
            )
        })
        .unwrap_or(fallback)
}

/// Paint and handle the lane rows and the "+ lane" strips of the visible
/// tracks. Edits go into `ops` (applied after the frame).
#[allow(clippy::too_many_arguments)]
pub(crate) fn lanes_ui(
    ui: &mut egui::Ui,
    song: &crate::model::Song,
    c: &Cache,
    lay: &Layout,
    g: &Geo,
    tracks: std::ops::Range<usize>,
    drag: Option<&AutoDrag>,
    menu_ctx: Option<&AutoMenu>,
    menu: Option<&super::super::lanes::LaneMenu>,
    song_lanes: &[(usize, String)],
    units: &[Units],
    follow_touch: bool,
    has_last_touch: bool,
    tool: crate::automation::AutoTool,
    ops: &mut Vec<AutoOp>,
) {
    let t = th();
    let lp = ui.painter_at(g.lanes);
    let hp = ui.painter_at(g.heads);
    let mods = ui.input(|i| i.modifiers);
    let hover = ui.input(|i| i.pointer.hover_pos());
    let x_of = g.x_of;
    let tick_of = g.tick_of;
    // automation snaps to rows (Shift: free)
    let snap_pos = |x: f32| -> f64 {
        let tk = tick_of(x).max(0.0);
        if mods.shift {
            tk
        } else {
            (tl::snap(tk as Tick, ROW)) as f64
        }
    };
    for tr in tracks {
        if !lay.open.get(tr).copied().unwrap_or(false) {
            continue;
        }
        let _col = track_colour(tr);
        let n = lay.lanes[tr].len();
        // the whole folded-out block: absorbs clicks meant for the clip rows
        let y_block = g.lanes.top() + lay.lane_top(tr, 0) - g.sy;
        let block = Rect::from_min_max(
            Pos2::new(g.lanes.left(), y_block),
            Pos2::new(g.lanes.right(), y_block + n as f32 * AUTO_H + ADD_H),
        );
        if block.bottom() < g.lanes.top() || block.top() > g.lanes.bottom() {
            continue;
        }
        let _ = ui.interact(
            block.intersect(g.lanes),
            ui.scope_id().with(("tl_auto_block", tr)),
            Sense::click(),
        );
        lp.rect_filled(block, 0, Color32::from_black_alpha(70));
        // the "+ lane" strip
        let sr = Rect::from_min_max(
            Pos2::new(g.heads.left() + 10.0, block.bottom() - ADD_H + 1.0),
            Pos2::new(g.heads.right() - 4.0, block.bottom() - 1.0),
        );
        if sr.intersects(g.heads) {
            let sr = sr.intersect(g.heads);
            ui.scope_builder(egui::UiBuilder::new().max_rect(sr), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 0.0);
                    ui.menu_button(egui::RichText::new("+ lane").small(), |ui| {
                        let mut add = None;
                        if let Some(m) = menu {
                            super::super::lanes::lane_menu_ui(ui, m, true, &mut add);
                        } else {
                            ui.label("…");
                        }
                        if let Some(tg) = add {
                            ops.push(AutoOp::AddLane(tr, tg));
                        }
                    })
                    .response
                    .on_hover_text("automate a parameter on this track (mixer, instrument, FX, plugin, modulation depth)");
                    let ft = ui
                        .selectable_label(
                            follow_touch,
                            egui::RichText::new("last").small(),
                        )
                        .on_hover_text(
                            "follow last-touched knob: moving a knob opens or focuses its lane on this track",
                        );
                    if ft.clicked() {
                        ops.push(AutoOp::FollowTouch(!follow_touch));
                    }
                    if has_last_touch
                        && ui
                            .small_button("show")
                            .on_hover_text("show the lane for the last knob you moved")
                            .clicked()
                    {
                        ops.push(AutoOp::ShowLastTouch);
                    }
                    // write mode chip (design A §3.3)
                    let wm = c.tl.tracks.get(tr).map(|x| x.write_mode).unwrap_or_default();
                    ui.menu_button(egui::RichText::new(wm.name()).small(), |ui| {
                        ui.label(egui::RichText::new("Automation write").small().color(t.ink2));
                        for m in WriteMode::ALL {
                            if ui.selectable_label(wm == m, m.name()).clicked() {
                                ops.push(AutoOp::SetWriteMode(tr, m));
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text("Read / Touch / Latch / Write / Off — recording mode for this track");
                    if !song_lanes.is_empty() {
                        ui.menu_button(egui::RichText::new("pin song lane").small(), |ui| {
                            ui.label(
                                egui::RichText::new("move a song lane onto this track")
                                    .small()
                                    .color(t.ink2),
                            );
                            for (k, name) in song_lanes {
                                if ui.button(name).clicked() {
                                    ops.push(AutoOp::Pin(tr, *k));
                                    ui.close();
                                }
                            }
                        });
                    }
                });
            });
        }
        for (k, tgt) in lay.lanes[tr].iter().enumerate() {
            let y = g.lanes.top() + lay.lane_top(tr, k) - g.sy;
            let row = Rect::from_min_max(
                Pos2::new(g.lanes.left(), y),
                Pos2::new(g.lanes.right(), y + AUTO_H),
            );
            if row.bottom() < g.lanes.top() || row.top() > g.lanes.bottom() {
                continue;
            }
            let col = lane_colour(song, tgt, track_colour(tr));
            let inner = Rect::from_min_max(
                Pos2::new(row.left() + SCALE_W, row.top() + 2.0),
                Pos2::new(row.right(), row.bottom() - 2.0),
            );
            // value scale on the left (design A)
            {
                let sr = Rect::from_min_max(
                    Pos2::new(row.left(), row.top()),
                    Pos2::new(row.left() + SCALE_W, row.bottom()),
                );
                lp.rect_filled(sr, 0, Color32::from_black_alpha(90));
                if let Some(tg) = Target::parse(tgt).or_else(|| Target::parse_in(tgt, song)) {
                    for &(nv, lab) in tg.scale_ticks() {
                        let yy = y_of(inner, nv);
                        lp.hline(
                            egui::Rangef::new(sr.right() - 6.0, row.right()),
                            yy,
                            Stroke::new(1.0, t.crt_grid.gamma_multiply(0.7)),
                        );
                        lp.text(
                            Pos2::new(sr.right() - 4.0, yy),
                            egui::Align2::RIGHT_CENTER,
                            lab,
                            crate::theme::mono(crate::theme::ts::TINY),
                            t.ink2,
                        );
                    }
                }
            }
            lp.hline(
                row.x_range(),
                row.bottom() - 0.5,
                Stroke::new(1.0, t.crt_grid),
            );
            let tlane = track_lane(&c.tl, tr, tgt);
            let enabled = tlane.is_none_or(|l| l.enabled);
            let lc = if enabled {
                col.gamma_multiply(1.5).to_opaque()
            } else {
                t.ink2.gamma_multiply(0.6)
            };
            // the track lane over the visible span
            if let Some(l) = tlane.filter(|l| !l.points.is_empty()) {
                let mut pts = vec![];
                let mut fill = egui::Mesh::default();
                let mut x = g.lanes.left();
                while x <= g.lanes.right() + 3.0 {
                    let pos = (tick_of(x) / ROW as f64) as f32;
                    if let Some(v) = value_at(&l.points, pos) {
                        let yy = y_of(inner, v);
                        pts.push(Pos2::new(x, yy));
                        quad(
                            &mut fill,
                            Rect::from_min_max(
                                Pos2::new(x, yy),
                                Pos2::new(x + 3.0, inner.bottom()),
                            ),
                            lc.gamma_multiply(0.10),
                        );
                    }
                    x += 3.0;
                }
                lp.add(egui::Shape::mesh(fill));
                lp.add(egui::Shape::line(pts, Stroke::new(1.5, lc)));
            } else {
                lp.text(
                    Pos2::new(g.lanes.left() + 8.0, row.center().y),
                    egui::Align2::LEFT_CENTER,
                    "no track lane: click to start one (clip lanes play inside their clips)",
                    crate::theme::prop(crate::theme::ts::TINY),
                    t.ink2.gamma_multiply(0.7),
                );
            }
            // clip lanes: a box per clip that has one (it wins there)
            for i in c.idx.visible(&c.tl, tr, g.v0, g.v1) {
                let cl = &c.tl.clips[i];
                let Some(l) = clip_lane(cl, tgt) else {
                    continue;
                };
                let (x0, x1) = (x_of(cl.start as f64), x_of(cl.end() as f64));
                let br = Rect::from_min_max(
                    Pos2::new(x0.max(g.lanes.left() - 2.0), inner.top()),
                    Pos2::new(x1.min(g.lanes.right() + 2.0), inner.bottom()),
                );
                let k_m = if cl.muted { 0.35 } else { 1.0 };
                lp.rect_filled(br, 2, t.crt_bot.gamma_multiply(0.92));
                lp.rect_filled(br, 2, col.gamma_multiply(0.22 * k_m));
                lp.rect_stroke(
                    br,
                    2,
                    Stroke::new(1.0, col.gamma_multiply(0.9 * k_m)),
                    egui::StrokeKind::Inside,
                );
                let relative = l.mode != ClipMode::Absolute;
                let cc = if relative {
                    Color32::from_rgb(0x6A, 0xB0, 0xF2).gamma_multiply(k_m)
                } else {
                    Color32::from_rgb(0xF2, 0xE6, 0xB8).gamma_multiply(k_m)
                };
                let mut pts = vec![];
                let mut x = br.left();
                while x <= br.right() {
                    let tk = tick_of(x);
                    if tk >= cl.start as f64 && tk < cl.end() as f64 {
                        if let Some(v) = value_at(&l.points, clip_pos(cl, tk)) {
                            pts.push(Pos2::new(x, y_of(inner, v)));
                        }
                    }
                    x += 2.0;
                }
                if relative {
                    // dashed: draw every other segment
                    for w in pts.windows(2).step_by(2) {
                        lp.line_segment([w[0], w[1]], Stroke::new(1.5, cc));
                    }
                } else {
                    lp.add(egui::Shape::line(pts, Stroke::new(1.5, cc)));
                }
                for (_, px, pt) in point_xs(l, Some(cl), x_of, g.v0, g.v1) {
                    let v = value_at(&l.points, clip_pos(cl, pt as f64)).unwrap_or(0.0);
                    lp.circle_filled(Pos2::new(px, y_of(inner, v)), POINT_R, cc);
                }
            }
            // which lane and point the mouse is on
            let lane_at = |x: f32| -> (LaneRef, Option<&tl::Clip>) {
                let tk = tick_of(x).max(0.0) as Tick;
                match clip_with_lane(c, tr, tgt, tk) {
                    Some(cl) => (LaneRef::Clip(cl.id, tgt.clone()), Some(cl)),
                    None => (LaneRef::Track(tr, tgt.clone()), None),
                }
            };
            let lane_of = |r: &LaneRef| -> Option<(&Lane, Option<&tl::Clip>)> {
                match r {
                    LaneRef::Track(tr, tg) => track_lane(&c.tl, *tr, tg).map(|l| (l, None)),
                    LaneRef::Clip(id, tg) => {
                        let cl = c.tl.clips.iter().find(|x| x.id == *id)?;
                        clip_lane(cl, tg).map(|l| (l, Some(cl)))
                    }
                }
            };
            let point_at = |p: Pos2, r: &LaneRef| -> Option<usize> {
                let (l, cl) = lane_of(r)?;
                point_xs(l, cl, x_of, g.v0, g.v1)
                    .into_iter()
                    .filter(|(i, px, _)| {
                        (px - p.x).abs() <= HIT
                            && (y_of(inner, l.points[*i].value) - p.y).abs() <= HIT + 2.0
                    })
                    .min_by(|a, b| (a.1 - p.x).abs().total_cmp(&(b.1 - p.x).abs()))
                    .map(|x| x.0)
            };
            // points of the track lane (drawn last: on top)
            if let Some(l) = tlane {
                let hot = hover.filter(|p| row.contains(*p)).and_then(|p| {
                    let (r, _) = lane_at(p.x);
                    matches!(r, LaneRef::Track(..))
                        .then(|| point_at(p, &r))
                        .flatten()
                });
                for (i, px, pt) in point_xs(l, None, x_of, g.v0, g.v1) {
                    if clip_with_lane(c, tr, tgt, pt).is_some() {
                        continue;
                    }
                    let pp = Pos2::new(px, y_of(inner, l.points[i].value));
                    let dragging = drag.is_some_and(|d| {
                        d.lane == LaneRef::Track(tr, tgt.clone())
                            && d.kind == AutoDragKind::Point(i)
                    });
                    if hot == Some(i) || dragging {
                        lp.circle_filled(pp, POINT_R_HOT, t.selection_outline);
                        lp.circle_filled(pp, POINT_R, lc);
                    } else {
                        lp.circle_filled(pp, POINT_R, lc);
                    }
                }
            }

            // command diamonds (38xx–3Bxx) on cutoff / resonance lanes (design A §3.7)
            if let Some(tg) = Target::parse(tgt).or_else(|| Target::parse_in(tgt, song)) {
                let cmd = match tg {
                    Target::Cutoff(_) => Some((0x38u8, 0x3Au8, 0x3Bu8)),
                    Target::Resonance(_) => Some((0x39u8, 0, 0)),
                    _ => None,
                };
                if let Some((set, up, down)) = cmd {
                    let nch = song.channels.min(crate::model::MAX_CHANNELS);
                    if tr < nch {
                        for (bi, pat) in song.patterns.iter().enumerate() {
                            for (row, cells) in pat.cells.iter().enumerate() {
                                let Some(cell) = cells.get(tr) else { continue };
                                let fx = cell.fx;
                                if fx != set && fx != up && fx != down {
                                    continue;
                                }
                                // diamond at the first clip that plays this block on this track
                                let tick = row as i64 * ROW;
                                for cidx in c.idx.visible(&c.tl, tr, tick, tick + 1) {
                                    let cl = &c.tl.clips[cidx];
                                    let crate::timeline::ClipKind::Block { pattern, .. } = cl.kind
                                    else {
                                        continue;
                                    };
                                    if pattern != bi {
                                        continue;
                                    }
                                    let x = x_of(cl.start as f64 + tick as f64);
                                    if x < g.lanes.left() - 4.0 || x > g.lanes.right() + 4.0 {
                                        continue;
                                    }
                                    let nv = cell.fxp as f32 / 255.0;
                                    let y = y_of(inner, nv);
                                    let d = 4.0;
                                    let diamond = [
                                        Pos2::new(x, y - d),
                                        Pos2::new(x + d, y),
                                        Pos2::new(x, y + d),
                                        Pos2::new(x - d, y),
                                    ];
                                    lp.add(egui::Shape::convex_polygon(
                                        diamond.to_vec(),
                                        Color32::from_rgb(0xE0, 0xC0, 0x40).gamma_multiply(0.85),
                                        Stroke::new(1.0, Color32::from_rgb(0xFF, 0xE0, 0x80)),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            // modulation band: value ± depth where a route targets this param (design A §3.7)
            if let Some(tg) = Target::parse(tgt) {
                let id = tg.id();
                let depths: Vec<f32> = song
                    .modulation
                    .routes
                    .iter()
                    .filter(|r| r.target == id)
                    .map(|r| r.amount.abs())
                    .collect();
                if let Some(depth) = depths
                    .into_iter()
                    .fold(None, |a: Option<f32>, d| Some(a.map_or(d, |x| x.max(d))))
                {
                    if depth > 0.01 {
                        if let Some(l) = tlane.filter(|l| !l.points.is_empty()) {
                            let mut band = egui::Mesh::default();
                            let mut x = g.lanes.left();
                            while x <= g.lanes.right() + 3.0 {
                                let pos = (tick_of(x) / ROW as f64) as f32;
                                if let Some(v) = value_at(&l.points, pos) {
                                    let lo = y_of(inner, (v - depth * 0.5).clamp(0.0, 1.0));
                                    let hi = y_of(inner, (v + depth * 0.5).clamp(0.0, 1.0));
                                    quad(
                                        &mut band,
                                        Rect::from_min_max(
                                            Pos2::new(x, hi.min(lo)),
                                            Pos2::new(x + 3.0, lo.max(hi)),
                                        ),
                                        Color32::from_white_alpha(18),
                                    );
                                }
                                x += 3.0;
                            }
                            lp.add(egui::Shape::mesh(band));
                        }
                    }
                }
            }

            // ---- lane header
            let hr = Rect::from_min_max(
                Pos2::new(g.heads.left() + 10.0, y + 1.0),
                Pos2::new(g.heads.right() - 2.0, y + AUTO_H - 1.0),
            );
            if hr.intersects(g.heads) {
                hp.rect_filled(hr, 2, t.cart_top.gamma_multiply(0.55));
                hp.rect_filled(
                    Rect::from_min_max(hr.min, Pos2::new(hr.left() + 3.0, hr.bottom())),
                    1,
                    col.gamma_multiply(0.7),
                );
                let np = hp.with_clip_rect(hr.shrink(1.0).intersect(g.heads));
                np.text(
                    Pos2::new(hr.left() + 8.0, hr.top() + 11.0),
                    egui::Align2::LEFT_CENTER,
                    label_of(song, c, tr, tgt),
                    crate::theme::prop(crate::theme::ts::LABEL),
                    if enabled { t.ink } else { t.ink2 },
                );
                // the value that plays at the playhead / play marker
                let at = g.at;
                let (r, cl) = lane_at(x_of(at));
                let v = match (&r, cl) {
                    (LaneRef::Clip(..), Some(cl)) => {
                        clip_lane(cl, tgt).and_then(|l| value_at(&l.points, clip_pos(cl, at)))
                    }
                    _ => tlane.and_then(|l| value_at(&l.points, (at / ROW as f64) as f32)),
                };
                let target = Target::parse(tgt);
                let vt = match (v, target) {
                    (Some(v), Some(tg)) => tg.format_in(v, units),
                    (None, _) => "—".into(),
                    (Some(v), None) => format!("{:.0} %", v * 100.0),
                };
                np.text(
                    Pos2::new(hr.left() + 8.0, hr.bottom() - 10.0),
                    egui::Align2::LEFT_CENTER,
                    format!(
                        "{vt}{}",
                        if matches!(r, LaneRef::Clip(..)) {
                            " · clip lane"
                        } else if !enabled {
                            " · off"
                        } else {
                            ""
                        }
                    ),
                    crate::theme::mono(crate::theme::ts::TINY),
                    t.ink2,
                );
                // on/off and remove
                let bs = 15.0;
                let xr = Rect::from_min_size(
                    Pos2::new(hr.right() - bs - 3.0, hr.top() + 3.0),
                    egui::vec2(bs, bs),
                );
                let or = Rect::from_min_size(
                    Pos2::new(xr.left() - bs - 3.0, xr.top()),
                    egui::vec2(bs, bs),
                );
                for (r, label, tip) in [
                    (or, "", "lane on / off"),
                    (
                        xr,
                        "×",
                        "remove this lane (and the clips' lanes for it on this track)",
                    ),
                ] {
                    let resp = ui
                        .interact(
                            r,
                            ui.scope_id().with(("tl_auto_btn", tr, k, label)),
                            Sense::click(),
                        )
                        .on_hover_text(tip);
                    hp.rect_filled(
                        r,
                        3,
                        if resp.hovered() {
                            t.pill.gamma_multiply(1.4)
                        } else {
                            t.pill
                        },
                    );
                    if label.is_empty() {
                        // on: a filled dot, off: a ring
                        if enabled {
                            hp.circle_filled(r.center(), 4.0, t.sage);
                        } else {
                            hp.circle_stroke(r.center(), 4.0, Stroke::new(1.2, t.ink2));
                        }
                    } else {
                        hp.text(
                            r.center(),
                            egui::Align2::CENTER_CENTER,
                            label,
                            crate::theme::prop(crate::theme::ts::SMALL),
                            t.ink2,
                        );
                    }
                    if resp.clicked() {
                        ops.push(if label == "×" {
                            AutoOp::Remove(tr, tgt.clone())
                        } else {
                            AutoOp::Enable(tr, tgt.clone(), !enabled)
                        });
                    }
                }
                let hresp = ui
                    .interact(
                        Rect::from_min_max(hr.min, Pos2::new(or.left() - 2.0, hr.bottom())),
                        ui.scope_id().with(("tl_auto_head", tr, k)),
                        Sense::click(),
                    )
                    .on_hover_text(format!(
                        "{} · {}\nright-click: clear, remove",
                        label_of(song, c, tr, tgt),
                        tgt
                    ));
                let has = tlane.is_some();
                hresp.context_menu(|ui| {
                    if has && ui.button("Clear the track lane").clicked() {
                        ops.push(AutoOp::Clear(LaneRef::Track(tr, tgt.clone())));
                        ui.close();
                    }
                    if ui.button("Remove lane").clicked() {
                        ops.push(AutoOp::Remove(tr, tgt.clone()));
                        ui.close();
                    }
                });
            }
            // ---- editing
            if !g.mode {
                continue;
            }
            let resp = ui.interact(
                row,
                ui.scope_id().with(("tl_auto", tr, k)),
                Sense::click_and_drag(),
            );
            if let Some(p) = hover.filter(|p| row.contains(*p) && drag.is_none()) {
                let (r, _) = lane_at(p.x);
                if point_at(p, &r).is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                } else if mods.command {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                }
            }
            // position of x in lane `r`'s own units (rows; clip: its source)
            let pos_in = |r: &LaneRef, tk: f64| -> f32 {
                match lane_of(r).and_then(|x| x.1) {
                    Some(cl) => clip_pos(cl, tk.clamp(cl.start as f64, cl.end() as f64 - 1.0)),
                    None => match r {
                        LaneRef::Clip(id, _) => {
                            c.tl.clips
                                .iter()
                                .find(|x| x.id == *id)
                                .map_or(0.0, |cl| clip_pos(cl, tk))
                        }
                        _ => (tk / ROW as f64) as f32,
                    },
                }
            };
            if resp.drag_started() {
                if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                    let (r, _) = lane_at(p.x);
                    let v = v_of(inner, p.y);
                    let pos = pos_in(&r, snap_pos(p.x));
                    if let Some(i) = point_at(p, &r) {
                        ops.push(AutoOp::Snapshot);
                        ops.push(AutoOp::StartDrag(AutoDrag {
                            lane: r,
                            kind: AutoDragKind::Point(i),
                            grab_y: p.y,
                            row: (tr, k),
                        }));
                    } else if mods.alt {
                        let raw = pos_in(&r, tick_of(p.x));
                        if let Some((l, _)) = lane_of(&r) {
                            if let Some(i) = segment_at(l, raw) {
                                let rising = l.points[i + 1].value >= l.points[i].value;
                                ops.push(AutoOp::Snapshot);
                                ops.push(AutoOp::StartDrag(AutoDrag {
                                    lane: r,
                                    kind: AutoDragKind::Bend(i, l.points[i].curve, rising),
                                    grab_y: p.y,
                                    row: (tr, k),
                                }));
                            }
                        }
                    } else if mods.command || tool == crate::automation::AutoTool::Pencil {
                        ops.push(AutoOp::Snapshot);
                        ops.push(AutoOp::Pencil(r.clone(), pos, v, 0.5));
                        ops.push(AutoOp::StartDrag(AutoDrag {
                            lane: r,
                            kind: AutoDragKind::Draw,
                            grab_y: p.y,
                            row: (tr, k),
                        }));
                    } else if tool == crate::automation::AutoTool::Line {
                        ops.push(AutoOp::Snapshot);
                        ops.push(AutoOp::StartDrag(AutoDrag {
                            lane: r,
                            kind: AutoDragKind::Line(pos, v),
                            grab_y: p.y,
                            row: (tr, k),
                        }));
                    } else if tool == crate::automation::AutoTool::Shape {
                        ops.push(AutoOp::Snapshot);
                        ops.push(AutoOp::StartDrag(AutoDrag {
                            lane: r,
                            kind: AutoDragKind::Stamp(pos, v),
                            grab_y: p.y,
                            row: (tr, k),
                        }));
                    } else {
                        // Pointer: new point that follows the mouse (drag from empty)
                        ops.push(AutoOp::Add(r, pos, v, Some((p.y, (tr, k)))));
                    }
                }
            } else if resp.dragged() {
                if let (Some(p), Some(d)) = (resp.interact_pointer_pos(), drag) {
                    if d.row == (tr, k) {
                        let v = v_of(inner, p.y);
                        match d.kind {
                            AutoDragKind::Point(i) => {
                                let pos = pos_in(&d.lane, snap_pos(p.x));
                                ops.push(AutoOp::Move(d.lane.clone(), i, pos, v));
                            }
                            AutoDragKind::Draw => {
                                let pos = pos_in(&d.lane, snap_pos(p.x));
                                ops.push(AutoOp::Pencil(d.lane.clone(), pos, v, 0.5));
                            }
                            AutoDragKind::Bend(i, c0, rising) => {
                                let dy = (d.grab_y - p.y) / 60.0;
                                let cv = (c0 + if rising { dy } else { -dy }).clamp(-1.0, 1.0);
                                ops.push(AutoOp::Curve(d.lane.clone(), i, cv));
                            }
                            AutoDragKind::Line(..) | AutoDragKind::Stamp(..) => {
                                // committed on drag_stopped
                            }
                        }
                    }
                }
            }
            if resp.drag_stopped() {
                if let (Some(p), Some(d)) = (resp.interact_pointer_pos(), drag) {
                    if d.row == (tr, k) {
                        let v1 = v_of(inner, p.y);
                        let pos1 = pos_in(&d.lane, snap_pos(p.x));
                        match d.kind {
                            AutoDragKind::Line(p0, v0) => {
                                ops.push(AutoOp::Line(d.lane.clone(), p0, v0, pos1, v1));
                            }
                            AutoDragKind::Stamp(p0, _) => {
                                let depth =
                                    ((d.grab_y - p.y).abs() / inner.height()).clamp(0.1, 1.0);
                                let cycles = ((pos1 - p0).abs() / 4.0).max(1.0);
                                ops.push(AutoOp::Stamp(
                                    d.lane.clone(),
                                    p0,
                                    pos1,
                                    depth,
                                    cycles,
                                    StampShape::Sine,
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                ops.push(AutoOp::EndDrag);
            }
            // design A: double-click empty = add; double-click a point = delete;
            // a plain click on empty space does nothing (drag-from-empty still adds)
            if resp.double_clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let (r, _) = lane_at(p.x);
                    if let Some(i) = point_at(p, &r) {
                        ops.push(AutoOp::Delete(r, i));
                    } else {
                        let pos = pos_in(&r, snap_pos(p.x));
                        ops.push(AutoOp::Add(r, pos, v_of(inner, p.y), None));
                    }
                }
            }
            if resp.secondary_clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let (r, _) = lane_at(p.x);
                    let tk = tick_of(p.x).max(0.0) as Tick;
                    let under = c
                        .idx
                        .visible(&c.tl, tr, tk, tk + 1)
                        .map(|i| &c.tl.clips[i])
                        .max_by_key(|cl| (cl.start, cl.id))
                        .map(|cl| (cl.id, clip_lane(cl, tgt).is_some()));
                    ops.push(AutoOp::Menu(AutoMenu {
                        point: point_at(p, &r),
                        lane: r,
                        track: tr,
                        clip: under,
                        has_track_lane: tlane.is_some(),
                    }));
                }
            }
            // tooltip: the value under a point being dragged / hovered
            if let (Some(p), Some(tg)) = (hover.filter(|p| row.contains(*p)), Target::parse(tgt)) {
                let (r, _) = lane_at(p.x);
                let dragged = drag
                    .filter(|d| d.row == (tr, k))
                    .and_then(|d| match d.kind {
                        AutoDragKind::Point(i) => Some((d.lane.clone(), i)),
                        _ => None,
                    });
                if let Some((r, i)) = dragged.or_else(|| point_at(p, &r).map(|i| (r, i))) {
                    if let Some((l, cl)) = lane_of(&r) {
                        if let Some(pt) = l.points.get(i) {
                            let at = match cl {
                                Some(_) => format!("clip row {:.2}", pt.pos),
                                None => {
                                    let bp = c.tl.bar_pos((pt.pos as f64 * ROW as f64) as Tick);
                                    format!("bar {}.{}", bp.bar, bp.beat)
                                }
                            };
                            resp.clone().on_hover_text_at_pointer(format!(
                                "{} · {at}{}",
                                tg.format_in(pt.value, units),
                                if pt.curve != 0.0 {
                                    format!(" · curve {:+.0} %", pt.curve * 100.0)
                                } else {
                                    String::new()
                                }
                            ));
                        }
                    }
                } else if drag.is_none() {
                    resp.clone().on_hover_text(
                        "double-click: add a point · drag a point: move (Shift: off the grid) · Ctrl-drag: draw · Alt-drag a segment: bend · double-click a point: delete · right-click: more",
                    );
                }
            }
            let ctx_here = menu_ctx.filter(|m| m.track == tr && m.lane.target() == tgt);
            resp.context_menu(|ui| {
                let Some(m) = ctx_here else {
                    ui.label("…");
                    return;
                };
                if let Some(i) = m.point {
                    ui.label(egui::RichText::new("Point").strong());
                    for sh in Shape::ALL {
                        if ui.button(format!("Shape: {}", sh.name())).clicked() {
                            ops.push(AutoOp::Shape(m.lane.clone(), i, sh));
                            ui.close();
                        }
                    }
                    if ui.button("Straighten (no curve)").clicked() {
                        ops.push(AutoOp::Curve(m.lane.clone(), i, 0.0));
                        ui.close();
                    }
                    if ui.button("Delete point").clicked() {
                        ops.push(AutoOp::Delete(m.lane.clone(), i));
                        ui.close();
                    }
                    ui.separator();
                }
                match m.clip {
                    Some((id, false)) => {
                        if ui
                            .button("Add clip automation to this clip")
                            .on_hover_text("a lane that moves, loops and is copied with the clip; inside the clip it wins over the track lane")
                            .clicked()
                        {
                            ops.push(AutoOp::AddClipLane(id, tgt.clone()));
                            ui.close();
                        }
                    }
                    Some((id, true)) => {
                        if ui.button("Clear this clip's lane").clicked() {
                            ops.push(AutoOp::Clear(LaneRef::Clip(id, tgt.clone())));
                            ui.close();
                        }
                        if ui.button("Remove this clip's lane").clicked() {
                            ops.push(AutoOp::RemoveClipLane(id, tgt.clone()));
                            ui.close();
                        }
                    }
                    None => {}
                }
                if let Some((id, true)) = m.clip {
                    ui.separator();
                    ui.label(egui::RichText::new("Clip envelope").strong());
                    for mode in [ClipMode::Absolute, ClipMode::RelAdd, ClipMode::RelMul] {
                        if ui.button(format!("Mode: {}", mode.name())).clicked() {
                            ops.push(AutoOp::SetClipMode(id, tgt.clone(), mode));
                            ui.close();
                        }
                    }
                }
                if m.has_track_lane && ui.button("Clear the track lane").clicked() {
                    ops.push(AutoOp::Clear(LaneRef::Track(tr, tgt.clone())));
                    ui.close();
                }
                if ui.button("Remove lane").clicked() {
                    ops.push(AutoOp::Remove(tr, tgt.clone()));
                    ui.close();
                }
            });
        }
    }
}

/// the lane `r` points at, created when it is a missing track lane
fn lane_mut<'a>(
    t: &'a mut tl::Timeline,
    r: &LaneRef,
    label: &str,
    start: f32,
    create: bool,
) -> Option<&'a mut Lane> {
    match r {
        LaneRef::Track(tr, tg) => {
            let tk = t.tracks.get_mut(*tr)?;
            let k = match tk.auto.iter().position(|l| l.target == *tg) {
                Some(k) => k,
                None if create => {
                    let mut l = Lane::new(Target::Master, label.to_string());
                    l.target = tg.clone();
                    l.points.push(Point {
                        pos: 0.0,
                        value: start,
                        shape: Shape::Linear,
                        curve: 0.0,
                    });
                    tk.auto.push(l);
                    tk.auto.len() - 1
                }
                None => return None,
            };
            tk.auto.get_mut(k)
        }
        LaneRef::Clip(id, tg) => {
            let c = t.clips.iter_mut().find(|c| c.id == *id)?;
            c.auto.iter_mut().find(|l| l.target == *tg)
        }
    }
}

/// highest position a point of `r` may take (clip lanes: their loop)
fn max_pos(t: &tl::Timeline, r: &LaneRef) -> f32 {
    match r {
        LaneRef::Clip(id, _) => t.clips.iter().find(|c| c.id == *id).map_or(f32::MAX, |c| {
            (c.loop_len.filter(|l| *l > 0).unwrap_or(c.offset + c.len) as f64 / ROW as f64) as f32
                - 1e-3
        }),
        _ => f32::MAX,
    }
}

impl App {
    /// data for the lane rows: the "+ lane" menu (refreshed every half
    /// second while lanes show), the song lanes and the value units
    pub(crate) fn lanes_view_data(&mut self) -> (Vec<(usize, String)>, Vec<Units>) {
        let now = std::time::Instant::now();
        if self
            .tlv
            .lane_menu
            .as_ref()
            .is_none_or(|(t, _)| now.duration_since(*t).as_millis() > 500)
        {
            self.tlv.lane_menu = Some((now, self.lane_menu()));
        }
        self.with(|s| {
            let song_lanes = s
                .song
                .automation
                .iter()
                .enumerate()
                .filter(|(_, l)| Target::parse_in(&l.target, &s.song).is_some_and(tl::track_target))
                .map(|(k, l)| {
                    (
                        k,
                        if l.label.is_empty() {
                            l.target.clone()
                        } else {
                            l.label.clone()
                        },
                    )
                })
                .collect();
            let units = s
                .song
                .instruments
                .iter()
                .map(|i| Units {
                    fx: i.fx.iter().map(|d| d.kind).collect(),
                    gen: Units::gen_of(&i.kind),
                })
                .collect();
            (song_lanes, units)
        })
    }

    pub(crate) fn auto_op(&mut self, op: AutoOp) {
        let mode = self.with(|s| s.song.timeline.is_some());
        match op {
            AutoOp::Toggle(tr) if !mode => {
                let _ = tr;
                self.tlv.ask_switch = Some(None);
                return;
            }
            AutoOp::StartDrag(d) => {
                self.tlv.auto_drag = Some(d);
                return;
            }
            AutoOp::EndDrag => {
                self.tlv.auto_drag = None;
                // points keep their order
                self.with(|s| {
                    if let Some(t) = s.song.timeline.as_deref_mut() {
                        for l in t.tracks.iter_mut().flat_map(|x| x.auto.iter_mut()) {
                            l.sort();
                        }
                        for l in t.clips.iter_mut().flat_map(|x| x.auto.iter_mut()) {
                            l.sort();
                        }
                    }
                });
                return;
            }
            AutoOp::Menu(m) => {
                self.tlv.auto_menu = Some(m);
                return;
            }
            AutoOp::FollowTouch(on) => {
                self.tlv.follow_touch = on;
                self.status = if on {
                    "Follow last-touched: on — move a knob to open its lane".into()
                } else {
                    "Follow last-touched: off".into()
                };
                return;
            }
            AutoOp::ShowLastTouch => {
                if let Some(t) = self.tlv.last_touch {
                    self.add_lane(t);
                } else {
                    self.status = "Move a knob first, then click show".into();
                }
                return;
            }
            AutoOp::Snapshot => {
                self.snapshot();
                return;
            }
            _ if !mode => return,
            _ => {}
        }
        let discrete = !matches!(
            op,
            AutoOp::Move(..) | AutoOp::Pencil(..) | AutoOp::Curve(..) | AutoOp::Line(..)
        );
        if discrete && !matches!(op, AutoOp::Toggle(_)) {
            self.snapshot();
        }
        // a new lane's label and first value come from the song
        let tgt_of = |op: &AutoOp| -> Option<Target> {
            match op {
                AutoOp::AddLane(_, t) => Some(*t),
                AutoOp::Add(r, ..) | AutoOp::Pencil(r, ..) => Target::parse(r.target()),
                AutoOp::AddClipLane(_, t) => Target::parse(t),
                _ => None,
            }
        };
        let (label, start) = match tgt_of(&op) {
            Some(t) => (
                self.lane_label(t),
                self.with(|s| super::super::lanes::lane_start_value(s, t)),
            ),
            None => (String::new(), 0.5),
        };
        let mut status = String::new();
        let mut drag = None;
        self.with(|s| {
            let Some(t) = s.song.timeline.as_deref_mut() else {
                return;
            };
            match &op {
                AutoOp::Toggle(tr) => {
                    if *tr < 64 {
                        t.view.auto_open ^= 1 << tr;
                    }
                }
                AutoOp::AddLane(tr, tg) => {
                    let id = tg.id();
                    let nch = s.song.channels.min(crate::model::MAX_CHANNELS);
                    let tr = if matches!(
                        *tg,
                        Target::Master | Target::Tempo | Target::Ret(..) | Target::ModAmount(_)
                    ) {
                        crate::timeline::ensure_master_track(t, nch)
                    } else {
                        *tr
                    };
                    if let Some(x) = t.tracks.get_mut(tr) {
                        if !x.auto.iter().any(|l| l.target == id) {
                            let mut l = Lane::new(*tg, label.clone());
                            l.points.push(Point {
                                pos: 0.0,
                                value: start,
                                shape: Shape::Linear,
                                curve: 0.0,
                            });
                            x.auto.push(l);
                            status =
                                format!("Added track lane '{label}' (double-click to add points)");
                        }
                        if tr < 64 {
                            t.view.auto_open |= 1 << tr;
                        }
                    }
                }
                AutoOp::Pin(tr, k) => {
                    if *k < s.song.automation.len() {
                        let l = s.song.automation.remove(*k);
                        if let Some(x) = t.tracks.get_mut(*tr) {
                            status = format!("Pinned '{}' to track {:02}", l.label, tr + 1);
                            x.auto.retain(|o| o.target != l.target);
                            x.auto.push(l);
                        }
                    }
                }
                AutoOp::Remove(tr, tg) => {
                    if let Some(x) = t.tracks.get_mut(*tr) {
                        x.auto.retain(|l| l.target != *tg);
                    }
                    for c in t.clips.iter_mut().filter(|c| c.track as usize == *tr) {
                        c.auto.retain(|l| l.target != *tg);
                    }
                    status = "Lane removed (Ctrl+Z brings it back)".into();
                }
                AutoOp::Enable(tr, tg, on) => {
                    if let Some(l) = t
                        .tracks
                        .get_mut(*tr)
                        .and_then(|x| x.auto.iter_mut().find(|l| l.target == *tg))
                    {
                        l.enabled = *on;
                    }
                }
                AutoOp::Clear(r) => {
                    if let Some(l) = lane_mut(t, r, &label, start, false) {
                        l.points.truncate(1);
                    }
                }
                AutoOp::AddClipLane(id, tg) => {
                    let tr = t
                        .clips
                        .iter()
                        .find(|c| c.id == *id)
                        .map(|c| c.track as usize);
                    // start flat at what the track lane plays at the clip start
                    let from = t.clips.iter().find(|c| c.id == *id).and_then(|c| {
                        let tl_ = t.tracks.get(c.track as usize)?;
                        let l = tl_.auto.iter().find(|l| l.target == *tg)?;
                        value_at(&l.points, (c.start / ROW) as f32)
                    });
                    if let Some(c) = t.clips.iter_mut().find(|c| c.id == *id) {
                        if !c.auto.iter().any(|l| l.target == *tg) {
                            let mut l = Lane::new(Target::Master, label.clone());
                            l.target = tg.clone();
                            l.points.push(Point {
                                pos: (c.offset as f64 / ROW as f64) as f32,
                                value: from.unwrap_or(start),
                                shape: Shape::Linear,
                                curve: 0.0,
                            });
                            c.auto.push(l);
                            status = "Clip lane added: it moves and loops with the clip".into();
                        }
                    }
                    if let Some(tr) = tr.filter(|t| *t < 32) {
                        t.view.auto_open |= 1 << tr;
                    }
                }
                AutoOp::RemoveClipLane(id, tg) => {
                    if let Some(c) = t.clips.iter_mut().find(|c| c.id == *id) {
                        c.auto.retain(|l| l.target != *tg);
                    }
                }
                AutoOp::Add(r, pos, v, follow) => {
                    let hi = max_pos(t, r);
                    if let Some(l) = lane_mut(t, r, &label, *v, true) {
                        let pos = pos.clamp(0.0, hi);
                        let i = match l.points.iter().position(|p| (p.pos - pos).abs() < 1e-4) {
                            Some(i) => {
                                l.points[i].value = *v;
                                i
                            }
                            None => {
                                let k = l.points.partition_point(|p| p.pos <= pos);
                                let shape = k
                                    .checked_sub(1)
                                    .map_or(Shape::Linear, |j| l.points[j].shape);
                                l.points.insert(
                                    k,
                                    Point {
                                        pos,
                                        value: *v,
                                        shape,
                                        curve: 0.0,
                                    },
                                );
                                k
                            }
                        };
                        if let Some((y, row)) = follow {
                            drag = Some(AutoDrag {
                                lane: r.clone(),
                                kind: AutoDragKind::Point(i),
                                grab_y: *y,
                                row: *row,
                            });
                        }
                        status =
                            "Added automation point (drag to move, double-click to delete)".into();
                    }
                }
                AutoOp::Move(r, i, pos, v) => {
                    let hi = max_pos(t, r);
                    if let Some(l) = lane_mut(t, r, "", 0.0, false) {
                        let n = l.points.len();
                        if *i < n {
                            let lo = if *i > 0 { l.points[i - 1].pos } else { 0.0 };
                            let up = l.points.get(i + 1).map_or(hi, |p| p.pos);
                            l.points[*i].pos = pos.clamp(lo, up.max(lo));
                            l.points[*i].value = *v;
                        }
                    }
                }
                AutoOp::Delete(r, i) => {
                    if let Some(l) = lane_mut(t, r, "", 0.0, false) {
                        if *i < l.points.len() && l.points.len() > 1 {
                            l.points.remove(*i);
                            status = "Deleted automation point".into();
                        } else {
                            status = "A lane keeps its last point (Remove lane deletes it)".into();
                        }
                    }
                }
                AutoOp::Shape(r, i, sh) => {
                    if let Some(p) =
                        lane_mut(t, r, "", 0.0, false).and_then(|l| l.points.get_mut(*i))
                    {
                        p.shape = *sh;
                    }
                }
                AutoOp::Curve(r, i, cv) => {
                    if let Some(p) =
                        lane_mut(t, r, "", 0.0, false).and_then(|l| l.points.get_mut(*i))
                    {
                        p.curve = *cv;
                        if p.shape != Shape::Linear && *cv != 0.0 {
                            p.shape = Shape::Linear;
                        }
                    }
                }
                AutoOp::Pencil(r, pos, v, w) => {
                    let hi = max_pos(t, r);
                    if let Some(l) = lane_mut(t, r, &label, *v, true) {
                        let pos = pos.clamp(0.0, hi);
                        l.points.retain(|p| (p.pos - pos).abs() >= *w);
                        let k = l.points.partition_point(|p| p.pos <= pos);
                        l.points.insert(
                            k,
                            Point {
                                pos,
                                value: *v,
                                shape: Shape::Linear,
                                curve: 0.0,
                            },
                        );
                    }
                }
                AutoOp::Line(r, p0, v0, p1, v1) => {
                    let hi = max_pos(t, r);
                    let (a, b, va, vb) = if *p0 <= *p1 {
                        (*p0, *p1, *v0, *v1)
                    } else {
                        (*p1, *p0, *v1, *v0)
                    };
                    if let Some(l) = lane_mut(t, r, &label, va, true) {
                        let a = a.clamp(0.0, hi);
                        let b = b.clamp(0.0, hi);
                        l.points.retain(|p| p.pos < a - 1e-4 || p.pos > b + 1e-4);
                        let k = l.points.partition_point(|p| p.pos <= a);
                        l.points.insert(
                            k,
                            Point {
                                pos: a,
                                value: va,
                                shape: Shape::Linear,
                                curve: 0.0,
                            },
                        );
                        l.points.insert(
                            k + 1,
                            Point {
                                pos: b.max(a + 1e-3),
                                value: vb,
                                shape: Shape::Linear,
                                curve: 0.0,
                            },
                        );
                        status = "Line drawn".into();
                    }
                }
                AutoOp::Stamp(r, p0, p1, depth, cycles, sh) => {
                    let hi = max_pos(t, r);
                    let (a, b) = if *p0 <= *p1 { (*p0, *p1) } else { (*p1, *p0) };
                    if let Some(l) = lane_mut(t, r, &label, 0.5, true) {
                        let a = a.clamp(0.0, hi);
                        let b = b.clamp(a + 0.25, hi);
                        let n = ((*cycles).max(1.0) * 8.0).ceil() as usize;
                        let n = n.clamp(4, 64);
                        let mid = value_at(&l.points, (a + b) * 0.5).unwrap_or(0.5);
                        l.points.retain(|p| p.pos < a - 1e-4 || p.pos > b + 1e-4);
                        let seed = (a.to_bits()).wrapping_add(b.to_bits());
                        for i in 0..=n {
                            let tfrac = i as f32 / n as f32;
                            let pos = a + (b - a) * tfrac;
                            let w = sh.at(tfrac * cycles.max(1.0), seed);
                            let v = (mid + (w - 0.5) * depth.clamp(0.0, 1.0)).clamp(0.0, 1.0);
                            let k = l.points.partition_point(|p| p.pos <= pos);
                            l.points.insert(
                                k,
                                Point {
                                    pos,
                                    value: v,
                                    shape: Shape::Smooth,
                                    curve: 0.0,
                                },
                            );
                        }
                        status = format!("{} stamp", sh.name());
                    }
                }
                AutoOp::SetWriteMode(tr, m) => {
                    if let Some(x) = t.tracks.get_mut(*tr) {
                        x.write_mode = *m;
                        status = format!("Write mode: {}", m.name());
                    }
                }
                AutoOp::SetClipMode(id, tg, m) => {
                    if let Some(l) = t
                        .clips
                        .iter_mut()
                        .find(|c| c.id == *id)
                        .and_then(|c| c.auto.iter_mut().find(|l| l.target == *tg))
                    {
                        l.mode = *m;
                        status = format!("Clip envelope: {}", m.name());
                    }
                }
                _ => {}
            }
        });
        if let Some(d) = drag {
            self.tlv.auto_drag = Some(d);
        }
        if !status.is_empty() {
            self.status = status;
        }
        self.dirty = true;
    }
}
