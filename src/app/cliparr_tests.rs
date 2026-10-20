//! The Clip Arranger (docs/CLIP_ARRANGER.md): scenes and slots left of the
//! Timeline, rows on the timeline's tracks, quantised launches, stops,
//! slot drags that make Tracker clips, the Master panel and Ctrl+8.
use super::ui_tests::{click_at, drag, frame, harness_with, key};
use super::*;
use crate::engine::launch::bit;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// a widget or label with this text
fn has(h: &Harness<'static, App>, text: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text))
        .next()
        .is_some()
}

fn tl_demo() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.reduce_motion = true;
    frames(&mut h, 4);
    h
}

/// render the engine (no audio device in tests) until `rows` more rows played
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

fn slot(h: &Harness<'static, App>, tr: usize, b: usize) -> (Rect, Rect) {
    h.state()
        .ca
        .slot_rects
        .iter()
        .find(|x| x.0 == tr && x.1 == b)
        .map(|x| (x.2, x.3))
        .unwrap_or_else(|| panic!("slot {tr} {b} not drawn"))
}

#[test]
fn shows_on_the_timeline_with_rows_on_the_tracks_and_ctrl8_toggles_it() {
    let mut h = tl_demo();
    assert!(h.state().ca_active());
    assert!(has(&h, "Clip Arranger") && has(&h, "Master"));
    assert!(!has(&h, "Instruments"), "the Instruments list made room");
    let nch = h.state().with(|s| s.song.channels);
    // every track row has its slots, centred on the timeline's track row
    for tr in 0..nch {
        let (hr, _) = h.state().tlv.track_rects(tr).unwrap();
        let (_, whole) = slot(&h, tr, h.state().ca.first);
        assert!(
            (whole.center().y - hr.center().y).abs() < 2.5,
            "track {tr} row lines up"
        );
        let st = h
            .state()
            .ca
            .stop_rects
            .iter()
            .find(|x| x.0 == tr)
            .unwrap()
            .1;
        assert!((st.center().y - hr.center().y).abs() < 2.5);
    }
    // the scene buttons sit on the ruler's rows
    let ruler = h.state().tlv.ruler;
    let sc = h.state().ca.scene_rects[0].1;
    assert!(sc.top() >= ruler.top() - 1.0 && sc.bottom() <= ruler.bottom() + 1.0);
    // Ctrl+8: the Instruments list again (and back)
    key(&mut h, Key::Num8, egui::Modifiers::COMMAND);
    frames(&mut h, 20);
    assert!(!h.state().extra.clip_arranger);
    assert!(has(&h, "Instruments") && !has(&h, "Clip Arranger"));
    key(&mut h, Key::Num8, egui::Modifiers::COMMAND);
    frames(&mut h, 20);
    assert!(has(&h, "Clip Arranger"));
    // the Tracker keeps its Instruments list
    h.state_mut().view = View::Pattern;
    frames(&mut h, 20);
    assert!(has(&h, "Instruments") && !has(&h, "Clip Arranger"));
}

#[test]
fn a_scene_plays_its_block_on_every_track_and_the_next_one_waits_for_the_bar() {
    let mut h = tl_demo();
    let name = h
        .state()
        .with(|s| format!("Scene 01 {}", s.song.patterns[1].name));
    h.get_by_label(&name).click();
    frames(&mut h, 2);
    assert_eq!(
        h.state().with(|s| s.engine.scene()),
        Some(1),
        "stopped: it starts at once"
    );
    play_rows(&mut h, 3);
    let name2 = h
        .state()
        .with(|s| format!("Scene 02 {}", s.song.patterns[2].name));
    h.get_by_label(&name2).click();
    frames(&mut h, 2);
    let q = h.state().with(|s| s.engine.cue()).expect("queued");
    assert_eq!((q.scene, q.every), (Some(2), 16), "one bar of 4/4");
    assert!(
        h.state().status.contains("next bar"),
        "{}",
        h.state().status
    );
    play_rows(&mut h, 12);
    assert_eq!(h.state().with(|s| s.engine.scene()), Some(1));
    play_rows(&mut h, 1);
    assert_eq!(
        h.state().with(|s| (s.engine.scene(), s.engine.row)),
        (Some(2), 0)
    );
    // a second click while queued takes it back
    h.get_by_label(&name).click();
    frames(&mut h, 2);
    assert!(h.state().with(|s| s.engine.cue()).is_some());
    h.get_by_label(&name).click();
    frames(&mut h, 2);
    assert!(h.state().with(|s| s.engine.cue()).is_none());
}

#[test]
fn slot_play_stop_and_stop_all_follow_the_launch_setting() {
    let mut h = tl_demo();
    h.state_mut().extra.launch_q = 2; // a beat
    frames(&mut h, 2);
    // a slot of a new scene: only that track plays
    let b = 3;
    let (strip, _) = slot(&h, 1, b);
    click_at(&mut h, strip.center());
    let (scene, held) = h.state().with(|s| (s.engine.scene(), s.engine.held()));
    assert_eq!(scene, Some(b));
    assert_eq!(held & bit(1), 0);
    assert_ne!(held & bit(0), 0, "the other tracks are held");
    // another track's slot of the playing scene joins on the next beat
    let (strip, _) = slot(&h, 0, b);
    click_at(&mut h, strip.center());
    let q = h.state().with(|s| s.engine.cue()).unwrap();
    assert_eq!((q.scene, q.on & bit(0) != 0, q.every), (None, true, 4));
    play_rows(&mut h, 4);
    assert_eq!(h.state().with(|s| s.engine.held()) & bit(0), 0);
    // ■ on track 2's row stops it on the beat
    h.get_by_label("Stop track 02").click();
    frames(&mut h, 2);
    play_rows(&mut h, 4);
    assert_ne!(h.state().with(|s| s.engine.held()) & bit(1), 0);
    // ■ in the header stops everything
    h.get_by_label("Stop all").click();
    frames(&mut h, 2);
    play_rows(&mut h, 8);
    assert!(!h.state().with(|s| s.engine.playing));
}

#[test]
fn dragging_a_slot_onto_the_timeline_makes_a_tracker_clip_on_the_bar() {
    let mut h = tl_demo();
    let before = h
        .state()
        .with(|s| s.song.timeline.as_ref().unwrap().clips.len());
    let (b, col, tr) = (2usize, 4usize, 6usize);
    let (_, whole) = slot(&h, col, b);
    let (_, row) = h.state().tlv.track_rects(tr).unwrap();
    let bar = 4 * crate::timeline::PPQ;
    let x = h.state().tlv.x_of(30 * bar) + 6.0;
    drag(
        &mut h,
        whole.center() + egui::vec2(10.0, 0.0),
        Pos2::new(x, row.center().y),
    );
    frames(&mut h, 2);
    let (n, clip) = h.state().with(|s| {
        let t = s.song.timeline.as_ref().unwrap();
        let id = t.clips.iter().map(|c| c.id).max().unwrap();
        (
            t.clips.len(),
            t.clips.iter().find(|c| c.id == id).unwrap().clone(),
        )
    });
    assert_eq!(n, before + 1);
    assert_eq!(clip.track as usize, tr);
    assert_eq!(
        clip.block(),
        Some((b, col)),
        "a Tracker clip: block + column"
    );
    assert_eq!(clip.start, 30 * bar, "snapped to the bar under the mouse");
    let rows = h.state().with(|s| s.song.patterns[b].rows);
    assert_eq!(clip.len, crate::timeline::rows_to_ticks(rows));
    assert!(
        h.state().status.starts_with("Tracker clip"),
        "{}",
        h.state().status
    );
    // one undo step takes it away
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(
        h.state()
            .with(|s| s.song.timeline.as_ref().unwrap().clips.len()),
        before
    );
}

#[test]
fn master_panel_edits_the_song_mixer() {
    let mut h = tl_demo();
    let m = h.state().ca.master;
    assert!(m.is_positive(), "the Master panel has room under the grid");
    let on = h.state().with(|s| s.song.mixer.limiter.enabled);
    let label = if on { "Limiter on" } else { "Limiter off" };
    h.get_by_label(label).click();
    frames(&mut h, 2);
    assert_ne!(h.state().with(|s| s.song.mixer.limiter.enabled), on);
    assert!(h.state().dirty);
}

#[test]
fn the_inspector_picks_the_tracks_instrument() {
    let mut h = tl_demo();
    h.state_mut().extra.inspector_open = true;
    h.state_mut().ch = 0;
    frames(&mut h, 3);
    let names: Vec<String> = h.state().with(|s| {
        s.song
            .instruments
            .iter()
            .enumerate()
            .map(|(i, x)| format!("{:02X} {}", i + 1, x.name))
            .collect()
    });
    let cur = h.state().track_slot(0) as usize;
    // the picker shows the current one; its list has them all
    let want = names[cur - 1].clone();
    let combo = h
        .query_all_by(|n| {
            n.role() == egui::accesskit::Role::ComboBox
                && (n.value().as_deref() == Some(want.as_str())
                    || n.label().as_deref() == Some(want.as_str()))
        })
        .next()
        .expect("the instrument picker");
    combo.click();
    frames(&mut h, 3);
    let k = if cur == 2 { 3 } else { 2 };
    h.query_all_by_label(&names[k - 1]).last().unwrap().click();
    frames(&mut h, 3);
    assert_eq!(h.state().track_slot(0) as usize, k);
    assert!(
        h.state().status.starts_with("Track 01 now plays"),
        "{}",
        h.state().status
    );
}

#[test]
fn the_view_tab_bar_lights_the_view_you_are_in() {
    let mut h = tl_demo();
    h.state_mut().view = View::Pattern;
    h.state_mut().show_instrument_panel();
    frames(&mut h, 3);
    // the lower panel shows the instrument; TRACKER is still the lit tab
    assert_eq!(h.state().vtab_current(), Some(super::viewbar::VTab::Inst));
    let lit = h.state().vtab_lit();
    assert_eq!(lit, Some(super::viewbar::VTab::Tracker));
}
