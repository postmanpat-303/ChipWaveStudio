//! Timeline mode (M2): switching a song between tracker and timeline mode,
//! and the clip edits of the Timeline view. Every function edits the song
//! in place; the app's song-wide undo records the result (one step per
//! gesture), so none of them keep history themselves.
use super::*;

/// Song position in rows (as the engine's `song_pos`, which counts full
/// block lengths) -> timeline row, through the derived order map.
fn row_of_song_pos(map: &OrderMap, pos: f32) -> f32 {
    let mut before = 0usize;
    for (k, &full) in map.full.iter().enumerate() {
        if (pos as f64) < (before + full) as f64 || k + 1 == map.full.len() {
            return (map.tick_of_song_pos(k, pos) / ROW as f64) as f32;
        }
        before += full;
    }
    pos
}

/// Switch a tracker-mode song to **timeline mode**, losslessly: the derived
/// timeline (one clip per order entry and used column, runs looped) becomes
/// the song, and automation positions move from block rows to timeline rows
/// (they differ only after a `0F00` pattern break). The order list and the
/// blocks stay as they are. Does nothing in timeline mode.
pub fn enter_timeline_mode(song: &mut Song) {
    if song.timeline.is_some() {
        return;
    }
    let (mut tl, map) = from_order(song);
    // the song's BPM is the start tempo; the map holds later changes only
    tl.tempo.retain(|p| p.tick > 0);
    if map.lens != map.full {
        for lane in &mut song.automation {
            for p in &mut lane.points {
                p.pos = row_of_song_pos(&map, p.pos);
            }
        }
    }
    song.timeline = Some(Box::new(tl));
}

/// Back to tracker mode: the timeline is consolidated into blocks and an
/// order list ([`consolidate`]); the result plays the same. Errors (piano-
/// roll clips, too many blocks) leave the song unchanged.
pub fn leave_timeline_mode(song: &mut Song) -> Result<(), String> {
    let Some(tl) = song.timeline.as_deref() else {
        return Ok(());
    };
    let (patterns, order) = consolidate(song, tl)?;
    song.patterns = patterns;
    song.order = order;
    song.timeline = None;
    Ok(())
}

/// A tracker-mode copy of a timeline-mode song (File > Export tracker-
/// compatible copy): opens in ChipWave 0.2.4.0 and older and plays the same.
pub fn tracker_copy(song: &Song) -> Result<Song, String> {
    let mut s = song.clone();
    leave_timeline_mode(&mut s)?;
    Ok(s)
}

/// round `t` to the nearest multiple of `grid` (> 0)
pub fn snap(t: Tick, grid: Tick) -> Tick {
    if grid <= 1 {
        return t;
    }
    (t + grid / 2).div_euclid(grid) * grid
}

/// rows that block `pattern` plays (its source length for loops)
fn src_len(song: &Song, pattern: usize) -> Tick {
    rows_to_ticks(source_limit(song, pattern).max(1))
}

fn find(tl: &Timeline, id: u32) -> Option<usize> {
    tl.clips.iter().position(|c| c.id == id)
}

/// Move clips by `dt` ticks and `dtrack` tracks (kept inside the song).
pub fn move_clips(tl: &mut Timeline, ids: &[u32], dt: Tick, dtrack: i32, tracks: usize) {
    let sel: Vec<&Clip> = tl.clips.iter().filter(|c| ids.contains(&c.id)).collect();
    let min_start = sel.iter().map(|c| c.start).min().unwrap_or(0);
    let min_tr = sel.iter().map(|c| c.track as i32).min().unwrap_or(0);
    let max_tr = sel.iter().map(|c| c.track as i32).max().unwrap_or(0);
    let dt = dt.max(-min_start);
    let dtrack = dtrack.clamp(-min_tr, tracks as i32 - 1 - max_tr);
    for c in tl.clips.iter_mut().filter(|c| ids.contains(&c.id)) {
        c.start += dt;
        c.track = (c.track as i32 + dtrack) as u16;
    }
    tl.sort();
}

/// Drag the right edge: the clip ends at `end` (at least one row long).
/// Past the end of its block the source loops (with notches).
pub fn set_end(song: &Song, tl: &mut Timeline, id: u32, end: Tick) {
    let Some(i) = find(tl, id) else { return };
    let c = &mut tl.clips[i];
    let len = (end - c.start).max(ROW);
    if let Some((pattern, _)) = c.block() {
        let sl = src_len(song, pattern);
        if c.loop_len.is_none() && c.offset + len > sl {
            c.loop_len = Some(sl);
        }
    }
    c.len = len;
}

/// Drag the left edge: the clip starts at `start` (its content stays where
/// it is, so this trims or reveals the beginning). Without a loop it can't
/// reveal rows before the block start.
pub fn set_start(tl: &mut Timeline, id: u32, start: Tick) {
    let Some(i) = find(tl, id) else { return };
    let c = &mut tl.clips[i];
    let end = c.end();
    let lo = if c.loop_len.is_some() {
        Tick::MIN / 4
    } else {
        c.start - c.offset
    };
    let start = start.clamp(lo.max(0), end - ROW);
    c.offset += start - c.start;
    if let Some(l) = c.loop_len.filter(|l| *l > 0) {
        c.offset = c.offset.rem_euclid(l);
    }
    c.len = end - start;
    c.start = start;
    tl.sort();
}

/// Split clip `id` at `at` (inside it). Returns the new right-hand clip id.
pub fn split(tl: &mut Timeline, id: u32, at: Tick) -> Option<u32> {
    let i = find(tl, id)?;
    let c = tl.clips[i].clone();
    if at <= c.start || at >= c.end() {
        return None;
    }
    let nid = tl.new_id();
    let mut right = c.clone();
    right.id = nid;
    right.seed = seed_for(nid);
    right.start = at;
    right.len = c.end() - at;
    right.offset = c.offset + (at - c.start);
    if let Some(l) = c.loop_len.filter(|l| *l > 0) {
        right.offset = right.offset.rem_euclid(l);
    }
    tl.clips[i].len = at - c.start;
    tl.clips.push(right);
    tl.sort();
    Some(nid)
}

/// Copies of `ids` placed right after them (linked: same block). Returns the
/// new ids.
pub fn duplicate(tl: &mut Timeline, ids: &[u32]) -> Vec<u32> {
    let sel: Vec<Clip> = tl
        .clips
        .iter()
        .filter(|c| ids.contains(&c.id))
        .cloned()
        .collect();
    let (Some(a), Some(b)) = (
        sel.iter().map(|c| c.start).min(),
        sel.iter().map(|c| c.end()).max(),
    ) else {
        return vec![];
    };
    copies_at(tl, &sel, b - a)
}

/// copies of `sel` shifted by `dt` ticks and `dtrack` tracks (Alt-drag)
pub fn copies_at(tl: &mut Timeline, sel: &[Clip], dt: Tick) -> Vec<u32> {
    let mut out = vec![];
    for c in sel {
        let nid = tl.new_id();
        let mut n = c.clone();
        n.id = nid;
        n.seed = seed_for(nid);
        n.start = (c.start + dt).max(0);
        tl.clips.push(n);
        out.push(nid);
    }
    tl.sort();
    out
}

pub fn delete(tl: &mut Timeline, ids: &[u32]) {
    tl.clips.retain(|c| !ids.contains(&c.id));
}

/// Mute or unmute the clips (all muted -> unmute, else mute).
pub fn toggle_mute(tl: &mut Timeline, ids: &[u32]) {
    let all = tl
        .clips
        .iter()
        .filter(|c| ids.contains(&c.id))
        .all(|c| c.muted);
    for c in tl.clips.iter_mut().filter(|c| ids.contains(&c.id)) {
        c.muted = !all;
    }
}

/// Make unique: clip `id` gets its own copy of its block, so editing it no
/// longer changes the other clips. Returns the new block number.
pub fn make_unique(song: &mut Song, id: u32) -> Option<usize> {
    let tl = song.timeline.as_deref()?;
    let c = &tl.clips[find(tl, id)?];
    let (pattern, _) = c.block()?;
    if song.patterns.len() >= crate::fileio::MAX_BLOCKS {
        return None;
    }
    let mut p = song.patterns.get(pattern)?.clone();
    p.name = format!("{} copy", p.name);
    p.derived = None;
    song.patterns.push(p);
    let nb = song.patterns.len() - 1;
    let tl = song.timeline.as_deref_mut()?;
    let i = find(tl, id)?;
    if let ClipKind::Block { pattern, .. } = &mut tl.clips[i].kind {
        *pattern = nb;
    }
    Some(nb)
}

/// Make unique for a note clip: it gets its own copy of the note data.
/// Returns the new data index.
pub fn make_unique_notes(tl: &mut Timeline, id: u32) -> Option<usize> {
    let i = find(tl, id)?;
    let ClipKind::Notes { data } = tl.clips[i].kind else {
        return None;
    };
    let mut d = (**tl.notes.get(data as usize)?).clone();
    d.name = format!("{} copy", d.name);
    tl.notes.push(Shared::new(d));
    let nd = tl.notes.len() - 1;
    tl.clips[i].kind = ClipKind::Notes { data: nd as u32 };
    Some(nd)
}

/// A new empty note clip (piano roll, 4 bars) on `track` at `start`.
/// Returns its id.
pub fn new_note_clip(song: &mut Song, track: usize, start: Tick) -> Option<u32> {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    if track >= nch {
        return None;
    }
    let tl = song.timeline.as_deref_mut()?;
    let len = 16 * PPQ;
    let n = tl.notes.len();
    tl.notes.push(Shared::new(NoteClipData {
        name: format!("Notes {}", n + 1),
        len,
        notes: vec![],
    }));
    let id = tl.new_id();
    tl.clips.push(Clip {
        id,
        track: track as u16,
        start: start.max(0),
        len,
        kind: ClipKind::Notes { data: n as u32 },
        seed: seed_for(id),
        ..Default::default()
    });
    tl.sort();
    Some(id)
}

/// Glue: the selected clips on one track become one clip of a new block
/// (written into the block's column for that track) that plays exactly what
/// they played; gaps become empty rows. Returns the new clip id.
pub fn glue(song: &mut Song, ids: &[u32]) -> Result<u32, String> {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    let tl = song.timeline.as_deref().ok_or("not in timeline mode")?;
    let first = tl
        .clips
        .iter()
        .find(|c| ids.contains(&c.id))
        .ok_or("select clips to glue")?;
    let track = first.track;
    let sel: Vec<Clip> = tl
        .clips
        .iter()
        .filter(|c| ids.contains(&c.id) && c.track == track && c.block().is_some())
        .cloned()
        .collect();
    let a = sel.iter().map(|c| c.start).min().unwrap_or(0);
    let b = sel.iter().map(|c| c.end()).max().unwrap_or(0);
    let rows = ((b - a + ROW - 1) / ROW) as usize;
    if rows > Pattern::MAX_EDIT_ROWS {
        return Err(format!(
            "glue makes a block of at most {} rows (this is {rows})",
            Pattern::MAX_EDIT_ROWS
        ));
    }
    if song.patterns.len() >= crate::fileio::MAX_BLOCKS {
        return Err("too many blocks".into());
    }
    // what the selection plays, row by row (same rules as playback)
    let mut sub = Timeline {
        tracks: tl.tracks.clone(),
        clips: sel
            .iter()
            .map(|c| Clip {
                track: track.min(nch as u16 - 1),
                muted: c.muted,
                ..c.clone()
            })
            .collect(),
        ..Default::default()
    };
    sub.sort();
    let plan = build_plan(song, &sub);
    let col = (track as usize).min(nch - 1);
    let mut p = Pattern::new(
        &format!("Glue {:02}", song.patterns.len()),
        rows.max(1),
        nch,
    );
    if let Some(segs) = plan.tracks.get(col) {
        let r0 = (a / ROW) as usize;
        for s in segs {
            let limit = source_limit(song, s.pattern);
            for row in s.start.max(r0)..s.end.min(r0 + rows) {
                let mut cell = plan_cell(song, s, row, limit);
                if cell.fx == 0x0F && cell.fxp == 0 {
                    cell.fx = 0; // a glued block never breaks
                }
                p.cells[row - r0][col] = cell;
            }
        }
    }
    song.patterns.push(p);
    let nb = song.patterns.len() - 1;
    let tl = song.timeline.as_deref_mut().ok_or("not in timeline mode")?;
    tl.clips.retain(|c| !sel.iter().any(|s| s.id == c.id));
    let id = tl.new_id();
    tl.clips.push(Clip {
        id,
        track,
        start: a,
        len: rows_to_ticks(rows.max(1)),
        kind: ClipKind::Block {
            pattern: nb,
            col: col as u8,
        },
        seed: seed_for(id),
        ..Default::default()
    });
    tl.sort();
    Ok(id)
}

/// A new empty block (64 rows) placed as a clip on `track` at `start`
/// (double-click on empty space). Returns (block, clip id).
pub fn new_clip(song: &mut Song, track: usize, start: Tick) -> Option<(usize, u32)> {
    let nch = song.channels.clamp(1, MAX_CHANNELS);
    if song.patterns.len() >= crate::fileio::MAX_BLOCKS || track >= nch {
        return None;
    }
    let n = song.patterns.len();
    song.patterns
        .push(Pattern::new(&format!("Block {n:02}"), ROWS, nch));
    let tl = song.timeline.as_deref_mut()?;
    let id = tl.new_id();
    tl.clips.push(Clip {
        id,
        track: track as u16,
        start: start.max(0),
        len: rows_to_ticks(ROWS),
        kind: ClipKind::Block {
            pattern: n,
            col: track as u8,
        },
        seed: seed_for(id),
        ..Default::default()
    });
    tl.sort();
    Some((n, id))
}

/// Add a marker at `tick` (named "Marker N"); returns its index.
pub fn add_marker(tl: &mut Timeline, tick: Tick) -> usize {
    let name = format!("Marker {}", tl.markers.len() + 1);
    tl.markers.push(Marker {
        name,
        tick: tick.max(0),
        ..Default::default()
    });
    tl.markers.sort_by_key(|m| m.tick);
    tl.markers
        .iter()
        .position(|m| m.tick == tick.max(0))
        .unwrap_or(0)
}

/// Set the loop region (sorted, at least one row) and switch it on.
pub fn set_loop(tl: &mut Timeline, a: Tick, b: Tick) {
    let (a, b) = (a.min(b).max(0), a.max(b).max(0));
    tl.loop_range = Some((a, b.max(a + ROW)));
    tl.loop_on = true;
}
