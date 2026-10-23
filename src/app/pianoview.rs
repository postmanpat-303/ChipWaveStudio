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
    /// Alt+drag: a freehand line (`PianoUi::line`) selecting the notes it
    /// crosses
    Line,
    /// Pencil: painting a run of notes from this start, one per length
    Paint(Tick),
}

/// the roll's tools (toolbar; keys 1 2 3 while the roll has the keyboard)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum PrTool {
    /// smart: click empty space draws a note, drag a note moves it, its
    /// right edge resizes it; drag on empty space draws a selection box,
    /// Alt+drag a line that selects the notes it crosses
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
    /// a move last applied as copies (Ctrl held): the originals stay
    copy: bool,
    /// a Pointer press on empty space: a box once it moves, a click (a
    /// new note) if it is let go within a few pixels
    soft: bool,
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

/// note-clip snap grids (ticks, label); 0 = Auto (follows the zoom)
const GRIDS: [(Tick, &str); 8] = [
    (4 * PPQ, "Bar"),
    (PPQ, "Beat"),
    (PPQ / 2, "1/8"),
    (ROW, "1/16"),
    (ROW / 2, "1/32"),
    (PPQ / 6, "1/16T"),
    (0, "Auto"),
    (1, "off"),
];

/// Auto snap: the finest of 1/32 … bar that is at least 12 px wide
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
    /// Alt+drag selection line so far (screen points)
    pub(crate) line: Vec<Pos2>,
    /// this swipe has deleted something (its undo point is set), and how many
    erase_started: bool,
    erased: usize,
    /// notes going up in smoke
    pub(crate) smoke: Vec<Puff>,
    offs: Vec<(Instant, u8)>,
    /// the keyboard key held with the mouse (sounding until release)
    pub(crate) key_down: Option<u8>,
    /// the last audition: (note, instrument, channel) (tests, status)
    pub(crate) heard: Option<(u8, u8, u8)>,
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
    /// the playhead as last drawn: (tick in the clip's source, playing);
    /// None when it is outside the clip (tests, screenshots)
    pub(crate) head: Option<(f64, bool)>,
    /// a Clip Arranger launcher clip shown in the roll (while no timeline
    /// clip is selected; Stage 4b)
    pub(crate) launch: Option<u32>,
    /// the toolbar's clip chip as drawn (tests)
    pub(crate) chip: String,
    /// copied notes (Ctrl+C / Ctrl+X): the first at tick 0, `clip_from`
    /// where it was; `clip_text` is what went to the system clipboard
    pub(crate) clip: Vec<tl::Note>,
    clip_from: Tick,
    clip_text: String,
    /// the last paste: (target, the playhead base it used, where the next
    /// Ctrl+V goes), so pasting again lays the copies end to end
    paste_next: Option<(Target, Option<Tick>, Tick)>,
    /// the paste marker set in the ruler (the bar strip): where Ctrl+V puts
    /// the first copied note in that target (a source tick); it moves to the
    /// end of each paste so Ctrl+V again lays copies end to end
    pub(crate) marker: Option<(Target, Tick)>,
}

impl PianoUi {
    /// the song changed under the roll (undo / redo, another song): note
    /// ids are indices or rows, so a kept selection would point at other
    /// notes. The selection and any gesture in progress are dropped.
    pub(crate) fn forget_selection(&mut self) {
        self.sel.clear();
        self.drag = None;
        self.erase = None;
        self.erase_started = false;
        self.press = None;
        self.pan = None;
    }
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
            line: vec![],
            erase_started: false,
            erased: 0,
            smoke: vec![],
            offs: vec![],
            key_down: None,
            heard: None,
            grid_rect: Rect::NOTHING,
            vel_rect: Rect::NOTHING,
            press: None,
            pan: None,
            step: None,
            step_used: false,
            step_input: false,
            head: None,
            launch: None,
            chip: String::new(),
            clip: vec![],
            clip_from: 0,
            clip_text: String::new(),
            paste_next: None,
            marker: None,
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
    #[cfg(test)]
    pub(crate) fn key_at_pub(&self, y: f32) -> i32 {
        self.key_at(y)
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
        if self.view == View::Timeline && self.tlv.sel.is_none() {
            // a launcher clip (no timeline clip selected): its notes
            let data = self.pr.launch.and_then(|id| {
                self.with(|s| {
                    s.song
                        .timeline
                        .as_deref()
                        .and_then(|t| t.launch_clip(id))
                        // while its track is the current one
                        .filter(|l| l.clip.track as usize == self.ch)
                        .and_then(|l| l.clip.notes())
                })
            });
            if let Some(data) = data {
                return Some(Target::Notes { data, clip: None });
            }
        }
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
    /// the instrument new notes in this clip get: its track's assigned
    /// instrument, 0 (the channel's current one) when it has none
    /// instrument when it has none (or it was deleted: says so)
    pub(super) fn pr_track_instr(&mut self, t: Target) -> u8 {
        let Target::Notes { clip: Some(id), .. } = t else {
            return 0;
        };
        let (tr, given, n) = self
            .with(|s| {
                let tl = s.song.timeline.as_deref()?;
                let c = tl.clips.iter().find(|c| c.id == id)?;
                let tr = c.track as usize;
                Some((tr, tl.track_instr(tr), s.song.instruments.len()))
            })
            .unwrap_or((0, 0, 0));
        if given as usize > n {
            self.status = format!(
                "Track {:02}'s instrument {given:02X} is missing (deleted?): new notes play the \
                 track's current sound. Drop an instrument from the Browser on the track to fix it",
                tr + 1
            );
            return 0;
        }
        given
    }

    /// the channel / track the roll's notes play on
    fn pr_channel(&self) -> usize {
        match self.pr.target {
            Some(Target::Notes { clip: Some(id), .. }) => self
                .tlv
                .cache
                .as_ref()
                .and_then(|c| c.tl.clips.iter().find(|x| x.id == id))
                .map_or(self.ch, |c| c.track as usize),
            Some(Target::Block { col, .. }) => col,
            _ => self.ch,
        }
    }

    fn pr_audition(&mut self, key: u8, instr: u8, vel: f32) {
        let chu = self.pr_channel();
        // an Empty track makes no sound until it gets an instrument
        if instr == 0 && self.track_needs_instrument(chu) {
            return;
        }
        let ch = chu as u8;
        // a note's own instrument, else the track's (as typing plays it)
        let ins = if instr > 0 {
            instr
        } else {
            self.track_instr(chu)
        };
        self.pr.heard = Some((key, ins, ch));
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
        let chu = self.pr_channel();
        if self.track_needs_instrument(chu) {
            return;
        }
        let ch = chu as u8;
        let ins = self.track_instr(chu);
        if let Some(&k) = keys.first() {
            self.pr.heard = Some((k, ins, ch));
        }
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

    /// the keyboard strip: press (and drag across) the keys to hear them
    /// on the track's instrument, a tooltip names the key. Never writes a
    /// note or an undo step.
    fn pr_keyboard(&mut self, ui: &mut egui::Ui, keys_r: Rect, modal: bool) {
        let r = ui.interact(
            keys_r,
            ui.scope_id().with("pr_keys"),
            Sense::click_and_drag(),
        );
        let held = r.is_pointer_button_down_on() && !modal;
        let at = r
            .interact_pointer_pos()
            .or(r.hover_pos())
            .filter(|p| keys_r.contains(*p))
            .map(|p| self.pr.key_at(p.y).clamp(1, 120) as u8);
        match (held, at) {
            (true, Some(k)) if self.pr.key_down != Some(k) => {
                self.pr_audition(k, 0, 0.8);
                // sounds while held (a long safety stop)
                for o in &mut self.pr.offs {
                    o.0 = Instant::now() + Duration::from_secs(8);
                }
                self.pr.key_down = Some(k);
            }
            (true, _) => {}
            (false, _) => {
                if self.pr.key_down.take().is_some() {
                    let now = Instant::now();
                    for o in &mut self.pr.offs {
                        o.0 = now;
                    }
                    self.pr_release(false);
                }
            }
        }
        if !modal && r.hovered() {
            if let Some(k) = at {
                let ins = self.track_instr(self.pr_channel());
                let name = self.with(|s| {
                    s.song
                        .instruments
                        .get(ins as usize - 1)
                        .map(|i| i.name.clone())
                        .unwrap_or_default()
                });
                r.on_hover_text_at_pointer(format!(
                    "{} · octave {}\nclick or drag: hear it on {ins:02X} {name}",
                    key_label(k),
                    (k - 1) / 12
                ));
            }
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
    /// state at the press, so a gesture is one clean edit. `copy` (Ctrl
    /// held, checked again on every frame up to the drop): a move leaves
    /// the originals and moves copies, which become the selection.
    fn pr_apply_drag(
        &mut self,
        d: &PrDrag,
        t: Target,
        tick: f64,
        key: i32,
        len_ext: Tick,
        copy: bool,
    ) {
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
                        self.pr.sel = if copy {
                            roll::copy_notes(p, col, &rows, drow, dk)
                        } else {
                            roll::move_notes(p, col, &rows, drow, dk)
                        };
                    }
                    DragK::Band(_) | DragK::Line | DragK::Paint(_) => return,
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
                    // the notes at the press (the selection then, or the
                    // grabbed one): copies change the live selection
                    let ids: Vec<usize> = if d.sel.contains(&d.id) {
                        d.sel.clone()
                    } else {
                        vec![d.id]
                    };
                    if copy {
                        self.pr.sel = roll::copy_shifted(&mut n.notes, &ids, dt, dk);
                        self.pr_write(t, src);
                        return;
                    }
                    self.pr.sel = ids.clone();
                    let min_start = ids
                        .iter()
                        .filter_map(|&i| n.notes.get(i))
                        .map(|x| x.start)
                        .min()
                        .unwrap_or(0);
                    let dt = dt.max(-min_start);
                    // keep the chord shape: clamp the group, not each note
                    let (lo, hi) = ids
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
                    for &i in &ids {
                        if let Some(x) = n.notes.get_mut(i) {
                            x.start += dt;
                            x.key = (x.key as i32 + dk).clamp(1, 120) as u8;
                        }
                    }
                }
                DragK::Band(_) | DragK::Line | DragK::Paint(_) => return,
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
                let instr = self.pr_track_instr(target);
                for kk in self.pr_chord_keys(key, song_key) {
                    nd.notes.push(tl::Note {
                        start,
                        len,
                        key: kk,
                        vel: roll::vel_u16(self.pr.last_vel),
                        instr,
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
    /// Arrow keys: move the selection `dt` ticks and `dk` semitones as one
    /// group (one undo step). If any note would leave the clip's start or
    /// end (block columns) or the key range C-0..B-9, nothing moves.
    /// Returns whether it moved.
    pub(super) fn pr_nudge(&mut self, dt: Tick, dk: i32) -> bool {
        let Some(t) = self.pr.target else {
            return false;
        };
        let Some(mut src) = self.pr_read(t) else {
            return false;
        };
        if self.pr.sel.is_empty() || (dt == 0 && dk == 0) {
            return false;
        }
        let keys_ok = |keys: &[u8]| keys.iter().all(|&k| (1..=120).contains(&(k as i32 + dk)));
        match (&mut src, t) {
            (Source::Block(p), Target::Block { col, .. }) => {
                let ns: Vec<_> = roll::col_notes(p, col)
                    .into_iter()
                    .filter(|n| self.pr.sel.contains(&n.row))
                    .collect();
                if ns.is_empty() {
                    return false;
                }
                let drow = dt / ROW;
                let rows = p.rows.max(1) as i64;
                let time_ok = ns
                    .iter()
                    .all(|n| (0..rows).contains(&(n.row as i64 + drow)));
                if !time_ok || !keys_ok(&ns.iter().map(|n| n.key).collect::<Vec<_>>()) {
                    self.status = Self::nudge_blocked(time_ok);
                    return false;
                }
                self.snapshot();
                let rows: Vec<usize> = ns.iter().map(|n| n.row).collect();
                self.pr.sel = roll::move_notes(p, col, &rows, drow, dk);
            }
            (Source::Notes(d), _) => {
                let ids: Vec<usize> = self
                    .pr
                    .sel
                    .iter()
                    .copied()
                    .filter(|&i| i < d.notes.len())
                    .collect();
                if ids.is_empty() {
                    return false;
                }
                let time_ok = ids.iter().all(|&i| d.notes[i].start + dt >= 0);
                if !time_ok || !keys_ok(&ids.iter().map(|&i| d.notes[i].key).collect::<Vec<_>>()) {
                    self.status = Self::nudge_blocked(time_ok);
                    return false;
                }
                self.snapshot();
                for &i in &ids {
                    let n = &mut d.notes[i];
                    n.start += dt;
                    n.key = (n.key as i32 + dk) as u8;
                }
            }
            _ => return false,
        }
        self.pr_write(t, src);
        true
    }

    fn nudge_blocked(time_ok: bool) -> String {
        if time_ok {
            "Not moved: a selected note would go outside the key range (C-0 to B-9)".into()
        } else {
            "Not moved: a selected note would go outside the clip".into()
        }
    }

    /// the step for Left / Right: the snap grid. With snap off, plain
    /// arrows nudge one tick (free timing) and Shift+arrows Auto's step
    /// (the finest grid at least 12 px wide at this zoom)
    fn pr_nudge_step(&self, shift: bool) -> Tick {
        match self.pr.target {
            Some(Target::Notes { .. }) if shift && self.pr.grid == 1 => smart_grid(self.pr.zoom),
            _ => self.pr_grid(),
        }
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

    // ------------------------------------------------- clipboard (Ctrl+C X V D)

    /// the selected notes as clipboard notes, and where the first starts
    fn pr_sel_clip(&self, t: Target) -> Option<(Vec<tl::Note>, Tick)> {
        if self.pr.sel.is_empty() {
            return None;
        }
        let src = self.pr_read(t)?;
        let (c, from) = match (&src, t) {
            (Source::Block(p), Target::Block { col, .. }) => roll::clip_block(p, col, &self.pr.sel),
            (Source::Notes(d), _) => roll::clip_notes(&d.notes, &self.pr.sel),
            _ => return None,
        };
        (!c.is_empty()).then_some((c, from))
    }

    /// Ctrl+C / Ctrl+X: the selected notes to the roll's clipboard (and as
    /// text to the system clipboard); cut also deletes them (one undo step)
    pub(super) fn pr_copy(&mut self, cut: bool, ctx: &egui::Context) {
        let Some((c, from)) = self.pr.target.and_then(|t| self.pr_sel_clip(t)) else {
            self.status =
                "Select notes to copy first (click, Shift+click, drag a box or Ctrl+A)".into();
            return;
        };
        let n = c.len();
        self.pr.clip_text = roll::notes_to_text(&c);
        // this also makes egui deliver Event::Paste on Ctrl+V
        ctx.copy_text(self.pr.clip_text.clone());
        self.pr.clip = c;
        self.pr.clip_from = from;
        self.pr.paste_next = None;
        if cut {
            self.pr_delete();
        }
        self.status = format!(
            "{} {n} note(s) · Ctrl+V pastes them at the paste marker (click the bar strip) or the playhead",
            if cut { "Cut" } else { "Copied" }
        );
    }

    /// where Ctrl+V puts the first note: the playhead (or play marker) in
    /// this clip, on the grid; in the Tracker the cursor row; else None
    /// (the notes go back where they were copied from)
    fn pr_paste_base(&self, t: Target) -> Option<Tick> {
        let g = self.pr_grid().max(1);
        if let Some((h, _)) = self.pr.head {
            return Some((h.max(0.0) as Tick).div_euclid(g) * g);
        }
        match t {
            Target::Block { .. } if self.view == View::Pattern => Some(tl::rows_to_ticks(self.row)),
            _ => None,
        }
    }

    /// the ruler's markings: bar numbers, taller ticks on beats and short
    /// ones on the grid (each only while 6 px or more apart), and beat
    /// labels (2.3) once beats are wide enough
    fn pr_ruler_marks(&self, ui: &egui::Ui, fx_r: Rect, grid_t: Tick, t: &crate::theme::Theme) {
        let p = ui.painter_at(fx_r);
        let ppt = self.pr.ppt() as f32;
        // marks that read on the strip (crt_bot) in every theme: the theme's
        // ink if it contrasts enough (Amiga's is dark on a dark strip)
        let lum = |c: Color32| 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
        let bg = lum(t.crt_bot);
        let ink = [
            t.ink2,
            t.ink,
            Color32::from_gray(215),
            Color32::from_gray(25),
        ]
        .into_iter()
        .find(|c| (lum(*c) - bg).abs() >= 100.0)
        .unwrap_or(t.ink2);
        let (bar_t, beat_t) = (16 * ROW, 4 * ROW);
        let t0 = self.pr.scroll_x.max(0.0) as Tick;
        let t1 = self.pr.tick_at(fx_r.right()).ceil() as Tick + 1;
        let bot = fx_r.bottom();
        let ticks = |step: Tick, h: f32, c: Color32, skip: &dyn Fn(Tick) -> bool| {
            if step <= 0 || step as f32 * ppt < 6.0 {
                return;
            }
            let mut tk = t0.div_euclid(step) * step;
            while tk <= t1 {
                if !skip(tk) {
                    p.vline(self.pr.x_of(tk), (bot - h)..=bot, Stroke::new(1.0, c));
                }
                tk += step;
            }
        };
        if grid_t < beat_t {
            ticks(grid_t, 3.0, ink.gamma_multiply(0.45), &|tk| {
                tk % beat_t == 0
            });
        }
        ticks(beat_t, 7.0, ink.gamma_multiply(0.75), &|tk| tk % bar_t == 0);
        ticks(bar_t, fx_r.height(), ink, &|_| false);
        // bar numbers: every bar, or every 2nd/4th... when they would crowd
        let mut every = 1;
        while (every * bar_t) as f32 * ppt < 24.0 && every < 1024 {
            every *= 2;
        }
        let font = FontId::proportional(10.0);
        let mut b = t0 / bar_t;
        while b * bar_t <= t1 {
            if b % every == 0 {
                p.text(
                    Pos2::new(self.pr.x_of(b * bar_t) + 3.0, fx_r.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("{}", b + 1),
                    font.clone(),
                    ink,
                );
            }
            b += 1;
        }
        if beat_t as f32 * ppt >= 40.0 {
            let small = FontId::proportional(9.0);
            let mut tk = t0.div_euclid(beat_t) * beat_t;
            while tk <= t1 {
                if tk % bar_t != 0 {
                    p.text(
                        Pos2::new(self.pr.x_of(tk) + 3.0, fx_r.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("{}.{}", tk / bar_t + 1, tk % bar_t / beat_t + 1),
                        small.clone(),
                        ink.gamma_multiply(0.8),
                    );
                }
                tk += beat_t;
            }
        }
    }

    /// the ruler's paste marker, if it was set in this target
    pub(crate) fn pr_marker(&self, t: Target) -> Option<Tick> {
        self.pr.marker.filter(|(mt, _)| *mt == t).map(|(_, m)| m)
    }

    /// the song tick where `t`'s source tick `src` plays: in the clip under
    /// the play marker if it holds `t`, else the earliest clip that does (a
    /// looped clip: its first pass). None outside the Timeline.
    fn pr_song_tick(&self, t: Target, src: Tick) -> Option<Tick> {
        let c = self.tlv.cache.as_ref()?;
        let holds = |x: &tl::Clip| match t {
            Target::Notes { clip: Some(id), .. } => x.id == id,
            Target::Notes { data, clip: None } => {
                matches!(x.kind, tl::ClipKind::Notes { data: d } if d as usize == data)
            }
            Target::Block { pattern, col } => x.block() == Some((pattern, col)),
        };
        let cue = self.tlv.cue;
        let mut cands: Vec<&tl::Clip> = c.tl.clips.iter().filter(|x| holds(x)).collect();
        cands.sort_by_key(|x| (!(cue >= x.start && cue < x.end()), x.start));
        cands.into_iter().find_map(|x| {
            let d = match x.loop_len {
                Some(l) if l > 0 => (src - x.offset).rem_euclid(l),
                _ => src - x.offset,
            };
            let at = x.start + d;
            (d >= 0 && at < x.end()).then_some(at)
        })
    }

    /// a press in the ruler: the paste marker goes there (on the roll's
    /// grid; Alt: no snap, as in the Timeline's ruler) and, in the
    /// Timeline, the play marker follows. Like the Timeline's ruler this
    /// also seeks while playing.
    pub(crate) fn pr_ruler_press(&mut self, t: Target, x: f32, free: bool, len: Tick) {
        let raw = self.pr.tick_at(x).max(0.0) as Tick;
        let m = if free {
            raw
        } else {
            roll::snap(raw, self.pr_grid().max(1))
        }
        .clamp(0, len.max(0));
        self.pr.marker = Some((t, m));
        self.pr.paste_next = None;
        let seek = self.pr_song_tick(t, m);
        if let Some(st) = seek {
            self.timeline_seek(st);
        }
        let (bar, beat) = (m / (16 * ROW) + 1, m % (16 * ROW) / (4 * ROW) + 1);
        self.status = format!(
            "Paste marker: {bar}.{beat}{} · Ctrl+V pastes the copied notes there",
            if seek.is_some() {
                " (the play marker moved too)"
            } else {
                ""
            }
        );
    }

    /// write clipboard notes into `t` with the first at `at` as one undo
    /// step; they become the selection. Returns (pasted, left out).
    fn pr_paste_clip(&mut self, t: Target, clip: &[tl::Note], at: Tick) -> Option<(usize, usize)> {
        let mut src = self.pr_read(t)?;
        let (sel, dropped) = match (&mut src, t) {
            (Source::Block(p), Target::Block { col, .. }) => {
                roll::paste_block(p, col, clip, (at.max(0) / ROW) as usize, 0)
            }
            (Source::Notes(d), _) => {
                let len = d.len.max(1);
                roll::paste_notes(&mut d.notes, clip, at, 0, len)
            }
            _ => return None,
        };
        if sel.is_empty() {
            return Some((0, dropped));
        }
        self.snapshot();
        let n = sel.len();
        self.pr.sel = sel;
        self.pr_write(t, src);
        Some((n, dropped))
    }

    fn left_out(dropped: usize) -> String {
        if dropped == 0 {
            String::new()
        } else {
            format!(
                " · {dropped} left out (past the clip's end, or a second note on one row of a \
                 Tracker column)"
            )
        }
    }

    /// Ctrl+V: paste the copied notes at the playhead (see
    /// [`Self::pr_paste_base`]), keeping their timing and pitches; `text`
    /// is the system clipboard (notes copied in another ChipWave window)
    pub(super) fn pr_paste(&mut self, text: Option<String>) {
        if let Some(c) = text
            .as_deref()
            .filter(|x| *x != self.pr.clip_text)
            .and_then(roll::notes_from_text)
        {
            self.pr.clip_text = text.unwrap_or_default();
            self.pr.clip = c;
            self.pr.clip_from = 0;
            self.pr.paste_next = None;
        }
        let Some(t) = self.pr.target else {
            return;
        };
        if self.pr.clip.is_empty() {
            self.status = "Nothing to paste (select notes and press Ctrl+C first)".into();
            return;
        }
        let clip = self.pr.clip.clone();
        if let Some(m) = self.pr_marker(t) {
            // the ruler's paste marker: the first note lands on it, and the
            // marker moves past the block so Ctrl+V again chains
            match self.pr_paste_clip(t, &clip, m) {
                Some((0, _)) => {
                    self.status =
                        "Nothing pasted: the paste marker is past the end of this clip".into()
                }
                Some((n, dropped)) => {
                    self.pr.marker = Some((t, m + roll::clip_span(&clip, self.pr_grid())));
                    self.pr.paste_next = None;
                    self.status = format!(
                        "Pasted {n} note(s) at the paste marker{} · Ctrl+V again pastes after them",
                        Self::left_out(dropped)
                    );
                }
                None => {}
            }
            return;
        }
        let base = self.pr_paste_base(t);
        let at = match (base, self.pr.paste_next) {
            // again at the same spot: after the last paste
            (b, Some((pt, pb, next))) if pt == t && pb == b => next,
            (Some(b), _) => b,
            (None, _) => self.pr.clip_from,
        };
        match self.pr_paste_clip(t, &clip, at) {
            Some((0, _)) => {
                self.status = "Nothing pasted: the playhead is past the end of this clip".into()
            }
            Some((n, dropped)) => {
                let next = at + roll::clip_span(&clip, self.pr_grid());
                self.pr.paste_next = Some((t, base, next));
                self.status = format!(
                    "Pasted {n} note(s){} · Ctrl+V again pastes after them",
                    Self::left_out(dropped)
                );
            }
            None => {}
        }
    }

    /// Ctrl+D: a copy of the selection right after it (its length rounded
    /// up to the grid), selected, so Ctrl+D again repeats it once more
    pub(super) fn pr_duplicate(&mut self) {
        let Some(t) = self.pr.target else { return };
        let Some((c, from)) = self.pr_sel_clip(t) else {
            self.status = "Select notes to duplicate first (Ctrl+A selects all)".into();
            return;
        };
        let at = from + roll::clip_span(&c, self.pr_grid());
        match self.pr_paste_clip(t, &c, at) {
            Some((0, _)) => {
                self.status =
                    "Nothing duplicated: there is no room after the selection in this clip".into()
            }
            Some((n, dropped)) => {
                // a paste marker in this clip follows the copy, so Ctrl+V
                // after Ctrl+D carries on from there
                if self.pr_marker(t).is_some() {
                    self.pr.marker = Some((t, at + roll::clip_span(&c, self.pr_grid())));
                }
                self.status = format!(
                    "Duplicated {n} note(s) after the selection{}",
                    Self::left_out(dropped)
                )
            }
            None => {}
        }
    }

    /// keys while the roll has focus; true = used
    pub(super) fn pr_key(&mut self, key: Key, m: egui::Modifiers) -> bool {
        let grid = self.pr_nudge_step(m.shift);
        match key {
            Key::Delete | Key::Backspace if self.pr.sel.is_empty() && self.cl.sel.is_some() => {
                self.cl_delete_sel()
            }
            Key::Delete | Key::Backspace => self.pr_delete(),
            // arrows (plain or Shift) move the selection a semitone / a
            // grid step; Ctrl+Up/Down an octave (Ctrl+Left/Right stay the
            // previous / next block)
            Key::ArrowUp | Key::ArrowDown => {
                let d = if m.command { 12 } else { 1 };
                self.pr_nudge(0, if key == Key::ArrowUp { d } else { -d });
            }
            Key::ArrowLeft if !m.command => {
                self.pr_nudge(-grid, 0);
            }
            Key::ArrowRight if !m.command => {
                self.pr_nudge(grid, 0);
            }
            Key::A if m.command => {
                let Some(t) = self.pr.target else {
                    return false;
                };
                if let Some(src) = self.pr_read(t) {
                    self.pr.sel = notes_of(&src, t).0.iter().map(|n| n.id).collect();
                }
            }
            Key::D if m.command && !m.shift && !m.alt => self.pr_duplicate(),
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
        self.pr_keyboard(ui, keys_r, modal);
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
        let mut use_instr_clip: Option<u32> = None;
        let mut make_unique: Option<u32> = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(tool.shrink2(egui::vec2(6.0, 1.0))), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // the clip / block chip: colour dot + name
                let (label, tip) = match target {
                    Target::Block { pattern, col } => {
                        let name = self.with(|s| s.song.patterns.get(pattern).map(|p| p.name.clone()).unwrap_or_default());
                        if self.tracker_on() {
                            (
                                format!("Block {pattern:02} {name} · ch {}", col + 1),
                                format!("block {pattern:02} {name}, channel {}", col + 1),
                            )
                        } else {
                            // without the Tracker: a clip on a track (5.5)
                            (
                                format!("Clip {pattern:02} {name} · Track {:02}", col + 1),
                                format!("clip {pattern:02} {name} on track {:02}", col + 1),
                            )
                        }
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
                self.pr.chip = label;
                // linked clips (D4): the count and Make unique
                if let Target::Notes { data, clip } = target {
                    let owner = clip.or(self.pr.launch);
                    let (a, b) = self.with(|s| s.song.timeline.as_deref().map_or((0, 0), |t| tl::linked(t, data)));
                    if a + b > 1 {
                        let tip = format!(
                            "Linked: {} clips ({a} in the arrangement, {b} in the launcher) share these notes, so an \
                             edit here changes all of them · click: Make unique (this clip gets its own copy)",
                            a + b
                        );
                        if pr_btn(ui, &format!("Linked ×{}", a + b), false, owner.is_some(), &tip).clicked() {
                            if let Some(id) = owner {
                                make_unique = Some(id);
                            }
                        }
                    }
                }
                if pr_btn(ui, "ALL", false, true, "select every note (Ctrl+A)").clicked() {
                    self.pr.sel = notes.iter().map(|n| n.id).collect();
                    self.pr.focus = true;
                }
                ui.add_space(6.0);
                // tools
                for (tool, tip) in [
                    (PrTool::Pointer, "Pointer (1): click empty space to draw a note; drag a note to move it, its right edge to resize; drag on empty space to box-select (Shift adds); Alt+drag a line through notes to select them (Shift+Alt adds)"),
                    (PrTool::Select, "Select (2): drag a box to select notes; Shift adds to the selection; Alt+drag a line through notes to select them"),
                    (PrTool::Pencil, "Pencil (3): click to draw, drag to paint a run of notes at the note length"),
                ] {
                    if pr_tool_btn(ui, tool, self.pr.tool == tool, tip).clicked() {
                        self.pr.tool = tool;
                    }
                }
                ui.add_space(6.0);
                if let Target::Notes { .. } = target {
                    let shown = if self.pr.grid == 0 {
                        format!("Auto · {}", len_name(smart_grid(self.pr.zoom)))
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
                    if pr_btn(ui, "Keys → Notes", on, true, "Keys → Notes (Ctrl+K): play notes on your computer keyboard. Off: those keys are normal shortcuts again.").clicked() {
                        self.typing_toggle();
                    }
                    // "Use track instrument": only while the clip has notes
                    // without an instrument (the button is the hint)
                    if let Target::Notes { clip: Some(id), .. } = target {
                        let (unset, slot) = self.with(|s| {
                            let t = s.song.timeline.as_deref();
                            let unset = t.map_or(0, |t| crate::timeline::unset_instr_notes(t, id));
                            let tr = t.and_then(|t| t.clips.iter().find(|c| c.id == id)).map(|c| c.track as usize);
                            (unset, tr.map_or(0, |tr| crate::timeline::assigned_instr(&s.song, tr)))
                        });
                        if unset > 0 {
                            let tip = if slot == 0 {
                                format!(
                                    "{unset} note{} here have no instrument, so they play whatever the channel last played. The track has no instrument assigned: give it one in the Inspector first",
                                    if unset == 1 { "" } else { "s" }
                                )
                            } else {
                                format!(
                                    "{unset} note{} here have no instrument, so they play whatever the channel last played. Click to give them the track's instrument {slot:02} (notes that have one are left alone; one undo step)",
                                    if unset == 1 { "" } else { "s" }
                                )
                            };
                            if pr_btn(ui, "Use track instrument", false, slot > 0, &tip).clicked() && slot > 0 {
                                use_instr_clip = Some(id);
                            }
                        }
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
                            "click: draw (same length as the last note) · drag a note: move · its right edge: length · drag empty space: box select · Alt+drag: line select · Shift: add · right-click or right-drag: erase · click the bar strip: paste marker".into()
                        };
                        ui.label(egui::RichText::new(tip).small().color(t.ink2));
                    }
                }
            });
        });
        if let Some(id) = use_instr_clip {
            // the roll's clip, whatever the Timeline has selected
            let (sel, sels) = (
                self.tlv.sel,
                std::mem::replace(&mut self.tlv.sels, vec![id]),
            );
            self.tl_apply(super::timeline::TlOp::UseTrackInstr);
            self.tlv.sel = sel;
            self.tlv.sels = sels;
        }
        if let Some(id) = make_unique {
            let ok = self.with(|s| {
                s.song
                    .timeline
                    .as_deref_mut()
                    .and_then(|t| tl::make_unique_notes(t, id))
                    .is_some()
            });
            if ok {
                self.pr.sel.clear();
                self.dirty = true;
                self.status = "Made unique: this clip has its own notes now (edits no longer change the others)".into();
            }
        }
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
        }
        self.pr_ruler_marks(ui, fx_r, grid_t, &t);
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
        // a group being moved: ghosts where the notes were (copies: the
        // originals are still there)
        if let Some(d) = self
            .pr
            .drag
            .as_ref()
            .filter(|d| d.kind == DragK::Move && !d.copy)
        {
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
        // playhead: the Timeline's own (the transport while it plays, the
        // play marker when stopped), mapped through the clip's start, trim
        // and loop into this clip; a Tracker block loop in tracker mode
        let tl_now = self.with(|s| {
            s.song
                .timeline
                .is_some()
                .then(|| match super::timeline::tl_head(&s.engine) {
                    Some(h) => (h, true),
                    None => (self.tlv.cue as f64, false),
                })
        });
        let ph: Option<(f64, bool)> = match (tl_now, self.tlv.cache.as_ref()) {
            (Some((h, live)), Some(c)) => {
                let tk = h.floor() as Tick;
                let inside = |x: &&tl::Clip| tk >= x.start && tk < x.end();
                let clip = match target {
                    Target::Notes { clip: Some(id), .. } => {
                        c.tl.clips.iter().find(|x| x.id == id).filter(inside)
                    }
                    Target::Notes { data, clip: None } => c.tl.clips.iter().filter(inside).find(
                        |x| matches!(x.kind, tl::ClipKind::Notes { data: d } if d as usize == data),
                    ),
                    Target::Block { pattern, col } => {
                        c.tl.clips
                            .iter()
                            .filter(inside)
                            .find(|x| x.block() == Some((pattern, col)))
                    }
                };
                clip.map(|x| (x.source_at(tk) as f64 + (h - tk as f64), live))
            }
            _ => self.with(|s| {
                let e = &s.engine;
                match target {
                    Target::Block { pattern, .. } if e.playing && e.pattern == pattern => Some((
                        tl::rows_to_ticks(e.row) as f64 + e.row_phase() as f64 * ROW as f64,
                        true,
                    )),
                    _ => None,
                }
            }),
        };
        self.pr.head = ph;
        if let Some((ph, live)) = ph {
            let x = self.pr.x_of(0) + (ph * self.pr.ppt()) as f32;
            if grid_r.x_range().contains(x) {
                let col = if live {
                    t.play
                } else {
                    t.play.gamma_multiply(0.55)
                };
                p.vline(x, grid_r.union(vel_r).y_range(), Stroke::new(1.5, col));
            }
            if live {
                // Follow (the Timeline's toggle): page so the playhead stays
                // in view, a third in, like the Timeline
                if self.follow && !ui.input(|i| i.pointer.any_down()) {
                    let vis = grid_r.width() as f64 / self.pr.ppt();
                    let s = self.pr.scroll_x;
                    if ph < s || ph > s + vis * 0.92 {
                        self.pr.scroll_x = (ph - vis / 3.0).max(0.0);
                    }
                }
                ui.ctx().request_repaint();
            }
        }
        // the paste marker: a triangle in the ruler and a dashed line (the
        // playhead is a solid line, so the two never look alike)
        if let Some(m) = self.pr_marker(target) {
            let x = self.pr.x_of(m);
            if grid_r.x_range().contains(x) {
                ui.painter_at(grid_r).add(egui::Shape::dashed_line(
                    &[Pos2::new(x, grid_r.top()), Pos2::new(x, grid_r.bottom())],
                    Stroke::new(1.0, t.accent),
                    4.0,
                    3.0,
                ));
                ui.painter_at(fx_r).add(egui::Shape::convex_polygon(
                    vec![
                        Pos2::new(x - 5.0, fx_r.top() + 2.0),
                        Pos2::new(x + 5.0, fx_r.top() + 2.0),
                        Pos2::new(x, fx_r.bottom()),
                    ],
                    t.accent,
                    Stroke::new(1.0, Color32::from_black_alpha(160)),
                ));
            }
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
        // ---- input: the ruler (the bar strip) places the paste marker
        let rresp = ui.interact(
            fx_r,
            egui::Id::unique("piano_ruler"),
            if modal {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            },
        );
        if rresp.clicked() || rresp.drag_started() || rresp.dragged() {
            if let Some(p) = rresp.interact_pointer_pos() {
                let alt = ui.input(|i| i.modifiers.alt);
                let before = self.pr_marker(target);
                let raw = self.pr.tick_at(p.x).max(0.0) as Tick;
                let m = if alt {
                    raw
                } else {
                    roll::snap(raw, grid_t.max(1))
                }
                .clamp(0, len.max(0));
                if rresp.clicked() || rresp.drag_started() || before != Some(m) {
                    self.pr_ruler_press(target, p.x, alt, len);
                }
            }
            self.pr.focus = true;
        }
        rresp.on_hover_text(
            "Click or drag: place the paste marker (on the grid; Alt: no snap). Ctrl+V pastes \
             the copied notes there and moves it past them, so Ctrl+V again lays copies end to \
             end. In the Timeline the play marker follows (it seeks while playing too, like the \
             Timeline's ruler).",
        );
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
        // a Pointer box let go within a few pixels was a click after all
        let late_click = match &self.pr.drag {
            Some(d) if resp.drag_stopped() && d.soft && !d.moved => match d.kind {
                DragK::Band(a) => Some(a),
                _ => None,
            },
            _ => None,
        };
        let clicked = resp.clicked() || late_click.is_some();
        if panning {
            // the pan owns this press: no note, no box, no draw
        } else if resp.drag_started_by(egui::PointerButton::Primary) || clicked {
            self.pr.press = None;
            let pos = late_click
                .or(press_pos)
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
                copy: false,
                soft: false,
            };
            match hit {
                Some((n, edge)) => {
                    if mods.shift || mods.command {
                        // Shift (or Ctrl) + click toggles it in the selection
                        if let Some(i) = self.pr.sel.iter().position(|&x| x == n.id) {
                            if clicked {
                                self.pr.sel.remove(i);
                            }
                        } else {
                            self.pr.sel.push(n.id);
                        }
                    } else if clicked || !self.pr.sel.contains(&n.id) {
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
                // Alt+drag on empty space (Pointer or Select): a freehand
                // line selecting every note it crosses (Shift+Alt adds)
                None if !clicked && mods.alt && self.pr.tool != PrTool::Pencil => {
                    d.kind = DragK::Line;
                    self.pr.line = vec![pos];
                    if !(mods.shift || mods.command) {
                        self.pr.sel.clear();
                    }
                }
                // Shift (or Ctrl) on empty space: a box that adds to the
                // selection (never a new note); the Select tool, or a drag
                // with the Pointer: a new box
                None if mods.shift
                    || mods.command
                    || self.pr.tool == PrTool::Select
                    || (self.pr.tool == PrTool::Pointer && !clicked) =>
                {
                    d.kind = DragK::Band(pos);
                    d.soft = self.pr.tool == PrTool::Pointer && !(mods.shift || mods.command);
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
            if clicked {
                self.pr.drag = None;
            } else {
                self.pr.drag = Some(d);
            }
        } else if resp.dragged_by(egui::PointerButton::Primary) {
            if let (Some(d), Some(pos)) = (self.pr.drag.clone(), resp.interact_pointer_pos()) {
                let tk = self.pr.tick_at(pos.x);
                let k = self.pr.key_at(pos.y).clamp(1, 120);
                match d.kind {
                    DragK::Line => {
                        let last = self.pr.line.last().copied().unwrap_or(pos);
                        if pos.distance(last) >= 1.0 {
                            for n in swept_notes(&self.pr, &notes, last, pos) {
                                if !self.pr.sel.contains(&n.id) {
                                    self.pr.sel.push(n.id);
                                }
                            }
                            self.pr.line.push(pos);
                        }
                        self.status = format!(
                            "Line select: {} note(s) (let go to finish)",
                            self.pr.sel.len()
                        );
                    }
                    DragK::Band(a) if d.soft && !d.moved && a.distance(pos) <= HOLD_PX => {}
                    DragK::Band(a) => {
                        if let Some(x) = self.pr.drag.as_mut() {
                            x.moved = true;
                        }
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
                            self.pr_apply_drag(&d, target, tk, k, grid_t, false);
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
                        // Ctrl held: copies (decided again up to the drop)
                        let copy = mods.command;
                        self.pr_apply_drag(&d, target, tk, k, grid_t, copy);
                        if let Some(x) = self.pr.drag.as_mut() {
                            x.copy = copy;
                        }
                        if copy {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Copy);
                            let n = self.pr.sel.len();
                            ui.painter().text(
                                pos + egui::vec2(14.0, -14.0),
                                egui::Align2::LEFT_BOTTOM,
                                format!("+ copy {n}"),
                                FontId::proportional(11.0),
                                t.ink,
                            );
                            self.status = format!(
                                "Copying {n} note(s): let go to drop the copies (release Ctrl first to move instead)"
                            );
                        }
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
        // the Alt line, drawn while it is being dragged
        if self.pr.drag.as_ref().is_some_and(|d| d.kind == DragK::Line) && self.pr.line.len() > 1 {
            // clipped to the note grid, like the notes
            let p = ui.painter_at(self.pr.grid_rect);
            p.add(egui::Shape::line(
                self.pr.line.clone(),
                Stroke::new(1.5, t.accent),
            ));
            if let Some(&end) = self.pr.line.last() {
                p.circle_filled(end, 2.5, t.accent);
            }
        }
        if resp.drag_stopped() {
            if self.pr.drag.as_ref().is_some_and(|d| d.kind == DragK::Line) {
                self.pr.line.clear();
                self.status = format!("Selected {} note(s)", self.pr.sel.len());
            }
            // Ctrl decides at the drop: pressed or let go after the last
            // mouse move, the move becomes copies or back
            if let (Some(d), Some(pos)) = (self.pr.drag.clone(), latest) {
                if d.kind == DragK::Move && d.copy != mods.command {
                    let tk = self.pr.tick_at(pos.x);
                    let k = self.pr.key_at(pos.y).clamp(1, 120);
                    self.pr_apply_drag(&d, target, tk, k, grid_t, mods.command);
                }
                if d.kind == DragK::Move && mods.command {
                    self.status = format!("Copied {} note(s) (Ctrl+Z undoes)", self.pr.sel.len());
                }
            }
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
            self.status = format!(
                "This {} has volumes or effects between its notes, so it holds one note at a \
                 time; draw chords in a note clip (double-click empty space)",
                self.clip_word()
            );
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
        self.status = format!(
            "Chord: this {} is a note clip now (Ctrl+Z puts it back)",
            self.clip_word()
        );
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
        return (
            fill,
            Color32::from_rgb(0xFF, 0xF1, 0xD6),
            Color32::from_rgb(0x20, 0x14, 0x08),
        );
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
    if ui.is_rect_visible(r) && t.ember {
        // Slate: the outlined button (LED dot when on)
        crate::ember::paint_button(ui.painter(), r, &resp, label, on, false, enabled);
    } else if ui.is_rect_visible(r) {
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

/// "C#4" for a roll key (1 = C0, as the tracker counts)
pub(crate) fn key_label(k: u8) -> String {
    const N: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let i = k.max(1) as usize - 1;
    format!("{}{}", N[i % 12], i / 12)
}
