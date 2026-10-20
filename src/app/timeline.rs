//! Timeline view (docs/TIMELINE.md): the song as tracks (= mixer channels) x
//! time, Bitwig-style. M1 is read-only: in tracker mode the timeline is
//! derived from the order list (`crate::timeline::from_order`) and cached
//! until the song changes. One painter-based widget: clips, previews and
//! grid go into a few meshes, only visible tracks and clips are touched
//! (binary search per track), so hundreds or thousands of clips stay cheap.
//! M2 (timeline mode): when the song has a timeline (`song.timeline`), the
//! view shows and edits it: move, resize (loop), trim, split, duplicate,
//! make unique, glue, delete and mute clips, snap, loop region and markers.
//! In tracker mode the first edit asks to switch (undo puts it back).
use super::*;
use crate::timeline::{self as tl, Tick, PPQ, ROW};
use std::collections::HashMap;

pub(crate) mod audioclip;
pub(crate) mod autolanes;
mod edit;
use edit::DragEdit;

/// track header column width
pub(crate) const HEAD_W: f32 = 250.0;
const MARK_H: f32 = 18.0;
const RULER_H: f32 = 20.0;
const OVERVIEW_H: f32 = 18.0;
const VBAR_W: f32 = 10.0;
const DEFAULT_TRACK_H: f32 = 46.0;
/// zoom limits in pixels per beat
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 480.0;

/// track colours: muted, distinct, readable with white text (dark and Amiga themes)
const TRACK_COLOURS: [u32; 12] = [
    0xB0605A, 0xC0864A, 0xB9A04E, 0x6E9A58, 0x4F9A96, 0x5A86B4, 0x7A6CB4, 0xA8679A, 0x8A7A68,
    0x5F8FA0, 0x9A6A50, 0x7F8A99,
];

pub(crate) fn track_colour(i: usize) -> Color32 {
    hex(TRACK_COLOURS[i % TRACK_COLOURS.len()])
}

/// track `i`'s colour: its own (Inspector › Colour) or the palette's
pub(crate) fn track_colour_in(t: &tl::Timeline, i: usize) -> Color32 {
    match t.tracks.get(i).and_then(|x| x.colour) {
        Some(c) => hex(c & 0xFF_FFFF),
        None => track_colour(i),
    }
}

/// the palette offered for a track's colour (Inspector)
pub(crate) fn track_palette() -> impl Iterator<Item = u32> {
    TRACK_COLOURS.iter().copied()
}

/// what a block column looks like in a clip: note bars (row, rows, key)
#[derive(Clone, Debug, Default)]
struct Preview {
    notes: Vec<(u16, u16, u8)>,
    lo: u8,
    hi: u8,
}

/// the derived timeline and everything the painter needs, rebuilt only when
/// the song's content changes (fingerprint)
pub(crate) struct Cache {
    /// fingerprints: block contents (previews) and the timeline / order list
    pkey: u64,
    pub(crate) key: u64,
    /// the song is in timeline mode (`tl` is the song's own timeline)
    pub(crate) mode: bool,
    pub(crate) tl: tl::Timeline,
    pub(crate) map: tl::OrderMap,
    idx: tl::TrackIndex,
    previews: HashMap<(usize, usize), Preview>,
    /// the most used instrument of each (block, column) and how often
    col_instr: HashMap<(usize, usize), (u8, usize)>,
    /// "02 Groove" per block
    names: Vec<String>,
    /// the instrument a track mostly plays
    instr: Vec<String>,
    /// ... as its slot (1-based; 0 = none): header icon tiles, Inspector
    pub(crate) instr_ix: Vec<u8>,
    order: Vec<usize>,
    /// used[block][column]: the column has something in its played rows
    used: Vec<Vec<bool>>,
    /// where the song ends (ticks)
    pub(crate) end: Tick,
}

/// snap grid for clip edits
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Snap {
    /// bar, beat or row, whatever is at least ~10 px wide at this zoom
    Adaptive,
    Bar,
    Beat,
    Row,
}

/// a mouse gesture in progress (timeline mode)
#[derive(Clone, Debug)]
enum DragKind {
    /// move the selection (Alt at the start: copies)
    Move { dup: bool },
    /// right edge of clip `id`
    End(u32),
    /// left edge of clip `id`
    Start(u32),
    /// rubber band from this point
    Band(Pos2),
    /// marker index
    Marker(usize),
    /// Shift-drag on the ruler: loop region from this tick
    Loop(Tick),
    /// fade handle of audio clip `id` (true: fade in)
    Fade(u32, bool),
}

#[derive(Clone, Debug)]
struct Drag {
    kind: DragKind,
    /// the timeline when the gesture started (edits are re-applied to it)
    origin: tl::Timeline,
    /// before Alt-copies were made (restored when the copy didn't move)
    before: Option<tl::Timeline>,
    grab_tick: f64,
    grab_y: f32,
    /// start of the clip under the mouse (moves snap by it)
    anchor: Tick,
    moved: bool,
}

/// an edit that waits for the switch to timeline mode
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TlOp {
    Split(Tick),
    Duplicate,
    Delete,
    Mute,
    MakeUnique,
    Glue,
    LoopFromSelection,
    ToggleLoop,
    AddMarker(Tick),
    NewClip(usize, Tick),
    /// a new note clip (piano roll) on (track, tick)
    NewNotes(usize, Tick),
    /// the phrase just played on the MIDI keyboard as a note clip
    Capture(usize, Tick),
    Speed(usize, u8),
    SelectAll,
    /// place the decoded audio imports (`TimelineUi::ready_audio`)
    PlaceAudio,
    /// a Browser drum pattern (`drumpat::PATTERNS` index) as a note clip on
    /// `track` (u8::MAX = a new drum track below the last) at `at`
    DrumPattern {
        pat: u16,
        track: u8,
        at: Tick,
    },
    /// a Clip Arranger slot (block, column) as a Tracker clip on `track` at `at`
    PlaceBlock {
        block: u16,
        col: u8,
        track: u8,
        at: Tick,
    },
}

pub(crate) struct TimelineUi {
    pub(crate) cache: Option<Cache>,
    /// pixels per beat (0 = fit on the next frame)
    pub(crate) zoom: f32,
    /// first visible tick (fractional while zooming)
    pub(crate) scroll: f64,
    scroll_y: f32,
    pub(crate) track_h: f32,
    /// start a new song's layout (a few bars, not "fit one empty block")
    new_song: bool,
    wheel: f32,
    /// the clip clicked last (Enter opens it); always in `sels` when set
    pub(crate) sel: Option<u32>,
    /// selected clips (edits act on these)
    pub(crate) sels: Vec<u32>,
    drag: Option<Drag>,
    /// timeline-mode play marker (ticks, on a row)
    pub(crate) cue: Tick,
    pub(crate) snap: Snap,
    /// "switch to timeline mode?" is open, with the edit that asked
    pub(crate) ask_switch: Option<Option<TlOp>>,
    /// "back to tracker mode?" is open
    pub(crate) ask_leave: bool,
    /// marker being renamed (index, text)
    rename: Option<(usize, String)>,
    /// mouse position in timeline terms (S splits here)
    hover_at: Option<(usize, Tick)>,
    /// right-click spot (track, tick) and marker for the context menus
    menu_at: Option<(usize, Tick)>,
    menu_marker: Option<usize>,
    /// snap grid as last drawn (keys use it)
    grid_now: Tick,
    /// the ruler shows minutes:seconds instead of bars
    time_ruler: bool,
    ruler_drag: bool,
    overview_drag: bool,
    vbar_drag: bool,
    /// screen rects (tests, screenshots): lanes, ruler, headers
    pub(crate) lanes: Rect,
    pub(crate) ruler: Rect,
    pub(crate) heads: Rect,
    /// the playhead as last drawn (ticks; None = stopped) - tests
    pub(crate) head_drawn: Option<f64>,
    /// time to build and paint the last frame, in microseconds
    pub(crate) frame_us: u128,
    /// audio files being decoded, and decoded ones waiting to be placed
    pub(crate) imports: Vec<audioclip::Import>,
    pub(crate) ready_audio: Vec<(tl::AudioAsset, usize, Tick)>,
    /// track rows and unfolded automation lanes as last drawn (M5)
    pub(crate) layout: autolanes::Layout,
    pub(crate) auto_drag: Option<autolanes::AutoDrag>,
    pub(crate) auto_menu: Option<autolanes::AutoMenu>,
    pub(crate) lane_menu: Option<(std::time::Instant, super::lanes::LaneMenu)>,
    /// a track header being dragged (reorder): the track
    pub(crate) head_drag: Option<usize>,
    /// follow last-touched knob (design A §3.4): open/show its lane
    pub(crate) follow_touch: bool,
    /// last automatable target the user moved (instrument/FX/mixer knob)
    pub(crate) last_touch: Option<crate::automation::Target>,
}

impl Default for TimelineUi {
    fn default() -> Self {
        Self {
            cache: None,
            zoom: 0.0,
            scroll: 0.0,
            scroll_y: 0.0,
            track_h: 0.0,
            new_song: false,
            wheel: 0.0,
            sel: None,
            sels: vec![],
            drag: None,
            cue: 0,
            snap: Snap::Adaptive,
            ask_switch: None,
            ask_leave: false,
            rename: None,
            hover_at: None,
            menu_at: None,
            menu_marker: None,
            grid_now: PPQ,
            time_ruler: false,
            ruler_drag: false,
            overview_drag: false,
            vbar_drag: false,
            lanes: Rect::NOTHING,
            ruler: Rect::NOTHING,
            heads: Rect::NOTHING,
            head_drawn: None,
            frame_us: 0,
            imports: vec![],
            ready_audio: vec![],
            layout: Default::default(),
            auto_drag: None,
            auto_menu: None,
            lane_menu: None,
            head_drag: None,
            follow_touch: false,
            last_touch: None,
        }
    }
}

impl TimelineUi {
    /// File > New: a fresh layout (zoomed to show a few dozen bars)
    pub(super) fn start_new_song(&mut self) {
        self.new_song = true;
        self.zoom = 0.0;
        self.scroll = 0.0;
        self.scroll_y = 0.0;
        self.sel = None;
        self.sels.clear();
        self.cue = 0;
        self.drag = None;
    }

    /// snap grid in ticks at pixels-per-tick `ppt` (`fine`: Shift = rows)
    fn grid(&self, ppt: f64, bar: Tick, fine: bool) -> Tick {
        if fine {
            return ROW;
        }
        match self.snap {
            Snap::Bar => bar,
            Snap::Beat => PPQ,
            Snap::Row => ROW,
            Snap::Adaptive => [ROW, PPQ, bar, bar * 4]
                .into_iter()
                .find(|g| *g as f64 * ppt >= 10.0)
                .unwrap_or(bar * 4),
        }
    }
    fn th(&self) -> f32 {
        if self.track_h > 0.0 {
            self.track_h
        } else {
            DEFAULT_TRACK_H
        }
    }
}

/// cheap content hash of the timeline / order list part of the view
fn fingerprint(song: &Song) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    put(song.channels as u64);
    put(song.bpm.to_bits() as u64);
    for &o in &song.order {
        put(o as u64);
    }
    put(u64::MAX);
    if let Some(t) = &song.timeline {
        put(tl::plan_key(song, t));
        put(t.next_id as u64);
        put(t.end as u64);
        for m in &t.markers {
            put(m.tick as u64);
            for b in m.name.bytes() {
                put(b as u64);
            }
        }
        for c in &t.clips {
            put(c.id as u64);
        }
        for d in &t.notes {
            for b in d.name.bytes() {
                put(b as u64);
            }
            put(0x1_0000);
        }
        for a in &t.audio {
            for b in a.name.bytes().chain(a.path.bytes()) {
                put(b as u64);
            }
            put(a.embed as u64 | (a.missing() as u64) << 1 | (a.bytes.is_some() as u64) << 2);
        }
        for c in t.clips.iter().filter(|c| c.audio().is_some()) {
            put(c.gain.to_bits() as u64 | (c.fade_shape as u64) << 32);
        }
        // automation lanes (M5): points, labels, which tracks are unfolded
        put(tl::auto_key(song, t));
        put(t.view.auto_open);
        for x in &t.tracks {
            put(x.colour.map_or(u64::MAX, |c| c as u64) ^ (x.instr as u64) << 40);
        }
        for l in t
            .tracks
            .iter()
            .flat_map(|x| x.auto.iter())
            .chain(t.clips.iter().flat_map(|c| c.auto.iter()))
        {
            put(l.enabled as u64 | (l.points.len() as u64) << 1);
            for b in l.label.bytes().chain(l.target.bytes()) {
                put(b as u64);
            }
        }
    }
    put(u64::MAX - 1);
    for n in &song.channel_names {
        for b in n.bytes() {
            put(b as u64);
        }
        put(0x1_0000);
    }
    // block lengths and breaks change the derived clips
    for p in &song.patterns {
        put(p.rows as u64 | (p.role.map_or(0, |r| r as u64 + 1) << 32));
        put(tl::effective_rows(p, song.channels) as u64);
    }
    put(pattern_key(song));
    h
}

/// cheap content hash of the blocks and names (previews, labels)
fn pattern_key(song: &Song) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    put(song.channels as u64);
    for n in song
        .channel_names
        .iter()
        .chain(song.instruments.iter().map(|i| &i.name))
    {
        for b in n.bytes() {
            put(b as u64);
        }
        put(0x1_0000);
    }
    for p in &song.patterns {
        for b in p.name.bytes() {
            put(b as u64);
        }
        put(p.rows as u64 | (p.role.map_or(0, |r| r as u64 + 1) << 32));
        for row in &p.cells {
            for c in row {
                put(u64::from_le_bytes([
                    c.note, c.instr, c.vol, c.fx, c.fxp, 0, 0, 1,
                ]));
            }
        }
    }
    h
}

/// Rebuild what changed: block previews only when the blocks changed, the
/// clips whenever the timeline / order list did (a drag rebuilds only those).
fn build_cache(song: &Song, key: u64, old: Option<Cache>) -> Cache {
    let pkey = pattern_key(song);
    let nch = song.channels.min(MAX_CHANNELS);
    let (previews, col_instr, used, names) = match old {
        Some(o) if o.pkey == pkey => (o.previews, o.col_instr, o.used, o.names),
        _ => build_previews(song, nch),
    };
    let (mut t, map) = tl::from_order(song);
    let mode = song.timeline.is_some();
    if let Some(own) = &song.timeline {
        t = (**own).clone();
        // keep Song/Master tracks past the mixer channels
        if t.tracks.len() < nch {
            t.tracks.resize_with(nch, Default::default);
        }
        for (tr, n) in t.tracks.iter_mut().zip(&song.channel_names) {
            if tr.kind != tl::TrackKind::Master {
                tr.name.clone_from(n);
            }
        }
    }
    let idx = t.index();
    let end = if mode { tl::play_end(&t) } else { map.end() };
    // the instrument each track plays most: clips x their columns' instruments
    let mut count: Vec<HashMap<u8, usize>> = vec![HashMap::new(); nch];
    for c in &t.clips {
        if let Some((b, col)) = c.block() {
            if let (Some(&(i, n)), Some(m)) =
                (col_instr.get(&(b, col)), count.get_mut(c.track as usize))
            {
                let passes = c.loop_len.map_or(1, |l| (c.len / l.max(1)).max(1) as usize);
                *m.entry(i).or_default() += n * passes;
            }
        }
        if let (tl::ClipKind::Notes { data }, Some(m)) = (&c.kind, count.get_mut(c.track as usize))
        {
            if let Some(d) = t.notes.get(*data as usize) {
                for n in d.notes.iter().filter(|n| n.instr > 0) {
                    *m.entry(n.instr).or_default() += 1;
                }
            }
        }
    }
    let instr_ix: Vec<u8> = (0..t.tracks.len())
        .map(|tr| {
            count
                .get(tr)
                .and_then(|m| m.iter().max_by_key(|(k, n)| (**n, std::cmp::Reverse(**k))))
                .map(|(k, _)| *k)
                .filter(|k| (*k as usize) <= song.instruments.len())
                .unwrap_or_else(|| t.tracks[tr].instr)
        })
        .collect();
    let mut instr: Vec<String> = count
        .iter()
        .map(|m| {
            m.iter()
                .max_by_key(|(k, n)| (**n, std::cmp::Reverse(**k)))
                .and_then(|(k, _)| {
                    song.instruments
                        .get(*k as usize - 1)
                        .map(|i| format!("{k:02X} {}", i.name))
                })
                .unwrap_or_default()
        })
        .collect();
    // Song/Master tracks past nch have no mixer instrument subtitle
    while instr.len() < t.tracks.len() {
        instr.push(String::new());
    }
    Cache {
        pkey,
        key,
        mode,
        tl: t,
        map,
        idx,
        previews,
        col_instr,
        names,
        instr,
        instr_ix,
        order: song.order.clone(),
        used,
        end,
    }
}

type Previews = (
    HashMap<(usize, usize), Preview>,
    HashMap<(usize, usize), (u8, usize)>,
    Vec<Vec<bool>>,
    Vec<String>,
);

fn build_previews(song: &Song, nch: usize) -> Previews {
    let mut previews = HashMap::new();
    let mut col_instr = HashMap::new();
    let mut used = vec![];
    for (b, p) in song.patterns.iter().enumerate() {
        let eff = tl::effective_rows(p, nch);
        let mut u = vec![false; nch];
        for (c, used_c) in u.iter_mut().enumerate() {
            let mut pv = Preview {
                lo: 255,
                ..Default::default()
            };
            let mut open: Option<(usize, u8)> = None;
            let mut ic: HashMap<u8, usize> = HashMap::new();
            for r in 0..eff {
                let Some(cell) = p.cells.get(r).and_then(|row| row.get(c)) else {
                    continue;
                };
                *used_c |= !cell.is_empty();
                if cell.instr > 0 {
                    *ic.entry(cell.instr).or_default() += 1;
                }
                if cell.note == 0 {
                    continue;
                }
                if let Some((r0, k)) = open.take() {
                    pv.notes.push((r0 as u16, (r - r0) as u16, k));
                }
                if cell.note != NOTE_OFF {
                    open = Some((r, cell.note));
                    pv.lo = pv.lo.min(cell.note);
                    pv.hi = pv.hi.max(cell.note);
                }
            }
            if let Some((r0, k)) = open {
                pv.notes.push((r0 as u16, (eff - r0).max(1) as u16, k));
            }
            if let Some((&i, &n)) = ic.iter().max_by_key(|(k, n)| (**n, std::cmp::Reverse(**k))) {
                col_instr.insert((b, c), (i, n));
            }
            if !pv.notes.is_empty() {
                // at least an octave of range, centred
                let span = pv.hi - pv.lo;
                if span < 12 {
                    let pad = (12 - span) / 2;
                    pv.lo = pv.lo.saturating_sub(pad).max(1);
                    pv.hi = (pv.lo + 12).max(pv.hi);
                }
                previews.insert((b, c), pv);
            }
        }
        used.push(u);
    }
    let names = song
        .patterns
        .iter()
        .enumerate()
        .map(|(b, p)| format!("{b:02} {}", p.name))
        .collect();
    (previews, col_instr, used, names)
}

/// what the painter reads from the engine and mixer once per frame
struct Live {
    playing: bool,
    /// playhead tick (None: stopped, or a looped block that is not in the order)
    head: Option<f64>,
    bpm: f32,
    mix: Vec<ChanMix>,
    peaks: Vec<f32>,
    any_solo: bool,
}

/// adaptive bar-label step: 1, 2, 4, 8 ... bars so labels stay >= `min_px` apart
fn bar_step(bar_px: f32, min_px: f32) -> i64 {
    let mut s = 1;
    while (s as f32) * bar_px < min_px && s < 1 << 20 {
        s *= 2;
    }
    s
}

/// readable text on a clip title bar of colour `c`
pub(super) fn title_ink(c: Color32) -> Color32 {
    if theme::luma(c) > 0.58 {
        Color32::from_rgb(0x14, 0x16, 0x19)
    } else {
        Color32::WHITE
    }
}

fn mmss(secs: f64) -> String {
    let s = secs.max(0.0);
    format!("{}:{:04.1}", (s / 60.0) as u32, s % 60.0)
}

fn quad(mesh: &mut egui::Mesh, r: Rect, c: Color32) {
    if r.width() > 0.0 && r.height() > 0.0 {
        mesh.add_colored_rect(r, c);
    }
}

impl App {
    /// derived timeline for the current song (rebuilt when the song changed)
    fn timeline_refresh(&mut self) -> Live {
        // design A: pin remaining song lanes onto tracks / Song-Master
        if self.with(|s| s.song.timeline.is_some() && !s.song.automation.is_empty()) {
            self.with(|s| {
                let n = crate::timeline::migrate_song_lanes(&mut s.song);
                if n > 0 {
                    s.engine.ensure_plan(&s.song);
                }
            });
        }
        let shared = self.shared.clone();
        let s = audio::lock(&shared);
        let key = fingerprint(&s.song);
        if self.tlv.cache.as_ref().map(|c| c.key) != Some(key) {
            let old = self.tlv.cache.take();
            self.tlv.cache = Some(build_cache(&s.song, key, old));
            // clip ids may be gone
            if let Some(c) = &self.tlv.cache {
                self.tlv
                    .sels
                    .retain(|id| c.tl.clips.iter().any(|x| x.id == *id));
                if self.tlv.sel.is_some_and(|id| !self.tlv.sels.contains(&id)) {
                    self.tlv.sel = self.tlv.sels.last().copied();
                }
            }
        }
        let c = self.tlv.cache.as_ref().unwrap();
        let e = &s.engine;
        let head = if !e.playing {
            None
        } else if c.mode {
            e.tl_row().map(|_| e.song_pos as f64 * ROW as f64)
        } else if e.loop_pattern {
            c.order
                .iter()
                .position(|&b| b == e.pattern)
                .map(|k| c.map.tick_of_song_pos(k, e.song_pos))
        } else {
            Some(c.map.tick_of_song_pos(e.order_pos, e.song_pos))
        };
        let n = c.tl.tracks.len();
        Live {
            playing: e.playing,
            head,
            bpm: s.song.bpm,
            mix: s.song.mixer.channels.iter().take(n).copied().collect(),
            peaks: e.chan_peak.iter().take(n).copied().collect(),
            any_solo: s.song.mixer.channels.iter().take(n).any(|m| m.solo),
        }
    }

    /// Ctrl+T view: toolbar, ruler, track headers, clips
    pub(super) fn timeline_view(&mut self, ui: &mut egui::Ui) {
        let t_start = std::time::Instant::now();
        let live = self.timeline_refresh();
        let t = th();
        let mode = self.tlv.cache.as_ref().unwrap().mode;
        let cue_tick = {
            let c = self.tlv.cache.as_ref().unwrap();
            if mode {
                self.tlv.cue
            } else {
                c.map.tick_of(self.cue.0, self.cue.1)
            }
        };
        let (n_tracks, n_clips, end) = {
            let c = self.tlv.cache.as_ref().unwrap();
            (
                c.tl.tracks.len(),
                c.tl.clips.len(),
                if mode {
                    c.end
                } else {
                    c.map.end().max(c.tl.end())
                },
            )
        };
        // ---- toolbar
        let mut fit = false;
        ui.horizontal(|ui| {
            // a slim tool strip (0.2.4 polish): tighter buttons, one line
            let sp = ui.spacing_mut();
            sp.button_padding = egui::vec2(5.0, 0.0);
            sp.item_spacing.x = 5.0;
            sp.interact_size.y = 17.0;
            ui.label(
                egui::RichText::new(" TIMELINE ")
                    .strong()
                    .monospace()
                    .size(10.5)
                    .color(Color32::from_rgb(0x1A, 0x1C, 0x1F))
                    .background_color(t.accent),
            )
            .on_hover_text(if mode && !self.tracker_on() {
                "This song is arranged here: clips on tracks (track = mixer channel). \
                 Drag to move (Alt: copy, Shift: by rows), drag an edge to trim or loop, \
                 S split, Ctrl+D duplicate, M mute, Del delete, Ctrl+J glue, \
                 Ctrl+Shift+D make unique. Double-click a clip to edit it in the piano roll, \
                 empty space to make a new note clip."
            } else if mode {
                "This song is arranged here: clips on tracks (track = mixer channel). \
                 Drag to move (Alt: copy, Shift: by rows), drag an edge to trim or loop, \
                 S split, Ctrl+D duplicate, M mute, Del delete, Ctrl+J glue, \
                 Ctrl+Shift+D make unique. Double-click a clip to edit it in the piano roll \
                 (Alt+double-click or Enter: the Tracker; Settings can swap them), \
                 empty space to write a new block. Song › Back to tracker mode… turns it back."
            } else {
                "This tracker-mode song's order list, shown as clips per track."
            });
            // timeline mode needs no label: Song › Back to tracker
            // mode… turns it back
            if !mode {
                ui.label(egui::RichText::new("Tracker mode · from the order list").color(t.ink2))
                    .on_hover_text(
                        "The timeline shows this song's order list as clips per track (track = mixer channel). \
                         To move, loop and split clips here, switch the song to timeline mode. \
                         Double-click a clip to edit it in the piano roll (Alt+double-click or Enter: \
                         the Tracker; Settings can swap them).",
                    );
                if ui
                    .small_button("Switch to timeline mode")
                    .on_hover_text("arrange this song with clips (asks first; Undo puts it back)")
                    .clicked()
                {
                    self.tlv.ask_switch = Some(None);
                }
            }
            ui.separator();
            ui.checkbox(&mut self.follow, "Follow")
                .on_hover_text("keep the playhead in view while playing");
            ui.label(egui::RichText::new("zoom").color(t.ink2));
            if ui.small_button("−").on_hover_text("zoom out (-)").clicked() {
                self.tlv.zoom_by(0.8, None);
            }
            if ui.small_button("+").on_hover_text("zoom in (+)").clicked() {
                self.tlv.zoom_by(1.25, None);
            }
            if ui
                .small_button("Fit")
                .on_hover_text("show the whole song (F)")
                .clicked()
            {
                fit = true;
            }
            ui.separator();
            if ui
                .selectable_label(!self.tlv.time_ruler, "Bars")
                .on_hover_text("ruler in bars")
                .clicked()
            {
                self.tlv.time_ruler = false;
            }
            if ui
                .selectable_label(self.tlv.time_ruler, "Time")
                .on_hover_text("ruler in minutes:seconds")
                .clicked()
            {
                self.tlv.time_ruler = true;
            }
            ui.separator();
            ui.label(egui::RichText::new("snap").color(t.ink2));
            egui::ComboBox::from_id_salt("tl_snap")
                .width(74.0)
                .selected_text(match self.tlv.snap {
                    Snap::Adaptive => "Auto",
                    Snap::Bar => "Bar",
                    Snap::Beat => "Beat",
                    Snap::Row => "Row",
                })
                .show_ui(ui, |ui| {
                    for (v, l) in [
                        (Snap::Adaptive, "Auto (bar, beat or row by zoom)"),
                        (Snap::Bar, "Bar"),
                        (Snap::Beat, "Beat"),
                        (Snap::Row, "Row"),
                    ] {
                        ui.selectable_value(&mut self.tlv.snap, v, l);
                    }
                })
                .response
                .on_hover_text("grid for moving and resizing clips (hold Shift: rows)");
            let (has_loop, loop_on) = {
                let c = self.tlv.cache.as_ref().unwrap();
                (c.tl.loop_range.is_some(), c.tl.loop_on)
            };
            if ui
                .selectable_label(mode && loop_on, "Loop")
                .on_hover_text(if has_loop {
                    "play the loop region over and over (L) · Shift-drag the ruler to set it · Ctrl+L: around the selection"
                } else {
                    "no loop region yet: Shift-drag the ruler, or select clips and press Ctrl+L"
                })
                .clicked()
            {
                self.tl_apply(TlOp::ToggleLoop);
            }
            if ui
                .small_button("+ Marker")
                .on_hover_text("add a marker at the play marker (Shift+M)")
                .clicked()
            {
                let at = live.head.map_or(cue_tick, |h| tl::snap(h as Tick, ROW));
                self.tl_apply(TlOp::AddMarker(at));
            }
            ui.separator();
            let pos = live.head.unwrap_or(cue_tick as f64) as Tick;
            let c = self.tlv.cache.as_ref().unwrap();
            let bp = c.tl.bar_pos(pos);
            ui.label(
                egui::RichText::new(format!(
                    "{}.{} · {}",
                    bp.bar,
                    bp.beat,
                    mmss(c.tl.seconds_at(pos, live.bpm))
                ))
                .monospace()
                .color(if live.playing { t.play } else { t.ink }),
            )
            .on_hover_text(format!(
                "playhead (stopped: the play marker) in bar.beat and minutes:seconds\n{n_tracks} tracks · {n_clips} clips · {}",
                mmss(c.tl.seconds_at(end, live.bpm))
            ));
        });
        ui.add_space(2.0);
        let full = ui.available_rect_before_wrap();
        if full.width() < HEAD_W + 80.0 || full.height() < MARK_H + RULER_H + OVERVIEW_H + 30.0 {
            return;
        }
        let heads = Rect::from_min_max(
            Pos2::new(full.left(), full.top() + MARK_H + RULER_H + 2.0),
            Pos2::new(full.left() + HEAD_W, full.bottom() - OVERVIEW_H - 4.0),
        );
        let x_l = full.left() + HEAD_W + 4.0;
        let x_r = full.right() - VBAR_W - 2.0;
        let marks = Rect::from_min_max(
            Pos2::new(x_l, full.top()),
            Pos2::new(x_r, full.top() + MARK_H),
        );
        let ruler = Rect::from_min_max(
            Pos2::new(x_l, marks.bottom()),
            Pos2::new(x_r, marks.bottom() + RULER_H),
        );
        let lanes = Rect::from_min_max(Pos2::new(x_l, heads.top()), Pos2::new(x_r, heads.bottom()));
        let overview = Rect::from_min_max(
            Pos2::new(x_l, full.bottom() - OVERVIEW_H),
            Pos2::new(x_r, full.bottom()),
        );
        let vbar = Rect::from_min_max(
            Pos2::new(x_r + 2.0, lanes.top()),
            Pos2::new(full.right(), lanes.bottom()),
        );
        self.tlv.lanes = lanes;
        self.tlv.ruler = ruler;
        self.tlv.heads = heads;
        // ---- zoom and scroll bounds
        let content_end = end.max(tl::rows_to_ticks(ROWS)) as f64;
        let fit_zoom = |w: f32| (w - 24.0) / (content_end / PPQ as f64).max(8.0) as f32;
        if self.tlv.new_song {
            // a new song: 16 bars across, its empty first block at the left
            self.tlv.zoom = (lanes.width() / (16.0 * 4.0)).clamp(MIN_ZOOM, MAX_ZOOM);
            self.tlv.new_song = false;
        }
        if fit || self.tlv.zoom <= 0.0 {
            self.tlv.zoom = fit_zoom(lanes.width()).clamp(MIN_ZOOM, MAX_ZOOM);
            self.tlv.scroll = 0.0;
        }
        let lane_h = self.tlv.th();
        let lay = autolanes::Layout::build(&self.tlv.cache.as_ref().unwrap().tl, mode, lane_h);
        let total_h = lay.total();
        let max_sy = (total_h - lanes.height()).max(0.0);
        // ---- wheel: tracks / Shift: time / Ctrl: zoom at the mouse / Alt: track height
        let hover = ui.input(|i| i.pointer.hover_pos());
        let over = hover.filter(|p| {
            lanes.contains(*p) || heads.contains(*p) || ruler.contains(*p) || overview.contains(*p)
        });
        if let Some(p) = over {
            let (ev, m) = ui.input(|i| (i.events.clone(), i.modifiers));
            let lines = input::wheel_lines(&ev);
            let dx: f32 = ev
                .iter()
                .filter_map(|e| match e {
                    egui::Event::MouseWheel { delta, unit, .. } if delta.y != 0.0 => {
                        Some(match unit {
                            egui::MouseWheelUnit::Point => delta.x,
                            egui::MouseWheelUnit::Line => delta.x * 40.0,
                            egui::MouseWheelUnit::Page => delta.x * 400.0,
                        })
                    }
                    _ => None,
                })
                .sum();
            if lines != 0.0 || dx != 0.0 {
                if m.command {
                    let n = input::take_notches(&mut self.tlv.wheel, lines);
                    if n != 0 {
                        self.tlv.zoom_by(1.2f32.powi(n), Some(p.x - lanes.left()));
                    }
                } else if m.alt {
                    let n = input::take_notches(&mut self.tlv.wheel, lines);
                    self.tlv.track_h = (lane_h * 1.15f32.powi(n)).clamp(24.0, 140.0);
                } else if m.shift || max_sy <= 0.0 || ruler.contains(p) || overview.contains(p) {
                    self.tlv.scroll -= (lines * 40.0) as f64 * PPQ as f64 / self.tlv.zoom as f64;
                } else {
                    self.tlv.scroll_y -= lines * 40.0;
                }
                if dx != 0.0 && !m.command {
                    self.tlv.scroll -= dx as f64 * PPQ as f64 / self.tlv.zoom as f64;
                }
            }
        }
        let zoom = self.tlv.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.tlv.zoom = zoom;
        let ppt = zoom as f64 / PPQ as f64; // pixels per tick
        let vis_ticks = lanes.width() as f64 / ppt;
        self.tlv.head_drawn = live.head;
        // ---- follow the playhead (page so it sits a third in)
        if let Some(h) = live.head {
            if self.follow && !self.tlv.ruler_drag && !self.tlv.overview_drag {
                let s = self.tlv.scroll;
                if h < s || h > s + vis_ticks * 0.92 {
                    self.tlv.scroll = h - vis_ticks / 3.0;
                }
            }
        }
        let max_scroll = (content_end + vis_ticks * 0.25 - vis_ticks).max(0.0);
        self.tlv.scroll = self.tlv.scroll.clamp(0.0, max_scroll);
        self.tlv.scroll_y = self.tlv.scroll_y.clamp(0.0, max_sy);
        let scroll = self.tlv.scroll;
        let sy = self.tlv.scroll_y;
        let x_of = |tk: f64| lanes.left() + ((tk - scroll) * ppt) as f32;
        let tick_of = |x: f32| scroll + (x - lanes.left()) as f64 / ppt;
        let v0 = scroll.floor() as Tick;
        let v1 = (scroll + vis_ticks).ceil() as Tick + 1;
        let first_tr = lay.track_at(sy).min(n_tracks);
        let last_tr = (lay.track_at(sy + lanes.height()) + 1).min(n_tracks);
        let lane_rect = |tr: usize| {
            let y = lanes.top() + lay.top[tr] - sy;
            Rect::from_min_max(
                Pos2::new(lanes.left(), y),
                Pos2::new(lanes.right(), y + lane_h),
            )
        };
        let (song_lanes, units, song_for_lanes) = if lay.open.iter().any(|o| *o) {
            let (sl, u) = self.lanes_view_data();
            let song = self.with(|s| s.song.clone());
            (sl, u, Some(song))
        } else {
            (vec![], vec![], None)
        };
        let mut auto_ops: Vec<autolanes::AutoOp> = vec![];
        let painter = ui.painter_at(full);
        // text on the dark CRT areas (ruler, lanes, overview): the theme inks
        // are dark in light themes (Amiga), so lift them there
        let lum = |c: Color32| {
            (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
        };
        let crt_dark = lum(t.crt_top) < 0.35;
        let crt_ink = if crt_dark && lum(t.ink2) < 0.45 {
            Color32::from_rgb(0xB4, 0xC4, 0xDC)
        } else {
            t.ink2
        };
        let crt_ink_hi = if crt_dark && lum(t.ink) < 0.45 {
            Color32::from_rgb(0xEC, 0xF1, 0xF8)
        } else {
            t.ink
        };
        // backgrounds
        painter.rect_filled(lanes, 4, t.crt_bot);
        painter.rect_filled(ruler.union(marks), 3, t.crt_top);
        painter.rect_filled(heads, 4, t.cart_bot);
        let lp = ui.painter_at(lanes);
        let c = self.tlv.cache.as_ref().unwrap();
        // ---- grid and ruler
        let bar_ticks = c.tl.sig_at(v0.max(0)).bar_ticks();
        let bar_px = bar_ticks as f32 * ppt as f32;
        let label_step = bar_step(bar_px, 46.0);
        let line_step = bar_step(bar_px, 7.0);
        let mut grid = egui::Mesh::default();
        let beat_px = PPQ as f32 * ppt as f32;
        if beat_px >= 12.0 {
            let mut b = (v0.max(0) / PPQ) * PPQ;
            while b < v1 {
                let x = x_of(b as f64);
                quad(
                    &mut grid,
                    Rect::from_min_max(
                        Pos2::new(x, lanes.top()),
                        Pos2::new(x + 1.0, lanes.bottom()),
                    ),
                    t.crt_grid.gamma_multiply(0.55),
                );
                b += PPQ;
            }
        }
        let rp = ui.painter_at(ruler);
        for (bt, bar) in c.tl.bars_in(v0.max(0), v1) {
            let x = x_of(bt as f64);
            if (bar - 1) % line_step == 0 {
                quad(
                    &mut grid,
                    Rect::from_min_max(
                        Pos2::new(x, lanes.top()),
                        Pos2::new(x + 1.0, lanes.bottom()),
                    ),
                    t.crt_grid,
                );
            }
            let major = (bar - 1) % label_step == 0;
            if major || (bar - 1) % line_step == 0 {
                rp.vline(
                    x,
                    ruler.top() + if major { 2.0 } else { 12.0 }..=ruler.bottom(),
                    Stroke::new(1.0, crt_ink.gamma_multiply(if major { 0.8 } else { 0.4 })),
                );
            }
            if major {
                let label = if self.tlv.time_ruler {
                    mmss(c.tl.seconds_at(bt, live.bpm))
                } else {
                    bar.to_string()
                };
                rp.text(
                    Pos2::new(x + 3.0, ruler.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    FontId::monospace(11.0),
                    crt_ink,
                );
            }
        }
        lp.add(egui::Shape::mesh(grid));
        // loop region: a band on the ruler, a tint over the lanes
        if let Some((a, b)) = c.tl.loop_range {
            let (xa, xb) = (x_of(a as f64), x_of(b as f64));
            if xb > ruler.left() && xa < ruler.right() {
                let on = c.tl.loop_on && mode;
                let lc = if on { t.accent } else { crt_ink };
                rp.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(xa, ruler.bottom() - 6.0),
                        Pos2::new(xb, ruler.bottom()),
                    ),
                    1,
                    lc.gamma_multiply(if on { 0.85 } else { 0.35 }),
                );
                if on {
                    lp.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(xa, lanes.top()),
                            Pos2::new(xb, lanes.bottom()),
                        ),
                        0,
                        lc.gamma_multiply(0.05),
                    );
                    for x in [xa, xb] {
                        lp.vline(x, lanes.y_range(), Stroke::new(1.0, lc.gamma_multiply(0.5)));
                    }
                }
            }
        }
        // ---- lanes: alternate shading, then clips (one mesh for bodies + notes)
        let mut bodies = egui::Mesh::default();
        let mut notes = egui::Mesh::default();
        let mut texts: Vec<(Rect, Pos2, String, Color32)> = vec![];
        // audio clips: fade curves and handles (rect, header height, clip, hot handle)
        let mut fades: Vec<(Rect, f32, tl::Clip, Option<bool>)> = vec![];
        let mut outlines: Vec<(Rect, Stroke)> = vec![];
        let pol = theme::polished();
        let mut glows: Vec<Rect> = vec![];
        for tr in first_tr..last_tr {
            let lr = lane_rect(tr);
            if tr % 2 == 1 {
                quad(&mut bodies, lr, Color32::from_white_alpha(4));
            }
            quad(
                &mut bodies,
                Rect::from_min_max(Pos2::new(lr.left(), lr.bottom() - 1.0), lr.max),
                t.crt_grid,
            );
        }
        let hover_clip = hover
            .filter(|p| lanes.contains(*p))
            .and_then(|p| clip_at(c, &lay, p, lanes, sy, tick_of(p.x)));
        // empty block columns of the order list: faint slots (new songs start here)
        let k0 = c.map.starts.partition_point(|&s| s <= v0).saturating_sub(1);
        let k1 = c.map.starts.partition_point(|&s| s < v1);
        // (only while the song is nearly empty: in a full song they'd be noise;
        // double-click on an empty spot opens the block there either way)
        if !mode && n_clips < 8 && (k1.saturating_sub(k0)) * (last_tr - first_tr) <= 4000 {
            for k in k0..k1 {
                let b = c.order[k];
                for tr in first_tr..last_tr {
                    if c.used
                        .get(b)
                        .and_then(|u| u.get(tr))
                        .copied()
                        .unwrap_or(true)
                    {
                        continue;
                    }
                    let lr = lane_rect(tr);
                    let r = Rect::from_min_max(
                        Pos2::new(x_of(c.map.starts[k] as f64) + 1.0, lr.top() + 3.0),
                        Pos2::new(
                            x_of((c.map.starts[k] + tl::rows_to_ticks(c.map.lens[k])) as f64) - 1.0,
                            lr.bottom() - 3.0,
                        ),
                    );
                    outlines.push((r, Stroke::new(1.0, crt_ink.gamma_multiply(0.18))));
                    if r.width() > 60.0 {
                        texts.push((
                            r,
                            r.left_top() + egui::vec2(6.0, 9.0),
                            c.names[b].clone(),
                            crt_ink.gamma_multiply(0.8),
                        ));
                        if r.height() > 30.0 {
                            texts.push((
                                r,
                                r.left_bottom() + egui::vec2(6.0, -9.0),
                                "empty · double-click to write".into(),
                                crt_ink.gamma_multiply(0.55),
                            ));
                        }
                    }
                }
            }
        }
        let px_row = ROW as f32 * ppt as f32;
        for tr in first_tr..last_tr {
            let lr = lane_rect(tr);
            let col = track_colour_in(&c.tl, tr);
            let muted = !live
                .mix
                .get(tr)
                .map(|m| !m.mute && (!live.any_solo || m.solo))
                .unwrap_or(true);
            for i in c.idx.visible(&c.tl, tr, v0, v1) {
                let clip = &c.tl.clips[i];
                let x0 = x_of(clip.start as f64);
                let x1 = x_of(clip.end() as f64);
                let r = Rect::from_min_max(
                    Pos2::new(x0 + 0.5, lr.top() + 2.0),
                    Pos2::new((x1 - 0.5).max(x0 + 1.5), lr.bottom() - 2.0),
                );
                let sel = self.tlv.sels.contains(&clip.id);
                let hot = hover_clip == Some(i);
                let k = if clip.muted {
                    0.22
                } else if muted {
                    0.35
                } else {
                    1.0
                };
                // solid title bar in the track colour over a softer body
                // (the musical data keeps the saturation; 0.2.4 polish)
                let body_c = theme::mix(t.crt_bot, col, if t.ember { 0.46 } else { 0.34 } * k);
                let head_c = theme::mix(
                    t.crt_bot,
                    if hot { theme::tone(col, 0.10) } else { col },
                    k.max(0.3),
                );
                let hh = (lane_h * 0.32).clamp(10.0, 16.0);
                let hr = Rect::from_min_max(r.min, Pos2::new(r.right(), r.top() + hh));
                if pol {
                    // rounded body with a gentle gradient, header rounded on top,
                    // a 1 px darker border; selected: a soft glow + outline
                    theme::round_mesh(
                        &mut bodies,
                        r,
                        [3.0; 4],
                        theme::tone(body_c, 0.06),
                        theme::tone(body_c, -0.14),
                    );
                    theme::round_mesh(
                        &mut bodies,
                        hr,
                        [3.0, 3.0, 0.0, 0.0],
                        theme::tone(head_c, 0.08),
                        theme::tone(head_c, -0.04),
                    );
                    if sel {
                        glows.push(r);
                        outlines.push((r, Stroke::new(2.0, t.selection_outline)));
                    } else {
                        outlines.push((r, Stroke::new(1.0, theme::tone(body_c, -0.45))));
                    }
                } else {
                    quad(&mut bodies, r, body_c);
                    quad(&mut bodies, hr, head_c);
                    // clip edge so neighbours stay apart
                    quad(
                        &mut bodies,
                        Rect::from_min_max(r.left_top(), Pos2::new(r.left() + 1.0, r.bottom())),
                        col.gamma_multiply(1.1 * k).to_opaque(),
                    );
                    if sel {
                        outlines.push((r, Stroke::new(2.0, t.selection_outline)));
                    }
                }
                // loop notches in the header
                if clip.loop_len.is_some() && r.width() > 8.0 {
                    for lt in clip.loop_points() {
                        if lt < v0 || lt > v1 {
                            continue;
                        }
                        let x = x_of(lt as f64);
                        quad(
                            &mut bodies,
                            Rect::from_min_max(
                                Pos2::new(x - 0.5, hr.top()),
                                Pos2::new(x + 0.5, r.bottom()),
                            ),
                            Color32::from_black_alpha(90),
                        );
                        quad(
                            &mut bodies,
                            Rect::from_min_max(
                                Pos2::new(x - 2.0, hr.top()),
                                Pos2::new(x + 2.0, hr.top() + 3.0),
                            ),
                            Color32::from_white_alpha(140),
                        );
                    }
                }
                if let tl::ClipKind::Audio { asset } = clip.kind {
                    let a = c.tl.audio.get(asset as usize);
                    let missing = a.is_none_or(|a| a.missing());
                    if missing {
                        quad(
                            &mut bodies,
                            Rect::from_min_max(Pos2::new(r.left(), hr.bottom()), r.max),
                            t.brick.gamma_multiply(0.35 * k),
                        );
                    }
                    if r.width() > 26.0 {
                        let vis_l = r.left().max(lanes.left()) + 4.0;
                        let mut name = a.map_or("audio".into(), |a| a.name.clone());
                        if missing {
                            name.push_str(" · missing: right-click > Locate");
                        }
                        if clip.gain != 1.0 {
                            name.push_str(&format!(
                                " · {:+.1} dB",
                                20.0 * clip.gain.max(1e-6).log10()
                            ));
                        }
                        if clip.muted {
                            name.push_str(" · muted");
                        }
                        texts.push((
                            r,
                            Pos2::new(
                                vis_l + if clip.fade_in == 0 { 8.0 } else { 0.0 },
                                hr.center().y,
                            ),
                            name,
                            title_ink(head_c),
                        ));
                    }
                    let body = Rect::from_min_max(
                        Pos2::new(r.left() + 1.0, hr.bottom() + 2.0),
                        Pos2::new(r.right() - 1.0, r.bottom() - 2.0),
                    );
                    if r.width() >= 2.0 && body.height() >= 4.0 && !missing {
                        let vis = body.intersect(lanes);
                        audioclip::paint_waveform(
                            &mut notes,
                            vis,
                            clip,
                            &c.tl,
                            live.bpm,
                            &x_of,
                            &tick_of,
                            r,
                            col.gamma_multiply(1.6 * k).to_opaque(),
                        );
                    }
                    if mode && (sel || hot) {
                        let hot_h =
                            hover.and_then(|p| audioclip::fade_handle_at(p, r, hh, clip, &x_of));
                        fades.push((r, hh, clip.clone(), hot_h.filter(|_| hot)));
                    } else if clip.fade_in > 0 || clip.fade_out > 0 {
                        fades.push((r, hh, clip.clone(), None));
                    }
                    continue;
                }
                if let tl::ClipKind::Notes { data } = clip.kind {
                    let Some(d) = c.tl.notes.get(data as usize) else {
                        continue;
                    };
                    if r.width() > 26.0 {
                        let vis_l = r.left().max(lanes.left()) + 4.0;
                        let mut name = d.name.clone();
                        if clip.muted {
                            name.push_str(" · muted");
                        }
                        texts.push((r, Pos2::new(vis_l, hr.center().y), name, title_ink(head_c)));
                    }
                    let body = Rect::from_min_max(
                        Pos2::new(r.left() + 1.0, hr.bottom() + 2.0),
                        Pos2::new(r.right() - 1.0, r.bottom() - 2.0),
                    );
                    if r.width() < 6.0 || body.height() < 5.0 || d.notes.is_empty() {
                        continue;
                    }
                    let (lo, hi) = d
                        .notes
                        .iter()
                        .fold((u8::MAX, 0u8), |(a, b), n| (a.min(n.key), b.max(n.key)));
                    let range = (hi - lo + 1).max(12) as f32;
                    let lo = lo.saturating_sub(((range as u8).saturating_sub(hi - lo + 1)) / 2);
                    let nh = (body.height() / range).clamp(1.5, 4.0);
                    let note_c = col.gamma_multiply(1.6 * k).to_opaque();
                    // the notes selected in the piano roll, in its colour
                    let roll_sel: &[usize] = match self.pr.target {
                        Some(super::pianoview::Target::Notes { clip: Some(id), .. })
                            if id == clip.id && self.pr.open =>
                        {
                            &self.pr.sel
                        }
                        _ => &[],
                    };
                    let sel_c = super::pianoview::sel_note_colours(&t, 1.0).0;
                    let l = clip.loop_len.unwrap_or(Tick::MAX / 4);
                    let p0 = clip.pass_at(clip.start.max(v0));
                    let p1 = clip.pass_at((clip.end() - 1).min(v1));
                    for pass in p0..=p1.min(p0 + 10_000) {
                        let base = clip.start - clip.offset + pass as Tick * l;
                        for (ni, n) in d.notes.iter().enumerate() {
                            if clip.loop_len.is_some() && n.start >= l {
                                continue;
                            }
                            let a = base + n.start;
                            let e = (a + n.len).min(base + l).min(clip.end());
                            if e <= clip.start.max(v0) || a >= clip.end().min(v1) || a < clip.start
                            {
                                continue;
                            }
                            let xa = x_of(a as f64).max(body.left());
                            let xe = x_of(e as f64).min(body.right()).max(xa + 1.0);
                            let y = body.bottom()
                                - ((n.key.saturating_sub(lo)) as f32 + 0.5) / range * body.height();
                            quad(
                                &mut notes,
                                Rect::from_min_max(
                                    Pos2::new(xa, y - nh * 0.5),
                                    Pos2::new(xe - 0.5, y + nh * 0.5),
                                ),
                                if roll_sel.contains(&ni) {
                                    sel_c
                                } else {
                                    note_c
                                },
                            );
                        }
                    }
                    continue;
                }
                let Some((b, colm)) = clip.block() else {
                    continue;
                };
                if r.width() > 26.0 {
                    let vis_l = r.left().max(lanes.left()) + 4.0;
                    let mut name = c.names.get(b).cloned().unwrap_or_default();
                    if clip.muted {
                        name.push_str(" · muted");
                    }
                    texts.push((r, Pos2::new(vis_l, hr.center().y), name, title_ink(head_c)));
                }
                // note preview
                let body = Rect::from_min_max(
                    Pos2::new(r.left() + 1.0, hr.bottom() + 2.0),
                    Pos2::new(r.right() - 1.0, r.bottom() - 2.0),
                );
                if r.width() < 6.0 || body.height() < 5.0 {
                    continue;
                }
                let Some(pv) = c.previews.get(&(b, colm)) else {
                    continue;
                };
                let range = (pv.hi - pv.lo + 1) as f32;
                let nh = (body.height() / range).clamp(1.5, 4.0);
                let note_c = col.gamma_multiply(1.6 * k).to_opaque();
                let l = clip.loop_len.unwrap_or(Tick::MAX / 4);
                let p0 = clip.pass_at(clip.start.max(v0));
                let p1 = clip.pass_at((clip.end() - 1).min(v1));
                let decimate = px_row < 1.0;
                for pass in p0..=p1 {
                    let base = clip.start - clip.offset + pass as Tick * l;
                    let mut last_x = f32::NEG_INFINITY;
                    for &(row, len, key) in &pv.notes {
                        let a = base + row as Tick * ROW;
                        let e = (a + len as Tick * ROW).min(base + l).min(clip.end());
                        if e <= clip.start.max(v0) || a >= clip.end().min(v1) {
                            continue;
                        }
                        let a = a.max(clip.start);
                        let xa = x_of(a as f64).max(body.left());
                        if decimate && xa - last_x < 1.0 {
                            continue;
                        }
                        last_x = xa;
                        let xe = x_of(e as f64).min(body.right()).max(xa + 1.0);
                        let y =
                            body.bottom() - ((key - pv.lo) as f32 + 0.5) / range * body.height();
                        quad(
                            &mut notes,
                            Rect::from_min_max(
                                Pos2::new(xa, y - nh * 0.5),
                                Pos2::new(xe - 0.5, y + nh * 0.5),
                            ),
                            note_c,
                        );
                    }
                }
            }
        }
        lp.add(egui::Shape::mesh(bodies));
        lp.add(egui::Shape::mesh(notes));
        for (r, hh, clip, hot) in &fades {
            let fp = lp.with_clip_rect(r.intersect(lanes));
            audioclip::paint_fades(&fp, *r, *hh, clip, &x_of, *hot);
        }
        for r in glows {
            theme::glow(&lp, r, 3, t.selection_outline);
        }
        let orad: u8 = if pol { 3 } else { 2 };
        for (r, s) in outlines {
            lp.rect_stroke(r, orad, s, egui::StrokeKind::Inside);
        }
        for (r, p, txt, colr) in texts {
            let tp = lp.with_clip_rect(r.intersect(lanes).shrink(1.0));
            tp.text(
                p,
                egui::Align2::LEFT_CENTER,
                txt,
                FontId::proportional(11.0),
                colr,
            );
        }
        // ---- markers: each one a section band up to the next (0.2.4 polish)
        let mp = ui.painter_at(marks);
        let song_end = if mode { c.end } else { c.map.end() };
        let nm = c.tl.markers.len();
        for (i, m) in c.tl.markers.iter().enumerate() {
            let next = c.tl.markers[i + 1..]
                .iter()
                .map(|n| n.tick)
                .find(|&tk| tk > m.tick)
                .unwrap_or(song_end.max(m.tick + bar_ticks));
            if next < v0 || m.tick > v1 {
                continue;
            }
            let x = x_of(m.tick as f64);
            let xe = x_of(next as f64);
            let colr = t.ochre;
            let band = Rect::from_min_max(
                Pos2::new(x + 1.0, marks.top() + 1.0),
                Pos2::new((xe - 1.0).max(x + 4.0), marks.bottom() - 1.0),
            );
            mp.rect_filled(
                band,
                if pol { 3 } else { 0 },
                crt_ink.gamma_multiply(if i % 2 == 0 { 0.13 } else { 0.08 }),
            );
            mp.rect_filled(
                Rect::from_min_max(band.min, Pos2::new(x + 3.0, band.bottom())),
                if pol { 1 } else { 0 },
                colr,
            );
            let lx = x.max(marks.left() + 2.0) + 6.0;
            if lx < band.right() - 6.0 || nm == 1 {
                mp.with_clip_rect(band.intersect(marks)).text(
                    Pos2::new(lx, marks.center().y),
                    egui::Align2::LEFT_CENTER,
                    &m.name,
                    FontId::proportional(11.0),
                    crt_ink_hi,
                );
            }
            lp.vline(
                x,
                lanes.y_range(),
                Stroke::new(1.0, colr.gamma_multiply(0.25)),
            );
        }
        // captions for the marker and ruler rows, over the header column
        {
            let cap = Rect::from_min_max(
                Pos2::new(full.left(), full.top()),
                Pos2::new(full.left() + HEAD_W, ruler.bottom()),
            );
            let cp = ui.painter_at(cap);
            cp.rect_filled(
                cap.shrink2(egui::vec2(0.0, 1.0)),
                3,
                t.cart_bot.gamma_multiply(0.7),
            );
            for (r, txt) in [
                (marks, "Markers"),
                (ruler, if self.tlv.time_ruler { "Time" } else { "Bars" }),
            ] {
                cp.text(
                    Pos2::new(cap.right() - 8.0, r.center().y),
                    egui::Align2::RIGHT_CENTER,
                    txt,
                    FontId::proportional(10.5),
                    t.ink2,
                );
            }
            if nm == 0 && mode {
                cp.text(
                    Pos2::new(cap.left() + 8.0, marks.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Shift+M adds one",
                    FontId::proportional(10.0),
                    t.ink2.gamma_multiply(0.7),
                );
            }
        }
        // ---- playhead / play marker
        let ph = live.head.unwrap_or(cue_tick as f64);
        let phx = x_of(ph);
        if phx >= lanes.left() - 1.0 && phx <= lanes.right() + 1.0 {
            let colr = if live.head.is_some() {
                t.play
            } else {
                t.accent
            };
            if t.ember && live.head.is_some() {
                // Slate: a 2 px acc-hot playhead with a soft bloom
                crate::ember::glow_line(
                    &lp,
                    &[Pos2::new(phx, lanes.top()), Pos2::new(phx, lanes.bottom())],
                    2.0,
                    t.acc_hot,
                );
            } else {
                lp.vline(
                    phx,
                    lanes.y_range(),
                    Stroke::new(if live.head.is_some() { 1.5 } else { 1.0 }, colr),
                );
            }
            rp.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(phx - 5.0, ruler.top() + 4.0),
                    Pos2::new(phx + 5.0, ruler.top() + 4.0),
                    Pos2::new(phx, ruler.bottom() - 2.0),
                ],
                colr,
                Stroke::NONE,
            ));
        }
        // end of song
        let ex = x_of(if mode { c.end } else { c.map.end() } as f64);
        if ex < lanes.right() {
            lp.rect_filled(
                Rect::from_min_max(Pos2::new(ex, lanes.top()), lanes.max),
                0,
                Color32::from_black_alpha(50),
            );
        }
        // ---- overview (mini-map of the whole song) and scroll position
        let op = ui.painter_at(overview);
        op.rect_filled(overview, 3, t.crt_top);
        let o_ppt = overview.width() as f64 / content_end.max(1.0);
        let rows_h = ((overview.height() - 4.0) / n_tracks.max(1) as f32).max(0.5);
        let mut om = egui::Mesh::default();
        for clip in &c.tl.clips {
            let y = overview.top() + 2.0 + clip.track as f32 * rows_h;
            let x0 = overview.left() + (clip.start as f64 * o_ppt) as f32;
            let x1 = overview.left() + (clip.end() as f64 * o_ppt) as f32;
            quad(
                &mut om,
                Rect::from_min_max(
                    Pos2::new(x0, y),
                    Pos2::new(x1.max(x0 + 1.0), y + rows_h.max(1.0)),
                ),
                track_colour_in(&c.tl, clip.track as usize).gamma_multiply(0.75),
            );
        }
        op.add(egui::Shape::mesh(om));
        let win = Rect::from_min_max(
            Pos2::new(overview.left() + (scroll * o_ppt) as f32, overview.top()),
            Pos2::new(
                overview.left() + ((scroll + vis_ticks) * o_ppt) as f32,
                overview.bottom(),
            ),
        );
        op.rect_stroke(
            win,
            2,
            Stroke::new(1.5, crt_ink_hi.gamma_multiply(0.8)),
            egui::StrokeKind::Inside,
        );
        if let Some(h) = live.head {
            let x = overview.left() + (h * o_ppt) as f32;
            op.vline(x, overview.y_range(), Stroke::new(1.0, t.play));
        }
        // ---- vertical scroll bar
        if max_sy > 0.0 {
            let vp = ui.painter_at(vbar);
            vp.rect_filled(vbar, 3, t.crt_top);
            let f0 = sy / total_h;
            let f1 = (sy + lanes.height()) / total_h;
            let th_r = Rect::from_min_max(
                Pos2::new(vbar.left() + 2.0, vbar.top() + f0 * vbar.height()),
                Pos2::new(vbar.right() - 2.0, vbar.top() + f1 * vbar.height()),
            );
            vp.rect_filled(th_r, 3, t.ink2.gamma_multiply(0.6));
        }
        // ---- track headers
        let hp = ui.painter_at(heads);
        let mut head_ops: Vec<HeadOp> = vec![];
        let mut rename_at: Option<Rect> = None;
        let mut pie_at: Option<(usize, Pos2)> = None;
        let mut hd = self.tlv.head_drag;
        // the subtitle: the instrument the track mostly plays, else the one
        // it was given (Browser drop, Add track with instrument)
        let given: Vec<String> = {
            let names: Vec<String> =
                self.with(|s| s.song.instruments.iter().map(|i| i.name.clone()).collect());
            c.tl.tracks
                .iter()
                .map(|t| match t.instr {
                    0 => String::new(),
                    k => names
                        .get(k as usize - 1)
                        .map(|n| format!("{k:02X} {n}"))
                        .unwrap_or_default(),
                })
                .collect()
        };
        // type tags per instrument slot (index 0 = none) for the icon tiles
        let kinds: Vec<&'static str> = self.with(|s| {
            std::iter::once("")
                .chain(
                    s.song
                        .instruments
                        .iter()
                        .map(|i| super::params::type_tag(&i.kind)),
                )
                .collect()
        });
        let sub = |tr: usize| -> String {
            // Master / Song tracks sit past mixer channels — c.instr is nch long
            if c.instr.get(tr).is_none_or(|s| s.is_empty()) {
                given.get(tr).cloned().unwrap_or_default()
            } else {
                c.instr[tr].clone()
            }
        };
        for tr in first_tr..last_tr {
            let y = heads.top() + lay.top[tr] - sy;
            let hr = Rect::from_min_max(
                Pos2::new(heads.left(), y + 1.0),
                Pos2::new(heads.right(), y + lane_h - 1.0),
            );
            if hr.bottom() < heads.top() || hr.top() > heads.bottom() {
                continue;
            }
            let colr = track_colour_in(&c.tl, tr);
            let cur = self.ch == tr;
            hp.rect_filled(
                hr,
                3,
                if cur {
                    t.sel.gamma_multiply(0.35)
                } else if tr % 2 == 1 {
                    t.cart_top.gamma_multiply(0.9)
                } else {
                    t.cart_top.gamma_multiply(0.75)
                },
            );
            hp.rect_filled(
                Rect::from_min_max(hr.min, Pos2::new(hr.left() + 3.0, hr.bottom())),
                2,
                colr,
            );
            let m = live.mix.get(tr).copied().unwrap_or_default();
            let name = &c.tl.tracks[tr].name;
            let one_line = lane_h < 36.0;
            let ty = if one_line {
                hr.center().y
            } else {
                hr.top() + 12.0
            };
            // icon tile: the instrument type's glyph on the track colour
            let ts = if one_line {
                (hr.height() - 6.0).clamp(12.0, 22.0)
            } else {
                (hr.height() - 10.0).clamp(18.0, 28.0)
            };
            let tile_r = Rect::from_min_size(
                Pos2::new(hr.left() + 7.0, hr.center().y - ts / 2.0),
                egui::vec2(ts, ts),
            );
            let kind = match c.tl.tracks[tr].kind {
                tl::TrackKind::Audio => "Audio",
                tl::TrackKind::Master => "Master",
                tl::TrackKind::Note => kinds
                    .get(c.instr_ix.get(tr).copied().unwrap_or(0) as usize)
                    .copied()
                    .unwrap_or(""),
            };
            super::glyphs::tile(
                &hp,
                tile_r,
                super::glyphs::track_glyph(c.tl.tracks[tr].icon, kind),
                colr,
                title_ink(colr),
                pol,
            );
            let tx = tile_r.right() + 6.0;
            let np = hp.with_clip_rect(Rect::from_min_max(
                Pos2::new(tx, hr.top()),
                Pos2::new(hr.right() - 132.0, hr.bottom()),
            ));
            let num = hp.layout_no_wrap(
                if c.tl.tracks[tr].kind == tl::TrackKind::Master {
                    "M".to_string()
                } else {
                    format!("{:02}", tr + 1)
                },
                FontId::monospace(10.0),
                t.ink2,
            );
            let nw = num.size().x;
            np.galley(Pos2::new(tx, ty - num.size().y / 2.0), num, t.ink2);
            if self.track_rename.as_ref().is_some_and(|r| r.0 == tr) && !self.pie_open() {
                rename_at = Some(Rect::from_min_max(
                    Pos2::new(tx + nw + 2.0, ty - 10.0),
                    Pos2::new((hr.right() - 136.0).max(tx + nw + 60.0), ty + 10.0),
                ));
            } else {
                np.text(
                    Pos2::new(tx + nw + 5.0, ty),
                    egui::Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(13.0),
                    t.ink,
                );
            }
            if !one_line {
                np.text(
                    Pos2::new(tx, hr.top() + 28.0),
                    egui::Align2::LEFT_CENTER,
                    sub(tr),
                    FontId::proportional(10.5),
                    t.ink2,
                );
            }
            // M / S
            let bs = 17.0;
            let by = if one_line {
                hr.center().y - bs / 2.0
            } else {
                hr.top() + 4.0
            };
            let mr = Rect::from_min_size(Pos2::new(hr.right() - 108.0, by), egui::vec2(bs, bs));
            let sr = Rect::from_min_size(Pos2::new(mr.right() + 3.0, by), egui::vec2(bs, bs));
            for (r, on, label, on_c, op_) in [
                (mr, m.mute, "M", t.ochre, HeadOp::Mute(tr)),
                (sr, m.solo, "S", t.accent, HeadOp::Solo(tr)),
            ] {
                let resp = ui
                    .interact(r, ui.scope_id().with(("tl_ms", label, tr)), Sense::click())
                    .on_hover_text(if label == "M" {
                        "mute this track (F9 on the cursor channel)"
                    } else {
                        "solo this track (F10)"
                    });
                let bg = if on {
                    on_c
                } else if resp.hovered() {
                    t.pill.gamma_multiply(1.4)
                } else {
                    t.pill
                };
                hp.rect_filled(r, 3, bg);
                hp.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(11.0),
                    if on {
                        Color32::from_rgb(0x16, 0x18, 0x1B)
                    } else {
                        t.ink2
                    },
                );
                if resp.clicked() {
                    head_ops.push(op_);
                }
            }
            // automation lanes: fold / unfold (M5)
            {
                let ar =
                    Rect::from_min_size(Pos2::new(mr.left() - bs - 3.0, by), egui::vec2(bs, bs));
                let open = lay.open.get(tr).copied().unwrap_or(false);
                let nl = autolanes::track_targets(&c.tl, tr).len();
                let resp = ui
                    .interact(ar, ui.scope_id().with(("tl_auto_fold", tr)), Sense::click())
                    .on_hover_text(if !mode {
                        "automation lanes on this track (needs timeline mode; asks first)"
                            .to_string()
                    } else if open {
                        "hide this track's automation lanes".to_string()
                    } else if nl > 0 {
                        format!("show this track's automation lanes ({nl})")
                    } else {
                        "automation lanes: show them, then + lane".to_string()
                    });
                hp.rect_filled(
                    ar,
                    3,
                    if open {
                        t.sage
                    } else if resp.hovered() {
                        t.pill.gamma_multiply(1.4)
                    } else {
                        t.pill
                    },
                );
                hp.text(
                    ar.center(),
                    egui::Align2::CENTER_CENTER,
                    "A",
                    FontId::proportional(11.0),
                    if open {
                        Color32::from_rgb(0x16, 0x18, 0x1B)
                    } else if nl > 0 {
                        t.sage
                    } else {
                        t.ink2
                    },
                );
                if resp.clicked() {
                    auto_ops.push(autolanes::AutoOp::Toggle(tr));
                }
            }
            // volume mini-fader (0..2 linear, unity marked) and meter
            let fr = Rect::from_min_max(
                Pos2::new(sr.right() + 6.0, by + bs / 2.0 - 3.0),
                Pos2::new(hr.right() - 12.0, by + bs / 2.0 + 3.0),
            );
            let fresp = ui
                .interact(
                    fr.expand2(egui::vec2(2.0, 5.0)),
                    ui.scope_id().with(("tl_vol", tr)),
                    Sense::click_and_drag(),
                )
                .on_hover_text(format!(
                    "volume {} · drag · double-click: 0 dB",
                    crate::model::db_text(m.volume)
                ));
            hp.rect_filled(fr, 3, t.crt_bot);
            let f = (m.volume / 2.0).clamp(0.0, 1.0);
            hp.rect_filled(
                Rect::from_min_max(fr.min, Pos2::new(fr.left() + f * fr.width(), fr.bottom())),
                3,
                colr.gamma_multiply(0.8),
            );
            hp.vline(
                fr.center().x,
                fr.y_range(),
                Stroke::new(1.0, t.ink2.gamma_multiply(0.6)),
            );
            hp.circle_filled(
                Pos2::new(fr.left() + f * fr.width(), fr.center().y),
                4.5,
                t.ink,
            );
            if fresp.double_clicked() {
                head_ops.push(HeadOp::Volume(tr, 1.0));
            } else if fresp.dragged() || fresp.clicked() {
                if let Some(p) = fresp.interact_pointer_pos() {
                    let v = ((p.x - fr.left()) / fr.width()).clamp(0.0, 1.0) * 2.0;
                    head_ops.push(HeadOp::Volume(tr, v));
                }
            }
            let pk = live.peaks.get(tr).copied().unwrap_or(0.0);
            let lvl = ((20.0 * pk.max(1e-6).log10() + 48.0) / 48.0).clamp(0.0, 1.0);
            let meter = Rect::from_min_max(
                Pos2::new(hr.right() - 7.0, hr.top() + 4.0),
                Pos2::new(hr.right() - 3.0, hr.bottom() - 4.0),
            );
            hp.rect_filled(meter, 1, t.crt_bot);
            if lvl > 0.0 {
                let mc = if pk > 0.98 { t.brick } else { t.sage };
                hp.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(meter.left(), meter.bottom() - lvl * meter.height()),
                        meter.max,
                    ),
                    1,
                    mc,
                );
            }
            // the rest of the header selects the track (cursor channel)
            let nr = Rect::from_min_max(hr.min, Pos2::new(mr.left() - bs - 5.0, hr.bottom()));
            let speed = c.tl.tracks[tr].speed;
            if speed > 0 {
                hp.text(
                    Pos2::new(mr.left() - bs - 7.0, hr.bottom() - 9.0),
                    egui::Align2::RIGHT_CENTER,
                    format!("speed {speed}"),
                    FontId::proportional(10.0),
                    t.ochre,
                );
            }
            let nresp = ui
                .interact(nr, ui.scope_id().with(("tl_head", tr)), Sense::click_and_drag())
                .on_hover_text(
                    "select this track (the Tracker's cursor channel) · drag up or down: reorder · right-click: the track menu (rename, colour, icon, add, devices, routing, organize, delete)",
                );
            let is_master = c.tl.tracks[tr].kind == tl::TrackKind::Master;
            if !is_master
                && nresp.hovered()
                && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
            {
                if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                    pie_at = Some((tr, p));
                }
            }
            if nresp.clicked() {
                head_ops.push(HeadOp::Select(tr));
            }
            if nresp.drag_started() {
                hd = Some(tr);
            }
            if nresp.drag_stopped() && hd == Some(tr) {
                hd = None;
                if let Some(p) = ui.ctx().input(|i| i.pointer.latest_pos()) {
                    let ins = insert_index(&lay, p.y - heads.top() + sy);
                    let to = if ins > tr { ins - 1 } else { ins };
                    if to != tr {
                        head_ops.push(HeadOp::Move(tr, to));
                    }
                }
            }
            let ntracks = c.tl.tracks.len();
            if !is_master {
                continue;
            }
            // the Song / Master row keeps the small menu (add a track)
            nresp.context_menu(|ui| {
                ui.label(egui::RichText::new(format!("Track {:02}", tr + 1)).strong());
                let full = ntracks >= MAX_CHANNELS;
                let tip = "all 32 tracks are used (the channel limit)";
                if ui
                    .add_enabled(!full, egui::Button::new("Add track above"))
                    .on_disabled_hover_text(tip)
                    .clicked()
                {
                    head_ops.push(HeadOp::Add(tr));
                    ui.close();
                }
                if ui
                    .add_enabled(!full, egui::Button::new("Add track below"))
                    .on_disabled_hover_text(tip)
                    .clicked()
                {
                    head_ops.push(HeadOp::Add(tr + 1));
                    ui.close();
                }
                if ui
                    .add_enabled(!full, egui::Button::new("Add track with instrument…"))
                    .on_hover_text(
                        "opens the Browser's Instruments tab: double-click one to make the track",
                    )
                    .on_disabled_hover_text(tip)
                    .clicked()
                {
                    head_ops.push(HeadOp::AddWith(tr + 1));
                    ui.close();
                }
                ui.separator();
                ui.label(egui::RichText::new(format!("Track {:02} speed", tr + 1)).strong());
                ui.label(
                    egui::RichText::new("ticks per row for this track's effects (timeline mode)")
                        .small(),
                );
                for (v, l) in [
                    (0u8, "Follow the song"),
                    (2, "2"),
                    (3, "3"),
                    (4, "4"),
                    (6, "6"),
                    (8, "8"),
                    (12, "12"),
                ] {
                    if ui.radio(speed == v, l).clicked() {
                        head_ops.push(HeadOp::Speed(tr, v));
                        ui.close();
                    }
                }
            });
        }
        // ---- header drag: the insertion line
        if let (Some(from), Some(p)) = (hd, ui.ctx().input(|i| i.pointer.latest_pos())) {
            let ins = insert_index(&lay, p.y - heads.top() + sy);
            let y = heads.top() + lay.top.get(ins).copied().unwrap_or(lay.total()) - sy;
            let lp = ui.painter_at(Rect::from_min_max(
                heads.min,
                Pos2::new(lanes.right(), heads.bottom()),
            ));
            if ins != from && ins != from + 1 {
                lp.line_segment(
                    [Pos2::new(heads.left(), y), Pos2::new(lanes.right(), y)],
                    egui::Stroke::new(2.5, t.accent),
                );
            }
            lp.text(
                Pos2::new(p.x + 14.0, p.y),
                egui::Align2::LEFT_CENTER,
                format!("move track {:02}", from + 1),
                FontId::proportional(11.0),
                t.accent,
            );
            if !ui.ctx().input(|i| i.pointer.primary_down()) {
                hd = None;
            }
        }
        // ---- "+ Add track" below the last mixer track, in the gap above
        // Song / Master (which sits at the bottom, after a divider)
        {
            let gap_top = lay
                .master
                .map_or(lay.total(), |m| lay.top[m] - autolanes::MASTER_GAP);
            if let Some(m) = lay.master {
                let dy = heads.top() + lay.top[m] - sy - 4.0;
                if dy > heads.top() && dy < heads.bottom() {
                    ui.painter().hline(
                        heads.left()..=lanes.right(),
                        dy,
                        Stroke::new(1.0, t.ink2.gamma_multiply(0.35)),
                    );
                }
            }
            let y = heads.top() + gap_top - sy + 4.0;
            if y + 20.0 < heads.bottom() {
                let br =
                    Rect::from_min_size(Pos2::new(heads.left() + 6.0, y), egui::vec2(110.0, 20.0));
                // after the last mixer track (the Song / Master track stays last)
                let n =
                    c.tl.tracks
                        .iter()
                        .filter(|x| x.kind != tl::TrackKind::Master)
                        .count();
                let full = n >= MAX_CHANNELS;
                let r = ui.put(
                    br,
                    egui::Button::new(egui::RichText::new("+ Add track").size(11.5)).small(),
                );
                let r = if full {
                    r.on_hover_text("all 32 tracks are used (the channel limit)")
                } else {
                    r.on_hover_text("a new empty track below the last one · drop an instrument from the Browser here for a track that plays it")
                };
                if r.clicked() {
                    head_ops.push(HeadOp::Add(n));
                }
            }
        }
        // ---- a clean new song: no tracks yet
        if self.with(|s| s.song.channels) == 0 {
            let y = (heads.top() + lay.total() - sy + 40.0).min(lanes.bottom() - 30.0);
            let p = ui.painter_at(lanes);
            p.text(
                Pos2::new(lanes.left() + 24.0, y),
                egui::Align2::LEFT_TOP,
                "Drag an instrument from the Browser here, or + Add track",
                egui::FontId::proportional(15.0),
                t.ink2,
            );
        }
        // ---- input: ruler seek / loop, markers, clips, scroll bars
        let mods = ui.input(|i| i.modifiers);
        let grid = self.tlv.grid(ppt, bar_ticks, mods.shift);
        self.tlv.grid_now = self.tlv.grid(ppt, bar_ticks, false);
        let snap_t = |x: f32| tl::snap(tick_of(x).max(0.0) as Tick, grid);
        let rresp = ui
            .interact(ruler, ui.scope_id().with("tl_ruler"), Sense::click_and_drag())
            .on_hover_text(if mode {
                "click: move the play marker here (snaps; jumps while playing) · drag: scrub · Shift-drag: set the loop region · Alt: exact row"
            } else {
                "click: move the play marker here (jumps while playing) · drag: scrub · Alt: exact row"
            });
        let mresp = ui
            .interact(marks, ui.scope_id().with("tl_marks"), Sense::click_and_drag())
            .on_hover_text(if mode {
                "markers · click: jump there · drag: move · double-click: rename · right-click: rename / delete · Shift+M: add one"
            } else {
                "markers (from the blocks' roles) · click: jump there"
            });
        if rresp.hovered() || rresp.dragged() {
            rp.rect_filled(ruler, 2, t.ink2.gamma_multiply(0.10));
            ui.ctx().set_cursor_icon(if mods.shift && mode {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::PointingHand
            });
        }
        // the marker under x (its flag or its label)
        let marker_at = |x: f32| {
            c.tl.markers.iter().rposition(|m| {
                let mx = x_of(m.tick as f64);
                x >= mx - 4.0 && x <= mx + 10.0 + m.name.chars().count() as f32 * 6.5
            })
        };
        let mut seek: Option<(usize, usize)> = None;
        let mut seek_tl: Option<Tick> = None;
        let mut edit: Option<DragEdit> = None;
        let mut start_drag: Option<Drag> = None;
        let mut end_drag = false;
        let mut ops: Vec<TlOp> = vec![];
        let mut ask = false;
        let origin = || c.tl.clone();
        let new_drag = |kind: DragKind, p: Pos2, anchor: Tick| Drag {
            kind,
            origin: origin(),
            before: None,
            grab_tick: tick_of(p.x),
            grab_y: p.y,
            anchor,
            moved: false,
        };
        // ruler: seek, or Shift-drag the loop region
        let looping = matches!(
            self.tlv.drag.as_ref().map(|d| &d.kind),
            Some(DragKind::Loop(_))
        );
        if rresp.drag_started() && mods.shift {
            if mode {
                if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                    // (Shift is the gesture here, so the grid stays coarse)
                    let a = tl::snap(tick_of(p.x).max(0.0) as Tick, self.tlv.grid_now);
                    start_drag = Some(new_drag(DragKind::Loop(a), p, 0));
                }
            } else {
                ask = true;
            }
        } else if looping {
            if let (Some(p), Some(DragKind::Loop(a))) = (
                rresp.interact_pointer_pos(),
                self.tlv.drag.as_ref().map(|d| d.kind.clone()),
            ) {
                edit = Some(DragEdit::Loop(
                    a,
                    tl::snap(tick_of(p.x).max(0.0) as Tick, self.tlv.grid_now),
                ));
            }
        } else if rresp.clicked() || rresp.dragged() || rresp.drag_started() {
            if let Some(p) = rresp.interact_pointer_pos() {
                if mode {
                    let tk = tick_of(p.x).max(0.0) as Tick;
                    let tgt = if mods.alt {
                        tk / ROW * ROW
                    } else {
                        tl::snap(tk, self.tlv.grid_now).max(0) / ROW * ROW
                    };
                    if rresp.clicked() || rresp.drag_started() || tgt != self.tlv.cue {
                        seek_tl = Some(tgt);
                    }
                } else {
                    let abs = (tick_of(p.x).max(0.0) / ROW as f64) as usize;
                    let tgt = super::arranger::ruler_target(&c.map.lens, abs, !mods.alt);
                    if rresp.clicked() || rresp.drag_started() || tgt != self.cue {
                        seek = Some(tgt);
                    }
                }
            }
        }
        if rresp.drag_stopped() && looping {
            end_drag = true;
        }
        self.tlv.ruler_drag = rresp.dragged() && !looping;
        // markers: jump, drag, rename, delete
        let marking = matches!(
            self.tlv.drag.as_ref().map(|d| &d.kind),
            Some(DragKind::Marker(_))
        );
        if mresp.clicked() {
            if let Some(p) = mresp.interact_pointer_pos() {
                match marker_at(p.x) {
                    Some(i) if mode => seek_tl = Some(c.tl.markers[i].tick / ROW * ROW),
                    Some(i) => seek = Some(c.map.entry_at(c.tl.markers[i].tick)),
                    None if mode => seek_tl = Some(snap_t(p.x) / ROW * ROW),
                    None => {
                        let abs = (tick_of(p.x).max(0.0) / ROW as f64) as usize;
                        seek = Some(super::arranger::ruler_target(&c.map.lens, abs, !mods.alt));
                    }
                }
            }
        }
        if mresp.drag_started() {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                if let Some(i) = marker_at(p.x) {
                    if mode {
                        start_drag = Some(new_drag(DragKind::Marker(i), p, c.tl.markers[i].tick));
                    } else {
                        ask = true;
                    }
                }
            }
        } else if marking && mresp.dragged() {
            if let (Some(p), Some(d)) = (mresp.interact_pointer_pos(), self.tlv.drag.as_ref()) {
                if let DragKind::Marker(i) = d.kind {
                    let dt = tick_of(p.x) - d.grab_tick;
                    edit = Some(DragEdit::Marker(
                        i,
                        tl::snap((d.anchor as f64 + dt).max(0.0) as Tick, grid),
                    ));
                }
            }
        }
        if mresp.drag_stopped() && marking {
            end_drag = true;
        }
        if mresp.double_clicked() {
            if let Some(i) = mresp.interact_pointer_pos().and_then(|p| marker_at(p.x)) {
                if mode {
                    self.tlv.rename = Some((i, c.tl.markers[i].name.clone()));
                } else {
                    ask = true;
                }
            }
        }
        if mresp.secondary_clicked() {
            self.tlv.menu_marker = mresp.interact_pointer_pos().and_then(|p| marker_at(p.x));
        }
        let menu_marker = self.tlv.menu_marker;
        let mut marker_op: Option<(usize, bool)> = None; // (index, delete?)
        if mode {
            mresp.context_menu(|ui| match menu_marker {
                Some(i) => {
                    if ui.button("Rename…").clicked() {
                        marker_op = Some((i, false));
                        ui.close();
                    }
                    if ui.button("Delete marker").clicked() {
                        marker_op = Some((i, true));
                        ui.close();
                    }
                }
                None => {
                    if ui.button("Add marker here").clicked() {
                        ui.close();
                        if let Some(p) = ui.ctx().input(|i| i.pointer.press_origin()) {
                            ops.push(TlOp::AddMarker(tl::snap(
                                tick_of(p.x).max(0.0) as Tick,
                                ROW,
                            )));
                        }
                    }
                }
            });
        }
        // lanes: select, move, trim / loop, rubber band, double-click
        let cresp = ui.interact(
            lanes,
            ui.scope_id().with("tl_lanes"),
            Sense::click_and_drag(),
        );
        // automation lane rows (on top of the clip area's input)
        {
            let geo = autolanes::Geo {
                lanes,
                heads,
                sy,
                x_of: &x_of,
                tick_of: &tick_of,
                v0,
                v1,
                at: live.head.unwrap_or(cue_tick as f64),
                mode,
            };
            if let Some(ref song) = song_for_lanes {
                autolanes::lanes_ui(
                    ui,
                    song,
                    c,
                    &lay,
                    &geo,
                    first_tr..last_tr,
                    self.tlv.auto_drag.as_ref(),
                    self.tlv.auto_menu.as_ref(),
                    self.tlv.lane_menu.as_ref().map(|m| &m.1),
                    &song_lanes,
                    &units,
                    self.tlv.follow_touch,
                    self.tlv.last_touch.is_some(),
                    self.auto_tool,
                    &mut auto_ops,
                );
            }
        }
        // which part of a clip is under p: 0 body, 1 left edge, 2 right
        // edge, 3 fade-in handle, 4 fade-out handle (audio clips)
        let part = |i: usize, p: Pos2| {
            let cl = &c.tl.clips[i];
            let (x0, x1) = (x_of(cl.start as f64), x_of(cl.end() as f64));
            if cl.audio().is_some() {
                let lr = lane_rect(cl.track as usize);
                let r = Rect::from_min_max(
                    Pos2::new(x0 + 0.5, lr.top() + 2.0),
                    Pos2::new((x1 - 0.5).max(x0 + 1.5), lr.bottom() - 2.0),
                );
                let hh = (lane_h * 0.3).clamp(9.0, 15.0);
                match audioclip::fade_handle_at(p, r, hh, cl, &x_of) {
                    Some(true) => return 3,
                    Some(false) => return 4,
                    None => {}
                }
            }
            let e = if x1 - x0 > 24.0 {
                6.0
            } else {
                ((x1 - x0) / 4.0).max(2.0)
            };
            if p.x >= x1 - e {
                2
            } else if p.x <= x0 + e {
                1
            } else {
                0
            }
        };
        // the track at screen y (-1 above, n_tracks below the tracks)
        let tr_of = |y: f32| {
            let cy = y - lanes.top() + sy;
            if cy < 0.0 {
                -1.0
            } else if cy >= lay.total() {
                n_tracks as f32
            } else {
                lay.track_at(cy) as f32
            }
        };
        // a clip row (not an automation lane or strip) at screen y
        let in_clip_row = |y: f32| matches!(lay.part_at(y - lanes.top() + sy), Some((_, None)));
        self.tlv.hover_at = hover
            .filter(|p| lanes.contains(*p))
            .map(|p| (tr_of(p.y).max(0.0) as usize, tick_of(p.x).max(0.0) as Tick));
        if let (Some(i), Some(p), None) = (hover_clip, hover, self.tlv.drag.as_ref()) {
            if mode {
                ui.ctx().set_cursor_icon(match part(i, p) {
                    0 => egui::CursorIcon::Grab,
                    3 | 4 => egui::CursorIcon::PointingHand,
                    _ => egui::CursorIcon::ResizeHorizontal,
                });
            }
        }
        let add_mod = mods.command || mods.shift;
        if cresp.drag_started() {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
                let hit = clip_at(c, &lay, p, lanes, sy, tick_of(p.x));
                match (hit, mode) {
                    (Some(_), false) => ask = true,
                    (Some(i), true) => {
                        let cl = &c.tl.clips[i];
                        let id = cl.id;
                        let mut sels = self.tlv.sels.clone();
                        if !sels.contains(&id) {
                            if !add_mod {
                                sels.clear();
                            }
                            sels.push(id);
                        }
                        let kind = match part(i, p) {
                            1 => {
                                sels = vec![id];
                                DragKind::Start(id)
                            }
                            2 => {
                                sels = vec![id];
                                DragKind::End(id)
                            }
                            k @ (3 | 4) => {
                                sels = vec![id];
                                DragKind::Fade(id, k == 3)
                            }
                            _ => DragKind::Move { dup: mods.alt },
                        };
                        let mut d = new_drag(kind.clone(), p, cl.start);
                        if let DragKind::End(_) = kind {
                            d.anchor = cl.end();
                        }
                        if mods.alt && matches!(kind, DragKind::Move { .. }) {
                            // Alt-drag: copies (linked: same blocks) follow the mouse
                            d.before = Some(d.origin.clone());
                            let sel: Vec<tl::Clip> = d
                                .origin
                                .clips
                                .iter()
                                .filter(|x| sels.contains(&x.id))
                                .cloned()
                                .collect();
                            sels = tl::copies_at(&mut d.origin, &sel, 0);
                            edit = Some(DragEdit::Move(0, 0));
                        }
                        self.tlv.sel = Some(*sels.last().unwrap_or(&id));
                        self.tlv.sels = sels;
                        start_drag = Some(d);
                    }
                    (None, _) => {
                        if !add_mod {
                            {
                                let x: Option<u32> = None;
                                self.tlv.sel = x;
                                self.tlv.sels = x.into_iter().collect();
                            }
                        }
                        start_drag = Some(new_drag(DragKind::Band(p), p, 0));
                    }
                }
            }
        } else if cresp.dragged() {
            if let (Some(p), Some(d)) = (cresp.interact_pointer_pos(), self.tlv.drag.as_ref()) {
                let raw = tick_of(p.x) - d.grab_tick;
                let at = |base: Tick| tl::snap((base as f64 + raw).max(0.0) as Tick, grid);
                edit = match d.kind {
                    DragKind::Move { .. } => {
                        let dtr = (tr_of(p.y) - tr_of(d.grab_y)) as i32;
                        Some(DragEdit::Move(at(d.anchor) - d.anchor, dtr))
                    }
                    DragKind::End(id) => Some(DragEdit::End(id, at(d.anchor))),
                    DragKind::Start(id) => Some(DragEdit::Start(id, at(d.anchor))),
                    DragKind::Fade(id, inn) => {
                        // free (Shift: on the grid)
                        let tk = tick_of(p.x).max(0.0) as Tick;
                        let tk = if mods.shift { tl::snap(tk, ROW) } else { tk };
                        Some(DragEdit::Fade(id, inn, tk))
                    }
                    DragKind::Band(p0) => {
                        let band = Rect::from_two_pos(p0, p);
                        lp.rect(
                            band,
                            0,
                            t.selection_outline.gamma_multiply(0.08),
                            Stroke::new(1.0, t.selection_outline),
                            egui::StrokeKind::Inside,
                        );
                        let (t0, t1) =
                            (tick_of(band.left()) as Tick, tick_of(band.right()) as Tick);
                        let (r0, r1) = (
                            tr_of(band.top()).max(0.0) as usize,
                            tr_of(band.bottom()).max(0.0) as usize,
                        );
                        let mut sels: Vec<u32> = if add_mod {
                            self.tlv.sels.clone()
                        } else {
                            vec![]
                        };
                        for tr in r0..=r1.min(n_tracks.saturating_sub(1)) {
                            for i in c.idx.visible(&c.tl, tr, t0, t1 + 1) {
                                let id = c.tl.clips[i].id;
                                if !sels.contains(&id) {
                                    sels.push(id);
                                }
                            }
                        }
                        self.tlv.sel = sels.last().copied();
                        self.tlv.sels = sels;
                        None
                    }
                    _ => None,
                };
            }
        }
        if cresp.drag_stopped() {
            end_drag = true;
        }
        let mut open: Option<(usize, usize, usize, usize)> = None; // block, col, row, entry
        let mut open_roll = false;
        if cresp.clicked() || cresp.double_clicked() {
            if let Some(p) = cresp.interact_pointer_pos() {
                let tk = tick_of(p.x);
                let tr = tr_of(p.y) as usize;
                let hit = clip_at(c, &lay, p, lanes, sy, tk);
                let id = hit.map(|i| c.tl.clips[i].id);
                match id {
                    Some(id) if add_mod && cresp.clicked() => {
                        if let Some(k) = self.tlv.sels.iter().position(|x| *x == id) {
                            self.tlv.sels.remove(k);
                            self.tlv.sel = self.tlv.sels.last().copied();
                        } else {
                            self.tlv.sels.push(id);
                            self.tlv.sel = Some(id);
                        }
                    }
                    None if add_mod => {}
                    _ => {
                        self.tlv.sel = id;
                        self.tlv.sels = id.into_iter().collect();
                    }
                }
                if cresp.double_clicked() && tr < n_tracks && tk >= 0.0 && in_clip_row(p.y) {
                    let tk = tk as Tick;
                    let entry = if mode {
                        usize::MAX
                    } else {
                        c.map.entry_at(tk).0
                    };
                    open = match hit.map(|i| &c.tl.clips[i]) {
                        Some(cl) if cl.audio().is_some() => None,
                        Some(cl) if cl.block().is_none() => {
                            open_roll = true;
                            None
                        }
                        None if mode && mods.shift => {
                            // Shift+double-click: a new note clip
                            ops.push(TlOp::NewNotes(tr, tk / bar_ticks * bar_ticks));
                            None
                        }
                        // block clips: the piano roll or the pattern editor,
                        // as set in Settings (Alt: the other one)
                        Some(_) if self.extra.dblclick_roll != mods.alt => {
                            open_roll = true;
                            None
                        }
                        Some(cl) => cl
                            .block()
                            .map(|(b, col)| (b, col, (cl.source_at(tk) / ROW) as usize, entry)),
                        None if mode && !self.tracker_on() => {
                            // empty space: a new note clip (chords and all)
                            ops.push(TlOp::NewNotes(tr, tk / bar_ticks * bar_ticks));
                            None
                        }
                        None if mode => {
                            // empty space: a new block, starting on the bar
                            ops.push(TlOp::NewClip(tr, tk / bar_ticks * bar_ticks));
                            None
                        }
                        None if tk < c.map.end() => {
                            let (e, row) = c.map.entry_at(tk);
                            c.order.get(e).map(|&b| (b, tr, row, e))
                        }
                        None => None,
                    };
                }
            }
        }
        // right-click: the clip menu (also offers to switch in tracker mode)
        if cresp.secondary_clicked() {
            if let Some(p) = cresp.interact_pointer_pos() {
                let tk = tick_of(p.x).max(0.0) as Tick;
                self.tlv.menu_at = Some((tr_of(p.y).max(0.0) as usize, tk));
                let id = clip_at(c, &lay, p, lanes, sy, tick_of(p.x)).map(|i| c.tl.clips[i].id);
                if id.is_none_or(|id| !self.tlv.sels.contains(&id)) {
                    {
                        let x: Option<u32> = id;
                        self.tlv.sel = x;
                        self.tlv.sels = x.into_iter().collect();
                    }
                }
            }
        }
        let n_sel = self.tlv.sels.len();
        let menu_at = self.tlv.menu_at;
        let menu_open: Option<(usize, usize, usize)> = self
            .tlv
            .sel
            .and_then(|id| c.tl.clips.iter().find(|x| x.id == id))
            .and_then(|cl| {
                let (b, col) = cl.block()?;
                let at = menu_at.map_or(cl.start, |m| m.1.clamp(cl.start, cl.end() - 1));
                Some((b, col, (cl.source_at(at) / ROW) as usize))
            });
        let mut menu_open_now = false;
        let capture_empty = self.noterec.recent.is_empty();
        let mut audio_ops: Vec<audioclip::AudioOp> = vec![];
        let sel_audio: Option<&tl::Clip> = self
            .tlv
            .sel
            .and_then(|id| c.tl.clips.iter().find(|x| x.id == id))
            .filter(|cl| cl.audio().is_some());
        cresp.context_menu(|ui| {
            let snap_at = menu_at.map_or(0, |m| tl::snap(m.1, ROW));
            if !mode {
                ui.label("Clip editing needs timeline mode.");
                if ui.button("Switch to timeline mode…").clicked() {
                    ops.push(TlOp::SelectAll);
                    ui.close();
                }
                return;
            }
            if n_sel > 0 {
                if let Some(cl) = sel_audio {
                    audioclip::audio_menu(ui, cl, cl.audio().and_then(|k| c.tl.audio.get(k)), &mut audio_ops);
                    if !audio_ops.is_empty() && !matches!(audio_ops[0], audioclip::AudioOp::Gain(_)) {
                        ui.close();
                    }
                } else {
                    if ui
                        .add(egui::Button::new("Open in Tracker").shortcut_text(if self.extra.dblclick_roll { "Enter / Alt+double-click" } else { "Enter / double-click" }))
                        .clicked()
                    {
                        menu_open_now = true;
                        ui.close();
                    }
                    if ui
                        .add(egui::Button::new("Open in the piano roll").shortcut_text(if self.extra.dblclick_roll { "double-click" } else { "Alt+double-click" }))
                        .clicked()
                    {
                        open_roll = true;
                        ui.close();
                    }
                }
                ui.separator();
                let items: [(&str, &str, TlOp); 6] = [
                    ("Split here", "S", TlOp::Split(snap_at)),
                    ("Duplicate", "Ctrl+D", TlOp::Duplicate),
                    ("Make unique", "Ctrl+Shift+D", TlOp::MakeUnique),
                    ("Glue", "Ctrl+J", TlOp::Glue),
                    ("Mute / unmute", "M", TlOp::Mute),
                    ("Delete", "Del", TlOp::Delete),
                ];
                for (l, k, op) in items {
                    if ui.add(egui::Button::new(l).shortcut_text(k)).clicked() {
                        ops.push(op);
                        ui.close();
                    }
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("Loop around the selection").shortcut_text("Ctrl+L"))
                    .clicked()
                {
                    ops.push(TlOp::LoopFromSelection);
                    ui.close();
                }
            } else if let Some((tr, tk)) = menu_at {
                let tracker = self.tracker_on();
                if !tracker
                    && ui
                        .add(egui::Button::new("New clip here").shortcut_text("double-click"))
                        .clicked()
                {
                    ops.push(TlOp::NewNotes(tr, tk / bar_ticks * bar_ticks));
                    ui.close();
                }
                if tracker && ui.button("New clip here").clicked() {
                    ops.push(TlOp::NewClip(tr, tk / bar_ticks * bar_ticks));
                    ui.close();
                }
                if tracker
                    && ui
                        .add(egui::Button::new("New note clip here").shortcut_text("Shift+double-click"))
                        .clicked()
                {
                    ops.push(TlOp::NewNotes(tr, tk / bar_ticks * bar_ticks));
                    ui.close();
                }
                if !capture_empty
                    && ui
                        .button("Capture played MIDI here")
                        .on_hover_text("what you just played on the MIDI keyboard (recording or not) becomes a note clip")
                        .clicked()
                {
                    ops.push(TlOp::Capture(tr, tk / bar_ticks * bar_ticks));
                    ui.close();
                }
                if ui
                    .button("Import audio file here…")
                    .on_hover_text("WAV, FLAC, MP3 or Ogg Vorbis (or drop files on a track)")
                    .clicked()
                {
                    audio_ops.push(audioclip::AudioOp::Import(tr, tl::snap(tk, ROW)));
                    ui.close();
                }
            }
            if ui
                .add(egui::Button::new("Add marker here").shortcut_text("Shift+M"))
                .clicked()
            {
                ops.push(TlOp::AddMarker(snap_at));
                ui.close();
            }
        });
        if menu_open_now {
            if let Some((b, col, row)) = menu_open {
                open = Some((b, col, row, usize::MAX));
            }
        }
        if let Some(i) = hover_clip.filter(|_| self.tlv.drag.is_none()) {
            let cl = &c.tl.clips[i];
            if let Some(a) = cl.audio().and_then(|k| c.tl.audio.get(k)) {
                let bp = c.tl.bar_pos(cl.start);
                cresp.clone().on_hover_text(format!(
                    "{} · {} · bar {}.{}{}{}\n{}\ndrag: move · edges: trim / loop · top corners: fades · right-click: gain, fades, file",
                    a.name,
                    audioclip::describe_asset(a),
                    bp.bar,
                    bp.beat,
                    if a.embed { " · stored in the song" } else { " · referenced" },
                    if cl.muted { " · muted" } else { "" },
                    if a.missing() {
                        format!("missing: {}", a.path)
                    } else {
                        a.path.clone()
                    },
                ));
            }
            if let Some((b, col)) = cl.block() {
                let bp = c.tl.bar_pos(cl.start);
                let passes = cl
                    .loop_len
                    .map(|l| format!(" · loops {}x", (cl.len + l - 1) / l))
                    .unwrap_or_default();
                let dbl = if self.extra.dblclick_roll {
                    "double-click: piano roll (Alt: Tracker)"
                } else {
                    "double-click: Tracker (Alt: piano roll)"
                };
                let how = if mode {
                    format!(
                        "drag: move (Alt: copy) · edges: trim / loop · {dbl} · right-click: more"
                    )
                } else {
                    dbl.to_string()
                };
                cresp.clone().on_hover_text(format!(
                    "{} · column {} · bar {}.{} · {} rows{passes}{}\n{how}",
                    c.names.get(b).cloned().unwrap_or_default(),
                    col + 1,
                    bp.bar,
                    bp.beat,
                    cl.len / ROW,
                    if cl.muted { " · muted" } else { "" },
                ));
            }
        }
        // overview: click / drag moves the view
        let oresp = ui.interact(
            overview,
            ui.scope_id().with("tl_overview"),
            Sense::click_and_drag(),
        );
        self.tlv.overview_drag = oresp.dragged();
        if oresp.clicked() || oresp.dragged() {
            if let Some(p) = oresp.interact_pointer_pos() {
                let centre = (p.x - overview.left()) as f64 / o_ppt;
                self.tlv.scroll = centre - vis_ticks / 2.0;
            }
        }
        let vresp = ui.interact(vbar, ui.scope_id().with("tl_vbar"), Sense::click_and_drag());
        self.tlv.vbar_drag = vresp.dragged();
        if (vresp.clicked() || vresp.dragged()) && max_sy > 0.0 {
            if let Some(p) = vresp.interact_pointer_pos() {
                let f = (p.y - vbar.top()) / vbar.height();
                self.tlv.scroll_y = f * total_h - lanes.height() / 2.0;
            }
        }
        // ---- apply
        if ops.contains(&TlOp::SelectAll) && !mode {
            // the tracker-mode menu's "Switch to timeline mode…"
            ops.clear();
            ask = true;
        }
        if ask && self.tlv.ask_switch.is_none() {
            self.tlv.ask_switch = Some(None);
        }
        if let Some(d) = start_drag {
            self.tlv.drag = Some(d);
        }
        if let Some(e) = edit {
            self.timeline_drag_edit(e);
        }
        if end_drag {
            self.timeline_drag_end();
        }
        if let Some(t) = seek_tl {
            self.timeline_seek(t);
        }
        for op in ops {
            self.tl_apply(op);
        }
        for op in audio_ops {
            self.audio_op(op);
        }
        for op in auto_ops {
            self.auto_op(op);
        }
        if self.tlv.auto_drag.is_some() && !ui.input(|i| i.pointer.any_down()) {
            // released outside its row
            self.auto_op(autolanes::AutoOp::EndDrag);
        }
        self.tlv.layout = lay;
        if let Some((i, del)) = marker_op {
            if del {
                self.with(|s| {
                    if let Some(t) = s.song.timeline.as_deref_mut() {
                        if i < t.markers.len() {
                            t.markers.remove(i);
                        }
                    }
                });
                self.status = "Marker deleted (Ctrl+Z brings it back)".into();
            } else {
                let name = self
                    .tlv
                    .cache
                    .as_ref()
                    .and_then(|c| c.tl.markers.get(i))
                    .map(|m| m.name.clone());
                if let Some(n) = name {
                    self.tlv.rename = Some((i, n));
                }
            }
        }
        self.timeline_rename(ui.ctx());
        self.tlv.head_drag = hd;
        for op in head_ops {
            self.timeline_head_op(op);
        }
        if let Some(r) = rename_at {
            self.rename_field(ui, r);
        }
        if let Some((tr, p)) = pie_at {
            let ctx = ui.ctx().clone();
            self.open_track_pie(tr, p, true, &ctx);
        }
        if let Some(tgt) = seek {
            self.seek_to(tgt);
        }
        if let Some((b, col, row, entry)) = open {
            self.timeline_open(b, col, row, entry);
        }
        if open_roll {
            self.timeline_open_roll();
        }
        if live.playing {
            ui.ctx().request_repaint();
        }
        self.tlv.frame_us = t_start.elapsed().as_micros();
    }

    /// a track's effect speed (track pie › Organize; header menu)
    pub(super) fn track_speed(&mut self, tr: usize, v: u8) {
        self.tl_apply(TlOp::Speed(tr, v));
    }

    fn timeline_head_op(&mut self, op: HeadOp) {
        self.with(|s| {
            let mx = &mut s.song.mixer.channels;
            match op {
                HeadOp::Mute(i) => mx[i].mute = !mx[i].mute,
                HeadOp::Solo(i) => mx[i].solo = !mx[i].solo,
                HeadOp::Volume(i, v) => mx[i].volume = v.clamp(0.0, 2.0),
                HeadOp::Select(_)
                | HeadOp::Speed(..)
                | HeadOp::Move(..)
                | HeadOp::Add(_)
                | HeadOp::AddWith(_) => {}
            }
        });
        match op {
            HeadOp::Move(from, to) => {
                self.track_move(from, to);
                return;
            }
            HeadOp::Add(at) => {
                if self.track_add(at) {
                    self.status = format!("Added track {:02}", at + 1);
                }
                return;
            }
            HeadOp::AddWith(at) => {
                self.track_add_with(at);
                return;
            }
            _ => {}
        }
        if let HeadOp::Speed(i, v) = op {
            self.tl_apply(TlOp::Speed(i, v));
            return;
        }
        if let HeadOp::Select(i) = op {
            self.ch = i;
            let given = self.with(|s| {
                s.song
                    .timeline
                    .as_deref()
                    .and_then(|t| t.tracks.get(i))
                    .map(|t| t.instr)
                    .unwrap_or(0)
            });
            if given > 0 && (given as usize) <= self.with(|s| s.song.instruments.len()) {
                self.instr = given;
            }
            self.status = format!("Track {:02} selected (pattern cursor channel)", i + 1);
        }
        self.dirty = true;
    }

    /// double-click (or right-click › Open in the piano roll): the selected
    /// clip in the lower panel's piano roll. A block clip also becomes the
    /// pattern editor's current block and channel, so the roll keeps showing
    /// it in other views.
    pub(super) fn timeline_open_roll(&mut self) {
        let blk = self.tlv.sel.and_then(|id| {
            let c = self.tlv.cache.as_ref()?;
            c.tl.clips.iter().find(|x| x.id == id)?.block()
        });
        if let Some((b, col)) = blk {
            let (nb, nch) = self.with(|s| (s.song.patterns.len(), s.song.channels));
            if b < nb {
                self.pattern = b;
                self.ch = col.min(nch.saturating_sub(1));
                self.row = self.row.min(self.rows().saturating_sub(1));
                self.sel = None;
            }
        }
        self.set_lower(3);
        self.status = if blk.is_some() {
            if self.tracker_on() {
                "Piano roll: this clip's block column (Alt+double-click or Enter: the Tracker)"
                    .into()
            } else {
                "Piano roll: this Tracker clip (adding a chord makes it a note clip)".into()
            }
        } else {
            "Piano roll: this note clip".into()
        };
    }

    /// double-click: the block in the pattern editor, cursor on the column and row
    pub(super) fn timeline_open(&mut self, block: usize, col: usize, row: usize, entry: usize) {
        let (nb, nch) = self.with(|s| (s.song.patterns.len(), s.song.channels));
        if block >= nb {
            return;
        }
        self.pattern = block;
        self.ch = col.min(nch.saturating_sub(1));
        self.col = 0;
        self.row = row.min(self.rows().saturating_sub(1));
        self.sel = None;
        if entry < self.order_len() {
            self.arr_sel = entry;
            self.arr_multi.clear();
            self.arr_multi.insert(entry);
        }
        self.view = View::Pattern;
        self.arr_focus = false;
        let name = self.with(|s| s.song.patterns[block].name.clone());
        let rows = self.rows();
        self.status = format!(
            "Editing block {block:02} {name} · channel {} · row {} (Ctrl+T: back to the timeline)",
            self.ch + 1,
            self.row_text(self.row, rows)
        );
    }

    /// keys while the Timeline view is showing; true = used
    pub(super) fn timeline_key(&mut self, key: Key, m: egui::Modifiers) -> bool {
        // the prompts: Enter = yes, Esc = no; nothing else gets through
        if self.tlv.ask_switch.is_some() || self.tlv.ask_leave {
            if matches!(key, Key::Enter | Key::Escape) {
                let yes = key == Key::Enter;
                if self.tlv.ask_leave {
                    self.answer_leave(yes);
                } else {
                    self.answer_switch(yes);
                }
            }
            return true;
        }
        if self.tlv.rename.is_some() {
            return true;
        }
        // clip edits (in tracker mode they ask to switch first)
        let at_mouse = || {
            self.tlv
                .hover_at
                .map(|(_, t)| tl::snap(t, self.tlv.grid_now.max(ROW)))
        };
        let play_marker = self.tlv.cue;
        let op = match key {
            Key::D if m.command && m.shift => Some(TlOp::MakeUnique),
            Key::D if m.command => Some(TlOp::Duplicate),
            Key::J if m.command => Some(TlOp::Glue),
            Key::L if m.command => Some(TlOp::LoopFromSelection),
            Key::A if m.command => Some(TlOp::SelectAll),
            _ if m.command || m.alt => return false,
            Key::S if !m.shift => Some(TlOp::Split(at_mouse().unwrap_or(play_marker))),
            Key::M if m.shift => Some(TlOp::AddMarker(play_marker)),
            Key::M => Some(TlOp::Mute),
            Key::L => Some(TlOp::ToggleLoop),
            Key::Delete | Key::Backspace => Some(TlOp::Delete),
            _ => None,
        };
        if let Some(op) = op {
            if op == TlOp::SelectAll && self.tlv.cache.as_ref().is_some_and(|c| !c.mode) {
                return false;
            }
            self.tl_apply(op);
            return true;
        }
        let bar = (4 * PPQ) as f64;
        match key {
            Key::Plus | Key::Equals => self.tlv.zoom_by(1.25, None),
            Key::Minus => self.tlv.zoom_by(0.8, None),
            Key::F => self.tlv.zoom = 0.0,
            Key::Home => self.tlv.scroll = 0.0,
            Key::End => self.tlv.scroll = f64::MAX / 4.0,
            Key::ArrowLeft => self.tlv.scroll -= bar,
            Key::ArrowRight => self.tlv.scroll += bar,
            Key::ArrowUp => self.tlv.scroll_y -= self.tlv.th(),
            Key::ArrowDown => self.tlv.scroll_y += self.tlv.th(),
            Key::Escape => {
                self.tlv.sel = None;
                self.tlv.sels.clear();
            }
            Key::Enter => {
                let hit = self.tlv.sel.and_then(|id| {
                    let c = self.tlv.cache.as_ref()?;
                    let cl = c.tl.clips.iter().find(|x| x.id == id)?;
                    let (b, col) = cl.block()?;
                    Some((
                        b,
                        col,
                        (cl.offset / ROW) as usize,
                        c.map.entry_at(cl.start).0,
                    ))
                });
                let notes = self.tlv.sel.and_then(|id| {
                    let c = self.tlv.cache.as_ref()?;
                    c.tl.clips
                        .iter()
                        .find(|x| x.id == id && matches!(x.kind, tl::ClipKind::Notes { .. }))
                });
                match hit {
                    Some((b, col, row, e)) => self.timeline_open(b, col, row, e),
                    None if notes.is_some() => self.set_lower(3),
                    None => return false,
                }
            }
            _ => return false,
        }
        true
    }
}

#[derive(Clone, Copy)]
enum HeadOp {
    Mute(usize),
    Solo(usize),
    Volume(usize, f32),
    Select(usize),
    Speed(usize, u8),
    /// drag reorder: track, new index
    Move(usize, usize),
    /// an empty track at this index
    Add(usize),
    /// a track at this index with the instrument picked in the Browser
    AddWith(usize),
}

/// the gap (0..=tracks) nearest content y, for the reorder insertion line
fn insert_index(lay: &autolanes::Layout, y: f32) -> usize {
    let n = lay.tracks();
    (0..n)
        .filter(|&k| {
            let mid = (lay.top[k] + lay.top[k + 1].min(lay.top[k] + lay.clip_h)) / 2.0;
            mid < y
        })
        .count()
}

/// the clip under `p` (index into `tl.clips`; clip rows only)
fn clip_at(
    c: &Cache,
    lay: &autolanes::Layout,
    p: Pos2,
    lanes: Rect,
    sy: f32,
    tk: f64,
) -> Option<usize> {
    if tk < 0.0 {
        return None;
    }
    let (tr, None) = lay.part_at(p.y - lanes.top() + sy)? else {
        return None;
    };
    let tk = tk as Tick;
    c.idx.visible(&c.tl, tr, tk, tk + 1).last()
}

impl TimelineUi {
    /// what a clip is called in the view (block name, note clip, audio file)
    pub(crate) fn clip_label(&self, id: u32) -> Option<(tl::Clip, String)> {
        let c = self.cache.as_ref()?;
        let clip = c.tl.clips.iter().find(|x| x.id == id)?.clone();
        let name = match &clip.kind {
            _ if clip.name.is_some() => clip.name.clone().unwrap_or_default(),
            tl::ClipKind::Audio { asset } => {
                c.tl.audio
                    .get(*asset as usize)
                    .map_or("audio".into(), |a| a.name.clone())
            }
            tl::ClipKind::Notes { data } => {
                c.tl.notes
                    .get(*data as usize)
                    .map_or("notes".into(), |d| d.name.clone())
            }
            _ => clip
                .block()
                .and_then(|(b, _)| c.names.get(b).cloned())
                .unwrap_or_default(),
        };
        Some((clip, name))
    }

    /// the order-list entry a clip starts in (tracker mode; Enter uses it)
    pub(crate) fn entry_at(&self, t: Tick) -> usize {
        self.cache
            .as_ref()
            .map_or(usize::MAX, |c| c.map.entry_at(t).0)
    }

    /// screen rect of clip `id` as last drawn (tests, M2 hit-testing)
    #[allow(dead_code)]
    pub(crate) fn clip_rect(&self, id: u32) -> Option<Rect> {
        let c = self.cache.as_ref()?;
        let cl = c.tl.clips.iter().find(|x| x.id == id)?;
        let ppt = self.zoom as f64 / PPQ as f64;
        let x = |t: Tick| self.lanes.left() + ((t as f64 - self.scroll) * ppt) as f32;
        let y = self.lanes.top()
            + self
                .layout
                .top
                .get(cl.track as usize)
                .copied()
                .unwrap_or(cl.track as f32 * self.th())
            - self.scroll_y;
        Some(Rect::from_min_max(
            Pos2::new(x(cl.start), y + 2.0),
            Pos2::new(x(cl.end()), y + self.th() - 2.0),
        ))
    }

    /// Where a Browser drop at `p` lands (headers or lanes): Ok(track) on
    /// a track's rows, Err(n) below the last track (a new track); with the
    /// snapped tick when `p` is over the lanes
    pub(crate) fn drop_at(&self, p: Pos2) -> Option<(Result<usize, usize>, Option<Tick>)> {
        let area = Rect::from_min_max(
            self.heads.min,
            Pos2::new(self.lanes.right(), self.heads.bottom()),
        );
        if !area.contains(p) || self.layout.tracks() == 0 {
            return None;
        }
        let y = p.y - self.heads.top() + self.scroll_y;
        let tick = self.lanes.contains(p).then(|| {
            let t = self.scroll
                + (p.x - self.lanes.left()) as f64 * PPQ as f64 / self.zoom.max(0.01) as f64;
            tl::snap(t.max(0.0) as Tick, self.grid_now.max(1)).max(0)
        });
        if y >= self.layout.total() {
            return Some((Err(self.layout.tracks()), tick));
        }
        // the gap above Song / Master: a new track there
        if let (true, Some(m)) = (self.layout.in_gap(y), self.layout.master) {
            return Some((Err(m), tick));
        }
        Some((Ok(self.layout.track_at(y)), tick))
    }

    /// track `tr`'s header rect and its whole row (header + lanes) as last drawn
    pub(crate) fn track_rects(&self, tr: usize) -> Option<(Rect, Rect)> {
        let top = *self.layout.top.get(tr)?;
        let y = self.heads.top() + top - self.scroll_y;
        let h = self.layout.clip_h;
        let hr = Rect::from_min_max(
            Pos2::new(self.heads.left(), y),
            Pos2::new(self.heads.right(), y + h),
        )
        .intersect(self.heads);
        let row = Rect::from_min_max(
            Pos2::new(self.heads.left(), y),
            Pos2::new(self.lanes.right(), y + h),
        )
        .intersect(Rect::from_min_max(
            self.heads.min,
            Pos2::new(self.lanes.right(), self.heads.bottom()),
        ));
        Some((hr, row))
    }

    /// screen y just below the last track (clamped to the view)
    pub(crate) fn tracks_bottom(&self) -> f32 {
        (self.heads.top() + self.layout.total() - self.scroll_y)
            .clamp(self.heads.top(), self.heads.bottom() - 2.0)
    }

    /// screen x of tick `t` as last drawn
    #[allow(dead_code)]
    pub(crate) fn x_of(&self, t: Tick) -> f32 {
        self.lanes.left() + ((t as f64 - self.scroll) * self.zoom as f64 / PPQ as f64) as f32
    }

    /// zoom by `f` around `at` (pixels from the lanes' left edge; None = centre)
    pub(crate) fn zoom_by(&mut self, f: f32, at: Option<f32>) {
        let z0 = if self.zoom > 0.0 { self.zoom } else { 8.0 };
        let z1 = (z0 * f).clamp(MIN_ZOOM, MAX_ZOOM);
        let rel = at.unwrap_or(self.lanes.width() / 2.0).max(0.0) as f64;
        let anchor = self.scroll + rel * PPQ as f64 / z0 as f64;
        self.zoom = z1;
        self.scroll = (anchor - rel * PPQ as f64 / z1 as f64).max(0.0);
    }
}
