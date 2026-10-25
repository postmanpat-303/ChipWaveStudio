//! What the engine plays in timeline mode (docs/TIMELINE.md, plan §2.1):
//! every track's clips resolved into non-overlapping row segments, built on
//! the UI thread after an edit and handed to the engine behind an `Arc`, so
//! the audio thread only follows cursors (no allocation, O(1) per row).
use super::*;
use std::hash::{Hash, Hasher};

/// One stretch of rows on a track that plays one block column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seg {
    /// timeline rows [start, end)
    pub start: usize,
    pub end: usize,
    pub pattern: usize,
    pub col: usize,
    /// source row at `start`
    pub src0: usize,
    /// > 0: the source wraps every `loop_rows` rows
    pub loop_rows: usize,
}

impl Seg {
    /// the block row played at timeline row `row` (start <= row < end)
    pub fn src_row(&self, row: usize) -> usize {
        let d = row.saturating_sub(self.start);
        if self.loop_rows > 0 {
            (self.src0 + d) % self.loop_rows
        } else {
            self.src0 + d
        }
    }
    /// the source starts again from its first row at `row`
    pub fn wraps_at(&self, row: usize) -> bool {
        row > self.start && self.loop_rows > 0 && self.src_row(row) == 0
    }
}

/// One note of a note clip, placed on the timeline (absolute ticks).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NoteSpan {
    pub start: Tick,
    pub end: Tick,
    /// tracker note number (1 = C-0)
    pub key: u8,
    /// 0..1
    pub vel: f32,
    /// instrument slot (1-based, 0 = the track's current one)
    pub instr: u8,
    /// detune in semitones
    pub tune: f32,
}

/// One audio clip, placed on the timeline (M4). The file plays at its own
/// speed from source frame `off` at `start` (the engine converts its rate
/// while it plays); seeking finds the frame through the tempo map.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioSpan {
    pub start: Tick,
    pub end: Tick,
    /// index into `Plan::pcm`
    pub asset: u32,
    /// source frame at `start`
    pub off: f64,
    /// > 0: the source wraps every `loop_len` frames
    pub loop_len: f64,
    /// the file's sample rate
    pub rate: f64,
    pub gain: f32,
    pub fade_in: Tick,
    pub fade_out: Tick,
    pub shape: FadeShape,
    /// seconds from tick 0 to `start`
    pub secs0: f64,
}

impl AudioSpan {
    /// clip gain with the fades at timeline tick `t`
    #[inline]
    pub fn env(&self, t: f64) -> f32 {
        let mut g = self.gain;
        let u = t - self.start as f64;
        if self.fade_in > 0 && u < self.fade_in as f64 {
            g *= self.shape.gain((u / self.fade_in as f64) as f32);
        }
        let v = self.end as f64 - t;
        if self.fade_out > 0 && v < self.fade_out as f64 {
            g *= self.shape.gain((v / self.fade_out as f64) as f32);
        }
        g
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// [`plan_key`] of the timeline this was built from
    pub key: u64,
    /// segments per track, sorted, not overlapping
    pub tracks: Vec<Vec<Seg>>,
    /// rows until the song ends (then playback wraps to row 0)
    pub end: usize,
    /// loop region in rows, when switched on
    pub loop_rows: Option<(usize, usize)>,
    /// per-track effect speed (0 = the song speed)
    pub speeds: Vec<u8>,
    /// the tempo map changes the tempo after the start
    pub tempo_changes: bool,
    /// note-clip notes per track, sorted by start (M3b; empty for tracker
    /// songs, so they play exactly as before)
    pub notes: Vec<Vec<NoteSpan>>,
    /// audio clips per track, sorted by start (M4; empty without audio
    /// clips, so other songs play exactly as before)
    pub audio: Vec<Vec<AudioSpan>>,
    /// decoded files, indexed by `AudioSpan::asset` (`None` = missing)
    pub pcm: Vec<Option<Arc<crate::audiofile::Pcm>>>,
    /// the tempo map and start tempo (audio seeks)
    pub tempo: Vec<TempoPoint>,
    pub bpm: f32,
    /// launcher clips (Stage 4b; empty without them, so other songs play
    /// exactly as before)
    pub launch: Vec<LaunchNotes>,
}

/// A launcher clip as the engine plays it: its notes from tick 0, looping
/// at `len`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchNotes {
    pub id: u32,
    pub track: usize,
    pub len: Tick,
    pub notes: Vec<NoteSpan>,
}

impl Plan {
    /// any track has note-clip notes
    pub fn has_notes(&self) -> bool {
        self.notes.iter().any(|v| !v.is_empty())
    }

    /// any track has an audio clip that can sound
    pub fn has_audio(&self) -> bool {
        self.audio.iter().any(|v| !v.is_empty())
    }

    /// Source frame (before the loop wrap) of span `s` at tick `t`: the
    /// time since its start through the tempo map, at the file's rate.
    pub fn audio_frame(&self, s: &AudioSpan, t: f64) -> f64 {
        let ti = t.floor();
        let frac = t - ti;
        let ti = ti as Tick;
        let secs = tempo_seconds(&self.tempo, ti, self.bpm)
            + frac * 60.0 / (PPQ as f64 * tempo_bpm(&self.tempo, ti, self.bpm).max(1.0) as f64);
        s.off + (secs - s.secs0) * s.rate
    }

    /// index of the first note on `track` that starts at or after `t`
    pub fn note_seek(&self, track: usize, t: Tick) -> usize {
        self.notes
            .get(track)
            .map_or(0, |v| v.partition_point(|n| n.start < t))
    }

    /// index of the segment at or after `row` on `track` (binary search)
    pub fn seek(&self, track: usize, row: usize) -> usize {
        self.tracks
            .get(track)
            .map_or(0, |v| v.partition_point(|s| s.end <= row))
    }
}

/// Where the song ends in timeline mode: the last clip end, at least one
/// empty block (64 rows) so an empty song still plays.
pub fn play_end(tl: &Timeline) -> Tick {
    let e = tl
        .clips
        .iter()
        .filter(|c| !c.muted)
        .map(|c| c.end())
        .max()
        .unwrap_or(0);
    let e = (e.max(tl.end) + ROW - 1).div_euclid(ROW) * ROW;
    if e > 0 {
        e
    } else {
        rows_to_ticks(ROWS)
    }
}

/// [`plan_key`] of a live plan built with the Fill key `fill` held
pub fn plan_key_live(song: &Song, tl: &Timeline, fill: bool) -> u64 {
    // the Fill key only changes clips with step conditions: without any,
    // holding it doesn't rebuild (and re-seek) the plan
    let fill = fill && tl.notes.iter().any(|d| d.has_conds());
    plan_key(song, tl) ^ if fill { 0xF111_F111_F111_F111 } else { 0 }
}

/// Cheap fingerprint of everything the [`Plan`] depends on (no allocation).
pub fn plan_key(song: &Song, tl: &Timeline) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    song.channels.hash(&mut h);
    tl.clips.len().hash(&mut h);
    for c in &tl.clips {
        (c.track, c.start, c.len, c.offset, c.loop_len, c.muted).hash(&mut h);
        match c.kind {
            ClipKind::Block { pattern, col } => (0u8, pattern, col).hash(&mut h),
            ClipKind::Audio { asset } => {
                (
                    2u8,
                    asset,
                    c.gain.to_bits(),
                    c.fade_in,
                    c.fade_out,
                    c.fade_shape,
                )
                    .hash(&mut h);
                song.bpm.to_bits().hash(&mut h);
                if let Some(a) = tl.audio.get(asset as usize) {
                    (a.rate, a.frames).hash(&mut h);
                    a.pcm.as_ref().map(|p| Arc::as_ptr(p) as usize).hash(&mut h);
                }
            }
            ClipKind::Notes { data } => {
                (1u8, data as usize, c.seed).hash(&mut h);
                if let Some(d) = tl.notes.get(data as usize) {
                    d.len.hash(&mut h);
                    hash_var(d, &mut h);
                    for n in &d.notes {
                        (
                            n.start,
                            n.len,
                            n.key,
                            n.vel,
                            n.instr,
                            n.chance,
                            n.tune.to_bits(),
                            n.cond,
                        )
                            .hash(&mut h);
                    }
                }
            }
        }
    }
    (tl.loop_range, tl.loop_on).hash(&mut h);
    for t in &tl.tracks {
        t.speed.hash(&mut h);
    }
    for p in &tl.tempo {
        (p.tick, p.bpm.to_bits(), p.ramp).hash(&mut h);
    }
    // launcher clips (only when there are some: other keys stay the same)
    for l in &tl.launcher {
        let c = &l.clip;
        (c.id, c.track, c.len, c.offset, c.loop_len, c.seed).hash(&mut h);
        if let Some(d) = c.notes().and_then(|k| tl.notes.get(k)) {
            hash_var(d, &mut h);
            for n in &d.notes {
                (
                    n.start,
                    n.len,
                    n.key,
                    n.vel,
                    n.instr,
                    n.chance,
                    n.tune.to_bits(),
                    n.cond,
                )
                    .hash(&mut h);
            }
        }
    }
    // markers decide which passes are fills (only matters with conditions)
    for m in &tl.markers {
        (m.tick, m.track).hash(&mut h);
    }
    h.finish()
}

/// the variations' takes and order (nothing without them, so other keys
/// stay the same)
fn hash_var(d: &NoteClipData, h: &mut impl Hasher) {
    let Some(v) = &d.var else { return };
    (0x7A55u16, v.order, v.seed, v.cur, v.takes.len()).hash(h);
    for t in &v.takes {
        t.len().hash(h);
        for n in t {
            (
                n.start,
                n.len,
                n.key,
                n.vel,
                n.instr,
                n.chance,
                n.tune.to_bits(),
                n.cond,
            )
                .hash(h);
        }
    }
}

/// Resolve the clips into per-track segments. Overlaps: the clip that
/// starts later wins (the same rule as [`consolidate`]); muted and piano-roll
/// clips play nothing yet.
pub fn build_plan(song: &Song, tl: &Timeline) -> Plan {
    build_plan_live(song, tl, false)
}

/// [`build_plan`], with `fill` = the Fill key is held (live playback only:
/// every pass counts as a fill; renders and exports always pass false).
pub fn build_plan_live(song: &Song, tl: &Timeline, fill: bool) -> Plan {
    let nch = song.channels.min(MAX_CHANNELS);
    let mut per: Vec<Vec<&Clip>> = vec![vec![]; nch];
    for c in &tl.clips {
        if c.len <= 0 || c.muted || c.track as usize >= nch || c.block().is_none() {
            continue;
        }
        per[c.track as usize].push(c);
    }
    let row_of = |t: Tick| (t.max(0) / ROW) as usize;
    let mut tracks = Vec::with_capacity(nch);
    for v in per.iter_mut() {
        v.sort_by_key(|c| (c.start, c.id));
        let mut cuts: Vec<usize> = v
            .iter()
            .flat_map(|c| [row_of(c.start), row_of(c.end())])
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        let mut segs: Vec<Seg> = vec![];
        let mut last: Option<u32> = None;
        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let at = rows_to_ticks(a);
            // the latest-starting clip covering row a
            let win = v
                .iter()
                .rev()
                .filter(|c| row_of(c.start) <= a)
                .find(|c| row_of(c.end()) > a);
            match win {
                Some(c) => {
                    let (pattern, col) = c.block().unwrap_or((0, 0));
                    if last == Some(c.id) {
                        if let Some(s) = segs.last_mut() {
                            s.end = b;
                            continue;
                        }
                    }
                    let lr = c.loop_len.map_or(0, |l| (l.max(0) / ROW) as usize);
                    segs.push(Seg {
                        start: a,
                        end: b,
                        pattern,
                        col,
                        src0: (c.source_at(at).max(0) / ROW) as usize,
                        loop_rows: lr,
                    });
                    last = Some(c.id);
                }
                None => last = None,
            }
        }
        tracks.push(segs);
    }
    let end = row_of(play_end(tl));
    let loop_rows = tl
        .loop_range
        .filter(|_| tl.loop_on)
        .map(|(a, b)| (row_of(a), row_of(b)))
        .filter(|(a, b)| b > a);
    Plan {
        key: plan_key_live(song, tl, fill),
        tracks,
        end,
        loop_rows,
        speeds: (0..nch)
            .map(|i| tl.tracks.get(i).map_or(0, |t| t.speed))
            .collect(),
        tempo_changes: tl.tempo.iter().any(|p| p.tick > 0),
        notes: note_spans_live(tl, nch, fill),
        audio: audio_spans(song, tl, nch),
        pcm: tl.audio.iter().map(|a| a.pcm.clone()).collect(),
        tempo: tl.tempo.clone(),
        bpm: song.bpm,
        launch: launch_plan(tl),
    }
}

/// Every unmuted audio clip whose file is loaded, per track, sorted.
pub fn audio_spans(song: &Song, tl: &Timeline, nch: usize) -> Vec<Vec<AudioSpan>> {
    let mut out: Vec<Vec<AudioSpan>> = vec![vec![]; nch];
    for c in &tl.clips {
        let Some(ai) = c.audio() else { continue };
        if c.len <= 0 || c.muted || c.track as usize >= nch {
            continue;
        }
        let Some(a) = tl.audio.get(ai).filter(|a| !a.missing()) else {
            continue;
        };
        let fpt = tl.frames_per_tick(c.start, song.bpm, a.rate);
        let src_ticks = tl.audio_ticks(ai, c.start, song.bpm);
        let loop_len = match c.loop_len.filter(|l| *l > 0) {
            // a loop of the whole file wraps exactly at its last frame
            Some(l) if l == src_ticks => a.frames as f64,
            Some(l) => l as f64 * fpt,
            None => 0.0,
        };
        out[c.track as usize].push(AudioSpan {
            start: c.start,
            end: c.end(),
            asset: ai as u32,
            off: c.offset as f64 * fpt,
            loop_len,
            rate: a.rate as f64,
            gain: if c.gain.is_finite() {
                c.gain.clamp(0.0, 16.0)
            } else {
                1.0
            },
            fade_in: c.fade_in.clamp(0, c.len),
            fade_out: c.fade_out.clamp(0, c.len),
            shape: c.fade_shape,
            secs0: tl.seconds_at(c.start, song.bpm),
        });
    }
    for v in &mut out {
        v.sort_by_key(|s| (s.start, s.asset));
    }
    out
}

/// Every note of the unmuted note clips, per track, in timeline ticks.
/// Looped clips repeat their notes each pass; notes end at the clip end
/// (and at the end of their loop pass). `chance` draws from
/// `ClipRng::new(seed, pass)`, so every play, seek and export is the same.
pub fn note_spans(tl: &Timeline, nch: usize) -> Vec<Vec<NoteSpan>> {
    note_spans_live(tl, nch, false)
}

/// [`note_spans`] with the live Fill key `fill` (see [`build_plan_live`])
pub fn note_spans_live(tl: &Timeline, nch: usize, fill: bool) -> Vec<Vec<NoteSpan>> {
    let mut out: Vec<Vec<NoteSpan>> = vec![vec![]; nch];
    for c in &tl.clips {
        if c.notes().is_none() || c.len <= 0 || c.muted || c.track as usize >= nch {
            continue;
        }
        clip_note_spans_live(tl, c, &mut out[c.track as usize], fill);
    }
    for v in &mut out {
        v.sort_by_key(|n| (n.start, n.key));
    }
    out
}

/// The notes note clip `c` plays, in timeline ticks, appended to `v`
/// (unsorted; see [`note_spans`]).
pub(crate) fn clip_note_spans(tl: &Timeline, c: &Clip, v: &mut Vec<NoteSpan>) {
    clip_note_spans_live(tl, c, v, false)
}

/// [`clip_note_spans`] with the live Fill key `fill`. Per pass k: the
/// chance draws come from `ClipRng::new(seed, k)` exactly as before (a
/// note's condition is tested after its draw, so conditions never move
/// another note's randomness).
pub(crate) fn clip_note_spans_live(tl: &Timeline, c: &Clip, v: &mut Vec<NoteSpan>, fill: bool) {
    let ClipKind::Notes { data } = c.kind else {
        return;
    };
    if c.len <= 0 {
        return;
    }
    let Some(d) = tl.notes.get(data as usize) else {
        return;
    };
    let push = |v: &mut Vec<NoteSpan>,
                n: &Note,
                u: Tick,
                cap: Tick,
                rng: &mut ClipRng,
                pass: u32,
                is_fill: bool| {
        if n.chance > 0 && n.chance < 100 && rng.next_f32() * 100.0 >= n.chance as f32 {
            return;
        }
        if !n.cond.plays(pass, is_fill) {
            return;
        }
        if u < 0 || u >= c.len {
            return;
        }
        let end = (u + n.len.max(1)).min(cap).min(c.len);
        if end <= u || n.key == 0 || n.key > 120 {
            return;
        }
        v.push(NoteSpan {
            start: c.start + u,
            end: c.start + end,
            key: n.key,
            vel: n.vel as f32 / 65535.0,
            instr: n.instr,
            tune: n.tune,
        });
    };
    let conds = d.has_conds();
    match c.loop_len.filter(|l| *l > 0) {
        Some(l) => {
            // pass k plays source [k*l, (k+1)*l) at clip time k*l - offset
            let off = c.offset.rem_euclid(l);
            let passes = ((c.len + off) + l - 1) / l;
            for k in 0..passes.min(100_000) {
                let mut rng = ClipRng::new(c.seed, k as u32);
                let base = k * l - off;
                let f = conds && rr::is_fill(tl, c, k, passes, base, l, fill);
                let notes = d.var_notes(c.seed, k as u32);
                for n in notes.iter().filter(|n| n.start >= 0 && n.start < l) {
                    push(v, n, base + n.start, base + l, &mut rng, k as u32, f);
                }
            }
        }
        None => {
            let mut rng = ClipRng::new(c.seed, 0);
            for n in d.var_notes(c.seed, 0) {
                push(v, n, n.start - c.offset, Tick::MAX, &mut rng, 0, fill);
            }
        }
    }
}

/// The launcher clips as the engine plays them when launched (Stage 4b):
/// each clip's notes from tick 0, in launcher order.
pub fn launch_plan(tl: &Timeline) -> Vec<LaunchNotes> {
    tl.launcher
        .iter()
        .filter(|l| l.clip.len > 0)
        .map(|l| {
            let (len, notes) = launch_cycle(tl, &l.clip);
            LaunchNotes {
                id: l.clip.id,
                track: l.clip.track as usize,
                len,
                notes,
            }
        })
        .collect()
}

/// passes a launched clip with variations is unrolled to (the engine
/// loops the whole cycle, so each pass still picks its own take)
pub const LAUNCH_PASSES: Tick = 16;

/// A launcher clip's loop as the engine plays it: (cycle length, notes).
/// Without variations this is the clip as before; with them, 16 passes
/// unrolled so every pass picks its take from (seed, pass).
pub fn launch_cycle(tl: &Timeline, c: &Clip) -> (Tick, Vec<NoteSpan>) {
    let vars = c
        .notes()
        .and_then(|k| tl.notes.get(k))
        .is_some_and(|d| d.var.is_some());
    if !vars || c.len <= 0 {
        return (c.len, launch_spans(tl, c));
    }
    let mut one = c.clone();
    one.loop_len = Some(c.loop_len.filter(|l| *l > 0).unwrap_or(c.len));
    one.len = c.len * LAUNCH_PASSES;
    (one.len, launch_spans(tl, &one))
}

/// The cell track `tr` plays at timeline row `row`, given the segment `s`
/// and the source limit (rows after a pattern break are silent).
pub fn plan_cell(song: &Song, s: &Seg, row: usize, limit: usize) -> Cell {
    let sr = s.src_row(row);
    if sr >= limit {
        return Cell::default();
    }
    song.patterns
        .get(s.pattern)
        .and_then(|p| p.cells.get(sr))
        .and_then(|r| r.get(s.col))
        .copied()
        .unwrap_or_default()
}

/// rows of block `pattern` that play (a `0F00` in any column ends it)
pub fn source_limit(song: &Song, pattern: usize) -> usize {
    song.patterns
        .get(pattern)
        .map_or(0, |p| effective_rows(p, song.channels.min(MAX_CHANNELS)))
}
