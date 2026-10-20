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
pub(super) enum Source {
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
    /// Pencil: painting a run of notes from this start, one per length
    Paint(Tick),
}

/// the roll's tools (toolbar; keys 1 2 3 while the roll has the keyboard)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum PrTool {
    /// smart: click empty space draws a note, drag a note moves it, its
    /// right edge resizes it
    #[default]
    Pointer,
    /// drag on empty space draws a selection box
    Select,
    /// like Pointer, but dragging on empty space paints a run of notes
    /// (each the remembered length, pitch following the mouse)
    Pencil,
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
    /// the selection when the drag began (block notes' ids are their rows,
    /// so a group move re-reads them from the original state)
    sel: Vec<usize>,
    /// the pointer has left the press point by half a grid step: a drawn
    /// note's length now follows it (until then it keeps the remembered
    /// length, so a slightly long or wobbly click doesn't shorten it)
    moved: bool,
}

/// A left press on empty grid held this long without moving turns into a
/// hand-grab pan (seconds)
pub(crate) const HOLD_S: f64 = 0.25;
/// ... and "without moving" means within this many pixels (egui's own
/// click / drag distance, so a quick drag starts exactly as before)
pub(crate) const HOLD_PX: f32 = 6.0;

/// What a left press on empty grid is so far (see [`press_decision`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Press {
    /// still deciding (held still, not long enough yet)
    Pending,
    /// released quickly without moving: a click (adds a note)
    Click,
    /// held still for [`HOLD_S`]: pan
    Hold,
    /// moved more than [`HOLD_PX`] before [`HOLD_S`]: the usual drag
    /// (draw / marquee / paint)
    Drag,
}

/// Click vs hold vs drag for a press `elapsed` seconds ago that has moved
/// `moved` pixels from where it went down; `down` = the button is still
/// held. Movement wins over time (a quick drag stays a drag).
pub(crate) fn press_decision(elapsed: f64, moved: f32, down: bool) -> Press {
    if moved > HOLD_PX {
        Press::Drag
    } else if elapsed >= HOLD_S {
        Press::Hold
    } else if !down {
        Press::Click
    } else {
        Press::Pending
    }
}

/// a deleted note going up in smoke (purely visual)
#[derive(Clone, Copy, Debug)]
pub(crate) struct Puff {
    pub(crate) rect: Rect,
    pub(crate) colour: Color32,
    pub(crate) born: Instant,
}

/// how long a puff of smoke lasts
const SMOKE_S: f32 = 0.5;

/// note-clip snap grids (ticks, label); 0 = Smart (follows the zoom)
const GRIDS: [(Tick, &str); 8] = [
    (4 * PPQ, "Bar"),
    (PPQ, "Beat"),
    (PPQ / 2, "1/8"),
    (ROW, "1/16"),
    (ROW / 2, "1/32"),
    (PPQ / 6, "1/16T"),
    (0, "Smart"),
    (1, "off"),
];

/// Smart snap: the finest of 1/32 … bar that is at least 12 px wide
fn smart_grid(zoom: f32) -> Tick {
    let ppt = zoom as f64 / ROW as f64;
    [ROW / 2, ROW, PPQ / 2, PPQ, 4 * PPQ]
        .into_iter()
        .find(|g| *g as f64 * ppt >= 12.0)
        .unwrap_or(4 * PPQ)
}

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
    pub(crate) target: Option<Target>,
    /// scroll to the notes on the next frame
    fit: bool,
    drag: Option<PrDrag>,
    /// note-clip snap grid
    pub(crate) grid: Tick,
    /// draw whole chords (note clips)
    pub(crate) chord: Option<chords::ChordKind>,
    pub(crate) tool: PrTool,
    /// the song key the Scale button turns back on
    last_key: Option<chords::Key>,
    /// length of the next drawn note: the last one drawn, resized or
    /// clicked (per session; starts at the grid step)
    pub(crate) last_len: Tick,
    last_vel: f32,
    /// right-drag eraser: the pointer's last position in this swipe
    erase: Option<Pos2>,
    /// this swipe has deleted something (its undo point is set), and how many
    erase_started: bool,
    erased: usize,
    /// notes going up in smoke
    pub(crate) smoke: Vec<Puff>,
    offs: Vec<(Instant, u8)>,
    /// screen rect of the note grid as last drawn (tests)
    pub(crate) grid_rect: Rect,
    pub(crate) vel_rect: Rect,
    /// a left press on empty grid that may become a pan: (where, when)
    /// (wall-clock: the app's frames and the test harness's steps alike)
    press: Option<(Pos2, Instant)>,
    /// panning (hand grab): the pointer's last position
    pub(crate) pan: Option<Pos2>,
    /// MIDI step input: (note data, tick in it) where the next key goes
    pub(crate) step: Option<(usize, Tick)>,
    /// keys went in at the step since the last full release (a chord)
    pub(crate) step_used: bool,
    /// Step input on (the header's toggle; off by default): MIDI and
    /// musical-typing keys go in one step after the other
    pub(crate) step_input: bool,
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
            tool: PrTool::Pointer,
            last_key: None,
            last_len: ROW,
            last_vel: 1.0,
            erase: None,
            erase_started: false,
            erased: 0,
            smoke: vec![],
            offs: vec![],
            grid_rect: Rect::NOTHING,
            vel_rect: Rect::NOTHING,
            press: None,
            pan: None,
            step: None,
            step_used: false,
            step_input: false,
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
pub(super) fn notes_of(src: &Source, t: Target) -> (Vec<RNote>, Tick) {
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
                        tl::ClipKind::Audio { .. } => return None,
                    });
                }
            }
            // timeline mode, nothing selected: the selected track's first
            // note or block clip, else nothing (an empty roll with a hint)
            if self.tlv.sel.is_none() && self.with(|s| s.song.timeline.is_some()) {
                let c = self.tlv.cache.as_ref()?;
                let ch = self.ch;
                return c
                    .tl
                    .clips
                    .iter()
                    .filter(|x| x.track as usize == ch)
                    .filter_map(|cl| {
                        let t = match cl.kind {
                            tl::ClipKind::Block { pattern, col } => Target::Block {
                                pattern,
                                col: col as usize,
                            },
                            tl::ClipKind::Notes { data } => Target::Notes {
                                data: data as usize,
                                clip: Some(cl.id),
                            },
                            tl::ClipKind::Audio { .. } => return None,
                        };
                        Some((cl.start, t))
                    })
                    .min_by_key(|(start, _)| *start)
                    .map(|(_, t)| t);
            }
        }
        // the Timeline in timeline mode: only its clips - never the
        // Tracker's cursor block (it isn't on the Timeline; its notes would
        // go nowhere you can see). The Tracker view keeps its block.
        if self.view == View::Timeline && self.with(|s| s.song.timeline.is_some()) {
            return None;
        }
        Some(Target::Block {
            pattern: self.pattern,
            col: self.ch,
        })
    }

    pub(super) fn pr_read(&self, t: Target) -> Option<Source> {
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
    pub(super) fn pr_write(&mut self, t: Target, src: Source) {
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

    /// play several keys at once for `ms` (chords from the chord lane)
    pub(super) fn pr_audition_keys(&mut self, keys: &[u8], ms: u64) {
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
        let ins = self.instr.max(1);
        let old: Vec<u8> = self.pr.offs.drain(..).map(|(_, k)| k).collect();
        self.with(|s| {
            let audio::Shared { song, engine, .. } = s;
            for k in old {
                engine.live_event(song, crate::midi::LiveEv::Off { key: k });
            }
            if engine.playing {
                return;
            }
            for (i, &note) in keys.iter().take(12).enumerate() {
                engine.live_event(
                    song,
                    crate::midi::LiveEv::On {
                        key: AUDITION_KEY + i as u8,
                        note,
                        vel: 0.7,
                        instr: ins,
                        ch,
                    },
                );
            }
        });
        let end = Instant::now() + Duration::from_millis(ms.clamp(80, 3000));
        for i in 0..keys.len().min(12) {
            self.pr.offs.push((end, AUDITION_KEY + i as u8));
        }
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

    pub(super) fn pr_grid_now(&self) -> Tick {
        self.pr_grid()
    }

    /// snap grid in ticks for the current target
    fn pr_grid(&self) -> Tick {
        match self.pr.target {
            Some(Target::Notes { .. }) if self.pr.grid == 0 => smart_grid(self.pr.zoom),
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
                        // the whole selection moves as a group (clamped)
                        let dt = tl::snap((tick - d.grab_tick) as Tick, ROW);
                        let drow = (row_of(d.start + dt) as i64) - (d.start / ROW);
                        let dk = key - d.grab_key;
                        let rows: Vec<usize> = if d.sel.contains(&d.id) {
                            d.sel.clone()
                        } else {
                            vec![d.id]
                        };
                        self.pr.sel = roll::move_notes(p, col, &rows, drow, dk);
                    }
                    DragK::Band(_) | DragK::Paint(_) => return,
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
                    // keep the chord shape: clamp the group, not each note
                    let (lo, hi) = self
                        .pr
                        .sel
                        .iter()
                        .filter_map(|&i| n.notes.get(i))
                        .fold((120i32, 1i32), |(a, b), x| {
                            (a.min(x.key as i32), b.max(x.key as i32))
                        });
                    let dk = if lo <= hi {
                        dk.clamp(1 - lo, 120 - hi)
                    } else {
                        dk
                    };
                    for &i in &self.pr.sel {
                        if let Some(x) = n.notes.get_mut(i) {
                            x.start += dt;
                            x.key = (x.key as i32 + dk).clamp(1, 120) as u8;
                        }
                    }
                }
                DragK::Band(_) | DragK::Paint(_) => return,
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

    /// write a new note (or the chord stamp) into `s2`; selects it (`add`:
    /// adds to the selection) and returns its id
    #[allow(clippy::too_many_arguments)]
    fn pr_put(
        &mut self,
        s2: &mut Source,
        target: Target,
        start: Tick,
        len: Tick,
        key: u8,
        song_key: Option<chords::Key>,
        add: bool,
    ) -> usize {
        if !add {
            self.pr.sel.clear();
        }
        match (s2, target) {
            (Source::Block(p), Target::Block { col, .. }) => {
                let row = (start / ROW) as usize;
                let ln = ((len + ROW - 1) / ROW).max(1) as usize;
                let vol = if self.pr.last_vel < 1.0 {
                    roll::vel_to_vol(self.pr.last_vel)
                } else {
                    0
                };
                roll::put_note(p, col, row, ln, key, self.instr, vol);
                if !self.pr.sel.contains(&row) {
                    self.pr.sel.push(row);
                }
                row
            }
            (Source::Notes(nd), _) => {
                for kk in self.pr_chord_keys(key, song_key) {
                    nd.notes.push(tl::Note {
                        start,
                        len,
                        key: kk,
                        vel: roll::vel_u16(self.pr.last_vel),
                        ..Default::default()
                    });
                    self.pr.sel.push(nd.notes.len() - 1);
                }
                nd.notes.len().saturating_sub(1)
            }
            _ => 0,
        }
    }

    /// the eraser: delete these notes at once (no undo boundary inside a
    /// swipe, so the whole swipe is one undo step) and send them up in smoke
    fn pr_erase(&mut self, t: Target, hit: &[RNote], colour: &dyn Fn(&RNote) -> Color32) {
        let Some(mut src) = self.pr_read(t) else {
            return;
        };
        let ids: Vec<usize> = hit.iter().map(|n| n.id).collect();
        match (&mut src, t) {
            (Source::Block(p), Target::Block { col, .. }) => {
                let mut rows = ids.clone();
                rows.sort_unstable();
                rows.dedup();
                for r in rows.into_iter().rev() {
                    roll::delete_note(p, col, r);
                }
            }
            (Source::Notes(d), _) => {
                let mut i = 0;
                d.notes.retain(|_| {
                    i += 1;
                    !ids.contains(&(i - 1))
                });
            }
            _ => return,
        }
        if !self.pr.erase_started {
            // the state before the swipe is the undo point
            self.snapshot();
            self.pr.erase_started = true;
            self.pr.erased = 0;
        }
        self.pr.erased += ids.len();
        self.pr.sel.clear();
        self.pr_write(t, src);
        self.status = format!("{} note(s) deleted", self.pr.erased);
        // the app's own setting, not the global (parallel tests flip that)
        if !self.extra.flat_ui {
            let born = Instant::now();
            for n in hit {
                self.pr.smoke.push(Puff {
                    rect: note_rect(&self.pr, n),
                    colour: colour(n),
                    born,
                });
            }
        }
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
        let g = self.pr_grid().max(ROW / 2);
        roll::quantise(&mut d.notes, &idx, g, 1.0, false);
        self.status = format!("{} note(s) quantised to {}", idx.len(), grid_name(g));
        self.pr_write(t, Source::Notes(d));
    }

    /// keys while the roll has focus; true = used
    pub(super) fn pr_key(&mut self, key: Key, m: egui::Modifiers) -> bool {
        let grid = self.pr_grid();
        match key {
            Key::Delete | Key::Backspace if self.pr.sel.is_empty() && self.cl.sel.is_some() => {
                self.cl_delete_sel()
            }
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
            Key::Num1 if !m.command => self.pr.tool = PrTool::Pointer,
            Key::Num2 if !m.command => self.pr.tool = PrTool::Select,
            Key::Num3 if !m.command => self.pr.tool = PrTool::Pencil,
            Key::F if !m.command => self.pr.fit = true,
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
            self.pr.target = None;
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Piano roll").strong().color(t.ink2));
                ui.label(
                    egui::RichText::new(
                        "Select a clip on the Timeline to edit its notes here · Shift+double-click \
                         empty space on a track makes a note clip",
                    )
                    .color(t.ink2),
                );
            });
            return;
        };
        if self.pr.target != Some(target) {
            self.pr.target = Some(target);
            self.pr.sel.clear();
            self.pr.drag = None;
            self.pr.fit = true;
            self.pr.smoke.clear();
            self.pr.erase = None;
            self.pr.erase_started = false;
            self.cl.sel = None;
            self.cl.pie = None;
            self.cl.list = None;
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
        // the chord lane (note clips, Chords on)
        let lane_h = if matches!(target, Target::Notes { .. }) && !self.cl.hidden {
            super::chordlane::LANE_H
        } else {
            0.0
        };
        let lane_r = Rect::from_min_max(
            Pos2::new(full.left() + KEYS_W, tool.bottom()),
            Pos2::new(full.right(), tool.bottom() + lane_h),
        );
        let fx_r = Rect::from_min_max(
            Pos2::new(full.left() + KEYS_W, lane_r.bottom()),
            Pos2::new(full.right(), lane_r.bottom() + FX_H),
        );
        // a pie or list menu is open: it has the mouse
        let modal = self.cl.pie.is_some() || self.cl.list.is_some();
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
        // ---- toolbar (target, tools, snap, key, inspector)
        let chip_col = match target {
            Target::Notes { clip: Some(id), .. } => self
                .tlv
                .cache
                .as_ref()
                .and_then(|c| c.tl.clips.iter().find(|x| x.id == id))
                .map_or(t.note, |c| super::timeline::track_colour(c.track as usize)),
            _ => t.note,
        };
        let sel_note = (self.pr.sel.len() == 1)
            .then(|| notes.iter().find(|n| n.id == self.pr.sel[0]).copied())
            .flatten();
        let mut tool_edit: Option<RNote> = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(tool.shrink2(egui::vec2(6.0, 1.0))), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // the clip / block chip: colour dot + name
                let (label, tip) = match target {
                    Target::Block { pattern, col } => {
                        let name = self.with(|s| s.song.patterns.get(pattern).map(|p| p.name.clone()).unwrap_or_default());
                        (
                            format!("Block {pattern:02} {name} · ch {}", col + 1),
                            format!("block {pattern:02} {name}, channel {}", col + 1),
                        )
                    }
                    Target::Notes { .. } => {
                        let name = match &src {
                            Source::Notes(d) => d.name.clone(),
                            _ => String::new(),
                        };
                        let name = if name.is_empty() { "Note clip".to_string() } else { name };
                        (name.clone(), format!("note clip {name} · {} notes", notes.len()))
                    }
                };
                pr_chip(ui, chip_col, &label).on_hover_text(tip);
                if pr_btn(ui, "ALL", false, true, "select every note (Ctrl+A)").clicked() {
                    self.pr.sel = notes.iter().map(|n| n.id).collect();
                    self.pr.focus = true;
                }
                ui.add_space(6.0);
                // tools
                for (tool, tip) in [
                    (PrTool::Pointer, "Pointer (1): click empty space to draw a note, drag a note to move it, its right edge to resize"),
                    (PrTool::Select, "Select (2): drag a box to select notes; Shift adds to the selection"),
                    (PrTool::Pencil, "Pencil (3): click to draw, drag to paint a run of notes at the note length"),
                ] {
                    if pr_tool_btn(ui, tool, self.pr.tool == tool, tip).clicked() {
                        self.pr.tool = tool;
                    }
                }
                ui.add_space(6.0);
                if let Target::Notes { .. } = target {
                    let shown = if self.pr.grid == 0 {
                        format!("Smart · {}", len_name(smart_grid(self.pr.zoom)))
                    } else {
                        grid_name(self.pr.grid).to_string()
                    };
                    egui::ComboBox::from_id_salt("pr_grid")
                        .width(92.0)
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for (g, n) in GRIDS {
                                let r = ui.selectable_value(&mut self.pr.grid, g, n);
                                match g {
                                    0 => r.on_hover_text("follows the zoom: finer as you zoom in"),
                                    x if x == PPQ => r.on_hover_text("a beat (1/4)"),
                                    _ => r,
                                };
                            }
                        })
                        .response
                        .on_hover_text("snap grid");
                    if pr_btn(ui, "Quantize", false, true, "snap the selected notes (or all) to the grid (Q)").clicked() {
                        self.pr_quantise();
                    }
                }
                if pr_btn(ui, "Fit", false, true, "zoom to the notes (F)").clicked() {
                    self.pr.fit = true;
                }
                if let Target::Notes { .. } = target {
                    let on = self.pr.step_input;
                    if pr_btn(ui, "Step input", on, true, "MIDI / typed notes go in one step after the other at the step position (chords: hold the keys together). Off: they only play - press Record (R) to record in time").clicked() {
                        self.pr.step_input = !on;
                        self.pr.step = None;
                        self.status = format!("Step input {}", if on { "off" } else { "on" });
                    }
                    let on = self.typing.on;
                    if pr_btn(ui, "Typing", on, true, "musical typing: play notes on the computer keyboard (Ctrl+K)").clicked() {
                        self.typing_toggle();
                    }
                }
                if let Target::Notes { .. } = target {
                    let on = !self.cl.hidden;
                    if pr_btn(ui, "Chords", on, true, "show the chord lane above the notes").clicked() {
                        self.cl.hidden = on;
                    }
                    if pr_btn(ui, "Generate", false, true, "Generate chord progression… (styles, key, length, colour)").clicked() {
                        let at = self.cl.sel.and_then(|i| self.cl_marks(target).get(i).map(|m| m.start)).unwrap_or(0);
                        self.cl_open_gen_at(at);
                    }
                    let mut k = self.pr.chord;
                    egui::ComboBox::from_id_salt("pr_chord")
                        .width(58.0)
                        .selected_text(k.map_or("Stamp", |c| c.name()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut k, None, "off");
                            for c in chords::ChordKind::ALL {
                                ui.selectable_value(&mut k, Some(c), c.name());
                            }
                        })
                        .response
                        .on_hover_text("chord stamp: a click draws the song key's chord on the clicked note");
                    if k != self.pr.chord {
                        if k.is_some() && song_key.is_none() {
                            self.status = "The chord stamp needs a song key: turn on Scale".into();
                        }
                        self.pr.chord = k;
                    }
                }
                // the song key: highlights its notes (and sets the Chords key)
                let on = song_key.is_some();
                if pr_btn(ui, "Scale", on, true, "highlight the song key's notes (the same key as the Chords window)").clicked() {
                    let k = if on { None } else { Some(self.pr.last_key.unwrap_or(chords::Key::new(0, chords::Scale::Major))) };
                    self.with(|s| s.song.info.key = k);
                    self.dirty = true;
                }
                if let Some(sk) = song_key {
                    let mut k = sk;
                    egui::ComboBox::from_id_salt("pr_key_root")
                        .width(40.0)
                        .selected_text(k.note_name(k.root))
                        .show_ui(ui, |ui| {
                            for r in 0..12u8 {
                                ui.selectable_value(&mut k.root, r, sk.note_name(r));
                            }
                        })
                        .response
                        .on_hover_text("key");
                    egui::ComboBox::from_id_salt("pr_key_scale")
                        .width(92.0)
                        .selected_text(k.scale.name())
                        .show_ui(ui, |ui| {
                            for sc in chords::Scale::ALL {
                                ui.selectable_value(&mut k.scale, sc, sc.name());
                            }
                        })
                        .response
                        .on_hover_text("scale");
                    if k != sk {
                        let k = chords::Key::new(k.root, k.scale);
                        self.with(|s| s.song.info.key = Some(k));
                        self.dirty = true;
                    }
                    self.pr.last_key = Some(sk);
                }
                ui.add_space(6.0);
                // the remembered length of the next drawn note
                ui.label(egui::RichText::new("Len").color(t.ink2));
                let gstep = self.pr_grid();
                egui::ComboBox::from_id_salt("pr_len")
                    .width(52.0)
                    .selected_text(len_name(self.pr.last_len.max(gstep)))
                    .show_ui(ui, |ui| {
                        for l in [ROW / 2, ROW, PPQ / 2, 3 * ROW, PPQ, 2 * PPQ, 4 * PPQ] {
                            if l >= gstep {
                                ui.selectable_value(&mut self.pr.last_len, l, len_name(l));
                            }
                        }
                    })
                    .response
                    .on_hover_text("length of the next note you click in: the last note drawn, resized or clicked");
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
                            "click: draw (same length as the last note) · drag a note: move · its right edge: length · right-click or right-drag: erase · Ctrl+click: select more".into()
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
                .and_then(|c| {
                    let x = c.tl.clips.iter().find(|x| x.id == id)?;
                    Some(super::timeline::track_colour_in(&c.tl, x.track as usize))
                })
                .unwrap_or(t.note),
            _ => t.note,
        };
        // a group being moved: ghosts where the notes were
        if let Some(d) = self.pr.drag.as_ref().filter(|d| d.kind == DragK::Move) {
            let ids: &[usize] = if d.sel.contains(&d.id) {
                &d.sel
            } else {
                std::slice::from_ref(&d.id)
            };
            for n in notes_of(&d.origin, target)
                .0
                .iter()
                .filter(|n| ids.contains(&n.id))
            {
                let r = note_rect(&self.pr, n);
                if r.intersects(grid_r) {
                    notes_p.rect_filled(r, 2.0, sel_note_colours(&t, n.vel).0.gamma_multiply(0.18));
                    notes_p.rect_stroke(
                        r,
                        2.0,
                        Stroke::new(1.0, sel_note_colours(&t, 1.0).1.gamma_multiply(0.5)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
        let hover_at = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|h| grid_r.contains(*h));
        let hovered = hover_at
            .and_then(|h| hit_note(&self.pr, &notes, h))
            .map(|(n, _)| n.id);
        for n in &notes {
            let r = note_rect(&self.pr, n);
            if !r.intersects(grid_r) {
                continue;
            }
            let sel = self.pr.sel.contains(&n.id);
            let hot = hovered == Some(n.id);
            let (fill, edge, ink) = if sel {
                sel_note_colours(&t, n.vel)
            } else if t.ember {
                // Slate: light grey notes, selected ones orange with a glow
                (
                    crate::theme::mix(
                        crate::theme::hex(0x7E8187),
                        crate::ember::NOTE,
                        0.4 + 0.6 * n.vel,
                    ),
                    Color32::from_black_alpha(120),
                    crate::theme::hex(0x1B1D20),
                )
            } else {
                (
                    base.gamma_multiply(0.45 + 0.55 * n.vel),
                    Color32::from_black_alpha(120),
                    Color32::WHITE,
                )
            };
            if sel && t.ember {
                crate::ember::glow_rect(&notes_p, r, 2, t.accent);
            }
            // hover: a slight lighten
            let fill = if hot {
                crate::theme::mix(fill, Color32::WHITE, 0.18)
            } else {
                fill
            };
            notes_p.rect_filled(r, 2.0, fill);
            notes_p.rect_stroke(
                r,
                2.0,
                Stroke::new(if sel { 1.5 } else { 1.0 }, edge),
                egui::StrokeKind::Inside,
            );
            if r.width() > 26.0 && self.pr.key_h >= 9.0 {
                notes_p.text(
                    r.left_center() + egui::vec2(3.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    note_name(n.key),
                    FontId::monospace(self.pr.key_h.clamp(7.0, 10.0)),
                    ink,
                );
            }
            if n.fx.is_some() {
                notes_p.circle_filled(Pos2::new(r.left() + 2.5, r.top() + 2.5), 2.0, t.fx);
            }
        }
        // ---- deleted notes going up in smoke
        let now = Instant::now();
        self.pr
            .smoke
            .retain(|p| now.duration_since(p.born).as_secs_f32() < SMOKE_S);
        for puff in &self.pr.smoke {
            paint_smoke(
                &notes_p,
                puff,
                now.duration_since(puff.born).as_secs_f32() / SMOKE_S,
            );
        }
        if !self.pr.smoke.is_empty() {
            ui.ctx().request_repaint();
        }
        // ---- velocity lane
        for n in &notes {
            let x = self.pr.x_of(n.start);
            if x < vel_r.left() - 2.0 || x > vel_r.right() {
                continue;
            }
            let h = (vel_r.height() - 4.0) * n.vel;
            let sel = self.pr.sel.contains(&n.id);
            let c = if sel {
                sel_note_colours(&t, 1.0).0
            } else {
                base
            };
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
        // ---- chord lane (draws and takes its own clicks)
        if lane_h > 0.0 {
            self.chord_lane(ui, lane_r, target, grid_t, len, modal);
        }
        // ---- input: grid
        let resp = ui.interact(
            grid_r,
            egui::Id::unique("piano_grid"),
            if modal {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            },
        );
        let mods = ui.input(|i| i.modifiers);
        if let Some(h) = resp.hover_pos() {
            if let Some((_, true)) = hit_note(&self.pr, &notes, h) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
        // ---- right-click deletes; right-drag erases every note it passes
        let (sec_down, sec_pressed, latest) = ui.input(|i| {
            (
                i.pointer.secondary_down(),
                i.pointer.button_pressed(egui::PointerButton::Secondary),
                i.pointer.latest_pos(),
            )
        });
        if sec_pressed && !modal && press_pos.is_some_and(|p| grid_r.contains(p)) {
            self.pr.erase = press_pos;
        }
        if let (Some(from), true) = (self.pr.erase, sec_down || sec_pressed) {
            let to = latest.unwrap_or(from);
            let hit = swept_notes(&self.pr, &notes, from, to);
            if !hit.is_empty() {
                let colour = |n: &RNote| base.gamma_multiply(0.45 + 0.55 * n.vel);
                self.pr_erase(target, &hit, &colour);
            }
            self.pr.erase = Some(to);
            // the eraser cursor
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
            paint_eraser(ui.ctx(), to);
        } else if self.pr.erase.take().is_some() {
            self.pr.erase_started = false;
            ui.ctx().request_repaint();
        }
        // ---- hold-to-pan (left press held still on empty grid), middle-drag
        let now = Instant::now();
        let (prim_down, mid_down, prim_pressed, mid_pressed) = ui.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.middle_down(),
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_pressed(egui::PointerButton::Middle),
            )
        });
        if !modal {
            if let Some(p) = press_pos.filter(|p| grid_r.contains(*p)) {
                if prim_pressed
                    && !(mods.shift || mods.command)
                    && hit_note(&self.pr, &notes, p).is_none()
                {
                    self.pr.press = Some((p, now));
                }
                if mid_pressed {
                    self.pr.pan = Some(p);
                }
            }
        }
        if let (Some((p0, t0)), None) = (self.pr.press, self.pr.pan) {
            let moved = latest.map_or(0.0, |l| l.distance(p0));
            match press_decision(now.duration_since(t0).as_secs_f64(), moved, prim_down) {
                Press::Hold => self.pr.pan = Some(latest.unwrap_or(p0)),
                Press::Pending => ui.ctx().request_repaint(),
                Press::Click | Press::Drag => self.pr.press = None,
            }
        }
        let panning = self.pr.pan.is_some();
        if let Some(last) = self.pr.pan {
            if prim_down || mid_down {
                let to = latest.unwrap_or(last);
                let d = to - last;
                self.pr_pan_by(d, grid_r, len);
                self.pr.pan = Some(to);
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else {
                self.pr.pan = None;
                self.pr.press = None;
            }
        }
        if panning {
            // the pan owns this press: no note, no box, no draw
        } else if resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked() {
            self.pr.press = None;
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
                sel: vec![],
                moved: false,
            };
            match hit {
                Some((n, edge)) => {
                    if mods.shift || mods.command {
                        // Shift (or Ctrl) + click toggles it in the selection
                        if let Some(i) = self.pr.sel.iter().position(|&x| x == n.id) {
                            if resp.clicked() {
                                self.pr.sel.remove(i);
                            }
                        } else {
                            self.pr.sel.push(n.id);
                        }
                    } else if resp.clicked() || !self.pr.sel.contains(&n.id) {
                        // a plain click selects only this note; dragging a
                        // selected note moves the whole selection
                        self.pr.sel = vec![n.id];
                    }
                    if !edge {
                        self.pr_audition(n.key, n.instr, n.vel);
                    }
                    d.kind = if edge { DragK::Resize } else { DragK::Move };
                    d.sel = self.pr.sel.clone();
                    d.id = n.id;
                    d.start = n.start;
                    d.key = n.key;
                    d.len = n.len;
                    d.heard = n.key;
                    d.moved = true;
                    self.pr.last_len = n.len;
                    if !n.no_vel {
                        self.pr.last_vel = n.vel;
                    }
                }
                // Shift (or Ctrl) on empty space: a box that adds to the
                // selection (never a new note); the Select tool: a new box
                None if mods.shift || mods.command || self.pr.tool == PrTool::Select => {
                    d.kind = DragK::Band(pos);
                    if !(mods.shift || mods.command) {
                        self.pr.sel.clear();
                    }
                }
                None if tk >= 0.0 && (tk as Tick) < len => {
                    let start = (tk as Tick).div_euclid(grid_t) * grid_t;
                    d.start = start;
                    d.len = self.pr.last_len.max(grid_t);
                    let key = k as u8;
                    // a chord on a Tracker clip: it becomes a note clip first
                    let (target, mut s2) = self
                        .pr_make_room(target, &src, start, key, song_key)
                        .unwrap_or_else(|| (target, src.clone()));
                    // write the new note(s) now; the drag sets the length
                    d.id = self.pr_put(&mut s2, target, start, d.len, key, song_key, false);
                    // the drag re-applies its length to this state
                    d.origin = s2.clone();
                    if self.pr.tool == PrTool::Pencil {
                        d.kind = DragK::Paint(start);
                    } else if let Source::Block(_) = s2 {
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
        } else if resp.dragged_by(egui::PointerButton::Primary) {
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
                        // a new note keeps the remembered length until the
                        // pointer really moves (half a grid step)
                        let go = d.moved || (tk - d.grab_tick).abs() >= grid_t as f64 / 2.0;
                        if go {
                            if let Some(x) = self.pr.drag.as_mut() {
                                x.moved = true;
                            }
                            self.pr_apply_drag(&d, target, tk, k, grid_t);
                            self.pr.last_len = (tl::snap(tk as Tick, grid_t) - d.start).max(grid_t);
                        }
                    }
                    DragK::Paint(from) => {
                        // one more note each time the mouse reaches the next
                        // length step (either way), at the mouse's pitch
                        let step = d.len.max(1);
                        let at = from + ((tk as Tick - from).div_euclid(step)) * step;
                        if at != d.start
                            && at >= 0
                            && at < len
                            && !notes.iter().any(|n| n.start == at && n.key == k as u8)
                        {
                            if let Some(s2) = self.pr_read(target) {
                                let (target, mut s2) = self
                                    .pr_make_room(target, &s2, at, k as u8, song_key)
                                    .unwrap_or((target, s2));
                                let id =
                                    self.pr_put(&mut s2, target, at, step, k as u8, song_key, true);
                                self.pr_write(target, s2);
                                self.pr_audition(k as u8, 0, self.pr.last_vel);
                                if let Some(x) = self.pr.drag.as_mut() {
                                    x.start = at;
                                    x.id = id;
                                }
                            }
                        }
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
            if modal {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            },
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
        // pies, list menu, Generate dialog
        if let Target::Notes { .. } = target {
            self.chord_popups(ui.ctx(), target);
        }
    }

    /// Scroll the roll by a pointer move `d` (the grid follows the hand),
    /// kept inside the block / clip and the key range.
    fn pr_pan_by(&mut self, d: egui::Vec2, grid_r: Rect, len: Tick) {
        let ppt = self.pr.ppt();
        // up to half a screen past the end, never before the start
        let view = grid_r.width() as f64 / ppt;
        let max_x = (len as f64 - view).max(0.0) + view / 2.0;
        self.pr.scroll_x =
            (self.pr.scroll_x - d.x as f64 / ppt).clamp(0.0, max_x.max(self.pr.scroll_x));
        let max_y = (120.0 * self.pr.key_h - grid_r.height()).max(0.0);
        self.pr.scroll_y = (self.pr.scroll_y - d.y).clamp(0.0, max_y);
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

    /// Chords on a Tracker clip: a Tracker column holds one note at a time,
    /// so a new note at `start` where another one starts (or a chord from
    /// the chord helper) first turns the clip - and
    /// its linked copies - into a note clip (one undo step with the note).
    /// Returns the new target and its contents; None: draw as before.
    fn pr_make_room(
        &mut self,
        target: Target,
        src: &Source,
        start: Tick,
        key: u8,
        song_key: Option<chords::Key>,
    ) -> Option<(Target, Source)> {
        let Target::Block { pattern, col } = target else {
            return None;
        };
        if self.view != View::Timeline {
            return None;
        }
        // the timeline clip the roll shows
        let id = {
            let c = self.tlv.cache.as_ref()?;
            let plays = |cl: &tl::Clip| cl.block() == Some((pattern, col));
            self.tlv
                .sel
                .filter(|id| c.tl.clips.iter().any(|x| x.id == *id && plays(x)))
                .or_else(|| c.tl.clips.iter().find(|x| plays(x)).map(|x| x.id))?
        };
        // a note already starting there (a later note simply ends the one
        // before, as in the Tracker), or the chord helper
        let (notes, _) = notes_of(src, target);
        let chord = self.pr_chord_keys(key, song_key).len() > 1
            || notes.iter().any(|n| n.start == start && n.key != key);
        if !chord {
            return None;
        }
        let data = self.with(|s| tl::block_col_to_notes(&mut s.song, pattern, col));
        let Some(data) = data else {
            self.status = "This Tracker clip has volumes or effects between its notes, so it \
                           holds one note at a time; draw chords in a note clip (double-click \
                           empty space)"
                .into();
            return None;
        };
        let t = Target::Notes {
            data,
            clip: Some(id),
        };
        let src = self.pr_read(t)?;
        // keep the roll's state (selection, the press in progress)
        self.pr.target = Some(t);
        self.dirty = true;
        self.status = "Chord: this Tracker clip is a note clip now (Ctrl+Z puts it back)".into();
        Some((t, src))
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

/// Selected notes: an orange fill (darker for quiet notes, so velocity
/// still reads), a lighter outline and dark text. Theme-aware: Amiga's
/// light chrome gets a deeper orange.
pub(crate) fn sel_note_colours(t: &crate::theme::Theme, vel: f32) -> (Color32, Color32, Color32) {
    if t.ember {
        // acc-hot with a white rim (plus a bloom where they are drawn)
        let fill = crate::theme::mix(t.accent, t.acc_hot, 0.35 + 0.65 * vel.clamp(0.0, 1.0));
        return (fill, Color32::from_rgb(0xFF, 0xF1, 0xD6), Color32::from_rgb(0x20, 0x14, 0x08));
    }
    let (lo, hi, edge) = if t.light {
        (
            Color32::from_rgb(0xA8, 0x4C, 0x0C),
            Color32::from_rgb(0xF5, 0x82, 0x1F),
            Color32::from_rgb(0xFF, 0xE2, 0xBC),
        )
    } else {
        (
            Color32::from_rgb(0xB0, 0x5A, 0x14),
            Color32::from_rgb(0xFF, 0xA2, 0x3A),
            Color32::from_rgb(0xFF, 0xE0, 0xB4),
        )
    };
    let fill = crate::theme::mix(lo, hi, vel.clamp(0.0, 1.0));
    let ink = if crate::theme::luma(fill) > 0.5 {
        Color32::from_rgb(0x20, 0x14, 0x08)
    } else {
        Color32::WHITE
    };
    (fill, edge, ink)
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

/// the notes the segment `a`→`b` passes over (sampled every 2 px)
fn swept_notes(pr: &PianoUi, notes: &[RNote], a: Pos2, b: Pos2) -> Vec<RNote> {
    let steps = ((b - a).length() / 2.0).ceil().max(1.0) as usize;
    let mut out: Vec<RNote> = vec![];
    for k in 0..=steps {
        let p = a + (b - a) * (k as f32 / steps as f32);
        for n in notes {
            if note_rect(pr, n).expand(1.0).contains(p) && !out.iter().any(|m| m.id == n.id) {
                out.push(*n);
            }
        }
    }
    out
}

/// a small integer hash -> 0..1
fn h01(a: u32, b: u32, c: u32) -> f32 {
    let mut x =
        a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// a deleted note at `age` (0..1 of its life): it breaks into square pixels
/// of its colour that drift up and sideways and fade out
fn paint_smoke(p: &egui::Painter, puff: &Puff, age: f32) {
    let r = puff.rect;
    let px = (r.height() / 2.0).clamp(2.0, 4.0);
    let cols = ((r.width() / px).ceil() as usize).clamp(1, 64);
    let rows = ((r.height() / px).ceil() as usize).clamp(1, 4);
    let (cw, ch) = (r.width() / cols as f32, r.height() / rows as f32);
    let seed = (r.left() as i32 as u32) ^ ((r.top() as i32 as u32) << 11);
    for i in 0..cols {
        for j in 0..rows {
            let (iu, ju) = (i as u32, j as u32);
            // each pixel has its own life: the note dissolves, not blinks
            let life = 0.45 + 0.55 * h01(iu, ju, seed);
            if age >= life {
                continue;
            }
            let a = age / life;
            let c0 = Pos2::new(
                r.left() + (i as f32 + 0.5) * cw,
                r.top() + (j as f32 + 0.5) * ch,
            );
            let drift = egui::vec2(
                (h01(iu, ju, seed ^ 0x51) - 0.5) * 56.0 * age,
                -(10.0 + 34.0 * h01(iu, ju, seed ^ 0xA7)) * age - 14.0 * age * age,
            );
            let sz = px * (1.0 - 0.45 * a);
            let alpha = (1.0 - a).powf(1.3);
            p.rect_filled(
                Rect::from_center_size(c0 + drift, egui::vec2(sz, sz)),
                0.0,
                puff.colour.gamma_multiply(alpha),
            );
        }
    }
}

/// the eraser cursor (tip at `at`) while right-dragging in the roll
fn paint_eraser(ctx: &egui::Context, at: Pos2) {
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::unique("pr_eraser"),
    ));
    // a tilted block eraser: pink rubber at the tip, a pale sleeve behind
    let (u, v) = (egui::vec2(0.82, -0.57), egui::vec2(0.57, 0.82));
    let quad = |a: f32, b: f32| {
        vec![
            at + u * a - v * 4.5,
            at + u * b - v * 4.5,
            at + u * b + v * 4.5,
            at + u * a + v * 4.5,
        ]
    };
    let edge = Stroke::new(1.0, Color32::from_black_alpha(200));
    p.add(egui::Shape::convex_polygon(
        quad(0.0, 8.0),
        Color32::from_rgb(0xF2, 0x8C, 0xA8),
        edge,
    ));
    p.add(egui::Shape::convex_polygon(
        quad(8.0, 18.0),
        Color32::from_rgb(0xE8, 0xEC, 0xF2),
        edge,
    ));
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

/// a note length as a fraction of a bar ("1/4", "3/16", "1 bar", "2 bars")
pub(crate) fn len_name(l: Tick) -> String {
    let bar = 4 * PPQ;
    if l > 0 && l % bar == 0 {
        let n = l / bar;
        return if n == 1 {
            "1 bar".into()
        } else {
            format!("{n} bars")
        };
    }
    fn gcd(a: Tick, b: Tick) -> Tick {
        if b == 0 {
            a
        } else {
            gcd(b, a % b)
        }
    }
    let g = gcd(l.max(1), bar);
    format!("{}/{}", l.max(1) / g, bar / g)
}

const BTN_H: f32 = 20.0;

/// a toolbar text button with a highlighted active state
fn pr_btn(ui: &mut egui::Ui, label: &str, on: bool, enabled: bool, tip: &str) -> egui::Response {
    let t = th();
    let font = FontId::proportional(12.0);
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
    let w = (g.size().x + 16.0).max(30.0);
    let (r, resp) = ui.allocate_exact_size(
        egui::vec2(w, BTN_H),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if ui.is_rect_visible(r) {
        btn_back(ui.painter(), r, &resp, on);
        let col = if !enabled {
            t.ink2.gamma_multiply(0.5)
        } else if on {
            if t.light {
                t.ink
            } else {
                Color32::WHITE
            }
        } else {
            t.ink
        };
        ui.painter().galley(r.center() - g.size() / 2.0, g, col);
    }
    let name = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &name));
    resp.on_hover_text(tip)
}

fn btn_back(p: &egui::Painter, r: Rect, resp: &egui::Response, on: bool) {
    let t = th();
    let fill = if on {
        t.sel
    } else if resp.hovered() {
        Color32::from_white_alpha(18)
    } else {
        Color32::from_black_alpha(50)
    };
    p.rect_filled(r, 4.0, fill);
    p.rect_stroke(
        r,
        4.0,
        Stroke::new(1.0, if on { t.accent } else { t.border }),
        egui::StrokeKind::Inside,
    );
    theme::raise(p, r, 4.0, on || resp.is_pointer_button_down_on());
}

/// a tool button with a painted icon
fn pr_tool_btn(ui: &mut egui::Ui, tool: PrTool, on: bool, tip: &str) -> egui::Response {
    let t = th();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(26.0, BTN_H), Sense::click());
    if ui.is_rect_visible(r) {
        let p = ui.painter();
        btn_back(p, r, &resp, on);
        let col = if on && !t.light {
            Color32::WHITE
        } else {
            t.ink
        };
        let c = r.center();
        let v = egui::vec2;
        match tool {
            PrTool::Pointer => {
                // an arrow
                let pts = vec![
                    c + v(-4.0, -7.0),
                    c + v(-4.0, 5.0),
                    c + v(-1.0, 2.0),
                    c + v(1.5, 7.0),
                    c + v(3.5, 6.0),
                    c + v(1.0, 1.0),
                    c + v(5.0, 1.0),
                ];
                p.add(egui::Shape::closed_line(pts, Stroke::new(1.4, col)));
            }
            PrTool::Select => {
                // a dashed box
                let b = Rect::from_center_size(c, v(13.0, 11.0));
                let corners = [
                    b.left_top(),
                    b.right_top(),
                    b.right_bottom(),
                    b.left_bottom(),
                ];
                for k in 0..4 {
                    let (a, z) = (corners[k], corners[(k + 1) % 4]);
                    p.extend(egui::Shape::dashed_line(
                        &[a, z],
                        Stroke::new(1.3, col),
                        2.5,
                        2.0,
                    ));
                }
            }
            PrTool::Pencil => {
                // a pencil, point down-left
                let (u, w) = (v(0.707, -0.707), v(0.707, 0.707));
                let tip = c + v(-6.0, 6.0);
                let body = |a: f32, b: f32| {
                    vec![
                        tip + u * a - w * 2.3,
                        tip + u * b - w * 2.3,
                        tip + u * b + w * 2.3,
                        tip + u * a + w * 2.3,
                    ]
                };
                p.add(egui::Shape::closed_line(
                    body(4.0, 15.0),
                    Stroke::new(1.3, col),
                ));
                p.add(egui::Shape::convex_polygon(
                    vec![tip, tip + u * 4.0 - w * 2.3, tip + u * 4.0 + w * 2.3],
                    col,
                    Stroke::NONE,
                ));
            }
        }
    }
    resp.on_hover_text(tip)
}

/// the clip / block name chip with its colour dot
fn pr_chip(ui: &mut egui::Ui, col: Color32, label: &str) -> egui::Response {
    let t = th();
    let font = FontId::proportional(12.0);
    let g = ui.painter().layout_no_wrap(label.to_string(), font, t.ink);
    let w = (g.size().x + 26.0).min(220.0);
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, BTN_H), Sense::hover());
    if ui.is_rect_visible(r) {
        let p = ui.painter_at(r);
        p.rect_filled(r, 4.0, Color32::from_black_alpha(70));
        p.circle_filled(Pos2::new(r.left() + 10.0, r.center().y), 4.0, col);
        p.galley(
            Pos2::new(r.left() + 19.0, r.center().y - g.size().y / 2.0),
            g,
            t.ink,
        );
    }
    resp
}

fn grid_name(g: Tick) -> &'static str {
    GRIDS.iter().find(|x| x.0 == g).map_or("?", |x| x.1)
}
