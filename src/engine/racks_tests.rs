//! Nested Device Chains audio (docs/racks/PLAN.md Stage 3-4): devices
//! added, reordered and removed change what is heard; a Split's paths add
//! up; a bypassed Split or one empty path is the dry sound.
use super::*;
use crate::fx::{Device, FxKind, FxPath};

/// one held note on track 1 (instrument 1, a plain saw synth), nothing
/// else playing, no sends
pub(crate) fn one_note_song() -> Song {
    let mut song = crate::fileio::demo_song();
    song.automation.clear();
    song.instruments[0].kind = InstrumentKind::Synth(crate::synth::Patch {
        cutoff: 3000.0,
        amp: crate::synth::Adsr {
            a: 0.002,
            d: 0.3,
            s: 0.8,
            r: 0.1,
        },
        ..Default::default()
    });
    song.instruments[0].fx.clear();
    song.instruments[0].volume = 1.0;
    for p in song.patterns.iter_mut() {
        for row in p.cells.iter_mut() {
            row.iter_mut().for_each(|c| *c = Cell::default());
        }
    }
    song.order = vec![0];
    song.patterns[0].cells[0][0] = Cell {
        note: 37,
        instr: 1,
        vol: 0,
        fx: 0,
        fxp: 0,
    };
    for m in song.mixer.channels.iter_mut() {
        m.send = [0.0; 2];
        m.duck = 0.0;
    }
    song.mixer.master = 0.3;
    song
}

/// `secs` of the song, stereo interleaved
pub(crate) fn render(song: &Song, secs: f32) -> Vec<f32> {
    let mut e = Engine::new(48000.0);
    e.start(song, 0, 0, false, 0);
    let mut out = vec![0.0f32; (secs * 48000.0) as usize * 2];
    for c in out.chunks_mut(512) {
        e.render(song, c);
    }
    out
}

/// mean absolute difference
pub(crate) fn diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len().max(1) as f32
}

fn with_fx(fx: Vec<Device>) -> Song {
    let mut s = one_note_song();
    s.instruments[0].fx = fx;
    s
}

fn drive() -> Device {
    let mut d = Device::new(FxKind::Drive);
    d.set_real("drive", 30.0);
    d
}

fn filter() -> Device {
    let mut d = Device::new(FxKind::Filter);
    d.set_real("cutoff", 400.0);
    d
}

#[test]
fn adding_reordering_and_removing_devices_changes_what_is_heard() {
    let dry = render(&with_fx(vec![]), 0.5);
    let one = render(&with_fx(vec![drive()]), 0.5);
    assert!(diff(&dry, &one) > 1e-3, "a device changes the sound");
    let ab = render(&with_fx(vec![drive(), filter()]), 0.5);
    let ba = render(&with_fx(vec![filter(), drive()]), 0.5);
    assert!(diff(&ab, &ba) > 1e-4, "the order is heard");
    // removed again: the dry sound, bit for bit
    let back = render(&with_fx(vec![]), 0.5);
    assert_eq!(dry, back);
}

fn split(paths: Vec<(Vec<Device>, f32, f32, bool)>) -> Device {
    let mut d = Device::split();
    d.paths = paths
        .into_iter()
        .map(|(devices, level_db, pan, on)| FxPath {
            devices,
            on,
            level_db,
            pan,
        })
        .collect();
    d
}

#[test]
fn one_empty_split_path_at_0_db_is_the_dry_sound_bit_for_bit() {
    let dry = render(&with_fx(vec![]), 0.5);
    let s = render(&with_fx(vec![split(vec![(vec![], 0.0, 0.0, true)])]), 0.5);
    assert_eq!(dry, s);
    // a new Split: two empty paths at -6 dB add up to (nearly) the dry sound
    let s2 = render(&with_fx(vec![Device::split()]), 0.5);
    assert!(diff(&dry, &s2) < 1e-4, "{}", diff(&dry, &s2));
    // a bypassed Split is the dry sound
    let mut b = split(vec![
        (vec![drive()], 0.0, 0.0, true),
        (vec![filter()], 0.0, 0.0, true),
    ]);
    b.enabled = false;
    assert_eq!(dry, render(&with_fx(vec![b]), 0.5));
}

#[test]
fn split_paths_add_up_and_level_pan_and_mute_work() {
    // the master limiter and soft clip would bend a sum: keep levels low
    let lo = |mut s: Song| {
        s.mixer.master = 0.05;
        s.mixer.limiter.enabled = false;
        s
    };
    let a = render(
        &lo(with_fx(vec![split(vec![(vec![drive()], 0.0, 0.0, true)])])),
        0.4,
    );
    let b = render(
        &lo(with_fx(vec![split(vec![(vec![filter()], 0.0, 0.0, true)])])),
        0.4,
    );
    let ab = render(
        &lo(with_fx(vec![split(vec![
            (vec![drive()], 0.0, 0.0, true),
            (vec![filter()], 0.0, 0.0, true),
        ])])),
        0.4,
    );
    // the master stage is tanh (limiter off), so compare in its linear range
    let atanh = |v: &[f32]| {
        v.iter()
            .map(|x| x.clamp(-0.999, 0.999).atanh())
            .collect::<Vec<_>>()
    };
    let sum: Vec<f32> = atanh(&a)
        .iter()
        .zip(atanh(&b))
        .map(|(x, y)| x + y)
        .collect();
    assert!(
        diff(&atanh(&ab), &sum) < 1e-4,
        "{}",
        diff(&atanh(&ab), &sum)
    );
    // path B muted: only A is heard
    let muted = render(
        &lo(with_fx(vec![split(vec![
            (vec![drive()], 0.0, 0.0, true),
            (vec![filter()], 0.0, 0.0, false),
        ])])),
        0.4,
    );
    assert!(diff(&muted, &a) < 1e-6);
    // -60 dB is (almost) silent; hard left leaves the right channel empty
    let quiet = render(
        &lo(with_fx(vec![split(vec![(vec![], -60.0, 0.0, true)])])),
        0.4,
    );
    assert!(quiet.iter().all(|x| x.abs() < 1e-3));
    let left = render(
        &lo(with_fx(vec![split(vec![(vec![], 0.0, -1.0, true)])])),
        0.4,
    );
    assert!(left.chunks(2).all(|f| f[1].abs() < 1e-7));
    assert!(left.chunks(2).any(|f| f[0].abs() > 1e-3));
}

#[test]
fn moving_a_device_between_paths_changes_which_path_processes_it() {
    let lo = |mut s: Song| {
        s.mixer.master = 0.05;
        s
    };
    // A: drive at 0 dB, B: dry at -20 dB  vs  A: dry at 0 dB, B: drive at -20 dB
    let x = render(
        &lo(with_fx(vec![split(vec![
            (vec![drive()], 0.0, 0.0, true),
            (vec![], -20.0, 0.0, true),
        ])])),
        0.4,
    );
    let y = render(
        &lo(with_fx(vec![split(vec![
            (vec![], 0.0, 0.0, true),
            (vec![drive()], -20.0, 0.0, true),
        ])])),
        0.4,
    );
    assert!(diff(&x, &y) > 1e-4);
}

#[test]
fn a_split_inside_a_split_renders_and_stays_allocation_free() {
    let inner = split(vec![
        (vec![filter()], -3.0, 0.3, true),
        (vec![], -3.0, -0.3, true),
    ]);
    let outer = split(vec![
        (vec![drive(), inner], -6.0, 0.0, true),
        (vec![], -6.0, 0.0, true),
    ]);
    let song = with_fx(vec![
        Device::new(FxKind::Eq),
        outer,
        Device::new(FxKind::Reverb),
    ]);
    let out = render(&song, 0.5);
    assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    assert!(out.iter().any(|x| x.abs() > 1e-3));
    let mut e = Engine::new(48000.0);
    e.start(&song, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 512];
    for _ in 0..20 {
        e.render(&song, &mut buf);
    }
    let n = alloc_count::count(|| {
        for _ in 0..40 {
            e.render(&song, &mut buf);
        }
    });
    assert_eq!(n, 0, "no allocation while a nested Split plays");
}

#[test]
fn split_files_round_trip_and_need_reader_version_7() {
    let mut song = with_fx(vec![split(vec![
        (vec![drive()], -4.0, 0.25, true),
        (vec![Device::new(FxKind::Reverb)], 0.0, 0.0, false),
    ])]);
    song.instruments[0].fx[0].fix();
    let back: Vec<Device> =
        toml::from_str::<toml::Value>(&toml::to_string(&song.instruments[0]).unwrap())
            .unwrap()
            .get("fx")
            .unwrap()
            .clone()
            .try_into()
            .unwrap();
    assert_eq!(back, song.instruments[0].fx);
    assert!(crate::fileio::needs_reader_7(&song));
    assert!(!crate::fileio::needs_reader_7(&one_note_song()));
}

#[test]
fn damaged_splits_are_fixed_on_load_never_dropped() {
    let mut d = split(vec![]);
    d.paths.clear();
    d.fix();
    assert_eq!(d.paths.len(), 1, "an empty Split gets its dry path");
    let mut d = split((0..6).map(|_| (vec![], f32::NAN, 9.0, true)).collect());
    d.fix();
    assert_eq!(d.paths.len(), 4);
    assert!(d.paths.iter().all(|p| p.level_db == 0.0 && p.pan == 1.0));
    // a plugin effect inside a Split (a newer or edited file) is kept, bypassed
    let mut plug = Device::new(FxKind::Plugin);
    plug.enabled = true;
    let mut d = split(vec![(vec![plug], 0.0, 0.0, true)]);
    d.fix();
    assert_eq!(d.paths[0].devices.len(), 1);
    assert!(!d.paths[0].devices[0].enabled);
    // three Splits deep: the deepest is kept, bypassed
    let deep = split(vec![(
        vec![split(vec![(vec![split(vec![])], 0.0, 0.0, true)])],
        0.0,
        0.0,
        true,
    )]);
    let mut d = deep.clone();
    d.fix();
    let third = &d.paths[0].devices[0].paths[0].devices[0];
    assert_eq!(third.kind, FxKind::Split);
    assert!(!third.enabled);
    // a non-Split never keeps paths
    let mut e = Device::new(FxKind::Eq);
    e.paths.push(FxPath::default());
    e.fix();
    assert!(e.paths.is_empty());
}
