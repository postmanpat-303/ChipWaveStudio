//! Plugin hosting tests against the ChipWave Test Synth fixture
//! (plugins/cw-test-synth, built on demand).
use super::*;
use crate::model::{Cell, Instrument, NOTE_OFF};
use std::path::PathBuf;

/// build the fixture once and return its `.clap` path (copied as `name`)
pub fn fixture_named(name: &str) -> PathBuf {
    static BUILT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let lib = BUILT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target = root.join("target").join("fixture");
        let st = std::process::Command::new(option_env!("CARGO").unwrap_or("cargo"))
            .args(["build", "--release", "--quiet", "--manifest-path"])
            .arg(root.join("plugins/cw-test-synth/Cargo.toml"))
            .env("CARGO_TARGET_DIR", &target)
            .status()
            .expect("cargo build of the test plugin");
        assert!(st.success(), "test plugin build failed");
        let f = if cfg!(windows) {
            "cw_test_synth.dll"
        } else if cfg!(target_os = "macos") {
            "libcw_test_synth.dylib"
        } else {
            "libcw_test_synth.so"
        };
        target.join("release").join(f)
    });
    let dir = lib.parent().unwrap().parent().unwrap().join("clap");
    std::fs::create_dir_all(&dir).unwrap();
    let dst = dir.join(name);
    let fresh = std::fs::metadata(&dst)
        .and_then(|m| m.modified())
        .ok()
        .zip(std::fs::metadata(lib).and_then(|m| m.modified()).ok())
        .is_some_and(|(a, b)| a >= b);
    if !fresh {
        // copy to a temp name first: a loaded library is never overwritten
        let tmp = dir.join(format!("{name}.{}.tmp", std::process::id()));
        std::fs::copy(lib, &tmp).unwrap();
        let _ = std::fs::rename(&tmp, &dst);
    }
    dst
}

pub fn fixture() -> PathBuf {
    fixture_named("cw-test-synth.clap")
}

pub const TEST_ID: &str = "org.chipwave.test-synth";

pub fn patch() -> PluginPatch {
    PluginPatch {
        format: "clap".into(),
        id: TEST_ID.into(),
        name: "ChipWave Test Synth".into(),
        vendor: "ChipWave".into(),
        path: fixture().to_string_lossy().into_owned(),
        state: String::new(),
        params: vec![],
        macros: vec![1, 2],
    }
}

/// demo song with instrument 1 = the test plugin playing C-4 .. on channel 0
pub fn plugin_song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.instruments[0] = Instrument {
        name: "Test Synth".into(),
        volume: 1.0,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Plugin(patch()),
    };
    s.order = vec![0];
    for p in &mut s.patterns {
        for row in &mut p.cells {
            for c in row.iter_mut() {
                *c = Cell::default();
            }
        }
    }
    let p = &mut s.patterns[0];
    p.cells[0][0] = Cell {
        note: 49, // C-4
        instr: 1,
        ..Default::default()
    };
    p.cells[8][0] = Cell {
        note: NOTE_OFF,
        ..Default::default()
    };
    p.cells[16][0] = Cell {
        note: 56,
        instr: 1,
        vol: 33,
        ..Default::default()
    };
    s
}

pub(super) fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

#[test]
fn scan_check_lists_the_fixture_as_an_instrument() {
    let r = scan::check_file(&fixture());
    assert!(r.ok, "{}", r.message);
    // the library also holds the Test FX effect (plugin FX tests)
    assert_eq!(r.plugins.len(), 2);
    assert!(r.plugins[1].desc.is_effect() && !r.plugins[1].desc.is_instrument());
    assert!(!r.plugins[0].desc.is_effect());
    let p = &r.plugins[0];
    assert_eq!(p.desc.id, TEST_ID);
    assert_eq!(p.desc.name, "ChipWave Test Synth");
    assert!(p.desc.is_instrument());
    assert_eq!(p.format, "clap");
}

#[test]
fn not_a_plugin_fails_cleanly() {
    let dir = std::env::temp_dir().join(format!("cw-plug-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("garbage.clap");
    std::fs::write(&f, b"this is not a shared library").unwrap();
    let r = scan::check_file(&f);
    assert!(!r.ok);
    assert!(r.message.starts_with("load"), "{}", r.message);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn instance_plays_notes_and_reports_params() {
    let mut i = clap::ClapInst::new(&fixture(), TEST_ID, 48000.0).unwrap();
    let ps = i.params();
    assert_eq!(ps.len(), 2);
    assert_eq!(ps[0].name, "Gain");
    assert_eq!(i.value(1), Some(0.5));
    let (mut l, mut r) = (vec![0.0; 512], vec![0.0; 512]);
    i.process(512, &mut l, &mut r);
    assert_eq!(rms(&l), 0.0, "silent before a note");
    i.note_on(69, 1.0);
    i.process(512, &mut l, &mut r);
    assert!(rms(&l) > 0.05, "a note sounds: {}", rms(&l));
    // a parameter event changes the level
    let before = rms(&l);
    i.param(1, 1.0);
    i.process(512, &mut l, &mut r);
    assert!(rms(&l) > before * 1.5);
    i.note_off(69);
    for _ in 0..20 {
        i.process(512, &mut l, &mut r);
    }
    assert_eq!(rms(&l), 0.0, "released and asleep");
    assert_eq!(i.value_text(1, 0.25).as_deref(), Some("25 %"));
}

#[test]
fn state_round_trips_through_base64_in_the_patch() {
    let mut a = clap::ClapInst::new(&fixture(), TEST_ID, 48000.0).unwrap();
    a.set_param_main(1, 0.8);
    a.set_param_main(2, 0.3);
    let mut p = patch();
    p.set_state_bytes(&a.save_state().unwrap());
    assert!(!p.state.is_empty());
    let b = instantiate(&p, 44100.0).unwrap();
    assert_eq!(b.value(1), Some(0.8));
    assert_eq!(b.value(2), Some(0.3));
    // and through a saved song file
    let mut song = plugin_song();
    if let InstrumentKind::Plugin(q) = &mut song.instruments[0].kind {
        *q = p.clone();
    }
    let dir = std::env::temp_dir().join(format!("cw-plug-song-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("plug.cwt");
    crate::fileio::save_cwt(&song, &f).unwrap();
    let (back, warn) = crate::fileio::load_cwt_warn(&f).unwrap();
    assert!(warn.is_empty(), "{warn:?}");
    assert_eq!(back.instruments[0].kind, InstrumentKind::Plugin(p));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn song_renders_plugin_notes_offline_through_the_engine() {
    let song = plugin_song();
    let out = crate::engine::render_song(&song, 22050);
    let row = (22050.0 * 60.0 / (song.bpm as f64 * 4.0)) as usize * 2;
    let first = &out[..row * 7];
    let gap = &out[row * 11..row * 15];
    let second = &out[row * 17..row * 24];
    assert!(rms(first) > 0.02, "note 1: {}", rms(first));
    assert!(rms(gap) < 1e-4, "note off released it: {}", rms(gap));
    // volume column 33 (half) on the second note: quieter than the first
    assert!(
        rms(second) > 0.005 && rms(second) < rms(first),
        "{} {}",
        rms(second),
        rms(first)
    );
    assert!(out.iter().all(|x| x.is_finite()));
}

#[test]
fn render_with_plugins_is_allocation_free() {
    let song = plugin_song();
    let mut e = crate::engine::Engine::new(48000.0);
    prepare(&mut e.plugins, &song, 48000.0);
    assert_eq!(e.plugins.slots.len(), 1);
    e.start(&song, 0, 0, true, 0);
    let mut buf = vec![0.0f32; 512];
    for _ in 0..50 {
        e.render(&song, &mut buf);
    }
    let n = crate::engine::alloc_count::count(|| {
        for _ in 0..400 {
            e.render(&song, &mut buf);
        }
    });
    assert_eq!(n, 0, "plugin rendering allocated {n} times");
}

#[test]
fn missing_plugin_is_a_silent_placeholder_with_a_warning() {
    let mut song = plugin_song();
    if let InstrumentKind::Plugin(p) = &mut song.instruments[0].kind {
        p.id = "org.example.not-installed".into();
        p.path = "/nowhere/missing.clap".into();
        p.state = "AAEC".into();
    }
    let w = missing_warnings(&song);
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("not installed"), "{}", w[0]);
    let mut rack = Rack::new();
    prepare(&mut rack, &song, 48000.0);
    assert!(rack.is_empty());
    assert!(rack.error(0).unwrap().contains("not installed"));
    let out = crate::engine::render_song(&song, 22050);
    assert!(out.iter().all(|x| *x == 0.0 || x.abs() < 1e-6));
    // the saved state stays in the song
    match &song.instruments[0].kind {
        InstrumentKind::Plugin(p) => assert_eq!(p.state, "AAEC"),
        _ => unreachable!(),
    }
}

#[test]
fn macro_43xy_and_automation_drive_mapped_params() {
    let mut song = plugin_song();
    // 4311 on row 0: macro 1 (Gain) = 1/15 -> much quieter
    song.patterns[0].cells[0][0].fx = 0x43;
    song.patterns[0].cells[0][0].fxp = 0x11;
    let quiet = crate::engine::render_song(&song, 22050);
    let loud = crate::engine::render_song(&plugin_song(), 22050);
    let n = 22050;
    assert!(
        rms(&quiet[..n]) < rms(&loud[..n]) * 0.4,
        "{} {}",
        rms(&quiet[..n]),
        rms(&loud[..n])
    );
    // automation lane on macro 2 (Tone) parses and renders
    let t = crate::automation::Target::parse("instrument.1.macro.2").unwrap();
    assert_eq!(t, crate::automation::Target::Macro(0, 1));
    assert_eq!(t.id(), "instrument.1.macro.2");
}

#[test]
fn rack_follows_the_song_and_instrument_renumbering() {
    let song = plugin_song();
    assert_eq!(Rack::needed(&song, None), vec![(0, 0)]);
    assert_eq!(Rack::needed(&song, Some((0, 3))), vec![(0, 0), (0, 3)]);
    assert!(Rack::needed(&crate::fileio::demo_song(), Some((0, 0))).is_empty());
    let mut rack = Rack::new();
    prepare(&mut rack, &song, 48000.0);
    assert_eq!(rack.slot(0, 0), Some(0));
    // instrument 1 moves to slot 3
    drop(rack.remap(&|i| Some(i + 2)));
    assert_eq!(rack.slot(2, 0), Some(0));
    assert_eq!(rack.slot(0, 0), None);
    // removed
    drop(rack.remap(&|_| None));
    assert!(rack.is_empty());
}

#[test]
fn text_song_clap_patch_loads() {
    let path = fixture();
    let text = format!(
        r#"[song]
title = "t"
bpm = 120
speed = 6
tracks = 2
[[instrument]]
slot = 1
name = "Plug"
patch = "clap:{TEST_ID}"
plugin_path = '{}'
macros = [1, 2]
[[block]]
name = "A"
lines = 4
rows = """
00 | C-4 01 .. ....
"""
"#,
        path.display()
    );
    let s = crate::fileio::song_from_text(&text).unwrap();
    match &s.instruments[0].kind {
        InstrumentKind::Plugin(p) => {
            assert_eq!(p.id, TEST_ID);
            assert_eq!(p.macros, vec![1, 2]);
        }
        k => panic!("{k:?}"),
    }
}

#[test]
fn bundle_folders_load_their_binary() {
    // macOS-style bundle: Contents/MacOS/<CFBundleExecutable>
    let d = std::env::temp_dir().join(format!("cw-bundle-{}", std::process::id()));
    let b = d.join("Synth.clap");
    std::fs::create_dir_all(b.join("Contents/MacOS")).unwrap();
    std::fs::write(
        b.join("Contents/Info.plist"),
        "<plist><dict><key>CFBundleExecutable</key>\n<string>RealBin</string></dict></plist>",
    )
    .unwrap();
    std::fs::copy(fixture(), b.join("Contents/MacOS/RealBin")).unwrap();
    assert_eq!(clap::binary_path(&b), b.join("Contents/MacOS/RealBin"));
    // Loading needs a real macOS-style bundle binary. On Windows, LoadLibraryExW
    // appends ".dll" to a file name without an extension ("RealBin"), so the copy
    // can't be loaded there, and Windows CLAPs are plain files anyway.
    if cfg!(not(windows)) {
        let r = scan::check_file(&b);
        assert!(r.ok, "{}", r.message);
        assert_eq!(r.plugins[0].desc.id, TEST_ID);
    }
    // found once by the scanner (the bundle, not its insides)
    let found = scan::find_files(std::slice::from_ref(&d));
    assert_eq!(found, vec![(b.clone(), "clap")]);
    // a plain file is its own binary
    assert_eq!(clap::binary_path(&fixture()), fixture());
    let _ = std::fs::remove_dir_all(d);
}

/// A real plugin (CHIPWAVE_TEST_PLUGIN=<file.clap>, e.g. Surge XT):
/// `cargo test real_plugin -- --ignored --nocapture`
#[test]
#[ignore]
fn real_plugin_from_env() {
    let Some(path) = std::env::var_os("CHIPWAVE_TEST_PLUGIN").map(PathBuf::from) else {
        eprintln!("set CHIPWAVE_TEST_PLUGIN");
        return;
    };
    let r = scan::check_file(&path);
    assert!(r.ok, "{}", r.message);
    let f = r
        .plugins
        .iter()
        .find(|p| p.desc.is_instrument())
        .expect("an instrument");
    let p = PluginPatch {
        format: "clap".into(),
        id: f.desc.id.clone(),
        name: f.desc.name.clone(),
        vendor: f.desc.vendor.clone(),
        path: path.to_string_lossy().into_owned(),
        state: String::new(),
        params: vec![],
        macros: vec![],
    };
    let mut a = instantiate(&p, 48000.0).unwrap();
    let ps = a.params();
    eprintln!(
        "{}: {} parameters, editor {}",
        f.desc.name,
        ps.len(),
        a.has_editor()
    );
    assert!(!ps.is_empty());
    // change a parameter, save, load into a fresh instance
    let k = ps.iter().find(|x| !x.readonly && !x.stepped).unwrap();
    let v = k.min + (k.max - k.min) * 0.37;
    a.set_param_main(k.id, v);
    let st = a.save_state().expect("state");
    eprintln!(
        "state {} bytes; {} = {:?}",
        st.len(),
        k.name,
        a.value_text(k.id, v)
    );
    let mut q = p.clone();
    q.set_state_bytes(&st);
    let b = instantiate(&q, 48000.0).unwrap();
    let got = b.value(k.id).unwrap();
    assert!(
        (got - v).abs() < 1e-3 * (k.max - k.min).max(1.0),
        "{got} vs {v}"
    );
    // a song with it renders sound, allocation-free per block after warm-up
    let mut song = plugin_song();
    song.instruments[0].kind = InstrumentKind::Plugin(q);
    let mut e = crate::engine::Engine::new(48000.0);
    prepare(&mut e.plugins, &song, 48000.0);
    e.start(&song, 0, 0, true, 0);
    let mut buf = vec![0.0f32; 1024];
    let mut peak = 0.0f32;
    for _ in 0..100 {
        e.render(&song, &mut buf);
        peak = buf.iter().fold(peak, |m, x| m.max(x.abs()));
    }
    eprintln!("peak {peak}");
    assert!(peak > 1e-3);
    let n = crate::engine::alloc_count::count(|| {
        for _ in 0..50 {
            e.render(&song, &mut buf);
        }
    });
    eprintln!("allocations in 50 blocks (plugin's own included): {n}");
}
