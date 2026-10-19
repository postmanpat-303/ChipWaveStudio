//! VST3 hosting tests against the ChipWave VST3 Test Synth / Test FX
//! fixtures (plugins/cw-test-vst3, built on demand into a `.vst3` bundle).
//! They mirror the CLAP tests: the two fixtures play the same algorithm, so
//! a song renders the same through either format.
use super::fxrack::{self, FxRack, FX_BLOCK};
use super::fxtests::{err_at, left, render, synth_song};
use super::tests::{plugin_song, rms};
use super::*;
use crate::automation::{Lane, Point, Shape, Target};
use crate::fx::{Device, FxKind};
use std::path::PathBuf;

pub const SYNTH_ID: &str = "C3A15E017B2D4F109E440A615C2B7701";
pub const FX_ID: &str = "C3A15E037B2D4F109E440A615C2B7703";
const LAT: usize = 32;
const SR: u32 = 48000;

/// build the fixture once and lay it out as a `.vst3` bundle named `name`
pub fn vst3_fixture_named(name: &str) -> PathBuf {
    static BUILT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let lib = BUILT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target = root.join("target").join("fixture-vst3");
        let st = std::process::Command::new(option_env!("CARGO").unwrap_or("cargo"))
            .args(["build", "--release", "--quiet", "--manifest-path"])
            .arg(root.join("plugins/cw-test-vst3/Cargo.toml"))
            .env("CARGO_TARGET_DIR", &target)
            .status()
            .expect("cargo build of the VST3 test plugin");
        assert!(st.success(), "VST3 test plugin build failed");
        let f = if cfg!(windows) {
            "cw_test_vst3.dll"
        } else if cfg!(target_os = "macos") {
            "libcw_test_vst3.dylib"
        } else {
            "libcw_test_vst3.so"
        };
        target.join("release").join(f)
    });
    let bundle = lib
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("bundles")
        .join(name);
    let stem = name.trim_end_matches(".vst3");
    let bin = if cfg!(target_os = "macos") {
        let c = bundle.join("Contents");
        std::fs::create_dir_all(c.join("MacOS")).unwrap();
        std::fs::write(
            c.join("Info.plist"),
            format!(
                "<plist><dict><key>CFBundleExecutable</key><string>{stem}</string></dict></plist>"
            ),
        )
        .unwrap();
        c.join("MacOS").join(stem)
    } else {
        let arch = match (cfg!(windows), cfg!(target_arch = "aarch64")) {
            (true, true) => "arm64-win",
            (true, false) => "x86_64-win",
            (false, true) => "aarch64-linux",
            (false, false) => "x86_64-linux",
        };
        let d = bundle.join("Contents").join(arch);
        std::fs::create_dir_all(&d).unwrap();
        d.join(format!(
            "{stem}.{}",
            if cfg!(windows) { "vst3" } else { "so" }
        ))
    };
    let fresh = std::fs::metadata(&bin)
        .and_then(|m| m.modified())
        .ok()
        .zip(std::fs::metadata(lib).and_then(|m| m.modified()).ok())
        .is_some_and(|(a, b)| a >= b);
    if !fresh {
        // copy to a temp name first: a loaded library is never overwritten
        let tmp = bin.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::copy(lib, &tmp).unwrap();
        let _ = std::fs::rename(&tmp, &bin);
    }
    bundle
}

pub fn vst3_fixture() -> PathBuf {
    vst3_fixture_named("cw-test-vst3.vst3")
}

pub fn vst3_patch() -> PluginPatch {
    PluginPatch {
        format: "vst3".into(),
        id: SYNTH_ID.into(),
        name: "ChipWave VST3 Test Synth".into(),
        vendor: "ChipWave".into(),
        path: vst3_fixture().to_string_lossy().into_owned(),
        state: String::new(),
        params: vec![],
        macros: vec![1, 2],
    }
}

fn vst3_song() -> Song {
    let mut s = plugin_song();
    s.instruments[0].kind = InstrumentKind::Plugin(vst3_patch());
    s
}

fn fx_patch() -> PluginPatch {
    PluginPatch {
        format: "vst3".into(),
        id: FX_ID.into(),
        name: "ChipWave VST3 Test FX".into(),
        vendor: "ChipWave".into(),
        path: vst3_fixture().to_string_lossy().into_owned(),
        state: String::new(),
        params: vec![],
        macros: vec![],
    }
}

fn with_fx(mut s: Song, params: Vec<(u32, f64)>) -> Song {
    let mut p = fx_patch();
    p.params = params;
    let mut d = Device::plugin(p);
    d.uid = 7;
    s.instruments[0].fx = vec![d];
    s
}

fn synth() -> vst3::Vst3Inst {
    vst3::Vst3Inst::new(&vst3_fixture(), SYNTH_ID, 48000.0).unwrap()
}

#[test]
fn class_ids_use_the_canonical_text_form() {
    let t = vst3::parse_cid(SYNTH_ID).unwrap();
    assert_eq!(vst3::cid_string(&t), SYNTH_ID);
    assert!(vst3::parse_cid("C3A15E01").is_none());
    assert!(vst3::parse_cid("zz").is_none());
    assert_eq!(
        vst3::features("Instrument|Synth"),
        vec!["instrument", "synth"]
    );
    assert_eq!(vst3::features("Fx|Delay"), vec!["audio-effect", "delay"]);
}

#[test]
fn scan_lists_the_vst3_instrument_and_effect() {
    let r = scan::check_file(&vst3_fixture());
    assert!(r.ok, "{}", r.message);
    assert_eq!(r.plugins.len(), 2, "processor classes only");
    let (a, b) = (&r.plugins[0], &r.plugins[1]);
    assert_eq!(a.format, "vst3");
    assert_eq!(a.desc.id, SYNTH_ID);
    assert_eq!(a.desc.name, "ChipWave VST3 Test Synth");
    assert_eq!(a.desc.vendor, "ChipWave");
    assert!(a.desc.is_instrument() && !a.desc.is_effect());
    assert_eq!(b.desc.id, FX_ID);
    assert!(b.desc.is_effect() && !b.desc.is_instrument());
    // the bundle is found once, not its insides
    let dir = vst3_fixture().parent().unwrap().to_path_buf();
    let found = scan::find_files(std::slice::from_ref(&dir));
    assert!(found.contains(&(vst3_fixture(), "vst3")), "{found:?}");
    assert!(found.iter().all(|f| f.1 == "vst3"));
}

#[test]
fn a_broken_vst3_fails_cleanly() {
    let dir = std::env::temp_dir().join(format!("cw-vst3-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("garbage.vst3");
    std::fs::write(&f, b"this is not a shared library").unwrap();
    let r = scan::check_file(&f);
    assert!(!r.ok);
    assert!(r.message.starts_with("load"), "{}", r.message);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn vst3_instance_plays_notes_and_reports_params() {
    let mut i = synth();
    let ps = i.params();
    let ids: Vec<u32> = ps.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![1, 2, 3]);
    assert_eq!(ps[0].name, "Gain");
    assert_eq!((ps[0].min, ps[0].max), (0.0, 1.0), "normalised");
    assert!(ps[2].readonly && !ps[0].readonly);
    assert_eq!(i.value(1), Some(0.5));
    let (mut l, mut r) = (vec![0.0; 512], vec![0.0; 512]);
    clap::AUDIO_THREAD.with(|a| a.set(true));
    i.process(512, &mut l, &mut r);
    assert_eq!(rms(&l), 0.0, "silent before a note");
    i.note_on(69, 1.0);
    i.process(512, &mut l, &mut r);
    assert!(rms(&l) > 0.05, "a note sounds: {}", rms(&l));
    let before = rms(&l);
    i.param(1, 1.0);
    i.process(512, &mut l, &mut r);
    assert!(rms(&l) > before * 1.5);
    i.note_off(69);
    for _ in 0..20 {
        i.process(512, &mut l, &mut r);
    }
    clap::AUDIO_THREAD.with(|a| a.set(false));
    assert_eq!(rms(&l), 0.0, "released and asleep");
    // the audio thread's value reaches the controller on the main thread
    i.poll_main();
    assert_eq!(i.value(1), Some(1.0));
    assert_eq!(i.value_text(1, 0.25).as_deref(), Some("25 %"));
    // the processor told the controller through IConnectionPoint + IMessage
    assert_eq!(i.value(3), Some(1.0), "processor -> controller message");
    assert_eq!(i.value_text(3, 1.0).as_deref(), Some("linked"));
}

#[test]
fn per_note_tuning_moves_the_pitch() {
    fn crossings(v: &[f32]) -> usize {
        v.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count()
    }
    let mut i = synth();
    let mut take = |i: &mut vst3::Vst3Inst| {
        let (mut l, mut r) = (vec![0.0; 512], vec![0.0; 512]);
        let mut all = vec![];
        for _ in 0..8 {
            i.process(512, &mut l, &mut r);
            all.extend_from_slice(&l);
        }
        all
    };
    clap::AUDIO_THREAD.with(|a| a.set(true));
    i.note_on(57, 1.0);
    let plain = crossings(&take(&mut i)[512..]);
    i.tuning(57, 12.0);
    let up = crossings(&take(&mut i)[512..]);
    clap::AUDIO_THREAD.with(|a| a.set(false));
    assert!(
        (up as f32 / plain as f32 - 2.0).abs() < 0.1,
        "an octave up: {plain} -> {up}"
    );
}

#[test]
fn vst3_and_clap_fixtures_render_the_same_song_alike() {
    let a = crate::engine::render_song(&plugin_song(), 22050);
    let b = crate::engine::render_song(&vst3_song(), 22050);
    assert_eq!(a.len(), b.len());
    assert!(rms(&b) > 0.01);
    let e = a
        .iter()
        .zip(&b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max);
    assert!(e < 1e-4, "max difference {e}");
}

#[test]
fn vst3_state_round_trips_through_the_patch_and_the_song_file() {
    let mut a = synth();
    a.set_param_main(1, 0.8);
    a.set_param_main(2, 0.3);
    let st = a.save_state().unwrap();
    assert_eq!(&st[..4], b"CWV3");
    let mut p = vst3_patch();
    p.set_state_bytes(&st);
    let b = instantiate(&p, 44100.0).unwrap();
    assert_eq!(b.value(1), Some(0.8));
    assert_eq!(b.value(2), Some(0.3));
    let mut song = vst3_song();
    song.instruments[0].kind = InstrumentKind::Plugin(p.clone());
    let dir = std::env::temp_dir().join(format!("cw-vst3-song-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("plug.cwt");
    crate::fileio::save_cwt(&song, &f).unwrap();
    let (back, warn) = crate::fileio::load_cwt_warn(&f).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    assert!(warn.is_empty(), "{warn:?}");
    assert_eq!(back.instruments[0].kind, InstrumentKind::Plugin(p));
    // garbage is refused, not crashed on
    let mut c = synth();
    assert!(!c.load_state(b"nope"));
}

#[test]
fn editor_edits_through_the_component_handler_reach_the_processor() {
    let mut i = synth();
    let h = i.handler();
    assert!(!i.take_dirty());
    h.edit(1, 0.9);
    assert!(i.take_dirty());
    // not playing: poll_main flushes it into the processor
    i.poll_main();
    let st = i.save_state().unwrap();
    let mut p = vst3_patch();
    p.set_state_bytes(&st);
    let b = instantiate(&p, 48000.0).unwrap();
    assert!((b.value(1).unwrap() - 0.9).abs() < 1e-9);
    // a restart request re-reads the parameter list
    h.restart(1 << 3); // kParamValuesChanged
    i.poll_main();
    assert_eq!(i.params().len(), 3);
}

#[test]
fn vst3_song_render_is_allocation_free() {
    let song = vst3_song();
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
    assert_eq!(n, 0, "VST3 rendering allocated {n} times");
}

#[test]
fn missing_vst3_is_a_silent_placeholder_with_a_warning() {
    let mut song = vst3_song();
    if let InstrumentKind::Plugin(p) = &mut song.instruments[0].kind {
        p.id = "0123456789ABCDEF0123456789ABCDEF".into();
        p.path = "/nowhere/missing.vst3".into();
        p.state = "AAEC".into();
    }
    let w = missing_warnings(&song);
    assert_eq!(w.len(), 1);
    assert!(
        w[0].contains("not installed") && w[0].contains("VST3"),
        "{}",
        w[0]
    );
    let mut rack = Rack::new();
    prepare(&mut rack, &song, 48000.0);
    assert!(rack.is_empty());
    let out = crate::engine::render_song(&song, 22050);
    assert!(out.iter().all(|x| x.abs() < 1e-6));
    match &song.instruments[0].kind {
        InstrumentKind::Plugin(p) => assert_eq!(p.state, "AAEC"),
        _ => unreachable!(),
    }
}

#[test]
fn vst3_macros_43xy_drive_mapped_params() {
    let mut song = vst3_song();
    song.patterns[0].cells[0][0].fx = 0x43;
    song.patterns[0].cells[0][0].fxp = 0x11;
    let quiet = crate::engine::render_song(&song, 22050);
    let loud = crate::engine::render_song(&vst3_song(), 22050);
    let n = 22050;
    assert!(rms(&quiet[..n]) < rms(&loud[..n]) * 0.4);
}

#[test]
fn text_song_vst3_patch_loads() {
    let text = format!(
        r#"[song]
title = "t"
bpm = 120
speed = 6
tracks = 2
[[instrument]]
slot = 1
name = "Plug"
patch = "vst3:{SYNTH_ID}"
plugin_path = '{}'
macros = [1, 2]
[[block]]
name = "A"
lines = 4
rows = """
00 | C-4 01 .. ....
"""
"#,
        vst3_fixture().display()
    );
    let s = crate::fileio::song_from_text(&text).unwrap();
    match &s.instruments[0].kind {
        InstrumentKind::Plugin(p) => {
            assert_eq!(p.format, "vst3");
            assert_eq!(p.id, SYNTH_ID);
            assert_eq!(p.macros, vec![1, 2]);
        }
        k => panic!("{k:?}"),
    }
    let pcm = crate::engine::render_song(&s, 22050);
    assert!(rms(&pcm) > 0.01);
}

// ------------------------------------------------------------------ effects

#[test]
fn vst3_fx_instance_processes_input_and_reports_latency() {
    let mut i = vst3::Vst3Inst::new(&vst3_fixture(), FX_ID, 48000.0).unwrap();
    assert_eq!(i.latency(), LAT as u32);
    let ps = i.params();
    let ids: Vec<u32> = ps.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![10, 11]);
    assert!(ps[1].stepped && !ps[0].stepped);
    let inp: Vec<f32> = (0..256).map(|k| ((k as f32) * 0.1).sin()).collect();
    let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
    clap::AUDIO_THREAD.with(|a| a.set(true));
    i.param(10, 0.25); // normalised: x0.5
    i.process_fx(256, &inp, &inp, &mut l, &mut r);
    clap::AUDIO_THREAD.with(|a| a.set(false));
    for k in LAT..256 {
        assert!((l[k] - 0.5 * inp[k - LAT]).abs() < 1e-6, "{k}");
        assert_eq!(l[k], r[k]);
    }
    assert_eq!(i.value_text(10, 0.25).as_deref(), Some("x0.50"));
}

#[test]
fn a_vst3_effect_in_a_chain_follows_automation_and_modulation() {
    let dry = left(&render(&synth_song(false), 0.6));
    let song = with_fx(synth_song(false), vec![(10, 0.25)]);
    let mut rack = FxRack::new();
    fxrack::prepare(&mut rack, &song, SR as f32);
    assert_eq!(rack.slots.len(), 1);
    assert_eq!(rack.slots[0].latency, (FX_BLOCK + LAT) as u32);
    let lag = FX_BLOCK + LAT;
    let wet = left(&render(&song, 0.6));
    let e = err_at(&wet, &dry, lag, 0.5, 2000, 20000);
    assert!(e < 1e-4, "wet != 0.5 x dry delayed {lag}: {e}");
    // automation lane on Gain: 0 -> 1 (x2) over three rows
    let mut auto = with_fx(synth_song(false), vec![]);
    let mut l = Lane::new(Target::Fx(0, 0, 10), "Gain".into());
    for (pos, value) in [(0.0, 0.0), (3.0, 1.0)] {
        l.points.push(Point {
            pos,
            value,
            shape: Shape::Linear,
            curve: 0.0,
        });
    }
    auto.automation = vec![l];
    let w = left(&render(&auto, 0.6));
    let early = rms(&w[lag + 200..lag + 2000]) / rms(&dry[200..2000]);
    let late = rms(&w[20000 + lag..24000 + lag]) / rms(&dry[20000..24000]);
    assert!(early < 0.2 && late > 1.5, "{early} {late}");
    // stepped Flip inverts
    let flip = with_fx(synth_song(false), vec![(11, 1.0)]);
    let inv = left(&render(&flip, 0.6));
    assert!(err_at(&inv, &dry, lag, -1.0, 4000, 20000) < 1e-4);
    // a macro route at +0.25 takes Gain 0.25 -> 0.5 = unity
    use crate::modulation::ModKind;
    let mut m = with_fx(synth_song(false), vec![(10, 0.25)]);
    let k = m.modulation.add(0, ModKind::Macro).unwrap();
    m.modulation.mods[k].value = 1.0;
    let id = m.modulation.mods[k].id;
    m.modulation
        .connect(id, "instrument.1.fx.1.p10", 0.25)
        .unwrap();
    let wm = left(&render(&m, 0.6));
    assert!(err_at(&wm, &dry, lag, 1.0, 4000, 20000) < 1e-3);
}

#[test]
fn vst3_effect_state_is_captured_saved_and_restored_allocation_free() {
    let mut song = with_fx(synth_song(true), vec![]);
    song.instruments[1].fx = vec![Device::new(FxKind::Delay)];
    let mut e = crate::engine::Engine::new(48000.0);
    fxrack::prepare(&mut e.fx_plugins, &song, 48000.0);
    e.fx_plugins.set_param(0, 7, 10, 0.875);
    let mut saved = song.clone();
    e.fx_plugins.capture_all(&mut saved);
    let st = &saved.instruments[0].fx[0].plugin.as_ref().unwrap().state;
    assert!(!st.is_empty());
    let dir = std::env::temp_dir().join(format!("cw-vst3fx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("state.cwt");
    crate::fileio::save_cwt(&saved, &f).unwrap();
    let back = crate::fileio::load_cwt(&f).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    let mut r2 = FxRack::new();
    fxrack::prepare(&mut r2, &back, 48000.0);
    let v = r2.slots[0].inst.value(10).unwrap();
    assert!((v - 0.875).abs() < 1e-9, "{v}");
    // rendering with an automated VST3 effect allocates nothing
    let mut l = Lane::new(Target::Fx(0, 0, 10), "Gain".into());
    for (pos, value) in [(0.0, 0.3), (16.0, 0.9)] {
        l.points.push(Point {
            pos,
            value,
            shape: Shape::Linear,
            curve: 0.0,
        });
    }
    song.automation = vec![l];
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
    assert_eq!(n, 0, "VST3 FX rendering allocated {n} times");
}

#[test]
fn missing_vst3_effect_is_bypassed_and_keeps_its_state() {
    let mut song = with_fx(synth_song(false), vec![]);
    {
        let p = song.instruments[0].fx[0].plugin.as_mut().unwrap();
        p.path = "/nowhere/missing.vst3".into();
        p.id = "0123456789ABCDEF0123456789ABCDEF".into();
        p.state = "AAEC".into();
    }
    let w = fxrack::missing_warnings(&song);
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("bypassed"), "{}", w[0]);
    assert_eq!(render(&song, 0.5), render(&synth_song(false), 0.5));
}

/// A real VST3 (CHIPWAVE_TEST_VST3=<bundle.vst3>, e.g. Surge XT or an SDK
/// sample): `cargo test real_vst3 -- --ignored --nocapture`
#[test]
#[ignore]
fn real_vst3_from_env() {
    let Some(path) = std::env::var_os("CHIPWAVE_TEST_VST3").map(PathBuf::from) else {
        eprintln!("set CHIPWAVE_TEST_VST3");
        return;
    };
    let r = scan::check_file(&path);
    assert!(r.ok, "{}", r.message);
    for f in &r.plugins {
        let p = PluginPatch {
            format: "vst3".into(),
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
            "{} [{}] {:?}: {} parameters, latency {}",
            f.desc.name,
            f.desc.id,
            f.desc.features,
            ps.len(),
            a.latency()
        );
        if let Some(k) = ps.iter().find(|x| !x.readonly && !x.stepped) {
            a.set_param_main(k.id, 0.37);
            let st = a.save_state().expect("state");
            let mut q = p.clone();
            q.set_state_bytes(&st);
            let b = instantiate(&q, 48000.0).unwrap();
            let got = b.value(k.id).unwrap();
            eprintln!("  {} = {:?} -> {got}", k.name, a.value_text(k.id, 0.37));
            assert!((got - 0.37).abs() < 2e-2, "{got}");
        }
        let mut song = if f.desc.is_instrument() {
            let mut s = plugin_song();
            s.instruments[0].kind = InstrumentKind::Plugin(p.clone());
            s
        } else {
            let mut s = synth_song(false);
            s.instruments[0].fx = vec![Device::plugin(p.clone())];
            s
        };
        song.automation.clear();
        let pcm = crate::engine::render_song(&song, 48000);
        let peak = pcm.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        eprintln!("  peak {peak}");
        assert!(pcm.iter().all(|x| x.is_finite()));
    }
}
