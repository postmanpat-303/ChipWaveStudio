//! Record (live into the Timeline), Step input, musical typing and the
//! Song / Master track at the bottom of the track list.
use super::ui_tests::*;
use super::*;
use crate::timeline::{self as tl, PPQ, ROW};
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// render the engine (no audio device in tests) for `rows` more rows;
/// returns the loudest sample
fn play_rows(h: &mut Harness<'static, App>, rows: u64) -> f32 {
    h.state().with(|s| {
        let Shared { song, engine, .. } = s;
        let target = engine.rows_played + rows;
        let mut buf = vec![0.0f32; 128];
        let mut peak = 0.0f32;
        let mut guard = 0;
        while engine.rows_played < target && engine.playing && guard < 200_000 {
            engine.render(song, &mut buf);
            peak = buf.iter().fold(peak, |a, x| a.max(x.abs()));
            guard += 1;
        }
        peak
    })
}

/// render `frames` stereo frames, the loudest sample
fn render_frames(h: &mut Harness<'static, App>, frames: usize) -> f32 {
    h.state().with(|s| {
        let Shared { song, engine, .. } = s;
        let mut buf = vec![0.0f32; 128];
        let mut peak = 0.0f32;
        for _ in 0..frames / 64 {
            engine.render(song, &mut buf);
            peak = buf.iter().fold(peak, |a, x| a.max(x.abs()));
        }
        peak
    })
}

/// as Del runs it (the Tracker hidden): File > New and `n` tracks
fn new_song(n: usize) -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().extra.show_tracker = false;
    h.state_mut().extra.reduce_motion = true;
    frames(&mut h, 2);
    h.state_mut().new_song();
    frames(&mut h, 3);
    for _ in 0..n {
        h.get_by_label("+ Add track").click();
        frames(&mut h, 3);
    }
    h
}

fn note_clips(h: &Harness<'static, App>) -> Vec<(tl::Clip, Vec<tl::Note>)> {
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        t.clips
            .iter()
            .filter_map(|c| match c.kind {
                tl::ClipKind::Notes { data } => {
                    Some((c.clone(), t.notes[data as usize].notes.clone()))
                }
                _ => None,
            })
            .collect()
    })
}

fn tl_tick(h: &Harness<'static, App>) -> Option<f64> {
    h.state().with(|s| s.engine.tl_ticks())
}

#[test]
fn record_off_by_default_and_step_input_is_a_toggle() {
    let x = crate::settings::Extra::default();
    assert!(!x.metronome && x.count_in && !x.rec_quantise);
    let old: crate::settings::Extra = toml::from_str("").unwrap();
    assert!(!old.metronome && old.count_in && !old.rec_quantise);
    let mut h = new_song(1);
    assert!(!h.state().pr.step_input, "Step input is off by default");
    assert!(!h.state().typing.on, "musical typing is off by default");
    assert!(!h.state().recording());
    // Del's case: edit mode on, stopped, a clip in the piano roll - MIDI
    // keys only play; they no longer go in one after the other
    let id = h
        .state()
        .with(|s| tl::new_note_clip(&mut s.song, 0, 0).unwrap());
    {
        let a = h.state_mut();
        a.view = View::Timeline;
        a.tlv.sel = Some(id);
        a.tlv.sels = vec![id];
        a.set_lower(3);
        a.edit = true;
    }
    frames(&mut h, 2);
    let now = std::time::Instant::now;
    for k in [60u8, 62, 64] {
        h.state_mut().midi_note_on(k, k - 11, 0.8, now());
        h.state_mut().midi_note_off(k);
    }
    assert!(note_clips(&h)[0].1.is_empty(), "no step input unless asked");
    // the header's Step input toggle turns it on
    assert!(h.query_by_label("Step input").is_some());
    h.get_by_label("Step input").click();
    frames(&mut h, 2);
    assert!(h.state().pr.step_input);
    for k in [60u8, 62] {
        h.state_mut().midi_note_on(k, k - 11, 0.8, now());
        h.state_mut().midi_note_off(k);
    }
    let n = &note_clips(&h)[0].1;
    assert_eq!(n.len(), 2);
    assert!(n[1].start > n[0].start, "one step after the other");
}

#[test]
fn record_takes_notes_at_their_played_time_length_and_velocity() {
    let mut h = new_song(2);
    h.state_mut().ch = 1;
    h.state_mut().extra.midi.velocity = true;
    frames(&mut h, 2);
    let undo0 = h.state().hist.len();
    // the toolbar's Record button
    assert!(h.query_by_label_contains("Record (R)").is_some());
    key(&mut h, Key::R, egui::Modifiers::NONE);
    assert!(h.state().recording(), "R records");
    assert!(h.state().with(|s| s.engine.playing), "and plays");
    assert!(
        !h.state().with(|s| s.engine.counting_in()),
        "no count-in: metronome off"
    );
    // a bar in, a chord (C E G) held for 4 rows, then D two rows later
    // held over the chord's end (overlapping)
    play_rows(&mut h, 16);
    let t0 = tl_tick(&h).unwrap();
    let now = std::time::Instant::now;
    for (k, v) in [(60u8, 0.5f32), (64, 0.75), (67, 1.0)] {
        h.state_mut().midi_note_on(k, k - 11, v, now());
    }
    play_rows(&mut h, 2);
    h.state_mut().midi_note_on(62, 51, 0.8, now());
    play_rows(&mut h, 2);
    for k in [60u8, 64, 67] {
        h.state_mut().midi_note_off(k);
    }
    play_rows(&mut h, 4);
    h.state_mut().midi_note_off(62);
    frames(&mut h, 2);
    let clips = note_clips(&h);
    assert_eq!(clips.len(), 1, "one take clip");
    let (c, n) = &clips[0];
    assert_eq!(c.track, 1, "on the selected track");
    assert_eq!(c.start, 4 * PPQ, "from the bar the take started in");
    assert_eq!(n.len(), 4);
    let start = t0.round() as tl::Tick - c.start;
    for x in &n[..3] {
        assert!((x.start - start).abs() <= 2, "the chord together: {n:?}");
        assert!((x.len - 4 * ROW).abs() <= 2, "4 rows long: {n:?}");
    }
    assert!((n[3].start - (start + 2 * ROW)).abs() <= 2, "{n:?}");
    assert!((n[3].len - 6 * ROW).abs() <= 2, "overlaps the chord: {n:?}");
    assert!((tl::roll::vel_f(n[0].vel) - 0.5).abs() < 1e-3);
    assert!((tl::roll::vel_f(n[1].vel) - 0.75).abs() < 1e-3);
    // R again: recording stops, playback goes on
    key(&mut h, Key::R, egui::Modifiers::NONE);
    assert!(!h.state().recording());
    assert!(h.state().with(|s| s.engine.playing));
    // the take is one undo step
    frames(&mut h, 3);
    assert_eq!(h.state().hist.len(), undo0 + 1, "one undo step per take");
    h.state().with(|s| s.engine.stop());
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert!(note_clips(&h).is_empty(), "one undo removes the whole take");
}

#[test]
fn record_into_the_clip_under_the_playhead_and_loop_overdubs() {
    let mut h = new_song(1);
    // a clip at bar 2 (4 bars long): recording there goes into it
    let id = h
        .state()
        .with(|s| tl::new_note_clip(&mut s.song, 0, 4 * PPQ).unwrap());
    h.state_mut().timeline_seek(4 * PPQ + 2 * PPQ);
    frames(&mut h, 2);
    key(&mut h, Key::R, egui::Modifiers::NONE);
    play_rows(&mut h, 1);
    h.state_mut()
        .midi_note_on(60, 49, 1.0, std::time::Instant::now());
    play_rows(&mut h, 1);
    h.state_mut().midi_note_off(60);
    key(&mut h, Key::R, egui::Modifiers::NONE);
    h.state().with(|s| s.engine.stop());
    frames(&mut h, 2);
    let clips = note_clips(&h);
    assert_eq!(clips.len(), 1, "no new clip");
    assert_eq!(clips[0].0.id, id);
    assert_eq!(clips[0].1.len(), 1);
    // loop recording: a new take starts on the loop's start and every pass
    // lands in the same clip
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        t.clips.clear();
        t.loop_range = Some((8 * PPQ, 12 * PPQ));
        t.loop_on = true;
    });
    h.state_mut().timeline_seek(8 * PPQ);
    frames(&mut h, 2);
    key(&mut h, Key::R, egui::Modifiers::NONE);
    for pass in 0..2 {
        play_rows(&mut h, 4 + pass);
        h.state_mut().midi_note_on(
            60 + pass as u8,
            49 + pass as u8,
            1.0,
            std::time::Instant::now(),
        );
        play_rows(&mut h, 1);
        h.state_mut().midi_note_off(60 + pass as u8);
        // to the loop's end and round
        play_rows(&mut h, 16 - 5 - pass);
    }
    key(&mut h, Key::R, egui::Modifiers::NONE);
    let clips = note_clips(&h);
    assert_eq!(clips.len(), 1, "both passes in one clip: {:?}", clips);
    assert_eq!(clips[0].0.start, 8 * PPQ);
    assert_eq!(clips[0].1.len(), 2, "overdubbed");
    h.state().with(|s| s.engine.stop());
}

#[test]
fn count_in_waits_a_bar_with_clicks_and_records_nothing_during_it() {
    let mut h = new_song(1);
    h.state_mut().extra.metronome = true;
    frames(&mut h, 2);
    h.state_mut().rec_toggle();
    assert!(h.state().recording());
    assert!(h.state().with(|s| s.engine.counting_in()));
    let t0 = tl_tick(&h).unwrap();
    // half a bar at 120 BPM (one second): clicks, the song waits
    let sr = h.state().with(|s| s.engine.sr) as usize;
    let peak = render_frames(&mut h, sr);
    assert!(peak > 0.05, "count-in clicks: {peak}");
    assert_eq!(tl_tick(&h).unwrap(), t0, "the song waits");
    h.state_mut()
        .midi_note_on(60, 49, 1.0, std::time::Instant::now());
    h.state_mut().midi_note_off(60);
    assert!(
        note_clips(&h).is_empty(),
        "nothing recorded in the count-in"
    );
    render_frames(&mut h, sr + 256);
    assert!(!h.state().with(|s| s.engine.counting_in()));
    assert!(tl_tick(&h).unwrap() > t0, "then it plays");
    // the metronome goes on clicking on the beats
    assert!(play_rows(&mut h, 8) > 0.05);
    h.state().with(|s| s.engine.stop());
    frames(&mut h, 2);
    assert!(!h.state().recording(), "Stop ends the take");
    // metronome off: an empty song is silent
    h.state_mut().extra.metronome = false;
    frames(&mut h, 2);
    h.state_mut().play(0);
    assert_eq!(play_rows(&mut h, 8), 0.0);
    h.state().with(|s| s.engine.stop());
}

#[test]
fn musical_typing_plays_and_records_and_takes_the_letter_keys() {
    let mut h = new_song(1);
    let id = h
        .state()
        .with(|s| tl::new_note_clip(&mut s.song, 0, 16 * PPQ).unwrap());
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    frames(&mut h, 2);
    key(&mut h, Key::K, egui::Modifiers::COMMAND);
    assert!(h.state().typing.on, "Ctrl+K: musical typing on");
    assert!(
        h.query_by_label_contains("Typing C4").is_some(),
        "the toolbar shows it with its octave"
    );
    // S would split the selected clip: now it is a note
    let n0 = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().clips.len());
    key(&mut h, Key::S, egui::Modifiers::NONE);
    assert_eq!(
        h.state()
            .with(|s| s.song.timeline.as_deref().unwrap().clips.len()),
        n0,
        "no shortcut"
    );
    // Record with typed keys: A D G held together (a chord, C4 E4 G4)
    key(&mut h, Key::R, egui::Modifiers::NONE);
    play_rows(&mut h, 4);
    let press = |h: &mut Harness<'static, App>, k: Key, down: bool| {
        h.input_mut().events.push(egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: down,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        frame(h);
    };
    for k in [Key::A, Key::D, Key::G] {
        press(&mut h, k, true);
    }
    assert_eq!(h.state().midi.held.len(), 3, "the MIDI way");
    play_rows(&mut h, 4);
    for k in [Key::A, Key::D, Key::G] {
        press(&mut h, k, false);
    }
    assert!(h.state().midi.held.is_empty());
    // X: an octave up, then W (C#5)
    key(&mut h, Key::X, egui::Modifiers::NONE);
    assert_eq!(h.state().typing.octave, 5);
    press(&mut h, Key::W, true);
    play_rows(&mut h, 2);
    press(&mut h, Key::W, false);
    key(&mut h, Key::R, egui::Modifiers::NONE);
    h.state().with(|s| s.engine.stop());
    frames(&mut h, 2);
    let clips = note_clips(&h);
    let take = clips.iter().find(|(c, _)| c.id != id).expect("a take clip");
    let mut keys: Vec<u8> = take.1.iter().map(|n| n.key).collect();
    keys.sort();
    // C4 = MIDI 60 = tracker note 49
    assert_eq!(keys, vec![49, 53, 56, 62]);
    let s0 = take.1.iter().find(|n| n.key == 49).unwrap().start;
    assert!(
        take.1.iter().filter(|n| n.key < 60).all(|n| n.start == s0),
        "a chord"
    );
    // C / V: velocity
    key(&mut h, Key::C, egui::Modifiers::NONE);
    assert!((h.state().typing.vel - 0.7).abs() < 1e-4);
    // Ctrl+K again: off, and S splits again
    key(&mut h, Key::K, egui::Modifiers::COMMAND);
    assert!(!h.state().typing.on);
    assert!(super::typing::typed_note(4, 0) == Some(49));
    assert!(super::typing::typed_note(-2, 0).is_none());
}

#[test]
fn song_master_sits_at_the_bottom_after_add_track() {
    let mut h = new_song(2);
    frames(&mut h, 2);
    let (n, master_last) = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        (
            t.tracks.len(),
            t.tracks.last().unwrap().kind == tl::TrackKind::Master,
        )
    });
    assert_eq!(n, 3);
    assert!(master_last, "the song file keeps Master last");
    let lay = h.state().tlv.layout.clone();
    assert_eq!(lay.master, Some(2));
    let (h1, _) = h.state().tlv.track_rects(1).unwrap();
    let (hm, rowm) = h.state().tlv.track_rects(2).unwrap();
    let add = exact(&h, "+ Add track");
    assert!(add.top() >= h1.bottom(), "+ Add track below the tracks");
    assert!(hm.top() >= add.bottom(), "Master below + Add track");
    assert!(
        rowm.right() > hm.right(),
        "the Master row runs across the lanes, aligned"
    );
    assert!(lay.in_gap(lay.top[2] - 10.0));
    assert!(
        lay.part_at(lay.top[2] - 10.0).is_none(),
        "the gap is no track"
    );
    // a new track goes above Master
    h.get_by_label("+ Add track").click();
    frames(&mut h, 3);
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        assert_eq!(t.tracks.len(), 4);
        assert_eq!(t.tracks[3].kind, tl::TrackKind::Master);
    });
    assert_eq!(h.state().tlv.layout.master, Some(3));
}
