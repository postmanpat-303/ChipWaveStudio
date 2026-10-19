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
}

impl Plan {
    /// any track has note-clip notes
    pub fn has_notes(&self) -> bool {
        self.notes.iter().any(|v| !v.is_empty())
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

/// Cheap fingerprint of everything the [`Plan`] depends on (no allocation).
pub fn plan_key(song: &Song, tl: &Timeline) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    song.channels.hash(&mut h);
    tl.clips.len().hash(&mut h);
    for c in &tl.clips {
        (c.track, c.start, c.len, c.offset, c.loop_len, c.muted).hash(&mut h);
        match c.kind {
            ClipKind::Block { pattern, col } => (0u8, pattern, col).hash(&mut h),
            ClipKind::Notes { data } => {
                (1u8, data as usize, c.seed).hash(&mut h);
                if let Some(d) = tl.notes.get(data as usize) {
                    d.len.hash(&mut h);
                    for n in &d.notes {
                        (
                            n.start,
                            n.len,
                            n.key,
                            n.vel,
                            n.instr,
                            n.chance,
                            n.tune.to_bits(),
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
    h.finish()
}

/// Resolve the clips into per-track segments. Overlaps: the clip that
/// starts later wins (the same rule as [`consolidate`]); muted and piano-roll
/// clips play nothing yet.
pub fn build_plan(song: &Song, tl: &Timeline) -> Plan {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
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
        key: plan_key(song, tl),
        tracks,
        end,
        loop_rows,
        speeds: (0..nch)
            .map(|i| tl.tracks.get(i).map_or(0, |t| t.speed))
            .collect(),
        tempo_changes: tl.tempo.iter().any(|p| p.tick > 0),
        notes: note_spans(tl, nch),
    }
}

/// Every note of the unmuted note clips, per track, in timeline ticks.
/// Looped clips repeat their notes each pass; notes end at the clip end
/// (and at the end of their loop pass). `chance` draws from
/// `ClipRng::new(seed, pass)`, so every play, seek and export is the same.
pub fn note_spans(tl: &Timeline, nch: usize) -> Vec<Vec<NoteSpan>> {
    let mut out: Vec<Vec<NoteSpan>> = vec![vec![]; nch];
    for c in &tl.clips {
        let ClipKind::Notes { data } = c.kind else {
            continue;
        };
        if c.len <= 0 || c.muted || c.track as usize >= nch {
            continue;
        }
        let Some(d) = tl.notes.get(data as usize) else {
            continue;
        };
        let v = &mut out[c.track as usize];
        let push = |v: &mut Vec<NoteSpan>, n: &Note, u: Tick, cap: Tick, rng: &mut ClipRng| {
            if n.chance > 0 && n.chance < 100 && rng.next_f32() * 100.0 >= n.chance as f32 {
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
        match c.loop_len.filter(|l| *l > 0) {
            Some(l) => {
                // pass k plays source [k*l, (k+1)*l) at clip time k*l - offset
                let off = c.offset.rem_euclid(l);
                let passes = ((c.len + off) + l - 1) / l;
                for k in 0..passes.min(100_000) {
                    let mut rng = ClipRng::new(c.seed, k as u32);
                    let base = k * l - off;
                    for n in d.notes.iter().filter(|n| n.start >= 0 && n.start < l) {
                        push(v, n, base + n.start, base + l, &mut rng);
                    }
                }
            }
            None => {
                let mut rng = ClipRng::new(c.seed, 0);
                for n in &d.notes {
                    push(v, n, n.start - c.offset, Tick::MAX, &mut rng);
                }
            }
        }
    }
    for v in &mut out {
        v.sort_by_key(|n| (n.start, n.key));
    }
    out
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
    song.patterns.get(pattern).map_or(0, |p| {
        effective_rows(p, song.channels.clamp(1, MAX_CHANNELS))
    })
}
