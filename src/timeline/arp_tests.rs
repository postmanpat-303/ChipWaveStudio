//! The Arp on note clips: steps from the song position, seeks and loops
//! land on the same notes, nothing hangs on after stop / seek / loop, and a
//! song without an Arp plays exactly as before.
use super::*;
use crate::engine::{render_song, Engine};
use crate::model::{Cell, Pattern, Song};

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
    s
}

fn tl(s: &mut Song) -> &mut Timeline {
    s.timeline.as_deref_mut().unwrap()
}

/// a sustaining instrument of the demo, 1-based
fn pad(s: &Song) -> u8 {
    s.instruments
        .iter()
        .position(|i| i.kind.patch().is_some_and(|p| p.amp.s >= 0.5))
        .unwrap() as u8
        + 1
}

fn note_clip(s: &mut Song, start: Tick, notes: Vec<Note>) {
    let id = new_note_clip(s, 1, start).unwrap();
    let t = tl(s);
    let c = t.clips.iter().find(|c| c.id == id).unwrap();
    let ClipKind::Notes { data } = c.kind else {
        unreachable!()
    };
    t.notes[data as usize].make_mut().notes = notes;
}

fn n(start: Tick, len: Tick, key: u8, instr: u8) -> Note {
    Note {
        start,
        len,
        key,
        instr,
        ..Default::default()
    }
}

fn run(e: &mut Engine, s: &Song, frames: usize) {
    let mut out = vec![0.0f32; frames * 2];
    for c in out.chunks_mut(256) {
        e.render(s, c);
    }
}

/// a C E G chord on the pad for `rows`, with `a` on the pad
fn chord_song(a: Option<crate::arp::Arp>, rows: Tick) -> Song {
    let mut s = song();
    let p = pad(&s);
    s.instruments[p as usize - 1].arp = a;
    note_clip(
        &mut s,
        0,
        vec![
            n(0, rows * ROW, 49, p),
            n(0, rows * ROW, 53, p),
            n(0, rows * ROW, 56, p),
        ],
    );
    s
}

fn keys(l: &[(usize, Tick, u8)]) -> Vec<u8> {
    l.iter().map(|x| x.2).collect()
}

#[test]
fn the_arp_plays_the_chord_one_note_per_step() {
    let s = chord_song(Some(Default::default()), 8);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    run(&mut e, &s, ROW_FRAMES / 4);
    assert_eq!(e.note_voices(1), 1, "one note at a time, not the chord");
    run(&mut e, &s, ROW_FRAMES / 2);
    assert_eq!(e.note_voices(1), 0, "the 50% gate let it go");
    run(&mut e, &s, 8 * ROW_FRAMES);
    let log = e.arp_log();
    assert_eq!(keys(&log), [49, 53, 56, 49, 53, 56, 49, 53]);
    let starts: Vec<Tick> = log.iter().map(|x| x.1).collect();
    assert_eq!(starts, (0..8).map(|k| k * ROW).collect::<Vec<_>>());
    assert!(!e.arp_holding(1), "done when the notes end");
    assert_eq!(e.note_voices(1), 0);
}

#[test]
fn a_seek_plays_the_same_steps_as_playing_through() {
    let a = crate::arp::Arp {
        mode: crate::arp::Mode::Random,
        octaves: 2,
        swing: 0.25,
        ..Default::default()
    };
    let mut s = chord_song(Some(a), 16);
    let p = pad(&s);
    tl(&mut s).notes[0]
        .make_mut()
        .notes
        .push(n(6 * ROW, 6 * ROW, 60, p));
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    run(&mut e, &s, 17 * ROW_FRAMES);
    let full = e.arp_log();
    assert!(full.len() >= 15);
    for from in [3usize, 5, 7, 10] {
        let mut e = Engine::new(SR as f32);
        e.start(&s, 0, from, false, 0);
        run(&mut e, &s, (17 - from) * ROW_FRAMES);
        let part = e.arp_log();
        let tail: Vec<_> = full
            .iter()
            .filter(|x| x.1 >= from as Tick * ROW)
            .copied()
            .collect();
        assert_eq!(part, tail, "from row {from}");
        // a seek while playing as well
        e.seek(&s, 0, from);
        e.arp_log();
        run(&mut e, &s, 2 * ROW_FRAMES);
        let again = e.arp_log();
        assert_eq!(again[..], tail[..again.len()], "seek to row {from}");
    }
}

#[test]
fn stop_and_seek_leave_nothing_hanging() {
    let a = crate::arp::Arp {
        gate: 1.0,
        latch: true,
        ..Default::default()
    };
    let s = chord_song(Some(a), 2);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    run(&mut e, &s, 5 * ROW_FRAMES);
    // Latch: the chord plays on after the notes end
    assert!(e.arp_log().iter().any(|x| x.1 >= 4 * ROW));
    assert!(e.arp_holding(1));
    e.stop();
    assert!(!e.arp_holding(1));
    assert_eq!(e.note_voices(1), 0);
    run(&mut e, &s, ROW_FRAMES);
    assert!(e.arp_log().is_empty(), "stopped: no more steps");
    // a seek past the notes: Latch still holds them (as playing through)
    e.start(&s, 0, 8, false, 0);
    run(&mut e, &s, ROW_FRAMES - 300);
    assert_eq!(keys(&e.arp_log()), [56]);
    e.stop();
    assert_eq!(e.note_voices(1), 0);
    // without Latch a seek past the notes plays nothing
    let s = chord_song(Some(Default::default()), 2);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 4, false, 0);
    run(&mut e, &s, 2 * ROW_FRAMES);
    assert!(e.arp_log().is_empty());
    assert!(!e.arp_holding(1));
}

#[test]
fn a_loop_repeats_the_steps_and_nothing_hangs() {
    let mut s = chord_song(Some(Default::default()), 8);
    set_loop(tl(&mut s), 0, 4 * ROW);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    run(&mut e, &s, 12 * ROW_FRAMES + 10);
    let lg = e.arp_log();
    let k = keys(&lg);
    assert_eq!(
        k,
        [49, 53, 56, 49]
            .repeat(3)
            .into_iter()
            .chain([49])
            .collect::<Vec<_>>(),
        "{lg:?}"
    );
    assert!(e.note_voices(1) <= 1);
    e.stop();
    assert_eq!(e.note_voices(1), 0);
    assert!(!e.arp_holding(1));
}

#[test]
fn the_arp_switched_off_mid_play_stops_stepping() {
    let mut s = chord_song(Some(Default::default()), 8);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    run(&mut e, &s, 2 * ROW_FRAMES);
    assert!(e.arp_holding(1));
    let p = pad(&s) as usize - 1;
    s.instruments[p].arp = None;
    run(&mut e, &s, ROW_FRAMES);
    assert!(!e.arp_holding(1));
    e.arp_log();
    run(&mut e, &s, 2 * ROW_FRAMES);
    assert!(e.arp_log().is_empty());
}

#[test]
fn songs_without_an_arp_render_and_save_as_before() {
    let s = chord_song(None, 8);
    let json = serde_json::to_string(&s.instruments).unwrap();
    assert!(!json.contains("\"arp\""), "no arp key when off");
    let back: Vec<crate::model::Instrument> = serde_json::from_str(&json).unwrap();
    assert!(back.iter().all(|i| i.arp.is_none()));
    let a = render_song(&s, SR);
    let mut s2 = s.clone();
    s2.instruments = back;
    let b = render_song(&s2, SR);
    assert!(a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
    // the Arp on changes the render, and saves
    let s3 = chord_song(Some(Default::default()), 8);
    assert_ne!(render_song(&s3, SR), a);
    let j = serde_json::to_string(&s3.instruments).unwrap();
    assert_eq!(j.matches("\"arp\"").count(), 1);
    let back: Vec<crate::model::Instrument> = serde_json::from_str(&j).unwrap();
    assert_eq!(back, s3.instruments);
}
