//! Timeline view (M1) UI tests: view switching, new songs, clips, ruler
//! seek, double-click to the pattern editor, keys, and the frame budget.
use super::ui_tests::{double_click, frame, harness_with, key};
use super::*;
use crate::timeline::{rows_to_ticks, ROW};
use egui_kittest::Harness;

fn click(h: &mut Harness<'static, App>, p: Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    frame(h);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        frame(h);
    }
    for _ in 0..3 {
        frame(h);
    }
}

fn on_timeline(song: Song) -> Harness<'static, App> {
    let mut h = harness_with(song);
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    assert_eq!(h.state().view, View::Timeline, "Ctrl+T opens the timeline");
    h
}

#[test]
fn ctrl_t_tab_and_cycle_reach_the_timeline() {
    let mut h = on_timeline(fileio::demo_song());
    let c = h.state().tlv.cache.as_ref().expect("derived timeline");
    assert_eq!(c.tl.tracks.len(), 8);
    assert!(c.tl.clips.len() > 50);
    assert!(h.state().tlv.lanes.width() > 300.0);
    // the VIEW tab is there, and Ctrl+Tab goes on to the mixer
    assert!(super::topbar::VIEWS
        .iter()
        .any(|v| v.0 == View::Timeline && v.1 == "Timeline"));
    key(&mut h, Key::Tab, egui::Modifiers::COMMAND);
    assert_eq!(h.state().view, View::Mixer);
    key(&mut h, Key::Num1, egui::Modifiers::COMMAND);
    assert_eq!(h.state().view, View::Pattern);
}

#[test]
fn new_songs_open_on_the_timeline_unless_switched_off() {
    let mut h = harness_with(fileio::demo_song());
    assert!(h.state().extra.new_in_timeline, "Del's default");
    h.state_mut().new_song();
    frame(&mut h);
    frame(&mut h);
    assert_eq!(h.state().view, View::Timeline);
    let c = h.state().tlv.cache.as_ref().unwrap();
    assert_eq!(c.tl.clips.len(), 0, "an empty block has no clips");
    assert_eq!(c.map.lens, vec![64]);
    // 16 bars across: the empty block takes a quarter of the width
    let w = h.state().tlv.x_of(rows_to_ticks(64)) - h.state().tlv.x_of(0);
    let lw = h.state().tlv.lanes.width();
    assert!((w / lw - 0.25).abs() < 0.02, "{w} of {lw}");
    // double-click an empty track opens block 00 at that channel
    let lanes = h.state().tlv.lanes;
    let p = Pos2::new(
        h.state().tlv.x_of(rows_to_ticks(8)) + 2.0,
        lanes.top() + 46.0 * 2.5,
    );
    double_click(&mut h, p);
    assert_eq!(h.state().view, View::Pattern);
    assert_eq!((h.state().pattern, h.state().ch, h.state().row), (0, 2, 8));
    // the setting off: File > New stays on the pattern editor
    h.state_mut().extra.new_in_timeline = false;
    h.state_mut().new_song();
    frame(&mut h);
    assert_eq!(h.state().view, View::Pattern);
}

#[test]
fn double_click_a_clip_opens_its_block_column_and_row() {
    let mut h = on_timeline(fileio::demo_song());
    // a clip on track 3 (index 2) that starts after the first entry
    let (id, start, block, col) = {
        let c = h.state().tlv.cache.as_ref().unwrap();
        let cl =
            c.tl.clips
                .iter()
                .find(|cl| cl.track == 2 && cl.start > 0 && cl.loop_len.is_none())
                .expect("a clip");
        let (b, col) = cl.block().unwrap();
        (cl.id, cl.start, b, col)
    };
    // zoom in (10 px a row) and make sure it is on screen
    h.state_mut().tlv.zoom = 40.0;
    h.state_mut().tlv.scroll = (start - rows_to_ticks(16)).max(0) as f64;
    frame(&mut h);
    let r = h.state().tlv.clip_rect(id).unwrap();
    let x = h.state().tlv.x_of(start + 5 * ROW + ROW / 2);
    let p = Pos2::new(x, r.center().y);
    assert!(h.state().tlv.lanes.contains(p));
    click(&mut h, p);
    assert_eq!(h.state().tlv.sel, Some(id), "a click selects");
    double_click(&mut h, p);
    let s = h.state();
    assert_eq!(s.view, View::Pattern);
    assert_eq!((s.pattern, s.ch, s.row), (block, col, 5));
    assert!(s.status.contains("Editing block"), "{}", s.status);
    // Ctrl+T back; Enter opens the selected clip at its start
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    h.state_mut().tlv.sel = Some(id);
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    assert_eq!(h.state().view, View::Pattern);
    assert_eq!((h.state().pattern, h.state().row), (block, 0));
}

#[test]
fn ruler_click_seeks_to_the_bar_like_the_arranger() {
    let mut h = on_timeline(fileio::demo_song());
    let (lens, ruler) = {
        let s = h.state();
        (s.tlv.cache.as_ref().unwrap().map.lens.clone(), s.tlv.ruler)
    };
    let target_row = lens[0] + lens[1] + 21; // bar-snaps to row 16 of entry 2
    let x = h.state().tlv.x_of(rows_to_ticks(target_row)) + 1.0;
    click(&mut h, Pos2::new(x, ruler.center().y));
    assert_eq!(h.state().cue, (2, 16));
    // playing: the engine jumps there and the playhead follows it
    h.state_mut().with(|s| {
        let Shared { song, engine, .. } = s;
        engine.start(song, 0, 0, false, 0);
    });
    click(&mut h, Pos2::new(x, ruler.center().y));
    let (op, row) = h.state().with(|s| (s.engine.order_pos, s.engine.row));
    assert_eq!((op, row), (2, 16));
}

#[test]
fn plain_keys_do_not_type_notes_and_zoom_keys_work() {
    let mut h = on_timeline(fileio::demo_song());
    let before = h.state().with(|s| s.song.patterns.clone());
    let z0 = h.state().tlv.zoom;
    for k in [Key::Q, Key::W, Key::Num2, Key::Delete, Key::Backspace] {
        key(&mut h, k, egui::Modifiers::NONE);
    }
    assert_eq!(h.state().with(|s| s.song.patterns.clone()), before);
    key(&mut h, Key::Plus, egui::Modifiers::NONE);
    assert!(h.state().tlv.zoom > z0);
    key(&mut h, Key::F, egui::Modifiers::NONE);
    assert!((h.state().tlv.zoom - z0).abs() < 1e-3, "F fits again");
    // F11: full-height timeline, the view stays
    key(&mut h, Key::F11, egui::Modifiers::NONE);
    assert_eq!(h.state().view, View::Timeline);
}

#[test]
fn header_mute_solo_and_volume_drive_the_mixer() {
    let mut h = on_timeline(fileio::demo_song());
    let heads = h.state().tlv.heads;
    // M of track 2 sits 108 px from the header's right edge
    let y = heads.top() + 46.0 + 1.0 + 4.0 + 8.5;
    click(&mut h, Pos2::new(heads.right() - 108.0 + 8.0, y));
    assert!(h.state().with(|s| s.song.mixer.channels[1].mute));
    click(&mut h, Pos2::new(heads.right() - 108.0 + 8.0, y));
    assert!(!h.state().with(|s| s.song.mixer.channels[1].mute));
    click(&mut h, Pos2::new(heads.right() - 88.0 + 8.0, y));
    assert!(h.state().with(|s| s.song.mixer.channels[1].solo));
    // the fader: far left = silent
    click(
        &mut h,
        Pos2::new(heads.right() - 108.0 + 37.0 + 6.0 + 1.0, y),
    );
    let v = h.state().with(|s| s.song.mixer.channels[1].volume);
    assert!(v < 0.1, "{v}");
}

#[test]
fn hundreds_of_clips_stay_within_the_frame_budget() {
    // 32 channels x 300 entries over 30 distinct blocks: ~9 600 clips
    let mut s = fileio::demo_song();
    s.channels = 32;
    s.channel_names = (1..=32).map(|i| format!("Ch {i}")).collect();
    s.patterns = (0..30)
        .map(|b| {
            let mut p = Pattern::new(&format!("B{b}"), 64, 32);
            for r in (0..64).step_by(2) {
                for c in 0..32 {
                    if (r + c + b) % 3 == 0 {
                        p.cells[r][c] = Cell {
                            note: 30 + ((r + c) % 24) as u8,
                            instr: 1,
                            ..Default::default()
                        };
                    }
                }
            }
            p
        })
        .collect();
    s.order = (0..300).map(|i| (i * 7) % 30).collect();
    let mut h = on_timeline(s);
    assert!(h.state().tlv.cache.as_ref().unwrap().tl.clips.len() > 9000);
    // fit (the whole song: every clip on screen) and a close zoom
    for zoom in [0.0, 40.0] {
        h.state_mut().tlv.zoom = zoom;
        frame(&mut h);
        let mut worst = 0;
        for _ in 0..5 {
            frame(&mut h);
            worst = worst.max(h.state().tlv.frame_us);
        }
        eprintln!("timeline frame at zoom {zoom}: {worst} us");
        // generous for unoptimised test builds on CI; release is far faster
        assert!(worst < 60_000, "{worst} us");
    }
}
