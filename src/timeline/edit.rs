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
    // design A: song lanes become track / Song-Master lanes
    let _ = super::migrate_song_lanes(song);
}

/// Back to tracker mode: the timeline is consolidated into blocks and an
/// order list ([`consolidate`]); the result plays the same. Errors (piano-
/// roll clips, too many blocks) leave the song unchanged.
pub fn leave_timeline_mode(song: &mut Song) -> Result<(), String> {
    if song.timeline.is_none() {
        return Ok(());
    }
    // a clean new song has no tracks: tracker mode needs one channel
    if song.channels == 0 {
        crate::chanops::insert_channel(song, 0, "Ch 1");
    }
    let Some(tl) = song.timeline.as_deref() else {
        return Ok(());
    };
    let (patterns, order) = consolidate(song, tl)?;
    song.patterns = patterns;
    song.order = order;
    // track and clip lanes become song lanes (sampled per row)
    if let Some(tl) = song.timeline.take() {
        bake_into_song(song, &tl);
    }
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
    let audio_len = tl.clips[i]
        .audio()
        .map(|a| tl.audio_ticks(a, tl.clips[i].start, song.bpm));
    let c = &mut tl.clips[i];
    let len = (end - c.start).max(ROW);
    let sl = match (c.block(), audio_len) {
        (Some((pattern, _)), _) => Some(src_len(song, pattern)),
        (None, Some(t)) => Some(t),
        _ => None,
    };
    if let Some(sl) = sl {
        if c.loop_len.is_none() && c.offset + len > sl {
            c.loop_len = Some(sl);
        }
    }
    c.len = len;
    c.fade_in = c.fade_in.min(len);
    c.fade_out = c.fade_out.min(len - c.fade_in);
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
    c.fade_in = c.fade_in.min(c.len);
    c.fade_out = c.fade_out.min(c.len - c.fade_in);
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
    // fades stay at the outer ends
    right.fade_in = 0;
    right.fade_out = c.fade_out.min(right.len);
    tl.clips[i].len = at - c.start;
    tl.clips[i].fade_out = 0;
    tl.clips[i].fade_in = c.fade_in.min(at - c.start);
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
    let data = tl.any_clip(id)?.notes()?;
    let mut d = (**tl.notes.get(data)?).clone();
    d.name = format!("{} copy", d.name);
    tl.notes.push(Shared::new(d));
    let nd = tl.notes.len() - 1;
    let kind = ClipKind::Notes { data: nd as u32 };
    match find(tl, id) {
        Some(i) => tl.clips[i].kind = kind,
        // a launcher clip
        None => {
            if let Some(l) = tl.launcher.iter_mut().find(|l| l.clip.id == id) {
                l.clip.kind = kind;
            }
        }
    }
    Some(nd)
}

/// The instrument track `tr` is assigned (`Track::instr`) when it is a
/// real instrument of the song, else 0 (none, or a deleted one).
pub fn assigned_instr(song: &Song, tr: usize) -> u8 {
    let k = song.timeline.as_deref().map_or(0, |t| t.track_instr(tr));
    if k as usize <= song.instruments.len() {
        k
    } else {
        0
    }
}

/// How many of note clip `id`'s notes have no instrument of their own
/// (instrument 0: they play whatever the channel last played).
pub fn unset_instr_notes(tl: &Timeline, id: u32) -> usize {
    let Some(i) = find(tl, id) else {
        return 0;
    };
    let ClipKind::Notes { data } = tl.clips[i].kind else {
        return 0;
    };
    tl.notes
        .get(data as usize)
        .map_or(0, |d| d.notes.iter().filter(|n| n.instr == 0).count())
}

/// "Use the track's instrument" on note clip `id`: its notes without an
/// instrument (0) get the track's assigned instrument; notes that already
/// have one are left alone. Linked copies on the same track share the
/// notes and change with it; when a copy sits on another track (where
/// those notes may play another instrument) this track's copies get
/// their own notes first, so no other track's clip changes. Returns the notes stamped
/// (0, and nothing changes, when the track has no assigned instrument or
/// the clip has no such notes).
pub fn use_track_instr(song: &mut Song, id: u32) -> usize {
    let Some(tl) = song.timeline.as_deref() else {
        return 0;
    };
    let Some(i) = find(tl, id) else {
        return 0;
    };
    let ClipKind::Notes { data } = tl.clips[i].kind else {
        return 0;
    };
    let tr = tl.clips[i].track;
    let slot = assigned_instr(song, tr as usize);
    if slot == 0 || unset_instr_notes(tl, id) == 0 {
        return 0;
    }
    let shared_elsewhere = tl
        .clips
        .iter()
        .any(|c| c.track != tr && matches!(c.kind, ClipKind::Notes { data: d } if d == data));
    let tl = song.timeline.as_deref_mut().unwrap();
    let data = if shared_elsewhere {
        // this track's copies get their own notes together
        let Some(d) = tl.notes.get(data as usize).map(|d| (**d).clone()) else {
            return 0;
        };
        tl.notes.push(Shared::new(d));
        let nd = tl.notes.len() - 1;
        for c in tl.clips.iter_mut().filter(|c| c.track == tr) {
            if matches!(c.kind, ClipKind::Notes { data: d } if d == data) {
                c.kind = ClipKind::Notes { data: nd as u32 };
            }
        }
        nd
    } else {
        data as usize
    };
    let mut n = 0;
    if let Some(d) = tl.notes.get_mut(data) {
        for note in d.make_mut().notes.iter_mut().filter(|x| x.instr == 0) {
            note.instr = slot;
            n += 1;
        }
    }
    n
}

/// A Tracker clip column as a note clip, so it can hold chords (the piano
/// roll does this when a new note would overlap another one): every clip
/// that plays column `col` of block `pattern` becomes a note clip on one
/// new note data, so linked copies stay linked. The notes are the column's
/// as the Tracker plays them, with their instruments, volumes and effect
/// commands. None, and nothing changes, when the column has a volume or an
/// effect on a row without a note (a note clip has nowhere to keep it).
/// Returns the new data index.
pub fn block_col_to_notes(song: &mut Song, pattern: usize, col: usize) -> Option<usize> {
    let p = song.patterns.get(pattern)?;
    let rows = p.rows.min(p.cells.len());
    let mono = roll::col_notes(p, col);
    for r in 0..rows {
        let c = p.cells[r].get(col).copied().unwrap_or_default();
        let starts = mono.iter().any(|n| n.row == r);
        if !starts && (c.fx != 0 || c.fxp != 0 || c.vol != 0) {
            return None;
        }
    }
    let notes = mono
        .iter()
        .map(|n| {
            let c = p.cells[n.row].get(col).copied().unwrap_or_default();
            Note {
                start: rows_to_ticks(n.row),
                len: rows_to_ticks(n.len.max(1)),
                key: n.key,
                vel: if n.vol == 0 {
                    0xFFFF
                } else {
                    roll::vel_u16(roll::vol_to_vel(n.vol))
                },
                instr: n.instr,
                fx: (c.fx != 0 || c.fxp != 0).then_some((c.fx, c.fxp)),
                ..Default::default()
            }
        })
        .collect();
    let data = NoteClipData {
        name: p.name.clone(),
        len: rows_to_ticks(rows.max(1)),
        notes,
        chords: vec![],
        drum: None,
        var: None,
    };
    let tl = song.timeline.as_deref_mut()?;
    let n = tl.notes.len();
    tl.notes.push(Shared::new(data));
    for c in &mut tl.clips {
        if matches!(c.kind, ClipKind::Block { pattern: b, col: k } if b == pattern && k as usize == col)
        {
            c.kind = ClipKind::Notes { data: n as u32 };
        }
    }
    Some(n)
}

/// A new empty note clip (piano roll, 4 bars) on `track` at `start`.
/// Returns its id.
pub fn new_note_clip(song: &mut Song, track: usize, start: Tick) -> Option<u32> {
    let nch = song.channels.min(MAX_CHANNELS);
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
        chords: vec![],
        drum: None,
        var: None,
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

/// Add an audio file to the pool; a loaded file with the same path is
/// shared instead. Returns its index.
pub fn add_audio_asset(tl: &mut Timeline, a: AudioAsset) -> usize {
    if !a.path.is_empty() {
        if let Some(k) = tl
            .audio
            .iter()
            .position(|x| x.path == a.path && !x.missing() && x.frames == a.frames)
        {
            return k;
        }
    }
    tl.audio.push(a);
    tl.audio.len() - 1
}

/// A new audio clip of pooled file `asset` on `track` at `start`, as long
/// as the file. Returns its id.
pub fn new_audio_clip(song: &mut Song, track: usize, start: Tick, asset: usize) -> Option<u32> {
    let nch = song.channels.min(MAX_CHANNELS);
    let bpm = song.bpm;
    let tl = song.timeline.as_deref_mut()?;
    if track >= nch || asset >= tl.audio.len() {
        return None;
    }
    let start = start.max(0);
    let len = tl.audio_ticks(asset, start, bpm);
    let id = tl.new_id();
    let name = tl.audio[asset].name.clone();
    tl.clips.push(Clip {
        id,
        track: track as u16,
        start,
        len,
        kind: ClipKind::Audio {
            asset: asset as u32,
        },
        name: Some(name),
        seed: seed_for(id),
        ..Default::default()
    });
    if let Some(t) = tl.tracks.get_mut(track) {
        if !tl
            .clips
            .iter()
            .any(|c| c.track as usize == track && c.audio().is_none())
        {
            t.kind = TrackKind::Audio;
        }
    }
    tl.sort();
    Some(id)
}

/// Fades of clip `id` in ticks (kept inside the clip, not overlapping).
pub fn set_fades(tl: &mut Timeline, id: u32, fade_in: Tick, fade_out: Tick) {
    let Some(i) = find(tl, id) else { return };
    let c = &mut tl.clips[i];
    c.fade_in = fade_in.clamp(0, c.len);
    c.fade_out = fade_out.clamp(0, c.len - c.fade_in);
}

/// Glue: the selected clips on one track become one clip of a new block
/// (written into the block's column for that track) that plays exactly what
/// they played; gaps become empty rows. Returns the new clip id.
pub fn glue(song: &mut Song, ids: &[u32]) -> Result<u32, String> {
    let nch = song.channels.min(MAX_CHANNELS);
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
    // merge clip automation onto the glued clip (design A §3.5): remap
    // each source clip's lane points into the glued span (rows from glue start)
    let mut glued_auto: Vec<crate::automation::Lane> = vec![];
    for c in &sel {
        let shift = ((c.start - a) as f32) / ROW as f32;
        for lane in &c.auto {
            let mut pts: Vec<crate::automation::Point> = lane
                .points
                .iter()
                .map(|p| {
                    let mut q = *p;
                    q.pos = p.pos + shift;
                    q
                })
                .collect();
            if let Some(dst) = glued_auto.iter_mut().find(|l| l.target == lane.target) {
                dst.points.append(&mut pts);
                dst.sort();
                // Absolute wins if either side is Absolute or modes differ
                if dst.mode != lane.mode
                    || dst.mode == crate::automation::ClipMode::Absolute
                    || lane.mode == crate::automation::ClipMode::Absolute
                {
                    dst.mode = crate::automation::ClipMode::Absolute;
                }
            } else {
                let mut l = lane.clone();
                l.points = pts;
                glued_auto.push(l);
            }
        }
    }
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
        auto: glued_auto,
        ..Default::default()
    });
    tl.sort();
    Ok(id)
}

/// A new empty block (64 rows) placed as a clip on `track` at `start`
/// (double-click on empty space). Returns (block, clip id).
pub fn new_clip(song: &mut Song, track: usize, start: Tick) -> Option<(usize, u32)> {
    let nch = song.channels.min(MAX_CHANNELS);
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

/// A Tracker clip of `block`'s column `col` on `track` at `start`, as long
/// as the block (a slot dragged from the Clip Arranger). Returns its id.
pub fn place_block(
    song: &mut Song,
    track: usize,
    start: Tick,
    block: usize,
    col: usize,
) -> Option<u32> {
    let nch = song.channels.min(MAX_CHANNELS);
    let rows = song.patterns.get(block)?.rows.max(1);
    if track >= nch || col >= nch {
        return None;
    }
    let tl = song.timeline.as_deref_mut()?;
    let id = tl.new_id();
    tl.clips.push(Clip {
        id,
        track: track as u16,
        start: start.max(0),
        len: rows_to_ticks(rows),
        kind: ClipKind::Block {
            pattern: block,
            col: col as u8,
        },
        seed: seed_for(id),
        ..Default::default()
    });
    tl.sort();
    Some(id)
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

// ------------------------------------------------------------ placing clips
// (docs/unified/PLAN.md Stage 4a: one drop system, nothing overwritten
// silently)

/// Ids of the clips on `track` that overlap `[a, b)`, except `skip`.
pub fn overlaps(tl: &Timeline, track: usize, a: Tick, b: Tick, skip: &[u32]) -> Vec<u32> {
    tl.clips
        .iter()
        .filter(|c| c.track as usize == track && !skip.contains(&c.id))
        .filter(|c| c.len > 0 && c.start < b && c.end() > a)
        .map(|c| c.id)
        .collect()
}

/// the clips (not in `ids`) that the clips `ids` overlap on their tracks
pub fn covered(tl: &Timeline, ids: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = vec![];
    for c in tl.clips.iter().filter(|c| ids.contains(&c.id)) {
        for o in overlaps(tl, c.track as usize, c.start, c.end(), ids) {
            if !out.contains(&o) {
                out.push(o);
            }
        }
    }
    out
}

/// Clear `[a, b)` on `track`: clips inside it go, clips across its edges
/// are split there and only their part inside goes (what "Replace" does
/// before a clip is placed there). Returns how many clips were touched.
pub fn clear_range(tl: &mut Timeline, track: usize, a: Tick, b: Tick, skip: &[u32]) -> usize {
    let ids = overlaps(tl, track, a, b, skip);
    let mut gone = vec![];
    for id in &ids {
        let mut id = *id;
        let Some(i) = find(tl, id) else { continue };
        let (s, e) = (tl.clips[i].start, tl.clips[i].end());
        if s < a {
            // keep the left part; the rest is a new clip
            match split(tl, id, a) {
                Some(r) => id = r,
                None => continue,
            }
        }
        if e > b {
            // the part after `b` stays
            let _ = split(tl, id, b);
        }
        gone.push(id);
    }
    delete(tl, &gone);
    ids.len()
}

/// The first start at or after `start` (on the `grid`) where a clip of
/// `len` fits on `track` without overlapping any clip ("Place after it").
pub fn free_start(tl: &Timeline, track: usize, start: Tick, len: Tick, grid: Tick) -> Tick {
    let grid = grid.max(1);
    let mut at = start.max(0);
    // each pass moves past one overlapping clip, so this ends
    for _ in 0..=tl.clips.len() {
        let hit = tl
            .clips
            .iter()
            .filter(|c| c.track as usize == track && c.len > 0)
            .filter(|c| c.start < at + len.max(1) && c.end() > at)
            .map(|c| c.end())
            .max();
        match hit {
            Some(end) => at = (end + grid - 1) / grid * grid,
            None => break,
        }
    }
    at
}

/// A copy of clip `src` on `track` at `start`: its content (linked: the
/// same notes, block or audio file), length, trim, loop, name, colour,
/// gain, fades and clip automation. `unique`: the copy gets its own notes
/// or block (an independent copy). Returns the new id.
pub fn place_copy(
    song: &mut Song,
    src: u32,
    track: usize,
    start: Tick,
    unique: bool,
) -> Option<u32> {
    let nch = song.channels.min(MAX_CHANNELS);
    if track >= nch {
        return None;
    }
    let tl = song.timeline.as_deref_mut()?;
    // a timeline clip, or a launcher clip placed in the arrangement
    let c = tl.any_clip(src)?.clone();
    let nid = tl.new_id();
    let mut n = c;
    n.id = nid;
    n.seed = seed_for(nid);
    n.track = track as u16;
    n.start = start.max(0);
    tl.clips.push(n);
    tl.sort();
    if unique && make_unique_notes(tl, nid).is_none() {
        // a Tracker clip: its own block (an audio clip keeps its file)
        let _ = make_unique(song, nid);
    }
    Some(nid)
}

/// The instrument slots (1-based) whose Arp plays note clip `id`'s notes
/// (a note's own instrument, else the track's).
pub fn arp_slots(song: &Song, id: u32) -> Vec<u8> {
    let Some(tl) = song.timeline.as_deref() else {
        return vec![];
    };
    let Some(i) = find(tl, id) else {
        return vec![];
    };
    let ClipKind::Notes { data } = tl.clips[i].kind else {
        return vec![];
    };
    let own = assigned_instr(song, tl.clips[i].track as usize);
    let mut out: Vec<u8> = vec![];
    for n in tl.notes.get(data as usize).map_or(&[][..], |d| &d.notes[..]) {
        let k = if n.instr > 0 { n.instr } else { own };
        let has = song
            .instruments
            .get((k as usize).wrapping_sub(1))
            .is_some_and(|i| i.arp.is_some());
        if has && !out.contains(&k) {
            out.push(k);
        }
    }
    out
}

/// "Bake Arp to notes": every note clip that plays through instrument
/// `slot`'s Arp gets the notes the Arp plays instead of its held notes,
/// then that Arp is switched off, so the song sounds the same and each note
/// can be edited. Linked copies stay linked. Returns the Arp taken off
/// (for the UI to keep) and (clips, notes) baked.
pub fn bake_arp(song: &mut Song, slot: u8) -> Option<(crate::arp::Arp, usize, usize)> {
    let inst = song.instruments.get((slot as usize).wrapping_sub(1))?;
    let a = inst.arp.clone()?;
    let kit = crate::drumkit::kit_of(inst).is_some();
    let tl = song.timeline.as_deref()?;
    // each note data once, with the song tick its start sits on
    let mut todo: Vec<(usize, f64, u8)> = vec![];
    let mut clips = 0;
    for c in &tl.clips {
        let ClipKind::Notes { data } = c.kind else {
            continue;
        };
        let own = assigned_instr(song, c.track as usize);
        let Some(d) = tl.notes.get(data as usize) else {
            continue;
        };
        if !d
            .notes
            .iter()
            .any(|n| (if n.instr > 0 { n.instr } else { own }) == slot)
        {
            continue;
        }
        clips += 1;
        if !todo.iter().any(|t| t.0 == data as usize) {
            todo.push((data as usize, (c.start - c.offset) as f64, own));
        }
    }
    let tl = song.timeline.as_deref_mut()?;
    let mut notes = 0;
    for (data, origin, own) in todo {
        let d = tl.notes[data].make_mut();
        let plays = |n: &Note| (if n.instr > 0 { n.instr } else { own }) == slot;
        let held: Vec<(f64, f64, u8, f32)> = d
            .notes
            .iter()
            .filter(|n| plays(n))
            .map(|n| {
                (
                    n.start as f64,
                    (n.start + n.len.max(1)) as f64,
                    n.key,
                    n.vel as f32 / 65535.0,
                )
            })
            .collect();
        let instr = d
            .notes
            .iter()
            .find(|n| plays(n))
            .map_or(0, |n| n.instr);
        let until = d.len.max(1) as f64;
        let baked = crate::arp::bake(&a, kit, origin, &held, until);
        d.notes.retain(|n| !plays(n));
        for (s, e, key, vel) in baked {
            let start = s.round() as Tick;
            let end = (e.round() as Tick).max(start + 1);
            d.notes.push(Note {
                start,
                len: end - start,
                key,
                vel: (vel.clamp(0.0, 1.0) * 65535.0).round() as u16,
                instr,
                ..Default::default()
            });
            notes += 1;
        }
        d.notes.sort_by_key(|n| (n.start, n.key));
    }
    song.instruments[slot as usize - 1].arp = None;
    Some((a, clips, notes))
}
