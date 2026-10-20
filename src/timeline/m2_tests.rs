//! Timeline mode (M2): the engine playing clips, seeks, the loop region,
//! per-track speed, and the clip edits (each checked by ear: renders).
use super::*;
use crate::engine::{render_song, Engine};
use crate::model::{Cell, Pattern, Song};

const SR: u32 = 22050;
/// 125 BPM at 22 050 Hz: a row is exactly 2 646 frames
const ROW_FRAMES: usize = 2646;

fn note(n: u8, instr: u8) -> Cell {
    Cell {
        note: n,
        instr,
        ..Default::default()
    }
}

/// 2 channels at 125 BPM / speed 6; block 0 has a note on row 0 (ch 0) and
/// row 8 (ch 1); block 1 a note on row 0 of ch 0
fn song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.bpm = 125.0;
    s.speed = 6;
    s.channels = 2;
    s.channel_names = vec!["A".into(), "B".into()];
    let mut a = Pattern::new("A", 16, 2);
    a.cells[0][0] = note(49, 1);
    a.cells[8][1] = note(56, 1);
    let mut b = Pattern::new("B", 16, 2);
    b.cells[0][0] = note(61, 1);
    s.patterns = vec![a, b];
    s.order = vec![0, 1];
    s.mixer.channels.resize(2, Default::default());
    enter_timeline_mode(&mut s);
    s
}

fn tl(s: &mut Song) -> &mut Timeline {
    s.timeline.as_deref_mut().unwrap()
}

/// first stereo frame that is not silent
fn onset(a: &[f32]) -> usize {
    a.iter().position(|x| x.abs() > 1e-6).unwrap() / 2
}

fn end_to(s: &mut Song, id: u32, at: Tick) {
    let mut t = s.timeline.take().unwrap();
    set_end(s, &mut t, id, at);
    s.timeline = Some(t);
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn a_moved_clip_plays_exactly_one_bar_later() {
    let mut s = song();
    let a0 = render_song(&s, SR);
    let ids: Vec<u32> = tl(&mut s).clips.iter().map(|c| c.id).collect();
    move_clips(tl(&mut s), &ids, 16 * ROW, 0, 2);
    let a1 = render_song(&s, SR);
    assert_eq!(onset(&a1), onset(&a0) + 16 * ROW_FRAMES);
    // and the same audio, shifted
    let k = 16 * ROW_FRAMES * 2;
    assert!(same(&a1[k..k + 44100], &a0[..44100]));
}

#[test]
fn split_duplicate_and_glue_play_the_same() {
    let base = song();
    let want = render_song(&base, SR);
    // split every clip in the middle: nothing changes
    let mut s = base.clone();
    let ids: Vec<(u32, Tick)> = tl(&mut s)
        .clips
        .iter()
        .map(|c| (c.id, c.start + c.len / 2))
        .collect();
    for (id, at) in ids {
        assert!(split(tl(&mut s), id, at).is_some());
    }
    assert!(same(&render_song(&s, SR), &want), "split");
    // glue the halves on track 0 back into one block: still the same
    let t0: Vec<u32> = tl(&mut s)
        .clips
        .iter()
        .filter(|c| c.track == 0)
        .map(|c| c.id)
        .collect();
    let n = s.patterns.len();
    let id = glue(&mut s, &t0).unwrap();
    assert_eq!(s.patterns.len(), n + 1);
    assert_eq!(tl(&mut s).clips.iter().filter(|c| c.track == 0).count(), 1);
    assert!(tl(&mut s).clips.iter().any(|c| c.id == id));
    assert!(same(&render_song(&s, SR), &want), "glue");
    // make unique: same sound, and editing the copy leaves the original
    let mut u = base.clone();
    let first = tl(&mut u).clips[0].id;
    let b = make_unique(&mut u, first).unwrap();
    assert_eq!(u.patterns[b].name, "A copy");
    assert!(same(&render_song(&u, SR), &want), "make unique");
    u.patterns[b].cells[0][0] = note(25, 1);
    assert_eq!(u.patterns[0].cells[0][0].note, 49);
    // duplicate: copies right after, linked to the same block
    let mut d = base.clone();
    let all: Vec<u32> = tl(&mut d).clips.iter().map(|c| c.id).collect();
    let end = tl(&mut d).clips.iter().map(|c| c.end()).max().unwrap();
    let new = duplicate(tl(&mut d), &all);
    assert_eq!(new.len(), all.len());
    let first_copy = tl(&mut d)
        .clips
        .iter()
        .filter(|c| new.contains(&c.id))
        .map(|c| c.start)
        .min();
    assert_eq!(first_copy, Some(end));
}

#[test]
fn resizing_past_the_block_loops_it_and_trimming_skips_rows() {
    let mut s = song();
    let id = tl(&mut s)
        .clips
        .iter()
        .find(|c| c.track == 0 && c.start == 0)
        .unwrap()
        .id;
    // the right edge to 3 blocks: the source loops every 16 rows
    let tm = s.timeline.clone().unwrap();
    end_to(&mut s, id, 48 * ROW);
    {
        let c = tl(&mut s)
            .clips
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .clone();
        assert_eq!((c.len, c.loop_len), (48 * ROW, Some(16 * ROW)));
    }
    // (the later clips on track 0 win where they overlap; drop them)
    tl(&mut s).clips.retain(|c| c.track != 0 || c.id == id);
    let a = render_song(&s, SR);
    // the note on row 0 comes back on rows 16 and 32
    let frame = |row: usize| row * ROW_FRAMES * 2;
    let pass = 4 * ROW_FRAMES * 2;
    assert!(same(
        &a[frame(16)..frame(16) + pass],
        &a[frame(32)..frame(32) + pass]
    ));
    // trim: the left edge to row 4 skips the first note on track 0
    s.timeline = Some(tm);
    set_start(tl(&mut s), id, 4 * ROW);
    let c = tl(&mut s)
        .clips
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .clone();
    assert_eq!((c.start, c.offset, c.len), (4 * ROW, 4 * ROW, 12 * ROW));
    // its rows 4..15 are empty: the same as no clip there
    let b = render_song(&s, SR);
    delete(tl(&mut s), &[id]);
    assert!(same(&b, &render_song(&s, SR)));
}

#[test]
fn muted_and_deleted_clips_are_silent() {
    let mut s = song();
    let ids: Vec<u32> = tl(&mut s).clips.iter().map(|c| c.id).collect();
    toggle_mute(tl(&mut s), &ids);
    assert!(tl(&mut s).clips.iter().all(|c| c.muted));
    let a = render_song(&s, SR);
    assert!(a.iter().all(|x| x.abs() < 1e-6), "muted");
    toggle_mute(tl(&mut s), &ids);
    delete(tl(&mut s), &ids);
    assert!(tl(&mut s).clips.is_empty());
}

#[test]
fn the_loop_region_wraps_and_the_song_end_is_the_last_clip() {
    let mut s = song();
    set_loop(tl(&mut s), 4 * ROW, 8 * ROW);
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 4, false, 0);
    let mut buf = vec![0.0f32; 512];
    let mut rows = std::collections::BTreeSet::new();
    for _ in 0..(SR as usize * 2 / 256) {
        e.render(&s, &mut buf);
        rows.insert(e.tl_row().unwrap());
    }
    assert_eq!(rows.into_iter().collect::<Vec<_>>(), vec![4, 5, 6, 7]);
    assert!(!e.song_ended);
    // the loop off: plays to the end of the last clip (32 rows) and stops
    tl(&mut s).loop_on = false;
    assert_eq!(play_end(s.timeline.as_deref().unwrap()), 32 * ROW);
    let secs = s.length_seconds();
    assert!((secs - 32.0 * 0.12).abs() < 1e-6, "{secs}");
}

#[test]
fn a_seek_chases_tempo_and_held_notes() {
    let mut s = song();
    // a tempo change on row 2 of block 0 (0F96 = 150 BPM)
    s.patterns[0].cells[2][1] = Cell {
        fx: 0x0F,
        fxp: 150,
        ..Default::default()
    };
    // a sustaining instrument for the held note
    let k = s
        .instruments
        .iter()
        .position(|i| i.kind.patch().is_some_and(|p| p.amp.s >= 0.5))
        .expect("a sustaining patch in the demo");
    s.patterns[0].cells[8][1].instr = k as u8 + 1;
    let mut e = Engine::new(SR as f32);
    e.start(&s, 0, 12, false, 0);
    assert_eq!(e.bpm(), 150.0, "tempo before the start point is chased");
    assert_eq!(e.tl_row(), Some(12));
    // seek back before the change: the song tempo again
    e.seek(&s, 0, 1);
    assert_eq!(e.bpm(), 125.0);
    // a held note (track 1, row 8) sounds right after a seek to row 10
    e.seek(&s, 0, 10);
    let mut buf = vec![0.0f32; 1024];
    e.render(&s, &mut buf);
    assert!(buf.iter().any(|x| x.abs() > 1e-4), "held note re-struck");
}

#[test]
fn per_track_speed_changes_effect_steps_not_timing() {
    let mut s = song();
    // a volume slide down under the note on track 0
    s.patterns[0].cells[0][0].fx = 0x0D;
    s.patterns[0].cells[0][0].fxp = 0x0F;
    let a = render_song(&s, SR);
    tl(&mut s).tracks[0].speed = 3;
    let b = render_song(&s, SR);
    assert_eq!(a.len(), b.len(), "same timing");
    assert!(!same(&a, &b), "fewer slide steps per row");
    // track 1 (no effects) is unchanged: compare its note's onset
    assert_eq!(onset(&a), onset(&b));
}

#[test]
fn tracker_copy_plays_like_the_edited_timeline() {
    let mut s = song();
    let ids: Vec<u32> = tl(&mut s).clips.iter().map(|c| c.id).collect();
    let id = ids[0];
    end_to(&mut s, id, 40 * ROW);
    split(tl(&mut s), id, 20 * ROW);
    move_clips(tl(&mut s), &[ids[1]], 3 * ROW, 0, 2);
    let a = render_song(&s, SR);
    let c = tracker_copy(&s).unwrap();
    assert!(c.timeline.is_none());
    let b = render_song(&c, SR);
    let first = a
        .iter()
        .zip(&b)
        .position(|(x, y)| x.to_bits() != y.to_bits());
    assert_eq!(
        (a.len(), first.map(|f| f / 2 / ROW_FRAMES)),
        (b.len(), None),
        "{:?} {:?}",
        s.timeline.as_ref().unwrap().clips,
        c.order
    );
    // and it saves as a reader-version-1 song
    let dir = std::env::temp_dir().join(format!("cw_tlcopy_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("copy.cwt");
    crate::fileio::save_cwt(&c, &p).unwrap();
    let back = crate::fileio::load_cwt(&p).unwrap();
    assert!(back.timeline.is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_loop_scope_exports_the_loop_region_once() {
    let mut s = song();
    set_loop(tl(&mut s), 8 * ROW, 24 * ROW);
    let o = crate::export::Opts {
        scope: crate::export::Scope::Loop,
        sample_rate: SR,
        tail_secs: 0.0,
        normalise: false,
        limiter: false,
        ..Default::default()
    };
    let sc = crate::export::scoped(&s, o.scope);
    assert!(!sc.timeline.as_ref().unwrap().loop_on, "exports never loop");
    assert_eq!(crate::export::start_row(&s, o.scope), 8);
    assert_eq!(play_end(sc.timeline.as_deref().unwrap()), 24 * ROW);
}

// ---- placing clips (unified plan Stage 4a)

fn note_song() -> (Song, u32) {
    let mut s = crate::fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let id = new_note_clip(&mut s, 1, 4 * PPQ * 4).unwrap();
    let tl = s.timeline.as_deref_mut().unwrap();
    let i = tl.clips.iter().position(|c| c.id == id).unwrap();
    tl.clips[i].len = 4 * PPQ * 4;
    if let ClipKind::Notes { data } = tl.clips[i].kind {
        tl.notes[data as usize].make_mut().notes.push(Note {
            start: 0,
            len: PPQ,
            key: 49,
            ..Default::default()
        });
    }
    (s, id)
}

#[test]
fn overlaps_find_the_clips_a_placement_would_cover() {
    let (s, id) = note_song();
    let tl = s.timeline.as_deref().unwrap();
    let bar = 4 * PPQ;
    // the note clip covers bars 5-8 on track 1 (index 1)
    assert!(overlaps(tl, 1, 0, 4 * bar, &[]).iter().all(|&x| x != id));
    assert!(overlaps(tl, 1, 7 * bar, 9 * bar, &[]).contains(&id));
    assert!(!overlaps(tl, 1, 7 * bar, 9 * bar, &[id]).contains(&id));
    assert!(
        !overlaps(tl, 1, 8 * bar, 9 * bar, &[]).contains(&id),
        "touching is not overlapping"
    );
}

#[test]
fn clear_range_keeps_the_parts_outside() {
    let (mut s, id) = note_song();
    let bar = 4 * PPQ;
    let tl = s.timeline.as_deref_mut().unwrap();
    let others: Vec<u32> = overlaps(tl, 1, 5 * bar, 6 * bar, &[id]);
    tl.clips.retain(|c| !(others.contains(&c.id)));
    clear_range(tl, 1, 5 * bar, 6 * bar);
    assert!(overlaps(tl, 1, 5 * bar, 6 * bar, &[]).is_empty());
    let mut parts: Vec<(Tick, Tick, Tick)> = tl
        .clips
        .iter()
        .filter(|c| c.track == 1 && c.start >= 4 * bar && c.notes().is_some())
        .map(|c| (c.start, c.end(), c.offset))
        .collect();
    parts.sort();
    assert_eq!(
        parts,
        vec![(4 * bar, 5 * bar, 0), (6 * bar, 8 * bar, 2 * bar)]
    );
}

#[test]
fn free_start_moves_past_the_clips_in_the_way() {
    let (s, _) = note_song();
    let bar = 4 * PPQ;
    let tl = s.timeline.as_deref().unwrap();
    let at = free_start(tl, 1, 5 * bar, 2 * bar, bar);
    assert!(at >= 8 * bar, "{at}");
    assert!(overlaps(tl, 1, at, at + 2 * bar, &[]).is_empty());
    assert_eq!(at % bar, 0);
}

#[test]
fn place_copy_links_or_copies_the_notes() {
    let (mut s, id) = note_song();
    let bar = 4 * PPQ;
    let linked = place_copy(&mut s, id, 2, 20 * bar, false).unwrap();
    let own = place_copy(&mut s, id, 2, 30 * bar, true).unwrap();
    let tl = s.timeline.as_deref().unwrap();
    let get = |k: u32| tl.clips.iter().find(|c| c.id == k).unwrap().clone();
    let (a, b, c) = (get(id), get(linked), get(own));
    assert_eq!(b.notes(), a.notes(), "linked: the same notes");
    assert_ne!(c.notes(), a.notes(), "independent: its own notes");
    let d = |x: &Clip| tl.notes[x.notes().unwrap()].notes.clone();
    assert_eq!(d(&c), d(&a), "with the same content");
    for x in [&b, &c] {
        assert_eq!((x.len, x.offset, x.loop_len), (a.len, a.offset, a.loop_len));
        assert_eq!(x.track, 2);
    }
    assert_eq!((b.start, c.start), (20 * bar, 30 * bar));
    // past the last track: nothing
    let n = s.channels;
    assert!(place_copy(&mut s, id, n, 0, false).is_none());
}
