//! The piano roll's Pressure lane (docs/MIDI.md): drawing a key pressure
//! curve along a note, one undo step per stroke, right-click to clear, and
//! the curve shading the note.
use super::prlanes::Lane;
use super::prollpan_tests::clip_roll;
use super::ui_tests::{click_at, click_with, drag, frame, key};
use super::*;
use crate::timeline::{self as tl, Tick};
use egui_kittest::Harness;

const R: Tick = tl::ROW;

fn curve(h: &Harness<'static, App>, data: usize) -> Option<Vec<(Tick, f32)>> {
    h.state().with(|s| {
        s.song.timeline.as_ref().unwrap().notes[data].notes[0]
            .pressure()
            .map(|p| p.to_vec())
    })
}

#[test]
fn the_pressure_lane_draws_undoes_and_clears_a_curve() {
    let (mut h, data) = clip_roll(&[(4 * R, 110)]);
    h.state_mut().pr.lane = Lane::Pressure;
    frame(&mut h);
    frame(&mut h);
    let pr = &h.state().pr;
    let vr = pr.vel_rect;
    let y = |v: f32| vr.bottom() - 2.0 - (vr.height() - 4.0) * v;
    let x0 = pr.x_of(4 * R) + 2.0;
    let x1 = pr.x_of(6 * R) - 3.0;
    // a click half way up: 50 % from the start of the note
    click_at(&mut h, Pos2::new(x0, y(0.5)));
    frame(&mut h);
    let c = curve(&h, data).expect("a curve");
    assert_eq!(c.len(), 1);
    assert!((c[0].1 - 0.5).abs() < 0.05, "{c:?}");
    // a stroke along the note, rising: more points, all inside the note
    drag(&mut h, Pos2::new(x0, y(0.2)), Pos2::new(x1, y(0.9)));
    let c = curve(&h, data).unwrap();
    assert!(c.len() >= 3, "{c:?}");
    assert!(c.windows(2).all(|w| w[0].0 < w[1].0));
    assert!(c.iter().all(|p| p.0 >= 0 && p.0 < 2 * R));
    assert!(c.last().unwrap().1 > 0.8);
    // the stroke is one undo step: back to the single click
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frame(&mut h);
    assert_eq!(curve(&h, data).map(|c| c.len()), Some(1));
    // right-click clears it, leaving no empty lane in the file
    click_with(
        &mut h,
        Pos2::new(x0, y(0.5)),
        egui::PointerButton::Secondary,
    );
    frame(&mut h);
    assert!(curve(&h, data).is_none());
    assert!(h
        .state()
        .with(|s| s.song.timeline.as_ref().unwrap().notes[data].notes[0]
            .expr
            .is_empty()));
    // clicks off any note change nothing
    let off = Pos2::new(pr_x(&h, 12 * R), y(0.5));
    click_at(&mut h, off);
    frame(&mut h);
    assert!(curve(&h, data).is_none());
}

fn pr_x(h: &Harness<'static, App>, t: Tick) -> f32 {
    h.state().pr.x_of(t)
}
