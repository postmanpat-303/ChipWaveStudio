//! MIDI key pressure (docs/MIDI.md, Pressure): recorded curves on notes,
//! their place in the plan, playback into the Pressure modulators, and
//! songs without pressure staying exactly as before.
use super::*;
use crate::engine::Engine;
use crate::model::{Pattern, Song};

const SR: u32 = 22050;
/// 125 BPM at 22 050 Hz: a row is exactly 2 646 frames
const ROW_FRAMES: usize = 2646;

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
    s
}

fn tl(s: &mut Song) -> &mut Timeline {
    s.timeline.as_deref_mut().unwrap()
}

fn clip(s: &mut Song, notes: Vec<Note>) -> u32 {
    let id = new_note_clip(s, 1, 0).unwrap();
    let t = tl(s);
    let c = t.clips.iter().find(|c| c.id == id).unwrap();
    let ClipKind::Notes { data } = c.kind else {
        unreachable!()
    };
    t.notes[data as usize].make_mut().notes = notes;
    id
}

fn pressed(start: Tick, len: Tick) -> Note {
    let mut n = Note {
        start,
        len,
        key: 49,
        instr: 1,
        ..Default::default()
    };
    n.press_push(0, 0.25);
    n.press_push(ROW, 0.5);
    n.press_push(2 * ROW, 1.0);
    n
}

#[test]
fn the_recorder_keeps_a_lean_curve() {
    let mut n = Note::default();
    assert!(n.pressure().is_none());
    n.press_push(0, 0.5);
    n.press_push(0, 0.6); // same tick: replaced
    n.press_push(10, 0.6); // unchanged: nothing added
    n.press_push(20, 2.0); // clamped
    n.press_push(5, 0.1); // behind the last point: replaces it
    assert_eq!(n.pressure().unwrap(), &[(0, 0.6), (20, 0.1)]);
    assert_eq!(n.pressure_at(-1), 0.0);
    assert_eq!(n.pressure_at(0), 0.6);
    assert_eq!(n.pressure_at(19), 0.6);
    assert_eq!(n.pressure_at(400), 0.1);
    n.clear_pressure();
    assert!(n.expr.is_empty(), "no empty lane left behind");
}

#[test]
fn curves_save_only_when_present() {
    let plain = Note::default();
    let j = serde_json::to_string(&plain).unwrap();
    assert!(!j.contains("expr"), "{j}");
    let n = pressed(0, 4 * ROW);
    let j = serde_json::to_string(&n).unwrap();
    assert!(j.contains("pressure"), "{j}");
    let back: Note = serde_json::from_str(&j).unwrap();
    assert_eq!(back, n);
}

#[test]
fn the_plan_carries_the_curves_of_the_notes_that_play() {
    let mut s = song();
    let mut skipped = pressed(8 * ROW, 4 * ROW);
    skipped.cond = Cond::NotFirst; // an unlooped clip is pass 0: never plays
    clip(&mut s, vec![pressed(0, 4 * ROW), skipped]);
    let t = s.timeline.as_deref().unwrap();
    let (spans, press) = note_press_spans(t, 2, false);
    assert_eq!(spans[1].len(), 1);
    assert!(press[0].is_empty());
    let p: Vec<(Tick, f32)> = press[1].iter().map(|p| (p.at, p.val)).collect();
    assert_eq!(
        p,
        vec![(0, 0.25), (ROW, 0.5), (2 * ROW, 1.0), (4 * ROW, 0.0)],
        "the curve, then 0 at the note's end"
    );
    // editing a curve rebuilds the plan
    let k0 = plan_key(&s, s.timeline.as_deref().unwrap());
    let t = tl(&mut s);
    t.notes[0].make_mut().notes[0].press_push(3 * ROW, 0.1);
    assert_ne!(k0, plan_key(&s, s.timeline.as_deref().unwrap()));
}

#[test]
fn a_song_without_pressure_has_an_empty_press_plan() {
    let mut s = song();
    clip(
        &mut s,
        vec![Note {
            start: 0,
            len: ROW,
            key: 49,
            instr: 1,
            ..Default::default()
        }],
    );
    let p = build_plan(&s, s.timeline.as_deref().unwrap());
    assert!(p.press.iter().all(|v| v.is_empty()));
}

#[test]
fn playback_feeds_the_pressure_modulators() {
    let mut s = song();
    clip(&mut s, vec![pressed(0, 4 * ROW)]);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    let run = |e: &mut Engine, frames: usize| {
        let mut buf = vec![0.0f32; 2 * frames];
        e.render(&s, &mut buf);
    };
    run(&mut e, ROW_FRAMES / 2);
    assert_eq!(e.mods.feed.press[0], 0.25);
    run(&mut e, ROW_FRAMES);
    assert_eq!(e.mods.feed.press[0], 0.5);
    run(&mut e, ROW_FRAMES);
    assert_eq!(e.mods.feed.press[0], 1.0);
    run(&mut e, 2 * ROW_FRAMES);
    assert_eq!(e.mods.feed.press[0], 0.0, "back to 0 when the note ends");
    // a seek into the pressed note hears its pressure again
    e.seek(&s, 0, 2);
    run(&mut e, 8);
    assert_eq!(e.mods.feed.press[0], 1.0);
}

#[test]
fn live_pressure_follows_the_hardest_held_key() {
    use crate::midi::{LiveEv, ALL_KEYS};
    let s = song();
    let mut e = Engine::new(SR as f32);
    let on = |key| LiveEv::On {
        key,
        note: 49,
        vel: 1.0,
        instr: 1,
        ch: 0,
    };
    let pr = |key, val| LiveEv::Pressure { key, val, instr: 1 };
    e.live_event(&s, on(60));
    e.live_event(&s, on(64));
    e.live_event(&s, pr(60, 0.75));
    e.live_event(&s, pr(64, 0.25));
    assert_eq!(e.mods.feed.press[0], 0.75, "the harder of the two");
    e.live_event(&s, pr(ALL_KEYS, 0.5));
    assert_eq!(
        e.mods.feed.press[0], 0.5,
        "channel pressure: every held key"
    );
    // a new strike starts unpressed
    e.live_event(&s, LiveEv::Off { key: 60 });
    e.live_event(&s, LiveEv::Off { key: 64 });
    e.live_event(&s, on(67));
    e.live_event(&s, pr(60, 1.0)); // a key that is up
    assert_eq!(e.mods.feed.press[0], 0.0);
}

#[test]
fn a_pressure_modulator_reads_the_feed() {
    use crate::modulation::{ModKind, Modulation};
    let mut m = Modulation::default();
    let k = m.add(0, ModKind::Pressure).unwrap();
    assert_eq!(m.mods[k].kind.name(), "Pressure");
    assert!(ModKind::ALL.contains(&ModKind::Pressure));
    assert!(crate::quickmod::Source::ALL
        .iter()
        .any(|s| s.kind() == ModKind::Pressure));
}
