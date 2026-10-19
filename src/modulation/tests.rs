//! Drag-to-modulate: model, modulator engine, rendering, file round trips.
use super::*;
use crate::automation::{Lane, Point, Shape, Target};
use crate::engine::{alloc_count, Engine};
use crate::model::{Cell, InstrumentKind, Song, NOTE_OFF};

const SR: f32 = 48000.0;

/// one saw synth on instrument 1, a held C-3 on channel 0 (rows 0..24)
fn song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.timeline = None;
    s.bpm = 120.0;
    s.speed = 6;
    s.instruments[0].kind = InstrumentKind::Synth(crate::synth::Patch {
        cutoff: 800.0,
        resonance: 0.1,
        amp: crate::synth::Adsr {
            a: 0.001,
            d: 0.5,
            s: 1.0,
            r: 0.05,
        },
        ..Default::default()
    });
    s.instruments[0].fx.clear();
    s.instruments[0].volume = 1.0;
    s.order = vec![0];
    for row in s.patterns[0].cells.iter_mut() {
        row.iter_mut().for_each(|c| *c = Cell::default());
    }
    s.patterns[0].cells[0][0] = Cell {
        note: 37,
        instr: 1,
        ..Default::default()
    };
    s.patterns[0].cells[24][0] = Cell {
        note: NOTE_OFF,
        ..Default::default()
    };
    for m in s.mixer.channels.iter_mut() {
        m.send = [0.0; 2];
        m.duck = 0.0;
    }
    s.mixer.master = 0.5;
    s
}

fn render(song: &Song, secs: f32) -> Vec<f32> {
    let mut e = Engine::new(SR);
    e.start(song, 0, 0, false, 0);
    let mut out = vec![0.0f32; (secs * SR) as usize * 2];
    for c in out.chunks_mut(512) {
        e.render(song, c);
    }
    out
}

fn left(v: &[f32]) -> Vec<f32> {
    v.chunks(2).map(|f| f[0]).collect()
}

/// derivative energy over energy: higher = brighter
fn bright(x: &[f32]) -> f32 {
    let e: f32 = x.iter().map(|v| v * v).sum::<f32>().max(1e-12);
    x.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>() / e
}

/// a modulator of `kind` on instrument 1 routed to `target`
fn routed(mut s: Song, kind: ModKind, target: &str, amount: f32) -> Song {
    let k = s.modulation.add(0, kind).unwrap();
    let id = s.modulation.mods[k].id;
    s.modulation.connect(id, target, amount).unwrap();
    s
}

// ------------------------------------------------------------------ registry

#[test]
fn registry_keys_round_trip_and_match_the_id_form() {
    let p = crate::synth::Patch::default();
    for (k, pp) in PATCH_PARAMS.iter().enumerate() {
        assert_eq!(patch_param(pp.key), Some(k));
        assert_eq!(
            patch_param_id(&pp.key.replace('.', "-")),
            Some(k),
            "{}",
            pp.key
        );
        let mut q = p.clone();
        let mid = from_norm(0.5, pp.lo, pp.hi, pp.log);
        (pp.set)(&mut q, mid);
        assert!(
            ((pp.get)(&q) - mid).abs() <= 1e-3 * mid.abs().max(1.0),
            "{}",
            pp.key
        );
        assert!(
            (to_norm(mid, pp.lo, pp.hi, pp.log) - 0.5).abs() < 1e-3,
            "{}",
            pp.key
        );
        let t = Target::Patch(2, k);
        assert_eq!(Target::parse(&t.id()), Some(t), "{}", t.id());
    }
    // two keys share a field: generator release is the amp release in ms
    let r = patch_param("gen.release").unwrap();
    let mut q = p.clone();
    (PATCH_PARAMS[r].set)(&mut q, 250.0);
    assert!((q.amp.r - 0.25).abs() < 1e-6);
    let t = Target::ModAmount(3);
    assert_eq!(Target::parse(&t.id()), Some(t));
    assert_eq!(t.to_real(1.0), 1.0);
    assert_eq!(t.to_real(0.0), -1.0);
}

#[test]
fn lfo_shapes_span_minus_one_to_one() {
    for sh in LfoShape::ALL {
        let v: Vec<f32> = (0..1000).map(|k| sh.at(k as f32 / 1000.0)).collect();
        let (lo, hi) = v
            .iter()
            .fold((1f32, -1f32), |(a, b), x| (a.min(*x), b.max(*x)));
        assert!(lo >= -1.0 && hi <= 1.0, "{sh:?}");
        assert!(lo < -0.98 && hi > 0.98, "{sh:?} {lo} {hi}");
    }
    assert!((LfoShape::Triangle.at(0.0)).abs() < 1e-6);
    assert!((LfoShape::Triangle.at(0.25) - 1.0).abs() < 1e-6);
    assert!((LfoShape::Sine.at(0.25) - 1.0).abs() < 1e-6);
}

// --------------------------------------------------------------------- model

#[test]
fn routes_connect_once_and_follow_instrument_edits() {
    let mut s = song();
    s.instruments.truncate(3);
    let a = s.modulation.add(0, ModKind::Lfo).unwrap();
    let b = s.modulation.add(2, ModKind::Envelope).unwrap();
    let (ia, ib) = (s.modulation.mods[a].id, s.modulation.mods[b].id);
    assert_ne!(ia, ib);
    assert_eq!(
        s.modulation.connect(ia, "instrument.3.cutoff", 0.4),
        Some(0)
    );
    assert_eq!(
        s.modulation.connect(ia, "instrument.3.cutoff", 0.9),
        Some(0)
    );
    assert_eq!(s.modulation.routes[0].amount, 0.4);
    s.modulation.connect(ib, "instrument.1.fx.1.p2", -0.2);
    s.modulation.connect(ia, "instrument.2.patch.amp-s", 0.1);
    assert_eq!(s.modulation.connect(999, "master", 0.1), None);
    assert_eq!(s.modulation.routes_to("instrument.3.cutoff").count(), 1);
    // a depth lane on route 3 (index 2)
    s.automation
        .push(Lane::new(Target::ModAmount(2), "depth".into()));
    // deleting instrument 2: its routes go, instrument 3 becomes 2, the
    // depth lane of the removed route goes with it
    s.delete_instrument(1);
    let m = &s.modulation;
    assert_eq!(m.mods.len(), 2);
    assert_eq!(m.mods[1].owner, 1);
    let t: Vec<&str> = m.routes.iter().map(|r| r.target.as_str()).collect();
    assert_eq!(t, vec!["instrument.2.cutoff", "instrument.1.fx.1.p2"]);
    assert!(s.automation.is_empty());
    // removing a modulator removes its routes and shifts depth lanes
    s.automation
        .push(Lane::new(Target::ModAmount(1), "depth".into()));
    s.remove_modulator(0);
    assert_eq!(s.modulation.routes.len(), 1);
    assert_eq!(s.automation[0].target, "mod.1.amount");
    s.remove_route(0);
    assert!(s.modulation.routes.is_empty() && s.automation.is_empty());
    // renumbering
    let mut s = song();
    s.instruments.truncate(2);
    let k = s.modulation.add(1, ModKind::Lfo).unwrap();
    let id = s.modulation.mods[k].id;
    s.modulation.connect(id, "instrument.2.resonance", 0.3);
    s.modulation.remap(&[1, 0]);
    assert_eq!(s.modulation.mods[0].owner, 0);
    assert_eq!(s.modulation.routes[0].target, "instrument.1.resonance");
}

// -------------------------------------------------------------------- engine

fn stepped(m: &Modulation, s: &Song, eng: &mut ModEngine, secs: f32) -> Vec<f32> {
    let dt = 32.0 / SR;
    (0..(secs / dt) as usize)
        .map(|_| {
            eng.step(m, s, dt, 120.0);
            eng.out[0]
        })
        .collect()
}

#[test]
fn lfo_runs_at_its_synced_rate_and_retriggers() {
    let s = song();
    let mut m = Modulation::default();
    let k = m.add(0, ModKind::Lfo).unwrap();
    m.mods[k].shape = LfoShape::SawUp;
    m.mods[k].sync = 1.0; // a beat at 120 BPM = 0.5 s
    let mut e = ModEngine::default();
    let v = stepped(&m, &s, &mut e, 2.0);
    let wraps = v.windows(2).filter(|w| w[1] < w[0] - 1.0).count();
    assert_eq!(wraps, 3, "4 cycles in 2 s → 3 wraps inside");
    // retrigger: a note puts the phase back to the start
    m.mods[k].retrig = true;
    m.mods[k].sync = 0.0;
    m.mods[k].rate = 0.1;
    let mut e = ModEngine::default();
    let a = stepped(&m, &s, &mut e, 1.0);
    assert!(*a.last().unwrap() > -0.85);
    e.feed.note_on(0, 37, 1.0);
    let b = stepped(&m, &s, &mut e, 0.01);
    assert!(b[0] <= -0.99, "{}", b[0]);
    // unipolar
    m.mods[k].unipolar = true;
    let mut e = ModEngine::default();
    let u = stepped(&m, &s, &mut e, 0.01);
    assert!(u[0].abs() < 1e-3);
}

#[test]
fn envelope_follows_the_owners_notes() {
    let s = song();
    let mut m = Modulation::default();
    let k = m.add(0, ModKind::Envelope).unwrap();
    m.mods[k].a = 0.05;
    m.mods[k].d = 0.1;
    m.mods[k].s = 0.5;
    m.mods[k].r = 0.1;
    let mut e = ModEngine::default();
    assert!(stepped(&m, &s, &mut e, 0.1).iter().all(|x| *x == 0.0));
    // another instrument's note does nothing
    e.feed.note_on(1, 37, 1.0);
    e.feed.held[1] = true;
    assert!(stepped(&m, &s, &mut e, 0.1).iter().all(|x| *x == 0.0));
    e.feed.note_on(0, 37, 1.0);
    e.feed.held[0] = true;
    let att = stepped(&m, &s, &mut e, 0.04);
    assert!(att.windows(2).all(|w| w[1] >= w[0]) && *att.last().unwrap() > 0.6);
    let sus = stepped(&m, &s, &mut e, 0.5);
    assert!(
        (sus.last().unwrap() - 0.5).abs() < 0.01,
        "{}",
        sus.last().unwrap()
    );
    e.feed.held[0] = false;
    let rel = stepped(&m, &s, &mut e, 0.3);
    assert!(*rel.last().unwrap() < 0.01);
    // velocity / keytrack / wheel / macro
    for (kind, want) in [
        (ModKind::Velocity, 0.75),
        (ModKind::Keytrack, (61.0 - 49.0) / 60.0),
        (ModKind::ModWheel, 0.25),
        (ModKind::Macro, 0.5),
    ] {
        let mut m = Modulation::default();
        m.add(0, kind).unwrap();
        let mut e = ModEngine::default();
        e.feed.note_on(0, 61, 0.75);
        e.feed.wheel = 0.25;
        let v = stepped(&m, &s, &mut e, 0.01);
        assert!((v[0] - want).abs() < 1e-5, "{kind:?} {}", v[0]);
    }
}

#[test]
fn random_steps_are_repeatable_and_in_range() {
    let s = song();
    let mut m = Modulation::default();
    let k = m.add(0, ModKind::Random).unwrap();
    m.mods[k].rate = 8.0;
    let a = stepped(&m, &s, &mut ModEngine::default(), 1.0);
    let b = stepped(&m, &s, &mut ModEngine::default(), 1.0);
    assert_eq!(a, b);
    assert!(a.iter().all(|x| (-1.0..=1.0).contains(x)));
    let mut d = a.clone();
    d.dedup();
    assert!((6..=10).contains(&d.len()), "{} steps", d.len());
}

// ----------------------------------------------------------------- rendering

#[test]
fn an_lfo_on_the_cutoff_opens_and_closes_the_filter() {
    let base = song();
    let s = routed(base.clone(), ModKind::Lfo, "instrument.1.cutoff", 0.4);
    let mut s2 = s.clone();
    s2.modulation.mods[0].shape = LfoShape::Square;
    s2.modulation.mods[0].sync = 2.0; // a cycle per second: up 0.5 s, down 0.5 s
    let out = left(&render(&s2, 2.0));
    let dry = left(&render(&base, 2.0));
    let w = |x: &[f32], a: f32| bright(&x[(a * SR) as usize..(a * SR) as usize + 4000]);
    // square up (first half of each second): brighter than the plain patch,
    // down: darker
    assert!(
        w(&out, 0.2) > 1.5 * w(&dry, 0.2),
        "{} {}",
        w(&out, 0.2),
        w(&dry, 0.2)
    );
    assert!(
        w(&out, 0.7) < 0.7 * w(&dry, 0.7),
        "{} {}",
        w(&out, 0.7),
        w(&dry, 0.7)
    );
    assert!(w(&out, 1.2) > 1.5 * w(&dry, 1.2));
    assert!(out.iter().all(|x| x.is_finite()));
    // the render is the same every time (modulators restart with the song)
    assert_eq!(render(&s2, 1.0), render(&s2, 1.0));
}

#[test]
fn zero_depth_and_unrouted_modulators_leave_the_sound_unchanged() {
    let base = song();
    let want = render(&base, 1.0);
    // modulators without routes
    let mut a = base.clone();
    a.modulation.add(0, ModKind::Lfo);
    assert_eq!(render(&a, 1.0), want);
    // a route at zero depth on several kinds of target
    let mut b = base.clone();
    let k = b.modulation.add(0, ModKind::Lfo).unwrap();
    let id = b.modulation.mods[k].id;
    for t in [
        "instrument.1.cutoff",
        "instrument.1.patch.amp-s",
        "track.1.volume",
        "instrument.1.level",
    ] {
        b.modulation.connect(id, t, 0.0).unwrap();
    }
    // (the value goes through its normalised form: float rounding only)
    let got = render(&b, 1.0);
    let err = got
        .iter()
        .zip(&want)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max);
    assert!(err < 1e-4, "{err}");
    // a depth lane at the centre (= 0) overrides a full route depth
    let mut c = routed(base.clone(), ModKind::Lfo, "instrument.1.cutoff", 1.0);
    let mut l = Lane::new(Target::ModAmount(0), "depth".into());
    l.points.push(Point {
        pos: 0.0,
        value: 0.5,
        shape: Shape::Step,
    });
    c.automation.push(l);
    let got = render(&c, 1.0);
    let err = got
        .iter()
        .zip(&want)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max);
    assert!(err < 1e-4, "{err}");
    // .. and without the lane it does change the sound
    c.automation.clear();
    assert_ne!(render(&c, 1.0), want);
}

#[test]
fn modulation_adds_on_top_of_an_automation_lane() {
    // lane holds the cutoff low; a macro at full adds +0.5 of the range
    let mut base = song();
    let mut l = Lane::new(Target::Cutoff(0), "cut".into());
    l.points.push(Point {
        pos: 0.0,
        value: 0.2,
        shape: Shape::Step,
    });
    base.automation.push(l);
    let mut s = routed(base.clone(), ModKind::Macro, "instrument.1.cutoff", 0.5);
    s.modulation.mods[0].value = 1.0;
    // the same as a lane at 0.7
    let mut same = base.clone();
    same.automation[0].points[0].value = 0.7;
    let (a, b) = (render(&s, 0.5), render(&same, 0.5));
    let err = a
        .iter()
        .zip(&b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max);
    assert!(err < 1e-4, "{err}");
}

#[test]
fn patch_parameters_and_fx_parameters_are_modulatable() {
    // an envelope on the amp sustain: with sustain 0 and an envelope that
    // holds at 1, the note keeps sounding
    let mut base = song();
    if let InstrumentKind::Synth(p) = &mut base.instruments[0].kind {
        p.amp.s = 0.0;
        p.amp.d = 0.05;
    }
    let mut s = routed(
        base.clone(),
        ModKind::Envelope,
        "instrument.1.patch.amp-s",
        1.0,
    );
    s.modulation.mods[0].s = 1.0;
    s.modulation.mods[0].a = 0.001;
    let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
    let held = left(&render(&s, 1.0));
    let plain = left(&render(&base, 1.0));
    assert!(rms(&plain[30000..40000]) < 1e-3);
    assert!(
        rms(&held[30000..40000]) > 0.02,
        "{}",
        rms(&held[30000..40000])
    );
    // a built-in FX parameter: the chain gets a Drive
    // device whose drive the LFO moves
    let mut f = song();
    f.instruments[0].fx = vec![crate::fx::Device::new(crate::fx::FxKind::Drive)];
    f.instruments[0].fx[0].params[0] = 0.0;
    let a = render(&f, 1.0);
    let mut g = routed(f.clone(), ModKind::Macro, "instrument.1.fx.1.p1", 1.0);
    g.modulation.mods[0].value = 1.0;
    assert_ne!(render(&g, 1.0), a);
    // tempo is not modulated
    let t = routed(song(), ModKind::Macro, "tempo", 1.0);
    let mut e = Engine::new(SR);
    e.start(&t, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 4096];
    e.render(&t, &mut buf);
    assert_eq!(e.bpm(), 120.0);
}

#[test]
fn modulators_run_while_stopped() {
    let s = routed(song(), ModKind::Lfo, "instrument.1.cutoff", 0.5);
    let mut e = Engine::new(SR);
    let mut buf = vec![0.0f32; 4800];
    let mut seen = vec![];
    for _ in 0..10 {
        e.render(&s, &mut buf);
        seen.push(e.mods.out[0]);
    }
    seen.dedup();
    assert!(seen.len() > 5, "{seen:?}");
}

#[test]
fn rendering_with_modulation_is_allocation_free() {
    let mut s = song();
    s.instruments[0].fx = vec![
        crate::fx::Device::new(crate::fx::FxKind::Chorus),
        crate::fx::Device::new(crate::fx::FxKind::Delay),
    ];
    for (kind, t) in [
        (ModKind::Lfo, "instrument.1.cutoff"),
        (ModKind::Envelope, "instrument.1.patch.pulse_width"),
        (ModKind::Random, "instrument.1.fx.1.p1"),
        (ModKind::Velocity, "track.1.pan"),
        (ModKind::Keytrack, "instrument.1.level"),
        (ModKind::InstrumentLfo, "master"),
    ] {
        s = routed(s, kind, t, 0.3);
    }
    let mut l = Lane::new(Target::ModAmount(0), "depth".into());
    l.points.push(Point {
        pos: 0.0,
        value: 0.2,
        shape: Shape::Linear,
    });
    l.points.push(Point {
        pos: 16.0,
        value: 0.9,
        shape: Shape::Linear,
    });
    s.automation.push(l);
    // notes on every 4th row keep the envelopes busy
    for r in (0..s.patterns[0].rows).step_by(4) {
        s.patterns[0].cells[r][0] = Cell {
            note: 37 + (r % 12) as u8,
            instr: 1,
            ..Default::default()
        };
    }
    let mut e = Engine::new(SR);
    e.start(&s, 0, 0, true, 0);
    let mut buf = vec![0.0f32; 512];
    for _ in 0..200 {
        e.render(&s, &mut buf);
    }
    let n = alloc_count::count(|| {
        for _ in 0..2000 {
            e.render(&s, &mut buf);
        }
    });
    assert_eq!(n, 0, "render with modulation allocated {n} times");
    assert!(buf.iter().all(|x| x.is_finite()));
}

// ------------------------------------------------------------------ files

#[test]
fn modulation_round_trips_through_cwt_and_old_songs_have_none() {
    let mut s = routed(song(), ModKind::Lfo, "instrument.1.cutoff", 0.35);
    let k = s.modulation.add(0, ModKind::Envelope).unwrap();
    s.modulation.mods[k].name = "Pluck".into();
    s.modulation.mods[k].a = 0.2;
    let id = s.modulation.mods[k].id;
    s.modulation.connect(id, "instrument.1.patch.amp-s", -0.5);
    let dir = std::env::temp_dir().join(format!("cw-mod-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("m.cwt");
    crate::fileio::save_cwt(&s, &p).unwrap();
    let back = crate::fileio::load_cwt(&p).unwrap();
    assert_eq!(back.modulation, s.modulation);
    // without modulation: no modulation.toml in the archive
    let q = dir.join("plain.cwt");
    crate::fileio::save_cwt(&song(), &q).unwrap();
    let f = std::fs::File::open(&q).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    assert!(z.by_name("modulation.toml").is_err());
    assert!(crate::fileio::load_cwt(&q).unwrap().modulation.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sanitize_drops_bad_entries_and_clamps() {
    let mut m = Modulation::default();
    let a = m.add(0, ModKind::Lfo).unwrap();
    m.mods[a].rate = 1e9;
    m.mods[a].s = 7.0;
    let id = m.mods[a].id;
    let mut dup = m.mods[a].clone();
    dup.owner = 0;
    m.mods.push(dup); // duplicate id
    let mut far = Modulator::new(50, 9, ModKind::Envelope); // no instrument 10
    far.owner = 9;
    m.mods.push(far);
    m.routes.push(ModRoute {
        src: id,
        target: "instrument.1.cutoff".into(),
        amount: 3.0,
    });
    m.routes.push(ModRoute {
        src: 50,
        target: "instrument.1.cutoff".into(),
        amount: 0.1,
    });
    m.routes.push(ModRoute {
        src: id,
        target: "nonsense".into(),
        amount: 0.1,
    });
    let m = crate::fileio::sanitize_modulation(m, 2);
    assert_eq!(m.mods.len(), 1);
    assert!(m.mods[0].rate <= 100.0 && m.mods[0].s <= 1.0);
    assert_eq!(m.routes.len(), 1);
    assert_eq!(m.routes[0].amount, 1.0);
}

#[test]
fn text_songs_read_a_modulation_table() {
    let t = r#"
[song]
title = "m"
bpm = 120
tracks = 1
[[instrument]]
name = "Lead"
patch = "builtin:init"
[[modulation.modulator]]
id = 1
owner = 0
kind = "Lfo"
rate = 2.0
[[modulation.route]]
src = 1
target = "instrument.1.cutoff"
amount = 0.25
"#;
    let s = crate::fileio::song_from_text(t).unwrap();
    assert_eq!(s.modulation.mods.len(), 1);
    assert_eq!(s.modulation.mods[0].rate, 2.0);
    assert_eq!(s.modulation.routes[0].amount, 0.25);
}
