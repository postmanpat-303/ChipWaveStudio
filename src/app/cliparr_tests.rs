//! The Clip Arranger (docs/CLIP_ARRANGER.md): scenes and slots left of the
//! Timeline, rows on the timeline's tracks, quantised launches, stops,
//! slot drags that make Tracker clips, the Master panel and Ctrl+8.
use super::ui_tests::{classic_toolbar, click_at, drag, frame, harness_with, key};
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
    // the pre-0.2.12 row height: every demo track fits the test window
    h.state_mut().tlv.track_h = 46.0;
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
    assert!(has(&h, "Clip Arranger") && has(&h, "Master meter"));
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
    h.state_mut().tlv.snap = super::timeline::Snap::Bar;
    frames(&mut h, 2);
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
    let count = |h: &Harness<'static, App>| {
        h.state()
            .with(|s| s.song.timeline.as_ref().unwrap().clips.len())
    };
    // track 07 has clips there: nothing is overwritten, it asks first
    let ask = h.state().drop_ask.clone().expect("the overlap question");
    assert_eq!(
        ask.plan.start,
        30 * bar,
        "snapped to the bar under the mouse"
    );
    assert!(!ask.plan.overlaps.is_empty());
    assert!(
        !ask.plan.layer_ok,
        "Tracker clips hide each other: no Keep both"
    );
    assert_eq!(count(&h), before, "nothing placed while it asks");
    h.get_by_label("Replace").click();
    frames(&mut h, 2);
    assert!(h.state().drop_ask.is_none());
    let rows = h.state().with(|s| s.song.patterns[b].rows);
    let len = crate::timeline::rows_to_ticks(rows);
    let id = h.state().tlv.sel.expect("the placed clip is selected");
    let (clip, under) = h.state().with(|s| {
        let t = s.song.timeline.as_ref().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap().clone();
        (
            c,
            crate::timeline::overlaps(t, tr, 30 * bar, 30 * bar + len, &[id]),
        )
    });
    assert_eq!(clip.track as usize, tr);
    assert_eq!(
        clip.block(),
        Some((b, col)),
        "a Tracker clip: block + column"
    );
    assert_eq!((clip.start, clip.len), (30 * bar, len));
    assert!(under.is_empty(), "Replace cleared its span: {under:?}");
    assert!(
        h.state().status.starts_with("Placed on track 07 at bar 31"),
        "{}",
        h.state().status
    );
    // one undo step takes it all back
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(count(&h), before);
}

#[test]
fn master_panel_edits_the_song_mixer() {
    let mut h = tl_demo();
    // (the classic toolbar; lcd_tests has the LCD toolbar's popover)
    classic_toolbar(&mut h);
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

/// Del's 0.2.5.0 report: a new song with note clips (a recorded take and a
/// Drum Kit pattern) showed the blocks "00 Block…" as scenes with only
/// empty dots, because the Clip Arranger read the song's Tracker blocks and
/// new songs keep their notes in note clips. The scenes are now the clips'
/// start times, each track's clip sits in its slot, and ▶ plays it.
#[test]
fn a_new_song_with_note_and_drum_kit_clips_shows_and_launches_them() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().extra.show_tracker = false;
    h.state_mut().extra.reduce_motion = true;
    h.state_mut().extra.clip_arranger = true;
    frames(&mut h, 2);
    h.state_mut().new_song();
    frames(&mut h, 3);
    for _ in 0..2 {
        h.get_by_label("+ Add track").click();
        frames(&mut h, 3);
    }
    h.state_mut().view = View::Timeline;
    // track 01: a take of 4 bars at bar 1 and another at bar 5; track 02:
    // a Drum Kit pattern at bar 1
    let bar = 4 * crate::timeline::PPQ;
    let (a, b2) = h.state().with(|s| {
        let mk = |s: &mut Shared, at| {
            let id = crate::timeline::new_note_clip(&mut s.song, 0, at).unwrap();
            let t = s.song.timeline.as_deref_mut().unwrap();
            let c = t.clips.iter().find(|c| c.id == id).unwrap();
            let crate::timeline::ClipKind::Notes { data } = c.kind else {
                unreachable!()
            };
            t.notes[data as usize].make_mut().notes = (0..8)
                .map(|k| crate::timeline::Note {
                    start: k * crate::timeline::PPQ * 2,
                    len: crate::timeline::PPQ,
                    key: 49 + k as u8,
                    ..Default::default()
                })
                .collect();
            id
        };
        (mk(s, 0), mk(s, 4 * bar))
    });
    h.state_mut().tl_apply(super::timeline::TlOp::DrumPattern {
        pat: 0,
        track: 1,
        at: 0,
    });
    frames(&mut h, 4);
    assert!(h.state().ca_active());
    let (blocks, drum) = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let d = t.clips.iter().find(|c| c.track == 1).map(|c| {
            let n = match c.kind {
                crate::timeline::ClipKind::Notes { data } => t.notes[data as usize].notes.len(),
                _ => 0,
            };
            (c.id, c.start, n)
        });
        (s.song.patterns.len(), d)
    });
    let (drum, drum_at, drum_notes) = drum.expect("the Drum Kit pattern clip");
    assert!(drum_notes > 0);
    assert_eq!(drum_at, 0);
    // the scenes: bar 1 and bar 5 (not the blocks "00 Block…")
    let secs = h
        .state()
        .ca
        .secs
        .clone()
        .expect("note clips: section scenes");
    assert!(blocks > 0);
    let names: Vec<&str> = secs.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Bar 1", "Bar 5"]);
    assert!(has(&h, "Scene Bar 1") && has(&h, "Scene Bar 5"));
    assert_eq!(secs[0].clips, vec![Some(a), Some(drum)]);
    assert_eq!(secs[1].clips, vec![Some(b2), None]);
    // the slots have previews, not empty dots
    for (k, tr) in [(0, 0), (0, 1), (1, 0)] {
        assert!(
            !h.state().ca.prev[k][tr].notes.is_empty(),
            "slot {tr} {k} shows its notes"
        );
    }
    assert!(
        h.state().ca.prev[1][1].notes.is_empty(),
        "no clip: an empty dot"
    );
    // ▶ on track 01's bar-5 slot: that clip loops, the other track is held
    h.state_mut().extra.launch_q = 3; // next row
    frames(&mut h, 2);
    let (strip, _) = slot(&h, 0, 1);
    click_at(&mut h, strip.center());
    let (a5, b5) = super::cliparr::sec_rows(&secs[1]);
    let (span, row, held) = h
        .state()
        .with(|s| (s.engine.span(), s.engine.tl_row(), s.engine.held()));
    assert_eq!(span, Some((a5 as usize, b5 as usize)));
    assert_eq!(row, Some(a5 as usize), "it starts at the clip");
    assert_eq!(held & bit(0), 0);
    assert_ne!(held & bit(1), 0, "the drum track is held");
    let len = (b5 - a5) as u64;
    play_rows(&mut h, len);
    assert_eq!(
        h.state().with(|s| s.engine.tl_row()),
        Some(a5 as usize),
        "the clip loops"
    );
    // it sounds: the take's notes play
    let peak = h.state().with(|s| {
        let Shared { song, engine, .. } = s;
        let mut buf = vec![0.0f32; 256];
        let mut pk = 0f32;
        for _ in 0..400 {
            engine.render(song, &mut buf);
            pk = buf.iter().fold(pk, |m, x| m.max(x.abs()));
        }
        pk
    });
    assert!(peak > 0.001, "the launched clip plays ({peak})");
    // the scene Bar 1 plays both tracks there, from the next row
    h.get_by_label("Scene Bar 1").click();
    frames(&mut h, 2);
    play_rows(&mut h, 1);
    let (a1, b1) = super::cliparr::sec_rows(&secs[0]);
    let (span, held) = h.state().with(|s| (s.engine.span(), s.engine.held()));
    assert_eq!(span, Some((a1 as usize, b1 as usize)));
    assert_eq!(held, 0, "every track plays");
    // Stop all, then Play plays the arrangement again (no span left over)
    h.get_by_label("Stop all").click();
    frames(&mut h, 2);
    play_rows(&mut h, 2);
    assert!(!h.state().with(|s| s.engine.playing));
    h.state().with(|s| {
        let song = &s.song;
        s.engine.start(song, 0, 0, false, 0);
    });
    assert_eq!(h.state().with(|s| s.engine.span()), None);
}

/// Tracker-clip songs (the demos) keep the blocks as scenes.
#[test]
fn tracker_clip_songs_keep_their_block_scenes() {
    let h = tl_demo();
    assert!(h.state().ca.secs.is_none());
    let s = h.state().with(|s| s.song.clone());
    assert!(super::cliparr::sections(&s).is_none());
}

#[test]
fn the_level_history_moves_a_column_per_quarter_second_and_keeps_every_peak() {
    let mut ca = super::cliparr::CaUi::default();
    // one second at 60 fps: four columns, not sixty
    for k in 0..60 {
        ca.hist_step(1.0 / 60.0, if k == 7 { 0.9 } else { 0.1 });
    }
    assert_eq!(ca.hist.len(), 4);
    assert_eq!(ca.hist[0], 0.9, "a one-frame peak still shows");
    // a quieter stretch falls half way per column (slow fall)
    assert!((ca.hist[1] - 0.5).abs() < 1e-6, "{:?}", ca.hist);
    assert!((ca.hist[2] - 0.3).abs() < 1e-6, "{:?}", ca.hist);
    // louder rises at once
    for _ in 0..15 {
        ca.hist_step(1.0 / 60.0, 1.0);
    }
    assert_eq!(ca.hist.back().copied(), Some(1.0));
    // 120 columns hold the last 30 s
    for _ in 0..(60 * 40) {
        ca.hist_step(1.0 / 60.0, 0.2);
    }
    assert_eq!(ca.hist.len(), 120);
    assert!((super::cliparr::HIST_STEP * 120.0 - 30.0).abs() < 1e-3);
}

#[test]
fn right_click_the_level_history_pauses_or_hides_it_and_brings_it_back() {
    let mut h = tl_demo();
    // the Master panel in the LCD toolbar's popover (its menus keep it open)
    h.state_mut().lcd.master_open = true;
    frames(&mut h, 3);
    let g = h.state().ca.level_rect;
    assert!(g.is_positive(), "the LEVEL history has room");
    super::objpie_tests::right_click(&mut h, g.center());
    frames(&mut h, 2);
    h.get_by_label("Paused").click();
    frames(&mut h, 2);
    assert_eq!(h.state().extra.level_hist, 1);
    // paused: playing adds no columns
    let n = h.state().ca.hist.len();
    h.state().with(|s| s.engine.playing = true);
    frames(&mut h, 30);
    assert_eq!(h.state().ca.hist.len(), n);
    h.state().with(|s| s.engine.playing = false);
    // hidden: a slim strip that brings it back
    super::objpie_tests::right_click(&mut h, g.center());
    frames(&mut h, 2);
    h.get_by_label("Hidden").click();
    frames(&mut h, 2);
    assert_eq!(h.state().extra.level_hist, 2);
    let strip = h.state().ca.level_rect;
    assert!(strip.height() <= 18.5, "{strip:?}");
    super::objpie_tests::right_click(&mut h, strip.center());
    frames(&mut h, 2);
    h.get_by_label("Scrolling").click();
    frames(&mut h, 2);
    assert_eq!(h.state().extra.level_hist, 0);
    assert!(h.state().ca.level_rect.height() > 36.0);
    assert!(h.state().lcd.master_open, "still open after its menus");
}

/// the Timeline's Arranger button hides the left panel for room (the lanes
/// get wider), it is remembered in the settings, Ctrl+Shift+8 / Ctrl+8
/// bring it back; the Tracker keeps its Instruments list
#[test]
fn the_arranger_button_hides_and_shows_the_left_panel() {
    let mut h = tl_demo();
    assert!(!h.state().extra.side_hidden);
    let w0 = h.state().tlv.lanes.width();
    h.get_by_label("Arranger").click();
    frames(&mut h, 4);
    assert!(h.state().extra.side_hidden, "remembered in the settings");
    assert!(!h.state().ca_active());
    assert!(!has(&h, "Clip Arranger") && !has(&h, "Instruments"));
    assert!(
        h.state().tlv.lanes.width() > w0 + 250.0,
        "the lanes get the room"
    );
    // old settings files have no field: shown
    let x: crate::settings::Extra = serde_json::from_str("{}").unwrap();
    assert!(!x.side_hidden && x.clip_arranger);
    // the Tracker keeps its Instruments list
    h.state_mut().view = View::Pattern;
    frames(&mut h, 4);
    assert!(has(&h, "Instruments"));
    h.state_mut().view = View::Timeline;
    frames(&mut h, 4);
    key(
        &mut h,
        Key::Num8,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    frames(&mut h, 4);
    assert!(h.state().ca_active() && has(&h, "Clip Arranger"));
    h.get_by_label("Arranger").click();
    frames(&mut h, 4);
    key(&mut h, Key::Num8, egui::Modifiers::COMMAND);
    frames(&mut h, 4);
    assert!(
        h.state().ca_active(),
        "Ctrl+8 brings the Clip Arranger back"
    );
}

/// Hiding the Clip Arranger hides its Master panel: the LCD toolbar always
/// has the Master meter (popover); the classic toolbar gets it while hidden.
#[test]
fn the_master_stays_reachable_with_the_arranger_hidden() {
    for lcd in [true, false] {
        let mut h = tl_demo();
        if !lcd {
            classic_toolbar(&mut h);
            assert!(
                !has(&h, "Master meter"),
                "classic: the panel is on the left"
            );
            assert!(h.state().ca.master.is_positive());
        }
        h.get_by_label("Arranger").click();
        frames(&mut h, 4);
        assert!(h.state().side_hidden());
        assert!(
            has(&h, "Master meter"),
            "lcd {lcd}: the meter is in the toolbar"
        );
        h.get_by_label("Master meter").click();
        frames(&mut h, 4);
        assert!(h.state().lcd.master_open, "lcd {lcd}");
        assert!(
            h.state().ca.master.is_positive(),
            "lcd {lcd}: the popover has the Master panel"
        );
        h.get_by_label("Arranger").click();
        frames(&mut h, 4);
        assert_eq!(has(&h, "Master meter"), lcd, "lcd {lcd}: shown again");
    }
}

/// a playing slot's ▶ is a ■: clicking it again stops the track at the
/// next launch boundary (the launch quantise), not at once
#[test]
fn a_playing_slots_play_button_toggles_to_stop_on_the_launch_boundary() {
    let mut h = tl_demo();
    h.state_mut().extra.launch_q = 2; // a beat
    frames(&mut h, 2);
    let b = 3;
    let (strip, _) = slot(&h, 1, b);
    click_at(&mut h, strip.center());
    play_rows(&mut h, 4);
    frames(&mut h, 2);
    let (scene, held) = h.state().with(|s| (s.engine.scene(), s.engine.held()));
    assert_eq!(scene, Some(b));
    assert_eq!(held & bit(1), 0, "track 02 plays");
    // click again: a quantised stop, queued for the next beat
    let (strip, _) = slot(&h, 1, b);
    click_at(&mut h, strip.center());
    let q = h.state().with(|s| s.engine.cue()).expect("a queued stop");
    assert_ne!(q.off & bit(1), 0);
    assert_eq!((q.on & bit(1), q.every), (0, 4));
    assert_eq!(h.state().with(|s| s.engine.held()) & bit(1), 0, "not yet");
    play_rows(&mut h, 4);
    frames(&mut h, 2);
    assert_ne!(h.state().with(|s| s.engine.held()) & bit(1), 0, "stopped");
    // and ▶ once more launches it again
    let (strip, _) = slot(&h, 1, b);
    click_at(&mut h, strip.center());
    play_rows(&mut h, 4);
    assert_eq!(
        h.state().with(|s| s.engine.held()) & bit(1),
        0,
        "plays again"
    );
}
