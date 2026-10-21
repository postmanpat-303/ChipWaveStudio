//! Racks in the Nested Device Chains panel (docs/racks/PLAN.md 4.7): make a
//! rack from an instrument (it sounds the same), add / remove layers, a
//! layer's settings, layer effects, the Instruments list. One undo step each.
use super::chains::{RowKind, Sel};
use super::chains_tests::{chain, chains_on, frames};
use super::fxview::FxOp;
use super::ui_tests::{click_at, exact};
use super::*;
use crate::engine::racks_tests::{clip_song, diff, one_note_song, render};
use crate::fx::{Device, FxKind};
use crate::model::Layer;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn has(h: &Harness<'static, App>, label: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(label) || n.value().as_deref() == Some(label))
        .next()
        .is_some()
}

fn layers(h: &Harness<'static, App>, rack: usize) -> Vec<Layer> {
    h.state().with(|s| match &s.song.instruments[rack].kind {
        InstrumentKind::Rack(r) => r.layers.clone(),
        _ => vec![],
    })
}

/// note clip of `keys` on slot `slot` (1-based), with the harness's
/// instruments, quiet and without the limiter
fn heard(h: &Harness<'static, App>, slot: u8) -> Vec<f32> {
    let mut s = one_note_song();
    s.instruments = h.state().with(|x| x.song.instruments.clone());
    s.mixer.master = 0.05;
    s.mixer.limiter.enabled = false;
    render(&clip_song(s, slot, &[37, 44]), 0.8)
}

/// instrument 7 made a rack from the panel
fn rack7() -> Harness<'static, App> {
    let mut h = chains_on(7);
    let b = exact(&h, "Make a rack from this instrument");
    click_at(&mut h, b.center());
    frames(&mut h, 3);
    h
}

#[test]
fn make_a_rack_keeps_the_sound_the_effects_and_is_one_undo_step() {
    let mut h = chains_on(7);
    let before = heard(&h, 7);
    let n0 = h.state().with(|s| s.song.instruments.len());
    let fx0 = chain(&h, 7);
    let b = exact(&h, "Make a rack from this instrument");
    click_at(&mut h, b.center());
    frames(&mut h, 3);
    let l = layers(&h, 6);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].slot, n0, "the sound moved to a new slot");
    assert_eq!(chain(&h, 7), fx0, "the rack keeps the effects");
    assert_eq!(
        h.state().dev.sel,
        Some(Sel::Layer(6, 0)),
        "layer A is selected"
    );
    // it sounds the same
    let after = heard(&h, 7);
    let d = diff(&before[8192..], &after[8192..]);
    assert!(d < 1e-6, "same sound ({d})");
    // the tree: Layers, Layer A, its effects, Rack effects
    assert!(h.query_by_label_contains("LAYER").is_some());
    let kinds: Vec<RowKind> = h
        .state()
        .dev
        .tree_rows
        .iter()
        .map(|(_, k)| k.clone())
        .collect();
    assert!(kinds.contains(&RowKind::Layers(6)));
    assert!(kinds.contains(&RowKind::Layer(6, 0)));
    assert!(kinds.contains(&RowKind::Effects(n0)));
    assert!(kinds.contains(&RowKind::Effects(6)));
    // one undo step back to the plain instrument
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert!(layers(&h, 6).is_empty());
    assert_eq!(h.state().with(|s| s.song.instruments.len()), n0);
    // a rack, a layer or a plugin instrument can't become a rack
    let mut h = rack7();
    let layer = n0;
    h.state_mut().make_rack(6);
    assert!(h.state().status.contains("it is a rack already"));
    h.state_mut().make_rack(layer);
    assert!(h.state().status.contains("it is a layer of a rack"));
}

#[test]
fn add_layers_new_or_existing_up_to_four_and_remove_one() {
    let mut h = rack7();
    let n0 = h.state().with(|s| s.song.instruments.len());
    h.state_mut().dev.sel = Some(Sel::Inst(6));
    frames(&mut h, 2);
    let b = exact(&h, "+ Add layer");
    click_at(&mut h, b.center());
    frames(&mut h, 2);
    let l = layers(&h, 6);
    assert_eq!(l.len(), 2);
    assert_eq!(l[1].slot, n0, "a new synth slot");
    assert_eq!(h.state().dev.sel, Some(Sel::Layer(6, 1)));
    // an existing instrument (slot 1), then refusals
    h.state_mut().add_layer(6, Some(0));
    assert_eq!(layers(&h, 6)[2].slot, 0);
    h.state_mut().add_layer(6, Some(0));
    assert!(h.state().status.contains("already"), "{}", h.state().status);
    h.state_mut().add_layer(6, Some(6));
    assert!(h.state().status.contains("rack can't go inside a rack"));
    h.state_mut().add_layer(6, None);
    assert_eq!(layers(&h, 6).len(), 4);
    h.state_mut().add_layer(6, None);
    assert_eq!(layers(&h, 6).len(), 4, "at most 4");
    assert!(h.state().status.contains("at most 4"));
    // the rack now plays more than the old sound
    // remove layer B from its detail: its instrument stays
    h.state_mut().dev.sel = Some(Sel::Layer(6, 1));
    frames(&mut h, 2);
    let b = exact(&h, "Remove layer");
    click_at(&mut h, b.center());
    frames(&mut h, 2);
    assert_eq!(layers(&h, 6).len(), 3);
    assert!(
        h.state().with(|s| s.song.instruments.len()) > n0,
        "the instrument stays"
    );
    h.state_mut().do_undo(false);
    frames(&mut h, 1);
    assert_eq!(layers(&h, 6).len(), 4, "one undo step");
}

#[test]
fn layer_settings_are_heard_and_each_is_one_undo_step() {
    let mut h = rack7();
    let base = heard(&h, 7);
    assert!(has(&h, "On"));
    assert!(h.query_by_label_contains("KEY RANGE").is_some());
    assert!(h.query_all_by_label_contains("Transpose").next().is_some());
    // mute layer A with its checkbox: silence
    let on = exact(&h, "On");
    click_at(&mut h, on.center());
    frames(&mut h, 2);
    assert!(!layers(&h, 6)[0].on);
    assert!(heard(&h, 7).iter().all(|x| *x == 0.0));
    h.state_mut().do_undo(false);
    frames(&mut h, 1);
    assert!(layers(&h, 6)[0].on);
    // level, key range, transpose: heard; a drag is one step
    let mut l = layers(&h, 6)[0].clone();
    l.level_db = -12.0;
    h.state_mut().set_layer(6, 0, l.clone(), true);
    l.level_db = -18.0;
    h.state_mut().set_layer(6, 0, l.clone(), false);
    assert!(diff(&heard(&h, 7)[8192..], &base[8192..]) > 1e-5);
    h.state_mut().do_undo(false);
    frames(&mut h, 1);
    assert_eq!(layers(&h, 6)[0].level_db, 0.0);
    let mut l = layers(&h, 6)[0].clone();
    l.key_lo = 40;
    h.state_mut().set_layer(6, 0, l, true);
    let hi_only = heard(&h, 7);
    // key 37 is out of range now: only the second note (key 44) sounds
    assert!(hi_only[..20000].iter().all(|x| *x == 0.0));
    assert!(hi_only.iter().any(|x| x.abs() > 1e-4));
}

#[test]
fn a_layer_has_its_own_effects_and_add_device_goes_there() {
    let mut h = rack7();
    let layer = layers(&h, 6)[0].slot;
    let base = heard(&h, 7);
    // + Add Device with layer A selected: the end of the layer's chain
    h.state_mut().add_device_here();
    frames(&mut h, 2);
    let a = h.state().dev.add.clone().expect("Add Device opens");
    assert_eq!(a.inst, layer);
    h.state_mut().apply_fx_ops(
        layer,
        vec![FxOp::Insert(
            usize::MAX,
            Box::new(Device::new(FxKind::Drive)),
        )],
    );
    h.state_mut().dev.add = None;
    frames(&mut h, 2);
    assert_eq!(chain(&h, layer as u8 + 1), vec![FxKind::Drive]);
    assert!(
        diff(&heard(&h, 7)[8192..], &base[8192..]) > 1e-5,
        "the layer's Drive is heard"
    );
    let kinds: Vec<RowKind> = h
        .state()
        .dev
        .tree_rows
        .iter()
        .map(|(_, k)| k.clone())
        .collect();
    assert!(kinds
        .iter()
        .any(|k| matches!(k, RowKind::Dev(i, _) if *i == layer)));
    // the rack's own chain is unchanged
    assert_eq!(chain(&h, 7), vec![FxKind::Filter, FxKind::Chorus]);
}

#[test]
fn layers_sit_under_their_rack_in_the_instruments_list_and_open_the_rack() {
    let mut h = rack7();
    let layer = layers(&h, 6)[0].slot;
    frames(&mut h, 2);
    assert!(
        h.query_all_by(|n| n.label().is_some_and(|l| l.contains("(layer of 07)")))
            .next()
            .is_some(),
        "the layer row names its rack"
    );
    // the layer row comes right after the rack's row
    let y = |prefix: String| {
        h.query_all_by(|n| {
            n.role() == egui::accesskit::Role::ListBoxOption
                && n.label().is_some_and(|l| l.starts_with(&prefix))
        })
        .next()
        .map(|n| n.rect().top())
    };
    let (r, l, next) = (
        y("07 ".into()),
        y(format!("{:02X} ", layer + 1)),
        y("08 ".into()),
    );
    assert!(r.is_some() && l.is_some() && next.is_some());
    assert!(l.unwrap() > r.unwrap() && l.unwrap() < next.unwrap());
    // picking the layer elsewhere: the rack's tree with that layer selected
    h.state_mut().instr = layer as u8 + 1;
    frames(&mut h, 3);
    assert_eq!(h.state().chains_root(), Some(6));
    assert_eq!(h.state().dev.sel, Some(Sel::Layer(6, 0)));
}
