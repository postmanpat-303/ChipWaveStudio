//! Launcher clips (docs/unified/PLAN.md Stage 4b): the model, the file
//! format (songs without them save byte-identically), and the engine
//! playing one per track, looped from its launch, while the arrangement
//! keeps playing on the other tracks.
use super::*;
use crate::engine::{launch::bit, launch::Cue, render_song, Engine};
use crate::model::{Cell, Pattern, Song};

const SR: u32 = 22050;
const BAR: Tick = 16 * ROW;

/// 2 tracks in timeline mode, no clips
fn song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.bpm = 125.0;
    s.speed = 6;
    s.channels = 2;
    s.channel_names = vec!["A".into(), "B".into()];
    let mut a = Pattern::new("A", 16, 2);
    a.cells[0][0] = Cell {
        note: 49,
        instr: 1,
        ..Default::default()
    };
    s.patterns = vec![a];
    s.order = vec![0];
    s.mixer.channels.resize(2, Default::default());
    enter_timeline_mode(&mut s);
    s.timeline.as_deref_mut().unwrap().clips.clear();
    s
}

fn tl(s: &mut Song) -> &mut Timeline {
    s.timeline.as_deref_mut().unwrap()
}

/// a sustaining (pad-like) instrument of the demo, 1-based
fn pad(s: &Song) -> u8 {
    s.instruments
        .iter()
        .position(|i| i.kind.patch().is_some_and(|p| p.amp.s >= 0.5))
        .expect("a sustaining patch") as u8
        + 1
}

fn notes(p: u8) -> Vec<Note> {
    [(0, 61), (4 * ROW, 65), (8 * ROW, 68), (12 * ROW, 73)]
        .iter()
        .map(|&(start, key)| Note {
            start,
            len: 2 * ROW,
            key,
            instr: p,
            ..Default::default()
        })
        .collect()
}

fn set_notes(t: &mut Timeline, data: usize, v: Vec<Note>) {
    t.notes[data].make_mut().notes = v;
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn run(e: &mut Engine, s: &Song, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames * 2];
    for c in out.chunks_mut(256) {
        e.render(s, c);
    }
    out
}

fn launch_one(tr: usize, id: u32, every: u32) -> Cue {
    let mut c = Cue {
        on: bit(tr),
        every,
        ..Default::default()
    };
    c.clips[tr] = id;
    c
}

#[test]
fn launcher_clips_round_trip_and_songs_without_them_save_the_same() {
    let mut s = song();
    let before = to_toml(tl(&mut s)).unwrap();
    assert!(!before.contains("launcher") && !before.contains("scenes"));
    let a = launch_new(tl(&mut s), 1, 2, BAR);
    assert_eq!(tl(&mut s).scenes.len(), 3, "rows up to the clip's scene");
    let text = to_toml(tl(&mut s)).unwrap();
    assert!(text.contains("[[launcher]]"), "{text}");
    let back = from_toml(&text).unwrap();
    assert!(back.same_content(tl(&mut s)));
    assert_eq!(back.launch_clip(a).unwrap().scene, 2);
    // the reader version: 6 only with launcher clips
    assert!(crate::fileio::needs_reader_6(&s));
    tl(&mut s).launcher.clear();
    tl(&mut s).scenes.clear();
    assert!(!crate::fileio::needs_reader_6(&s));
    let after = to_toml(tl(&mut s)).unwrap();
    assert!(
        !after.contains("launcher") && !after.contains("scenes"),
        "{after}"
    );
}

#[test]
fn copies_link_variations_dont_and_tracks_carry_their_launcher_clips() {
    let mut s = song();
    let id = new_note_clip(&mut s, 0, 0).unwrap();
    let t = tl(&mut s);
    let data = t.any_clip(id).unwrap().notes().unwrap();
    // Capture: a linked copy in a new scene
    let n = launch_capture(t, &[("Verse".into(), vec![(0, id)])]);
    assert_eq!(n, 1);
    assert_eq!(t.scenes[0].name, "Verse");
    let l = launch_at(t, 0, 0).unwrap().clip.id;
    assert_eq!(
        linked(t, data),
        (1, 1),
        "the timeline clip and its launcher copy"
    );
    // Duplicate (variation): its own notes, in the next free scene
    let v = launch_variation(t, l).unwrap();
    assert_eq!(t.launch_clip(v).unwrap().scene, 1);
    assert_ne!(t.any_clip(v).unwrap().notes(), Some(data));
    assert_eq!(linked(t, data), (1, 1));
    // placed in the arrangement: linked to the launcher clip
    let p = place_copy(&mut s, l, 1, 4 * BAR, false).unwrap();
    let t = tl(&mut s);
    assert_eq!(t.any_clip(p).unwrap().notes(), Some(data));
    assert_eq!(linked(t, data), (2, 1));
    // delete the track: its launcher clips go with it (track 1 becomes 0)
    let mut s2 = s.clone();
    crate::chanops::remove_channel(&mut s2, 0).unwrap();
    let t2 = s2.timeline.as_deref().unwrap();
    assert!(t2.launcher.is_empty(), "{:?}", t2.launcher);
    // and the launcher clip can go
    assert!(launch_delete(tl(&mut s), v));
    assert!(tl(&mut s).launch_clip(v).is_none());
}

#[test]
fn launcher_clips_change_nothing_in_the_render() {
    let mut s = song();
    let p = pad(&s);
    let id = new_note_clip(&mut s, 0, 0).unwrap();
    let data = tl(&mut s).any_clip(id).unwrap().notes().unwrap();
    set_notes(tl(&mut s), data, notes(p));
    let want = render_song(&s, SR);
    let mut with = s.clone();
    let t = tl(&mut with);
    launch_capture(t, &[("A".into(), vec![(0, id)])]);
    let k = launch_new(t, 1, 0, BAR);
    let d = t.launch_clip(k).unwrap().clip.notes().unwrap();
    set_notes(t, d, notes(p));
    assert!(
        same(&render_song(&with, SR), &want),
        "the render ignores the launcher"
    );
}

/// launched from a stop, a launcher clip sounds exactly like the same clip
/// looping in the arrangement, pass after pass
#[test]
fn a_launched_clip_plays_like_the_same_clip_looping_in_the_arrangement() {
    let base = song();
    let p = pad(&base);
    let mut arr = base.clone();
    let id = new_note_clip(&mut arr, 1, 0).unwrap();
    let t = tl(&mut arr);
    let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
    c.len = 3 * BAR;
    c.loop_len = Some(BAR);
    let data = c.notes().unwrap();
    set_notes(t, data, notes(p));
    let mut lau = base.clone();
    let t = tl(&mut lau);
    let k = launch_new(t, 1, 0, BAR);
    let d = t.launch_clip(k).unwrap().clip.notes().unwrap();
    set_notes(t, d, notes(p));
    // 2.5 passes
    let frames = (2 * 16 + 8) * 2646;
    let mut e = Engine::new(SR as f32);
    e.start(&arr, 0, 0, false, 0);
    let want = run(&mut e, &arr, frames);
    assert!(
        want.iter().any(|x| x.abs() > 1e-3),
        "the arrangement sounds"
    );
    let mut e = Engine::new(SR as f32);
    e.ensure_plan(&lau);
    e.launch(&lau, launch_one(1, k, 16));
    assert!(e.playing);
    assert_eq!(e.launched(1), Some(k));
    let got = run(&mut e, &lau, frames);
    assert!(same(&got, &want), "sample for sample the same");
}

/// while the arrangement plays, a launch replaces one track's clips on the
/// boundary; the other track plays on; a stop ends the launcher clip
#[test]
fn a_launch_while_playing_takes_over_one_track_on_the_boundary() {
    let mut s = song();
    let p = pad(&s);
    let long = |key| {
        vec![Note {
            start: 0,
            len: 8 * BAR,
            key,
            instr: p,
            ..Default::default()
        }]
    };
    for tr in 0..2 {
        let id = new_note_clip(&mut s, tr, 0).unwrap();
        let t = tl(&mut s);
        let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
        c.len = 8 * BAR;
        let data = c.notes().unwrap();
        set_notes(t, data, long(61 + tr as u8 * 4));
    }
    // the launcher clip on track 0: one short note at its start
    let t = tl(&mut s);
    let k = launch_new(t, 0, 0, BAR);
    let d = t.launch_clip(k).unwrap().clip.notes().unwrap();
    set_notes(
        t,
        d,
        vec![Note {
            start: 0,
            len: ROW,
            key: 70,
            instr: p,
            ..Default::default()
        }],
    );
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    run(&mut e, &s, 3 * 2646 + 10);
    assert_eq!((e.note_voices(0), e.note_voices(1)), (1, 1));
    e.launch(&s, launch_one(0, k, 4));
    run(&mut e, &s, 2646 / 2);
    assert_eq!(e.launched(0), None, "not before the boundary (row 4)");
    assert_eq!(e.note_voices(0), 1, "the arrangement still plays");
    run(&mut e, &s, 2646);
    assert_eq!(e.launched(0), Some(k));
    assert_eq!(
        e.note_voices(0),
        1,
        "the launcher note (the long one ended)"
    );
    assert_eq!(e.note_voices(1), 1, "track 1 plays the arrangement on");
    run(&mut e, &s, 2 * 2646);
    assert_eq!(e.note_voices(0), 0, "the launcher note was a row long");
    // a pass later it plays again
    run(&mut e, &s, 14 * 2646);
    assert_eq!(e.note_voices(0), 1, "looped");
    // a stop: the track leaves the launcher clip (held silent)
    e.launch(
        &s,
        Cue {
            off: bit(0),
            every: 1,
            ..Default::default()
        },
    );
    run(&mut e, &s, 2 * 2646);
    assert_eq!(e.launched(0), None);
    assert_eq!(e.held() & bit(0), bit(0));
    assert_eq!(e.note_voices(1), 1);
}

/// launching launcher clips inside the render loop allocates nothing
#[test]
fn launcher_launches_are_allocation_free() {
    let mut s = song();
    let p = pad(&s);
    let t = tl(&mut s);
    let ids: Vec<u32> = (0..3)
        .map(|k| {
            let id = launch_new(t, k % 2, k, BAR);
            let d = t.launch_clip(id).unwrap().clip.notes().unwrap();
            set_notes(t, d, notes(p));
            id
        })
        .collect();
    let mut e = Engine::new(48000.0);
    e.ensure_plan(&s);
    e.launch(&s, launch_one(0, ids[0], 4));
    let mut buf = vec![0.0f32; 512];
    let mut cycle = |e: &mut Engine| {
        for k in 0..2000usize {
            if k % 150 == 0 {
                let tr = k / 150 % 2;
                e.launch(&s, launch_one(tr, ids[k / 150 % 3], 4));
            }
            e.render(&s, &mut buf);
        }
    };
    cycle(&mut e);
    let n = crate::engine::alloc_count::count(|| cycle(&mut e));
    assert_eq!(n, 0, "launches allocated {n} times");
    assert!(e.playing);
}
