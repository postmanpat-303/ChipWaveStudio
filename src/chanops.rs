//! Channel (track) edits that renumber channels: reorder, insert, and
//! giving a track an instrument (Browser drops, docs/TIMELINE.md).
//!
//! Tracks are mixer channels, so a move carries everything that belongs to
//! the channel: its pattern column in every block, its mixer strip (volume,
//! pan, sends, ducking), its name, the timeline track (clips, colour,
//! height, track automation) and every automation lane / modulation route
//! that targets the channel. Playback after a move is the same music; only
//! the order the channels are summed in changes (see `tests`).

use crate::automation::Target;
use crate::model::{Cell, ChanMix, Song, MAX_CHANNELS};

/// Renumber channels: old channel `i` becomes `map[i]` in a song of `n`
/// channels. `map` must be injective into `0..n` with one entry per old
/// channel; new channels (not in the image) start empty with default
/// mixer settings and the name `new_name`.
pub fn remap_channels(song: &mut Song, map: &[usize], n: usize, new_name: &str) {
    let old_n = song.channels.min(map.len());
    debug_assert!(n <= MAX_CHANNELS && map.iter().take(old_n).all(|&m| m < n));
    let mv = |c: usize| -> Option<usize> { if c < old_n { Some(map[c]) } else { None } };
    // pattern columns
    for p in &mut song.patterns {
        for row in &mut p.cells {
            let mut out = vec![Cell::default(); n];
            for (i, cell) in row.iter().enumerate().take(old_n) {
                out[map[i]] = *cell;
            }
            *row = out;
        }
    }
    // mixer strips (the mixer keeps MAX_CHANNELS strips; unused ones too)
    let len = song.mixer.channels.len().max(n);
    let mut strips = vec![ChanMix::default(); len];
    let mut used = vec![false; len];
    for i in 0..old_n {
        if let Some(s) = song.mixer.channels.get(i) {
            strips[map[i]] = s.clone();
            used[map[i]] = true;
        }
    }
    // strips beyond the song's channels stay where they were if free
    for (i, s) in song.mixer.channels.iter().enumerate().skip(old_n) {
        if i < len && !used[i] && i >= n {
            strips[i] = s.clone();
        }
    }
    song.mixer.channels = strips;
    if let Some(k) = mv(song.mixer.duck_key) {
        song.mixer.duck_key = k;
    }
    // names
    let mut names = vec![new_name.to_string(); n];
    for i in 0..old_n {
        if let Some(s) = song.channel_names.get(i) {
            names[map[i]] = s.clone();
        }
    }
    song.channel_names = names;
    // timeline tracks and clips
    if let Some(tl) = song.timeline.as_deref_mut() {
        let mut tracks: Vec<crate::timeline::Track> = (0..n)
            .map(|_| crate::timeline::Track {
                name: new_name.to_string(),
                ..Default::default()
            })
            .collect();
        for (i, t) in std::mem::take(&mut tl.tracks).into_iter().enumerate() {
            if i < old_n {
                tracks[map[i]] = t;
            }
        }
        tl.tracks = tracks;
        tl.clips.retain(|c| (c.track as usize) < old_n);
        for c in &mut tl.clips {
            c.track = map[c.track as usize] as u16;
            if let crate::timeline::ClipKind::Block { col, .. } = &mut c.kind {
                if (*col as usize) < old_n {
                    *col = map[*col as usize] as u8;
                }
            }
        }
        let bits = tl.view.auto_open;
        let mut nb = 0u32;
        for (i, &m) in map.iter().enumerate().take(old_n.min(32)) {
            if bits >> i & 1 == 1 && m < 32 {
                nb |= 1 << m;
            }
        }
        tl.view.auto_open = nb;
    }
    // automation lanes and modulation routes that target a channel
    let fix = |id: &str| -> Option<String> {
        match Target::parse(id)? {
            Target::ChanVolume(c) => Some(Target::ChanVolume(mv(c)?).id()),
            Target::ChanPan(c) => Some(Target::ChanPan(mv(c)?).id()),
            Target::Send(c, r) => Some(Target::Send(mv(c)?, r).id()),
            _ => None,
        }
    };
    for lanes in song.lane_lists_mut() {
        for l in lanes.iter_mut() {
            if let Some(t) = fix(&l.target) {
                l.target = t;
            }
        }
    }
    for r in &mut song.modulation.routes {
        if let Some(t) = fix(&r.target) {
            r.target = t;
        }
    }
    song.channels = n;
}

/// move channel `from` so it lands at index `to` (0-based, after removal),
/// everything else keeps its order
pub fn move_channel(song: &mut Song, from: usize, to: usize) -> bool {
    let n = song.channels;
    if from >= n || to >= n || from == to {
        return false;
    }
    let mut order: Vec<usize> = (0..n).filter(|&c| c != from).collect();
    order.insert(to, from);
    // order[new] = old  ->  map[old] = new
    let mut map = vec![0; n];
    for (new, &old) in order.iter().enumerate() {
        map[old] = new;
    }
    remap_channels(song, &map, n, "Ch");
    true
}

/// insert an empty channel at `at` (0..=channels); false when the song
/// already has `MAX_CHANNELS`
pub fn insert_channel(song: &mut Song, at: usize, name: &str) -> bool {
    let n = song.channels;
    if n >= MAX_CHANNELS {
        return false;
    }
    let at = at.min(n);
    let map: Vec<usize> = (0..n).map(|c| if c < at { c } else { c + 1 }).collect();
    remap_channels(song, &map, n + 1, name);
    true
}

/// Give track `tr` instrument `slot` (1-based): every note on the track
/// is played by it from now on. Timeline mode: the notes of the track's
/// block clips (their column in the block) and note clips; tracker mode:
/// the channel's column in every block. Also remembered as the track's
/// instrument (`Track::instr`). Returns the number of notes changed.
pub fn assign_track_instrument(song: &mut Song, tr: usize, slot: u8) -> usize {
    let mut n = 0;
    let set = |c: &mut Cell, n: &mut usize| {
        if (1..=120).contains(&c.note) && c.instr != slot {
            c.instr = slot;
            *n += 1;
        }
    };
    if let Some(tl) = song.timeline.as_deref_mut() {
        let mut cols = std::collections::BTreeSet::new();
        let mut datas = std::collections::BTreeSet::new();
        for c in tl.clips.iter().filter(|c| c.track as usize == tr) {
            match c.kind {
                crate::timeline::ClipKind::Block { pattern, col } => {
                    cols.insert((pattern, col as usize));
                }
                crate::timeline::ClipKind::Notes { data } => {
                    datas.insert(data as usize);
                }
                crate::timeline::ClipKind::Audio { .. } => {}
            }
        }
        for d in datas {
            if let Some(nd) = tl.notes.get_mut(d) {
                for note in &mut nd.make_mut().notes {
                    if note.instr != slot {
                        note.instr = slot;
                        n += 1;
                    }
                }
            }
        }
        if let Some(t) = tl.tracks.get_mut(tr) {
            t.instr = slot;
        }
        for (p, col) in cols {
            if let Some(p) = song.patterns.get_mut(p) {
                for row in &mut p.cells {
                    if let Some(c) = row.get_mut(col) {
                        set(c, &mut n);
                    }
                }
            }
        }
    } else {
        for p in &mut song.patterns {
            for row in &mut p.cells {
                if let Some(c) = row.get_mut(tr) {
                    set(c, &mut n);
                }
            }
        }
    }
    n
}

#[cfg(test)]
mod tests;
