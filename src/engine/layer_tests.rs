//! Rack layers in the engine (docs/racks/PLAN.md Stage 4c): a rack of A
//! and B sounds like A and B played together; key ranges, level, pan and
//! on/off work; each layer goes through its own effects; nothing
//! allocates while a rack plays; songs without a rack never make a layer.
use super::racks_tests::{clip_song, diff, one_note_song, render};
use super::*;
use crate::fx::{Device, FxKind};
use crate::model::{Layer, RackP};

fn saw() -> InstrumentKind {
    one_note_song().instruments[0].kind.clone()
}

fn square() -> InstrumentKind {
    InstrumentKind::Synth(crate::synth::Patch {
        osc1: crate::synth::Wave::Square,
        cutoff: 1500.0,
        amp: crate::synth::Adsr {
            a: 0.004,
            d: 0.2,
            s: 0.7,
            r: 0.1,
        },
        ..Default::default()
    })
}

/// slot 1 = a rack of slots 2 (saw) and 3 (square with a Drive), quiet
/// master without the limiter (so mixes can be compared in the linear range)
fn rack_song(layers: Vec<Layer>) -> Song {
    let mut s = one_note_song();
    s.mixer.master = 0.05;
    s.mixer.limiter.enabled = false;
    let base = s.instruments[0].clone();
    s.instruments.truncate(1);
    s.instruments[0].kind = InstrumentKind::Rack(RackP { layers });
    s.instruments[0].name = "Rack".into();
    let mut a = base.clone();
    a.name = "Saw".into();
    a.kind = saw();
    let mut b = base;
    b.name = "Square".into();
    b.kind = square();
    let mut d = Device::new(FxKind::Drive);
    d.set_real("drive", 24.0);
    b.fx = vec![d];
    s.instruments.push(a);
    s.instruments.push(b);
    s
}

fn ab() -> Vec<Layer> {
    vec![Layer::new(1), Layer::new(2)]
}

/// the master stage is tanh with the limiter off: undo it to add mixes
fn lin(v: &[f32]) -> Vec<f32> {
    v.iter().map(|x| x.clamp(-0.999, 0.999).atanh()).collect()
}

fn sum(a: &[f32], b: &[f32]) -> Vec<f32> {
    lin(a).iter().zip(lin(b)).map(|(x, y)| x + y).collect()
}

/// past the first effect refresh of a note-clip track
const SKIP: usize = 8192;

#[test]
fn a_rack_of_a_and_b_sounds_like_a_and_b_together() {
    let keys = [37, 41, 44, 49];
    let rack = render(&clip_song(rack_song(ab()), 1, &keys), 1.2);
    let a = render(&clip_song(rack_song(ab()), 2, &keys), 1.2);
    let b = render(&clip_song(rack_song(ab()), 3, &keys), 1.2);
    assert!(rack.iter().any(|x| x.abs() > 1e-3), "the rack is heard");
    let want = sum(&a, &b);
    let d = diff(&lin(&rack)[SKIP..], &want[SKIP..]);
    assert!(d < 1e-5, "rack = A + B ({d})");
    // and it is not just one of them
    assert!(diff(&lin(&rack)[SKIP..], &lin(&a)[SKIP..]) > 1e-3);
}

#[test]
fn a_rack_without_layers_or_with_every_layer_off_is_silent() {
    let none = render(&clip_song(rack_song(vec![]), 1, &[37]), 0.4);
    assert!(none.iter().all(|x| *x == 0.0));
    let mut off = ab();
    off.iter_mut().for_each(|l| l.on = false);
    let off = render(&clip_song(rack_song(off), 1, &[37]), 0.4);
    assert!(off.iter().all(|x| *x == 0.0));
}

#[test]
fn key_range_level_pan_and_transpose_work() {
    let keys = [37, 61];
    // the saw only below key 49, the square only from 49 up
    let mut l = ab();
    l[0].key_hi = 48;
    l[1].key_lo = 49;
    let split = render(&clip_song(rack_song(l), 1, &keys), 1.0);
    let a_lo = render(&clip_song(rack_song(ab()), 2, &[37]), 1.0);
    let mut b_hi = clip_song(rack_song(ab()), 3, &[0, 61]);
    // (key 0 = no note: keeps the second note on beat 2)
    if let Some(t) = b_hi.timeline.as_deref_mut() {
        let n = &mut t.notes[0].make_mut().notes;
        n.remove(0);
    }
    let b_hi = render(&b_hi, 1.0);
    let d = diff(&lin(&split)[SKIP..], &sum(&a_lo, &b_hi)[SKIP..]);
    assert!(d < 1e-5, "key ranges ({d})");
    // -6.02 dB = half as loud (only the saw: no drive to bend it)
    let mut l = vec![Layer::new(1)];
    l[0].level_db = -6.0206;
    let half = render(&clip_song(rack_song(l), 1, &[37]), 0.6);
    let full = render(&clip_song(rack_song(vec![Layer::new(1)]), 1, &[37]), 0.6);
    let h: Vec<f32> = lin(&full).iter().map(|x| x * 0.5).collect();
    assert!(diff(&lin(&half)[SKIP..], &h[SKIP..]) < 1e-6);
    // hard left: the right channel is empty
    let mut l = vec![Layer::new(1)];
    l[0].pan = -1.0;
    let left = render(&clip_song(rack_song(l), 1, &[37]), 0.6);
    assert!(left.chunks(2).all(|f| f[1] == 0.0));
    assert!(left.chunks(2).any(|f| f[0].abs() > 1e-3));
    // a layer an octave up sounds like the note an octave up
    let mut l = vec![Layer::new(1)];
    l[0].transpose = 12;
    let up = render(&clip_song(rack_song(l), 1, &[37]), 0.6);
    let a_up = render(&clip_song(rack_song(ab()), 2, &[49]), 0.6);
    assert!(diff(&up[SKIP..], &a_up[SKIP..]) < 1e-6);
}

#[test]
fn the_rack_chain_comes_after_the_layers() {
    let plain = render(&clip_song(rack_song(ab()), 1, &[37]), 0.6);
    let mut s = rack_song(ab());
    let mut f = Device::new(FxKind::Filter);
    f.set_real("cutoff", 300.0);
    s.instruments[0].fx = vec![f];
    let filtered = render(&clip_song(s, 1, &[37]), 0.6);
    assert!(diff(&plain[SKIP..], &filtered[SKIP..]) > 1e-4);
    // a layer's own chain only touches that layer: without the square's
    // Drive the rack changes
    let mut s = rack_song(ab());
    s.instruments[2].fx.clear();
    let undriven = render(&clip_song(s, 1, &[37]), 0.6);
    assert!(diff(&plain[SKIP..], &undriven[SKIP..]) > 1e-4);
}

#[test]
fn tracker_notes_play_the_layers_until_the_next_note_or_note_off() {
    let mut s = rack_song(ab());
    s.patterns[0].cells[0][0].instr = 1;
    s.patterns[0].cells[8][0] = Cell {
        note: NOTE_OFF,
        instr: 0,
        vol: 0,
        fx: 0,
        fxp: 0,
    };
    let out = render(&s, 2.5);
    // 120 BPM speed 6 => a row is 1/8 s: the note-off at 1 s, release 0.1 s
    let rms =
        |a: usize, b: usize| (out[a..b].iter().map(|x| x * x).sum::<f32>() / (b - a) as f32).sqrt();
    assert!(
        rms(9600, 48000) > 1e-3,
        "the layers play from a tracker block"
    );
    assert!(
        rms(48000 * 2 + 28800, 48000 * 2 + 48000) < 1e-5,
        "and stop at the note-off"
    );
}

#[test]
fn a_playing_rack_does_not_allocate() {
    let song = clip_song(rack_song(ab()), 1, &[37, 41, 44, 49, 37, 41]);
    let mut e = Engine::new(48000.0);
    e.start(&song, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 512];
    for _ in 0..40 {
        e.render(&song, &mut buf);
    }
    let n = alloc_count::count(|| {
        for _ in 0..120 {
            e.render(&song, &mut buf);
        }
    });
    assert_eq!(n, 0, "no allocation while rack layers play");
}

#[test]
fn songs_without_a_rack_never_make_layer_voices() {
    let song = one_note_song();
    let mut e = Engine::new(48000.0);
    e.start(&song, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 4096];
    e.render(&song, &mut buf);
    assert!(e.nv.layers.banks.is_empty());
    let song = rack_song(ab());
    e.start(&song, 0, 0, false, 0);
    assert_eq!(e.nv.layers.banks.len(), MAX_CHANNELS);
}

#[test]
fn plugin_instruments_and_racks_never_play_as_layers() {
    let mut s = rack_song(vec![Layer::new(1), Layer::new(2)]);
    s.instruments[1].kind = InstrumentKind::Plugin(crate::plugin::PluginPatch {
        format: "clap".into(),
        id: "org.example.none".into(),
        name: "None".into(),
        vendor: String::new(),
        path: String::new(),
        state: String::new(),
        params: vec![],
        macros: vec![],
    });
    s.instruments[2].kind = InstrumentKind::Rack(RackP::default());
    let out = render(&clip_song(s, 1, &[37]), 0.4);
    assert!(out.iter().all(|x| x.is_finite() && *x == 0.0));
}

#[test]
fn layer_slots_follow_instrument_moves_and_deletes() {
    let mut s = rack_song(ab());
    let layers = |s: &Song| match &s
        .instruments
        .iter()
        .find(|i| i.name == "Rack")
        .unwrap()
        .kind
    {
        InstrumentKind::Rack(r) => r
            .layers
            .iter()
            .map(|l| s.instruments[l.slot].name.clone())
            .collect::<Vec<_>>(),
        _ => unreachable!(),
    };
    assert_eq!(layers(&s), ["Saw", "Square"]);
    // move the square to the front: the rack still plays the same two
    s.move_instrument(2, 0).unwrap();
    assert_eq!(layers(&s), ["Saw", "Square"]);
    assert_eq!(s.rack_of(0), Some(1), "the square is a layer of the rack");
    // delete the saw: the rack loses that layer, the square stays
    let saw = s.instruments.iter().position(|i| i.name == "Saw").unwrap();
    s.delete_instrument(saw);
    assert_eq!(layers(&s), ["Square"]);
}

#[test]
fn racks_round_trip_need_reader_7_and_bad_layers_are_dropped() {
    let mut s = rack_song(ab());
    if let InstrumentKind::Rack(r) = &mut s.instruments[0].kind {
        r.layers[1].level_db = -3.0;
        r.layers[1].pan = 0.4;
        r.layers[1].key_lo = 30;
    }
    assert!(crate::fileio::needs_reader_7(&s));
    let dir = std::env::temp_dir().join(format!("cw-rack-song-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("rack.cwt");
    crate::fileio::save_cwt(&s, &f).unwrap();
    let back = crate::fileio::load_cwt(&f).unwrap();
    assert_eq!(back.instruments, s.instruments);
    assert_eq!(render(&back, 0.3), render(&s, 0.3));
    // a damaged file: a layer pointing at itself, at a rack, out of range,
    // too many layers, odd values
    let mut bad = s.clone();
    bad.instruments.push(bad.instruments[0].clone());
    if let InstrumentKind::Rack(r) = &mut bad.instruments[0].kind {
        r.layers = vec![
            Layer::new(0),
            Layer::new(3),
            Layer::new(99),
            Layer::new(1),
            Layer::new(2),
            Layer::new(1),
            Layer::new(2),
            Layer::new(1),
        ];
        r.layers[3].level_db = f32::NAN;
        r.layers[4].key_lo = 100;
        r.layers[4].key_hi = 20;
    }
    bad.fix_racks();
    let InstrumentKind::Rack(r) = &bad.instruments[0].kind else {
        unreachable!()
    };
    assert_eq!(r.layers.len(), crate::model::MAX_LAYERS);
    assert!(r.layers.iter().all(|l| l.slot == 1 || l.slot == 2));
    assert_eq!(r.layers[0].level_db, 0.0);
    assert_eq!((r.layers[1].key_lo, r.layers[1].key_hi), (20, 100));
    std::fs::remove_dir_all(&dir).ok();
}
