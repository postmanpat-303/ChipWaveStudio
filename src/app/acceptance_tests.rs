//! Racks acceptance tests (docs/racks/PLAN.md 8.2), one per acceptance
//! point, numbered 01-11, end to end through the App: the panel, the
//! chains, Splits, layers, moves, undo, saving, My Racks, missing plugins
//! and old songs. The scenario test builds a 2-layer rack with a Split,
//! saves it, loads it into a new App and checks it renders the same.
use super::chains::{RowKind, Sel, TreeDrop};
use super::chains_tests::{chain, chains_on, frames};
use super::fxview::FxOp;
use super::ui_tests::harness_with;
use super::*;
use crate::engine::racks_tests::{clip_song, diff, one_note_song, render};
use crate::fx::{DevPath, Device, FxKind};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn has(h: &Harness<'static, App>, label: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(label) || n.value().as_deref() == Some(label))
        .next()
        .is_some()
}

/// a two-note clip on `slot` (1-based) with the harness's instruments
fn heard(h: &Harness<'static, App>, slot: usize) -> Vec<f32> {
    let mut s = one_note_song();
    s.instruments = h.state().with(|x| x.song.instruments.clone());
    s.mixer.master = 0.05;
    s.mixer.limiter.enabled = false;
    render(&clip_song(s, slot as u8, &[37, 44]), 0.8)[8192..].to_vec()
}

fn racks_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cw-accept-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    crate::racks::TEST_DIR.with(|t| *t.borrow_mut() = Some(d.clone()));
    d
}

fn rows(h: &Harness<'static, App>) -> Vec<RowKind> {
    h.state()
        .dev
        .tree_rows
        .iter()
        .map(|(_, k)| k.clone())
        .collect()
}

#[test]
fn acceptance_01_selecting_an_instrument_opens_nested_device_chains_under_the_timeline() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().view = View::Timeline;
    h.state_mut().set_lower(3);
    frames(&mut h, 2);
    h.state_mut().instr = 7;
    h.state_mut().dev.follow_ch = h.state().ch;
    h.state_mut().show_selection_panel();
    frames(&mut h, 3);
    assert_eq!(h.state().view, View::Timeline, "no navigating away");
    assert!(h.state().extra.lower_dev && h.state().devices_visible());
    // it opens on the Rack row; the chain editor is a detail view of it
    assert!(
        !h.state().dev.tree && has(&h, "Rack") && h.state().dev.inst.width() > 0.0,
        "the panel on the Rack row"
    );
    h.state_mut().dev.tree = true;
    frames(&mut h, 2);
    assert!(has(&h, "‹ Back to Rack"), "the chain editor");
    assert_eq!(
        h.state().chains_root(),
        Some(6),
        "on the selected instrument"
    );
}

#[test]
fn acceptance_02_the_panel_shows_instrument_layers_and_effects_as_a_tree() {
    let mut h = chains_on(7);
    h.state_mut().make_rack(6);
    h.state_mut().add_layer(6, None);
    frames(&mut h, 3);
    let k = rows(&h);
    assert!(k.contains(&RowKind::Layers(6)));
    assert!(k.contains(&RowKind::Layer(6, 0)) && k.contains(&RowKind::Layer(6, 1)));
    assert!(
        k.iter().any(|r| matches!(r, RowKind::Effects(_))),
        "chains under the instrument"
    );
    // the rack's own chain ("Rack effects") comes after the layers
    let fx = k.iter().rposition(|r| *r == RowKind::Effects(6)).unwrap();
    let lb = k.iter().position(|r| *r == RowKind::Layer(6, 1)).unwrap();
    assert!(fx > lb, "Rack effects after the layers: {k:?}");
}

#[test]
fn acceptance_03_selecting_a_row_shows_its_real_controls_beside_the_tree() {
    let mut h = chains_on(7);
    assert!(
        has(&h, "Open full editor  Ctrl+I"),
        "the instrument's controls"
    );
    h.state_mut().dev.sel = Some(Sel::Dev(6, DevPath::from_slice(&[0]).unwrap()));
    frames(&mut h, 3);
    assert!(
        !has(&h, "Open full editor  Ctrl+I"),
        "now the effect's controls"
    );
    // a knob there edits the song (one undo step)
    let v0 = h.state().with(|s| s.song.instruments[6].fx[0].params[0]);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Param(0, 0, 0.2, true)]);
    frames(&mut h, 2);
    assert_ne!(
        h.state().with(|s| s.song.instruments[6].fx[0].params[0]),
        v0
    );
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(
        h.state().with(|s| s.song.instruments[6].fx[0].params[0]),
        v0
    );
}

#[test]
fn acceptance_04_add_bypass_reorder_and_remove_devices_and_hear_it() {
    let mut h = chains_on(7);
    let dry = heard(&h, 7);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Add(FxKind::Drive, None)]);
    frames(&mut h, 2);
    assert_eq!(
        chain(&h, 7),
        vec![FxKind::Filter, FxKind::Chorus, FxKind::Drive]
    );
    assert!(diff(&dry, &heard(&h, 7)) > 1e-4, "the Drive is heard");
    h.state_mut().apply_fx_ops(6, vec![FxOp::Toggle(2)]);
    frames(&mut h, 2);
    assert!(
        diff(&dry, &heard(&h, 7)) < 1e-6,
        "bypassed: the sound as before"
    );
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Toggle(2), FxOp::Move(2, 0)]);
    frames(&mut h, 2);
    assert_eq!(
        chain(&h, 7),
        vec![FxKind::Drive, FxKind::Filter, FxKind::Chorus]
    );
    h.state_mut().apply_fx_ops(6, vec![FxOp::Remove(0)]);
    frames(&mut h, 2);
    assert_eq!(chain(&h, 7), vec![FxKind::Filter, FxKind::Chorus]);
    assert!(diff(&dry, &heard(&h, 7)) < 1e-6, "removed: as before");
}

#[test]
fn acceptance_05_a_split_runs_parallel_paths_and_mixes_them_back() {
    let mut h = chains_on(7);
    let dry = heard(&h, 7);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Insert(usize::MAX, Box::new(Device::split()))]);
    frames(&mut h, 2);
    assert!(diff(&dry, &heard(&h, 7)) < 1e-6, "two empty paths: dry");
    h.state_mut().apply_fx_ops(
        6,
        vec![super::chains::wrap(&[2, 0], FxOp::Add(FxKind::Drive, None))],
    );
    frames(&mut h, 2);
    let a = heard(&h, 7);
    assert!(diff(&dry, &a) > 1e-4, "path A's Drive is heard");
    h.state_mut().apply_fx_ops(6, vec![FxOp::PathOn(2, 1)]);
    frames(&mut h, 2);
    assert!(diff(&a, &heard(&h, 7)) > 1e-4, "path B off changes the mix");
    assert!(
        rows(&h).iter().any(|r| matches!(r, RowKind::Path(..))),
        "paths in the tree"
    );
}

#[test]
fn acceptance_06_rack_layers_play_together_with_their_own_settings() {
    let mut h = chains_on(7);
    let solo = heard(&h, 7);
    h.state_mut().make_rack(6);
    frames(&mut h, 2);
    assert!(
        diff(&solo, &heard(&h, 7)) < 1e-6,
        "a 1-layer rack sounds the same"
    );
    h.state_mut().add_layer(6, Some(0));
    frames(&mut h, 2);
    let both = heard(&h, 7);
    assert!(diff(&solo, &both) > 1e-4, "layer B is heard with layer A");
    let mut l = h.state().with(|s| match &s.song.instruments[6].kind {
        InstrumentKind::Rack(r) => r.layers[1].clone(),
        _ => panic!(),
    });
    l.on = false;
    h.state_mut().set_layer(6, 1, l, true);
    frames(&mut h, 2);
    assert!(diff(&solo, &heard(&h, 7)) < 1e-6, "layer B off: only A");
}

#[test]
fn acceptance_07_devices_move_between_compatible_chains() {
    let mut h = chains_on(7);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Insert(usize::MAX, Box::new(Device::split()))]);
    frames(&mut h, 2);
    let before = heard(&h, 7);
    // the Chorus from the main chain into path B
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::from_slice(&[1]).unwrap()),
        to: (6, vec![2, 1], usize::MAX),
        copy: false,
    });
    frames(&mut h, 2);
    assert_eq!(chain(&h, 7), vec![FxKind::Filter, FxKind::Split]);
    let paths: Vec<Vec<FxKind>> = h.state().with(|s| {
        s.song.instruments[6].fx[1]
            .paths
            .iter()
            .map(|p| p.devices.iter().map(|d| d.kind).collect())
            .collect()
    });
    assert_eq!(paths[1], vec![FxKind::Chorus]);
    assert!(
        diff(&before, &heard(&h, 7)) > 1e-4,
        "now only path B has it"
    );
}

#[test]
fn acceptance_08_every_rack_and_device_change_undoes_and_redoes() {
    let mut h = chains_on(7);
    let start = h.state().with(|s| format!("{:?}", s.song.instruments));
    h.state_mut().make_rack(6);
    frames(&mut h, 2);
    h.state_mut().add_layer(6, None);
    frames(&mut h, 2);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Insert(usize::MAX, Box::new(Device::split()))]);
    frames(&mut h, 2);
    let end = h.state().with(|s| format!("{:?}", s.song.instruments));
    for _ in 0..3 {
        h.state_mut().undo(false);
        frames(&mut h, 2);
    }
    assert_eq!(
        h.state().with(|s| format!("{:?}", s.song.instruments)),
        start
    );
    for _ in 0..3 {
        h.state_mut().undo(true);
        frames(&mut h, 2);
    }
    assert_eq!(h.state().with(|s| format!("{:?}", s.song.instruments)), end);
}

/// a 2-layer rack in slot 7 with a Split (Drive | dry) on layer B
fn two_layer_rack_with_split() -> Harness<'static, App> {
    let mut h = chains_on(7);
    h.state_mut().make_rack(6);
    h.state_mut().add_layer(6, None);
    frames(&mut h, 2);
    let b = h.state().with(|s| match &s.song.instruments[6].kind {
        InstrumentKind::Rack(r) => r.layers[1].slot,
        _ => panic!(),
    });
    h.state_mut()
        .apply_fx_ops(b, vec![FxOp::Insert(usize::MAX, Box::new(Device::split()))]);
    let at = h.state().with(|s| s.song.instruments[b].fx.len() - 1) as u8;
    h.state_mut().apply_fx_ops(
        b,
        vec![super::chains::wrap(
            &[at, 0],
            FxOp::Add(FxKind::Drive, None),
        )],
    );
    frames(&mut h, 2);
    h
}

#[test]
fn acceptance_09_save_custom_rack_keeps_everything_and_loads_into_a_new_song_sounding_the_same() {
    let d = racks_dir("scenario");
    let h = two_layer_rack_with_split();
    let want = heard(&h, 7);
    let mut h = h;
    h.state_mut().open_save_rack(6);
    let mut f = h.state().dev.save_rack.clone().unwrap();
    f.name = "Split Keys".into();
    f.description = "two layers, one through a Split".into();
    f.sound_type = "Keys".into();
    f.tags = "split, layered".into();
    let path = h.state_mut().save_rack_now(&f, None).unwrap();
    assert_eq!(path, d.join("Split Keys.cwr"));
    let info = crate::racks::read_info(&path).unwrap().0;
    assert_eq!(info.sound_type, "Keys");
    assert_eq!(info.tags, vec!["split".to_string(), "layered".to_string()]);
    // a new App, a different song: load it onto track 1
    let mut g = harness_with(fileio::demo_song());
    frames(&mut g, 2);
    let slot = g.state_mut().load_rack(&path, Some(0)).unwrap();
    frames(&mut g, 2);
    let got = heard(&g, slot + 1);
    let dd = diff(&want, &got);
    assert!(dd < 1e-6, "the loaded rack sounds the same ({dd})");
    let (layers, split) = g.state().with(|s| match &s.song.instruments[slot].kind {
        InstrumentKind::Rack(r) => (
            r.layers.len(),
            s.song.instruments[r.layers[1].slot]
                .fx
                .iter()
                .any(|x| x.kind == FxKind::Split && x.paths[0].devices[0].kind == FxKind::Drive),
        ),
        _ => panic!(),
    });
    assert_eq!(layers, 2);
    assert!(split, "the nested Split and its Drive came back");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn acceptance_10_my_racks_browses_searches_and_loads_without_touching_other_tracks() {
    let d = racks_dir("browse");
    let src = two_layer_rack_with_split();
    let r = src
        .state()
        .with(|s| crate::racks::from_song(&s.song, 6, crate::racks::tests::info("Split Keys")))
        .unwrap();
    crate::racks::save(&r, &d.join("Split Keys.cwr")).unwrap();
    let mut h = chains_on(1);
    h.state_mut().extra.browser_open = true;
    h.state_mut().br.filter.tab = crate::browser::Tab::Racks;
    h.state_mut().dev.racks_dirty = true;
    frames(&mut h, 4);
    assert!(has(&h, "Split Keys"));
    h.state_mut().br.filter.query = "zzz".into();
    frames(&mut h, 2);
    assert!(!has(&h, "Split Keys"), "search filters");
    h.state_mut().br.filter.query.clear();
    frames(&mut h, 2);
    let before = h.state().with(|s| s.song.clone());
    let ch = h.state().ch;
    h.state_mut()
        .rack_action(&d.join("Split Keys.cwr"), "track");
    frames(&mut h, 2);
    let after = h.state().with(|s| s.song.clone());
    let n0 = before.instruments.len();
    assert_eq!(after.instruments.len(), n0 + 3);
    assert_eq!(
        &after.instruments[..n0],
        &before.instruments[..n0],
        "old slots untouched"
    );
    for (p, q) in after.patterns.iter().zip(&before.patterns) {
        for (row, old) in p.cells.iter().zip(&q.cells) {
            for (c, (x, o)) in row.iter().zip(old).enumerate() {
                if c != ch {
                    assert_eq!(x, o, "track {} untouched", c + 1);
                }
            }
        }
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn acceptance_11_old_songs_load_unchanged_and_save_for_older_readers() {
    for (i, (name, _, _)) in fileio::DEMOS.iter().enumerate() {
        let s = fileio::demo_song_n(i);
        assert!(
            !fileio::needs_reader_7(&s),
            "{name}: no rack or Split, no new reader"
        );
        assert!(
            s.instruments
                .iter()
                .all(|x| !matches!(x.kind, InstrumentKind::Rack(_))),
            "{name}"
        );
        let mut t = s.clone();
        t.fix_racks();
        assert_eq!(
            format!("{:?}", t),
            format!("{:?}", s),
            "{name}: fix_racks changes nothing"
        );
    }
    // the render fingerprints of every demo are checked against
    // /workspace/fp-base.txt by the gate (render_hashes)
}
