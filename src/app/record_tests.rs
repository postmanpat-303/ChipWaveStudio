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
    assert!(!x.metronome && x.count_in_bars() == 1 && !x.rec_quantise);
    let old: crate::settings::Extra = toml::from_str("").unwrap();
    assert!(!old.metronome && old.count_in_bars() == 1 && !old.rec_quantise);
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
    h.state_mut().extra.set_count_in_bars(0);
    frames(&mut h, 2);
    let undo0 = h.state().hist.len();
    // the toolbar's Record button
    assert!(h.query_by_label_contains("Record (R)").is_some());
    key(&mut h, Key::R, egui::Modifiers::NONE);
    assert!(h.state().recording(), "R records");
    assert!(h.state().with(|s| s.engine.playing), "and plays");
    assert!(
        !h.state().with(|s| s.engine.counting_in()),
        "no count-in: set to Off"
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
fn count_in_clicks_even_with_the_metronome_off() {
    // the default: one bar, metronome off
    let mut h = new_song(1);
    assert!(!h.state().extra.metronome);
    assert_eq!(h.state().extra.count_in_bars(), 1);
    frames(&mut h, 2);
    key(&mut h, Key::R, egui::Modifiers::NONE);
    assert!(h.state().recording());
    assert!(h.state().with(|s| s.engine.counting_in()), "counts in");
    let t0 = tl_tick(&h).unwrap();
    let sr = h.state().with(|s| s.engine.sr) as usize;
    // half the bar: clicks although the metronome is off; the song waits
    let peak = render_frames(&mut h, sr);
    assert!(
        peak > 0.05,
        "count-in clicks with the metronome off: {peak}"
    );
    assert_eq!(tl_tick(&h).unwrap(), t0, "the song waits");
    render_frames(&mut h, sr + 256);
    assert!(!h.state().with(|s| s.engine.counting_in()));
    assert!(tl_tick(&h).unwrap() > t0, "then it records");
    // and with the metronome off, no clicks after it (an empty song)
    assert_eq!(play_rows(&mut h, 8), 0.0);
    h.state().with(|s| s.engine.stop());
}

#[test]
fn count_in_of_two_bars_counts_eight_beats_down() {
    let mut h = new_song(1);
    h.state_mut().extra.set_count_in_bars(2);
    frames(&mut h, 2);
    assert_eq!(
        h.query_all_by_label("Metronome").count(),
        1,
        "the toolbar's Metronome toggle"
    );
    assert_eq!(h.query_all_by_label("Click").count(), 0);
    h.state_mut().rec_toggle();
    assert_eq!(h.state().with(|s| s.engine.count_in_left()), Some(8));
    frames(&mut h, 2);
    assert!(
        h.query_by_label_contains("Count-in ").is_some(),
        "the toolbar counts down"
    );
    let t0 = tl_tick(&h).unwrap();
    let sr = h.state().with(|s| s.engine.sr) as usize;
    // one bar (2 s at 120 BPM): still counting in, four beats left
    render_frames(&mut h, 2 * sr);
    assert_eq!(h.state().with(|s| s.engine.count_in_left()), Some(4));
    assert_eq!(tl_tick(&h).unwrap(), t0, "the song waits two bars");
    render_frames(&mut h, 2 * sr + 256);
    assert_eq!(h.state().with(|s| s.engine.count_in_left()), None);
    assert!(tl_tick(&h).unwrap() > t0, "then it records");
    h.state().with(|s| s.engine.stop());
}

#[test]
fn count_in_off_records_at_once_even_with_the_metronome_on() {
    let mut h = new_song(1);
    h.state_mut().extra.metronome = true;
    h.state_mut().extra.set_count_in_bars(0);
    frames(&mut h, 2);
    h.state_mut().rec_toggle();
    assert!(h.state().recording());
    assert!(!h.state().with(|s| s.engine.counting_in()), "no count-in");
    let t0 = tl_tick(&h).unwrap();
    play_rows(&mut h, 4);
    assert!(tl_tick(&h).unwrap() > t0, "records at once");
    h.state().with(|s| s.engine.stop());
}

#[test]
fn old_count_in_settings_carry_over_as_bars() {
    use crate::settings::Extra;
    let on: Extra = toml::from_str("count_in = true").unwrap();
    assert_eq!(on.count_in_bars(), 1, "count_in = true: one bar");
    let off: Extra = toml::from_str("count_in = false").unwrap();
    assert_eq!(off.count_in_bars(), 0, "count_in = false: Off");
    let two: Extra = toml::from_str("count_in = true\ncount_in_bars = 2").unwrap();
    assert_eq!(two.count_in_bars(), 2);
    // set: the old flag follows (older versions read it), and it saves
    let mut x = Extra::default();
    x.set_count_in_bars(0);
    assert!(!x.count_in);
    x.set_count_in_bars(2);
    assert!(x.count_in);
    let back: Extra = toml::from_str(&toml::to_string(&x).unwrap()).unwrap();
    assert_eq!(back.count_in_bars(), 2);
    assert_eq!(
        Extra {
            count_in_bars: Some(9),
            ..x
        }
        .count_in_bars(),
        2
    );
}

#[test]
fn musical_typing_plays_and_records_and_takes_the_letter_keys() {
    let mut h = new_song(1);
    // the track plays an instrument (an empty track is silent)
    h.state().with(|s| {
        let mut one = crate::fileio::demo_song().instruments[0].clone();
        one.name = "Init".into();
        one.kind = InstrumentKind::Synth(Default::default());
        s.song.instruments = vec![one];
        s.song.timeline.as_deref_mut().unwrap().tracks[0].instr = 1;
    });
    let id = h
        .state()
        .with(|s| tl::new_note_clip(&mut s.song, 0, 16 * PPQ).unwrap());
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    frames(&mut h, 2);
    assert_eq!(
        h.query_all_by_label("Keys → Notes").count(),
        2,
        "the toolbar toggle and the piano roll's are called Keys → Notes"
    );
    assert_eq!(
        h.query_all_by_label_contains("Typing").count(),
        0,
        "no old Typing label"
    );
    key(&mut h, Key::K, egui::Modifiers::COMMAND);
    assert!(h.state().typing.on, "Ctrl+K: musical typing on");
    assert_eq!(
        h.query_all_by_label("Keys → Notes").count(),
        2,
        "the toolbar's is an icon button, still named Keys → Notes"
    );
    assert!(
        super::topbar::keys_tip(true, &h.state().typing_label())
            .0
            .contains("C4 · vel 102"),
        "its hover shows the octave and velocity"
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
    assert_eq!(
        h.state().extra.keys_notes,
        Some(false),
        "the choice is kept"
    );
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

/// Del's drawing: Song / Master docked as a footer at the bottom of the
/// track list (always in view), the mixer tracks above it, the empty
/// space between them free (display only: the song keeps Master last)
#[test]
fn song_master_is_docked_at_the_bottom_of_the_track_list() {
    let mut h = new_song(2);
    frames(&mut h, 2);
    let heads = h.state().tlv.heads;
    let lay = h.state().tlv.layout.clone();
    assert!(lay.docked);
    let m = lay.master.unwrap();
    let (hm, rowm) = h.state().tlv.track_rects(m).unwrap();
    assert!(
        heads.bottom() - hm.bottom() < 4.0,
        "Master's header at the panel's bottom: {hm:?} in {heads:?}"
    );
    assert!(rowm.right() > hm.right(), "its lane lines up with it");
    let (h1, _) = h.state().tlv.track_rects(1).unwrap();
    let mid = (h1.bottom() + hm.top()) / 2.0;
    assert!(hm.top() - h1.bottom() > 100.0, "room between them");
    let y = mid - heads.top() + h.state().tlv.scroll_y;
    assert!(
        lay.in_gap(y) && lay.part_at(y).is_none(),
        "the gap is no track"
    );
    // many tracks: the list scrolls, Master stays put
    for _ in 0..14 {
        h.get_by_label("+ Add track").click();
        frames(&mut h, 2);
        if h.query_by_label("+ Add track").is_none() {
            break;
        }
    }
    h.state_mut().tlv.scroll_y = 0.0;
    frames(&mut h, 2);
    let lay = h.state().tlv.layout.clone();
    let m = lay.master.unwrap();
    let (hm0, _) = h.state().tlv.track_rects(m).unwrap();
    h.state_mut().tlv.scroll_y = 1.0e6;
    frames(&mut h, 2);
    let (hm1, _) = h.state().tlv.track_rects(m).unwrap();
    assert!(h.state().tlv.scroll_y > 0.0, "the tracks scroll");
    assert!((hm0.top() - hm1.top()).abs() < 0.5, "Master stays docked");
    // at the end of the list the last track sits above the footer
    let (hl, _) = h.state().tlv.track_rects(m - 1).unwrap();
    assert!(hl.bottom() <= hm1.top(), "{hl:?} above {hm1:?}");
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        assert_eq!(t.tracks.last().unwrap().kind, tl::TrackKind::Master);
    });
}

// ------------------------------------------- WaveForge on a track (Del's report)

/// Del's report: a track given instrument 2 (WaveForge Supersaw Pad), a
/// note clip recorded live, a cutoff lane at 1.8 kHz. Live MIDI sounded
/// right, playback sounded like an Init patch. Checks what the recorded
/// notes play, that the lane drives instrument 2's cutoff knob, and that
/// playing (with the editor open) leaves the patch alone.
#[test]
fn waveforge_track_records_and_plays_its_own_instrument() {
    use crate::automation::{Lane, Point, Target};
    use crate::model::InstrumentKind;
    use crate::poly::{CUTOFF, POLY};
    let mut h = new_song(1);
    let pad = crate::gen::preset(concat!(crate::poly_name!(), " Supersaw Pad")).unwrap();
    h.state().with(|s| {
        // 01 a plain synth (Init), 02 the WaveForge pad
        let mut one = crate::fileio::demo_song().instruments[0].clone();
        one.name = "Init".into();
        one.kind = InstrumentKind::Synth(Default::default());
        let mut two = one.clone();
        two.name = concat!(crate::poly_name!(), " Supersaw Pad").into();
        two.kind = InstrumentKind::Gen(pad.clone());
        s.song.instruments = vec![one, two];
        s.song.timeline.as_deref_mut().unwrap().tracks[0].instr = 2;
    });
    // selecting the track makes its instrument the current one (what
    // live MIDI plays)
    h.state_mut().ch = 0;
    h.state_mut().instr = 2;
    frames(&mut h, 2);
    // record an A minor chord for a bar
    key(&mut h, Key::R, egui::Modifiers::NONE);
    assert!(h.state().recording());
    play_rows(&mut h, 16);
    let now = std::time::Instant::now;
    for k in [69u8, 72, 76] {
        h.state_mut().midi_note_on(k, k - 12, 0.8, now());
    }
    play_rows(&mut h, 16);
    for k in [69u8, 72, 76] {
        h.state_mut().midi_note_off(k);
    }
    play_rows(&mut h, 4);
    key(&mut h, Key::R, egui::Modifiers::NONE);
    h.state().with(|s| s.engine.stop());
    frames(&mut h, 2);
    let clips = note_clips(&h);
    assert_eq!(clips.len(), 1);
    let stored: Vec<u8> = clips[0].1.iter().map(|n| n.instr).collect();
    eprintln!("WFREPRO recorded note instruments: {stored:?}");
    // the same song with the recorded notes forced to instrument `i`
    let base = h.state().with(|s| s.song.clone());
    let with = |i: Option<u8>, lane: Option<f32>, patch: Option<crate::gen::GenPatch>| {
        let mut s = base.clone();
        let t = s.timeline.as_deref_mut().unwrap();
        if let Some(i) = i {
            for d in t.notes.iter_mut() {
                for n in d.make_mut().notes.iter_mut() {
                    n.instr = i;
                }
            }
        }
        if let Some(hz) = lane {
            let mut l = Lane::new(Target::Gen(1, CUTOFF), "Cutoff".into());
            l.points.push(Point {
                pos: 0.0,
                value: POLY[CUTOFF].unmap(hz),
                shape: Default::default(),
                curve: 0.0,
            });
            t.tracks[0].auto.push(l);
        }
        if let Some(p) = patch {
            s.instruments[1].kind = InstrumentKind::Gen(p);
        }
        s
    };
    let rend = |s: &crate::model::Song| crate::engine::render_song(s, 22050);
    let fp = |v: &[f32]| {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        v.iter().for_each(|x| x.to_bits().hash(&mut hs));
        hs.finish()
    };
    let as_rec = rend(&with(None, None, None));
    let as_1 = rend(&with(Some(1), None, None));
    let as_2 = rend(&with(Some(2), None, None));
    eprintln!(
        "WFREPRO recorded == instrument 1: {}, recorded == instrument 2: {}",
        fp(&as_rec) == fp(&as_1),
        fp(&as_rec) == fp(&as_2)
    );
    let peak = |v: &[f32]| v.iter().fold(0f32, |a, x| a.max(x.abs()));
    assert!(peak(&as_2) > 0.01, "instrument 2 sounds");
    assert_ne!(fp(&as_1), fp(&as_2), "instruments 1 and 2 sound different");
    // the lane: Target::Gen(1, 0) is instrument 2's Cutoff knob
    assert_eq!(Target::Gen(1, CUTOFF).id(), "instrument.2.gen.1");
    // a lane at 1.8 kHz sounds like the patch with Cutoff set to 1.8 kHz,
    // not like Init
    let diff = |a: &[f32], b: &[f32]| -> f32 {
        let e: f32 = a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum();
        let r: f32 = b.iter().map(|y| y * y).sum();
        (e / r.max(1e-9)).sqrt()
    };
    let lane = rend(&with(Some(2), Some(1800.0), None));
    let mut set = pad.clone();
    if let crate::gen::GenEngine::Poly(x) = &mut set.engine {
        x.set(&[(CUTOFF, 1800.0)]);
    }
    let fixed = rend(&with(Some(2), None, Some(set)));
    let init = rend(&with(
        Some(2),
        Some(1800.0),
        crate::gen::preset(concat!(crate::poly_name!(), " Init")),
    ));
    eprintln!(
        "WFREPRO lane vs knob at 1.8 kHz: {:.4}; lane vs no lane: {:.4}; vs Init + lane: {:.4}",
        diff(&lane, &fixed),
        diff(&lane, &as_2),
        diff(&lane, &init)
    );
    assert!(
        diff(&lane, &fixed) < 0.05,
        "the lane sets instrument 2's cutoff"
    );
    assert!(diff(&lane, &as_2) > 0.05, "the lane does something");
    assert!(diff(&lane, &init) > 0.5, "and doesn't sound like Init");
    // playing back with the lane and the WaveForge editor open leaves the
    // patch alone (no knob written back, nothing reset)
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let l = with(None, Some(1800.0), None);
        t.tracks[0].auto = l.timeline.as_deref().unwrap().tracks[0].auto.clone();
    });
    h.state_mut().instr = 2;
    h.state_mut().show_instrument_panel();
    frames(&mut h, 3);
    let before = h.state().with(|s| s.song.instruments[1].kind.clone());
    h.state().with(|s| {
        let song = &s.song;
        s.engine.start(song, 0, 0, false, 0);
    });
    for _ in 0..20 {
        play_rows(&mut h, 4);
        frame(&mut h);
    }
    h.state().with(|s| s.engine.stop());
    frames(&mut h, 2);
    let after = h.state().with(|s| s.song.instruments[1].kind.clone());
    assert_eq!(before, after, "playback changed the WaveForge patch");
    assert_eq!(after, InstrumentKind::Gen(pad), "still the Supersaw Pad");
    // what the fix is about: the recorded notes play instrument 2
    assert!(
        stored.iter().all(|&i| i == 2),
        "recorded notes carry instrument {stored:?}, not the track's 2"
    );
    assert_eq!(fp(&as_rec), fp(&as_2), "recorded notes play instrument 2");
}

/// 01 Init (a plain synth), 02 the WaveForge Supersaw Pad; track 01 is
/// assigned 02, the other tracks nothing
fn instr_song(tracks: usize) -> Harness<'static, App> {
    use crate::model::InstrumentKind;
    let mut h = new_song(tracks);
    let pad = crate::gen::preset(concat!(crate::poly_name!(), " Supersaw Pad")).unwrap();
    h.state().with(|s| {
        let mut one = crate::fileio::demo_song().instruments[0].clone();
        one.name = "Init".into();
        one.kind = InstrumentKind::Synth(Default::default());
        let mut two = one.clone();
        two.name = concat!(crate::poly_name!(), " Supersaw Pad").into();
        two.kind = InstrumentKind::Gen(pad);
        s.song.instruments = vec![one, two];
        s.song.timeline.as_deref_mut().unwrap().tracks[0].instr = 2;
    });
    frames(&mut h, 2);
    h
}

/// a note clip on `tr` at `at` with an A minor chord for a bar, its notes
/// stored with instrument `instr` (0 = none, as older takes were)
fn chord_clip(h: &Harness<'static, App>, tr: usize, at: tl::Tick, instr: u8) -> u32 {
    h.state().with(|s| {
        let id = tl::new_note_clip(&mut s.song, tr, at).unwrap();
        let t = s.song.timeline.as_deref_mut().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else {
            unreachable!()
        };
        t.notes[data as usize].make_mut().notes = [58u8, 61, 65]
            .iter()
            .map(|&key| tl::Note {
                start: 0,
                len: 4 * PPQ,
                key,
                instr,
                ..Default::default()
            })
            .collect();
        id
    })
}

fn clip_instrs(h: &Harness<'static, App>, id: u32) -> Vec<u8> {
    note_clips(h)
        .into_iter()
        .find(|(c, _)| c.id == id)
        .unwrap()
        .1
        .iter()
        .map(|n| n.instr)
        .collect()
}

fn song_fp(s: &crate::model::Song) -> u64 {
    use std::hash::{Hash, Hasher};
    let v = crate::engine::render_song(s, 22050);
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    v.iter().for_each(|x| x.to_bits().hash(&mut hs));
    hs.finish()
}

/// "Use the track's instrument" on an old take (notes stored with no
/// instrument): the clip then renders exactly as if it had been recorded
/// on instrument 02, notes that have an instrument and other clips are
/// left alone, and Ctrl+Z puts it back.
#[test]
fn use_track_instrument_stamps_an_old_take_with_the_tracks_instrument() {
    let mut h = instr_song(2);
    let a = chord_clip(&h, 0, 0, 0);
    // a note in the take that already has an instrument (01) stays 01
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let c = t.clips.iter().find(|c| c.id == a).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else {
            unreachable!()
        };
        t.notes[data as usize].make_mut().notes.push(tl::Note {
            start: 8 * PPQ,
            len: 2 * PPQ,
            key: 46,
            instr: 1,
            ..Default::default()
        });
    });
    // other clips: track 01 at bar 5 (instrument 01 notes), track 02
    // (no assigned instrument) with no-instrument notes
    let b = chord_clip(&h, 0, 16 * PPQ, 1);
    let c = chord_clip(&h, 1, 0, 0);
    frames(&mut h, 2);
    let base = h.state().with(|s| s.song.clone());
    // expected: only clip A's no-instrument notes on 02
    let mut want = base.clone();
    {
        let t = want.timeline.as_deref_mut().unwrap();
        let cl = t.clips.iter().find(|x| x.id == a).unwrap();
        let tl::ClipKind::Notes { data } = cl.kind else {
            unreachable!()
        };
        for n in t.notes[data as usize]
            .make_mut()
            .notes
            .iter_mut()
            .filter(|n| n.instr == 0)
        {
            n.instr = 2;
        }
    }
    // the clip menu's state: 3 notes to stamp, track instrument 02
    let st = h.state().with(|s| {
        super::timeline::use_instr_state(
            s.song.timeline.as_deref().unwrap(),
            &[a],
            s.song.instruments.len(),
        )
    });
    assert_eq!(st, (Some(3), Some(2)));
    h.state_mut().tlv.sels = vec![a];
    h.state_mut().tlv.sel = Some(a);
    h.state_mut().tl_apply(super::timeline::TlOp::UseTrackInstr);
    frames(&mut h, 2);
    assert!(
        h.state().status.contains("3 notes now play 02"),
        "{}",
        h.state().status
    );
    assert_eq!(clip_instrs(&h, a), vec![2, 2, 2, 1]);
    assert_eq!(clip_instrs(&h, b), vec![1, 1, 1], "other clips untouched");
    assert_eq!(clip_instrs(&h, c), vec![0, 0, 0], "other tracks untouched");
    let now = h.state().with(|s| s.song.clone());
    assert_eq!(now.timeline, want.timeline, "only clip A changed");
    let (fp_now, fp_want, fp_base) = (song_fp(&now), song_fp(&want), song_fp(&base));
    assert_eq!(fp_now, fp_want, "renders as instrument 02");
    assert_ne!(fp_now, fp_base, "and it sounds different from before");
    // nothing left to stamp: the menu item is disabled
    let st = h.state().with(|s| {
        super::timeline::use_instr_state(
            s.song.timeline.as_deref().unwrap(),
            &[a],
            s.song.instruments.len(),
        )
    });
    assert_eq!(st.0, Some(0));
    // one undo step back to the old take
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(clip_instrs(&h, a), vec![0, 0, 0, 1], "Ctrl+Z restores it");
    let back = h.state().with(|s| s.song.clone());
    assert_eq!(back.timeline, base.timeline);
    assert_eq!(song_fp(&back), fp_base);
}

/// No assigned instrument: nothing changes and the status says why. A
/// linked copy on another track keeps its notes (the clip is made unique).
#[test]
fn use_track_instrument_needs_an_assigned_instrument_and_spares_other_tracks() {
    let mut h = instr_song(2);
    let c = chord_clip(&h, 1, 0, 0);
    frames(&mut h, 2);
    let st = h.state().with(|s| {
        super::timeline::use_instr_state(
            s.song.timeline.as_deref().unwrap(),
            &[c],
            s.song.instruments.len(),
        )
    });
    assert_eq!(st, (Some(3), None), "disabled: no assigned instrument");
    let before = h.state().with(|s| s.song.timeline.clone());
    h.state_mut().tlv.sels = vec![c];
    h.state_mut().tl_apply(super::timeline::TlOp::UseTrackInstr);
    frames(&mut h, 2);
    assert!(
        h.state().status.contains("no instrument assigned"),
        "{}",
        h.state().status
    );
    assert_eq!(h.state().with(|s| s.song.timeline.clone()), before);
    // a linked copy of a track-01 clip on track 02
    let a = chord_clip(&h, 0, 0, 0);
    let copy = h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let src = t.clips.iter().find(|x| x.id == a).unwrap().clone();
        let id = t.new_id();
        t.clips.push(tl::Clip {
            id,
            track: 1,
            start: 16 * PPQ,
            ..src
        });
        t.sort();
        id
    });
    // and one on the same track, which shares the stamp
    let same = h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let src = t.clips.iter().find(|x| x.id == a).unwrap().clone();
        let id = t.new_id();
        t.clips.push(tl::Clip {
            id,
            start: 32 * PPQ,
            ..src
        });
        t.sort();
        id
    });
    frames(&mut h, 2);
    h.state_mut().tlv.sels = vec![a];
    h.state_mut().tl_apply(super::timeline::TlOp::UseTrackInstr);
    frames(&mut h, 2);
    assert_eq!(clip_instrs(&h, a), vec![2, 2, 2]);
    assert_eq!(
        clip_instrs(&h, same),
        vec![2, 2, 2],
        "a linked copy on the same track follows"
    );
    assert_eq!(
        clip_instrs(&h, copy),
        vec![0, 0, 0],
        "the copy on track 02 is untouched"
    );
    assert_eq!(clip_instrs(&h, c), vec![0, 0, 0]);
}

/// The piano roll header shows "Use track instrument" while its clip has
/// notes without an instrument (the hint), and it stamps them.
#[test]
fn piano_roll_header_offers_use_track_instrument() {
    let mut h = instr_song(1);
    let a = chord_clip(&h, 0, 0, 0);
    frames(&mut h, 2);
    h.state_mut().tlv.sels = vec![a];
    h.state_mut().tlv.sel = Some(a);
    h.state_mut().set_lower(3);
    frames(&mut h, 4);
    assert!(
        h.query_by_label("Use track instrument").is_some(),
        "the hint shows"
    );
    h.get_by_label("Use track instrument").click();
    frames(&mut h, 3);
    assert_eq!(clip_instrs(&h, a), vec![2, 2, 2]);
    assert!(
        h.query_by_label("Use track instrument").is_none(),
        "gone once every note has one"
    );
}

/// The piano roll's playhead is the Timeline's: the play marker when
/// stopped, the transport while playing, mapped through the clip's start
/// and loop into the clip; it is hidden outside the clip, and with Follow
/// on the roll pages to keep it in view.
#[test]
fn piano_roll_playhead_follows_the_timeline_through_the_clip_start_and_loop() {
    let mut h = instr_song(1);
    let bar = 4 * PPQ;
    // a clip at bar 3, four bars long, looping its first bar
    let a = chord_clip(&h, 0, 2 * bar, 2);
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let c = t.clips.iter_mut().find(|c| c.id == a).unwrap();
        c.len = 4 * bar;
        c.loop_len = Some(bar);
    });
    h.state_mut().tlv.sels = vec![a];
    h.state_mut().tlv.sel = Some(a);
    h.state_mut().set_lower(3);
    frames(&mut h, 4);
    // stopped: the play marker, at bar 4 beat 2 = one beat into the loop
    h.state_mut().tlv.cue = 3 * bar + PPQ;
    frames(&mut h, 2);
    assert_eq!(h.state().pr.head, Some((PPQ as f64, false)));
    // outside the clip: no line
    h.state_mut().tlv.cue = 0;
    frames(&mut h, 2);
    assert_eq!(h.state().pr.head, None);
    // playing from bar 5 beat 3 (the third pass of the loop)
    let row = ((4 * bar + 2 * PPQ) / ROW) as usize;
    h.state().with(|s| {
        let song = &s.song;
        s.engine.start(song, 0, row, false, 0);
    });
    for _ in 0..3 {
        play_rows(&mut h, 3);
        frame(&mut h);
        let tl_head = h
            .state()
            .with(|s| super::timeline::tl_head(&s.engine))
            .unwrap();
        assert_eq!(
            h.state().tlv.head_drawn,
            Some(tl_head),
            "the Timeline draws the same head"
        );
        let (src, live) = h.state().pr.head.expect("inside the clip");
        let want = (tl_head - (2 * bar) as f64).rem_euclid(bar as f64);
        assert!(live);
        assert!((src - want).abs() < 1e-6, "{src} vs {want}");
    }
    // Follow: zoomed in and scrolled away, the roll pages to the playhead
    h.state_mut().pr.zoom = 100.0;
    h.state_mut().pr.scroll_x = 3.0 * bar as f64;
    frame(&mut h);
    let (src, _) = h.state().pr.head.unwrap();
    let s = h.state().pr.scroll_x;
    assert!(
        s <= src && src - s < 2.0 * PPQ as f64,
        "in view: {s} .. {src}"
    );
    h.state().with(|s| s.engine.stop());
}

/// Keys → Notes at start: on with no MIDI keyboard, off with one, and the
/// user's saved choice wins either way
#[test]
fn keys_to_notes_defaults_from_midi_and_the_saved_choice() {
    use super::typing::{keys_default, midi_keyboards};
    let none: Vec<String> = vec![];
    let through = vec!["Midi Through:Midi Through Port-0 14:0".to_string()];
    let kb = vec!["Arturia KeyStep 32".to_string()];
    assert!(keys_default(None, &none), "no MIDI device: on");
    assert!(keys_default(None, &through), "a loopback is not a keyboard");
    assert_eq!(midi_keyboards(&through), 0);
    assert!(!keys_default(None, &kb), "a MIDI keyboard: off");
    assert!(
        keys_default(Some(true), &kb),
        "saved on wins over a keyboard"
    );
    assert!(
        !keys_default(Some(false), &none),
        "saved off wins with none"
    );
    // the app: start-up applies it, a click saves the choice
    let mut h = new_song(1);
    h.state_mut().typing_startup(&none);
    assert!(h.state().typing.on);
    h.state_mut().typing_startup(&kb);
    assert!(!h.state().typing.on);
    h.state_mut().typing_toggle();
    assert_eq!(h.state().extra.keys_notes, Some(true));
    h.state_mut().typing_startup(&kb);
    assert!(
        h.state().typing.on,
        "the saved choice wins over the keyboard"
    );
    // a saved choice also stops the hot-plug follow
    frames(&mut h, 2);
    assert!(h.state().typing.on);
}
