//! The piano roll's chord lane (note clips): chord markers above the grid,
//! the chord pies (empty lane: styles and their progressions; a chord:
//! play / edit / transpose / type / suggestions / write notes ...), the
//! list-menu alternative (Settings) and the Generate chord progression
//! dialog. The progressions and voicings are `crate::chordgen`.
use super::pianoview::{Source, Target};
use super::radial::{RIcon, Radial, RadialEv, RadialItem};
use super::*;
use crate::chordgen::{self as cg, ChordMark, GenOpts, Mode, Quality, Voicing};
use crate::chords::{Key, Scale};
use crate::timeline::{self as tl, Tick, PPQ};
use std::time::Instant;

pub(crate) const BAR: Tick = 4 * PPQ;
pub(crate) const LANE_H: f32 = 22.0;
const EDGE: f32 = 5.0;

/// what a pie was opened on
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PieOn {
    /// empty lane at this tick
    Lane(Tick),
    /// chord marker `idx`
    Mark(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum LDragK {
    Move,
    Resize,
}

#[derive(Clone)]
struct LDrag {
    kind: LDragK,
    idx: usize,
    grab: f64,
    origin: Vec<ChordMark>,
    moved: bool,
}

pub(crate) struct GenDlg {
    pub(crate) open: bool,
    pub(crate) opts: GenOpts,
    pub(crate) write: bool,
    pub(crate) voicing: Voicing,
    pub(crate) octave: u8,
    /// written notes go on top of the clip's notes; off (the default):
    /// they replace the notes that start inside each chord's span
    pub(crate) on_top: bool,
}

/// a chord-note write waiting for an OK: it would delete more than
/// [`CONFIRM_OVER`] notes, or notes outside the new chords' pitch range
pub(crate) struct PendingWrite {
    t: Target,
    marks: Vec<ChordMark>,
    notes: Vec<tl::Note>,
    spans: Vec<(Tick, Tick)>,
    /// notes that would be deleted / how many of those are outside the
    /// chord's pitch range
    pub(crate) removed: usize,
    pub(crate) outside: usize,
    /// "clip … on track NN, bars a–b"
    pub(crate) scope: String,
    what: String,
}

/// more deleted notes than this asks first
pub(crate) const CONFIRM_OVER: usize = 8;

/// one replace span per chord: (start, end, lowest key, highest key) of
/// the notes written for it
pub(crate) fn chord_spans(chords: &[ChordMark], notes: &[tl::Note]) -> Vec<(Tick, Tick, u8, u8)> {
    chords
        .iter()
        .map(|m| {
            let keys = notes
                .iter()
                .filter(|n| n.start >= m.start && n.start < m.end())
                .map(|n| n.key);
            let lo = keys.clone().min().unwrap_or(u8::MAX);
            let hi = keys.max().unwrap_or(0);
            (m.start, m.end(), lo, hi)
        })
        .collect()
}

/// what replacing would delete: indices into `old` of notes starting
/// inside a span and not written again unchanged, and how many of those
/// lie outside their chord's pitch range
pub(crate) fn replace_plan(
    old: &[tl::Note],
    spans: &[(Tick, Tick, u8, u8)],
    new: &[tl::Note],
) -> (Vec<usize>, usize) {
    let mut out = 0;
    let gone: Vec<usize> = old
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let sp = spans.iter().find(|s| n.start >= s.0 && n.start < s.1)?;
            if new
                .iter()
                .any(|x| x.start == n.start && x.key == n.key && x.len == n.len)
            {
                return None;
            }
            if n.key < sp.2 || n.key > sp.3 {
                out += 1;
            }
            Some(i)
        })
        .collect();
    (gone, out)
}

impl Default for GenDlg {
    fn default() -> Self {
        Self {
            open: false,
            opts: GenOpts::default(),
            write: false,
            voicing: Voicing::Full,
            octave: 4,
            on_top: false,
        }
    }
}

pub(crate) struct LaneUi {
    /// the Chords toggle: the lane is shown
    pub(crate) hidden: bool,
    pub(crate) sel: Option<usize>,
    drag: Option<LDrag>,
    pub(crate) pie: Option<(Radial, PieOn)>,
    /// the list menu (Settings › Chord menus: list) at this point
    pub(crate) list: Option<(Pos2, PieOn)>,
    pub(crate) gen: GenDlg,
    /// the chord edit window
    pub(crate) edit: Option<usize>,
    /// auditions queued: (seconds from `t0`, keys, seconds long)
    preview: Vec<(f64, Vec<u8>, f64)>,
    t0: Option<Instant>,
    /// screen rect of the lane (tests)
    pub(crate) rect: Rect,
    /// the "Replace notes?" confirm
    pub(crate) confirm: Option<PendingWrite>,
}

impl Default for LaneUi {
    fn default() -> Self {
        Self {
            hidden: false,
            sel: None,
            drag: None,
            pie: None,
            list: None,
            gen: GenDlg::default(),
            edit: None,
            preview: vec![],
            t0: None,
            rect: Rect::NOTHING,
            confirm: None,
        }
    }
}

/// the colour of a chord by its degree in the key
pub(crate) fn mark_colour(key: Key, m: &ChordMark) -> Color32 {
    const PAL: [(u8, u8, u8); 7] = [
        (112, 84, 186),
        (56, 112, 190),
        (34, 140, 130),
        (62, 150, 86),
        (186, 140, 54),
        (176, 72, 86),
        (160, 84, 160),
    ];
    const DEG: [usize; 12] = [0, 1, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6];
    let (r, g, b) = PAL[DEG[((m.root % 12 + 12 - key.root % 12) % 12) as usize]];
    Color32::from_rgb(r, g, b)
}

/// the plain quality of a chord from `chords` (diatonic triads / 7ths)
pub(crate) fn quality_of(iv: &[u8]) -> Quality {
    Quality::MENU
        .into_iter()
        .chain([Quality::M7b5, Quality::Dim7, Quality::Aug])
        .find(|q| q.iv() == iv)
        .unwrap_or(Quality::Maj)
}

/// the pie's short style names
pub(crate) fn pie_name(style: usize) -> &'static str {
    match cg::STYLES[style].name {
        "EDM / Trance" => "Trance",
        "Lo-fi / Neo Soul" => "Lo-fi",
        n => n,
    }
}

impl App {
    fn song_key_or_c(&self) -> Key {
        self.with(|s| s.song.info.key)
            .unwrap_or(Key::new(0, Scale::Major))
    }

    pub(super) fn cl_marks(&self, t: Target) -> Vec<ChordMark> {
        match self.pr_read(t) {
            Some(Source::Notes(d)) => d.chords.clone(),
            _ => vec![],
        }
    }

    /// write the markers (and maybe notes); grows the clip to `end` when
    /// it is shorter. `snapshot`: its own undo step
    pub(super) fn cl_write(
        &mut self,
        t: Target,
        marks: Vec<ChordMark>,
        notes: Vec<tl::Note>,
        snapshot: bool,
    ) {
        self.cl_write_ex(t, marks, notes, None, snapshot);
    }

    /// [`Self::cl_write`]; `replace`: first delete the notes that start
    /// inside these spans. A written note never doubles one already there
    /// (same start and key), so writing the same chords twice adds nothing.
    fn cl_write_ex(
        &mut self,
        t: Target,
        marks: Vec<ChordMark>,
        notes: Vec<tl::Note>,
        replace: Option<&[(Tick, Tick)]>,
        snapshot: bool,
    ) {
        let Some(Source::Notes(mut d)) = self.pr_read(t) else {
            return;
        };
        if snapshot {
            self.snapshot();
        }
        let end = marks
            .iter()
            .map(|m| m.end())
            .chain(notes.iter().map(|n| n.start + n.len))
            .max()
            .unwrap_or(0);
        let old_len = d.len;
        d.chords = marks;
        d.chords.sort_by_key(|m| m.start);
        if let Some(sp) = replace {
            let before = d.notes.len();
            d.notes
                .retain(|n| !sp.iter().any(|&(a, b)| n.start >= a && n.start < b));
            if d.notes.len() != before {
                // roll selections are note indices
                self.pr.sel.clear();
            }
        }
        for n in notes {
            if !d.notes.iter().any(|o| o.start == n.start && o.key == n.key) {
                d.notes.push(n);
            }
        }
        if end > d.len {
            d.len = (end + BAR - 1) / BAR * BAR;
        }
        let new_len = d.len;
        self.pr_write(t, Source::Notes(d));
        if new_len > old_len {
            if let Target::Notes { clip: Some(id), .. } = t {
                self.with(|s| {
                    if let Some(c) = s
                        .song
                        .timeline
                        .as_deref_mut()
                        .and_then(|tl| tl.clips.iter_mut().find(|c| c.id == id))
                    {
                        // an untrimmed, unlooped clip grows with its data
                        if c.loop_len.is_none() && c.len == old_len {
                            c.len = new_len;
                        }
                    }
                    let crate::audio::Shared { song, engine, .. } = s;
                    engine.ensure_plan(song);
                });
            }
        }
    }

    /// notes of `marks` in the dialog's voicing / octave
    fn cl_notes(&self, marks: &[ChordMark], v: Voicing, octave: u8) -> Vec<tl::Note> {
        cg::voice(marks, v, octave)
            .into_iter()
            .map(|(start, len, key)| tl::Note {
                start,
                len,
                key,
                vel: tl::roll::vel_u16(0.75),
                ..Default::default()
            })
            .collect()
    }

    /// insert generated chords: replace the markers they overlap
    pub(super) fn cl_insert(
        &mut self,
        t: Target,
        new: Vec<ChordMark>,
        write: bool,
        v: Voicing,
        octave: u8,
    ) {
        if new.is_empty() {
            return;
        }
        let (a, b) = (new[0].start, new.iter().map(|m| m.end()).max().unwrap_or(0));
        let mut marks = self.cl_marks(t);
        marks.retain(|m| m.end() <= a || m.start >= b);
        let notes = if write {
            self.cl_notes(&new, v, octave)
        } else {
            vec![]
        };
        let n = new.len();
        marks.extend(new.iter().cloned());
        if write {
            self.cl_put_notes(
                t,
                marks,
                notes,
                &new,
                format!("{n} chord(s) added with notes"),
            );
        } else {
            self.cl_write(t, marks, vec![], true);
            self.status = format!("{n} chord(s) added");
        }
    }

    /// "clip “name” on track NN, bars a–b" (bars of the clip, 1-based)
    pub(crate) fn cl_scope(&self, t: Target, a: Tick, b: Tick) -> String {
        let bars = if b > a + BAR {
            format!("bars {}–{}", a / BAR + 1, (b - 1) / BAR + 1)
        } else {
            format!("bar {}", a / BAR + 1)
        };
        let clip = match t {
            Target::Notes { clip: Some(id), .. } => self.with(|s| {
                let tl = s.song.timeline.as_deref()?;
                let c = tl.clips.iter().find(|c| c.id == id)?;
                let name = c
                    .name
                    .clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "note clip".into());
                Some(format!("“{name}” on track {:02}", c.track + 1))
            }),
            _ => None,
        };
        match clip {
            Some(c) => format!("{c}, {bars}"),
            None => format!("this clip, {bars}"),
        }
    }

    /// write chord notes: they replace the notes that start inside each
    /// chord's span (only there), or go on top (Generate's "Add on top").
    /// Asks first when that would delete more than [`CONFIRM_OVER`] notes
    /// or notes outside the chords' pitch range. One undo step.
    fn cl_put_notes(
        &mut self,
        t: Target,
        marks: Vec<ChordMark>,
        notes: Vec<tl::Note>,
        chords: &[ChordMark],
        what: String,
    ) {
        // new notes play the track's instrument (the header's)
        let instr = self.pr_track_instr(t);
        let notes: Vec<tl::Note> = notes
            .into_iter()
            .map(|n| tl::Note {
                instr: if n.instr == 0 { instr } else { n.instr },
                ..n
            })
            .collect();
        if self.cl.gen.on_top {
            let n = notes.len();
            self.cl_write_ex(t, marks, notes, None, true);
            self.status = format!("{what} · {n} note(s) added on top");
            return;
        }
        let Some(Source::Notes(d)) = self.pr_read(t) else {
            return;
        };
        let spans4 = chord_spans(chords, &notes);
        let (gone, outside) = replace_plan(&d.notes, &spans4, &notes);
        let spans: Vec<(Tick, Tick)> = spans4.iter().map(|s| (s.0, s.1)).collect();
        let a = spans.iter().map(|s| s.0).min().unwrap_or(0);
        let b = spans.iter().map(|s| s.1).max().unwrap_or(0);
        let scope = self.cl_scope(t, a, b);
        if gone.len() > CONFIRM_OVER || outside > 0 {
            self.status = format!(
                "Replace notes? {} note(s) in {scope} would be deleted",
                gone.len()
            );
            self.cl.confirm = Some(PendingWrite {
                t,
                marks,
                notes,
                spans,
                removed: gone.len(),
                outside,
                scope,
                what,
            });
            return;
        }
        self.cl_write_ex(t, marks, notes, Some(&spans), true);
        self.status = format!("{what} · replaced {} note(s) in {scope}", gone.len());
    }

    /// the "Replace notes?" confirm (while a write waits for it)
    pub(super) fn cl_confirm_window(&mut self, ctx: &egui::Context) {
        let Some(pw) = &self.cl.confirm else { return };
        let th = th();
        let mut choice = None;
        let mut open = true;
        let mut cancel = false;
        egui::Window::new("Replace notes?")
            .id(egui::Id::unique("cl_replace_confirm"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(format!("Target: {}", pw.scope));
                ui.label(
                    egui::RichText::new(
                        "Replaces the notes that start inside each chord's span; \
                         the rest of the clip is kept.",
                    )
                    .color(th.ink2),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("{} note(s) will be deleted", pw.removed)).strong(),
                );
                if pw.outside > 0 {
                    ui.label(format!(
                        "{} of them are outside the new chords' pitch range \
                         (a melody or bass line?)",
                        pw.outside
                    ));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            egui::RichText::new(format!("Replace (delete {})", pw.removed))
                                .strong(),
                        )
                        .clicked()
                    {
                        choice = Some(true);
                    }
                    if ui
                        .button("Add on top")
                        .on_hover_text("keep every note and add the chord notes")
                        .clicked()
                    {
                        choice = Some(false);
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if let Some(replace) = choice {
            let Some(pw) = self.cl.confirm.take() else {
                return;
            };
            let n = pw.notes.len();
            if replace {
                self.cl_write_ex(pw.t, pw.marks, pw.notes, Some(&pw.spans), true);
                self.status = format!(
                    "{} · replaced {} note(s) in {}",
                    pw.what, pw.removed, pw.scope
                );
            } else {
                self.cl_write_ex(pw.t, pw.marks, pw.notes, None, true);
                self.status = format!("{} · {n} note(s) added on top", pw.what);
            }
        } else if cancel || !open {
            self.cl.confirm = None;
            self.status = "Chord notes not written".into();
        }
    }

    /// play one chord now (on the track's instrument)
    pub(super) fn cl_play(&mut self, m: &ChordMark) {
        self.cl.preview.clear();
        let keys: Vec<u8> = cg::voice(std::slice::from_ref(m), Voicing::Full, 4)
            .into_iter()
            .map(|x| x.2)
            .collect();
        self.pr_audition_keys(&keys, 900);
    }

    /// play chords one after another at the song tempo (max 8)
    pub(super) fn cl_play_seq(&mut self, marks: &[ChordMark]) {
        let bpm = self.with(|s| s.song.bpm).max(20.0) as f64;
        let spt = 60.0 / (bpm * PPQ as f64);
        let t0 = marks.first().map_or(0, |m| m.start);
        self.cl.preview = marks
            .iter()
            .take(8)
            .map(|m| {
                let keys = cg::voice(std::slice::from_ref(m), Voicing::Full, 4)
                    .into_iter()
                    .map(|x| x.2)
                    .collect();
                (
                    (m.start - t0) as f64 * spt,
                    keys,
                    (m.len as f64 * spt).min(2.0),
                )
            })
            .collect();
        self.cl.t0 = Some(Instant::now());
    }

    /// queued auditions that are due (every frame)
    pub(super) fn cl_tick(&mut self, ctx: &egui::Context) {
        let Some(t0) = self.cl.t0 else { return };
        let now = t0.elapsed().as_secs_f64();
        while self.cl.preview.first().is_some_and(|p| p.0 <= now) {
            let (_, keys, dur) = self.cl.preview.remove(0);
            self.pr_audition_keys(&keys, (dur * 1000.0) as u64);
        }
        if self.cl.preview.is_empty() {
            self.cl.t0 = None;
        } else {
            ctx.request_repaint();
        }
    }

    /// the chord lane row (note clips)
    pub(super) fn chord_lane(
        &mut self,
        ui: &mut egui::Ui,
        r: Rect,
        t: Target,
        grid_t: Tick,
        len: Tick,
        modal: bool,
    ) {
        let th = th();
        self.cl.rect = r;
        let key = self.song_key_or_c();
        let marks = self.cl_marks(t);
        let p = ui.painter_at(r);
        p.rect_filled(r, 0.0, th.crt_top.gamma_multiply(0.6));
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, th.border));
        let (sx, ppt, gl) = (
            self.pr.scroll_x,
            self.pr.zoom as f64 / tl::ROW as f64,
            self.pr.grid_rect.left(),
        );
        let xo = move |t: Tick| gl + ((t as f64 - sx) * ppt) as f32;
        let mrect = move |m: &ChordMark| {
            Rect::from_min_max(
                Pos2::new(xo(m.start), r.top() + 2.0),
                Pos2::new(xo(m.end()).max(xo(m.start) + 4.0), r.bottom() - 2.0),
            )
        };
        for (i, m) in marks.iter().enumerate() {
            let mr = mrect(m);
            if !mr.intersects(r) {
                continue;
            }
            let c = mark_colour(key, m);
            p.rect_filled(
                mr.shrink2(egui::vec2(0.5, 0.0)),
                3.0,
                c.gamma_multiply(0.85),
            );
            let sel = self.cl.sel == Some(i);
            p.rect_stroke(
                mr.shrink2(egui::vec2(0.5, 0.0)),
                3.0,
                if sel {
                    Stroke::new(2.0, th.selection_outline)
                } else {
                    Stroke::new(1.0, Color32::from_black_alpha(110))
                },
                egui::StrokeKind::Inside,
            );
            let name = m.name(key);
            let g = p.layout_no_wrap(name, FontId::proportional(11.0), Color32::WHITE);
            let gw = g.size().x;
            if mr.width() > 18.0 {
                p.galley(
                    Pos2::new(mr.left() + 4.0, mr.center().y - g.size().y / 2.0),
                    g,
                    Color32::WHITE,
                );
            }
            if mr.width() > gw + 40.0 {
                p.text(
                    Pos2::new(mr.left() + gw + 10.0, mr.center().y),
                    egui::Align2::LEFT_CENTER,
                    m.numeral(key),
                    FontId::proportional(9.5),
                    Color32::from_white_alpha(170),
                );
            }
        }
        if marks.is_empty() {
            p.text(
                Pos2::new(r.left() + 6.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                "Chords: double-click to add one, drag to draw one, right-click for styles and progressions",
                FontId::proportional(10.0),
                th.ink2,
            );
        }
        // ---- input
        let resp = ui.interact(
            r,
            egui::Id::unique("pr_chord_lane"),
            if modal {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            },
        );
        let hover = resp.hover_pos();
        let hit = |pos: Pos2| -> Option<(usize, bool)> {
            marks.iter().enumerate().rev().find_map(|(i, m)| {
                let mr = mrect(m);
                mr.contains(pos)
                    .then(|| (i, pos.x > mr.right() - EDGE.min(mr.width() / 3.0)))
            })
        };
        let snap = grid_t.max(PPQ);
        let tick_at = move |x: f32| sx + ((x - gl) as f64 / ppt);
        if let Some(h) = hover {
            match hit(h) {
                Some((i, edge)) => {
                    if edge {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    let m = &marks[i];
                    resp.clone().on_hover_text_at_pointer(format!(
                        "{} · {} · {}\nClick: play  Drag: move  Edge: length  Right-click: chord menu",
                        m.name(key),
                        m.numeral(key),
                        m.q.long()
                    ));
                }
                None => {
                    resp.clone().on_hover_text_at_pointer(
                        "Double-click: add a chord  Drag: draw a chord  Right-click: more",
                    );
                }
            }
        }
        let press = ui.input(|i| i.pointer.press_origin());
        if resp.double_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if hit(pos).is_none() {
                    let at = (tick_at(pos.x).max(0.0) as Tick).div_euclid(BAR) * BAR;
                    self.cl_add_tonic(t, at, BAR);
                }
            }
        } else if resp.drag_started_by(egui::PointerButton::Primary) {
            let pos = press.unwrap_or(r.center());
            let tk = tick_at(pos.x);
            match hit(pos) {
                Some((i, edge)) => {
                    self.cl.sel = Some(i);
                    self.cl.drag = Some(LDrag {
                        kind: if edge { LDragK::Resize } else { LDragK::Move },
                        idx: i,
                        grab: tk,
                        origin: marks.clone(),
                        moved: edge,
                    });
                }
                None => {
                    // draw: a new chord from here, its length follows the mouse
                    let start = (tk.max(0.0) as Tick).div_euclid(snap) * snap;
                    let mut ms = marks.clone();
                    let c = crate::chords::diatonic(key, 0, crate::chords::ChordKind::Triad);
                    ms.push(ChordMark {
                        start,
                        len: snap,
                        root: c.root,
                        q: quality_of(&c.iv),
                        bass: None,
                    });
                    let idx = ms.len() - 1;
                    self.cl_write(t, ms.clone(), vec![], false);
                    self.cl.sel = Some(idx);
                    self.cl.drag = Some(LDrag {
                        kind: LDragK::Resize,
                        idx,
                        grab: tk,
                        origin: ms,
                        moved: true,
                    });
                }
            }
        } else if resp.dragged() {
            if let (Some(d), Some(pos)) = (self.cl.drag.clone(), resp.interact_pointer_pos()) {
                let tk = tick_at(pos.x);
                if d.moved || (tk - d.grab).abs() >= snap as f64 / 2.0 {
                    let mut ms = d.origin.clone();
                    if let Some(m) = ms.get_mut(d.idx) {
                        match d.kind {
                            LDragK::Move => {
                                let o = d.origin[d.idx].start;
                                m.start = (tl::snap(o + (tk - d.grab) as Tick, snap))
                                    .clamp(0, (len - m.len).max(0));
                            }
                            LDragK::Resize => {
                                m.len = (tl::snap(tk as Tick, snap) - m.start).max(snap);
                            }
                        }
                    }
                    if let Some(x) = self.cl.drag.as_mut() {
                        x.moved = true;
                    }
                    self.cl_write(t, ms, vec![], false);
                }
            }
        } else if resp.clicked() {
            let pos = resp.interact_pointer_pos().unwrap_or(r.center());
            match hit(pos) {
                Some((i, _)) => {
                    self.cl.sel = Some(i);
                    let m = marks[i].clone();
                    self.cl_play(&m);
                    self.status = format!("{} ({})", m.name(key), m.numeral(key));
                }
                None => self.cl.sel = None,
            }
        }
        if resp.drag_stopped() {
            if let Some(d) = self.cl.drag.take() {
                // the drag's result sorted; keep the moved chord selected
                let ms = self.cl_marks(t);
                let moved = d.origin.get(d.idx).cloned();
                let _ = moved;
                let mut sorted = ms.clone();
                sorted.sort_by_key(|m| m.start);
                if sorted != ms {
                    let sel = self.cl.sel.and_then(|i| ms.get(i).cloned());
                    self.cl_write(t, sorted.clone(), vec![], false);
                    self.cl.sel = sel.and_then(|s| sorted.iter().position(|m| *m == s));
                }
            }
        }
        if resp.secondary_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let on = match hit(pos) {
                    Some((i, _)) => {
                        self.cl.sel = Some(i);
                        PieOn::Mark(i)
                    }
                    None => PieOn::Lane((tick_at(pos.x).max(0.0) as Tick).div_euclid(BAR) * BAR),
                };
                self.cl_open_menu(ui.ctx(), pos, on, t);
            }
        }
    }

    /// add the key's tonic chord
    pub(super) fn cl_add_tonic(&mut self, t: Target, at: Tick, len: Tick) {
        let key = self.song_key_or_c();
        let c = crate::chords::diatonic(key, 0, crate::chords::ChordKind::Triad);
        let mut ms = self.cl_marks(t);
        ms.retain(|m| m.end() <= at || m.start >= at + len);
        let m = ChordMark {
            start: at,
            len,
            root: c.root,
            q: quality_of(&c.iv),
            bass: None,
        };
        ms.push(m.clone());
        ms.sort_by_key(|m| m.start);
        self.cl_write(t, ms.clone(), vec![], true);
        self.cl.sel = ms.iter().position(|x| *x == m);
        self.cl_play(&m);
    }

    /// right-click: the pie, or the list menu (Settings)
    pub(super) fn cl_open_menu(&mut self, ctx: &egui::Context, pos: Pos2, on: PieOn, t: Target) {
        if self.extra.chord_list_menu {
            self.cl.list = Some((pos, on));
            return;
        }
        let key = self.song_key_or_c();
        let mut pie = match on {
            PieOn::Lane(_) => {
                let mut items: Vec<RadialItem> = cg::PIE_STYLES
                    .iter()
                    .map(|&s| {
                        RadialItem::new(
                            pie_name(s),
                            None,
                            cg::STYLES[s]
                                .progs
                                .iter()
                                .map(|p| cg::prog_label(p))
                                .collect(),
                        )
                    })
                    .collect();
                items.push(RadialItem::new("Add chord", Some(RIcon::Add), vec![]));
                items.push(RadialItem::new("More…", Some(RIcon::More), vec![]));
                Radial::new(pos, "CHORDS", "Point to choose", items)
            }
            PieOn::Mark(i) => {
                let Some(m) = self.cl_marks(t).get(i).cloned() else {
                    return;
                };
                let sugg: Vec<String> = cg::suggestions(key, &m)
                    .iter()
                    .map(|(_, c)| c.name(key))
                    .collect();
                let items = vec![
                    RadialItem::new("Play", Some(RIcon::Play), vec![]),
                    RadialItem::new(
                        "Transpose",
                        Some(RIcon::Transpose),
                        TRANSPOSE.iter().map(|x| x.0.to_string()).collect(),
                    ),
                    RadialItem::new(
                        "Chord type",
                        Some(RIcon::Type),
                        Quality::MENU
                            .iter()
                            .map(|q| type_label(*q).to_string())
                            .collect(),
                    ),
                    RadialItem::new("Suggestions", Some(RIcon::Bulb), sugg),
                    RadialItem::new("Generate", Some(RIcon::Generate), vec![]),
                    RadialItem::new("Select", Some(RIcon::Select), vec![]),
                    RadialItem::new("Delete", Some(RIcon::Delete), vec![]),
                    RadialItem::new(
                        "Write notes",
                        Some(RIcon::Notes),
                        Voicing::ALL.iter().map(|v| v.name().to_string()).collect(),
                    ),
                    RadialItem::new("Edit", Some(RIcon::Edit), vec![]),
                ];
                Radial::new(pos, m.name(key), m.q.long(), items)
            }
        };
        pie.clamp_to(ctx.content_rect());
        self.cl.pie = Some((pie, on));
    }

    /// what pie entry (item, child) would make of the chord `m`
    fn cl_mark_variant(&self, m: &ChordMark, item: usize, child: usize) -> Option<ChordMark> {
        let key = self.song_key_or_c();
        let mut n = m.clone();
        match item {
            1 => {
                let (_, semis, steps) = TRANSPOSE.get(child)?;
                if *steps != 0 {
                    // up / down a scale step: the diatonic chord one degree over
                    let st = key.scale.steps();
                    let off = (m.root % 12 + 12 - key.root % 12) % 12;
                    let deg = st.iter().position(|&s| s == off).unwrap_or(0) as i32;
                    let nd = (deg + steps).rem_euclid(st.len() as i32) as usize;
                    let kind = if m.q.iv().len() > 3 {
                        crate::chords::ChordKind::Seventh
                    } else {
                        crate::chords::ChordKind::Triad
                    };
                    let c = crate::chords::diatonic(key, nd, kind);
                    n.root = c.root;
                    n.q = quality_of(&c.iv);
                    n.bass = None;
                } else {
                    n.root = ((m.root as i32 + semis).rem_euclid(12)) as u8;
                    n.bass = m.bass.map(|b| ((b as i32 + semis).rem_euclid(12)) as u8);
                }
            }
            2 => {
                n.q = *Quality::MENU.get(child)?;
            }
            3 => {
                n = cg::suggestions(key, m).get(child)?.1.clone();
            }
            _ => return None,
        }
        Some(n)
    }

    /// the pies, the list menu, the edit window and the Generate dialog
    pub(super) fn chord_popups(&mut self, ctx: &egui::Context, t: Target) {
        self.cl_tick(ctx);
        let key = self.song_key_or_c();
        if let Some((mut pie, on)) = self.cl.pie.take() {
            let ev = pie.show(ctx);
            let mut keep = true;
            match (ev, on) {
                (RadialEv::Cancel, _) => keep = false,
                (RadialEv::Hover(i, Some(c)), PieOn::Lane(at)) if i < cg::PIE_STYLES.len() => {
                    let marks = self.cl_gen_pie(cg::PIE_STYLES[i], c, at);
                    self.cl_play_seq(&marks);
                }
                (RadialEv::Hover(i, Some(c)), PieOn::Mark(mi)) => {
                    if let Some(m) = self.cl_marks(t).get(mi).cloned() {
                        if let Some(n) = self.cl_mark_variant(&m, i, c) {
                            self.cl_play(&n);
                        }
                    }
                }
                (RadialEv::Pick(i, c), PieOn::Lane(at)) => {
                    keep = false;
                    let n = cg::PIE_STYLES.len();
                    match (i, c) {
                        (i, Some(c)) if i < n => {
                            let marks = self.cl_gen_pie(cg::PIE_STYLES[i], c, at);
                            let (w, v, o) =
                                (self.cl.gen.write, self.cl.gen.voicing, self.cl.gen.octave);
                            self.cl_insert(t, marks, w, v, o);
                        }
                        (i, None) if i < n => self.cl_open_gen(Some(cg::PIE_STYLES[i]), at),
                        (i, _) if i == n => self.cl_add_tonic(t, at, BAR),
                        _ => self.cl_open_gen(None, at),
                    }
                }
                (RadialEv::Pick(i, c), PieOn::Mark(mi)) => {
                    keep = false;
                    self.cl_mark_action(t, mi, i, c);
                }
                _ => {}
            }
            if keep {
                self.cl.pie = Some((pie, on));
            }
        }
        if let Some((pos, on)) = self.cl.list {
            let mut close = false;
            let area = egui::Area::new(egui::Id::unique("pr_chord_list"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    egui::Frame::menu(ui.style()).show(ui, |ui| {
                        ui.set_min_width(170.0);
                        match on {
                            PieOn::Lane(at) => {
                                for &s in cg::PIE_STYLES.iter() {
                                    ui.menu_button(pie_name(s), |ui| {
                                        for (c, p) in cg::STYLES[s].progs.iter().enumerate() {
                                            let r = ui.button(cg::prog_label(p));
                                            if r.hovered()
                                                && !r.has_focus()
                                                && ui.input(|i| i.pointer.delta().length() > 0.0)
                                            {
                                                let marks = self.cl_gen_pie(s, c, at);
                                                self.cl_play_seq(&marks);
                                            }
                                            if r.clicked() {
                                                let marks = self.cl_gen_pie(s, c, at);
                                                let (w, v, o) = (
                                                    self.cl.gen.write,
                                                    self.cl.gen.voicing,
                                                    self.cl.gen.octave,
                                                );
                                                self.cl_insert(t, marks, w, v, o);
                                                close = true;
                                            }
                                        }
                                    });
                                }
                                ui.separator();
                                if ui.button("Add chord").clicked() {
                                    self.cl_add_tonic(t, at, BAR);
                                    close = true;
                                }
                                if ui.button("Generate…").clicked() {
                                    self.cl_open_gen(None, at);
                                    close = true;
                                }
                                if ui.button("Detect chords from the notes").clicked() {
                                    self.cl_detect(t);
                                    close = true;
                                }
                            }
                            PieOn::Mark(mi) => {
                                let Some(m) = self.cl_marks(t).get(mi).cloned() else {
                                    close = true;
                                    return;
                                };
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · {}",
                                        m.name(key),
                                        m.q.long()
                                    ))
                                    .strong(),
                                );
                                if ui.button("Play").clicked() {
                                    self.cl_play(&m);
                                }
                                for (item, title, labels) in [
                                    (
                                        1usize,
                                        "Transpose",
                                        TRANSPOSE
                                            .iter()
                                            .map(|x| x.0.to_string())
                                            .collect::<Vec<_>>(),
                                    ),
                                    (
                                        2,
                                        "Chord type",
                                        Quality::MENU
                                            .iter()
                                            .map(|q| type_label(*q).to_string())
                                            .collect(),
                                    ),
                                    (
                                        3,
                                        "Suggestions",
                                        cg::suggestions(key, &m)
                                            .iter()
                                            .map(|(why, c)| format!("{}  ({why})", c.name(key)))
                                            .collect(),
                                    ),
                                    (
                                        7,
                                        "Write notes",
                                        Voicing::ALL.iter().map(|v| v.name().to_string()).collect(),
                                    ),
                                ] {
                                    ui.menu_button(title, |ui| {
                                        for (c, l) in labels.iter().enumerate() {
                                            if ui.button(l).clicked() {
                                                self.cl_mark_action(t, mi, item, Some(c));
                                                close = true;
                                            }
                                        }
                                    });
                                }
                                for (item, l) in [
                                    (8usize, "Edit…"),
                                    (4, "Generate from here…"),
                                    (5, "Select its notes"),
                                    (6, "Delete"),
                                ] {
                                    if ui.button(l).clicked() {
                                        self.cl_mark_action(t, mi, item, None);
                                        close = true;
                                    }
                                }
                            }
                        }
                    });
                });
            let clicked_out = ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
                && ctx
                    .input(|i| i.pointer.interact_pos())
                    .is_some_and(|p| !area.response.rect.expand(4.0).contains(p));
            if close || clicked_out || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.cl.list = None;
            }
        }
        self.cl_edit_window(ctx, t);
        self.cl_gen_window(ctx, t);
        self.cl_confirm_window(ctx);
    }

    /// a pie progression at `at` with the dialog's remembered options
    fn cl_gen_pie(&self, style: usize, prog: usize, at: Tick) -> Vec<ChordMark> {
        let key = self.song_key_or_c();
        let o = GenOpts {
            style,
            prog: Some(prog),
            root: key.root,
            start_bar: (at / BAR) as u32,
            bars: cg::parse_prog(cg::STYLES[style].progs[prog])
                .len()
                .clamp(4, 16) as u32,
            ..self.cl.gen.opts
        };
        let o = GenOpts {
            bars: ((cg::STYLES[style].bars
                * match o.speed {
                    s if s < 0 => 2.0,
                    s if s > 0 => 0.5,
                    _ => 1.0,
                })
                * cg::parse_prog(cg::STYLES[style].progs[prog]).len() as f32)
                .ceil()
                .max(1.0) as u32,
            ..o
        };
        cg::generate(&o).marks
    }

    pub(super) fn cl_open_gen_at(&mut self, at: Tick) {
        self.cl_open_gen(None, at);
    }

    /// Del on a selected chord marker
    pub(super) fn cl_delete_sel(&mut self) {
        let (Some(t), Some(i)) = (self.pr.target, self.cl.sel) else {
            return;
        };
        let mut ms = self.cl_marks(t);
        if i < ms.len() {
            ms.remove(i);
            self.cl.sel = None;
            self.cl_write(t, ms, vec![], true);
            self.status = "Chord deleted".into();
        }
    }

    fn cl_open_gen(&mut self, style: Option<usize>, at: Tick) {
        let key = self.song_key_or_c();
        let g = &mut self.cl.gen;
        g.open = true;
        g.opts.start_bar = (at / BAR) as u32;
        g.opts.root = key.root;
        g.opts.prog = None;
        if let Some(s) = style {
            g.opts.style = s;
        }
        g.opts.mode = match key.scale {
            Scale::Major | Scale::Lydian | Scale::Mixolydian | Scale::MajorPentatonic => {
                Mode::Major
            }
            _ => Mode::Minor,
        };
    }

    /// chord pie / list actions on marker `mi`
    fn cl_mark_action(&mut self, t: Target, mi: usize, item: usize, child: Option<usize>) {
        let mut marks = self.cl_marks(t);
        let Some(m) = marks.get(mi).cloned() else {
            return;
        };
        match (item, child) {
            (0, _) => self.cl_play(&m),
            (1..=3, Some(c)) => {
                if let Some(n) = self.cl_mark_variant(&m, item, c) {
                    let key = self.song_key_or_c();
                    self.status = format!("{} → {}", m.name(key), n.name(key));
                    marks[mi] = n.clone();
                    self.cl_write(t, marks, vec![], true);
                    self.cl_play(&n);
                }
            }
            (4, _) => self.cl_open_gen(None, m.start),
            (5, _) => {
                if let Some(src) = self.pr_read(t) {
                    let (notes, _) = super::pianoview::notes_of(&src, t);
                    self.pr.sel = notes
                        .iter()
                        .filter(|n| n.start < m.end() && n.start + n.len > m.start)
                        .map(|n| n.id)
                        .collect();
                    self.status = format!(
                        "{} note(s) under {}",
                        self.pr.sel.len(),
                        m.name(self.song_key_or_c())
                    );
                }
            }
            (6, _) => {
                marks.remove(mi);
                self.cl.sel = None;
                self.cl_write(t, marks, vec![], true);
            }
            (7, c) => {
                let v = c
                    .and_then(|c| Voicing::ALL.get(c).copied())
                    .unwrap_or(self.cl.gen.voicing);
                let notes = self.cl_notes(std::slice::from_ref(&m), v, self.cl.gen.octave);
                let what = format!(
                    "{} {} note(s) written ({})",
                    m.name(self.song_key_or_c()),
                    notes.len(),
                    v.name()
                );
                self.cl_put_notes(t, marks, notes, std::slice::from_ref(&m), what);
            }
            (8, _) => self.cl.edit = Some(mi),
            _ => {}
        }
    }

    /// chords from the clip's notes, one per bar
    pub(super) fn cl_detect(&mut self, t: Target) {
        let Some(Source::Notes(d)) = self.pr_read(t) else {
            return;
        };
        let marks = cg::detect(&d.notes, d.len, BAR);
        let n = marks.len();
        self.cl_write(t, marks, vec![], true);
        self.status = format!("{n} chord(s) detected");
    }

    fn cl_edit_window(&mut self, ctx: &egui::Context, t: Target) {
        let Some(mi) = self.cl.edit else { return };
        let marks = self.cl_marks(t);
        let Some(m) = marks.get(mi).cloned() else {
            self.cl.edit = None;
            return;
        };
        let key = self.song_key_or_c();
        let mut e = m.clone();
        let mut open = true;
        egui::Window::new("Edit chord")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("cl_edit").num_columns(2).show(ui, |ui| {
                    ui.label("Root");
                    egui::ComboBox::from_id_salt("cl_root")
                        .selected_text(key.note_name(e.root))
                        .show_ui(ui, |ui| {
                            for r in 0..12u8 {
                                ui.selectable_value(&mut e.root, r, key.note_name(r));
                            }
                        });
                    ui.end_row();
                    ui.label("Type");
                    egui::ComboBox::from_id_salt("cl_q")
                        .selected_text(e.q.long())
                        .show_ui(ui, |ui| {
                            for q in Quality::MENU {
                                ui.selectable_value(&mut e.q, q, q.long());
                            }
                        });
                    ui.end_row();
                    ui.label("Bass");
                    egui::ComboBox::from_id_salt("cl_bass")
                        .selected_text(
                            e.bass
                                .map_or("root".to_string(), |b| key.note_name(b).to_string()),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut e.bass, None, "root");
                            for b in m.pcs() {
                                ui.selectable_value(&mut e.bass, Some(b), key.note_name(b));
                            }
                        });
                    ui.end_row();
                    ui.label("Length (beats)");
                    let mut beats = (e.len / PPQ).max(1);
                    if ui
                        .add(egui::DragValue::new(&mut beats).range(1..=64))
                        .changed()
                    {
                        e.len = beats * PPQ;
                    }
                    ui.end_row();
                });
                ui.label(
                    egui::RichText::new(format!("{}  ·  {}", e.name(key), e.numeral(key))).strong(),
                );
                if ui.button("Play").clicked() {
                    self.cl_play(&e);
                }
            });
        if e != m {
            let mut ms = marks;
            ms[mi] = e;
            self.cl_write(t, ms, vec![], true);
        }
        if !open {
            self.cl.edit = None;
        }
    }

    fn cl_gen_window(&mut self, ctx: &egui::Context, t: Target) {
        if !self.cl.gen.open {
            return;
        }
        let th = th();
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let mut preview = false;
        let res = cg::generate(&self.cl.gen.opts);
        // the style `res` was made in: a style button clicked below changes
        // `opts.style` this frame, and `res.prog` indexes the old style's list
        let res_style = self.cl.gen.opts.style.min(cg::STYLES.len() - 1);
        let res_len = cg::STYLES[res_style]
            .progs
            .get(res.prog)
            .map_or(0, |p| cg::parse_prog(p).len());
        let track = match t {
            Target::Notes { clip: Some(id), .. } => self
                .tlv
                .cache
                .as_ref()
                .and_then(|c| c.tl.clips.iter().find(|x| x.id == id))
                .map(|c| c.track as usize),
            _ => None,
        };
        let inst = self.with(|s| {
            track
                .and_then(|k| s.song.channel_names.get(k).cloned())
                .filter(|n| !n.is_empty())
                .or_else(|| track.map(|k| format!("track {}", k + 1)))
                .unwrap_or_else(|| "the track".into())
        });
        // where Apply writes: the progression's span in this clip
        let scope = {
            let a = res.marks.iter().map(|m| m.start).min().unwrap_or(0);
            let b = res.marks.iter().map(|m| m.end()).max().unwrap_or(a);
            self.cl_scope(t, a, b)
        };
        egui::Window::new("Generate chord progression")
            .id(egui::Id::unique("cl_gen_win"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(620.0)
            .show(ctx, |ui| {
                let g = &mut self.cl.gen;
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                ui.label(egui::RichText::new("Style").color(th.ink2));
                egui::Grid::new("cl_styles")
                    .spacing(egui::vec2(6.0, 6.0))
                    .show(ui, |ui| {
                        for (i, s) in cg::STYLES.iter().enumerate() {
                            let on = g.opts.style == i;
                            if ui
                                .add_sized([112.0, 24.0], egui::Button::new(s.name).selected(on))
                                .on_hover_text(s.desc)
                                .clicked()
                            {
                                g.opts.style = i;
                                g.opts.prog = None;
                            }
                            if i % 5 == 4 {
                                ui.end_row();
                            }
                        }
                    });
                ui.label(
                    egui::RichText::new(cg::STYLES[g.opts.style].desc)
                        .italics()
                        .color(th.ink2),
                );
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Key").color(th.ink2));
                ui.horizontal(|ui| {
                    let k = Key::new(
                        0,
                        if g.opts.mode == Mode::Major {
                            Scale::Major
                        } else {
                            Scale::Minor
                        },
                    );
                    for r in 0..12u8 {
                        let name = match r {
                            1 => "C#",
                            3 => "Eb",
                            6 => "F#",
                            8 => "Ab",
                            10 => "Bb",
                            _ => k.note_name(r),
                        };
                        if ui
                            .add_sized(
                                [30.0, 22.0],
                                egui::Button::new(name).selected(g.opts.root == r),
                            )
                            .clicked()
                        {
                            g.opts.root = r;
                        }
                    }
                    ui.add_space(12.0);
                    for (m, l) in [
                        (Mode::Any, "Any"),
                        (Mode::Major, "Major"),
                        (Mode::Minor, "Minor"),
                    ] {
                        if ui
                            .add_sized(
                                [50.0, 22.0],
                                egui::Button::new(l).selected(g.opts.mode == m),
                            )
                            .clicked()
                        {
                            g.opts.mode = m;
                            g.opts.prog = None;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Length").color(th.ink2));
                    for b in [4u32, 8, 16, 32] {
                        if ui
                            .add_sized(
                                [30.0, 22.0],
                                egui::Button::new(b.to_string()).selected(g.opts.bars == b),
                            )
                            .on_hover_text(format!("{b} bars"))
                            .clicked()
                        {
                            g.opts.bars = b;
                        }
                    }
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("Chord speed").color(th.ink2));
                    for (s, l) in [(-1i8, "Slower"), (0, "Style"), (1, "Faster")] {
                        if ui
                            .add_sized(
                                [54.0, 22.0],
                                egui::Button::new(l).selected(g.opts.speed == s),
                            )
                            .clicked()
                        {
                            g.opts.speed = s;
                        }
                    }
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("Starts at bar").color(th.ink2));
                    ui.label(egui::RichText::new((g.opts.start_bar + 1).to_string()).strong());
                    if ui.small_button(" - ").clicked() {
                        g.opts.start_bar = g.opts.start_bar.saturating_sub(1);
                    }
                    if ui.small_button(" + ").clicked() {
                        g.opts.start_bar += 1;
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().slider_width = 200.0;
                    let mut c = g.opts.colour * 100.0;
                    ui.add(
                        egui::Slider::new(&mut c, 0.0..=100.0)
                            .trailing_fill(true)
                            .fixed_decimals(0)
                            .suffix("%")
                            .text("Colour"),
                    )
                    .on_hover_text("7ths, 9ths and suspensions");
                    g.opts.colour = c / 100.0;
                    ui.add_space(10.0);
                    let mut v = g.opts.variation * 100.0;
                    ui.add(
                        egui::Slider::new(&mut v, 0.0..=100.0)
                            .trailing_fill(true)
                            .fixed_decimals(0)
                            .suffix("%")
                            .text("Variation"),
                    )
                    .on_hover_text("swap in related chords (relative, sus, 7ths)");
                    g.opts.variation = v / 100.0;
                });
                ui.checkbox(&mut g.opts.walking_bass, "Walking bass (inversions)")
                    .on_hover_text("slash chords: the bass moves to the nearest chord tone");
                ui.separator();
                // the result
                let key = res.key;
                let kname = format!(
                    "{} {}",
                    key.note_name(key.root),
                    if key.scale == Scale::Minor {
                        "minor"
                    } else {
                        "major"
                    }
                );
                let mut seen = vec![];
                let nums: Vec<String> = res
                    .marks
                    .iter()
                    .filter_map(|m| {
                        let n = m.numeral(key);
                        (seen.len() < 16 && seen.len() < res_len).then(|| {
                            seen.push(n.clone());
                            n
                        })
                    })
                    .collect();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(kname).strong());
                    ui.label(egui::RichText::new(nums.join(" - ")).color(th.ink2));
                });
                let (sr, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width().max(560.0), 46.0),
                    Sense::hover(),
                );
                let total = res.marks.iter().map(|m| m.end()).max().unwrap_or(1)
                    - res.marks.first().map_or(0, |m| m.start);
                let t0 = res.marks.first().map_or(0, |m| m.start);
                let p = ui.painter_at(sr);
                for m in &res.marks {
                    let x0 = sr.left() + sr.width() * ((m.start - t0) as f32 / total.max(1) as f32);
                    let x1 = sr.left() + sr.width() * ((m.end() - t0) as f32 / total.max(1) as f32);
                    let r = Rect::from_min_max(
                        Pos2::new(x0 + 1.0, sr.top()),
                        Pos2::new(x1 - 1.0, sr.bottom()),
                    );
                    p.rect_filled(r, 3.0, mark_colour(key, m));
                    if r.width() > 20.0 {
                        p.text(
                            r.left_top() + egui::vec2(5.0, 5.0),
                            egui::Align2::LEFT_TOP,
                            m.name(key),
                            FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                        p.text(
                            r.left_bottom() + egui::vec2(5.0, -5.0),
                            egui::Align2::LEFT_BOTTOM,
                            m.numeral(key),
                            FontId::proportional(10.0),
                            Color32::from_white_alpha(180),
                        );
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized([96.0, 26.0], egui::Button::new("Another one"))
                        .on_hover_text("roll a new progression in this style")
                        .clicked()
                    {
                        g.opts.seed = g.opts.seed.wrapping_add(1);
                        g.opts.prog = None;
                    }
                    if ui
                        .add_sized([80.0, 26.0], egui::Button::new("Preview"))
                        .on_hover_text("hear it on the track's instrument")
                        .clicked()
                    {
                        preview = true;
                    }
                    ui.add_space(10.0);
                    ui.checkbox(&mut g.write, format!("Also write the notes on {inst}"));
                    egui::ComboBox::from_id_salt("cl_voicing")
                        .width(84.0)
                        .selected_text(g.voicing.name())
                        .show_ui(ui, |ui| {
                            for v in Voicing::ALL {
                                ui.selectable_value(&mut g.voicing, v, v.name());
                            }
                        });
                    ui.label("Octave");
                    ui.add(egui::DragValue::new(&mut g.octave).range(1..=7));
                });
                // the target and what Apply does to the notes already there
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Target:").color(th.ink2));
                    ui.label(egui::RichText::new(&scope).strong());
                    let what = if !g.write {
                        "chord markers only (notes untouched)"
                    } else if g.on_top {
                        "notes added on top of the clip's notes"
                    } else {
                        "replaces the notes inside each chord's span only"
                    };
                    ui.label(egui::RichText::new(format!("· {what}")).color(th.ink2));
                });
                ui.add_enabled_ui(g.write, |ui| {
                    ui.checkbox(&mut g.on_top, "Add on top (keep the notes already there)")
                        .on_hover_text(
                            "off: the chord notes replace the notes that start inside each \
                             chord's span (asks first if that deletes more than 8 notes or \
                             notes outside the chords' range)",
                        );
                });
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [80.0, 26.0],
                            egui::Button::new(egui::RichText::new("Apply").strong()).fill(th.sel),
                        )
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui
                        .add_sized([80.0, 26.0], egui::Button::new("Cancel"))
                        .clicked()
                    {
                        cancel = true;
                    }
                });
            });
        if preview {
            self.cl_play_seq(&res.marks);
        }
        if apply {
            let (w, v, o) = (self.cl.gen.write, self.cl.gen.voicing, self.cl.gen.octave);
            if self.with(|s| s.song.info.key.is_none()) {
                self.with(|s| s.song.info.key = Some(res.key));
            }
            self.cl_insert(t, res.marks, w, v, o);
            self.cl.gen.open = false;
        }
        if cancel || !open {
            self.cl.gen.open = false;
        }
    }
}

/// transpose entries: (label, semitones, scale steps)
const TRANSPOSE: [(&str, i32, i32); 8] = [
    ("-5", -5, 0),
    ("-2", -2, 0),
    ("-1", -1, 0),
    ("step -", 0, -1),
    ("step +", 0, 1),
    ("+1", 1, 0),
    ("+2", 2, 0),
    ("+5", 5, 0),
];

/// chord-type labels in the pie ("maj" for a plain major)
fn type_label(q: Quality) -> &'static str {
    match q {
        Quality::Maj => "maj",
        Quality::Min => "min",
        q => q.short(),
    }
}
