//! Clip Variations in the piano roll (Round Robin, part A): the take tabs,
//! "+Var", the Variations popover and one undo step per change.
use super::prollpan_tests::clip_roll;
use super::ui_tests::{click_at, exact, frame, key};
use super::*;
use crate::timeline::{self as tl, rr::Order, Tick};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

const R: Tick = tl::ROW;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn data_of(h: &Harness<'static, App>, data: usize) -> tl::NoteClipData {
    h.state()
        .with(|s| (*s.song.timeline.as_ref().unwrap().notes[data]).clone())
}

#[test]
fn plus_var_adds_a_take_the_tabs_switch_and_undo_steps_back() {
    let (mut h, data) = clip_roll(&[(4 * R, 60)]);
    assert!(data_of(&h, data).var.is_none());
    let r = exact(&h, "+Var");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    let d = data_of(&h, data);
    assert_eq!(d.var_count(), 2);
    assert_eq!(d.var.as_ref().unwrap().cur, 1, "the new take is shown");
    assert_eq!(d.notes.len(), 1, "a copy of the one before");
    // change take B, then go back to A: A is untouched
    h.state_mut().with(|s| {
        s.song.timeline.as_deref_mut().unwrap().notes[data]
            .make_mut()
            .notes[0]
            .key = 67;
    });
    frames(&mut h, 1);
    let r = exact(&h, "A");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    let d = data_of(&h, data);
    assert_eq!(d.var.as_ref().unwrap().cur, 0);
    assert_eq!(d.notes[0].key, 60);
    assert_eq!(d.var_all()[1][0].key, 67);
    // "+Var" now adds C; four at most
    let r = exact(&h, "+Var");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    let r = exact(&h, "+Var");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert_eq!(data_of(&h, data).var_count(), 4);
    assert!(h.query_by_label("+Var").is_none(), "no +Var at four takes");
    // each step undoes on its own
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(data_of(&h, data).var_count(), 3);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(data_of(&h, data).var.as_ref().unwrap().cur, 1);
}

#[test]
fn the_popover_sets_the_order_and_seed_previews_and_removes() {
    let (mut h, data) = clip_roll(&[(4 * R, 60)]);
    let r = exact(&h, "+Var");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    let r = exact(&h, "Order…");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert!(h.state().pr.var_pop.is_some());
    exact(&h, "The first 16 passes");
    let r = exact(&h, "Ping-pong");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert_eq!(
        data_of(&h, data).var.as_ref().unwrap().order,
        Order::PingPong
    );
    let seed0 = data_of(&h, data).var.as_ref().unwrap().seed;
    let r = exact(&h, "New");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert_ne!(data_of(&h, data).var.as_ref().unwrap().seed, seed0);
    // the preview follows the order: ping-pong over two takes alternates
    let d = data_of(&h, data);
    let seq = super::prvars::preview(&d, 0, 6);
    assert_eq!(
        seq,
        d.var
            .as_ref()
            .map(|v| (0..6).map(|k| v.pick(0, k)).collect::<Vec<_>>())
            .unwrap()
    );
    // undo the seed and the order
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().pr.var_pop.is_none(), "Esc closes it");
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(data_of(&h, data).var.as_ref().unwrap().seed, seed0);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(
        data_of(&h, data).var.as_ref().unwrap().order,
        Order::default()
    );
    // Remove B: one take left turns Variations off (and the file is as before)
    let r = exact(&h, "Order…");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    let r = exact(&h, "Remove B");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert!(data_of(&h, data).var.is_none());
    assert!(h.state().pr.var_pop.is_none());
}

#[test]
fn the_clip_pie_variations_wedge_adds_and_opens_the_order() {
    let (mut h, data) = clip_roll(&[(4 * R, 60)]);
    let id = h
        .state()
        .with(|s| {
            s.song
                .timeline
                .as_ref()
                .unwrap()
                .clips
                .iter()
                .find(|c| c.notes() == Some(data))
                .map(|c| c.id)
        })
        .unwrap();
    // the wedge offers Add only at one take, then Order… too
    let it = super::objpie::var_item(1);
    assert_eq!(it.label, "Variations");
    assert_eq!(it.children, vec!["Add variation".to_string()]);
    assert_eq!(
        super::objpie::var_item(4).children,
        vec!["Order…".to_string()]
    );
    h.state_mut().var_pie_pick(Some(id), Some("Add variation"));
    frames(&mut h, 2);
    assert_eq!(data_of(&h, data).var_count(), 2);
    h.state_mut().var_pie_pick(Some(id), Some("Order…"));
    frames(&mut h, 2);
    assert_eq!(h.state().pr.var_pop.as_ref().map(|p| p.data), Some(data));
    exact(&h, "Random no repeat");
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(data_of(&h, data).var.is_none(), "one undo step");
}
