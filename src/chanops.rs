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
    let mv = |c: usize| -> Option<usize> {
        if c < old_n {
            Some(map[c])
        } else {
            None
        }
    };
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
            strips[map[i]] = *s;
            used[map[i]] = true;
        }
    }
    // strips beyond the song's channels stay where they were if free
    for (i, s) in song.mixer.channels.iter().enumerate().skip(old_n) {
        if i < len && !used[i] && i >= n {
            strips[i] = *s;
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
        // tracks past the channels (the Song / Master track with tempo,
        // master and return lanes) stay after them
        let old = std::mem::take(&mut tl.tracks);
        let tail_from = old_n.max(song.channels);
        for (i, t) in old.into_iter().enumerate() {
            if i < old_n {
                tracks[map[i]] = t;
            } else if i >= tail_from {
                tracks.push(t);
            }
        }
        tl.tracks = tracks;
        tl.clips.retain(|c| (c.track as usize) < old_n);
        tl.launcher.retain(|l| (l.clip.track as usize) < old_n);
        for l in &mut tl.launcher {
            l.clip.track = map[l.clip.track as usize] as u16;
        }
        for c in &mut tl.clips {
            c.track = map[c.track as usize] as u16;
            if let crate::timeline::ClipKind::Block { col, .. } = &mut c.kind {
                if (*col as usize) < old_n {
                    *col = map[*col as usize] as u8;
                }
            }
        }
        let bits = tl.view.auto_open;
        let mut nb = 0u64;
        for (i, &m) in map.iter().enumerate().take(old_n.min(64)) {
            if bits >> i & 1 == 1 && m < 64 {
                nb |= 1 << m;
            }
        }
        for i in tail_from..64 {
            let m = n + (i - tail_from);
            if bits >> i & 1 == 1 && m < 64 {
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

/// the channel an automation / modulation target id belongs to
fn target_channel(id: &str) -> Option<usize> {
    match Target::parse(id)? {
        Target::ChanVolume(c) | Target::ChanPan(c) | Target::Send(c, _) => Some(c),
        _ => None,
    }
}

/// Delete channel (track) `at` with everything that belongs to it: its
/// pattern column in every block, mixer strip, name, timeline track (clips,
/// lanes) and the lanes / modulation routes that target it. The others keep
/// their order. Errors (the song unchanged): no such channel, the last
/// channel of a tracker-mode song, or another track's clip plays this
/// track's block column.
pub fn remove_channel(song: &mut Song, at: usize) -> Result<(), String> {
    let n = song.channels;
    if at >= n {
        return Err("no such track".into());
    }
    if song.timeline.is_none() && n <= 1 {
        return Err("a tracker-mode song keeps at least one channel".into());
    }
    if let Some(tl) = song.timeline.as_deref() {
        let shared = tl.clips.iter().any(|c| {
            c.track as usize != at
                && matches!(c.kind, crate::timeline::ClipKind::Block { col, .. } if col as usize == at)
        });
        if shared {
            return Err("another track's clip plays this track's block column".into());
        }
    }
    if at != n - 1 {
        move_channel(song, at, n - 1);
    }
    let k = n - 1;
    // lanes and routes on the channel
    for lanes in song.lane_lists_mut() {
        lanes.retain(|l| target_channel(&l.target) != Some(k));
    }
    let gone: Vec<usize> = (0..song.modulation.routes.len())
        .filter(|&i| target_channel(&song.modulation.routes[i].target) == Some(k))
        .collect();
    for i in gone.into_iter().rev() {
        song.remove_route(i);
    }
    for p in &mut song.patterns {
        for row in &mut p.cells {
            row.truncate(k);
        }
    }
    if let Some(m) = song.mixer.channels.get_mut(k) {
        *m = ChanMix::default();
    }
    if song.mixer.duck_key == k {
        song.mixer.duck_key = 0;
    }
    song.channel_names.truncate(k);
    if let Some(tl) = song.timeline.as_deref_mut() {
        if tl
            .tracks
            .get(k)
            .is_some_and(|t| t.kind != crate::timeline::TrackKind::Master)
        {
            tl.tracks.remove(k);
        }
        tl.clips.retain(|c| c.track as usize != k);
        tl.launcher.retain(|l| l.clip.track as usize != k);
        // auto_open bits above k move down one (the Song / Master track)
        let b = tl.view.auto_open;
        let low = if k == 0 { 0 } else { b & ((1u64 << k) - 1) };
        let high = if k >= 63 { 0 } else { (b >> (k + 1)) << k };
        tl.view.auto_open = low | high;
    }
    song.channels = k;
    Ok(())
}

/// Duplicate channel (track) `tr` just below it: the new track gets a copy
/// of its block column in every block, its mixer strip, its track settings
/// (colour, icon, instrument, speed, lanes) and a copy of each of its clips
/// (note clips get their own notes). False when all channels are used.
pub fn duplicate_channel(song: &mut Song, tr: usize) -> bool {
    if tr >= song.channels || !insert_channel(song, tr + 1, "Ch") {
        return false;
    }
    let new = tr + 1;
    for p in &mut song.patterns {
        for row in &mut p.cells {
            if let Some(c) = row.get(tr).copied() {
                if let Some(d) = row.get_mut(new) {
                    *d = c;
                }
            }
        }
    }
    if let Some(m) = song.mixer.channels.get(tr).copied() {
        if let Some(d) = song.mixer.channels.get_mut(new) {
            *d = m;
        }
    }
    let name = song.channel_names.get(tr).cloned().unwrap_or_default();
    let copy = format!("{} copy", name.trim());
    if let Some(d) = song.channel_names.get_mut(new) {
        *d = copy.clone();
    }
    let retarget = |id: &str| -> Option<String> {
        match Target::parse(id)? {
            Target::ChanVolume(c) if c == tr => Some(Target::ChanVolume(new).id()),
            Target::ChanPan(c) if c == tr => Some(Target::ChanPan(new).id()),
            Target::Send(c, r) if c == tr => Some(Target::Send(new, r).id()),
            _ => None,
        }
    };
    if let Some(tl) = song.timeline.as_deref_mut() {
        if let Some(src) = tl.tracks.get(tr).cloned() {
            let mut t = src;
            t.name = copy;
            for l in &mut t.auto {
                if let Some(x) = retarget(&l.target) {
                    l.target = x;
                }
            }
            if let Some(d) = tl.tracks.get_mut(new) {
                *d = t;
            }
        }
        let src: Vec<crate::timeline::Clip> = tl
            .clips
            .iter()
            .filter(|c| c.track as usize == tr)
            .cloned()
            .collect();
        for mut c in src {
            c.id = tl.new_id();
            c.track = new as u16;
            match &mut c.kind {
                crate::timeline::ClipKind::Block { col, .. } if *col as usize == tr => {
                    *col = new as u8;
                }
                crate::timeline::ClipKind::Notes { data } => {
                    if let Some(nd) = tl.notes.get(*data as usize) {
                        let copy = crate::timeline::Shared::new((**nd).clone());
                        tl.notes.push(copy);
                        *data = (tl.notes.len() - 1) as u32;
                    }
                }
                _ => {}
            }
            for l in &mut c.auto {
                if let Some(x) = retarget(&l.target) {
                    l.target = x;
                }
            }
            tl.clips.push(c);
        }
        tl.clips.sort_by_key(|c| (c.track, c.start, c.id));
    }
    true
}

#[cfg(test)]
mod tests;
