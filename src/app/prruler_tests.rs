//! Piano roll ruler (Del, 23 Oct 2026): a click in the bar strip places the
//! paste marker on the grid, Ctrl+V pastes there and chains, and in the
//! Timeline the play marker follows.
use super::prollpan_tests::{clip_notes, clip_roll};
use super::timeline_m2_tests::{click_at, with_mods};
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline::{self as tl, Tick};
use egui_kittest::Harness;

const R: Tick = tl::ROW;
const CMD: egui::Modifiers = egui::Modifiers::COMMAND;

/// a point in the ruler (the strip just above the grid) at `row` + `dx` px
fn ruler(h: &Harness<'static, App>, row: Tick, dx: f32) -> Pos2 {
    let pr = &h.state().pr;
    Pos2::new(pr.x_of(row * R) + dx, pr.grid_rect.top() - 8.0)
}

fn marker(h: &Harness<'static, App>) -> Option<Tick> {
    h.state().pr.marker.map(|(_, m)| m)
}

fn copy_all(h: &mut Harness<'static, App>) {
    key(h, Key::A, CMD);
    key(h, Key::C, CMD);
}

#[test]
fn a_ruler_click_sets_the_marker_on_the_grid_and_alt_does_not_snap() {
    let (mut h, _) = clip_roll(&[(2 * R, 60)]);
    h.state_mut().pr.grid = 4 * R;
    frame(&mut h);
    // a bit past row 9: the nearest beat is row 8
    let p = ruler(&h, 9, 3.0);
    click_at(&mut h, p);
    assert_eq!(marker(&h), Some(8 * R));
    assert!(h.state().status.starts_with("Paste marker: 1.3"));
    // Alt (the Timeline ruler's no-snap key): exactly under the mouse
    let p = ruler(&h, 9, 0.0);
    with_mods(&mut h, egui::Modifiers::ALT, egui::Event::PointerMoved(p));
    for pressed in [true, false] {
        with_mods(
            &mut h,
            egui::Modifiers::ALT,
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::ALT,
            },
        );
    }
    frame(&mut h);
    let m = marker(&h).unwrap();
    assert!((m - 9 * R).abs() <= 2, "no snap: {m} vs {}", 9 * R);
}

#[test]
fn ctrl_v_pastes_at_the_marker_and_chains_end_to_end() {
    let (mut h, data) = clip_roll(&[(2 * R, 60), (2 * R, 64), (3 * R, 67)]);
    copy_all(&mut h);
    let p = ruler(&h, 16, 0.0);
    click_at(&mut h, p);
    assert_eq!(marker(&h), Some(16 * R));
    // the play marker elsewhere: the paste marker wins
    h.state_mut().tlv.cue = 4 * R;
    frame(&mut h);
    key(&mut h, Key::V, CMD);
    let n = clip_notes(&h, data);
    assert_eq!(&n[3..], &[(16 * R, 60), (16 * R, 64), (17 * R, 67)]);
    // the block is 3 rows (on the 1-row grid): the marker moves past it
    assert_eq!(marker(&h), Some(19 * R));
    key(&mut h, Key::V, CMD);
    key(&mut h, Key::V, CMD);
    let n = clip_notes(&h, data);
    assert_eq!(
        &n[6..],
        &[
            (19 * R, 60),
            (19 * R, 64),
            (20 * R, 67),
            (22 * R, 60),
            (22 * R, 64),
            (23 * R, 67)
        ],
        "repeated Ctrl+V lays copies end to end"
    );
    assert_eq!(marker(&h), Some(25 * R));
    // Ctrl+D still copies right after the selection, and the marker follows
    key(&mut h, Key::D, CMD);
    let n = clip_notes(&h, data);
    assert_eq!(&n[12..], &[(25 * R, 60), (25 * R, 64), (26 * R, 67)]);
    assert_eq!(marker(&h), Some(28 * R));
}

#[test]
fn without_a_marker_paste_works_as_before() {
    let (mut h, data) = clip_roll(&[(2 * R, 60), (3 * R, 67)]);
    copy_all(&mut h);
    h.state_mut().tlv.cue = 16 * R;
    frame(&mut h);
    frame(&mut h);
    key(&mut h, Key::V, CMD);
    assert_eq!(&clip_notes(&h, data)[2..], &[(16 * R, 60), (17 * R, 67)]);
    assert_eq!(marker(&h), None, "Ctrl+V sets no marker");
    key(&mut h, Key::V, CMD);
    assert_eq!(&clip_notes(&h, data)[4..], &[(19 * R, 60), (20 * R, 67)]);
}

#[test]
fn a_ruler_click_while_stopped_seeks_the_song_to_the_clip_position() {
    // a clip that starts at bar 3 of the song
    let mut s = Song::clean();
    crate::chanops::insert_channel(&mut s, 0, "Keys");
    let id = tl::new_note_clip(&mut s, 0, 32 * R).unwrap();
    let mut h = harness_with(s);
    h.state_mut().default_layout();
    h.state_mut().tlv.sel = Some(id);
    for _ in 0..3 {
        frame(&mut h);
    }
    {
        let a = h.state_mut();
        a.pr.zoom = 20.0;
        a.pr.scroll_x = 0.0;
        a.pr.grid = R;
    }
    frame(&mut h);
    frame(&mut h);
    assert!(!h.state().with(|s| s.engine.playing));
    let p = ruler(&h, 8, 0.0);
    click_at(&mut h, p);
    assert_eq!(marker(&h), Some(8 * R));
    assert_eq!(h.state().tlv.cue, 40 * R, "clip start + 8 rows");
    assert!(h.state().status.contains("play marker moved too"));
}
