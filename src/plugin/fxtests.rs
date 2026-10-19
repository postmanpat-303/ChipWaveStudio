//! Plugin effect (CLAP FX) tests against the ChipWave Test FX fixture (the
//! second plugin in plugins/cw-test-synth): gain 0-2 (id 10), polarity flip
//! (id 11), a 32-sample delay it reports as latency.
use super::fxrack::{self, FxRack, FX_BLOCK};
use super::tests::fixture;
use super::*;
use crate::automation::{Lane, Point, Shape, Target};
use crate::fx::{Device, FxKind};
use crate::model::{Cell, Instrument, Song, NOTE_OFF};

pub const FX_ID: &str = "org.chipwave.test-fx";
const LAT: usize = 32;
const SR: u32 = 48000;

pub fn fx_patch() -> PluginPatch {
    PluginPatch {
        format: "clap".into(),
        id: FX_ID.into(),
        name: "ChipWave Test FX".into(),
        vendor: "ChipWave".into(),
        path: fixture().to_string_lossy().into_owned(),
        state: String::new(),
        params: vec![],
        macros: vec![],
    }
}

/// a plain saw synth on instrument 1, C-4 on channels 0 (and 1 if `two`)
fn synth_song(two: bool) -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.timeline = None;
    s.instruments.truncate(2);
    let saw = crate::synth::Patch {
        cutoff: 3000.0,
        amp: crate::synth::Adsr {
            a: 0.002,
            d: 0.3,
            s: 0.8,
            r: 0.05,
        },
        ..Default::default()
    };
    for (k, inst) in s.instruments.iter_mut().enumerate() {
        *inst = Instrument {
            name: format!("Saw {k}"),
            volume: 0.8,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Synth(saw.clone()),
        };
    }
    s.order = vec![0];
    for p in &mut s.patterns {
        for row in &mut p.cells {
            row.iter_mut().for_each(|c| *c = Cell::default());
        }
    }
    for m in s.mixer.channels.iter_mut() {
        m.send = [0.0; 2];
        m.duck = 0.0;
        m.pan = 0.0;
        m.volume = 1.0;
    }
    s.bpm = 125.0; // a row = 0.12 s at speed 6
    s.speed = 6;
    let p = &mut s.patterns[0];
    for (ch, instr) in [(0usize, 1u8), (1, 2)] {
        if ch == 1 && !two {
            break;
        }
        p.cells[0][ch] = Cell {
            note: 49,
            instr,
            ..Default::default()
        };
        p.cells[12][ch] = Cell {
            note: NOTE_OFF,
            ..Default::default()
        };
    }
    s
}

fn with_fx(mut s: Song, params: Vec<(u32, f64)>) -> Song {
    let mut p = fx_patch();
    p.params = params;
    let mut d = Device::plugin(p);
    d.uid = 7;
    s.instruments[0].fx = vec![d];
    s
}

fn render(song: &Song, secs: f32) -> Vec<f32> {
    let mut out = crate::engine::render_song(song, SR);
    out.truncate((secs * SR as f32) as usize * 2);
    out
}

fn left(v: &[f32]) -> Vec<f32> {
    v.chunks(2).map(|f| f[0]).collect()
}

/// max |a[i] - g * b[i - lag]| over a window
fn err_at(a: &[f32], b: &[f32], lag: usize, g: f32, from: usize, to: usize) -> f32 {
    (from..to)
        .map(|i| (a[i] - g * b[i - lag]).abs())
        .fold(0.0, f32::max)
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

#[test]
fn test_fx_instance_processes_input_and_reports_latency() {
    let mut i = clap::ClapInst::new(&fixture(), FX_ID, 48000.0).unwrap();
    assert_eq!(i.latency(), LAT as u32);
    let ids: Vec<u32> = i.params().iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![10, 11]);
    let inp: Vec<f32> = (0..256).map(|k| ((k as f32) * 0.1).sin()).collect();
    let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
    clap::AUDIO_THREAD.with(|a| a.set(true));
    i.param(10, 0.5);
    i.process_fx(256, &inp, &inp, &mut l, &mut r);
    clap::AUDIO_THREAD.with(|a| a.set(false));
    for k in LAT..256 {
        assert!((l[k] - 0.5 * inp[k - LAT]).abs() < 1e-6, "{k}");
        assert_eq!(l[k], r[k]);
    }
}

#[test]
fn a_plugin_effect_in_an_instrument_chain_processes_the_channel() {
    let dry = left(&render(&synth_song(false), 0.6));
    let song = with_fx(synth_song(false), vec![(10, 0.5)]);
    let mut rack = FxRack::new();
    fxrack::prepare(&mut rack, &song, SR as f32);
    assert_eq!(rack.slots.len(), 1, "one instance: channel 0");
    assert_eq!(rack.slots[0].latency, (FX_BLOCK + LAT) as u32);
    let wet = left(&render(&song, 0.6));
    assert!(rms(&dry[4000..20000]) > 0.02);
    // gain 0.5, delayed by the block plus the plugin's 32 samples
    let lag = FX_BLOCK + LAT;
    let e = err_at(&wet, &dry, lag, 0.5, 2000, 20000);
    assert!(e < 1e-4, "wet != 0.5 x dry delayed {lag}: {e}");
}

#[test]
fn plugin_effect_parameters_follow_automation_lanes() {
    let dry = left(&render(&synth_song(false), 0.6));
    let mut song = with_fx(synth_song(false), vec![]);
    // lane on the plugin's Gain (param id 10): 0 -> 2 over the first 3 rows
    let t = Target::parse("instrument.1.fx.1.p10").unwrap();
    assert_eq!(t, Target::Fx(0, 0, 10));
    let mut l = Lane::new(t, "Gain".into());
    for (pos, value) in [(0.0, 0.0), (3.0, 1.0)] {
        l.points.push(Point {
            pos,
            value,
            shape: Shape::Linear,
        });
    }
    song.automation = vec![l];
    let wet = left(&render(&song, 0.6));
    let lag = FX_BLOCK + LAT;
    let early = rms(&wet[lag + 200..lag + 2000]) / rms(&dry[200..2000]);
    let late = rms(&wet[20000 + lag..24000 + lag]) / rms(&dry[20000..24000]);
    assert!(early < 0.2, "start ~0: {early}");
    assert!(late > 1.5, "end ~2: {late}");
    // a stepped parameter: Flip inverts the polarity
    let mut f = with_fx(synth_song(false), vec![]);
    let mut l = Lane::new(Target::Fx(0, 0, 11), "Flip".into());
    l.points.push(Point {
        pos: 0.0,
        value: 1.0,
        shape: Shape::Step,
    });
    f.automation = vec![l];
    let inv = left(&render(&f, 0.6));
    assert!(err_at(&inv, &dry, lag, -1.0, 4000, 20000) < 1e-4);
}

#[test]
fn bypassing_a_plugin_device_keeps_the_timing() {
    let dry = left(&render(&synth_song(false), 0.6));
    let mut song = with_fx(synth_song(false), vec![(10, 0.25)]);
    song.instruments[0].fx[0].enabled = false;
    let out = left(&render(&song, 0.6));
    // bypassed: the dry signal, delayed by the block (no plugin latency)
    assert!(err_at(&out, &dry, FX_BLOCK, 1.0, 4000, 20000) < 1e-4);
}

#[test]
fn plugin_delay_compensation_aligns_the_other_channels() {
    // channel 0 runs the plugin with Flip on, channel 1 plays the same note
    // dry: compensated, the two cancel exactly (uncompensated they would be
    // 96 samples apart and comb)
    let both = with_fx(synth_song(true), vec![(11, 1.0)]);
    let mut solo = both.clone();
    for row in &mut solo.patterns[0].cells {
        row[1] = Cell::default();
    }
    let a = left(&render(&both, 0.5));
    let b = left(&render(&solo, 0.5));
    assert!(rms(&b[4000..20000]) > 0.02, "channel 0 alone is audible");
    assert!(
        rms(&a[2000..22000]) < 1e-5,
        "not aligned: {}",
        rms(&a[2000..22000])
    );
    let mut e = crate::engine::Engine::new(SR as f32);
    fxrack::prepare(&mut e.fx_plugins, &both, SR as f32);
    e.start(&both, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 512];
    e.render(&both, &mut buf);
    assert_eq!(e.fx_plugins.max_lat, (FX_BLOCK + LAT) as u32);
    assert_eq!(e.fx_plugins.ch_lat[1], 0);
}

#[test]
fn plugin_effect_rendering_is_allocation_free() {
    let mut song = with_fx(synth_song(true), vec![]);
    song.instruments[1].fx = vec![Device::new(FxKind::Delay)];
    let mut l = Lane::new(Target::Fx(0, 0, 10), "Gain".into());
    l.points.push(Point {
        pos: 0.0,
        value: 0.3,
        shape: Shape::Linear,
    });
    l.points.push(Point {
        pos: 16.0,
        value: 0.9,
        shape: Shape::Linear,
    });
    song.automation = vec![l];
    let mut e = crate::engine::Engine::new(48000.0);
    fxrack::prepare(&mut e.fx_plugins, &song, 48000.0);
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
    assert_eq!(n, 0, "plugin FX rendering allocated {n} times");
}

#[test]
fn missing_plugin_effect_is_bypassed_with_a_notice_and_keeps_its_state() {
    let mut song = with_fx(synth_song(false), vec![]);
    {
        let p = song.instruments[0].fx[0].plugin.as_mut().unwrap();
        p.id = "org.example.not-installed".into();
        p.path = "/nowhere/missing.clap".into();
        p.state = "AAEC".into();
    }
    let w = fxrack::missing_warnings(&song);
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("bypassed"), "{}", w[0]);
    let mut rack = FxRack::new();
    fxrack::prepare(&mut rack, &song, 48000.0);
    assert!(rack.is_empty());
    assert!(rack.error(0, 7).unwrap().contains("not installed"));
    // bypassed: plays exactly like no device at all
    let dry = render(&synth_song(false), 0.5);
    assert_eq!(render(&song, 0.5), dry);
    // the saved state survives a save / load round trip
    let dir = std::env::temp_dir().join(format!("cw-fxplug-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("missing.cwt");
    crate::fileio::save_cwt(&song, &f).unwrap();
    let back = crate::fileio::load_cwt(&f).unwrap();
    // older readers can't parse a plugin device: the song asks for reader 2
    let mut z = zip::ZipArchive::new(std::fs::File::open(&f).unwrap()).unwrap();
    let mut m = String::new();
    std::io::Read::read_to_string(&mut z.by_name("manifest.toml").unwrap(), &mut m).unwrap();
    assert!(m.contains("min_reader_version = 2"), "{m}");
    let d = &back.instruments[0].fx[0];
    assert_eq!(d.kind, FxKind::Plugin);
    assert_eq!(d.uid, 7);
    assert_eq!(d.plugin.as_ref().unwrap().state, "AAEC");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn plugin_effect_state_is_captured_saved_and_restored() {
    let song = with_fx(synth_song(false), vec![]);
    let mut e = crate::engine::Engine::new(48000.0);
    fxrack::prepare(&mut e.fx_plugins, &song, 48000.0);
    // a UI edit (as from the editor), then capture into the song
    e.fx_plugins.set_param(0, 7, 10, 1.75);
    let mut saved = song.clone();
    e.fx_plugins.capture_all(&mut saved);
    let st = saved.instruments[0].fx[0]
        .plugin
        .as_ref()
        .unwrap()
        .state
        .clone();
    assert!(!st.is_empty());
    // round trip through the file and a fresh instance
    let dir = std::env::temp_dir().join(format!("cw-fxstate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("state.cwt");
    crate::fileio::save_cwt(&saved, &f).unwrap();
    let back = crate::fileio::load_cwt(&f).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    let mut r2 = FxRack::new();
    fxrack::prepare(&mut r2, &back, 48000.0);
    let v = r2.slots[0].inst.value(10).unwrap();
    assert!((v - 1.75).abs() < 1e-9, "{v}");
    let dry = left(&render(&synth_song(false), 0.5));
    let wet = left(&render(&back, 0.5));
    assert!(err_at(&wet, &dry, FX_BLOCK + LAT, 1.75, 4000, 20000) < 1e-3);
}

#[test]
fn rack_needs_follow_the_song_and_instrument_renumbering() {
    let song = with_fx(synth_song(true), vec![]);
    let need: Vec<(usize, usize, u32)> = FxRack::needed(&song, None)
        .into_iter()
        .map(|n| (n.0, n.1, n.2))
        .collect();
    assert_eq!(
        need,
        vec![(0, 0, 7)],
        "instrument 1 plays on channel 0 only"
    );
    // jamming instrument 1 on channel 3: that channel and the live chain too
    let need = FxRack::needed(&song, Some((0, 3)));
    assert_eq!(need.len(), 3);
    assert!(need.iter().any(|n| n.0 == fxrack::LIVE));
    let mut rack = FxRack::new();
    fxrack::prepare(&mut rack, &song, 48000.0);
    let gone = rack.remap(&|i| if i == 0 { Some(4) } else { Some(i) });
    assert!(gone.is_empty());
    assert_eq!(rack.slots[0].owner, 4);
    assert!(rack.primary(4, 7).is_some());
    assert_eq!(rack.slots[0].latency, (FX_BLOCK + LAT) as u32);
}

#[test]
fn songs_without_plugin_effects_render_as_before() {
    // no plugin device: no instances, no compensation, same samples as a
    // render that never touches the effect rack
    let song = synth_song(true);
    assert!(FxRack::needed(&song, None).is_empty());
    let mut e = crate::engine::Engine::new(48000.0);
    e.start(&song, 0, 0, false, 0);
    let mut a = vec![0.0f32; 4096];
    e.render(&song, &mut a);
    assert!(!e.fx_plugins.pdc_active());
    let b = crate::engine::render_song(&song, 48000);
    assert_eq!(&a[..], &b[..4096]);
}

#[test]
fn device_serde_is_backward_compatible() {
    // a 0.2.4.0 device (no plugin / uid keys) still loads
    let old: Device = toml::from_str("kind = \"Delay\"\nparams = [0.5]\n").unwrap();
    assert!(old.plugin.is_none() && old.uid == 0);
    // built-in devices don't write the new keys
    let t = toml::to_string(&Device::new(FxKind::Delay)).unwrap();
    assert!(!t.contains("plugin") && !t.contains("uid"), "{t}");
    // a plugin device round-trips
    let d = Device::plugin(fx_patch());
    assert!(d.uid != 0);
    let t = toml::to_string(&d).unwrap();
    let back: Device = toml::from_str(&t).unwrap();
    assert_eq!(back, d);
}
