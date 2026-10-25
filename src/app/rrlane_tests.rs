//! The piano roll's Conditions and Chance lanes (Round Robin, part C):
//! the lane menu, the "Play this note…" popover, one undo step per choice,
//! the Fill toggle and striped conditional notes.
use super::prlanes::{Choice, Lane};
use super::prollpan_tests::clip_roll;
use super::ui_tests::{click_at, exact, frame, key};
use super::*;
use crate::timeline::{self as tl, Cond, Tick};
use egui_kittest::Harness;

const R: Tick = tl::ROW;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn conds(h: &Harness<'static, App>, data: usize) -> Vec<(Cond, u8)> {
    h.state().with(|s| {
        s.song.timeline.as_ref().unwrap().notes[data]
            .notes
            .iter()
            .map(|n| (n.cond, n.chance))
            .collect()
    })
}

/// the lane spot under the note starting at `row`
fn lane_at(h: &Harness<'static, App>, row: usize) -> Pos2 {
    let pr = &h.state().pr;
    Pos2::new(
        pr.x_of(tl::rows_to_ticks(row)) + 1.0,
        pr.vel_rect.center().y,
    )
}

#[test]
fn the_lane_menu_switches_to_conditions_and_the_popover_sets_a_condition() {
    let (mut h, data) = clip_roll(&[(4 * R, 110), (8 * R, 112)]);
    // the lane label opens the menu; Conditions shows the condition chips
    let vr = h.state().pr.vel_rect;
    click_at(&mut h, Pos2::new(vr.left() - 20.0, vr.center().y));
    assert!(h.state().pr.lane_menu.is_some());
    let r = exact(&h, "Conditions");
    click_at(&mut h, r.center());
    assert_eq!(h.state().pr.lane, Lane::Cond);
    assert!(h.state().pr.lane_menu.is_none());
    // a click under the first note opens "Play this note…" for it
    let p = lane_at(&h, 4);
    click_at(&mut h, p);
    frames(&mut h, 1);
    let pop = h.state().pr.cond_pop.clone().expect("popover open");
    assert_eq!(pop.ids, vec![0]);
    exact(&h, "Play this note…");
    let r = exact(&h, Choice::Every.label());
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert_eq!(
        conds(&h, data),
        vec![(Cond::Every { a: 1, b: 4 }, 0), (Cond::Always, 0)]
    );
    let r = exact(&h, Choice::Fill.label());
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert_eq!(conds(&h, data)[0], (Cond::Fill, 0));
    // Sometimes = always, at 50 %
    let r = exact(&h, Choice::Sometimes.label());
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert_eq!(conds(&h, data)[0], (Cond::Always, 50));
    // each choice is its own undo step
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().pr.cond_pop.is_none(), "Esc closes it");
    for want in [
        (Cond::Fill, 0),
        (Cond::Every { a: 1, b: 4 }, 0),
        (Cond::Always, 0),
    ] {
        key(&mut h, Key::Z, egui::Modifiers::COMMAND);
        frames(&mut h, 2);
        assert_eq!(conds(&h, data)[0], want);
    }
}

#[test]
fn the_fill_toggle_reaches_the_engine_and_the_chance_lane_drags() {
    let (mut h, data) = clip_roll(&[(4 * R, 110)]);
    h.state_mut().pr.lane = Lane::Cond;
    frames(&mut h, 2);
    let p = lane_at(&h, 4);
    click_at(&mut h, p);
    frames(&mut h, 1);
    let r = exact(&h, "Fill");
    click_at(&mut h, r.center());
    frames(&mut h, 1);
    assert!(h.state().pr.fill);
    assert!(h.state().with(|s| s.engine.fill_live));
    let r = exact(&h, "Fill: ON");
    click_at(&mut h, r.center());
    frames(&mut h, 1);
    assert!(!h.state().with(|s| s.engine.fill_live));
    // Chance: a press half way up the lane sets 50 %
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    h.state_mut().pr.lane = Lane::Chance;
    frames(&mut h, 2);
    let vr = h.state().pr.vel_rect;
    let p = Pos2::new(
        lane_at(&h, 4).x,
        vr.bottom() - 2.0 - (vr.height() - 4.0) * 0.5,
    );
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(conds(&h, data)[0], (Cond::Always, 50));
    // the top is "always" again
    let p = Pos2::new(p.x, vr.top() + 1.0);
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(conds(&h, data)[0], (Cond::Always, 0));
}

#[test]
fn tracker_blocks_keep_the_velocity_lane() {
    let mut h = super::prollpan_tests::block_roll(0);
    h.state_mut().pr.lane = Lane::Cond;
    frames(&mut h, 2);
    assert_eq!(h.state().pr.lane, Lane::Vel);
}
