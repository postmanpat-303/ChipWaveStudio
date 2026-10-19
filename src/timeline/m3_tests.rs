//! Note clips (M3b): spans in the plan, polyphony and voice stealing,
//! sample-accurate timing, seeks, loops and chance, and block-only songs
//! staying bit-exact.
use super::*;
use crate::engine::{render_song, Engine};
use crate::model::{Cell, Pattern, Song};

const SR: u32 = 22050;
/// 125 BPM at 22 050 Hz: a row is exactly 2 646 frames
const ROW_FRAMES: usize = 2646;

/// 2 channels, block 0 with one note on track 0 row 0, in timeline mode
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

/// a sustaining (pad-like) instrument of the demo, 1-based
fn pad(s: &Song) -> u8 {
    s.instruments
        .iter()
        .position(|i| i.kind.patch().is_some_and(|p| p.amp.s >= 0.5))
        .expect("a sustaining patch in the demo") as u8
        + 1
}

/// a note clip on track 1 at `start` with `notes`; returns its id
fn note_clip(s: &mut Song, start: Tick, notes: Vec<Note>) -> u32 {
    let id = new_note_clip(s, 1, start).unwrap();
    let t = tl(s);
    let c = t.clips.iter().find(|c| c.id == id).unwrap();
    let ClipKind::Notes { data } = c.kind else {
        unreachable!()
    };
    t.notes[data as usize].make_mut().notes = notes;
    id
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

fn onset(a: &[f32]) -> usize {
    a.iter().position(|x| x.abs() > 1e-6).unwrap() / 2
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

#[test]
fn block_timelines_play_bit_exact_with_empty_or_muted_note_clips() {
    let s0 = song();
    let a0 = render_song(&s0, SR);
    let mut s = s0.clone();
    let id = note_clip(&mut s, 0, vec![n(0, 4 * ROW, 61, 1)]);
    toggle_mute(tl(&mut s), &[id]);
    assert!(build_plan(&s, s.timeline.as_deref().unwrap())
        .notes
        .iter()
        .all(|v| v.is_empty()));
    assert!(
        same(&render_song(&s, SR), &a0),
        "a muted note clip changes nothing"
    );
    let mut s = s0.clone();
    let id = note_clip(&mut s, 0, vec![]);
    tl(&mut s)
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .len = 16 * ROW;
    assert!(
        same(&render_song(&s, SR), &a0),
        "an empty note clip changes nothing"
    );
}

#[test]
fn a_chord_plays_three_voices_and_releases_them() {
    let mut s = song();
    let p = pad(&s);
    note_clip(
        &mut s,
        0,
        vec![
            n(0, 4 * ROW, 49, p),
            n(0, 4 * ROW, 53, p),
            n(0, 4 * ROW, 56, p),
        ],
    );
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    let a = run(&mut e, &s, ROW_FRAMES);
    assert_eq!(e.note_voices(1), 3);
    assert!(a.iter().any(|x| x.abs() > 1e-3));
    run(&mut e, &s, 4 * ROW_FRAMES);
    assert_eq!(e.note_voices(1), 0, "note-offs at the end of the notes");
    // edits while playing take effect (the plan key follows the notes)
    let k0 = plan_key(&s, s.timeline.as_deref().unwrap());
    let t = tl(&mut s);
    t.notes[0].make_mut().notes[0].key = 50;
    assert_ne!(k0, plan_key(&s, s.timeline.as_deref().unwrap()));
}

#[test]
fn more_notes_than_voices_steal_the_oldest() {
    let mut s = song();
    let p = pad(&s);
    // ten notes, one per 1/32, all held for two bars
    let notes = (0..10)
        .map(|k| n(k * ROW / 2, 32 * ROW, 37 + k as u8, p))
        .collect();
    note_clip(&mut s, 0, notes);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 0, false, 0);
    run(&mut e, &s, 6 * ROW_FRAMES);
    assert_eq!(
        e.note_voices(1),
        crate::engine::NOTE_VOICES,
        "capped at the pool size"
    );
    let out = run(&mut e, &s, ROW_FRAMES);
    assert!(out.iter().all(|x| x.is_finite()));
}

#[test]
fn notes_start_at_sample_accuracy_off_the_row_grid() {
    let mut s = song();
    tl(&mut s).clips.retain(|c| c.block().is_none());
    note_clip(&mut s, 0, vec![n(0, ROW, 49, 1)]);
    let a0 = onset(&render_song(&s, SR));
    let mut s2 = song();
    tl(&mut s2).clips.retain(|c| c.block().is_none());
    note_clip(&mut s2, 0, vec![n(ROW / 2, ROW, 49, 1)]);
    let a1 = onset(&render_song(&s2, SR));
    let d = a1 as i64 - a0 as i64;
    assert!(
        (d - ROW_FRAMES as i64 / 2).abs() <= 1,
        "half a row later: {d} frames"
    );
    // a third of a beat (triplet)
    let mut s3 = song();
    tl(&mut s3).clips.retain(|c| c.block().is_none());
    note_clip(&mut s3, 0, vec![n(PPQ / 3, ROW, 49, 1)]);
    let d = onset(&render_song(&s3, SR)) as i64 - a0 as i64;
    assert!(
        (d - (4 * ROW_FRAMES / 3) as i64).abs() <= 1,
        "1/3 beat: {d} frames"
    );
}

#[test]
fn a_seek_restrikes_held_notes_of_sustaining_instruments() {
    let mut s = song();
    let p = pad(&s);
    note_clip(
        &mut s,
        0,
        vec![n(0, 16 * ROW, 49, p), n(8 * ROW, ROW, 61, p)],
    );
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 6, false, 0);
    assert_eq!(e.note_voices(1), 1, "the held note sounds from row 6");
    run(&mut e, &s, 5 * ROW_FRAMES / 2);
    assert_eq!(e.note_voices(1), 2, "and the next note starts on row 8");
    e.seek(&s, 0, 12);
    assert_eq!(
        e.note_voices(1),
        1,
        "seek: the long note again, not the short one"
    );
    e.stop();
    assert_eq!(e.note_voices(1), 0);
}

#[test]
fn looped_clips_repeat_their_notes_and_chance_is_repeatable() {
    let mut s = song();
    let id = note_clip(
        &mut s,
        0,
        vec![
            n(0, ROW, 49, 1),
            Note {
                chance: 50,
                ..n(2 * ROW, ROW, 52, 1)
            },
        ],
    );
    {
        let t = tl(&mut s);
        let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
        c.loop_len = Some(4 * ROW);
        c.len = 400 * ROW;
    }
    let t = s.timeline.as_deref().unwrap();
    let a = note_spans(t, 2);
    let b = note_spans(t, 2);
    assert_eq!(a, b, "the same every time (ClipRng per pass)");
    let first = a[1].iter().filter(|x| x.key == 49).count();
    let maybe = a[1].iter().filter(|x| x.key == 52).count();
    assert_eq!(first, 100, "one per pass");
    assert!(
        (30..=70).contains(&maybe),
        "about half of the 50 % notes: {maybe}"
    );
    assert!(a[1].windows(2).all(|w| w[0].start <= w[1].start), "sorted");
    let ones: Vec<Tick> = a[1]
        .iter()
        .filter(|x| x.key == 49)
        .map(|x| x.start)
        .collect();
    assert!(
        ones.windows(2).all(|w| w[1] - w[0] == 4 * ROW),
        "every 4 rows"
    );
    // a left trim skips the notes before it; the clip end cuts notes
    let mut s = song();
    let id = note_clip(
        &mut s,
        0,
        vec![n(0, ROW, 49, 1), n(4 * ROW, 8 * ROW, 52, 1)],
    );
    {
        let t = tl(&mut s);
        let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
        c.offset = 2 * ROW;
        c.start = 2 * ROW;
        c.len = 6 * ROW;
    }
    let sp = note_spans(s.timeline.as_deref().unwrap(), 2);
    assert_eq!(sp[1].len(), 1);
    assert_eq!((sp[1][0].start, sp[1][0].end), (4 * ROW, 8 * ROW));
}

#[test]
fn new_and_unique_note_clips() {
    let mut s = song();
    let id = note_clip(&mut s, 0, vec![n(0, ROW, 49, 1)]);
    let t = tl(&mut s);
    let c = t.clips.iter().find(|c| c.id == id).unwrap().clone();
    assert_eq!((c.track, c.len), (1, 16 * PPQ));
    // a copy shares the notes until made unique
    let d = make_unique_notes(t, id).unwrap();
    assert_eq!(t.notes.len(), 2);
    assert_eq!(t.notes[d].notes, t.notes[0].notes);
    t.notes[d].make_mut().notes.clear();
    assert_eq!(t.notes[0].notes.len(), 1, "the original keeps its notes");
    // and it saves and loads
    let t = s.timeline.as_deref().unwrap();
    let back = from_toml(&to_toml(t).unwrap()).unwrap();
    assert_eq!(&back, t);
}
