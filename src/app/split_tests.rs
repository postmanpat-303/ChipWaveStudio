//! Nested Device Chains Stage 3 (docs/racks/PLAN.md): Add Device puts the
//! device where it was asked for and it is heard; Split paths in the tree
//! and detail; devices move or copy between chains; what may not go inside
//! a Split is refused with a reason. Every change is one undo step.
use super::chains::{AddPick, RowKind, Sel, TreeDrop};
use super::chains_tests::{chain, chains_on, frames};
use super::fxview::FxOp;
use super::ui_tests::{click_at, exact, key};
use super::*;
use crate::engine::racks_tests::{diff, one_note_song, render};
use crate::fx::{self, DevPath, Device, FxKind};
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn has(h: &Harness<'static, App>, label: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(label) || n.value().as_deref() == Some(label))
        .next()
        .is_some()
}

/// what instrument `slot` sounds like on its own, through its chain
fn heard(h: &Harness<'static, App>, slot: u8) -> Vec<f32> {
    let mut s = one_note_song();
    s.instruments[0].fx = h
        .state()
        .with(|x| x.song.instruments[slot as usize - 1].fx.clone());
    render(&s, 0.4)
}

fn split_paths(h: &Harness<'static, App>, slot: u8, at: usize) -> Vec<Vec<FxKind>> {
    h.state().with(|s| {
        s.song.instruments[slot as usize - 1].fx[at]
            .paths
            .iter()
            .map(|p| p.devices.iter().map(|d| d.kind).collect())
            .collect()
    })
}

/// instrument 7 (Filter, Chorus) with a Split at the end: [Filter, Chorus, Split]
fn with_split() -> Harness<'static, App> {
    let mut h = chains_on(7);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Insert(usize::MAX, Box::new(Device::split()))]);
    frames(&mut h, 2);
    assert_eq!(
        chain(&h, 7),
        vec![FxKind::Filter, FxKind::Chorus, FxKind::Split]
    );
    h
}

#[test]
fn add_device_puts_it_where_asked_and_it_is_heard() {
    let mut h = chains_on(7);
    let before = chain(&h, 7);
    let dry = heard(&h, 7);
    // the header button opens the window for the end of the chain
    let b = exact(&h, "+ Add Device");
    click_at(&mut h, b.center());
    frames(&mut h, 2);
    assert!(h.state().dev.add.is_some());
    assert!(h.query_by_label_contains("ADD DEVICE").is_some());
    assert!(
        has(&h, "Split · parallel paths"),
        "the Split container is offered"
    );
    h.state_mut().dev.add = None;
    frames(&mut h, 1);
    // between Filter and Chorus: search "drive", Enter adds the match
    h.state_mut().open_add_device(6, vec![], 1);
    frames(&mut h, 2);
    h.input_mut().events.push(egui::Event::Text("drive".into()));
    frames(&mut h, 2);
    assert_eq!(
        h.state().dev.add.as_ref().unwrap().pick,
        Some(AddPick::Kind(FxKind::Drive))
    );
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().dev.add.is_none(), "the window closes");
    assert_eq!(
        chain(&h, 7),
        vec![FxKind::Filter, FxKind::Drive, FxKind::Chorus]
    );
    assert_eq!(
        h.state().dev.sel,
        Some(Sel::Dev(6, DevPath::top(1))),
        "the new device is selected"
    );
    assert!(diff(&dry, &heard(&h, 7)) > 1e-4, "the new device is heard");
    // one undo step takes it out again, and the sound is back bit for bit
    h.state_mut().do_undo(false);
    frames(&mut h, 1);
    assert_eq!(chain(&h, 7), before);
    assert_eq!(dry, heard(&h, 7));
    // Esc closes without adding
    h.state_mut().open_add_device(6, vec![], 0);
    frames(&mut h, 2);
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().dev.add.is_none());
    assert_eq!(chain(&h, 7), before);
}

#[test]
fn a_new_split_shows_two_dry_paths_and_does_not_change_the_sound() {
    let mut h = chains_on(7);
    let dry = heard(&h, 7);
    h.state_mut().open_add_device(6, vec![], usize::MAX);
    frames(&mut h, 2);
    let b = exact(&h, "Split · parallel paths");
    click_at(&mut h, b.center());
    frames(&mut h, 1);
    let b = exact(&h, "Add  (Enter)");
    click_at(&mut h, b.center());
    frames(&mut h, 3);
    assert_eq!(
        chain(&h, 7),
        vec![FxKind::Filter, FxKind::Chorus, FxKind::Split]
    );
    assert_eq!(split_paths(&h, 7, 2), vec![vec![], vec![]]);
    assert!(
        diff(&dry, &heard(&h, 7)) < 1e-4,
        "two empty paths at -6 dB = dry"
    );
    // the tree shows its paths, labelled
    let paths: Vec<usize> = h
        .state()
        .dev
        .tree_rows
        .iter()
        .filter_map(|(_, k)| match k {
            RowKind::Path(6, sp, p) if *sp == DevPath::top(2) => Some(*p),
            _ => None,
        })
        .collect();
    assert_eq!(paths, vec![0, 1]);
    assert!(h.query_by_label_contains("Path A").is_some());
    assert!(h.query_by_label_contains("Path B").is_some());
    assert!(h.query_all_by_label("+ Add path").next().is_some());
    let flow = h.state().with(|s| chains::flow_text(&s.song, 6));
    assert!(flow.contains("Split [A: dry | B: dry]"), "{flow}");
}

#[test]
fn path_controls_mute_level_pan_and_remove_each_one_undo_step() {
    let mut h = with_split();
    let base = heard(&h, 7);
    // a device in path A, through the window aimed at that path
    h.state_mut().open_add_device(6, vec![2, 0], usize::MAX);
    frames(&mut h, 2);
    h.input_mut().events.push(egui::Event::Text("drive".into()));
    frames(&mut h, 2);
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert_eq!(split_paths(&h, 7, 2), vec![vec![FxKind::Drive], vec![]]);
    let with_a = heard(&h, 7);
    let fx_a = h.state().with(|s| s.song.instruments[6].fx.clone());
    assert!(diff(&base, &with_a) > 1e-4);
    // select path A: its controls
    h.state_mut().dev.sel = Some(Sel::Path(6, DevPath::top(2), 0));
    frames(&mut h, 2);
    assert!(has(&h, "On"));
    assert!(h.query_all_by_label_contains("Level").next().is_some());
    assert!(h.query_all_by_label_contains("Pan").next().is_some());
    assert!(!has(&h, "Remove path"), "a Split keeps at least 2 paths");
    // muting path A for real (the checkbox)
    let on = exact(&h, "On");
    click_at(&mut h, on.center());
    frames(&mut h, 2);
    assert!(!h.state().with(|s| s.song.instruments[6].fx[2].paths[0].on));
    assert!(diff(&heard(&h, 7), &with_a) > 1e-4, "muting is heard");
    h.state_mut().do_undo(false);
    frames(&mut h, 1);
    assert!(h.state().with(|s| s.song.instruments[6].fx[2].paths[0].on));
    assert_eq!(h.state().with(|s| s.song.instruments[6].fx.clone()), fx_a);
    assert_eq!(heard(&h, 7), with_a);
    // level and pan: a drag is one step
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::PathLevel(2, 0, -12.0, true)]);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::PathLevel(2, 0, -18.0, false)]);
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::PathPan(2, 1, -1.0, true)]);
    let p = h
        .state()
        .with(|s| s.song.instruments[6].fx[2].paths.clone());
    assert_eq!((p[0].level_db, p[1].pan), (-18.0, -1.0));
    h.state_mut().do_undo(false);
    h.state_mut().do_undo(false);
    frames(&mut h, 1);
    let p = h
        .state()
        .with(|s| s.song.instruments[6].fx[2].paths.clone());
    assert!((p[0].level_db - fx::NEW_PATH_DB).abs() < 1e-4 && p[1].pan == 0.0);
    // a third path (the Split's own detail), then Remove path
    h.state_mut().dev.sel = Some(Sel::Dev(6, DevPath::top(2)));
    frames(&mut h, 2);
    assert!(h.query_by_label_contains("PATHS").is_some());
    // the link on the Split's tree row (the leftmost of the two)
    let b = h
        .query_all_by_label("+ Add path")
        .map(|n| n.rect())
        .min_by(|a, b| a.left().total_cmp(&b.left()))
        .expect("an Add path link");
    click_at(&mut h, b.center());
    frames(&mut h, 2);
    assert_eq!(split_paths(&h, 7, 2).len(), 3);
    h.state_mut().dev.sel = Some(Sel::Path(6, DevPath::top(2), 2));
    frames(&mut h, 2);
    let b = exact(&h, "Remove path");
    click_at(&mut h, b.center());
    frames(&mut h, 2);
    assert_eq!(split_paths(&h, 7, 2).len(), 2);
    // never more than 4
    for _ in 0..5 {
        h.state_mut().apply_fx_ops(6, vec![FxOp::AddPath(2)]);
    }
    assert_eq!(split_paths(&h, 7, 2).len(), fx::MAX_PATHS);
}

#[test]
fn moving_a_device_into_a_path_changes_which_path_processes_it() {
    let mut h = with_split();
    // path B quieter, so where the Chorus sits is heard
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::PathLevel(2, 1, -24.0, true)]);
    h.state_mut().apply_fx_ops(
        6,
        vec![chains::wrap(
            &[2, 0],
            FxOp::Insert(0, Box::new(Device::new(FxKind::Drive))),
        )],
    );
    let start = heard(&h, 7);
    // Drive: path A -> path B
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::from_slice(&[2, 0, 0]).unwrap()),
        to: (6, vec![2, 1], usize::MAX),
        copy: false,
    });
    frames(&mut h, 2);
    assert_eq!(split_paths(&h, 7, 2), vec![vec![], vec![FxKind::Drive]]);
    assert!(diff(&start, &heard(&h, 7)) > 1e-4, "now path B drives it");
    assert!(
        h.state()
            .status
            .starts_with("Moved Drive to Split › Path B"),
        "{}",
        h.state().status
    );
    // one undo step
    h.state_mut().do_undo(false);
    frames(&mut h, 1);
    assert_eq!(split_paths(&h, 7, 2), vec![vec![FxKind::Drive], vec![]]);
    assert_eq!(heard(&h, 7), start);
    // the Filter from the main chain into path B: it leaves the main chain
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::top(0)),
        to: (6, vec![2, 1], 0),
        copy: false,
    });
    frames(&mut h, 2);
    assert_eq!(chain(&h, 7), vec![FxKind::Chorus, FxKind::Split]);
    assert_eq!(
        split_paths(&h, 7, 1),
        vec![vec![FxKind::Drive], vec![FxKind::Filter]]
    );
    // and back out to the start of the main chain
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::from_slice(&[1, 1, 0]).unwrap()),
        to: (6, vec![], 0),
        copy: false,
    });
    frames(&mut h, 2);
    assert_eq!(
        chain(&h, 7),
        vec![FxKind::Filter, FxKind::Chorus, FxKind::Split]
    );
    assert_eq!(split_paths(&h, 7, 2), vec![vec![FxKind::Drive], vec![]]);
}

#[test]
fn ctrl_drop_copies_and_keeps_the_original() {
    let mut h = with_split();
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::top(1)),
        to: (6, vec![2, 0], 0),
        copy: true,
    });
    frames(&mut h, 2);
    assert_eq!(
        chain(&h, 7),
        vec![FxKind::Filter, FxKind::Chorus, FxKind::Split]
    );
    assert_eq!(split_paths(&h, 7, 2), vec![vec![FxKind::Chorus], vec![]]);
    // the copy is its own device: changing it leaves the original alone
    h.state_mut().apply_fx_ops(
        6,
        vec![chains::wrap(&[2, 0], FxOp::Param(0, 0, 0.99, true))],
    );
    let (a, b) = h.state().with(|s| {
        let fx = &s.song.instruments[6].fx;
        (fx[1].params[0], fx[2].paths[0].devices[0].params[0])
    });
    assert!(b == 0.99 && a != 0.99);
    assert!(h.state().status.starts_with("Copied Chorus"));
    // the Move or copy buttons in the detail do the same
    h.state_mut().dev.sel = Some(Sel::Dev(6, DevPath::top(0)));
    frames(&mut h, 2);
    assert!(h.query_by_label_contains("MOVE OR COPY").is_some());
    // a copy within the same chain (Ctrl-drag onto its own chain)
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::top(0)),
        to: (6, vec![], usize::MAX),
        copy: true,
    });
    assert_eq!(
        chain(&h, 7),
        vec![
            FxKind::Filter,
            FxKind::Chorus,
            FxKind::Split,
            FxKind::Filter
        ]
    );
}

#[test]
fn what_may_not_go_inside_a_split_is_refused_with_a_reason() {
    let mut h = with_split();
    let fx0 = h.state().with(|s| s.song.instruments[6].fx.clone());
    // a plugin effect (D7)
    let mut plug = Device::new(FxKind::Plugin);
    plug.enabled = true;
    h.state_mut()
        .apply_fx_ops(6, vec![FxOp::Insert(0, Box::new(plug))]);
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::top(0)),
        to: (6, vec![3, 0], 0),
        copy: false,
    });
    assert!(
        h.state().status.contains("can't go inside a Split yet"),
        "{}",
        h.state().status
    );
    assert_eq!(split_paths(&h, 7, 3), vec![vec![], vec![]]);
    let mut c = h.state().with(|s| s.song.instruments[6].fx.clone());
    let e = super::fxview::apply_local(
        &mut c[3].paths[0].devices,
        FxOp::Insert(0, Box::new(Device::new(FxKind::Plugin))),
        1,
    );
    assert!(e.contains("can't go inside a Split"), "{e}");
    h.state_mut().do_undo(false);
    assert_eq!(h.state().with(|s| s.song.instruments[6].fx.clone()), fx0);
    // a Split into itself
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::top(2)),
        to: (6, vec![2, 0], 0),
        copy: false,
    });
    assert_eq!(h.state().status, "A Split can't go inside itself");
    // a third level of Splits
    h.state_mut().apply_fx_ops(
        6,
        vec![chains::wrap(
            &[2, 0],
            FxOp::Insert(0, Box::new(Device::split())),
        )],
    );
    let n = h
        .state()
        .with(|s| s.song.instruments[6].fx[2].paths[0].devices.len());
    assert_eq!(n, 1, "a Split inside a Split is fine");
    let before = h.state().with(|s| s.song.instruments[6].fx.clone());
    h.state_mut().apply_fx_ops(
        6,
        vec![chains::wrap(
            &[2, 0, 0, 0],
            FxOp::Insert(0, Box::new(Device::split())),
        )],
    );
    assert_eq!(h.state().with(|s| s.song.instruments[6].fx.clone()), before);
    // the Add Device window greys the Split there
    h.state_mut().open_add_device(6, vec![2, 0, 0, 0], 0);
    frames(&mut h, 2);
    assert_ne!(
        h.state().dev.add.as_ref().unwrap().pick,
        Some(AddPick::Kind(FxKind::Split))
    );
    // a full chain
    let fx16: Vec<Device> = (0..fx::MAX_DEVICES)
        .map(|_| Device::new(FxKind::Eq))
        .collect();
    h.state_mut()
        .with(|s| s.song.instruments[6].fx[2].paths[1].devices = fx16);
    h.state_mut().chains_drop(TreeDrop {
        from: (6, DevPath::top(0)),
        to: (6, vec![2, 1], 0),
        copy: false,
    });
    assert_eq!(h.state().status, "That chain is full (16 devices)");
}

#[test]
fn nested_devices_hide_automation_handles_and_say_why() {
    let mut h = with_split();
    h.state_mut().apply_fx_ops(
        6,
        vec![chains::wrap(
            &[2, 0],
            FxOp::Insert(0, Box::new(Device::new(FxKind::Filter))),
        )],
    );
    h.state_mut().dev.sel = Some(Sel::Dev(6, DevPath::from_slice(&[2, 0, 0]).unwrap()));
    frames(&mut h, 2);
    assert!(h
        .query_by_label_contains("Automation lanes: devices in the main chain")
        .is_some());
    assert!(h.query_by_label_contains("ALL PARAMETERS").is_some());
    // a main-chain device has no such note
    h.state_mut().dev.sel = Some(Sel::Dev(6, DevPath::top(0)));
    frames(&mut h, 2);
    assert!(h
        .query_by_label_contains("Automation lanes: devices in the main chain")
        .is_none());
}

#[test]
fn a_song_with_a_split_saves_loads_and_plays_the_same() {
    let mut h = with_split();
    h.state_mut().apply_fx_ops(
        6,
        vec![chains::wrap(
            &[2, 1],
            FxOp::Insert(0, Box::new(Device::new(FxKind::Reverb))),
        )],
    );
    let song = h.state().with(|s| s.song.clone());
    let dir = std::env::temp_dir().join(format!("cw-split-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("split.cwt");
    fileio::save_cwt(&song, &f).unwrap();
    let back = fileio::load_cwt(&f).unwrap();
    assert_eq!(back.instruments[6].fx, song.instruments[6].fx);
    let mut a = one_note_song();
    a.instruments[0].fx = song.instruments[6].fx.clone();
    let mut b = one_note_song();
    b.instruments[0].fx = back.instruments[6].fx.clone();
    assert_eq!(render(&a, 0.3), render(&b, 0.3));
    std::fs::remove_dir_all(&dir).ok();
}
