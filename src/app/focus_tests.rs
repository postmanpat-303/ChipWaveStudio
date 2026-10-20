//! Focus on the Timeline (20 Oct 2026): the playhead moves while the
//! timeline plays (every lower-panel tab, Clip Arranger on or off), chords
//! in the piano roll, the Tracker hidden by default, the save prompt.
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline as tl;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// a harness as Del runs it: the Tracker hidden (the default)
fn focus(song: Song) -> Harness<'static, App> {
    let mut h = harness_with(song);
    h.state_mut().extra.show_tracker = false;
    h.state_mut().extra.reduce_motion = true;
    frames(&mut h, 2);
    h
}

/// render the engine (no audio device in tests) for `rows` more rows
fn play_rows(h: &mut Harness<'static, App>, rows: u64) {
    h.state().with(|s| {
        let Shared { song, engine, .. } = s;
        let target = engine.rows_played + rows;
        let mut buf = vec![0.0f32; 128];
        let mut guard = 0;
        while engine.rows_played < target && engine.playing && guard < 200_000 {
            engine.render(song, &mut buf);
            guard += 1;
        }
    });
}

/// the demo in timeline mode with no automation at all (lanes used to be
/// the only thing that moved the playhead)
fn plain_demo() -> Song {
    let mut s = fileio::demo_song();
    tl::enter_timeline_mode(&mut s);
    s.automation.clear();
    s.modulation = Default::default();
    if let Some(t) = s.timeline.as_deref_mut() {
        for tr in &mut t.tracks {
            tr.auto.clear();
        }
        for c in &mut t.clips {
            c.auto.clear();
        }
    }
    s
}

/// File > New, one track, a note clip with a chord on it
fn new_song_with_notes() -> Harness<'static, App> {
    let mut h = focus(fileio::demo_song());
    h.state_mut().new_song();
    frames(&mut h, 3);
    h.get_by_label("+ Add track").click();
    frames(&mut h, 3);
    h.state().with(|s| {
        let id = tl::new_note_clip(&mut s.song, 0, 0).unwrap();
        let t = s.song.timeline.as_deref_mut().unwrap();
        let tl::ClipKind::Notes { data } = t.clips.iter().find(|c| c.id == id).unwrap().kind else {
            unreachable!()
        };
        let d = t.notes[data as usize].make_mut();
        for k in [49u8, 53, 56] {
            d.notes.push(tl::Note {
                start: 0,
                len: 8 * tl::PPQ,
                key: k,
                vel: 0xFFFF,
                ..Default::default()
            });
        }
        let Shared { song, engine, .. } = s;
        engine.ensure_plan(song);
    });
    frames(&mut h, 2);
    h
}

/// press Space, render `rows`, and return the playhead drawn before and after
fn heads_while_playing(h: &mut Harness<'static, App>) -> (Option<f64>, Option<f64>) {
    h.state().with(|s| s.engine.stop());
    frames(h, 2);
    key(h, Key::Space, egui::Modifiers::NONE);
    frames(h, 2);
    assert!(h.state().with(|s| s.engine.playing), "Space plays");
    play_rows(h, 6);
    frames(h, 2);
    let a = h.state().tlv.head_drawn;
    play_rows(h, 10);
    frames(h, 2);
    let b = h.state().tlv.head_drawn;
    h.state().with(|s| s.engine.stop());
    frames(h, 2);
    (a, b)
}

fn check_every_lower_tab(h: &mut Harness<'static, App>, what: &str) {
    use super::viewbar::VTab;
    for ca in [false, true] {
        h.state_mut().extra.clip_arranger = ca;
        for tab in [VTab::Piano, VTab::Devices, VTab::Inst, VTab::Auto] {
            h.state_mut().view = View::Timeline;
            h.state_mut().vtab_select(tab);
            frames(h, 3);
            assert_eq!(h.state().view, View::Timeline);
            assert_eq!(h.state().ca_active(), ca);
            let (a, b) = heads_while_playing(h);
            let (a, b) = (
                a.unwrap_or_else(|| panic!("{what} {tab:?} ca {ca}: no playhead")),
                b.unwrap(),
            );
            // 10 rows = 10 * ROW ticks further on
            let d = b - a;
            assert!(
                (d - 10.0 * tl::ROW as f64).abs() <= tl::ROW as f64,
                "{what} {tab:?} ca {ca}: playhead {a} -> {b}"
            );
        }
    }
}

#[test]
fn the_playhead_moves_while_the_timeline_plays_in_every_lower_tab() {
    let mut h = focus(plain_demo());
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    check_every_lower_tab(&mut h, "demo, no automation");
    let mut h = new_song_with_notes();
    check_every_lower_tab(&mut h, "new song");
    // the demo as shipped (with automation lanes) too
    let mut s = fileio::demo_song();
    tl::enter_timeline_mode(&mut s);
    let mut h = focus(s);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    check_every_lower_tab(&mut h, "demo");
}

#[test]
fn space_in_the_mixer_plays_the_timeline_not_a_block_loop() {
    let mut h = new_song_with_notes();
    h.state_mut().view = View::Mixer;
    frames(&mut h, 2);
    key(&mut h, Key::Space, egui::Modifiers::NONE);
    frames(&mut h, 2);
    let (playing, looping, tl_on) =
        h.state().with(|s| (s.engine.playing, s.engine.loop_pattern, s.engine.tl_row().is_some()));
    assert!(playing && !looping && tl_on);
    play_rows(&mut h, 8);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 2);
    let a = h.state().tlv.head_drawn.expect("playhead");
    assert!(a >= 7.0 * tl::ROW as f64, "{a}");
    // Enter restarts from the play marker (no block restart)
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().with(|s| s.engine.playing && !s.engine.loop_pattern));
}
