//! Launcher clips (docs/unified/PLAN.md Stage 4b, D1-D4): note clips of the
//! Clip Arranger's own, in scene rows, next to the arrangement. They use the
//! note pool and the clip ids of the timeline, so a launcher clip and a
//! timeline clip can share their notes (linked) and the piano roll edits
//! either. They never play in the arrangement, the render or the export.
use super::*;

/// make sure scene row `k` exists
fn ensure_scene(tl: &mut Timeline, k: usize) {
    while tl.scenes.len() <= k {
        let n = tl.scenes.len();
        tl.scenes.push(Scene {
            name: format!("Scene {}", n + 1),
        });
    }
}

/// the launcher clip on `track` in scene `scene`
pub fn launch_at(tl: &Timeline, track: usize, scene: usize) -> Option<&LaunchClip> {
    tl.launcher
        .iter()
        .find(|l| l.clip.track as usize == track && l.scene as usize == scene)
}

/// A new empty note clip of `len` in the launcher (track, scene); a clip
/// already in that slot is replaced. Returns its id.
pub fn launch_new(tl: &mut Timeline, track: usize, scene: usize, len: Tick) -> u32 {
    ensure_scene(tl, scene);
    let n = tl.notes.len();
    tl.notes.push(Shared::new(NoteClipData {
        name: format!("Clip {}", n + 1),
        len,
        notes: vec![],
        chords: vec![],
        drum: None,
    }));
    let id = tl.new_id();
    tl.launcher
        .retain(|l| !(l.clip.track as usize == track && l.scene as usize == scene));
    tl.launcher.push(LaunchClip {
        scene: scene as u16,
        clip: Clip {
            id,
            track: track as u16,
            start: 0,
            len,
            kind: ClipKind::Notes { data: n as u32 },
            seed: seed_for(id),
            ..Default::default()
        },
    });
    id
}

/// A copy of note clip `src` (timeline or launcher) into the launcher slot
/// (track, scene): linked (the same notes) or `unique` (its own copy). A
/// clip already in that slot is replaced. Not for Tracker or audio clips.
pub fn launch_copy(
    tl: &mut Timeline,
    src: u32,
    track: usize,
    scene: usize,
    unique: bool,
) -> Option<u32> {
    let c = tl.any_clip(src)?.clone();
    let ClipKind::Notes { data } = c.kind else {
        return None;
    };
    ensure_scene(tl, scene);
    let data = if unique {
        let mut d = (**tl.notes.get(data as usize)?).clone();
        d.name = format!("{} copy", d.name);
        tl.notes.push(Shared::new(d));
        (tl.notes.len() - 1) as u32
    } else {
        data
    };
    let id = tl.new_id();
    let mut n = c;
    n.id = id;
    n.seed = seed_for(id);
    n.track = track as u16;
    n.start = 0;
    n.kind = ClipKind::Notes { data };
    tl.launcher
        .retain(|l| !(l.clip.track as usize == track && l.scene as usize == scene));
    tl.launcher.push(LaunchClip {
        scene: scene as u16,
        clip: n,
    });
    Some(id)
}

/// Capture (D2): each `(name, clips)` becomes a new scene row after the
/// existing ones, with a linked copy of each (track, timeline clip). Only
/// note clips are taken. Returns how many clips were captured.
pub fn launch_capture(tl: &mut Timeline, scenes: &[(String, Vec<(usize, u32)>)]) -> usize {
    let mut n = 0;
    for (name, clips) in scenes {
        let k = tl.scenes.len();
        tl.scenes.push(Scene { name: name.clone() });
        for &(track, id) in clips {
            if launch_copy(tl, id, track, k, false).is_some() {
                n += 1;
            }
        }
    }
    n
}

/// Duplicate (variation): an independent copy on the same track in the
/// next scene row where that track is free (a new row when none is).
pub fn launch_variation(tl: &mut Timeline, id: u32) -> Option<u32> {
    let l = tl.launch_clip(id)?.clone();
    let track = l.clip.track as usize;
    let scene = (l.scene as usize + 1..)
        .find(|&k| launch_at(tl, track, k).is_none())
        .unwrap_or(tl.scenes.len());
    launch_copy(tl, id, track, scene, true)
}

/// Remove launcher clip `id` (its notes stay in the pool, as for timeline
/// clips).
pub fn launch_delete(tl: &mut Timeline, id: u32) -> bool {
    let n = tl.launcher.len();
    tl.launcher.retain(|l| l.clip.id != id);
    tl.launcher.len() != n
}

/// How many clips play note data `data`: (in the arrangement, in the
/// launcher). More than one in total: they are linked.
pub fn linked(tl: &Timeline, data: usize) -> (usize, usize) {
    let is = |c: &Clip| c.notes() == Some(data);
    (
        tl.clips.iter().filter(|c| is(c)).count(),
        tl.launcher.iter().filter(|l| is(&l.clip)).count(),
    )
}

/// The launcher clip's notes, from tick 0 of the clip, as the engine plays
/// them while it is launched (loops at its length).
pub fn launch_spans(tl: &Timeline, c: &Clip) -> Vec<NoteSpan> {
    let mut one = c.clone();
    one.start = 0;
    one.muted = false;
    let mut v = vec![];
    clip_note_spans(tl, &one, &mut v);
    v.sort_by_key(|n| (n.start, n.key));
    v
}

/// Remove scene row `k` and its launcher clips; later rows move up.
pub fn launch_delete_scene(tl: &mut Timeline, k: usize) -> bool {
    if k >= tl.scenes.len() {
        return false;
    }
    tl.scenes.remove(k);
    tl.launcher.retain(|l| l.scene as usize != k);
    for l in &mut tl.launcher {
        if l.scene as usize > k {
            l.scene -= 1;
        }
    }
    true
}

/// A new empty scene row at the end; returns its index.
pub fn launch_add_scene(tl: &mut Timeline) -> usize {
    let k = tl.scenes.len();
    ensure_scene(tl, k);
    k
}
