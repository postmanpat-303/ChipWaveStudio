//! The Clip Arranger (docs/CLIP_ARRANGER.md): a scene / slot grid on the
//! left of the Timeline, one row per track, aligned with the timeline's
//! track rows. Inspired by other DAWs' session views.
//!
//! - A scene is a block: scene N's button plays block N on every track,
//!   looping, from the next bar (Launch: 1 bar / ½ bar / beat / off).
//! - A slot is one track's column of that block, with a mini preview of
//!   its notes. Its ▶ plays just that track (in the playing scene, or in
//!   that scene with the other tracks held); dragging it onto the timeline
//!   makes a Tracker clip (block + column) on the bar under the mouse.
//! - ■ on a row stops that track at the next bar; ■ in the header stops all.
//!
//! Launches go to the engine as a queued `Cue` (engine/launch.rs): the
//! render thread switches on the boundary without locks or allocation.
//! Under it sits a small Master panel (volume, limiter drive and ceiling,
//! level and gain-reduction meters, loudness). Ctrl+8 or View › Clip
//! Arranger shows the Instruments list there instead.
use super::*;
use crate::engine::launch::{bit, Cue};
use crate::timeline::{self as tl, Tick, ROW};

/// Launch quantise choices (`Extra::launch_q`)
pub(crate) const QUANT: [(&str, &str); 4] = [
    ("1 bar", "launches and stops wait for the next bar line"),
    ("½ bar", "launches and stops wait for the next half bar"),
    ("Beat", "launches and stops wait for the next beat"),
    ("Off", "launches and stops happen on the next row"),
];

/// most notes drawn in one slot preview
const PREVIEW_NOTES: usize = 96;
/// narrowest slot column
const SLOT_MIN_W: f32 = 58.0;
const STOP_W: f32 = 24.0;
/// level history columns in the Master panel
const HIST: usize = 120;
/// seconds of playing per history column: 120 columns show the last 30 s,
/// so the graph drifts calmly instead of racing by a column per frame
pub(crate) const HIST_STEP: f32 = 0.25;

#[derive(Default)]
pub(crate) struct SlotPrev {
    /// (start, length) as fractions of the block, and the note
    pub(crate) notes: Vec<(f32, f32, u8)>,
    lo: u8,
    hi: u8,
}

pub(crate) struct CaUi {
    /// first scene shown (the grid pages through the blocks)
    pub(crate) first: usize,
    /// a slot being dragged to the timeline (docs/unified/PLAN.md 4a)
    pub(crate) drag: Option<super::dropplan::DropSrc>,
    /// the side panel's rect this frame (drawn after the Timeline)
    pub(crate) rect: Rect,
    /// mini previews per [scene][track], for `key`
    key: Option<u64>,
    pub(crate) prev: Vec<Vec<SlotPrev>>,
    /// timeline songs with note or audio clips: the scenes (None: the
    /// scenes are the blocks), for `key`
    pub(crate) secs: Option<Vec<Section>>,
    /// the scenes are the launcher's own clips (`Timeline::launcher`;
    /// docs/unified/PLAN.md Stage 4b), not the arrangement's
    pub(crate) launch: bool,
    /// the status line as last drawn (tests)
    pub(crate) line: String,
    /// the scene seen playing last frame (the grid pages to a new one)
    seen: Option<usize>,
    /// as last drawn (tests, screenshots)
    pub(crate) scene_rects: Vec<(usize, Rect)>,
    /// (track, block, ▶ strip, whole slot)
    pub(crate) slot_rects: Vec<(usize, usize, Rect, Rect)>,
    pub(crate) stop_rects: Vec<(usize, Rect)>,
    pub(crate) stop_all: Rect,
    pub(crate) master: Rect,
    /// screenshots: hold the slot drag at this point, with these keys
    pub(crate) shot_drag: Option<Pos2>,
    pub(crate) shot_mods: egui::Modifiers,
    /// master level history for the Master panel's graph (newest last)
    pub(crate) hist: std::collections::VecDeque<f32>,
    /// playing time gathered into the next history column, and its loudest peak
    hist_t: f32,
    hist_pk: f32,
    /// where the LEVEL history (or its hidden strip) was drawn
    pub(crate) level_rect: Rect,
}

impl CaUi {
    /// Gathers `dt` seconds of playing at level `pk` into the LEVEL history:
    /// one column per HIST_STEP holding that stretch's loudest peak, so no
    /// peak is lost; a column only falls part of the way towards a quieter
    /// stretch (fast rise, slow fall), which keeps the graph calm.
    pub(crate) fn hist_step(&mut self, dt: f32, pk: f32) {
        self.hist_t += dt.clamp(0.0, 0.25);
        self.hist_pk = self.hist_pk.max(pk);
        while self.hist_t >= HIST_STEP {
            self.hist_t -= HIST_STEP;
            let pk = std::mem::take(&mut self.hist_pk);
            let v = match self.hist.back() {
                Some(&last) if pk < last => last + (pk - last) * 0.5,
                _ => pk,
            };
            self.hist.push_back(v);
            while self.hist.len() > HIST {
                self.hist.pop_front();
            }
        }
    }
}

impl Default for CaUi {
    fn default() -> Self {
        Self {
            first: 0,
            drag: None,
            rect: Rect::NOTHING,
            key: None,
            prev: vec![],
            secs: None,
            launch: false,
            line: String::new(),
            seen: None,
            scene_rects: vec![],
            slot_rects: vec![],
            stop_rects: vec![],
            stop_all: Rect::NOTHING,
            master: Rect::NOTHING,
            shot_drag: None,
            shot_mods: egui::Modifiers::NONE,
            hist: Default::default(),
            hist_t: 0.0,
            hist_pk: 0.0,
            level_rect: Rect::NOTHING,
        }
    }
}

fn previews(song: &Song) -> Vec<Vec<SlotPrev>> {
    let nch = song.channels.min(MAX_CHANNELS);
    song.patterns
        .iter()
        .map(|b| {
            let cells = if b.derived.is_some() && !b.cache.is_empty() {
                &b.cache
            } else {
                &b.cells
            };
            let n = b.rows.max(1);
            (0..nch)
                .map(|ch| {
                    let note_at =
                        |r: usize| cells.get(r).and_then(|x| x.get(ch)).map_or(0, |c| c.note);
                    let mut sp = SlotPrev {
                        lo: 255,
                        ..Default::default()
                    };
                    for r in 0..n {
                        let k = note_at(r);
                        if !(1..=120).contains(&k) {
                            continue;
                        }
                        let end = (r + 1..n).find(|&q| note_at(q) != 0).unwrap_or(n);
                        sp.lo = sp.lo.min(k);
                        sp.hi = sp.hi.max(k);
                        if sp.notes.len() < PREVIEW_NOTES {
                            sp.notes
                                .push((r as f32 / n as f32, (end - r) as f32 / n as f32, k));
                        }
                    }
                    sp
                })
                .collect()
        })
        .collect()
}

/// A scene of a timeline song with note or audio clips: the clips that
/// start on one tick, one per track (docs/CLIP_ARRANGER.md).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Section {
    pub(crate) start: Tick,
    /// the end of its longest clip
    pub(crate) end: Tick,
    /// a marker on its start, else "Bar N" (or "N.b" off the bar)
    pub(crate) name: String,
    /// per track: the clip that starts there
    pub(crate) clips: Vec<Option<u32>>,
}

/// The scenes of a timeline song that has note or audio clips (new songs,
/// recorded takes, Drum Kit patterns): one per clip start time, in time
/// order, with each track's clip that starts there. None for a song whose
/// clips are all Tracker clips: its scenes stay the blocks.
pub(crate) fn sections(song: &Song) -> Option<Vec<Section>> {
    let t = song.timeline.as_deref()?;
    if t.clips.iter().all(|c| c.block().is_some()) {
        return None;
    }
    let nch = song.channels.min(MAX_CHANNELS);
    let on_track = |c: &&tl::Clip| (c.track as usize) < nch && c.len > 0;
    let mut starts: Vec<Tick> = t.clips.iter().filter(on_track).map(|c| c.start).collect();
    starts.sort_unstable();
    starts.dedup();
    Some(
        starts
            .into_iter()
            .map(|s| {
                let mut clips = vec![None; nch];
                let mut end = s;
                for c in t.clips.iter().filter(on_track).filter(|c| c.start == s) {
                    let slot = &mut clips[c.track as usize];
                    if slot.is_none() {
                        *slot = Some(c.id);
                    }
                    end = end.max(c.end());
                }
                let name = match t.markers.iter().find(|m| m.tick == s && !m.name.is_empty()) {
                    Some(m) => m.name.clone(),
                    None => {
                        let p = t.bar_pos(s);
                        if p.beat == 1 && p.tick == 0 {
                            format!("Bar {}", p.bar)
                        } else {
                            format!("{}.{}", p.bar, p.beat)
                        }
                    }
                };
                Section {
                    start: s,
                    end,
                    name,
                    clips,
                }
            })
            .collect(),
    )
}

/// The launcher's scene rows as scenes (Stage 4b): per track the launcher
/// clip in that row. None while the song has no launcher clips or scenes
/// (the grid then shows the arrangement).
pub(crate) fn launcher_sections(song: &Song) -> Option<Vec<Section>> {
    let t = song.timeline.as_deref()?;
    if t.launcher.is_empty() && t.scenes.is_empty() {
        return None;
    }
    let nch = song.channels.min(MAX_CHANNELS);
    Some(
        t.scenes
            .iter()
            .enumerate()
            .map(|(k, sc)| Section {
                start: 0,
                end: 0,
                name: sc.name.clone(),
                clips: (0..nch)
                    .map(|tr| tl::launch_at(t, tr, k).map(|l| l.clip.id))
                    .collect(),
            })
            .collect(),
    )
}

/// a section's timeline rows [a, b) (what its launch loops)
pub(crate) fn sec_rows(s: &Section) -> (u32, u32) {
    let a = (s.start / ROW).max(0) as u32;
    let b = ((s.end + ROW - 1) / ROW).max(0) as u32;
    (a, b.max(a + 1))
}

/// the slot previews of `secs` (per scene, per track): a note clip's notes
/// as they play in the clip (offset and loop), a Tracker clip's column,
/// an audio clip as one bar
fn section_previews(song: &Song, secs: &[Section], blocks: &[Vec<SlotPrev>]) -> Vec<Vec<SlotPrev>> {
    let Some(t) = song.timeline.as_deref() else {
        return vec![];
    };
    secs.iter()
        .map(|s| {
            s.clips
                .iter()
                .map(|id| {
                    let mut sp = SlotPrev {
                        lo: 255,
                        ..Default::default()
                    };
                    let Some(c) = id.and_then(|id| t.any_clip(id)) else {
                        return sp;
                    };
                    let len = c.len.max(1) as f32;
                    match c.kind {
                        tl::ClipKind::Notes { data } => {
                            let Some(d) = t.notes.get(data as usize) else {
                                return sp;
                            };
                            let lp = c.loop_len.filter(|&l| l > 0);
                            for n in &d.notes {
                                let mut at = n.start - c.offset;
                                if let Some(l) = lp {
                                    at = at.rem_euclid(l);
                                }
                                while at < c.len && sp.notes.len() < PREVIEW_NOTES {
                                    if at >= 0 {
                                        sp.lo = sp.lo.min(n.key);
                                        sp.hi = sp.hi.max(n.key);
                                        sp.notes.push((at as f32 / len, n.len as f32 / len, n.key));
                                    }
                                    match lp {
                                        Some(l) => at += l,
                                        None => break,
                                    }
                                }
                            }
                        }
                        tl::ClipKind::Block { pattern, col } => {
                            if let Some(b) = blocks.get(pattern).and_then(|v| v.get(col as usize)) {
                                sp.notes = b.notes.clone();
                                sp.lo = b.lo;
                                sp.hi = b.hi;
                            }
                        }
                        tl::ClipKind::Audio { .. } => {
                            sp.notes.push((0.0, 1.0, 60));
                            sp.lo = 60;
                            sp.hi = 60;
                        }
                    }
                    sp
                })
                .collect()
        })
        .collect()
}

fn play_tri(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![
            c + egui::vec2(-0.7 * s, -s),
            c + egui::vec2(s, 0.0),
            c + egui::vec2(-0.7 * s, s),
        ],
        col,
        Stroke::NONE,
    ));
}

/// 0..1 pulse for queued things (steady when motion is reduced)
fn pulse(ui: &egui::Ui, calm: bool) -> f32 {
    if calm {
        return 1.0;
    }
    ui.ctx().request_repaint();
    let t = ui.input(|i| i.time) as f32;
    0.5 + 0.5 * (t * std::f32::consts::TAU * 1.6).sin()
}

impl App {
    /// the Clip Arranger takes the left panel now
    pub(crate) fn ca_active(&self) -> bool {
        self.extra.clip_arranger
            && !self.side_hidden()
            && self.view == View::Timeline
            && self.with(|s| s.song.timeline.is_some())
    }

    /// the left panel is hidden on the Timeline (more room for the tracks)
    pub(crate) fn side_hidden(&self) -> bool {
        self.extra.side_hidden && self.view == View::Timeline
    }

    /// the Timeline's "Arranger" button / Ctrl+Shift+8: show or hide the
    /// Clip Arranger panel on the left (remembered)
    pub(crate) fn toggle_side_panel(&mut self) {
        self.extra.side_hidden = !self.extra.side_hidden;
        if !self.extra.side_hidden {
            self.extra.clip_arranger = true;
            if !self.lcd_on() {
                // (the classic toolbar's meter goes: so does its popover)
                self.lcd.master_open = false;
            }
        }
        self.save_settings();
        self.status = if self.extra.side_hidden {
            "Clip Arranger hidden: more room for the Timeline; the Master is in the toolbar meter (the Arranger button or Ctrl+Shift+8 shows it)"
        } else {
            "Clip Arranger shown on the left of the Timeline (the Arranger button or Ctrl+Shift+8 hides it)"
        }
        .into();
    }

    /// Ctrl+8 / View › Clip Arranger
    pub(crate) fn toggle_clip_arranger(&mut self) {
        if self.extra.side_hidden {
            // a hidden panel comes back first
            self.extra.side_hidden = false;
            self.extra.clip_arranger = false;
        }
        self.extra.clip_arranger = !self.extra.clip_arranger;
        self.save_settings();
        self.status = if self.extra.clip_arranger {
            "Clip Arranger on the left of the Timeline (Ctrl+8; Instruments: Browser › Instruments)"
        } else {
            "Instruments list on the left (Ctrl+8 brings the Clip Arranger back)"
        }
        .into();
    }

    /// (every, phase) in rows for the current launch quantise
    pub(crate) fn ca_quant(&self) -> (u32, u32) {
        let q = self.extra.launch_q.min(3);
        let (bar, beat, phase) = self.with(|s| match s.song.timeline.as_deref() {
            Some(t) => {
                let at = s.engine.tl_row().map_or(0, |r| r as Tick * ROW);
                let sig = t.sig_at(at);
                let from = if s.engine.tl_row().is_some() {
                    sig.tick
                } else {
                    0
                };
                (
                    (sig.bar_ticks() / ROW).max(1) as u32,
                    (sig.beat_ticks() / ROW).max(1) as u32,
                    (from / ROW).max(0) as u32,
                )
            }
            None => (4 * ROWS_PER_BEAT, ROWS_PER_BEAT, 0),
        });
        let every = match q {
            0 => bar,
            1 => bar / 2,
            2 => beat,
            _ => 1,
        };
        (every.max(1), phase)
    }

    /// queue a launch with the current quantise
    pub(crate) fn ca_launch(&mut self, mut c: Cue) {
        (c.every, c.phase) = self.ca_quant();
        self.with(|s| {
            let Shared { song, engine, .. } = s;
            engine.launch(song, c);
        });
    }

    /// sections: track `tr`'s clip in scene `k`
    pub(crate) fn ca_clip(&self, tr: usize, k: usize) -> Option<u32> {
        self.ca
            .secs
            .as_ref()?
            .get(k)?
            .clips
            .get(tr)
            .copied()
            .flatten()
    }

    /// sections: double-click a slot opens its clip (piano roll; a
    /// Tracker clip in the Tracker)
    fn ca_open_clip(&mut self, tr: usize, k: usize) {
        let Some(id) = self.ca_clip(tr, k) else {
            return;
        };
        self.ch = tr;
        self.tlv.sels = vec![id];
        self.tlv.sel = Some(id);
        let block = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.clips.iter().find(|c| c.id == id))
                .and_then(|c| c.block())
        });
        match block {
            Some((b, col)) => self.timeline_open(b, col, 0, usize::MAX),
            None => self.set_lower(3),
        }
    }

    /// sections: scene `k` loops its span on every track
    fn ca_scene_sec(&mut self, k: usize) {
        let Some(s) = self.ca.secs.as_ref().and_then(|x| x.get(k)).cloned() else {
            return;
        };
        let span = sec_rows(&s);
        if self.with(|x| x.engine.cue().is_some_and(|c| c.span == Some(span))) {
            self.with(|x| x.engine.cancel_cue());
            self.status = format!("Scene {} no longer queued", s.name);
            return;
        }
        self.ca_launch(Cue {
            span: Some(span),
            on: u64::MAX,
            ..Default::default()
        });
        self.status = format!(
            "Scene {}: every track plays its clips there, looping, from the next {}",
            s.name,
            self.ca_when()
        );
    }

    /// sections: slot ▶ plays track `tr`'s clip in scene `k` (the other
    /// tracks keep playing when that scene already plays; otherwise the
    /// scene starts with only this track)
    fn ca_slot_sec(&mut self, tr: usize, k: usize) {
        let Some(s) = self.ca.secs.as_ref().and_then(|x| x.get(k)).cloned() else {
            return;
        };
        let span = sec_rows(&s);
        let (now, queued, nch) = self.with(|x| {
            (
                x.engine.span().map(|(a, b)| (a as u32, b as u32)),
                x.engine.cue().and_then(|c| c.span),
                x.song.channels,
            )
        });
        let all = (0..nch).fold(0u64, |m, q| m | bit(q));
        let c = if queued == Some(span) || (queued.is_none() && now == Some(span)) {
            Cue {
                on: bit(tr),
                ..Default::default()
            }
        } else {
            Cue {
                span: Some(span),
                on: bit(tr),
                off: all & !bit(tr),
                ..Default::default()
            }
        };
        self.ca_launch(c);
        self.ch = tr;
        self.status = format!(
            "Track {:02} plays its clip at {} from the next {}",
            tr + 1,
            s.name,
            self.ca_when()
        );
    }

    /// scene ▶: block `b` on every track
    pub(crate) fn ca_scene(&mut self, b: usize) {
        if self.ca.launch {
            return self.ca_scene_launch(b);
        }
        if self.ca.secs.is_some() {
            return self.ca_scene_sec(b);
        }
        // a second click on the scene waiting for its bar takes it back
        if self.with(|s| s.engine.cue().is_some_and(|c| c.scene == Some(b))) {
            self.with(|s| s.engine.cancel_cue());
            self.status = format!("Scene {b:02} no longer queued");
            return;
        }
        self.ca_launch(Cue {
            scene: Some(b),
            on: u64::MAX,
            ..Default::default()
        });
        let name = self.with(|s| s.song.patterns.get(b).map(|p| p.name.clone()));
        self.status = format!(
            "Scene {b:02} {}: every track plays block {b:02} from the next {}",
            name.unwrap_or_default(),
            self.ca_when()
        );
    }

    /// slot ▶: track `tr` plays block `b` (the other tracks keep what they
    /// do in that scene; a new scene starts with only this track)
    pub(crate) fn ca_slot(&mut self, tr: usize, b: usize) {
        if self.ca.launch {
            return self.ca_slot_launch(tr, b);
        }
        if self.ca.secs.is_some() {
            return self.ca_slot_sec(tr, b);
        }
        let (now, queued, nch) = self.with(|s| {
            (
                s.engine.scene(),
                s.engine.cue().and_then(|c| c.scene),
                s.song.channels,
            )
        });
        let all = (0..nch).fold(0u64, |m, k| m | bit(k));
        let c = if queued == Some(b) || (queued.is_none() && now == Some(b)) {
            Cue {
                on: bit(tr),
                ..Default::default()
            }
        } else {
            Cue {
                scene: Some(b),
                on: bit(tr),
                off: all & !bit(tr),
                ..Default::default()
            }
        };
        self.ca_launch(c);
        self.ch = tr;
        self.status = format!(
            "Track {:02} plays block {b:02} from the next {}",
            tr + 1,
            self.ca_when()
        );
    }

    // ---------------------------------------------- launcher clips (4b)

    /// launcher: slot ▶ plays track `tr`'s clip in scene `k`, looping, from
    /// the next boundary; the other tracks play on (from a stop: only it)
    fn ca_slot_launch(&mut self, tr: usize, k: usize) {
        let Some(id) = self.ca_clip(tr, k) else {
            return;
        };
        let (playing, nch) = self.with(|s| (s.engine.playing, s.song.channels));
        let all = (0..nch).fold(0u64, |m, q| m | bit(q));
        let mut c = Cue {
            on: bit(tr),
            off: if playing { 0 } else { all & !bit(tr) },
            ..Default::default()
        };
        c.clips[tr] = id;
        self.ca_launch(c);
        self.ch = tr;
        let name = self
            .ca
            .secs
            .as_ref()
            .and_then(|x| x.get(k))
            .map(|s| s.name.clone());
        self.status = format!(
            "Track {:02} plays its clip in {} from the next {}, looping",
            tr + 1,
            name.unwrap_or_default(),
            self.ca_when()
        );
    }

    /// launcher: scene ▶ plays every clip of scene `k`; tracks without one
    /// stop. A second click while it waits takes it back.
    fn ca_scene_launch(&mut self, k: usize) {
        let Some(sc) = self.ca.secs.as_ref().and_then(|x| x.get(k)).cloned() else {
            return;
        };
        let nch = self.with(|s| s.song.channels).min(MAX_CHANNELS);
        let mut c = Cue::default();
        for tr in 0..nch {
            match sc.clips.get(tr).copied().flatten() {
                Some(id) => {
                    c.clips[tr] = id;
                    c.on |= bit(tr);
                }
                None => c.off |= bit(tr),
            }
        }
        let same = self.with(|s| {
            s.engine
                .cue()
                .is_some_and(|q| q.clips == c.clips && c.on != 0)
        });
        if same {
            self.with(|s| s.engine.cancel_cue());
            self.status = format!("Scene {} no longer queued", sc.name);
            return;
        }
        if c.on == 0 {
            self.status = format!(
                "Scene {} has no clips yet: double-click a slot for one",
                sc.name
            );
            return;
        }
        self.ca_launch(c);
        self.status = format!(
            "Scene {}: its clips play, each looping on its track, from the next {}",
            sc.name,
            self.ca_when()
        );
    }

    /// launcher: double-click opens the clip in the piano roll; an empty
    /// slot gets a new one-bar clip first (it plays the track's instrument)
    pub(super) fn ca_open_launch(&mut self, tr: usize, k: usize) {
        let id = match self.ca_clip(tr, k) {
            Some(id) => id,
            None => {
                let bar = self.pat_bar();
                let Some(id) = self.with(|s| {
                    s.song
                        .timeline
                        .as_deref_mut()
                        .map(|t| tl::launch_new(t, tr, k, bar))
                }) else {
                    return;
                };
                self.status = format!(
                    "New clip on track {:02}: draw its notes in the piano roll; ▶ plays it",
                    tr + 1
                );
                self.dirty = true;
                id
            }
        };
        self.ch = tr;
        self.tlv.sels.clear();
        self.tlv.sel = None;
        self.pr.launch = Some(id);
        self.set_lower(3);
    }

    /// Capture plus a variation of the first clip (screenshots)
    pub(super) fn ca_capture_now(&mut self) {
        let secs = self.with(|s| sections(&s.song));
        self.ca.secs = secs;
        self.ca.launch = false;
        self.ca_capture();
        let first = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.launcher.first())
                .map(|l| l.clip.id)
        });
        if let Some(id) = first {
            self.ca_variation(id);
        }
    }

    /// "Capture": the arrangement's scenes become launcher clips, linked
    fn ca_capture(&mut self) {
        let Some(secs) = self.ca.secs.clone().filter(|_| !self.ca.launch) else {
            return;
        };
        let rows: Vec<(String, Vec<(usize, u32)>)> = secs
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    s.clips
                        .iter()
                        .enumerate()
                        .filter_map(|(tr, id)| id.map(|id| (tr, id)))
                        .collect(),
                )
            })
            .collect();
        let n = self.with(|s| {
            let t = s.song.timeline.as_deref_mut()?;
            let mut try_t = t.clone();
            let n = tl::launch_capture(&mut try_t, &rows);
            if n > 0 {
                *t = try_t;
            }
            Some(n)
        });
        self.status = match n {
            Some(0) | None => "Nothing captured: only note clips can be launcher clips".into(),
            Some(n) => {
                self.dirty = true;
                format!(
                    "Captured {n} clip{} into {} scenes of the launcher (linked: editing their notes \
                     changes the arrangement's too · Ctrl+Z undoes)",
                    if n == 1 { "" } else { "s" },
                    rows.len()
                )
            }
        };
    }

    fn ca_add_scene(&mut self) {
        let k = self.with(|s| s.song.timeline.as_deref_mut().map(tl::launch_add_scene));
        if let Some(k) = k {
            self.dirty = true;
            self.status = format!(
                "Scene {} added: double-click its slots for new clips",
                k + 1
            );
        }
    }

    fn ca_delete_scene(&mut self, k: usize) {
        let ok = self.with(|s| {
            s.song
                .timeline
                .as_deref_mut()
                .is_some_and(|t| tl::launch_delete_scene(t, k))
        });
        if ok {
            self.dirty = true;
            self.status = "Scene deleted with its clips (Ctrl+Z brings it back)".into();
        }
    }

    /// a new scene after `k` with independent copies of its clips
    fn ca_duplicate_scene(&mut self, k: usize) {
        let n = self.with(|s| {
            let t = s.song.timeline.as_deref_mut()?;
            let name = t.scenes.get(k)?.name.clone();
            let ids: Vec<(usize, u32)> = t
                .launcher
                .iter()
                .filter(|l| l.scene as usize == k)
                .map(|l| (l.clip.track as usize, l.clip.id))
                .collect();
            let at = tl::launch_add_scene(t);
            t.scenes[at].name = format!("{name} copy");
            for (tr, id) in &ids {
                tl::launch_copy(t, *id, *tr, at, true);
            }
            Some(ids.len())
        });
        if let Some(n) = n {
            self.dirty = true;
            self.status = format!("Scene duplicated with {n} independent clip copies");
        }
    }

    pub(super) fn ca_variation(&mut self, id: u32) {
        let v = self.with(|s| {
            s.song
                .timeline
                .as_deref_mut()
                .and_then(|t| tl::launch_variation(t, id))
        });
        if v.is_some() {
            self.dirty = true;
            self.status =
                "Variation: an independent copy in the next free scene (its own notes)".into();
        }
    }

    pub(super) fn ca_delete(&mut self, id: u32) {
        if self.with(|s| {
            s.song
                .timeline
                .as_deref_mut()
                .is_some_and(|t| tl::launch_delete(t, id))
        }) {
            if self.pr.launch == Some(id) {
                self.pr.launch = None;
            }
            self.dirty = true;
            self.status = "Launcher clip deleted (Ctrl+Z brings it back)".into();
        }
    }

    /// row ■: track `tr` goes silent at the next boundary
    pub(crate) fn ca_stop(&mut self, tr: usize) {
        self.ca_launch(Cue {
            off: bit(tr),
            ..Default::default()
        });
        self.status = format!("Track {:02} stops at the next {}", tr + 1, self.ca_when());
    }

    /// header ■: the transport stops at the next boundary
    pub(crate) fn ca_stop_all(&mut self) {
        let playing = self.with(|s| s.engine.playing);
        self.ca_launch(Cue {
            halt: true,
            ..Default::default()
        });
        self.status = if playing {
            format!("Everything stops at the next {}", self.ca_when())
        } else {
            "Stopped".into()
        };
    }

    fn ca_when(&self) -> &'static str {
        match self.extra.launch_q {
            0 => "bar",
            1 => "half bar",
            2 => "beat",
            _ => "row",
        }
    }

    /// the left panel on the Timeline: Clip Arranger on top, Master below.
    /// Drawn after the Timeline so its rows line up with this frame's tracks.
    pub(super) fn clip_arranger(&mut self, ui: &mut egui::Ui) {
        let r = self.ca.rect;
        if !r.is_positive() {
            return;
        }
        let heads = self.tlv.heads;
        let ok = heads.is_positive() && heads.bottom() <= r.bottom() + 1.0;
        // the LCD toolbar has the Master (a popover): the grid runs down
        let lcd = self.lcd_on();
        let bottom = if lcd {
            r.bottom()
        } else if ok {
            (heads.bottom() + 30.0).min(r.bottom())
        } else {
            r.bottom()
        };
        let top = Rect::from_min_max(r.min, Pos2::new(r.right(), bottom));
        self.ca_grid(ui, top);
        let mr = Rect::from_min_max(Pos2::new(r.left(), bottom + 8.0), r.max);
        if !lcd {
            // (in the LCD toolbar the popover sets it)
            self.ca.master = Rect::NOTHING;
        }
        if mr.height() >= 150.0 && !lcd {
            self.ca_master(ui, mr);
        }
        self.ca_drag(ui.ctx());
    }

    fn ca_grid(&mut self, ui: &mut egui::Ui, top: Rect) {
        let t = th();
        let calm = self.extra.reduce_motion;
        let p = ui.painter().clone();
        theme::cartridge(&p, top, t.slate);
        // previews follow the song (the timeline's cache key changes on edits)
        let key = self.tlv.cache.as_ref().map(|c| c.key);
        if key.is_none() || key != self.ca.key {
            let (prev, secs, launch) = self.with(|s| {
                let blocks = previews(&s.song);
                if let Some(x) = launcher_sections(&s.song) {
                    return (section_previews(&s.song, &x, &blocks), Some(x), true);
                }
                match sections(&s.song) {
                    Some(x) => (section_previews(&s.song, &x, &blocks), Some(x), false),
                    None => (blocks, None, false),
                }
            });
            self.ca.prev = prev;
            self.ca.secs = secs;
            self.ca.launch = launch;
            self.ca.key = key;
        }
        let (names, nch, cols, playing, scene, cue, held, prog) = self.with(|s| {
            let song = &s.song;
            let e = &s.engine;
            let names: Vec<String> = song
                .patterns
                .iter()
                .enumerate()
                .map(|(i, b)| format!("{i:02} {}", b.name))
                .collect();
            let nch = song.channels.min(MAX_CHANNELS);
            let tlr = song.timeline.as_deref();
            let cols: Vec<Color32> = (0..nch)
                .map(|i| {
                    tlr.map_or(super::timeline::track_colour(i), |x| {
                        super::timeline::track_colour_in(x, i)
                    })
                })
                .collect();
            let prog = song
                .patterns
                .get(e.pattern)
                .map_or(0.0, |b| (e.row as f32 + 0.5) / b.rows.max(1) as f32);
            (
                names,
                nch,
                cols,
                e.playing,
                e.scene(),
                e.cue(),
                e.held(),
                prog,
            )
        });
        // timeline songs with note / audio clips: the scenes are sections
        // (a launched span is the playing scene; queued spans show as scenes)
        let secs = self.ca.secs.clone();
        let launch = self.ca.launch;
        let (names, scene, prog, cue) = match &secs {
            Some(x) if launch => (
                x.iter().map(|s| s.name.clone()).collect(),
                None,
                0.0,
                cue.map(|c| Cue { scene: None, ..c }),
            ),
            Some(x) => {
                let (span, tlrow) = self.with(|s| (s.engine.span(), s.engine.tl_row()));
                let of = |sp: (u32, u32)| x.iter().position(|s| sec_rows(s) == sp);
                let scene = span.and_then(|(a, b)| of((a as u32, b as u32)));
                let prog = match (span, tlrow) {
                    (Some((a, b)), Some(r)) => {
                        (r.saturating_sub(a) as f32 + 0.5) / (b - a).max(1) as f32
                    }
                    _ => 0.0,
                };
                let cue = cue.map(|c| Cue {
                    scene: c.span.and_then(of),
                    ..c
                });
                (x.iter().map(|s| s.name.clone()).collect(), scene, prog, cue)
            }
            None => (names, scene, prog, cue),
        };
        // the arrangement playing: the block under the playhead on each track
        let mut under: Vec<Option<(usize, f32)>> = vec![None; nch];
        if let (Some(x), true, None) = (&secs, playing, scene) {
            // sections: the clip under the playhead on each track
            let head = self.with(|s| {
                s.engine
                    .tl_row()
                    .map(|_| s.engine.song_pos as f64 * ROW as f64)
            });
            if let (Some(c), Some(h)) = (self.tlv.cache.as_ref(), head) {
                for cl in c.tl.clips.iter().filter(|c| !c.muted) {
                    let (a, z) = (cl.start as f64, cl.end() as f64);
                    let tr = cl.track as usize;
                    if h >= a && h < z && tr < nch {
                        if let Some(k) =
                            x.iter().position(|s| s.clips.get(tr) == Some(&Some(cl.id)))
                        {
                            under[tr] = Some((k, ((h - a) / (z - a)) as f32));
                        }
                    }
                }
            }
        } else if playing && scene.is_none() {
            let head = self.with(|s| {
                s.engine
                    .tl_row()
                    .map(|_| s.engine.song_pos as f64 * ROW as f64)
            });
            if let (Some(c), Some(h)) = (self.tlv.cache.as_ref(), head) {
                for cl in &c.tl.clips {
                    if let (Some((b, _)), false) = (cl.block(), cl.muted) {
                        let (a, z) = (cl.start as f64, cl.end() as f64);
                        let tr = cl.track as usize;
                        if h >= a && h < z && tr < nch {
                            under[tr] = Some((b, ((h - a) / (z - a)) as f32));
                        }
                    }
                }
            }
        }
        if let (Some(x), true) = (&secs, launch) {
            // launcher clips playing: their scene and how far into the pass
            let lp: Vec<(Option<u32>, f32)> = self.with(|s| {
                (0..nch)
                    .map(|tr| {
                        (
                            s.engine.launched(tr),
                            s.engine.launch_progress(tr).unwrap_or(0.0),
                        )
                    })
                    .collect()
            });
            for (tr, (id, f)) in lp.into_iter().enumerate() {
                under[tr] = id.and_then(|id| {
                    x.iter()
                        .position(|s| s.clips.get(tr) == Some(&Some(id)))
                        .map(|k| (k, f))
                });
            }
        }
        let inner = top.shrink2(egui::vec2(12.0, 6.0));
        let gx0 = inner.left();
        let gx1 = inner.right() - STOP_W - 6.0;
        let nb = names.len();
        let nvis = (((gx1 - gx0) / SLOT_MIN_W).floor() as usize)
            .clamp(1, 8)
            .min(nb.max(1));
        // a newly playing scene out of view comes into view (the buttons
        // under the mouse don't move otherwise)
        if scene != self.ca.seen {
            self.ca.seen = scene;
            if let Some(b) = scene.filter(|&b| b < self.ca.first || b >= self.ca.first + nvis) {
                self.ca.first = b.saturating_sub(1);
            }
        }
        self.ca.first = self.ca.first.min(nb.saturating_sub(nvis));
        let first = self.ca.first;
        let sw = (gx1 - gx0) / nvis as f32;
        // ---- title row: name, launch quantise, pages
        let title = Rect::from_min_size(inner.min, egui::vec2(inner.width(), 24.0));
        let mut title_op: Option<u8> = None;
        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt("ca_title")
                .max_rect(title)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(
                    egui::RichText::new("Clip Arranger")
                        .strong()
                        .size(crate::theme::ts::px(crate::theme::ts::LARGE))
                        .color(t.ink),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let more = first + nvis < nb;
                    if ui
                        .add_enabled(more, egui::Button::new("›").small())
                        .on_hover_text("next scenes")
                        .clicked()
                    {
                        self.ca.first = (first + nvis).min(nb.saturating_sub(nvis));
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "{}–{} of {nb}",
                            (first + 1).min(nb),
                            (first + nvis).min(nb)
                        ))
                        .size(crate::theme::ts::px(crate::theme::ts::SMALL))
                        .color(t.ink2),
                    );
                    if ui
                        .add_enabled(first > 0, egui::Button::new("‹").small())
                        .on_hover_text("previous scenes")
                        .clicked()
                    {
                        self.ca.first = first.saturating_sub(nvis);
                    }
                    ui.add_space(6.0);
                    let q = self.extra.launch_q.min(3) as usize;
                    let mut nq = q;
                    egui::ComboBox::from_id_salt("ca_launch_q")
                        .selected_text(
                            egui::RichText::new(QUANT[q].0)
                                .size(crate::theme::ts::px(crate::theme::ts::SMALL)),
                        )
                        .width(58.0)
                        .show_ui(ui, |ui| {
                            for (k, (n, tip)) in QUANT.iter().enumerate() {
                                ui.selectable_value(&mut nq, k, *n).on_hover_text(*tip);
                            }
                        })
                        .response
                        .on_hover_text("Launch: when scene, slot and stop clicks take effect");
                    if nq != q {
                        self.extra.launch_q = nq as u8;
                        self.save_settings();
                    }
                    ui.label(
                        egui::RichText::new("Launch")
                            .size(crate::theme::ts::px(crate::theme::ts::SMALL))
                            .color(t.ink2),
                    );
                });
            },
        );
        // ---- scene buttons, on the timeline ruler's rows
        let ruler = self.tlv.ruler;
        let (hy0, hy1) = if ruler.is_positive() && ruler.top() > title.bottom() {
            (ruler.top().max(title.bottom() + 2.0), ruler.bottom())
        } else {
            (title.bottom() + 4.0, title.bottom() + 26.0)
        };
        self.ca.scene_rects.clear();
        let queued_scene = cue.and_then(|c| c.scene);
        let lit_scene = scene.or_else(|| {
            // the arrangement: the block most tracks play
            let mut n = vec![0usize; nb];
            for x in under.iter().flatten() {
                if let Some(v) = n.get_mut(x.0) {
                    *v += 1;
                }
            }
            n.iter()
                .enumerate()
                .filter(|x| *x.1 > 0)
                .max_by_key(|x| *x.1)
                .map(|x| x.0)
        });
        let k_pulse = pulse(ui, calm || cue.is_none());
        let mut clicked_scene = None;
        let mut scene_op: Option<(usize, u8)> = None;
        for k in 0..nvis {
            let b = first + k;
            let Some(name) = names.get(b) else { break };
            let sr = Rect::from_min_max(
                Pos2::new(gx0 + k as f32 * sw + 2.0, hy0),
                Pos2::new(gx0 + (k + 1) as f32 * sw - 2.0, hy1),
            );
            let tip = if launch {
                format!(
                    "Scene {name}: play its clips, each looping on its track, from the next {}; tracks \
                     without a clip in it stop · right-click: rename or delete the scene",
                    self.ca_when()
                )
            } else if secs.is_some() {
                format!(
                    "Scene {name}: play the clips that start here on every track, looping, from the next {} \
                     (click again while it waits to take it back)",
                    self.ca_when()
                )
            } else {
                format!(
                    "Scene {name}: play block {b:02} on every track, looping, from the next {} \
                     (click again while it waits to take it back)",
                    self.ca_when()
                )
            };
            let resp = ui
                .interact(sr, ui.make_persistent_id(("ca_scene", b)), Sense::click())
                .on_hover_text(tip);
            resp.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Scene {name}"))
            });
            if resp.clicked() {
                clicked_scene = Some(b);
            }
            if launch {
                resp.context_menu(|ui| {
                    if ui.button("Delete scene").clicked() {
                        scene_op = Some((b, 1));
                        ui.close();
                    }
                    if ui.button("Duplicate scene").clicked() {
                        scene_op = Some((b, 2));
                        ui.close();
                    }
                });
            }
            let on = lit_scene == Some(b);
            let q = queued_scene == Some(b);
            let fill = if on {
                t.accent.gamma_multiply(0.85)
            } else if resp.hovered() {
                Color32::from_white_alpha(if t.light { 60 } else { 22 })
            } else {
                Color32::from_black_alpha(if t.light { 30 } else { 80 })
            };
            p.rect_filled(sr, 5.0, fill);
            let edge = if q {
                Stroke::new(1.5, t.accent.gamma_multiply(0.35 + 0.65 * k_pulse))
            } else {
                Stroke::new(
                    1.0,
                    Color32::from_white_alpha(if t.light { 60 } else { 22 }),
                )
            };
            p.rect_stroke(sr, 5.0, edge, egui::StrokeKind::Inside);
            let ink = if on {
                Color32::from_rgb(0x16, 0x18, 0x1B)
            } else {
                t.ink
            };
            play_tri(&p, Pos2::new(sr.left() + 10.0, sr.center().y), 4.5, ink);
            // a long scene name ends in "…" (the tooltip has it whole)
            crate::theme::text_fit(
                &p,
                Pos2::new(sr.left() + 18.0, sr.center().y),
                egui::Align2::LEFT_CENTER,
                name,
                crate::theme::prop(crate::theme::ts::LABEL),
                ink,
                sr.width() - 21.0,
            );
            self.ca.scene_rects.push((b, sr));
        }
        if let Some(b) = clicked_scene {
            self.ca_scene(b);
        }
        // stop all
        let stop_x = gx1 + 6.0 + STOP_W / 2.0;
        let sa = Rect::from_center_size(
            Pos2::new(stop_x, (hy0 + hy1) / 2.0),
            egui::vec2(STOP_W, hy1 - hy0),
        );
        let resp = ui
            .interact(sa, ui.make_persistent_id("ca_stop_all"), Sense::click())
            .on_hover_text("stop all: playback stops at the next bar (Launch)");
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Stop all"));
        let halt_q = cue.is_some_and(|c| c.halt);
        p.rect_filled(
            sa,
            5.0,
            Color32::from_black_alpha(if resp.hovered() { 120 } else { 80 }),
        );
        let sq = if halt_q {
            t.brick.gamma_multiply(0.4 + 0.6 * k_pulse)
        } else if playing {
            t.brick
        } else {
            t.ink2.gamma_multiply(0.6)
        };
        p.rect_filled(
            Rect::from_center_size(sa.center(), egui::vec2(8.0, 8.0)),
            1.0,
            sq,
        );
        self.ca.stop_all = sa;
        if resp.clicked() {
            self.ca_stop_all();
        }
        // ---- rows: one per track, on the timeline's track rows
        self.ca.slot_rects.clear();
        self.ca.stop_rects.clear();
        let clip_all = Rect::from_min_max(
            Pos2::new(top.left(), self.tlv.heads.top().max(hy1 + 1.0)),
            Pos2::new(top.right(), self.tlv.heads.bottom()),
        );
        let pc = p.with_clip_rect(clip_all);
        let mut act: Option<(u8, usize, usize)> = None;
        let mut slot_op: Option<(u32, u8)> = None;
        // linked counts of the launcher's clips (the ×N badge, D4)
        // and their own colour and name (Stage 5)
        let links: std::collections::HashMap<u32, (usize, Option<u32>, Option<String>)> = if launch
        {
            self.with(|s| {
                s.song
                    .timeline
                    .as_deref()
                    .map_or_else(Default::default, |t| {
                        t.launcher
                            .iter()
                            .filter_map(|l| {
                                let d = l.clip.notes()?;
                                let (a, b) = tl::linked(t, d);
                                let name = l.clip.name.clone().filter(|n| !n.is_empty());
                                Some((l.clip.id, (a + b, l.clip.colour, name)))
                            })
                            .collect()
                    })
            })
        } else {
            Default::default()
        };
        // launcher clips with a follow action: their one-line summary
        let follows: std::collections::HashMap<u32, String> = if launch {
            self.with(|s| {
                s.song
                    .timeline
                    .as_deref()
                    .map_or_else(Default::default, |t| {
                        t.launcher
                            .iter()
                            .filter_map(|l| Some((l.clip.id, l.follow?.describe())))
                            .collect()
                    })
            })
        } else {
            Default::default()
        };
        for tr in 0..nch {
            let Some((hr, _)) = self.tlv.track_rects(tr) else {
                continue;
            };
            if hr.height() < 8.0 || hr.bottom() <= clip_all.top() || hr.top() >= clip_all.bottom() {
                continue;
            }
            let col = cols[tr];
            let row = Rect::from_min_max(
                Pos2::new(gx0, hr.top() + 2.0),
                Pos2::new(gx1, hr.bottom() - 2.0),
            );
            let row_held = held & bit(tr) != 0;
            let row_off_q = cue.is_some_and(|c| c.off & bit(tr) != 0);
            let row_on_q =
                cue.is_some_and(|c| c.on & bit(tr) != 0 && (row_held || c.scene.is_some()));
            let mut any_play = false;
            for k in 0..nvis {
                let b = first + k;
                if b >= nb {
                    break;
                }
                let sr = Rect::from_min_max(
                    Pos2::new(gx0 + k as f32 * sw + 2.0, row.top()),
                    Pos2::new(gx0 + (k + 1) as f32 * sw - 2.0, row.bottom()),
                );
                let lid = if launch { self.ca_clip(tr, b) } else { None };
                let empty = if launch {
                    lid.is_none()
                } else {
                    self.ca
                        .prev
                        .get(b)
                        .and_then(|v| v.get(tr))
                        .is_none_or(|s| s.notes.is_empty())
                };
                let strip = Rect::from_min_max(sr.min, Pos2::new(sr.left() + 15.0, sr.bottom()));
                let vis = sr.intersect(clip_all);
                self.ca.slot_rects.push((tr, b, strip, sr));
                if empty {
                    pc.circle_filled(sr.center(), 2.2, t.empty);
                    if launch && vis.is_positive() {
                        let resp = ui
                            .interact(
                                vis,
                                ui.make_persistent_id(("ca_empty", tr, b)),
                                Sense::click(),
                            )
                            .on_hover_text(format!(
                                "{}: empty slot of track {:02} · double-click: a new clip here, \
                                 open in the piano roll (it plays the track's instrument)",
                                names[b],
                                tr + 1
                            ));
                        resp.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Button,
                                true,
                                format!("Empty slot {:02} {b:02}", tr + 1),
                            )
                        });
                        if resp.hovered() {
                            pc.rect_stroke(
                                sr,
                                4.0,
                                Stroke::new(1.0, Color32::from_white_alpha(40)),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if resp.double_clicked() {
                            act = Some((2, tr, b));
                        } else if resp.clicked() {
                            act = Some((3, tr, b));
                        }
                    }
                    continue;
                }
                // playing here: the scene (track not held), or the arrangement's clip
                let pl = if scene == Some(b) && !row_held {
                    Some(prog)
                } else {
                    under[tr].filter(|x| x.0 == b).map(|x| x.1)
                };
                any_play |= pl.is_some();
                let queued = if launch {
                    cue.is_some_and(|c| c.clips[tr] != 0 && Some(c.clips[tr]) == lid)
                } else {
                    cue.is_some_and(|c| {
                        let target = c.scene.or(scene);
                        target == Some(b) && c.on & bit(tr) != 0 && (c.scene.is_some() || row_held)
                    }) && pl.is_none_or(|_| row_held)
                };
                let hover_id = ui.make_persistent_id(("ca_slot", tr, b));
                let resp = ui.interact(vis, hover_id, Sense::click_and_drag());
                let strip_hover = resp.hover_pos().is_some_and(|q| strip.contains(q));
                let tip = if launch {
                    format!(
                        "{}: track {:02}'s launcher clip. ▶ plays it, looping, from the next {} (■ while playing: click stops it there) (the other \
                         tracks play on) · drag it onto the timeline for a linked copy (Alt: an independent \
                         one) · double-click opens it in the piano roll · right-click: play, edit, place, rename, colour…",
                        names[b],
                        tr + 1,
                        self.ca_when()
                    )
                } else if secs.is_some() {
                    format!(
                        "{}: track {:02}'s clip. ▶ plays it, looping, from the next {} (■ while playing: click stops it there) · drag it onto the \
                         timeline for a linked copy (Alt: an independent one) · double-click opens it",
                        names[b],
                        tr + 1,
                        self.ca_when()
                    )
                } else {
                    format!(
                        "{}: track {:02}'s column of block {b:02}. ▶ plays it from the next {} (■ while playing: click stops it there) · drag it onto the \
                         timeline for a {} · double-click opens it in the Tracker",
                        names[b],
                        tr + 1,
                        self.ca_when(),
                        self.clip_word()
                    )
                };
                let tip = match lid.and_then(|id| follows.get(&id)) {
                    Some(f) => {
                        format!("{tip}\n\nFollow action: {f} (right-click › Follow action…)")
                    }
                    None => tip,
                };
                let resp = resp.on_hover_text(tip);
                if let (Some(id), false, true) =
                    (lid, self.extra.clip_list_menu, resp.secondary_clicked())
                {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.opie_req = Some((super::objpie::PieFor::Launch(id), p));
                    }
                } else if let (Some(id), true) = (lid, self.extra.clip_list_menu) {
                    resp.context_menu(|ui| {
                        if ui
                            .button("Duplicate (variation)")
                            .on_hover_text(
                                "an independent copy in the next free scene of this track",
                            )
                            .clicked()
                        {
                            slot_op = Some((id, 1));
                            ui.close();
                        }
                        if ui
                            .button("Follow action…")
                            .on_hover_text("after some loops, move on to another clip by itself")
                            .clicked()
                        {
                            slot_op = Some((id, 3));
                            ui.close();
                        }
                        if ui.button("Delete").clicked() {
                            slot_op = Some((id, 2));
                            ui.close();
                        }
                    });
                }
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        format!("Slot {:02} {b:02}", tr + 1),
                    )
                });
                let src = if secs.is_some() {
                    self.ca_clip(tr, b).map(super::dropplan::DropSrc::Clip)
                } else {
                    Some(super::dropplan::DropSrc::Block { block: b, col: tr })
                };
                if resp.drag_started()
                    && !resp.drag_started_by(egui::PointerButton::Secondary)
                    && src.is_some()
                {
                    self.ca.drag = src;
                } else if resp.double_clicked() {
                    act = Some((2, tr, b));
                } else if resp.clicked() {
                    // ▶ toggles: playing here (and no stop queued yet) =
                    // stop the track at the next launch boundary (■)
                    act = Some(if !strip_hover {
                        (3, tr, b)
                    } else if pl.is_some() && !row_held && !row_off_q {
                        (4, tr, b)
                    } else {
                        (1, tr, b)
                    });
                }
                let dim = if row_held { 0.55 } else { 1.0 };
                let look = lid.and_then(|id| links.get(&id));
                let col = look.and_then(|x| x.1).map_or(col, |x| {
                    Color32::from_rgb((x >> 16) as u8, (x >> 8) as u8, x as u8)
                });
                let fill = if pl.is_some() {
                    theme::mix(col, Color32::BLACK, 0.15)
                } else {
                    theme::mix(col, t.bg_bot, 0.52)
                };
                pc.rect_filled(sr, 4.0, fill.gamma_multiply(dim));
                pc.rect_filled(
                    strip,
                    egui::CornerRadius {
                        nw: 4,
                        sw: 4,
                        ne: 0,
                        se: 0,
                    },
                    theme::mix(col, Color32::BLACK, if strip_hover { 0.25 } else { 0.45 }),
                );
                if pl.is_some() && !row_held {
                    // playing: ■ (click stops it); a queued stop shows dimmed
                    let q = if row_off_q { 0.45 } else { 1.0 };
                    pc.rect_filled(
                        Rect::from_center_size(strip.center(), egui::vec2(7.0, 7.0)),
                        1.0,
                        t.accent.gamma_multiply(q),
                    );
                } else {
                    play_tri(
                        &pc,
                        strip.center(),
                        4.0,
                        if pl.is_some() {
                            t.accent
                        } else {
                            Color32::from_white_alpha(220)
                        },
                    );
                }
                // mini preview: the column's notes
                if let Some(sp) = self.ca.prev.get(b).and_then(|v| v.get(tr)) {
                    let pv = Rect::from_min_max(
                        Pos2::new(strip.right() + 3.0, sr.top() + 4.0),
                        sr.max - egui::vec2(3.0, 5.0),
                    );
                    let span = (sp.hi as f32 - sp.lo as f32).max(1.0);
                    let pv = if sp.hi == sp.lo {
                        pv.shrink2(egui::vec2(0.0, pv.height() * 0.45))
                    } else {
                        pv
                    };
                    let ink = theme::mix(col, Color32::WHITE, 0.62).gamma_multiply(dim);
                    for (x, w, n) in &sp.notes {
                        let y = pv.bottom()
                            - ((*n as f32 - sp.lo as f32) / span) * (pv.height() - 2.0)
                            - 1.0;
                        let x0 = pv.left() + x * pv.width();
                        let x1 = (x0 + (w * pv.width()).max(2.0) - 1.0).min(pv.right());
                        pc.rect_filled(
                            Rect::from_min_max(Pos2::new(x0, y - 1.1), Pos2::new(x1, y + 1.1)),
                            0.0,
                            ink,
                        );
                    }
                }
                if let Some(name) = look
                    .and_then(|x| x.2.as_deref())
                    .filter(|_| sr.height() >= 22.0)
                {
                    pc.text(
                        Pos2::new(strip.right() + 3.0, sr.bottom() - 2.0),
                        egui::Align2::LEFT_BOTTOM,
                        name,
                        crate::theme::prop(crate::theme::ts::SMALL),
                        t.ink,
                    );
                }
                if let Some(n) = look.map(|x| x.0).filter(|&n| n > 1) {
                    let g = pc.layout_no_wrap(
                        format!("×{n}"),
                        crate::theme::prop(crate::theme::ts::SMALL),
                        t.ink,
                    );
                    let br = Rect::from_min_size(
                        Pos2::new(sr.right() - g.size().x - 7.0, sr.top() + 2.0),
                        g.size() + egui::vec2(5.0, 1.0),
                    );
                    pc.rect_filled(br, 3.0, Color32::from_black_alpha(150));
                    pc.galley(br.min + egui::vec2(2.5, 0.5), g, t.ink);
                }
                // a follow action: » at the bottom right
                if lid.is_some_and(|id| follows.contains_key(&id)) && sr.height() >= 14.0 {
                    let g = pc.layout_no_wrap(
                        "»".into(),
                        crate::theme::prop(crate::theme::ts::SMALL),
                        t.ink,
                    );
                    let br = Rect::from_min_size(
                        Pos2::new(
                            sr.right() - g.size().x - 7.0,
                            sr.bottom() - g.size().y - 3.0,
                        ),
                        g.size() + egui::vec2(5.0, 1.0),
                    );
                    pc.rect_filled(br, 3.0, Color32::from_black_alpha(150));
                    pc.galley(br.min + egui::vec2(2.5, 0.5), g, t.ink);
                }
                // the launcher clip open in the piano roll (5.6)
                if lid.is_some() && lid == self.pr.launch && self.pr.open && self.tlv.sel.is_none()
                {
                    pc.rect_stroke(
                        sr,
                        4.0,
                        Stroke::new(1.5, t.accent),
                        egui::StrokeKind::Inside,
                    );
                }
                if self.ca.drag.is_some() && self.ca.drag == src {
                    pc.rect_stroke(
                        sr.expand(1.0),
                        5.0,
                        Stroke::new(2.0, t.ink),
                        egui::StrokeKind::Inside,
                    );
                }
                if let Some(f) = pl {
                    pc.rect_stroke(
                        sr,
                        4.0,
                        Stroke::new(1.5, t.accent),
                        egui::StrokeKind::Inside,
                    );
                    let bar = Rect::from_min_max(
                        Pos2::new(strip.right(), sr.bottom() - 3.0),
                        Pos2::new(
                            strip.right() + (sr.right() - strip.right()) * f.clamp(0.0, 1.0),
                            sr.bottom(),
                        ),
                    );
                    pc.rect_filled(bar, 0.0, t.accent);
                } else if queued {
                    pc.rect_stroke(
                        sr,
                        4.0,
                        Stroke::new(1.5, t.accent.gamma_multiply(0.3 + 0.7 * k_pulse)),
                        egui::StrokeKind::Inside,
                    );
                } else {
                    pc.rect_stroke(
                        sr,
                        4.0,
                        Stroke::new(1.0, Color32::from_black_alpha(90)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            // per-track stop
            let st = Rect::from_center_size(
                Pos2::new(stop_x, row.center().y),
                egui::vec2(STOP_W - 2.0, 20.0_f32.min(row.height())),
            );
            let vis = st.intersect(clip_all);
            if vis.is_positive() {
                let resp = ui
                    .interact(vis, ui.make_persistent_id(("ca_stop", tr)), Sense::click())
                    .on_hover_text(format!(
                        "stop track {:02} at the next {}",
                        tr + 1,
                        self.ca_when()
                    ));
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        format!("Stop track {:02}", tr + 1),
                    )
                });
                if resp.clicked() {
                    act = Some((4, tr, 0));
                }
                let lit = any_play && !row_held;
                pc.rect_filled(
                    st,
                    4.0,
                    Color32::from_black_alpha(if lit || resp.hovered() { 110 } else { 55 }),
                );
                let c = if row_off_q {
                    t.brick.gamma_multiply(0.4 + 0.6 * k_pulse)
                } else if lit {
                    t.ink
                } else {
                    t.ink2.gamma_multiply(0.55)
                };
                pc.rect_filled(
                    Rect::from_center_size(st.center(), egui::vec2(7.0, 7.0)),
                    1.0,
                    c,
                );
                if row_on_q {
                    pc.rect_stroke(
                        st,
                        4.0,
                        Stroke::new(1.0, t.accent.gamma_multiply(k_pulse)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            self.ca.stop_rects.push((tr, st));
        }
        // the Song / Master row: not launchable
        if let Some((hr, _)) = self.tlv.track_rects(nch) {
            // not over a track row scrolled under the docked footer
            let covered = (0..nch).any(|k| {
                self.tlv.track_rects(k).is_some_and(|(r, _)| {
                    r.height() > 1.0 && r.y_range().intersects(hr.shrink(2.0).y_range())
                })
            });
            if hr.height() > 10.0 && !covered && clip_all.y_range().contains(hr.center().y) {
                pc.text(
                    Pos2::new(gx0 + 4.0, hr.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Song / Master · not launchable",
                    crate::theme::prop(crate::theme::ts::LABEL),
                    t.ink2,
                );
            }
        }
        // status line under the rows
        let msg = if launch {
            let n = (0..nch).filter(|&k| under[k].is_some()).count();
            let mut m = if n > 0 {
                format!("{n} launcher clip{} playing", if n == 1 { "" } else { "s" })
            } else {
                "Launcher: ▶ plays a clip · double-click an empty slot for a new one".into()
            };
            if cue.is_some() {
                m += &format!(" · changes queued (next {})", self.ca_when());
            }
            m
        } else if secs.is_some() {
            format!(
                "From the arrangement · {}",
                self.ca_status(&names, scene, cue, held, nch, playing)
            )
        } else {
            self.ca_status(&names, scene, cue, held, nch, playing)
        };
        // the launcher's own button at the status line's right end
        let mut text_clip = top.shrink(6.0);
        if secs.is_some() {
            let (label, tip, op) = if launch {
                (
                    "+ Scene",
                    "a new empty scene row (double-click its slots for new clips)".to_string(),
                    1,
                )
            } else {
                (
                    "Capture",
                    "Capture into the launcher: these scenes become clips of the launcher's own, \
                     linked to the arrangement's (editing the notes changes both; position, length \
                     and name are each clip's own)"
                        .to_string(),
                    2,
                )
            };
            // sized from its text (the type scale and Text size)
            let bf = crate::theme::prop(crate::theme::ts::SMALL);
            let w = crate::theme::text_w(&p, label, bf.clone()) + 16.0;
            let br = Rect::from_center_size(
                Pos2::new(inner.right() - w / 2.0, top.bottom() - 13.0),
                egui::vec2(w, (crate::theme::row_h(&p, &bf) + 4.0).max(18.0)),
            );
            text_clip.max.x = br.left() - 6.0;
            let resp = ui
                .put(
                    br,
                    // the measured font (not the Button style's semibold),
                    // never wrapped
                    egui::Button::new(egui::RichText::new(label).font(bf.clone()))
                        .small()
                        .wrap_mode(egui::TextWrapMode::Extend),
                )
                .on_hover_text(tip);
            if resp.clicked() {
                title_op = Some(op);
            }
        }
        // cut with "…" before the button; the full line is its tooltip
        let (tr_, cut) = crate::theme::text_fit(
            &p.with_clip_rect(text_clip),
            Pos2::new(gx0 + 2.0, top.bottom() - 13.0),
            egui::Align2::LEFT_CENTER,
            &msg,
            crate::theme::prop(crate::theme::ts::LABEL),
            t.ink2,
            text_clip.right() - (gx0 + 2.0),
        );
        if cut {
            ui.interact(tr_, ui.scope_id().with("ca_status_tip"), Sense::hover())
                .on_hover_text(msg.clone());
        }
        self.ca.line = msg;
        match title_op {
            Some(1) => self.ca_add_scene(),
            Some(2) => self.ca_capture(),
            _ => {}
        }
        match scene_op {
            Some((k, 1)) => self.ca_delete_scene(k),
            Some((k, 2)) => self.ca_duplicate_scene(k),
            _ => {}
        }
        match slot_op {
            Some((id, 1)) => self.ca_variation(id),
            Some((id, 2)) => self.ca_delete(id),
            Some((id, 3)) => {
                let at = ui.ctx().pointer_latest_pos().unwrap_or(top.center());
                self.follow_open(id, at);
            }
            _ => {}
        }
        match act {
            Some((1, tr, b)) => self.ca_slot(tr, b),
            Some((2, tr, b)) if launch => self.ca_open_launch(tr, b),
            Some((2, tr, b)) if secs.is_some() => self.ca_open_clip(tr, b),
            Some((3, tr, b)) if launch => {
                self.ch = tr;
                if let Some(id) = self.ca_clip(tr, b) {
                    self.tlv.sels.clear();
                    self.tlv.sel = None;
                    self.pr.launch = Some(id);
                }
            }
            Some((2, tr, b)) => self.timeline_open(b, tr, 0, usize::MAX),
            Some((3, tr, b)) if secs.is_some() => {
                self.ch = tr;
                if let Some(id) = self.ca_clip(tr, b) {
                    self.tlv.sels = vec![id];
                    self.tlv.sel = Some(id);
                }
            }
            Some((3, tr, _)) => self.ch = tr,
            Some((4, tr, _)) => self.ca_stop(tr),
            _ => {}
        }
    }

    fn ca_status(
        &self,
        names: &[String],
        scene: Option<usize>,
        cue: Option<Cue>,
        held: u64,
        nch: usize,
        playing: bool,
    ) -> String {
        let name = |b: usize| names.get(b).cloned().unwrap_or_default();
        let mut s = match scene {
            Some(b) => format!("Scene {} playing", name(b)),
            None if playing => "The arrangement is playing".into(),
            None => "Click a scene to play it · drag a slot onto the timeline".into(),
        };
        let stopped = (0..nch).filter(|&k| held & bit(k) != 0).count();
        if stopped > 0 && scene.is_some() {
            s += &format!(
                " · {stopped} track{} stopped",
                if stopped == 1 { "" } else { "s" }
            );
        }
        if let Some(c) = cue {
            let what = if c.halt {
                "stop".to_string()
            } else if let Some(b) = c.scene {
                format!("{} queued", name(b))
            } else {
                "changes queued".into()
            };
            s += &format!(" · {what} (next {})", self.ca_when());
        }
        s
    }

    /// a slot dragged onto the timeline: the drop preview, then a copy of
    /// the slot's clip where it says (see dropplan.rs)
    fn ca_drag(&mut self, ctx: &egui::Context) {
        let Some(src) = self.ca.drag else { return };
        let (pos, down, released, mods, esc) = match self.ca.shot_drag {
            Some(q) => (Some(q), true, false, self.ca.shot_mods, false),
            None => ctx.input(|i| {
                (
                    i.pointer.latest_pos(),
                    i.pointer.primary_down(),
                    i.pointer.primary_released(),
                    i.modifiers,
                    i.key_pressed(Key::Escape),
                )
            }),
        };
        if esc {
            self.ca.drag = None;
            self.status = "Drag cancelled".into();
            return;
        }
        let plan = pos.and_then(|q| self.drop_plan(q, src, mods.shift));
        if released || !down {
            self.ca.drag = None;
            if released {
                self.drop_release(src, plan, mods.alt);
            }
            return;
        }
        self.drop_paint(ctx, src, plan.as_ref(), pos, mods.alt);
        let ok = plan.as_ref().is_some_and(|p| p.bad.is_none());
        ctx.set_cursor_icon(if ok {
            egui::CursorIcon::Grabbing
        } else {
            egui::CursorIcon::NoDrop
        });
        ctx.request_repaint();
    }

    /// the Master panel in the Slate theme (mockup a): pic-1 knobs, LEVEL /
    /// REDUCTION dot rows, the level history as glowing bars in a well
    fn ca_master_ember(
        &mut self,
        ui: &mut egui::Ui,
        vol: &mut f32,
        lim: &mut crate::master::LimiterSettings,
        peaks: [f32; 2],
        loud: &crate::master::Readout,
    ) {
        let t = th();
        ui.horizontal(|ui| {
            crate::ember::caps_label(ui, "Master", 14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let txt = if lim.enabled {
                    "Limiter on"
                } else {
                    "Limiter off"
                };
                if crate::ember::button(ui, txt, lim.enabled)
                    .on_hover_text(
                        "the true-peak limiter on the master (off: the classic soft clipper)",
                    )
                    .clicked()
                {
                    lim.enabled = !lim.enabled;
                }
            });
        });
        let row = ui.available_rect_before_wrap();
        let cy = row.top() + 44.0;
        let w = row.width();
        let xs = [row.left() + w / 6.0, row.center().x, row.right() - w / 6.0];
        let id = ui.make_persistent_id("ca_master_ember");
        let db = |v: f32| {
            if v <= 0.0001 {
                "-inf".to_string()
            } else {
                format!("{:+.1}", 20.0 * v.log10())
            }
        };
        let mut v = (*vol / 2.0).clamp(0.0, 1.0);
        let r = super::mixstrip::big(
            ui,
            Pos2::new(xs[0], cy),
            id.with(0),
            &mut v,
            0.5,
            "Volume",
            &db(*vol),
            ("-inf", "+6"),
            true,
        )
        .on_hover_text("master volume (the mixer's Master fader)");
        if r.dragged() || r.double_clicked() {
            *vol = v * 2.0;
        }
        let en = lim.enabled;
        let mut g = (lim.gain_db / 24.0).clamp(0.0, 1.0);
        let r = super::mixstrip::big(
            ui,
            Pos2::new(xs[1], cy),
            id.with(1),
            &mut g,
            crate::master::LimiterSettings::default().gain_db / 24.0,
            "Drive",
            &format!("{:.1}", lim.gain_db),
            ("0", "24"),
            en,
        )
        .on_hover_text("limiter input gain: drives the mix into the limiter (louder, denser)");
        if r.dragged() || r.double_clicked() {
            lim.gain_db = g * 24.0;
        }
        let mut c = ((lim.ceiling_db + 12.0) / 12.0).clamp(0.0, 1.0);
        let r = super::mixstrip::big(
            ui,
            Pos2::new(xs[2], cy),
            id.with(2),
            &mut c,
            (crate::master::LimiterSettings::default().ceiling_db + 12.0) / 12.0,
            "Ceiling",
            &format!("{:.1}", lim.ceiling_db),
            ("-12", "0"),
            en,
        )
        .on_hover_text("limiter output ceiling (true peak)");
        if r.dragged() || r.double_clicked() {
            lim.ceiling_db = c * 12.0 - 12.0;
        }
        let below = cy + 54.0 - ui.cursor().top();
        if below > 0.0 {
            ui.add_space(below);
        }
        let frac = |v: f32| {
            if v <= 0.0 {
                0.0
            } else {
                ((20.0 * v.log10() + 48.0) / 51.0).clamp(0.0, 1.0)
            }
        };
        let num = |v: f32| {
            if v > 1e-5 {
                format!("{:.1}", 20.0 * v.log10())
            } else {
                "-inf".into()
            }
        };
        super::mixstrip::dot_row(ui, "Level L", frac(peaks[0]), true, &num(peaks[0]));
        super::mixstrip::dot_row(ui, "Level R", frac(peaks[1]), true, &num(peaks[1]));
        super::mixstrip::dot_row(
            ui,
            "Reduction",
            (-loud.gain_red / 12.0).clamp(0.0, 1.0),
            false,
            &format!("{:.1}", loud.gain_red),
        );
        ui.add_space(4.0);
        let fmt = |v: f32, unit: &str| {
            if v.is_finite() && v > -70.0 {
                format!("{v:.1} {unit}")
            } else {
                format!("–– {unit}")
            }
        };
        ui.label(
            egui::RichText::new(format!(
                "short-term {} · peak {}",
                fmt(loud.short_term, "LUFS"),
                fmt(loud.true_peak, "dBTP")
            ))
            .size(crate::theme::ts::px(crate::theme::ts::SMALL))
            .color(t.ink2),
        );
        // the level over the last 30 s: glowing bars in a well
        // (right-click: keep scrolling, pause or hide)
        if let Some(gr) = self.level_graph(ui) {
            let pp = ui.painter();
            crate::ember::well(pp, gr, 4);
            for db in [-6.0f32, -18.0, -30.0] {
                let y = gr.bottom() - ((db + 48.0) / 51.0) * gr.height();
                pp.hline(
                    gr.x_range().shrink(1.0),
                    y,
                    Stroke::new(1.0, crate::ember::GRID),
                );
            }
            let n = self.ca.hist.len();
            let bw = (gr.width() - 4.0) / HIST as f32;
            let inner = gr.shrink(2.0);
            let mut bloom = egui::Mesh::default();
            let mut bars = egui::Mesh::default();
            for (k, v) in self.ca.hist.iter().enumerate() {
                let f = frac(*v);
                if f <= 0.0 {
                    continue;
                }
                let x = inner.right() - (n - k) as f32 * bw;
                let top = inner.bottom() - f * inner.height();
                let b = Rect::from_min_max(
                    Pos2::new(x + 0.5, top),
                    Pos2::new(x + bw - 0.5, inner.bottom()),
                );
                theme::round_mesh(
                    &mut bloom,
                    b.expand2(egui::vec2(2.0, 3.0)),
                    [0.0; 4],
                    t.accent.gamma_multiply(0.10),
                    Color32::TRANSPARENT,
                );
                theme::round_mesh(
                    &mut bars,
                    b,
                    [0.0; 4],
                    t.accent.gamma_multiply(0.85),
                    t.acc_deep.gamma_multiply(0.15),
                );
                bars.add_colored_rect(
                    Rect::from_min_max(b.min, Pos2::new(b.right(), b.top() + 1.5)),
                    t.acc_hot,
                );
            }
            if crate::ember::bloom() {
                pp.add(egui::Shape::mesh(bloom));
            }
            pp.add(egui::Shape::mesh(bars));
            pp.text(
                gr.left_top() + egui::vec2(6.0, 4.0),
                egui::Align2::LEFT_TOP,
                "LEVEL",
                theme::label_font(crate::theme::ts::SMALL),
                t.ink2,
            );
        }
    }

    /// Room for the Master panel's LEVEL history, with its right-click menu
    /// (Scrolling / Paused / Hidden). None when there is no room or it is
    /// hidden; hidden leaves a slim strip that brings it back.
    fn level_graph(&mut self, ui: &mut egui::Ui) -> Option<Rect> {
        let t = th();
        let room = ui.available_height() - 4.0;
        let hidden = self.extra.level_hist == 2;
        if room <= if hidden { 18.0 } else { 36.0 } {
            return None;
        }
        let h = if hidden { 18.0 } else { room.min(150.0) };
        let (gr, resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
        self.ca.level_rect = gr;
        if hidden {
            ui.painter().text(
                gr.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                "Level history hidden · right-click to show",
                crate::theme::prop(crate::theme::ts::SMALL),
                t.ink2,
            );
        } else if self.extra.level_hist == 1 {
            ui.painter().text(
                gr.right_top() + egui::vec2(-6.0, 4.0),
                egui::Align2::RIGHT_TOP,
                "PAUSED",
                theme::label_font(crate::theme::ts::SMALL),
                t.ink2,
            );
        }
        let mut pick = None;
        let resp = resp.on_hover_text("Level over the last 30 s · right-click to pause or hide");
        resp.context_menu(|ui| {
            let cur = self.extra.level_hist;
            for (v, txt) in [(0u8, "Scrolling"), (1, "Paused"), (2, "Hidden")] {
                if ui.radio(cur == v, txt).clicked() {
                    pick = Some(v);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Clear history").clicked() {
                pick = Some(cur);
                self.ca.hist.clear();
                ui.close();
            }
        });
        if let Some(v) = pick {
            if v != self.extra.level_hist {
                self.extra.level_hist = v;
                self.save_settings();
            }
        }
        (!hidden).then_some(gr)
    }

    /// Master: volume, limiter drive / ceiling, meters, loudness
    pub(super) fn ca_master(&mut self, ui: &mut egui::Ui, r: Rect) {
        let t = th();
        let p = ui.painter().clone();
        theme::cartridge(&p, r, t.slate);
        self.ca.master = r;
        let (mut vol, mut lim, peaks, loud) = self.with(|s| {
            (
                s.song.mixer.master,
                s.song.mixer.limiter,
                s.engine.peak_lr,
                s.engine.loud,
            )
        });
        let (vol0, lim0) = (vol, lim);
        let inner = r.shrink2(egui::vec2(14.0, 8.0));
        if t.ember {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| self.ca_master_ember(ui, &mut vol, &mut lim, peaks, &loud),
            );
        } else {
            ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Master").strong().size(crate::theme::ts::px(crate::theme::ts::LARGE)).color(t.ink));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let txt = if lim.enabled { "Limiter on" } else { "Limiter off" };
                        if ui
                            .add(egui::Button::selectable(
                                lim.enabled,
                                egui::RichText::new(txt).size(crate::theme::ts::px(crate::theme::ts::SMALL)),
                            ))
                            .on_hover_text(
                                "the true-peak limiter on the master (off: the classic soft clipper)",
                            )
                            .clicked()
                        {
                            lim.enabled = !lim.enabled;
                        }
                    });
                });
                ui.add_space(2.0);
                let db = |v: f32| {
                    if v <= 0.0001 {
                        "−∞ dB".to_string()
                    } else {
                        format!("{:+.1} dB", 20.0 * v.log10())
                    }
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let room = ui.available_width() - 3.0 * knob::CELL.x;
                    ui.add_space((room / 2.0).max(0.0));
                    let mut v = (vol / 2.0).clamp(0.0, 1.0);
                    let o = knob::knob(
                        ui,
                        ui.make_persistent_id("ca_master_vol"),
                        &mut v,
                        knob::KnobArgs {
                            caption: None,
                            name: "Volume",
                            value_text: db(vol),
                            default: 0.5,
                            steps: 0.0,
                            enabled: true,
                            highlight: false,
                            badge: None,
                            locked: None,
                            tip: "master volume (the mixer's Master fader)",
                            rings: &[],
                            live: None,
                            auto_led: None,
                            mod_dot: None,
                        },
                    );
                    if o.changed {
                        vol = v * 2.0;
                    }
                    let mut g = (lim.gain_db / 24.0).clamp(0.0, 1.0);
                    let o = knob::knob(
                        ui,
                        ui.make_persistent_id("ca_master_drive"),
                        &mut g,
                        knob::KnobArgs {
                            caption: None,
                            name: "Drive",
                            value_text: format!("+{:.1} dB", lim.gain_db),
                            default: crate::master::LimiterSettings::default().gain_db / 24.0,
                            steps: 0.0,
                            enabled: lim.enabled,
                            highlight: false,
                            badge: None,
                            locked: None,
                            tip: "limiter input gain: drives the mix into the limiter (louder, denser)",
                            rings: &[],
                            live: None,
                            auto_led: None,
                            mod_dot: None,
                        },
                    );
                    if o.changed {
                        lim.gain_db = g * 24.0;
                    }
                    let mut c = ((lim.ceiling_db + 12.0) / 12.0).clamp(0.0, 1.0);
                    let o = knob::knob(
                        ui,
                        ui.make_persistent_id("ca_master_ceiling"),
                        &mut c,
                        knob::KnobArgs {
                            caption: None,
                            name: "Ceiling",
                            value_text: format!("{:.1} dBTP", lim.ceiling_db),
                            default: (crate::master::LimiterSettings::default().ceiling_db + 12.0) / 12.0,
                            steps: 0.0,
                            enabled: lim.enabled,
                            highlight: false,
                            badge: None,
                            locked: None,
                            tip: "limiter output ceiling (true peak)",
                            rings: &[],
                            live: None,
                            auto_led: None,
                            mod_dot: None,
                        },
                    );
                    if o.changed {
                        lim.ceiling_db = c * 12.0 - 12.0;
                    }
                });
                ui.add_space(4.0);
                let frac = |v: f32| {
                    if v <= 0.0 {
                        0.0
                    } else {
                        ((20.0 * v.log10() + 48.0) / 51.0).clamp(0.0, 1.0)
                    }
                };
                let rows = [
                    ("L", frac(peaks[0]), false),
                    ("R", frac(peaks[1]), false),
                    ("GR", (-loud.gain_red / 12.0).clamp(0.0, 1.0), true),
                ];
                for (n, f, gr) in rows {
                    let (rr, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 13.0), Sense::hover());
                    let pp = ui.painter();
                    pp.text(
                        Pos2::new(rr.left(), rr.center().y),
                        egui::Align2::LEFT_CENTER,
                        n,
                        crate::theme::prop(crate::theme::ts::SMALL),
                        t.ink2,
                    );
                    let bar = Rect::from_min_max(Pos2::new(rr.left() + 24.0, rr.top() + 3.0), Pos2::new(rr.right(), rr.bottom() - 3.0));
                    let segs = 24;
                    let w = bar.width() / segs as f32;
                    let lit = (f * segs as f32).round() as usize;
                    for k in 0..segs {
                        let sr = Rect::from_min_size(Pos2::new(bar.left() + k as f32 * w + 1.0, bar.top()), egui::vec2(w - 2.0, bar.height()));
                        let c = if k < lit {
                            if gr {
                                t.ochre
                            } else if k >= segs - 2 {
                                t.brick
                            } else if k >= segs - 6 {
                                t.ochre
                            } else {
                                t.accent
                            }
                        } else {
                            Color32::from_black_alpha(if t.light { 30 } else { 70 })
                        };
                        pp.rect_filled(sr, 1.0, c);
                    }
                }
                ui.add_space(4.0);
                let fmt = |v: f32, unit: &str| {
                    if v.is_finite() && v > -70.0 {
                        format!("{v:.1} {unit}")
                    } else {
                        format!("–– {unit}")
                    }
                };
                ui.label(
                    egui::RichText::new(format!(
                        "short-term {} · peak {}",
                        fmt(loud.short_term, "LUFS"),
                        fmt(loud.true_peak, "dBTP")
                    ))
                    .size(crate::theme::ts::px(crate::theme::ts::SMALL))
                    .color(t.ink2),
                );
                // the level over the last 30 s (one column per quarter
                // second of playing), filling the room left
                if let Some(gr) = self.level_graph(ui) {
                    let pp = ui.painter();
                    pp.rect_filled(gr, 4.0, Color32::from_black_alpha(if t.light { 25 } else { 60 }));
                    for db in [-6.0f32, -18.0, -30.0] {
                        let y = gr.bottom() - ((db + 48.0) / 51.0) * gr.height();
                        pp.hline(gr.x_range(), y, Stroke::new(1.0, t.crt_grid.gamma_multiply(0.6)));
                    }
                    let n = self.ca.hist.len();
                    let w = gr.width() / HIST as f32;
                    for (k, v) in self.ca.hist.iter().enumerate() {
                        let f = frac(*v);
                        if f <= 0.0 {
                            continue;
                        }
                        let x = gr.right() - (n - k) as f32 * w;
                        let bar = Rect::from_min_max(
                            Pos2::new(x + 0.5, gr.bottom() - f * gr.height()),
                            Pos2::new(x + w - 0.5, gr.bottom()),
                        );
                        pp.rect_filled(bar, 0.0, t.accent.gamma_multiply(0.35 + 0.5 * f));
                    }
                    pp.text(
                        gr.left_top() + egui::vec2(6.0, 4.0),
                        egui::Align2::LEFT_TOP,
                        "level",
                        crate::theme::prop(crate::theme::ts::SMALL),
                        t.ink2,
                    );
                }
            },
        );
        }
        let playing = self.with(|s| s.engine.playing);
        // (the LCD toolbar's Master steps it every frame instead)
        if playing && self.extra.level_hist != 1 && !self.master_in_toolbar() {
            let dt = ui.input(|i| i.stable_dt);
            self.ca.hist_step(dt, peaks[0].max(peaks[1]));
        }
        if vol != vol0 || lim != lim0 {
            self.with(|s| {
                s.song.mixer.master = vol;
                s.song.mixer.limiter = lim;
            });
            self.dirty = true;
        }
    }
}
