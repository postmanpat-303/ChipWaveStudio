//! Nested Device Chains (docs/racks/PLAN.md): Stage 1 (selecting opens
//! the panel, its layout is remembered) and Stage 2 (the instrument and
//! its effects as a tree, the selected row's real controls beside it).
use super::chains::{self, RowKind, Sel};
use super::fxview::FxOp;
use super::ui_tests::{click_at, exact, frame, harness_with, key};
use super::*;
use crate::fx::{DevPath, Device, FxKind};
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

pub(super) fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

pub(super) fn chain(h: &Harness<'static, App>, slot: u8) -> Vec<FxKind> {
    h.state().with(|s| {
        s.song.instruments[slot as usize - 1]
            .fx
            .iter()
            .map(|d| d.kind)
            .collect()
    })
}

/// the demo with Nested Device Chains showing instrument 7 (Supersaw Pad:
/// filter + chorus)
pub(super) fn chains_on(slot: u8) -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    key(&mut h, Key::Num7, egui::Modifiers::COMMAND);
    h.state_mut().instr = slot;
    h.state_mut().dev.follow_ch = h.state().ch;
    // these tests drive the tree (Cards is the default since 22 Oct 2026)
    h.state_mut().dev.tree = true;
    frames(&mut h, 3);
    h
}

fn has(h: &Harness<'static, App>, label: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(label) || n.value().as_deref() == Some(label))
        .next()
        .is_some()
}

#[test]
fn the_chain_editor_is_a_detail_view_of_the_rack() {
    let mut h = chains_on(7);
    assert!(h.state().devices_visible());
    assert!(has(&h, "Rack"), "the Notes | Rack toggle");
    assert!(!has(&h, "Nested Device Chains"), "the old name is gone");
    assert!(
        !has(&h, "Cards") && !has(&h, "Fold tree"),
        "no view toggles"
    );
    assert!(
        h.query_by_label("Devices").is_none(),
        "no second device view"
    );
    assert!(has(&h, "+ Add Device"));
    let r = exact(&h, "‹ Back to Rack");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    assert!(!h.state().dev.tree && !h.state().dev.vertical);
    assert_eq!(h.state().dev.cards.len(), chain(&h, 7).len());
}

#[test]
fn old_settings_select_chains_and_remember_the_tree() {
    // a 0.2.6 settings file has none of the new keys
    let e: crate::settings::Extra = toml::from_str("lower_dev = true").unwrap();
    assert!(e.lower_dev && e.select_chains && e.chains_cards && !e.chains_view_chosen);
    assert_eq!(e.chains_tree_w, 370.0);
    // the layout round-trips
    let mut s = crate::settings::Settings::default();
    s.extra.chains_tree_w = 300.0;
    s.extra.chains_tree_folded = true;
    s.extra.chains_closed = vec!["e6".into()];
    let back: crate::settings::Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.extra.chains_tree_w, 300.0);
    assert!(back.extra.chains_tree_folded);
    assert_eq!(back.extra.chains_closed, vec!["e6".to_string()]);
}

#[test]
fn selecting_a_track_opens_chains_unless_settings_say_instrument() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().set_lower(3);
    frames(&mut h, 2);
    h.state_mut().show_selection_panel();
    frames(&mut h, 2);
    assert!(h.state().extra.lower_dev && !h.state().extra.lower_inst);
    assert!(h.state().devices_visible());
    // Settings › Follow the Timeline selection › ... off: the Instrument tab
    h.state_mut().extra.select_chains = false;
    h.state_mut().show_selection_panel();
    frames(&mut h, 2);
    assert!(h.state().extra.lower_inst && !h.state().extra.lower_dev);
}

#[test]
fn the_tree_lists_the_instrument_then_its_effects_in_order() {
    let mut song = fileio::demo_song();
    song.instruments[0].fx.clear();
    let rows = chains::tree_rows(&song, 0, &[]);
    assert!(matches!(rows[0].kind, RowKind::Inst(0)));
    assert!(matches!(rows[1].kind, RowKind::Effects(0)));
    assert!(matches!(rows[2].kind, RowKind::Note(_)), "none yet");
    song.instruments[0].fx = vec![Device::new(FxKind::Eq)];
    let rows = chains::tree_rows(&song, 0, &[]);
    assert_eq!(rows.len(), 3);
    assert!(matches!(rows[2].kind, RowKind::Dev(0, p) if p == DevPath::top(0)));
    let six = [
        FxKind::Eq,
        FxKind::Drive,
        FxKind::Filter,
        FxKind::Chorus,
        FxKind::Delay,
        FxKind::Reverb,
    ];
    song.instruments[0].fx = six.iter().map(|k| Device::new(*k)).collect();
    let rows = chains::tree_rows(&song, 0, &[]);
    let devs: Vec<usize> = rows
        .iter()
        .filter_map(|r| match r.kind {
            RowKind::Dev(0, p) => Some(p.index()),
            _ => None,
        })
        .collect();
    assert_eq!(devs, (0..6).collect::<Vec<_>>());
    // a folded chain hides its rows; folding the instrument hides all
    let rows = chains::tree_rows(&song, 0, &["e0".into()]);
    assert_eq!(rows.len(), 2);
    let rows = chains::tree_rows(&song, 0, &["i0".into()]);
    assert_eq!(rows.len(), 1);
}

#[test]
fn clicking_rows_switches_the_detail_and_params_edit_in_one_undo_step() {
    let mut h = chains_on(7);
    assert_eq!(
        h.state().dev.sel,
        Some(Sel::Inst(6)),
        "the instrument first"
    );
    assert!(has(&h, "Open full editor  Ctrl+I"));
    // the first effect row
    let (r, _) = h
        .state()
        .dev
        .tree_rows
        .iter()
        .find(|(_, k)| matches!(k, RowKind::Dev(6, p) if p.index() == 0))
        .cloned()
        .expect("effect row drawn");
    click_at(&mut h, r.center() + egui::vec2(40.0, 0.0));
    frames(&mut h, 2);
    assert_eq!(h.state().dev.sel, Some(Sel::Dev(6, DevPath::top(0))));
    assert!(has(&h, "Bypass") && has(&h, "Duplicate") && has(&h, "Remove"));
    assert!(h.query_by_label_contains("ALL PARAMETERS").is_some());
    // a parameter change (as the knobs / rows send it) is one undo step
    let before = h
        .state()
        .with(|s| s.song.instruments[6].fx[0].params.clone());
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Param(0, 1, 0.9, true)]);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Param(0, 1, 0.8, false)]);
    let mid = h.state().with(|s| s.song.instruments[6].fx[0].params[1]);
    assert_eq!(mid, 0.8);
    h.state_mut().do_undo(false);
    frames(&mut h, 1);
    assert_eq!(
        h.state()
            .with(|s| s.song.instruments[6].fx[0].params.clone()),
        before
    );
    // Bypass in the detail header bypasses it for real
    let b = exact(&h, "Bypass");
    click_at(&mut h, b.center());
    frames(&mut h, 2);
    assert!(!h.state().with(|s| s.song.instruments[6].fx[0].enabled));
}

#[test]
fn the_chain_editor_keeps_three_timeline_rows_at_laptop_size() {
    // 1366 x 768 on the Timeline: three track rows stay above the panel
    let mut h = super::unified_tests::tl_demo();
    h.state_mut().instr = 7;
    h.state_mut().dev.tree = true;
    h.state_mut().set_lower(4);
    super::unified_tests::laptop(&mut h);
    assert!(h.state().devices_visible() && h.state().view == View::Timeline);
    let rows = h.state().tlv.lanes.height() / h.state().tlv.track_h_pts();
    assert!(rows >= 2.95, "{rows}");
    assert!(has(&h, "Open full editor  Ctrl+I"), "the detail fits");
    assert!(has(&h, "‹ Back to Rack"));
}

#[test]
fn dragging_a_tree_row_reorders_the_chain() {
    let mut h = chains_on(7);
    let before = chain(&h, 7);
    assert!(before.len() >= 2);
    let rows: Vec<Rect> = h
        .state()
        .dev
        .tree_rows
        .iter()
        .filter(|(_, k)| matches!(k, RowKind::Dev(6, _)))
        .map(|(r, _)| *r)
        .collect();
    let from = rows[0].center() + egui::vec2(60.0, 0.0);
    let to = Pos2::new(from.x, rows[rows.len() - 1].bottom() + 4.0);
    super::ui_tests::drag(&mut h, from, to);
    frames(&mut h, 2);
    let mut want = before.clone();
    let first = want.remove(0);
    want.push(first);
    assert_eq!(chain(&h, 7), want);
}
