//! The quick-mod adapter: add / remove per engine onto the data each engine
//! already saves, the fallbacks, and a save / load round trip.
use super::*;
use crate::model::{Instrument, InstrumentKind};

fn inst(kind: InstrumentKind) -> Instrument {
    Instrument {
        name: "T".into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind,
    }
}
fn gen(e: GenEngine) -> Instrument {
    inst(InstrumentKind::Gen(crate::gen::GenPatch {
        engine: e,
        ..Default::default()
    }))
}
fn poly() -> Instrument {
    gen(GenEngine::Poly(Default::default()))
}
fn strings() -> Instrument {
    gen(GenEngine::Strings(Default::default()))
}
fn synth() -> Instrument {
    inst(InstrumentKind::Synth(Default::default()))
}
fn spec(source: Source, amount: f32) -> Spec {
    Spec {
        source,
        amount,
        ..Default::default()
    }
}
fn lbl(s: &str) -> String {
    s.to_string()
}
fn ser(i: &Instrument) -> String {
    serde_json::to_string(&i.kind).unwrap()
}

#[test]
fn waveforge_envelope_and_velocity_use_its_per_note_slots() {
    use crate::poly::*;
    let mut i = poly();
    let mut m = Modulation::default();
    let before = rows(&i, 0, &m, &lbl).len();
    let a = add(
        &mut i,
        0,
        &mut m,
        Target::Gen(0, 3),
        &spec(Source::Envelope, 0.4),
        120.0,
    )
    .unwrap();
    let RowId::Native(Native::Poly(s)) = a.row else {
        panic!("{a:?}")
    };
    let InstrumentKind::Gen(g) = &i.kind else {
        panic!()
    };
    let GenEngine::Poly(p) = &g.engine else {
        panic!()
    };
    assert_eq!(p.routes[s as usize].src, ModSrc::Feg);
    assert_eq!(p.routes[s as usize].dst, 3);
    assert!((POLY[MOD_AMT + s as usize].map(p.p[MOD_AMT + s as usize]) - 0.4).abs() < 1e-3);
    assert!(m.is_empty(), "no song-level modulator for a per-note slot");
    let r = rows(&i, 0, &m, &lbl);
    assert_eq!(r.len(), before + 1);
    let row = r.iter().find(|r| r.id == a.row).unwrap();
    assert!(row.per_note);
    assert_eq!(row.knobs, vec![Target::Gen(0, 3)]);
    // amount edits the slot's own (automatable) amount knob
    set_amount(&mut i, &mut m, a.row, -0.5);
    let r = rows(&i, 0, &m, &lbl);
    assert!((r.iter().find(|r| r.id == a.row).unwrap().amount + 0.5).abs() < 1e-3);
    remove_native(&mut i, Native::Poly(s));
    assert_eq!(rows(&i, 0, &m, &lbl).len(), before);
}

#[test]
fn waveforge_full_slots_fall_back_with_a_message() {
    let mut i = poly();
    let mut m = Modulation::default();
    let mut native = 0;
    for k in 0..crate::poly::MOD_SLOTS {
        if let Ok(a) = add(
            &mut i,
            0,
            &mut m,
            Target::Gen(0, k),
            &spec(Source::Velocity, 0.2),
            120.0,
        ) {
            if matches!(a.row, RowId::Native(_)) {
                native += 1;
            }
        }
    }
    let free_before = native;
    assert!(free_before >= 1);
    // fill whatever is left, then one more
    let mut last = None;
    for _ in 0..crate::poly::MOD_SLOTS + 1 {
        last = Some(
            add(
                &mut i,
                0,
                &mut m,
                Target::Gen(0, 10),
                &spec(Source::Velocity, 0.2),
                120.0,
            )
            .unwrap(),
        );
    }
    let a = last.unwrap();
    assert!(matches!(a.row, RowId::Song(_)), "{a:?}");
    assert!(a.note.unwrap().contains("per-note slots are full"));
}

#[test]
fn waveforge_lfo_is_song_level() {
    let mut i = poly();
    let mut m = Modulation::default();
    let a = add(
        &mut i,
        2,
        &mut m,
        Target::Gen(2, 5),
        &spec(Source::Lfo, 0.3),
        120.0,
    )
    .unwrap();
    assert_eq!(a.row, RowId::Song(0));
    assert_eq!(m.routes[0].target, Target::Gen(2, 5).id());
    assert_eq!(m.mods[0].owner, 2);
    // a second knob with the same LFO shares the modulator
    add(
        &mut i,
        2,
        &mut m,
        Target::Gen(2, 6),
        &spec(Source::Lfo, 0.3),
        120.0,
    )
    .unwrap();
    assert_eq!(m.mods.len(), 1);
    assert_eq!(m.routes.len(), 2);
    let r = rows(&i, 2, &m, &lbl);
    assert_eq!(
        r.iter().filter(|r| matches!(r.id, RowId::Song(_))).count(),
        2
    );
    assert!(r[r.len() - 1].source.starts_with("LFO 1 sine · 1 beat"));
}

#[test]
fn stringwave_uses_its_lfos_and_mod_env_then_falls_back() {
    use crate::strings::*;
    let mut i = strings();
    let mut m = Modulation::default();
    // Velvet Ensemble: both LFOs already move pitch / cutoff
    let r0 = rows(&i, 0, &m, &lbl);
    assert_eq!(r0.len(), 2, "{r0:?}");
    assert_eq!(r0[0].target, "Pitch");
    assert_eq!(r0[1].target, "Cutoff");
    // the LFOs are full: a third LFO goes song-level, with a note
    let a = add(
        &mut i,
        0,
        &mut m,
        Target::Gen(0, RESO),
        &spec(Source::Lfo, 0.3),
        120.0,
    )
    .unwrap();
    assert!(matches!(a.row, RowId::Song(_)));
    assert!(a.note.unwrap().contains("LFOs are both in use"));
    // free one, add again: native, tempo-synced 1/4 at 120 bpm = 2 Hz
    remove_native(&mut i, Native::StrLfo(0));
    let a = add(
        &mut i,
        0,
        &mut m,
        Target::Gen(0, RESO),
        &spec(Source::Lfo, 0.3),
        120.0,
    )
    .unwrap();
    assert_eq!(a.row, RowId::Native(Native::StrLfo(0)));
    let InstrumentKind::Gen(g) = &i.kind else {
        panic!()
    };
    let GenEngine::Strings(p) = &g.engine else {
        panic!()
    };
    assert!(p.lfo_on[0]);
    assert_eq!(p.lfo_dest[0], Dest::Resonance);
    assert!((STRINGS[LFO[0]].map(p.p[LFO[0]]) - 2.0).abs() < 0.01);
    assert!((STRINGS[LFO[0] + 1].map(p.p[LFO[0] + 1]) - 0.3).abs() < 0.01);
    // the mod env (on, but amount 0 in the default) is free
    let e = add(
        &mut i,
        0,
        &mut m,
        Target::Gen(0, CHO_MIX),
        &spec(Source::Envelope, -0.5),
        120.0,
    )
    .unwrap();
    assert_eq!(e.row, RowId::Native(Native::StrEnv));
    let r = rows(&i, 0, &m, &lbl);
    let row = r.iter().find(|r| r.id == e.row).unwrap();
    assert_eq!(row.target, "Chorus");
    assert!((row.amount + 0.5).abs() < 0.01);
    remove_native(&mut i, Native::StrEnv);
    assert!(rows(&i, 0, &m, &lbl).iter().all(|r| r.id != e.row));
    // a layer's tone knob is the Tone destination (all three layers ring)
    remove_native(&mut i, Native::StrLfo(0));
    add(
        &mut i,
        0,
        &mut m,
        Target::Gen(0, LAYER[1] + L_TONE),
        &spec(Source::Random, 0.2),
        120.0,
    )
    .unwrap();
    let r = rows(&i, 0, &m, &lbl);
    let row = r
        .iter()
        .find(|r| r.id == RowId::Native(Native::StrLfo(0)))
        .unwrap();
    assert_eq!(row.knobs.len(), 3);
    assert!(row.source.contains("s&h"));
}

#[test]
fn chipwave_synth_lfo_takes_cutoff_and_volume() {
    let mut i = synth();
    let mut m = Modulation::default();
    assert!(rows(&i, 0, &m, &lbl).is_empty());
    let a = add(
        &mut i,
        0,
        &mut m,
        Target::Cutoff(0),
        &spec(Source::Lfo, 0.25),
        120.0,
    )
    .unwrap();
    assert_eq!(a.row, RowId::Native(Native::Chip(ChipDst::Cutoff)));
    let InstrumentKind::Synth(p) = &i.kind else {
        panic!()
    };
    assert!((p.lfo.cutoff - 1.0).abs() < 1e-4);
    assert_eq!(p.lfo.sync, 1.0);
    // the same LFO onto volume shares it
    let v = add(
        &mut i,
        0,
        &mut m,
        Target::Level(0),
        &spec(Source::Lfo, 0.5),
        120.0,
    )
    .unwrap();
    assert_eq!(v.row, RowId::Native(Native::Chip(ChipDst::Amp)));
    assert_eq!(rows(&i, 0, &m, &lbl).len(), 2);
    // another rate can't share the one LFO: song-level, with a note
    let mut s = spec(Source::Lfo, 0.5);
    s.sync = 4.0;
    remove_native(&mut i, Native::Chip(ChipDst::Amp));
    let w = add(&mut i, 0, &mut m, Target::Level(0), &s, 120.0).unwrap();
    assert!(matches!(w.row, RowId::Song(_)));
    assert!(w.note.unwrap().contains("one LFO"));
    remove_native(&mut i, Native::Chip(ChipDst::Cutoff));
    let InstrumentKind::Synth(p) = &i.kind else {
        panic!()
    };
    assert!(!p.lfo.active());
}

#[test]
fn other_engines_are_song_level_and_full_says_so() {
    let mut i = inst(InstrumentKind::Sample {
        file: String::new(),
        rate: 44100,
        data: Default::default(),
        base_note: 60,
        lp: Default::default(),
    });
    let mut m = Modulation::default();
    let t = Target::Level(0);
    let a = add(&mut i, 0, &mut m, t, &spec(Source::ModWheel, 0.5), 120.0).unwrap();
    assert_eq!(a.row, RowId::Song(0));
    assert_eq!(a.note, None);
    // fill the song's modulators: a different rate each time
    for n in 1..crate::modulation::MAX_MODS {
        let mut s = spec(Source::Lfo, 0.1);
        s.sync = 0.0;
        s.rate = n as f32 * 0.1;
        add(&mut i, 0, &mut m, Target::Gen(0, n % 30), &s, 120.0).unwrap();
    }
    let mut s = spec(Source::Lfo, 0.1);
    s.sync = 0.0;
    s.rate = 99.0;
    let e = add(&mut i, 0, &mut m, Target::Cutoff(0), &s, 120.0).unwrap_err();
    assert!(e.starts_with("slots full"), "{e}");
    // the tempo can't be modulated
    let e = add(
        &mut i,
        0,
        &mut m,
        Target::Tempo,
        &spec(Source::Velocity, 0.5),
        120.0,
    )
    .unwrap_err();
    assert!(e.contains("can't be modulated"));
}

#[test]
fn removing_a_song_level_row_goes_through_the_song() {
    let mut song = crate::fileio::demo_song();
    song.instruments.push(synth());
    let k = song.instruments.len() - 1;
    let mut i = song.instruments[k].clone();
    let a = add(
        &mut i,
        k,
        &mut song.modulation,
        Target::Cutoff(k),
        &spec(Source::Envelope, 0.3),
        120.0,
    )
    .unwrap();
    let RowId::Song(r) = a.row else { panic!() };
    song.remove_route(r);
    assert!(rows(&i, k, &song.modulation, &lbl).is_empty());
}

#[test]
fn movements_survive_save_and_load_unchanged() {
    let mut song = crate::fileio::demo_song();
    let bpm = song.bpm;
    song.instruments.push(poly());
    song.instruments.push(strings());
    song.instruments.push(synth());
    let n = song.instruments.len();
    let (wf, sw, cw) = (n - 3, n - 2, n - 1);
    let mut m = std::mem::take(&mut song.modulation);
    for (k, t, s) in [
        (wf, Target::Gen(wf, 3), spec(Source::Envelope, 0.4)),
        (wf, Target::Gen(wf, 4), spec(Source::Lfo, -0.2)),
        (
            sw,
            Target::Gen(sw, crate::strings::CHO_MIX),
            spec(Source::Envelope, 0.3),
        ),
        (cw, Target::Cutoff(cw), spec(Source::Lfo, 0.25)),
        (cw, Target::Level(cw), spec(Source::Random, 0.5)),
    ] {
        add(&mut song.instruments[k], k, &mut m, t, &s, bpm).unwrap();
    }
    song.modulation = m;
    let before: Vec<_> = [wf, sw, cw]
        .iter()
        .map(|&k| {
            (
                ser(&song.instruments[k]),
                rows(&song.instruments[k], k, &song.modulation, &lbl),
            )
        })
        .collect();
    let dir = std::env::temp_dir().join(format!("cw-quickmod-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("rt.cwt");
    crate::fileio::save_cwt(&song, &path).unwrap();
    let back = crate::fileio::load_cwt(&path).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let after: Vec<_> = [wf, sw, cw]
        .iter()
        .map(|&k| {
            (
                ser(&back.instruments[k]),
                rows(&back.instruments[k], k, &back.modulation, &lbl),
            )
        })
        .collect();
    assert_eq!(before, after);
    assert_eq!(song.modulation, back.modulation);
}

#[test]
fn speed_and_length_read_in_plain_words() {
    assert_eq!(speed_text(1.0, 2.0), "1 beat (1/4 note)");
    assert_eq!(speed_text(8.0, 2.0), "2 bars");
    assert_eq!(speed_text(0.25, 2.0), "1/16 note");
    assert_eq!(speed_text(0.0, 3.2), "3.2 Hz");
    assert_eq!(hz_text(0.5), "0.50 Hz");
    assert_eq!(secs_text(0.25), "250 ms");
    assert_eq!(secs_text(1.2), "1.2 s");
    assert_eq!(beats_text(32.0), "8 bars");
}

#[test]
fn song_level_timing_round_trips() {
    let mut i = poly();
    let mut m = Modulation::default();
    let a = add(
        &mut i,
        2,
        &mut m,
        Target::Gen(2, 5),
        &spec(Source::Lfo, 0.3),
        120.0,
    )
    .unwrap();
    let mut t = timing(&i, &m, a.row).unwrap();
    assert_eq!(t.speed.map(|s| s.0), Some(1.0));
    t.speed = Some((16.0, 1.0));
    set_timing(&mut i, &mut m, a.row, &t, 120.0);
    let back = timing(&i, &m, a.row).unwrap();
    assert_eq!(back.speed.map(|s| s.0), Some(16.0));
    let r = rows(&i, 2, &m, &lbl);
    assert!(
        r[r.len() - 1].source.contains("4 bars"),
        "{}",
        r[r.len() - 1].source
    );
}
