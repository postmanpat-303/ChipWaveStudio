//! Piano roll (lower panel, docs/TIMELINE.md "Piano roll"): edits the notes
//! of the selected timeline clip, or of the current block column.
//!
//! * Block clips / block columns (M3a): one note at a time per column on the
//!   row grid. Drawing writes note, instrument and volume (= velocity)
//!   cells and a note-off; everything else in the column stays as it was
//!   (`crate::timeline::roll`). Effect commands show as badges above the
//!   grid and are edited in the inspector.
//! * Note clips (M3b): polyphonic, free timing with a snap grid, velocity
//!   lane, quantise, chord stamp and the song key highlighted.
use super::*;
use crate::chords;
use crate::timeline::{self as tl, roll, Tick, PPQ, ROW};
use std::time::{Duration, Instant};

/// live-note keys for auditions (chord helper uses 200..)
const AUDITION_KEY: u8 = 230;
const KEYS_W: f32 = 46.0;
const TOOL_H: f32 = 26.0;
const FX_H: f32 = 16.0;
const VEL_H: f32 = 46.0;
const EDGE_PX: f32 = 5.0;

/// what the roll edits
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Block { pattern: usize, col: usize },
    Notes { data: usize, clip: Option<u32> },
}

/// a note as the roll shows it: `id` is the start row (blocks) or the index
/// into `NoteClipData::notes` (note clips)
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RNote {
    pub(crate) id: usize,
    pub(crate) start: Tick,
    pub(crate) len: Tick,
    pub(crate) key: u8,
    pub(crate) vel: f32,
    /// block notes: no volume set (plays at the instrument's level)
    pub(crate) no_vel: bool,
    pub(crate) instr: u8,
    pub(crate) fx: Option<(u8, u8)>,
}

/// the target's contents, read once per frame
#[derive(Clone)]
enum Source {
    Block(Pattern),
    Notes(tl::NoteClipData),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DragK {
    /// a new note being drawn: drag sets its length
    Draw,
    Move,
    Resize,
    /// rubber band from this screen point
    Band(Pos2),
}

#[derive(Clone)]
struct PrDrag {
    kind: DragK,
    origin: Source,
    /// the note grabbed / drawn (id in `origin`'s terms)
    id: usize,
    grab_tick: f64,
    grab_key: i32,
    start: Tick,
    key: u8,
    len: Tick,
    /// pitch auditioned last while moving
    heard: u8,
}

/// note-clip snap grids (ticks, label)
const GRIDS: [(Tick, &str); 6] = [
    (PPQ, "1/4"),
    (PPQ / 2, "1/8"),
    (ROW, "1/16"),
    (ROW / 2, "1/32"),
    (PPQ / 6, "1/16T"),
    (1, "off"),
];

pub(crate) struct PianoUi {
    /// the lower panel shows the piano roll
    pub(crate) open: bool,
    /// keys go to the roll (clicked into it last)
    pub(crate) focus: bool,
    /// pixels per row
    pub(crate) zoom: f32,
    pub(crate) key_h: f32,
    /// first visible tick, and pixels scrolled down from B-9
    pub(crate) scroll_x: f64,
    pub(crate) scroll_y: f32,
    /// selected notes (ids)
    pub(crate) sel: Vec<usize>,
    target: Option<Target>,
    /// scroll to the notes on the next frame
    fit: bool,
    drag: Option<PrDrag>,
    /// note-clip snap grid
    pub(crate) grid: Tick,
    /// draw whole chords (note clips)
    pub(crate) chord: Option<chords::ChordKind>,
    last_len: Tick,
    last_vel: f32,
    offs: Vec<(Instant, u8)>,
    /// screen rect of the note grid as last drawn (tests)
    pub(crate) grid_rect: Rect,
    pub(crate) vel_rect: Rect,
}

impl Default for PianoUi {
    fn default() -> Self {
        Self {
            open: false,
            focus: false,
            zoom: 16.0,
            key_h: 10.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            sel: vec![],
            target: None,
            fit: true,
            drag: None,
            grid: ROW,
            chord: None,
            last_len: ROW,
            last_vel: 1.0,
            offs: vec![],
            grid_rect: Rect::NOTHING,
            vel_rect: Rect::NOTHING,
        }
    }
}

impl PianoUi {
    fn ppt(&self) -> f64 {
        self.zoom as f64 / ROW as f64
    }
    pub(crate) fn x_of(&self, t: Tick) -> f32 {
        self.grid_rect.left() + ((t as f64 - self.scroll_x) * self.ppt()) as f32
    }
    pub(crate) fn y_of(&self, key: u8) -> f32 {
        self.grid_rect.top() + (120 - key as i32) as f32 * self.key_h - self.scroll_y
    }
    fn tick_at(&self, x: f32) -> f64 {
        self.scroll_x + ((x - self.grid_rect.left()) as f64 / self.ppt())
    }
    fn key_at(&self, y: f32) -> i32 {
        120 - ((y - self.grid_rect.top() + self.scroll_y) / self.key_h).floor() as i32
    }
}

fn source_notes(src: &Source) -> (Vec<RNote>, Tick) {
    match src {
        Source::Block(p) => (vec![], tl::rows_to_ticks(p.rows)),
        Source::Notes(d) => (
            d.notes
                .iter()
                .enumerate()
                .map(|(i, n)| RNote {
                    id: i,
                    start: n.start,
                    len: n.len,
                    key: n.key,
                    vel: roll::vel_f(n.vel),
                    no_vel: false,
                    instr: n.instr,
                    fx: n.fx,
                })
                .collect(),
            d.len.max(1),
        ),
    }
}

fn block_notes(p: &Pattern, col: usize) -> Vec<RNote> {
    roll::col_notes(p, col)
        .into_iter()
        .map(|n| {
            let c = p.cells[n.row][col];
            RNote {
                id: n.row,
                start: tl::rows_to_ticks(n.row),
                len: tl::rows_to_ticks(n.len),
                key: n.key,
                vel: roll::vol_to_vel(n.vol),
                no_vel: n.vol == 0,
                instr: n.instr,
                fx: (c.fx != 0 || c.fxp != 0).then_some((c.fx, c.fxp)),
            }
        })
        .collect()
}

/// all notes of the target (sorted as stored) and its length in ticks
fn notes_of(src: &Source, t: Target) -> (Vec<RNote>, Tick) {
    match (src, t) {
        (Source::Block(p), Target::Block { col, .. }) => {
            (block_notes(p, col), tl::rows_to_ticks(p.rows))
        }
        _ => source_notes(src),
    }
}

/// effect badges of a block column: (row tick, fx, param)
fn fx_badges(src: &Source, t: Target) -> Vec<(Tick, u8, u8)> {
    match (src, t) {
        (Source::Block(p), Target::Block { col, .. }) => p
            .cells
            .iter()
            .take(p.rows)
            .enumerate()
            .filter_map(|(r, row)| {
                let c = row.get(col)?;
                (c.fx != 0 || c.fxp != 0).then_some((tl::rows_to_ticks(r), c.fx, c.fxp))
            })
            .collect(),
        (Source::Notes(d), _) => d
            .notes
            .iter()
            .filter_map(|n| n.fx.map(|(a, b)| (n.start, a, b)))
            .collect(),
        _ => vec![],
    }
}

fn is_black(key: u8) -> bool {
    matches!((key as i32 - 1).rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

impl App {
    /// the roll's target: the selected timeline clip, else the current
    /// block column
    pub(super) fn pr_target(&self) -> Option<Target> {
        if self.view == View::Timeline {
            if let Some(id) = self.tlv.sel {
                let c = self.tlv.cache.as_ref()?;
                if let Some(cl) = c.tl.clips.iter().find(|x| x.id == id) {
                    return Some(match cl.kind {
                        tl::ClipKind::Block { pattern, col } => Target::Block {
                            pattern,
                            col: col as usize,
                        },
                        tl::ClipKind::Notes { data } => Target::Notes {
                            data: data as usize,
                            clip: Some(id),
                        },
                    });
                }
            }
        }
        Some(Target::Block {
            pattern: self.pattern,
            col: self.ch,
        })
    }

    fn pr_read(&self, t: Target) -> Option<Source> {
        self.with(|s| match t {
            Target::Block { pattern, col } => {
                let p = s.song.patterns.get(pattern)?;
                (col < s.song.channels.max(1)).then(|| Source::Block(p.clone()))
            }
            Target::Notes { data, .. } => s
                .song
                .timeline
                .as_deref()
                .and_then(|t| t.notes.get(data))
                .map(|d| Source::Notes((**d).clone())),
        })
    }

    /// write the edited source back (and refresh the timeline plan)
    fn pr_write(&mut self, t: Target, src: Source) {
        self.with(|s| {
            match (t, src) {
                (Target::Block { pattern, .. }, Source::Block(p)) => {
                    if let Some(dst) = s.song.patterns.get_mut(pattern) {
                        *dst = p;
                    }
                }
                (Target::Notes { data, .. }, Source::Notes(d)) => {
                    if let Some(x) = s
                        .song
                        .timeline
                        .as_deref_mut()
                        .and_then(|t| t.notes.get_mut(data))
                    {
                        *x.make_mut() = d;
                    }
                }
                _ => {}
            }
            let audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.dirty = true;
    }

    /// play `key` briefly (what a drawn or moved note sounds like)
    fn pr_audition(&mut self, key: u8, instr: u8, vel: f32) {
        let ch = match self.pr.target {
            Some(Target::Notes { clip: Some(id), .. }) => self
                .tlv
                .cache
                .as_ref()
                .and_then(|c| c.tl.clips.iter().find(|x| x.id == id))
                .map_or(self.ch, |c| c.track as usize),
            Some(Target::Block { col, .. }) => col,
            _ => self.ch,
        } as u8;
        let ins = if instr > 0 { instr } else { self.instr.max(1) };
        let old: Vec<u8> = self.pr.offs.drain(..).map(|(_, k)| k).collect();
        self.with(|s| {
            let audio::Shared { song, engine, .. } = s;
            if engine.playing {
                return;
            }
            for k in old {
                engine.live_event(song, crate::midi::LiveEv::Off { key: k });
            }
            engine.live_event(
                song,
                crate::midi::LiveEv::On {
                    key: AUDITION_KEY,
                    note: key,
                    vel: vel.clamp(0.1, 1.0),
                    instr: ins,
                    ch,
                },
            );
        });
        self.pr
            .offs
            .push((Instant::now() + Duration::from_millis(350), AUDITION_KEY));
    }

    /// release auditions that are due (every frame)
    pub(super) fn pr_release(&mut self, all: bool) {
        let now = Instant::now();
        if self.pr.offs.iter().all(|(t, _)| !all && *t > now) {
            return;
        }
        let due: Vec<u8> = self
            .pr
            .offs
            .iter()
            .filter(|(t, _)| all || *t <= now)
            .map(|x| x.1)
            .collect();
        self.pr.offs.retain(|(t, _)| !all && *t > now);
        self.with(|s| {
            let audio::Shared { song, engine, .. } = s;
            for k in due {
                engine.live_event(song, crate::midi::LiveEv::Off { key: k });
            }
        });
    }

    /// the piano roll is showing and has the keyboard
    pub(super) fn pr_active(&self) -> bool {
        self.pr.open && self.pr.focus && self.lower_visible() && !self.extra.lower_inst
    }

    /// snap grid in ticks for the current target
    fn pr_grid(&self) -> Tick {
        match self.pr.target {
            Some(Target::Notes { .. }) => self.pr.grid.max(1),
            _ => ROW,
        }
    }

    // ------------------------------------------------------------ edits

    /// Apply a drag from its origin: everything is recomputed from the
    /// state at the press, so a gesture is one clean edit.
    fn pr_apply_drag(&mut self, d: &PrDrag, t: Target, tick: f64, key: i32, len_ext: Tick) {
        let grid = self.pr_grid();
        let mut src = d.origin.clone();
        match (&mut src, t) {
            (Source::Block(p), Target::Block { col, .. }) => {
                let rows = p.rows.max(1);
                let row_of = |t: Tick| (t.max(0) / ROW).min(rows as Tick - 1) as usize;
                match d.kind {
                    DragK::Draw | DragK::Resize => {
                        let end = (tl::snap(tick as Tick, ROW).max(d.start + ROW))
                            .min(tl::rows_to_ticks(rows));
                        let len = ((end - d.start) / ROW).max(1) as usize;
                        if d.kind == DragK::Draw {
                            let ins = self.instr;
                            let vol = if self.pr.last_vel < 1.0 {
                                roll::vel_to_vol(self.pr.last_vel)
                            } else {
                                0
                            };
                            roll::put_note(p, col, row_of(d.start), len, d.key, ins, vol);
                        } else {
                            roll::resize_note(p, col, d.id, len);
                        }
                    }
                    DragK::Move => {
                        let dt = tl::snap((tick - d.grab_tick) as Tick, ROW);
                        let to = row_of(d.start + dt);
                        let k = (d.key as i32 + key - d.grab_key).clamp(1, 120) as u8;
                        roll::move_note(p, col, d.id, to, k);
                        self.pr.sel = vec![to];
                    }
                    DragK::Band(_) => return,
                }
            }
            (Source::Notes(n), Target::Notes { .. }) => match d.kind {
                DragK::Draw | DragK::Resize => {
                    let end = tl::snap(tick as Tick, grid).max(d.start + grid.min(len_ext.max(1)));
                    let len = (end - d.start).max(1);
                    if d.kind == DragK::Draw {
                        for &i in &self.pr.sel {
                            if let Some(x) = n.notes.get_mut(i) {
                                x.len = len;
                            }
                        }
                    } else if let Some(x) = n.notes.get(d.id).cloned() {
                        let dl = len - x.len;
                        for &i in &self.pr.sel {
                            if let Some(x) = n.notes.get_mut(i) {
                                x.len = (x.len + dl).max(1);
                            }
                        }
                    }
                }
                DragK::Move => {
                    let dt = tl::snap(d.start + (tick - d.grab_tick) as Tick, grid) - d.start;
                    let dk = key - d.grab_key;
                    let min_start = self
                        .pr
                        .sel
                        .iter()
                        .filter_map(|&i| n.notes.get(i))
                        .map(|x| x.start)
                        .min()
                        .unwrap_or(0);
                    let dt = dt.max(-min_start);
                    for &i in &self.pr.sel {
                        if let Some(x) = n.notes.get_mut(i) {
                            x.start += dt;
                            x.key = (x.key as i32 + dk).clamp(1, 120) as u8;
                        }
                    }
                }
                DragK::Band(_) => return,
            },
            _ => return,
        }
        self.pr_write(t, src);
    }

    /// delete the selected notes
    pub(super) fn pr_delete(&mut self) {
        let Some(t) = self.pr.target else { return };
        let Some(mut src) = self.pr_read(t) else {
            return;
        };
        if self.pr.sel.is_empty() {
            return;
        }
        self.snapshot();
        let n = self.pr.sel.len();
        match (&mut src, t) {
            (Source::Block(p), Target::Block { col, .. }) => {
                let mut rows = self.pr.sel.clone();
                rows.sort_unstable();
                for r in rows.into_iter().rev() {
                    roll::delete_note(p, col, r);
                }
            }
            (Source::Notes(d), _) => {
                let mut i = 0;
                d.notes.retain(|_| {
                    i += 1;
                    !self.pr.sel.contains(&(i - 1))
                });
            }
            _ => {}
        }
        self.pr.sel.clear();
        self.pr_write(t, src);
        self.status = format!("{n} note(s) deleted");
    }

    /// move the selection by `dt` ticks and `dk` semitones (keys)
    fn pr_nudge(&mut self, dt: Tick, dk: i32) {
        let Some(t) = self.pr.target else { return };
        let Some(mut src) = self.pr_read(t) else {
            return;
        };
        if self.pr.sel.is_empty() {
            return;
        }
        self.snapshot();
        match (&mut src, t) {
            (Source::Block(p), Target::Block { col, .. }) => {
                let rows = p.rows as i64;
                let mut sel = self.pr.sel.clone();
                // move in the direction of travel first, so notes don't collide
                sel.sort_unstable();
                if dt > 0 {
                    sel.reverse();
                }
                let mut out = vec![];
                for r in sel {
                    let Some(n) = roll::note_at(p, col, r).filter(|n| n.row == r) else {
                        continue;
                    };
                    let to = (r as i64 + dt / ROW).clamp(0, rows - 1) as usize;
                    let k = (n.key as i32 + dk).clamp(1, 120) as u8;
                    roll::move_note(p, col, r, to, k);
                    out.push(to);
                }
                self.pr.sel = out;
            }
            (Source::Notes(d), _) => {
                let idx = self.pr.sel.clone();
                roll::transpose(&mut d.notes, &idx, dk);
                let min = idx
                    .iter()
                    .filter_map(|&i| d.notes.get(i))
                    .map(|n| n.start)
                    .min()
                    .unwrap_or(0);
                let dt = dt.max(-min);
                for &i in &idx {
                    if let Some(n) = d.notes.get_mut(i) {
                        n.start += dt;
                    }
                }
            }
            _ => {}
        }
        self.pr_write(t, src);
    }

    /// Q: quantise the selection (or every note) of a note clip to the grid
    pub(super) fn pr_quantise(&mut self) {
        let Some(t @ Target::Notes { .. }) = self.pr.target else {
            self.status = "Block notes are always on the row grid".into();
            return;
        };
        let Some(Source::Notes(mut d)) = self.pr_read(t) else {
            return;
        };
        self.snapshot();
        let idx: Vec<usize> = if self.pr.sel.is_empty() {
            (0..d.notes.len()).collect()
        } else {
            self.pr.sel.clone()
        };
        let g = self.pr.grid.max(ROW / 2);
        roll::quantise(&mut d.notes, &idx, g, 1.0, false);
        self.status = format!("{} note(s) quantised to {}", idx.len(), grid_name(g));
        self.pr_write(t, Source::Notes(d));
    }

    /// keys while the roll has focus; true = used
    pub(super) fn pr_key(&mut self, key: Key, m: egui::Modifiers) -> bool {
        let grid = self.pr_grid();
        match key {
            Key::Delete | Key::Backspace => self.pr_delete(),
            Key::ArrowUp => self.pr_nudge(0, if m.shift { 12 } else { 1 }),
            Key::ArrowDown => self.pr_nudge(0, if m.shift { -12 } else { -1 }),
            Key::ArrowLeft if !m.command => self.pr_nudge(-grid, 0),
            Key::ArrowRight if !m.command => self.pr_nudge(grid, 0),
            Key::A if m.command => {
                let Some(t) = self.pr.target else {
                    return false;
                };
                if let Some(src) = self.pr_read(t) {
                    self.pr.sel = notes_of(&src, t).0.iter().map(|n| n.id).collect();
                }
            }
            Key::Q if !m.command => self.pr_quantise(),
            Key::Escape => {
                if self.pr.sel.is_empty() {
                    self.pr.focus = false;
                }
                self.pr.sel.clear();
            }
            Key::Plus | Key::Equals if !m.command => {
                self.pr.zoom = (self.pr.zoom * 1.25).min(120.0)
            }
            Key::Minus if !m.command => self.pr.zoom = (self.pr.zoom * 0.8).max(3.0),
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------ view

    /// the piano roll (lower panel)
    pub(super) fn piano_view(&mut self, ui: &mut egui::Ui) {
        let t = th();
        let full = ui.max_rect();
        let Some(target) = self.pr_target() else {
            return;
        };
        if self.pr.target != Some(target) {
            self.pr.target = Some(target);
            self.pr.sel.clear();
            self.pr.drag = None;
            self.pr.fit = true;
        }
        let Some(src) = self.pr_read(target) else {
            ui.label("Nothing to edit here");
            return;
        };
        let (notes, len) = notes_of(&src, target);
        let badges = fx_badges(&src, target);
        let grid_t = self.pr_grid();
        let (song_key, ninstr) = self.with(|s| (s.song.info.key, s.song.instruments.len()));
        // ---- layout
        let tool = Rect::from_min_size(full.min, egui::vec2(full.width(), TOOL_H));
        let fx_r = Rect::from_min_max(
            Pos2::new(full.left() + KEYS_W, tool.bottom()),
            Pos2::new(full.right(), tool.bottom() + FX_H),
        );
        let vel_r = Rect::from_min_max(
            Pos2::new(full.left() + KEYS_W, full.bottom() - VEL_H),
            full.max,
        );
        let grid_r = Rect::from_min_max(
            Pos2::new(full.left() + KEYS_W, fx_r.bottom()),
            Pos2::new(full.right(), vel_r.top() - 2.0),
        );
        let keys_r = Rect::from_min_max(
            Pos2::new(full.left(), grid_r.top()),
            Pos2::new(grid_r.left(), grid_r.bottom()),
        );
        self.pr.grid_rect = grid_r;
        self.pr.vel_rect = vel_r;
        // focus: a press inside takes the keys, a press elsewhere gives them back
        let (pressed, press_pos) =
            ui.input(|i| (i.pointer.any_pressed(), i.pointer.press_origin()));
        if pressed {
            self.pr.focus = press_pos.is_some_and(|p| full.contains(p) && !tool.contains(p));
        }
        if self.pr.fit && grid_r.height() > 10.0 {
            self.pr.fit = false;
            let (lo, hi) = notes
                .iter()
                .fold((120u8, 1u8), |(a, b), n| (a.min(n.key), b.max(n.key)));
            let mid = if notes.is_empty() {
                49.0
            } else {
                (lo as f32 + hi as f32) / 2.0
            };
            self.pr.scroll_y = (120.0 - mid) * self.pr.key_h - grid_r.height() / 2.0;
            self.pr.scroll_x = 0.0;
            let want = grid_r.width() / (len as f32 / ROW as f32).max(1.0);
            self.pr.zoom = want.clamp(4.0, 40.0);
        }
        // ---- toolbar (target, inspector)
        let sel_note = (self.pr.sel.len() == 1)
            .then(|| notes.iter().find(|n| n.id == self.pr.sel[0]).copied())
            .flatten();
        let mut tool_edit: Option<RNote> = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(tool.shrink2(egui::vec2(6.0, 1.0))), |ui| {
            ui.horizontal_centered(|ui| {
                let label = match target {
                    Target::Block { pattern, col } => {
                        let name = self.with(|s| s.song.patterns.get(pattern).map(|p| p.name.clone()).unwrap_or_default());
                        format!("Block {pattern:02} {name} · channel {}", col + 1)
                    }
                    Target::Notes { .. } => {
                        let name = match &src {
                            Source::Notes(d) => d.name.clone(),
                            _ => String::new(),
                        };
                        format!("Note clip {name} · {} notes", notes.len())
                    }
                };
                ui.label(egui::RichText::new(label).strong());
                if let Target::Notes { .. } = target {
                    ui.separator();
                    ui.label("Grid");
                    egui::ComboBox::from_id_salt("pr_grid")
                        .width(56.0)
                        .selected_text(grid_name(self.pr.grid))
                        .show_ui(ui, |ui| {
                            for (g, n) in GRIDS {
                                ui.selectable_value(&mut self.pr.grid, g, n);
                            }
                        });
                    ui.label("Chord");
                    egui::ComboBox::from_id_salt("pr_chord")
                        .width(64.0)
                        .selected_text(self.pr.chord.map_or("off", |c| c.name()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.pr.chord, None, "off");
                            for k in chords::ChordKind::ALL {
                                ui.selectable_value(&mut self.pr.chord, Some(k), k.name());
                            }
                        })
                        .response
                        .on_hover_text("draw the chord of the song key on the clicked note (Chords window sets the key)");
                    if ui.button("Quantise  Q").on_hover_text("snap the selected notes (or all) to the grid").clicked() {
                        self.pr_quantise();
                    }
                }
                ui.separator();
                match sel_note {
                    Some(n) => {
                        let mut e = n;
                        ui.label(egui::RichText::new(note_name(n.key)).monospace().color(t.note));
                        ui.label("Instr");
                        ui.add(egui::DragValue::new(&mut e.instr).range(0..=ninstr.min(255) as u8).hexadecimal(2, false, true))
                            .on_hover_text(if matches!(target, Target::Notes { .. }) { "00 = the track's instrument" } else { "00 = keep the channel's instrument" });
                        ui.label("Vel");
                        let mut v7 = (e.vel * 127.0).round() as u8;
                        if ui.add(egui::DragValue::new(&mut v7).range(0..=127)).changed() {
                            e.vel = v7 as f32 / 127.0;
                            e.no_vel = false;
                        }
                        let (mut fx, mut fxp) = e.fx.unwrap_or((0, 0));
                        ui.label("Fx");
                        let a = ui.add(egui::DragValue::new(&mut fx).range(0..=0x23).hexadecimal(2, false, true));
                        let b = ui.add(egui::DragValue::new(&mut fxp).hexadecimal(2, false, true));
                        if a.changed() || b.changed() {
                            e.fx = (fx != 0 || fxp != 0).then_some((fx, fxp));
                        }
                        if e != n {
                            tool_edit = Some(e);
                        }
                    }
                    None => {
                        let n = self.pr.sel.len();
                        let tip = if n > 1 {
                            format!("{n} notes selected · arrows move, Del deletes")
                        } else {
                            "click: draw · drag a note: move · its right edge: length · right-click: delete · Ctrl+click: select more".into()
                        };
                        ui.label(egui::RichText::new(tip).small().color(t.ink2));
                    }
                }
            });
        });
        if let (Some(e), Some(n)) = (tool_edit, sel_note) {
            self.pr_set_note(target, n, e);
        }
        let p = ui.painter_at(full);
        // ---- background: keys, scale, grid lines
        p.rect_filled(grid_r.union(vel_r).union(fx_r), 0.0, t.crt_bot);
        let vis_hi = self.pr.key_at(grid_r.top()).clamp(1, 120);
        let vis_lo = self.pr.key_at(grid_r.bottom()).clamp(1, 120);
        for k in vis_lo..=vis_hi {
            let k = k as u8;
            let y = self.pr.y_of(k);
            let row = Rect::from_min_max(
                Pos2::new(grid_r.left(), y),
                Pos2::new(grid_r.right(), y + self.pr.key_h),
            );
            let pc = chords::pc_of(k);
            let in_key = song_key.is_some_and(|sk| sk.contains(pc));
            let mut fill = if is_black(k) {
                t.crt_top.gamma_multiply(0.55)
            } else {
                t.crt_top.gamma_multiply(0.25)
            };
            if in_key {
                fill = t.accent.gamma_multiply(0.08);
            }
            p.rect_filled(row.intersect(grid_r), 0.0, fill);
            if pc == 0 {
                p.hline(
                    grid_r.x_range(),
                    y + self.pr.key_h,
                    Stroke::new(1.0, t.border.gamma_multiply(0.6)),
                );
            }
            // keyboard
            let kr = Rect::from_min_max(
                Pos2::new(keys_r.left(), y),
                Pos2::new(keys_r.right(), y + self.pr.key_h),
            )
            .intersect(keys_r);
            if kr.height() > 0.0 {
                let kc = if is_black(k) {
                    Color32::from_gray(40)
                } else {
                    Color32::from_gray(225)
                };
                p.rect_filled(kr.shrink2(egui::vec2(0.0, 0.5)), 1.0, kc);
                if pc == 0 || self.pr.key_h >= 13.0 {
                    p.text(
                        Pos2::new(kr.right() - 3.0, kr.center().y),
                        egui::Align2::RIGHT_CENTER,
                        note_name(k),
                        FontId::monospace(self.pr.key_h.clamp(7.0, 11.0)),
                        if is_black(k) {
                            Color32::from_gray(200)
                        } else {
                            Color32::from_gray(40)
                        },
                    );
                }
            }
        }
        // vertical lines: rows, beats, bars
        let first = (self.pr.scroll_x / ROW as f64).floor().max(0.0) as i64;
        let last = (self.pr.tick_at(grid_r.right()) / ROW as f64).ceil() as i64;
        let step = if self.pr.zoom < 6.0 { 4 } else { 1 };
        for r in first..=last {
            if r % step != 0 {
                continue;
            }
            let x = self.pr.x_of(r * ROW);
            let (w, c) = if r % 16 == 0 {
                (1.0, t.border)
            } else if r % 4 == 0 {
                (1.0, t.crt_grid)
            } else {
                (0.5, t.crt_grid.gamma_multiply(0.5))
            };
            p.vline(x, grid_r.y_range(), Stroke::new(w, c));
            p.vline(x, vel_r.y_range(), Stroke::new(0.5, c.gamma_multiply(0.6)));
            if r % 16 == 0 {
                p.text(
                    Pos2::new(x + 3.0, fx_r.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("{}", r / 16 + 1),
                    FontId::proportional(10.0),
                    t.ink2,
                );
            }
        }
        // past the end
        let ex = self.pr.x_of(len);
        if ex < grid_r.right() {
            let r = Rect::from_min_max(Pos2::new(ex.max(grid_r.left()), grid_r.top()), grid_r.max);
            p.rect_filled(r, 0.0, Color32::from_black_alpha(90));
            p.vline(ex, grid_r.y_range(), Stroke::new(1.5, t.ink2));
        }
        // ---- effect badges
        for &(tk, fx, fxp) in &badges {
            let x = self.pr.x_of(tk);
            if x < fx_r.left() - 30.0 || x > fx_r.right() {
                continue;
            }
            let txt = format!("{fx:02X}{fxp:02X}");
            let br = Rect::from_min_size(
                Pos2::new(x + 1.0, fx_r.top() + 1.0),
                egui::vec2(28.0, FX_H - 2.0),
            );
            p.rect_filled(br, 3.0, t.fx.gamma_multiply(0.35));
            p.text(
                br.center(),
                egui::Align2::CENTER_CENTER,
                txt,
                FontId::monospace(9.5),
                t.ink,
            );
        }
        // ---- notes
        let notes_p = ui.painter_at(grid_r);
        let base = match target {
            Target::Notes { clip: Some(id), .. } => self
                .tlv
                .cache
                .as_ref()
                .and_then(|c| c.tl.clips.iter().find(|x| x.id == id))
                .map_or(t.note, |c| super::timeline::track_colour(c.track as usize)),
            _ => t.note,
        };
        for n in &notes {
            let r = note_rect(&self.pr, n);
            if !r.intersects(grid_r) {
                continue;
            }
            let sel = self.pr.sel.contains(&n.id);
            let c = base.gamma_multiply(0.45 + 0.55 * n.vel);
            notes_p.rect_filled(r, 2.0, c);
            notes_p.rect_stroke(
                r,
                2.0,
                if sel {
                    Stroke::new(2.0, t.selection_outline)
                } else {
                    Stroke::new(1.0, Color32::from_black_alpha(120))
                },
                egui::StrokeKind::Inside,
            );
            if r.width() > 26.0 && self.pr.key_h >= 9.0 {
                notes_p.text(
                    r.left_center() + egui::vec2(3.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    note_name(n.key),
                    FontId::monospace(self.pr.key_h.clamp(7.0, 10.0)),
                    Color32::WHITE,
                );
            }
            if n.fx.is_some() {
                notes_p.circle_filled(Pos2::new(r.left() + 2.5, r.top() + 2.5), 2.0, t.fx);
            }
        }
        // ---- velocity lane
        for n in &notes {
            let x = self.pr.x_of(n.start);
            if x < vel_r.left() - 2.0 || x > vel_r.right() {
                continue;
            }
            let h = (vel_r.height() - 4.0) * n.vel;
            let sel = self.pr.sel.contains(&n.id);
            let c = if sel { t.selection_outline } else { base };
            let top = vel_r.bottom() - 2.0 - h;
            p.vline(
                x + 1.0,
                top..=vel_r.bottom() - 2.0,
                Stroke::new(2.0, if n.no_vel { c.gamma_multiply(0.4) } else { c }),
            );
            p.circle_filled(Pos2::new(x + 1.0, top), 2.5, c);
        }
        p.text(
            Pos2::new(full.left() + 4.0, vel_r.center().y),
            egui::Align2::LEFT_CENTER,
            "Vel",
            FontId::proportional(10.0),
            t.ink2,
        );
        // playhead (block loop or song in this block)
        let clip = match target {
            Target::Notes { clip: Some(id), .. } => self
                .tlv
                .cache
                .as_ref()
                .and_then(|c| c.tl.clips.iter().find(|x| x.id == id))
                .cloned(),
            _ => None,
        };
        let ph = self.with(|s| {
            let e = &s.engine;
            match target {
                Target::Notes { .. } => {
                    let c = clip.as_ref()?;
                    let t = e.tl_ticks()?;
                    let tk = t as Tick;
                    (tk >= c.start && tk < c.end())
                        .then(|| c.source_at(tk) as f64 + (t - tk as f64))
                }
                Target::Block { pattern, .. }
                    if e.playing && e.pattern == pattern && s.song.timeline.is_none() =>
                {
                    Some(tl::rows_to_ticks(e.row) as f64 + e.row_phase() as f64 * ROW as f64)
                }
                _ => None,
            }
        });
        if let Some(ph) = ph {
            let x = self.pr.x_of(0) + (ph * self.pr.ppt()) as f32;
            if grid_r.x_range().contains(x) {
                p.vline(x, grid_r.y_range(), Stroke::new(1.5, t.play));
            }
            ui.ctx().request_repaint();
        }
        if self.pr.focus {
            p.rect_stroke(
                full.shrink(1.0),
                3.0,
                Stroke::new(1.5, t.focus_ring),
                egui::StrokeKind::Inside,
            );
        }
        // ---- input: wheel
        let hover = ui.input(|i| i.pointer.hover_pos());
        if hover.is_some_and(|h| grid_r.union(vel_r).union(keys_r).contains(h)) {
            let (scroll, zoom_d, mods) =
                ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers));
            if zoom_d != 1.0 || (mods.command && scroll.y != 0.0) {
                let f = if zoom_d != 1.0 {
                    zoom_d
                } else {
                    (1.0 + scroll.y * 0.003).clamp(0.5, 2.0)
                };
                let hx = hover.map_or(grid_r.left(), |h| h.x);
                let at = self.pr.tick_at(hx);
                self.pr.zoom = (self.pr.zoom * f).clamp(3.0, 120.0);
                self.pr.scroll_x = (at - (hx - grid_r.left()) as f64 / self.pr.ppt()).max(0.0);
            } else {
                let (dx, dy) = if mods.shift {
                    (scroll.y + scroll.x, 0.0)
                } else {
                    (scroll.x, scroll.y)
                };
                self.pr.scroll_x = (self.pr.scroll_x - dx as f64 / self.pr.ppt()).max(0.0);
                let max_y = (120.0 * self.pr.key_h - grid_r.height()).max(0.0);
                self.pr.scroll_y = (self.pr.scroll_y - dy).clamp(0.0, max_y);
            }
        }
        // ---- input: grid
        let resp = ui.interact(
            grid_r,
            egui::Id::unique("piano_grid"),
            Sense::click_and_drag(),
        );
        let mods = ui.input(|i| i.modifiers);
        if let Some(h) = resp.hover_pos() {
            if let Some((_, true)) = hit_note(&self.pr, &notes, h) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
        if resp.secondary_clicked() {
            if let Some((n, _)) = resp
                .interact_pointer_pos()
                .and_then(|p| hit_note(&self.pr, &notes, p))
            {
                self.pr.sel = vec![n.id];
                self.pr_delete();
            }
        }
        if resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked() {
            let pos = press_pos
                .or(resp.interact_pointer_pos())
                .unwrap_or(grid_r.center());
            let tk = self.pr.tick_at(pos.x);
            let k = self.pr.key_at(pos.y).clamp(1, 120);
            let hit = hit_note(&self.pr, &notes, pos);
            let mut d = PrDrag {
                kind: DragK::Draw,
                origin: src.clone(),
                id: 0,
                grab_tick: tk,
                grab_key: k,
                start: 0,
                key: k as u8,
                len: 0,
                heard: 0,
            };
            match hit {
                Some((n, edge)) => {
                    if mods.command {
                        if let Some(i) = self.pr.sel.iter().position(|&x| x == n.id) {
                            if resp.clicked() {
                                self.pr.sel.remove(i);
                            }
                        } else {
                            self.pr.sel.push(n.id);
                        }
                    } else if !self.pr.sel.contains(&n.id) {
                        self.pr.sel = vec![n.id];
                    }
                    if !edge {
                        self.pr_audition(n.key, n.instr, n.vel);
                    }
                    d.kind = if edge { DragK::Resize } else { DragK::Move };
                    d.id = n.id;
                    d.start = n.start;
                    d.key = n.key;
                    d.len = n.len;
                    d.heard = n.key;
                    self.pr.last_len = n.len;
                    if !n.no_vel {
                        self.pr.last_vel = n.vel;
                    }
                }
                None if mods.shift || mods.command => {
                    d.kind = DragK::Band(pos);
                    if !mods.command {
                        self.pr.sel.clear();
                    }
                }
                None if tk >= 0.0 && (tk as Tick) < len => {
                    let start = (tk as Tick).div_euclid(grid_t) * grid_t;
                    d.start = start;
                    d.len = self.pr.last_len.max(grid_t);
                    let key = k as u8;
                    // write the new note(s) now; the drag sets the length
                    let mut s2 = src.clone();
                    match (&mut s2, target) {
                        (Source::Block(p), Target::Block { col, .. }) => {
                            let row = (start / ROW) as usize;
                            let ln = ((d.len + ROW - 1) / ROW).max(1) as usize;
                            let vol = if self.pr.last_vel < 1.0 {
                                roll::vel_to_vol(self.pr.last_vel)
                            } else {
                                0
                            };
                            roll::put_note(p, col, row, ln, key, self.instr, vol);
                            d.id = row;
                            self.pr.sel = vec![row];
                        }
                        (Source::Notes(nd), _) => {
                            let keys = self.pr_chord_keys(key, song_key);
                            self.pr.sel.clear();
                            for kk in keys {
                                nd.notes.push(tl::Note {
                                    start,
                                    len: d.len,
                                    key: kk,
                                    vel: roll::vel_u16(self.pr.last_vel),
                                    ..Default::default()
                                });
                                self.pr.sel.push(nd.notes.len() - 1);
                            }
                            d.id = nd.notes.len() - 1;
                        }
                        _ => {}
                    }
                    // the drag re-applies its length to this state
                    d.origin = s2.clone();
                    if let Source::Block(_) = s2 {
                        d.kind = DragK::Resize;
                    }
                    self.pr_write(target, s2);
                    self.pr_audition(key, 0, self.pr.last_vel);
                }
                None => self.pr.sel.clear(),
            }
            if resp.clicked() {
                self.pr.drag = None;
            } else {
                self.pr.drag = Some(d);
            }
        } else if resp.dragged() {
            if let (Some(d), Some(pos)) = (self.pr.drag.clone(), resp.interact_pointer_pos()) {
                let tk = self.pr.tick_at(pos.x);
                let k = self.pr.key_at(pos.y).clamp(1, 120);
                match d.kind {
                    DragK::Band(a) => {
                        let band = Rect::from_two_pos(a, pos);
                        ui.painter().rect_stroke(
                            band,
                            0.0,
                            Stroke::new(1.0, t.selection_outline),
                            egui::StrokeKind::Inside,
                        );
                        let ids: Vec<usize> = notes
                            .iter()
                            .filter(|n| note_rect(&self.pr, n).intersects(band))
                            .map(|n| n.id)
                            .collect();
                        for id in ids {
                            if !self.pr.sel.contains(&id) {
                                self.pr.sel.push(id);
                            }
                        }
                    }
                    DragK::Resize | DragK::Draw => {
                        self.pr_apply_drag(&d, target, tk, k, grid_t);
                        self.pr.last_len = (tl::snap(tk as Tick, grid_t) - d.start).max(grid_t);
                    }
                    DragK::Move => {
                        self.pr_apply_drag(&d, target, tk, k, grid_t);
                        // hear the new pitch when it changes
                        let nk = (d.key as i32 + k - d.grab_key).clamp(1, 120) as u8;
                        if self.pr.drag.as_ref().is_some_and(|x| x.heard != nk) {
                            if let Some(x) = self.pr.drag.as_mut() {
                                x.heard = nk;
                            }
                            self.pr_audition(nk, 0, self.pr.last_vel);
                        }
                    }
                }
            }
        }
        if resp.drag_stopped() {
            self.pr.drag = None;
        }
        // ---- input: velocity lane
        let vresp = ui.interact(
            vel_r,
            egui::Id::unique("piano_vel"),
            Sense::click_and_drag(),
        );
        if vresp.is_pointer_button_down_on() {
            if let Some(pos) = vresp.interact_pointer_pos() {
                let v = ((vel_r.bottom() - 2.0 - pos.y) / (vel_r.height() - 4.0)).clamp(0.0, 1.0);
                let ids: Vec<usize> = notes
                    .iter()
                    .filter(|n| (self.pr.x_of(n.start) + 1.0 - pos.x).abs() <= 5.0)
                    .filter(|n| {
                        self.pr.sel.is_empty()
                            || self.pr.sel.contains(&n.id)
                            || !notes
                                .iter()
                                .any(|m| self.pr.sel.contains(&m.id) && m.start == n.start)
                    })
                    .map(|n| n.id)
                    .collect();
                if !ids.is_empty() {
                    let mut s2 = self.pr_read(target).unwrap_or(src.clone());
                    for id in &ids {
                        set_vel(&mut s2, target, *id, v);
                    }
                    self.pr.last_vel = v;
                    self.pr_write(target, s2);
                    self.status = format!("Velocity {}", (v * 127.0).round());
                }
            }
        }
    }

    /// keys of what a click at `key` draws: the note, or with a chord type
    /// the song key's chord on that scale degree
    fn pr_chord_keys(&self, key: u8, song_key: Option<chords::Key>) -> Vec<u8> {
        let (Some(kind), Some(sk)) = (self.pr.chord, song_key) else {
            return vec![key];
        };
        let pc = chords::pc_of(key);
        let steps = sk.scale.steps();
        let Some(deg) = steps.iter().position(|&s| (sk.root + s) % 12 == pc) else {
            return vec![key];
        };
        let ch = chords::diatonic(sk, deg, kind);
        ch.iv
            .iter()
            .map(|&i| (key as i32 + i as i32).clamp(1, 120) as u8)
            .collect()
    }

    /// inspector edit of one note
    fn pr_set_note(&mut self, t: Target, old: RNote, e: RNote) {
        let Some(mut src) = self.pr_read(t) else {
            return;
        };
        match (&mut src, t) {
            (Source::Block(p), Target::Block { col, .. }) => {
                if let Some(c) = p.cells.get_mut(old.id).and_then(|r| r.get_mut(col)) {
                    c.instr = e.instr;
                    if e.vel != old.vel || e.no_vel != old.no_vel {
                        c.vol = roll::vel_to_vol(e.vel);
                    }
                    let (fx, fxp) = e.fx.unwrap_or((0, 0));
                    c.fx = fx;
                    c.fxp = fxp;
                }
            }
            (Source::Notes(d), _) => {
                if let Some(n) = d.notes.get_mut(old.id) {
                    n.instr = e.instr;
                    n.vel = roll::vel_u16(e.vel);
                    n.fx = e.fx;
                }
            }
            _ => return,
        }
        self.pr_write(t, src);
    }
}

fn note_rect(pr: &PianoUi, n: &RNote) -> Rect {
    let y = pr.y_of(n.key);
    let x0 = pr.x_of(n.start);
    Rect::from_min_max(
        Pos2::new(x0, y + 1.0),
        Pos2::new(pr.x_of(n.start + n.len).max(x0 + 3.0), y + pr.key_h - 1.0),
    )
}

/// the note under `p` (topmost) and whether `p` is on its right edge
fn hit_note(pr: &PianoUi, notes: &[RNote], p: Pos2) -> Option<(RNote, bool)> {
    notes.iter().rev().find_map(|n| {
        let r = note_rect(pr, n);
        r.contains(p)
            .then(|| (*n, p.x > r.right() - EDGE_PX.min(r.width() / 3.0)))
    })
}

fn set_vel(src: &mut Source, t: Target, id: usize, v: f32) {
    match (src, t) {
        (Source::Block(p), Target::Block { col, .. }) => {
            roll::set_note_vol(p, col, id, roll::vel_to_vol(v))
        }
        (Source::Notes(d), _) => {
            if let Some(n) = d.notes.get_mut(id) {
                n.vel = roll::vel_u16(v);
            }
        }
        _ => {}
    }
}

fn grid_name(g: Tick) -> &'static str {
    GRIDS.iter().find(|x| x.0 == g).map_or("?", |x| x.1)
}
