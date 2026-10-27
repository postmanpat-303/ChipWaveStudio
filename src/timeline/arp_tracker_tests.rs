//! Tracker cells through the Arp: a pattern's notes on an instrument with
//! an Arp step through the same engine as note clips, in tracker mode and
//! as a pattern clip on the timeline alike (same steps, same times); a 0xy
//! cell is a chord; a note-off lets go; no Arp = the plain tracker.
use super::*;
use crate::engine::Engine;
use crate::model::{Cell, Pattern, Song, NOTE_OFF};

const SR: u32 = 22050;
/// 125 BPM at 22 050 Hz: a row is exactly 2 646 frames
const ROW_FRAMES: usize = 2646;

fn pad(s: &Song) -> u8 {
    s.instruments
        .iter()
        .position(|i| i.kind.patch().is_some_and(|p| p.amp.s >= 0.5))
        .unwrap() as u8
        + 1
}

/// a 16-row pattern: C-4 with 047 (C E G) on row 0, a note-off on row 8
fn song(a: Option<crate::arp::Arp>, timeline: bool) -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.bpm = 125.0;
    s.speed = 6;
    s.channels = 2;
    s.channel_names = vec!["A".into(), "B".into()];
    s.timeline = None;
    let p = pad(&s);
    s.instruments[p as usize - 1].arp = a;
    let mut pt = Pattern::new("A", 16, 2);
    pt.cells[0][0] = Cell {
        note: 49,
        instr: p,
        fx: 0x00,
        fxp: 0x47,
        ..Default::default()
    };
    pt.cells[8][0] = Cell {
        note: NOTE_OFF,
        ..Default::default()
    };
    s.patterns = vec![pt];
    s.order = vec![0];
    s.mixer.channels.resize(2, Default::default());
    if timeline {
        enter_timeline_mode(&mut s);
    }
    s
}

fn run(e: &mut Engine, s: &Song, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames * 2];
    for c in out.chunks_mut(256) {
        e.render(s, c);
    }
    out
}

fn play(s: &Song, rows: usize) -> (Engine, Vec<(usize, Tick, u8)>) {
    let mut e = Engine::new(SR as f32);
    e.start(s, 0, 0, false, 0);
    run(&mut e, s, rows * ROW_FRAMES);
    let log = e.arp_log();
    (e, log)
}

#[test]
fn tracker_cells_step_through_the_arp_and_a_note_off_ends_them() {
    let s = song(Some(Default::default()), false);
    assert!(s.timeline.is_none(), "tracker mode");
    let (e, log) = play(&s, 12);
    let keys: Vec<u8> = log.iter().map(|x| x.2).collect();
    assert_eq!(
        keys,
        [49, 53, 56, 49, 53, 56, 49, 53],
        "047 = the C E G chord"
    );
    let t0 = log[0].1;
    let starts: Vec<Tick> = log.iter().map(|x| x.1 - t0).collect();
    assert_eq!(starts, (0..8).map(|k| k * ROW).collect::<Vec<_>>());
    assert!(!e.arp_holding(0), "the note-off let go");
    assert_eq!(e.note_voices(0), 0, "nothing hangs on");
}

#[test]
fn tracker_and_timeline_play_the_same_steps() {
    for a in [
        crate::arp::Arp::default(),
        crate::arp::Arp {
            mode: crate::arp::Mode::Random,
            octaves: 2,
            swing: 0.3,
            ..Default::default()
        },
        crate::arp::preset("Chip Fast Chord").unwrap(),
        crate::arp::preset("Euclid 5/8 Pluck").unwrap(),
    ] {
        let (_, tr) = play(&song(Some(a.clone()), false), 12);
        let s = song(Some(a), true);
        assert!(s.timeline.is_some());
        let (_, tl) = play(&s, 12);
        assert!(!tr.is_empty());
        assert_eq!(tr, tl, "tracker mode and the pattern clip: the same steps");
    }
}

#[test]
fn without_an_arp_tracker_cells_play_as_before() {
    let s = song(None, false);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    let out = run(&mut e, &s, 4 * ROW_FRAMES);
    assert!(e.arp_log().is_empty());
    assert!(!e.arp_holding(0));
    assert_eq!(e.note_voices(0), 0, "the track's own voice plays it");
    assert!(out.iter().any(|x| x.abs() > 1e-4), "and it sounds");
}

#[test]
fn a_rate_lane_drives_the_arp() {
    let mut s = song(Some(Default::default()), false);
    let i = pad(&s) as usize - 1;
    let mut lane =
        crate::automation::Lane::new(crate::automation::Target::Arp(i, 0), "Arp rate".into());
    for pos in [0.0, 16.0] {
        lane.points.push(crate::automation::Point {
            pos,
            value: 1.0,
            shape: Default::default(),
            curve: 0.0,
        });
    }
    s.automation.push(lane);
    let (_, log) = play(&s, 4);
    // the fastest rate (1/128): 8 steps a 1/16 row
    assert!(log.len() >= 24, "{} steps", log.len());
}

#[test]
fn arp_targets_parse_and_set() {
    use crate::automation::Target;
    let mut s = song(Some(Default::default()), false);
    let i = pad(&s) as usize - 1;
    for (k, p) in crate::arp::Param::ALL.iter().enumerate() {
        let t = Target::Arp(i, k);
        assert_eq!(Target::parse(&t.id()), Some(t), "{}", t.id());
        assert!(crate::automation::modulatable(t));
        assert!(crate::knobs::get(&s, t).is_some(), "{}", p.name());
        assert!(crate::knobs::set_norm(&mut s, t, 1.0));
        let n = crate::automation::song_norm(&s, t).unwrap();
        assert!((n - 1.0).abs() < 1e-3, "{} {n}", p.name());
    }
    let a = s.instruments[i].arp.as_ref().unwrap();
    assert_eq!((a.div, a.octaves), (128, 4));
    // no Arp: no knob
    s.instruments[i].arp = None;
    assert!(crate::knobs::get(&s, Target::Arp(i, 0)).is_none());
}

#[test]
fn scale_lock_follows_the_song_key_in_both_modes() {
    // C#-4 with 047 = C# F G#, none of them in A minor: Scale lock moves
    // every step into the song's key
    for tl in [false, true] {
        let mut s = song(
            Some(crate::arp::Arp {
                scale_lock: true,
                ..Default::default()
            }),
            false,
        );
        s.info.key = crate::chords::parse_key("A minor");
        s.patterns[0].cells[0][0].note = 50;
        if tl {
            enter_timeline_mode(&mut s);
        }
        let (_, log) = play(&s, 8);
        assert!(!log.is_empty());
        let key = s.info.key.unwrap();
        assert!(
            log.iter().all(|x| key.contains(crate::chords::pc_of(x.2))),
            "{:?}",
            log.iter().map(|x| x.2).collect::<Vec<_>>()
        );
    }
}
