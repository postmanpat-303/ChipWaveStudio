//! The lower panel's Devices tab (0.2.4): showing the selected track's
//! chain, Browser › Effects drops at a position, reorder by dragging a
//! card's title, undo, and every device body drawing
use super::ui_tests::{drag, frame, harness_with, key};
use super::*;
use crate::browser::{Item, Tab};
use crate::fx::{Device, FxKind};
use egui::Key;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn chain(h: &Harness<'static, App>, slot: u8) -> Vec<FxKind> {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    g.song.instruments[slot as usize - 1]
        .fx
        .iter()
        .map(|d| d.kind)
        .collect()
}

fn fx_item(kind: FxKind) -> Item {
    crate::browser::builtin_items()
        .into_iter()
        .find(|i| i.key == format!("fx:{kind:?}"))
        .expect("built-in effect item")
}

/// the demo, Devices showing on instrument 7 (Supersaw Pad: filter + chorus)
fn devices() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    key(&mut h, Key::Num7, egui::Modifiers::COMMAND);
    h.state_mut().instr = 7;
    h.state_mut().dev.follow_ch = h.state().ch;
    frames(&mut h, 3);
    h
}

#[test]
fn ctrl_7_shows_devices_and_timeline_piano_comes_back() {
    let mut h = devices();
    assert!(h.state().devices_visible());
    assert_eq!(
        h.state().vtab_current(),
        Some(super::viewbar::VTab::Devices)
    );
    assert!(!h.state().pr.open && !h.state().extra.lower_inst);
    // the instrument card and one card per device were drawn
    assert!(h.state().dev.inst.width() > 300.0);
    assert_eq!(h.state().dev.cards.len(), chain(&h, 7).len());
    key(&mut h, Key::Num6, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(h.state().is_default_layout());
    assert!(!h.state().devices_visible());
}

#[test]
fn browser_effects_drop_inserts_at_the_marker_and_undo_removes_it() {
    let mut h = devices();
    let before = chain(&h, 7);
    assert!(before.len() >= 2, "{before:?}");
    let ctx = h.ctx.clone();
    // dropped between device 1 and 2
    h.state_mut()
        .br_drop(&fx_item(FxKind::Eq), browserview::DropAt::Device(1), &ctx);
    frames(&mut h, 2);
    let mut want = before.clone();
    want.insert(1, FxKind::Eq);
    assert_eq!(chain(&h, 7), want);
    assert_eq!(h.state().dev.cards.len(), want.len());
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert_eq!(chain(&h, 7), before);
    // double-click in Browser › Effects: the end of the current chain
    h.state_mut().br_load(&fx_item(FxKind::Compressor), &ctx);
    frames(&mut h, 2);
    assert_eq!(chain(&h, 7).last(), Some(&FxKind::Compressor));
}

#[test]
fn a_drop_position_over_the_row_maps_to_the_card_gaps() {
    let mut h = devices();
    h.state_mut().br.drag = Some(fx_item(FxKind::Delay));
    let (inst, cards) = (h.state().dev.inst, h.state().dev.cards.clone());
    let at = |h: &Harness<'static, App>, p: Pos2| h.state().br_drop_at(p);
    use browserview::DropAt;
    assert_eq!(at(&h, inst.center()), Some(DropAt::Device(0)));
    assert_eq!(
        at(&h, Pos2::new(cards[0].right() - 4.0, cards[0].center().y)),
        Some(DropAt::Device(1))
    );
    // an instrument over the row replaces the track's instrument
    let synth = crate::browser::builtin_items()
        .into_iter()
        .find(|i| i.key == "init:synth")
        .unwrap();
    h.state_mut().br.drag = Some(synth);
    assert_eq!(at(&h, inst.center()), Some(DropAt::Slot(7)));
    h.state_mut().br.drag = None;
}

#[test]
fn dragging_a_card_title_reorders_the_chain() {
    let mut h = devices();
    let before = chain(&h, 7);
    let c0 = h.state().dev.cards[0];
    let c1 = h.state().dev.cards[1];
    drag(
        &mut h,
        Pos2::new(c0.left() + 50.0, c0.top() + 12.0),
        Pos2::new(c1.right() - 10.0, c1.center().y),
    );
    let mut want = before.clone();
    want.swap(0, 1);
    assert_eq!(chain(&h, 7), want);
}

#[test]
fn devices_follow_the_selected_track() {
    let mut h = devices();
    let cur = h.state().ch;
    let other = (0..h.state().with(|s| s.song.channels))
        .find(|&c| {
            let s = h.state().track_slot(c);
            c != cur && s > 0 && s != 7
        })
        .expect("another track");
    let slot = h.state().track_slot(other);
    h.state_mut().ch = other;
    frames(&mut h, 2);
    assert_eq!(h.state().ch, other);
    assert_eq!(h.state().dev.follow_ch, other);
    assert_eq!(h.state().instr, slot);
    assert!(h
        .state()
        .devices_target_label()
        .contains(&format!("{slot:02X}")));
}

#[test]
fn every_device_kind_draws_in_the_row() {
    let mut h = devices();
    h.state().with(|s| {
        s.song.instruments[6].fx = FxKind::ALL.iter().map(|k| Device::new(*k)).collect();
    });
    frames(&mut h, 3);
    assert_eq!(h.state().dev.cards.len(), FxKind::ALL.len());
    // collapsed cards are slim
    h.state().with(|s| {
        for d in &mut s.song.instruments[6].fx {
            d.collapsed = true;
        }
    });
    frames(&mut h, 2);
    assert!(h.state().dev.cards.iter().all(|r| r.width() < 40.0));
    // Browser › Effects shows where a double-click adds
    h.state_mut().extra.browser_open = true;
    h.state_mut().br.filter.tab = Tab::Effects;
    frames(&mut h, 2);
    let label = h.state().devices_target_label();
    assert!(label.contains("07"), "{label}");
}

#[test]
fn an_eq_device_saves_and_loads_with_the_song() {
    let mut song = fileio::demo_song();
    let mut eq = Device::new(FxKind::Eq);
    eq.set_real("m1_gain", 6.0);
    song.instruments[0].fx.insert(0, eq.clone());
    let d = std::env::temp_dir().join(format!("cw-dev-eq-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("eq.cwt");
    fileio::save_cwt(&song, &p).unwrap();
    let back = fileio::load_cwt(&p).unwrap();
    assert_eq!(back.instruments[0].fx[0], eq);
    let _ = std::fs::remove_dir_all(&d);
}
