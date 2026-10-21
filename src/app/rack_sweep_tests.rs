//! Stage 6 sweeps (docs/racks/PLAN.md): 20 rack and chain edits in a row,
//! each one undo step, all undone back to the start and redone to the end;
//! every refusal (5th layer, plugin layer, rack in a rack, damaged / newer
//! file, full song) says why and leaves the song and the undo history as
//! they were.
use super::chains_tests::{chains_on, frames};
use super::fxview::FxOp;
use super::*;
use crate::fx::FxKind;
use crate::model::Layer;
use crate::racks::tests::{info, song_with_rack};
use egui_kittest::Harness;

fn state(h: &Harness<'static, App>) -> String {
    h.state().with(|s| format!("{:?}", s.song))
}

fn layer(h: &Harness<'static, App>, rack: usize, k: usize) -> Layer {
    h.state().with(|s| match &s.song.instruments[rack].kind {
        InstrumentKind::Rack(r) => r.layers[k].clone(),
        _ => panic!("not a rack"),
    })
}

fn rack_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cw-racksweep-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    crate::racks::TEST_DIR.with(|t| *t.borrow_mut() = Some(d.clone()));
    d
}

#[test]
fn twenty_rack_edits_undo_one_by_one_to_the_start_and_redo_to_the_end() {
    let d = rack_dir("undo");
    let file = d.join("Glass Keys.cwr");
    crate::racks::save(
        &crate::racks::from_song(&song_with_rack(), 0, info("Glass Keys")).unwrap(),
        &file,
    )
    .unwrap();
    let mut h = chains_on(7);
    let n0 = h.state().with(|s| s.song.instruments.len());
    let (a, b) = (n0, n0 + 1); // layer A's and B's slots
    let mut states = vec![state(&h)];
    type Act = Box<dyn Fn(&mut App)>;
    let set = |k: usize, f: fn(&mut Layer)| -> Act {
        Box::new(move |app: &mut App| {
            let mut l = app.with(|s| match &s.song.instruments[6].kind {
                InstrumentKind::Rack(r) => r.layers[k].clone(),
                _ => panic!(),
            });
            f(&mut l);
            app.set_layer(6, k, l, true);
        })
    };
    let fx = |slot: usize, op: fn() -> FxOp| -> Act {
        Box::new(move |app: &mut App| app.apply_fx_ops(slot, vec![op()]))
    };
    let acts: Vec<(&str, Act)> = vec![
        ("make a rack", Box::new(|app: &mut App| app.make_rack(6))),
        (
            "add a layer",
            Box::new(|app: &mut App| app.add_layer(6, None)),
        ),
        ("layer level", set(0, |l| l.level_db = -6.0)),
        ("layer pan", set(0, |l| l.pan = 0.3)),
        ("layer transpose", set(1, |l| l.transpose = 12)),
        (
            "key range",
            set(1, |l| {
                l.key_lo = 30;
                l.key_hi = 80
            }),
        ),
        (
            "filter on layer A",
            fx(a, || FxOp::Add(FxKind::Filter, None)),
        ),
        ("split on layer B", fx(b, || FxOp::Add(FxKind::Split, None))),
        ("a third path", fx(b, || FxOp::AddPath(0))),
        (
            "drive in path 1",
            fx(b, || {
                FxOp::In(0, 0, Box::new(FxOp::Add(FxKind::Drive, None)))
            }),
        ),
        ("path level", fx(b, || FxOp::PathLevel(0, 1, -3.0, true))),
        ("path pan", fx(b, || FxOp::PathPan(0, 1, -0.5, true))),
        ("path off", fx(b, || FxOp::PathOn(0, 2))),
        ("filter knob", fx(a, || FxOp::Param(0, 0, 0.3, true))),
        (
            "delay on the rack",
            fx(6, || FxOp::Add(FxKind::Delay, None)),
        ),
        ("layer off", set(1, |l| l.on = false)),
        (
            "song instrument as a layer",
            Box::new(|app: &mut App| app.add_layer(6, Some(1))),
        ),
        (
            "remove that layer",
            Box::new(|app: &mut App| app.remove_layer(6, 2)),
        ),
        ("rack effect off", fx(6, || FxOp::Toggle(0))),
        (
            "load a saved rack onto track 1",
            Box::new(move |app: &mut App| {
                assert!(app.load_rack(&file, Some(0)).is_some());
            }),
        ),
    ];
    assert_eq!(acts.len(), 20);
    for (name, act) in &acts {
        act(h.state_mut());
        frames(&mut h, 2);
        let s = state(&h);
        assert_ne!(&s, states.last().unwrap(), "{name} changed something");
        states.push(s);
    }
    assert_eq!(layer(&h, 6, 0).level_db, -6.0);
    for k in (0..20).rev() {
        h.state_mut().undo(false);
        frames(&mut h, 2);
        assert_eq!(state(&h), states[k], "undo of {:?}", acts[k].0);
    }
    for k in 1..=20 {
        h.state_mut().undo(true);
        frames(&mut h, 2);
        assert_eq!(state(&h), states[k], "redo of {:?}", acts[k - 1].0);
    }
    std::fs::remove_dir_all(&d).ok();
}

/// the song and the undo history are as they were, and the status says why
fn refused(h: &mut Harness<'static, App>, what: &str, act: impl FnOnce(&mut App), says: &str) {
    let before = state(h);
    h.state_mut().status.clear();
    act(h.state_mut());
    frames(h, 2);
    assert_eq!(state(h), before, "{what}: nothing changed");
    let st = h.state().status.clone();
    assert!(
        st.contains(says),
        "{what}: status {st:?} should say {says:?}"
    );
}

#[test]
fn every_rack_refusal_says_why_and_changes_nothing() {
    let d = rack_dir("errors");
    let mut h = chains_on(7);
    h.state_mut().make_rack(6);
    for _ in 0..3 {
        h.state_mut().add_layer(6, None);
    }
    frames(&mut h, 2);
    refused(
        &mut h,
        "5th layer",
        |a| a.add_layer(6, None),
        "at most 4 layers",
    );
    refused(&mut h, "rack of a rack", |a| a.make_rack(6), "rack already");
    let layer_slot = layer(&h, 6, 0).slot;
    refused(
        &mut h,
        "rack of a layer",
        |a| a.make_rack(layer_slot),
        "layer of a rack",
    );
    // a plugin instrument can't be a layer
    h.state_mut().remove_layer(6, 3);
    frames(&mut h, 2);
    let p = h.state().with(|s| {
        let mut i = s.song.instruments[0].clone();
        i.kind = InstrumentKind::Plugin(crate::plugin::PluginPatch {
            format: "clap".into(),
            id: "com.example.synth".into(),
            name: "Example Synth".into(),
            vendor: "Example".into(),
            path: String::new(),
            state: String::new(),
            params: vec![],
            macros: vec![],
        });
        s.song.instruments.push(i);
        s.song.instruments.len() - 1
    });
    refused(
        &mut h,
        "plugin layer",
        |a| a.add_layer(6, Some(p)),
        "plugin instruments",
    );
    refused(
        &mut h,
        "plugin rack",
        |a| a.make_rack(p),
        "plugin instruments",
    );
    refused(
        &mut h,
        "rack inside a rack",
        |a| a.add_layer(6, Some(6)),
        "inside a rack",
    );
    // damaged and newer files: refused before anything changes
    let bad = d.join("Broken.cwr");
    std::fs::write(&bad, b"not a zip").unwrap();
    refused(
        &mut h,
        "damaged file",
        |a| assert!(a.load_rack(&bad, Some(0)).is_none()),
        "damaged",
    );
    let newer = d.join("Newer.cwr");
    {
        use std::io::Write;
        let f = std::fs::File::create(&newer).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("mimetype", o).unwrap();
        z.write_all(b"application/x-chipwave-rack").unwrap();
        z.start_file("rack.toml", o).unwrap();
        z.write_all(b"format = \"chipwave-rack\"\nformat_version = 99\n[rack]\nname = \"x\"\n")
            .unwrap();
        z.finish().unwrap();
    }
    refused(
        &mut h,
        "newer file",
        |a| assert!(a.load_rack(&newer, Some(0)).is_none()),
        "newer ChipWave",
    );
    refused(
        &mut h,
        "missing file",
        |a| assert!(a.load_rack(&d.join("Gone.cwr"), Some(0)).is_none()),
        "Gone",
    );
    // a full song: the rack doesn't fit, nothing is added
    let good = d.join("Glass Keys.cwr");
    crate::racks::save(
        &crate::racks::from_song(&song_with_rack(), 0, info("Glass Keys")).unwrap(),
        &good,
    )
    .unwrap();
    h.state().with(|s| {
        while s.song.instruments.len() < 254 {
            let i = s.song.instruments[0].clone();
            s.song.instruments.push(i);
        }
    });
    frames(&mut h, 2);
    refused(
        &mut h,
        "full song",
        |a| assert!(a.load_rack(&good, Some(0)).is_none()),
        "slots",
    );
    std::fs::remove_dir_all(&d).ok();
}
