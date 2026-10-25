//! Follow actions (docs/ROUND_ROBIN.md): each action's target, the chance
//! split, the file format (omitted while off), the engine moving on by
//! itself (and stopping), and Capture to timeline playing exactly what the
//! launcher plays.
use super::follow::*;
use super::*;
use crate::engine::{launch::bit, launch::Cue, Engine};
use crate::model::{Pattern, Song};

const SR: u32 = 22050;
const BAR: Tick = 16 * ROW;
/// frames per row at 125 BPM, speed 6, 22050 Hz
const FR: usize = 2646;

fn song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.bpm = 125.0;
    s.speed = 6;
    s.channels = 2;
    s.channel_names = vec!["A".into(), "B".into()];
    s.patterns = vec![Pattern::new("A", 16, 2)];
    s.order = vec![0];
    s.mixer.channels.resize(2, Default::default());
    enter_timeline_mode(&mut s);
    s.timeline.as_deref_mut().unwrap().clips.clear();
    s
}

fn tl(s: &mut Song) -> &mut Timeline {
    s.timeline.as_deref_mut().unwrap()
}

/// a launcher clip on (track, scene) with one note of `key`
fn clip(s: &mut Song, tr: usize, scene: usize, key: u8, f: Option<Follow>) -> u32 {
    let p = s
        .instruments
        .iter()
        .position(|i| i.kind.patch().is_some())
        .unwrap() as u8
        + 1;
    let t = tl(s);
    let id = launch_new(t, tr, scene, BAR);
    let d = t.launch_clip(id).unwrap().clip.notes().unwrap();
    t.notes[d].make_mut().notes = vec![Note {
        start: 0,
        len: 2 * ROW,
        key,
        instr: p,
        ..Default::default()
    }];
    t.launcher
        .iter_mut()
        .find(|l| l.clip.id == id)
        .unwrap()
        .follow = f;
    id
}

fn follow(after: u16, a: Act, b: Act, chance: u8) -> Option<Follow> {
    Some(Follow {
        after,
        a,
        b,
        chance,
    })
}

fn run_frames(e: &mut Engine, s: &Song, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames * 2];
    for c in out.chunks_mut(256) {
        e.render(s, c);
    }
    out
}

fn scene_cue(s: &Song, scene: usize) -> Cue {
    let mut c = Cue {
        every: 16,
        ..Default::default()
    };
    for l in &s.timeline.as_deref().unwrap().launcher {
        if l.scene as usize == scene {
            c.clips[l.clip.track as usize] = l.clip.id;
            c.on |= bit(l.clip.track as usize);
        }
    }
    c
}

#[test]
fn off_by_default_and_not_saved_until_used() {
    let mut s = song();
    let id = clip(&mut s, 0, 0, 60, None);
    let t = s.timeline.as_deref().unwrap();
    let j = serde_json::to_string(t).unwrap();
    assert!(!j.contains("follow"), "no follow key while off");
    let mut s2 = s.clone();
    tl(&mut s2).launcher[0].follow = follow(3, Act::Other, Act::Stop, 60);
    let j = serde_json::to_string(s2.timeline.as_deref().unwrap()).unwrap();
    assert!(j.contains("\"follow\""));
    let back: Timeline = serde_json::from_str(&j).unwrap();
    assert_eq!(
        back.launch_clip(id).unwrap().follow,
        follow(3, Act::Other, Act::Stop, 60)
    );
    assert_eq!(Follow::default().after, 4);
}

#[test]
fn each_action_picks_its_clip() {
    // track clips 10, 11, 12 in scene rows 0, 2, 5
    let slots = [(10u32, 0u16), (11, 2), (12, 5)];
    let it = || slots.iter().copied();
    assert_eq!(target(Act::Next, 10, 0, it(), 0), Some(11));
    assert_eq!(target(Act::Next, 12, 5, it(), 0), Some(10), "wraps");
    assert_eq!(target(Act::Again, 11, 2, it(), 0), Some(11));
    assert_eq!(target(Act::First, 12, 5, it(), 0), Some(10));
    assert_eq!(target(Act::Last, 10, 0, it(), 0), Some(12));
    assert_eq!(target(Act::Stop, 10, 0, it(), 0), None);
    for d in 0..50 {
        assert_ne!(target(Act::Other, 11, 2, it(), d), Some(11), "never itself");
        assert!(target(Act::Any, 11, 2, it(), d).is_some());
    }
    let any: std::collections::HashSet<_> =
        (0..50).map(|d| target(Act::Any, 11, 2, it(), d)).collect();
    assert_eq!(any.len(), 3, "Any reaches every clip, itself too");
    // alone on its track: Other and Next play it again
    let one = [(10u32, 0u16)];
    assert_eq!(target(Act::Other, 10, 0, one.iter().copied(), 7), Some(10));
    assert_eq!(target(Act::Next, 10, 0, one.iter().copied(), 7), Some(10));
}

#[test]
fn the_chance_splits_between_the_two_actions_the_same_every_time() {
    let f = Follow {
        after: 1,
        a: Act::Next,
        b: Act::Again,
        chance: 75,
    };
    let a: Vec<Act> = (0..400).map(|k| decide(&f, 1234, k).0).collect();
    let b: Vec<Act> = (0..400).map(|k| decide(&f, 1234, k).0).collect();
    assert_eq!(a, b, "pure");
    let next = a.iter().filter(|x| **x == Act::Next).count();
    assert!((260..340).contains(&next), "about 75 %: {next}");
    let c: Vec<Act> = (0..400).map(|k| decide(&f, 99, k).0).collect();
    assert_ne!(a, c, "another seed, another run");
    for k in 0..50 {
        let all = Follow { chance: 100, ..f };
        let none = Follow { chance: 0, ..f };
        assert_eq!(decide(&all, 5, k).0, Act::Next);
        assert_eq!(decide(&none, 5, k).0, Act::Again);
    }
    assert_eq!(f.describe(), "after 1 loop: 75 % Next, else Again");
}

#[test]
fn the_engine_moves_on_by_itself_and_stops() {
    let mut s = song();
    let a = clip(&mut s, 0, 0, 60, follow(2, Act::Next, Act::Next, 100));
    let b = clip(&mut s, 0, 1, 64, follow(1, Act::Stop, Act::Stop, 100));
    let other = clip(&mut s, 1, 0, 67, None);
    let mut e = Engine::new(SR as f32);
    e.ensure_plan(&s);
    e.launch(&s, scene_cue(&s, 0));
    assert_eq!(e.launched(0), Some(a));
    run_frames(&mut e, &s, 31 * FR);
    assert_eq!(e.launched(0), Some(a), "two loops of A");
    run_frames(&mut e, &s, 2 * FR);
    assert_eq!(e.launched(0), Some(b), "then Next");
    run_frames(&mut e, &s, 16 * FR);
    assert_eq!(e.launched(0), None, "B stops after one loop");
    assert_eq!(e.held() & bit(0), bit(0), "held silent, as the Stop button");
    assert_eq!(e.launched(1), Some(other), "the other track plays on");
}

/// what the engine launches bar by bar is `follow::run`
#[test]
fn the_engine_follows_the_same_run_as_capture() {
    let mut s = song();
    let ids = [
        clip(&mut s, 0, 0, 60, follow(1, Act::Any, Act::Next, 70)),
        clip(&mut s, 0, 1, 62, follow(2, Act::Other, Act::Again, 50)),
        clip(&mut s, 0, 2, 64, follow(1, Act::First, Act::Last, 40)),
    ];
    let plan = launch_plan(s.timeline.as_deref().unwrap());
    let bars = 24;
    let runs = follow::run(&plan, ids[0], bars * BAR);
    let mut want = vec![];
    for (id, d) in &runs {
        for _ in 0..d / BAR {
            want.push(*id);
        }
    }
    assert_eq!(want.len(), bars as usize);
    assert!(runs.len() > 4, "it moves around: {runs:?}");
    let mut e = Engine::new(SR as f32);
    e.ensure_plan(&s);
    e.launch(&s, scene_cue(&s, 0));
    let mut got = vec![];
    run_frames(&mut e, &s, FR / 2);
    for _ in 0..bars {
        got.push(e.launched(0).unwrap());
        run_frames(&mut e, &s, 16 * FR);
    }
    assert_eq!(got, want);
}

/// Capture to timeline: the arrangement it writes renders sample for
/// sample like the launched scene
#[test]
fn capture_to_timeline_renders_like_the_launcher() {
    let mut s = song();
    clip(&mut s, 0, 0, 60, follow(1, Act::Any, Act::Next, 70));
    clip(&mut s, 0, 1, 62, follow(2, Act::Other, Act::Again, 50));
    clip(&mut s, 0, 2, 64, follow(1, Act::First, Act::Last, 40));
    clip(&mut s, 1, 0, 55, None);
    let bars = 12;
    let mut arr = s.clone();
    let ids = capture(tl(&mut arr), 0, 0, bars * BAR);
    assert!(ids.len() > 3, "{} clips", ids.len());
    let t = arr.timeline.as_deref().unwrap();
    for tr in 0..2 {
        let span: Tick = t
            .clips
            .iter()
            .filter(|c| c.track as usize == tr)
            .map(|c| c.len)
            .sum();
        assert_eq!(span, bars * BAR, "track {tr} fills the span");
    }
    let frames = (bars as usize * 16 - 1) * FR;
    let mut e = Engine::new(SR as f32);
    e.start(&arr, 0, 0, false, 0);
    let want = run_frames(&mut e, &arr, frames);
    assert!(want.iter().any(|x| x.abs() > 1e-3));
    let mut e = Engine::new(SR as f32);
    e.ensure_plan(&s);
    e.launch(&s, scene_cue(&s, 0));
    let got = run_frames(&mut e, &s, frames);
    let same = got
        .iter()
        .zip(&want)
        .all(|(x, y)| x.to_bits() == y.to_bits());
    assert!(same, "sample for sample the same");
}

/// a song with no follow actions plans and plays as before
#[test]
fn clips_without_follow_actions_loop_as_before() {
    let mut s = song();
    let a = clip(&mut s, 0, 0, 60, None);
    clip(&mut s, 0, 1, 62, None);
    let mut e = Engine::new(SR as f32);
    e.ensure_plan(&s);
    e.launch(&s, scene_cue(&s, 0));
    run_frames(&mut e, &s, 16 * 6 * FR);
    assert_eq!(e.launched(0), Some(a));
    assert_eq!(
        follow::run(&launch_plan(s.timeline.as_deref().unwrap()), a, 8 * BAR),
        vec![(a, 8 * BAR)]
    );
}
