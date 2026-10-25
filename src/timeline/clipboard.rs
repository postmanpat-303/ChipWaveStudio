//! The Timeline's clip clipboard (Ctrl+C / Ctrl+X / Ctrl+V): a copy of the
//! selected clips with what they played when copied, pasted as independent
//! copies (their own notes and blocks) or, on request, linked ones.
use super::*;

/// Clips on the clipboard. `clips` keep their ids, starts and tracks as
/// copied; `notes` and `blocks` hold their content as it was then, so a
/// paste gives what was copied even after the originals changed or went.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipBoard {
    /// sorted by (start, track)
    pub clips: Vec<Clip>,
    /// note data by pool index at copy time
    pub notes: Vec<(u32, NoteClipData)>,
    /// blocks by index at copy time
    pub blocks: Vec<(usize, Pattern)>,
}

impl ClipBoard {
    pub fn len(&self) -> usize {
        self.clips.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }

    /// the earliest start and the top (lowest) track
    pub fn origin(&self) -> (Tick, u16) {
        (
            self.clips.iter().map(|c| c.start).min().unwrap_or(0),
            self.clips.iter().map(|c| c.track).min().unwrap_or(0),
        )
    }

    /// how many tracks the clips span (1 = all on one track)
    pub fn tracks(&self) -> usize {
        let (_, a) = self.origin();
        let b = self.clips.iter().map(|c| c.track).max().unwrap_or(a);
        (b - a) as usize + 1
    }
}

/// Copy the clips `ids` (timeline clips only) with their notes and blocks.
pub fn copy_clips(song: &Song, ids: &[u32]) -> ClipBoard {
    let mut cb = ClipBoard::default();
    let Some(tl) = song.timeline.as_deref() else {
        return cb;
    };
    for c in tl.clips.iter().filter(|c| ids.contains(&c.id)) {
        match c.kind {
            ClipKind::Notes { data } => {
                if !cb.notes.iter().any(|x| x.0 == data) {
                    if let Some(d) = tl.notes.get(data as usize) {
                        cb.notes.push((data, (**d).clone()));
                    }
                }
            }
            ClipKind::Block { pattern, .. } => {
                if !cb.blocks.iter().any(|x| x.0 == pattern) {
                    if let Some(p) = song.patterns.get(pattern) {
                        cb.blocks.push((pattern, p.clone()));
                    }
                }
            }
            ClipKind::Audio { .. } => {}
        }
        cb.clips.push(c.clone());
    }
    cb.clips.sort_by_key(|c| (c.start, c.track));
    cb
}

/// What [`paste_clips`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pasted {
    /// the new clips' ids
    pub ids: Vec<u32>,
    /// clips not pasted: no track of their kind there (audio clips go on
    /// audio tracks, note and Tracker clips on the others)
    pub left_out: usize,
    /// Tracker clips that play their original block (no room for another)
    pub linked: usize,
    /// the track the top clip went on (after keeping them all on the song)
    pub track: usize,
}

/// Paste `cb` with its earliest clip at `at` and its top track on `track`;
/// the clips keep their distances in time and between tracks (moved up
/// when they would run past the last track). Independent copies: each
/// copied note clip and block gets its own copy (clips that shared one
/// share the new one). `linked`: the copies play the same notes and blocks
/// as the originals. Clips land over what is there (as when clips are
/// moved); nothing on the timeline is removed or changed.
pub fn paste_clips(
    song: &mut Song,
    cb: &ClipBoard,
    track: usize,
    at: Tick,
    linked: bool,
) -> Pasted {
    let mut out = Pasted::default();
    let nch = song.channels.min(MAX_CHANNELS);
    if cb.is_empty() || nch == 0 {
        return out;
    }
    let (t0, top) = cb.origin();
    let span = cb.tracks();
    let base = track.min(nch.saturating_sub(span));
    out.track = base;
    let mut notes_map: HashMap<u32, u32> = HashMap::new();
    let mut block_map: HashMap<usize, usize> = HashMap::new();
    for c in &cb.clips {
        let tr = base + (c.track - top) as usize;
        let Some(tl) = song.timeline.as_deref() else {
            return out;
        };
        let kind_ok = match tl.tracks.get(tr).map(|t| t.kind) {
            Some(TrackKind::Master) => false,
            Some(TrackKind::Audio) => c.audio().is_some(),
            _ => c.audio().is_none(),
        };
        if tr >= nch || !kind_ok {
            out.left_out += 1;
            continue;
        }
        let kind = match c.kind {
            ClipKind::Notes { data } => {
                let tl = song.timeline.as_deref_mut().unwrap();
                let live = linked && (data as usize) < tl.notes.len();
                if live {
                    c.kind.clone()
                } else if let Some(&nd) = notes_map.get(&data) {
                    ClipKind::Notes { data: nd }
                } else {
                    let Some(d) = cb.notes.iter().find(|x| x.0 == data) else {
                        out.left_out += 1;
                        continue;
                    };
                    let mut d = d.1.clone();
                    if !linked {
                        d.name = format!("{} copy", d.name);
                    }
                    tl.notes.push(Shared::new(d));
                    let nd = (tl.notes.len() - 1) as u32;
                    notes_map.insert(data, nd);
                    ClipKind::Notes { data: nd }
                }
            }
            ClipKind::Block { pattern, col } => {
                let have = pattern < song.patterns.len();
                if linked && have {
                    c.kind.clone()
                } else if let Some(&nb) = block_map.get(&pattern) {
                    ClipKind::Block { pattern: nb, col }
                } else if song.patterns.len() < crate::fileio::MAX_BLOCKS {
                    let Some(p) = cb.blocks.iter().find(|x| x.0 == pattern) else {
                        out.left_out += 1;
                        continue;
                    };
                    let mut p = p.1.clone();
                    p.name = format!("{} copy", p.name);
                    p.derived = None;
                    song.patterns.push(p);
                    let nb = song.patterns.len() - 1;
                    block_map.insert(pattern, nb);
                    ClipKind::Block { pattern: nb, col }
                } else if have {
                    out.linked += 1;
                    c.kind.clone()
                } else {
                    out.left_out += 1;
                    continue;
                }
            }
            // the file never changes: copies share it
            ClipKind::Audio { .. } => c.kind.clone(),
        };
        let tl = song.timeline.as_deref_mut().unwrap();
        let nid = tl.new_id();
        let mut n = c.clone();
        n.id = nid;
        n.seed = seed_for(nid);
        n.track = tr as u16;
        n.start = (c.start - t0 + at).max(0);
        n.kind = kind;
        tl.clips.push(n);
        out.ids.push(nid);
    }
    if let Some(tl) = song.timeline.as_deref_mut() {
        tl.sort();
    }
    out
}
