use super::dsp::*;
use super::*;
use crate::gen::Ov;

const SR: f32 = 48000.0;

/// render `n` samples of one note (gate held for `held` samples)
fn note(p: &PolyP, f: f32, n: usize, held: usize, vel: f32) -> (Vec<f32>, Vec<f32>, PolyState) {
    let mut s = PolyState::default();
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b, _) = s.render(p, Ov::NONE, f, SR, i < held, vel);
        l.push(a);
        r.push(b);
    }
    (l, r, s)
}

fn rising_crossings(x: &[f32]) -> usize {
    x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
}

/// power at frequency `f` (Hz) over `x` (a single DFT bin)
fn bin(x: &[f32], f: f32) -> f32 {
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let a = std::f64::consts::TAU * f as f64 * i as f64 / SR as f64;
        // Hann window
        let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / x.len() as f64).cos();
        re += *v as f64 * w * a.cos();
        im += *v as f64 * w * a.sin();
    }
    ((re * re + im * im) / (x.len() as f64).powi(2)) as f32
}

fn db(x: f32) -> f32 {
    10.0 * x.max(1e-30).log10()
}

#[test]
fn knob_table_is_consistent() {
    assert_eq!(POLY.len(), PN);
    assert_eq!(KEYS.len(), PN);
    let mut k = KEYS.to_vec();
    k.sort();
    k.dedup();
    assert_eq!(k.len(), PN, "duplicate file keys");
    for (i, d) in POLY.iter().enumerate() {
        assert!(!d.exp || d.lo > 0.0, "{} exp needs lo > 0", d.name);
        assert!(d.dice.0 <= d.dice.1, "{}", d.name);
        let mid = d.map(0.37);
        assert!((d.unmap(mid) - 0.37).abs() < 1e-4, "{} map/unmap", d.name);
        assert!(!d.name.is_empty() && !d.tip.is_empty(), "knob {i}");
    }
    assert_eq!(knob_label(OSC[1] + VOICES), "Osc 2: Voices");
    assert_eq!(POLY[MOD_AMT].name, "Mod 1");
    // the new units read as semitones and footage
    assert_eq!(
        POLY[OSC[0] + PITCH].show(POLY[OSC[0] + PITCH].unmap(1.5)),
        "+1.50 st"
    );
    assert_eq!(POLY[OSC[0] + OCT].show(POLY[OSC[0] + OCT].unmap(0.0)), "8'");
    assert_eq!(
        POLY[OSC[0] + OCT].show(POLY[OSC[0] + OCT].unmap(-2.0)),
        "32'"
    );
    assert_eq!(POLY[OSC[0] + OCT].show(POLY[OSC[0] + OCT].unmap(3.0)), "1'");
}

#[test]
fn shape_morphs_pulse_saw_octave_saw() {
    // -1: pulse
    let w = weights(-1.0);
    assert_eq!(wave(0.2, 0.5, w), 1.0);
    assert_eq!(wave(0.7, 0.5, w), -1.0);
    // 0: saw
    let w = weights(0.0);
    assert!((wave(0.25, 0.5, w) + 0.5).abs() < 1e-6);
    assert!((wave(0.75, 0.5, w) - 0.5).abs() < 1e-6);
    // +1 at PW 50 %: a saw at twice the frequency
    let w = weights(1.0);
    assert!((wave(0.125, 0.5, w) + 0.5).abs() < 1e-6);
    assert!((wave(0.625, 0.5, w) + 0.5).abs() < 1e-6);
    // weights sum to one in between
    for s in [-0.7f32, -0.2, 0.3, 0.9] {
        let (a, b, c) = weights(s);
        assert!((a + b + c - 1.0).abs() < 1e-6);
    }
}

#[test]
fn blend_operators() {
    let (a, b) = (0.6f32, -0.4f32);
    assert_eq!(blend(BlendOp::Mix, a, b, 0.0), a);
    assert_eq!(blend(BlendOp::Mix, a, b, 1.0), b);
    assert!((blend(BlendOp::Mix, a, b, 0.5) - 0.1).abs() < 1e-6);
    assert!((blend(BlendOp::Neg, a, b, 0.5) - 0.5).abs() < 1e-6);
    assert!(
        (blend(BlendOp::Neg, a, a, 0.5)).abs() < 1e-7,
        "NEG cancels equal inputs"
    );
    // WIPE: same ends as MIX, holds each side longer
    assert_eq!(blend(BlendOp::Wipe, a, b, 1.0), b);
    assert!((blend(BlendOp::Wipe, a, b, 0.5) - 0.1).abs() < 1e-6);
    assert!(blend(BlendOp::Wipe, a, b, 0.2) > blend(BlendOp::Mix, a, b, 0.2));
    // AM: full depth = a·b
    assert!((blend(BlendOp::Am, a, b, 1.0) - a * b).abs() < 1e-6);
    assert_eq!(blend(BlendOp::Am, a, b, 0.0), a);
    // SIGN: |b| with a's sign
    assert!((blend(BlendOp::Sign, -0.3, 0.8, 1.0) + 0.8).abs() < 1e-6);
    assert!((blend(BlendOp::Sign, 0.3, -0.8, 1.0) - 0.8).abs() < 1e-6);
    // MAX
    assert!((blend(BlendOp::Max, a, b, 1.0) - a).abs() < 1e-6);
    assert!((blend(BlendOp::Max, -0.5, 0.2, 1.0) - 0.2).abs() < 1e-6);
}

#[test]
fn pitch_follows_note_octave_and_semitones() {
    let n = SR as usize;
    let p = PolyP::default();
    let (l, _, _) = note(&p, 440.0, n, n, 1.0);
    let c = rising_crossings(&l) as f32;
    assert!((c - 440.0).abs() <= 2.0, "440 Hz saw: {c} cycles");
    let mut p2 = p.clone();
    p2.set(&[(OSC[0] + OCT, 1.0)]);
    let c = rising_crossings(&note(&p2, 440.0, n, n, 1.0).0) as f32;
    assert!((c - 880.0).abs() <= 3.0, "octave up: {c}");
    let mut p3 = p.clone();
    p3.set(&[(OSC[0] + OCT, -2.0)]);
    let c = rising_crossings(&note(&p3, 440.0, n, n, 1.0).0) as f32;
    assert!((c - 110.0).abs() <= 1.0, "two octaves down: {c}");
    let mut p4 = p.clone();
    p4.set(&[(OSC[0] + PITCH, 7.0)]);
    let c = rising_crossings(&note(&p4, 200.0, n, n, 1.0).0) as f32;
    let want = 200.0 * 2f32.powf(7.0 / 12.0);
    assert!((c - want).abs() <= 2.0, "+7 st: {c} vs {want}");
}

#[test]
fn hard_sync_keeps_the_master_period() {
    // 480 Hz at 48 kHz: a period of exactly 100 samples
    let mut p = PolyP::default();
    p.set(&[(OSC[0] + SYNC, 7.0)]);
    let (l, _, _) = note(&p, 480.0, 6000, 6000, 1.0);
    let mut err = 0.0f32;
    let mut pk = 0.0f32;
    for i in 3000..5000 {
        err = err.max((l[i] - l[i + 100]).abs());
        pk = pk.max(l[i].abs());
    }
    assert!(pk > 0.2);
    assert!(
        err < 0.02 * pk,
        "synced wave repeats every master cycle: {err}"
    );
    // and it differs from the plain saw
    let (s, _, _) = note(&PolyP::default(), 480.0, 6000, 6000, 1.0);
    let d: f32 = (3000..3100).map(|i| (s[i] - l[i]).abs()).sum();
    assert!(d > 1.0);
}

#[test]
fn blep_saw_has_little_aliasing() {
    // 5 kHz saw: harmonics above 24 kHz fold back to non-harmonic bins
    let f = 5000.0;
    let n = 16384;
    let (l, _, _) = note(&PolyP::default(), f, n + 2000, n + 2000, 1.0);
    let x = &l[2000..];
    let fund = bin(x, f);
    // 5·5000 = 25000 → 23000, 6·5000 → 18000 ... these land between harmonics?
    // (with f = 5000 they do not) so use a detuned note instead
    let _ = fund;
    let f = 5150.0;
    let (l, _, _) = note(&PolyP::default(), f, n + 2000, n + 2000, 1.0);
    let x = &l[2000..];
    let fund = bin(x, f);
    let mut worst = -200.0f32;
    for k in 5..12 {
        let a = (k as f32 * f) % SR;
        let a = if a > SR / 2.0 { SR - a } else { a };
        worst = worst.max(db(bin(x, a)) - db(fund));
    }
    // a naive saw for comparison
    let mut ph = 0.0f32;
    let naive: Vec<f32> = (0..n)
        .map(|_| {
            ph = (ph + f / SR).fract();
            2.0 * ph - 1.0
        })
        .collect();
    let nf = bin(&naive, f);
    let mut worst_naive = -200.0f32;
    for k in 5..12 {
        let a = (k as f32 * f) % SR;
        let a = if a > SR / 2.0 { SR - a } else { a };
        worst_naive = worst_naive.max(db(bin(&naive, a)) - db(nf));
    }
    eprintln!("alias: blep {worst:.1} dB, naive {worst_naive:.1} dB");
    assert!(
        worst < worst_naive - 6.0,
        "blep {worst} naive {worst_naive}"
    );
    assert!(worst < -24.0, "{worst}");
}

fn sine_gain(mode: PolyFilter, res: f32, fc: f32, f: f32) -> f32 {
    let mut fl = Filt::default();
    let g = (std::f32::consts::PI * fc / SR).tan();
    let k = damping(mode, res);
    let n = 24000;
    let mut pk = 0.0f32;
    for i in 0..n {
        let x = (std::f32::consts::TAU * f * i as f32 / SR).sin();
        let y = fl.run(mode, x, g, k);
        if i > n / 2 {
            pk = pk.max(y.abs());
        }
    }
    pk
}

#[test]
fn filter_responses() {
    use PolyFilter::*;
    let fc = 1000.0;
    // low-pass: passes lows, 2-pole gentler than 4-pole
    assert!((sine_gain(Lp2, 0.0, fc, 100.0) - 1.0).abs() < 0.05);
    assert!((sine_gain(Lp4, 0.0, fc, 100.0) - 1.0).abs() < 0.05);
    let l2 = db(sine_gain(Lp2, 0.0, fc, 8000.0).powi(2));
    let l4 = db(sine_gain(Lp4, 0.0, fc, 8000.0).powi(2));
    assert!(l2 < -30.0 && l2 > -45.0, "LP 12 at 3 octaves: {l2}");
    assert!(l4 < -60.0, "LP 24 at 3 octaves: {l4}");
    // Butterworth 4-pole: -3 dB at the cutoff
    let c = db(sine_gain(Lp4, 0.0, fc, fc).powi(2));
    assert!((c + 3.0).abs() < 1.0, "LP 24 at fc: {c}");
    // high-pass mirrors it
    assert!((sine_gain(Hp2, 0.0, fc, 10000.0) - 1.0).abs() < 0.08);
    assert!(db(sine_gain(Hp4, 0.0, fc, 125.0).powi(2)) < -60.0);
    assert!(db(sine_gain(Hp2, 0.0, fc, 125.0).powi(2)) > -45.0);
    // band-pass: unity at the centre, falls away both sides; 4-pole steeper
    for m in [Bp2, Bp4] {
        let c = sine_gain(m, 0.5, fc, fc);
        assert!((c - 1.0).abs() < 0.1, "{m:?} centre {c}");
        assert!(sine_gain(m, 0.5, fc, 100.0) < 0.3);
        assert!(sine_gain(m, 0.5, fc, 10000.0) < 0.3);
    }
    assert!(sine_gain(Bp4, 0.5, fc, 4000.0) < sine_gain(Bp2, 0.5, fc, 4000.0));
    // band-reject: a notch at the cutoff, flat away from it
    assert!(sine_gain(Br, 0.0, fc, fc) < 0.05);
    assert!((sine_gain(Br, 0.0, fc, 100.0) - 1.0).abs() < 0.05);
    assert!((sine_gain(Br, 0.0, fc, 10000.0) - 1.0).abs() < 0.08);
    // resonance makes a peak at the cutoff
    assert!(sine_gain(Lp2, 0.9, fc, fc) > 3.0);
    assert!(sine_gain(Lp4, 0.9, fc, fc) > 3.0);
}

#[test]
fn envelopes_attack_decay_release() {
    let dt = 1.0 / SR;
    let c = Env::coefs(0.1, 0.2, 0.3, dt, 0.0015);
    let mut e = Env::default();
    let mut n = 0;
    while e.stage == ATTACK {
        e.next(c, 0.5);
        n += 1;
    }
    assert!((n as f32 - 0.1 * SR).abs() < 2.0, "attack {n}");
    for _ in 0..(SR as usize) {
        e.next(c, 0.5);
    }
    assert!((e.level - 0.5).abs() < 1e-3 && e.stage == SUSTAIN);
    e.release();
    let mut n = 0;
    while !e.done() {
        e.next(c, 0.5);
        n += 1;
    }
    // exponential release: -40 dB from 0.5 in about 0.3 s × ln(5000)/5
    let t = n as f32 / SR;
    assert!(t > 0.4 && t < 0.6, "release {t}");
    // retrigger starts again from the current level (no jump)
    let mut e = Env {
        stage: SUSTAIN,
        level: 0.4,
    };
    e.retrigger();
    let v = e.next(c, 0.5);
    assert!(v > 0.4 && v < 0.41);
    // a decay to zero sustain ends the envelope
    let mut e = Env::default();
    let c = Env::coefs(0.001, 0.05, 0.1, dt, 0.0015);
    for _ in 0..(SR as usize) {
        e.next(c, 0.0);
    }
    assert!(e.done());
}

#[test]
fn notes_end_after_release_and_velocity_scales() {
    let p = PolyP::default();
    let (l, _, s) = note(&p, 220.0, 48000, 12000, 1.0);
    assert!(s.done, "voice ends after the release");
    assert!(l[40000..].iter().all(|v| *v == 0.0));
    let pk = |v: &[f32]| v.iter().fold(0.0f32, |a, x| a.max(x.abs()));
    let loud = pk(&note(&p, 220.0, 4800, 4800, 1.0).0);
    let soft = pk(&note(&p, 220.0, 4800, 4800, 0.2).0);
    // Vel > amp 50 %: velocity 0.2 → 60 %
    assert!((soft / loud - 0.6).abs() < 0.03, "{soft} {loud}");
}

#[test]
fn unison_spreads_stereo_and_keeps_level() {
    let mut p = PolyP::default();
    p.set_osc(0, 0.0, 0.5, 0.0, 0, 0.0, 9, 40.0, 1.0);
    p.reset = [false, false];
    let (l, r, _) = note(&p, 220.0, 24000, 24000, 1.0);
    let d: f32 = l.iter().zip(&r).map(|(a, b)| (a - b).abs()).sum();
    assert!(d > 100.0, "unison is stereo");
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    let (m, _, _) = note(&PolyP::default(), 220.0, 24000, 24000, 1.0);
    let ratio = rms(&l) / rms(&m);
    assert!(
        ratio > 0.5 && ratio < 1.5,
        "1/sqrt(n) keeps the level: {ratio}"
    );
}

#[test]
fn glide_and_retrigger() {
    let p = PolyP::default().clone();
    let mut p = p;
    p.set(&[(GLIDE, 0.2), (AEG + 2, 0.5)]);
    let mut s = PolyState::default();
    s.glide_from = 220.0;
    let mut l = vec![];
    for _ in 0..48000 {
        l.push(s.render(&p, Ov::NONE, 440.0, SR, true, 1.0).0);
    }
    // first 10 ms near 220 Hz, the last half second at 440 Hz
    let early = rising_crossings(&l[0..2400]) as f32 / 0.05;
    let late = rising_crossings(&l[24000..]) as f32 / 0.5;
    assert!(early < 330.0, "starts low: {early}");
    assert!((late - 440.0).abs() < 4.0, "arrives: {late}");
    assert!((s.sounding() - 440.0).abs() < 1.0);
    // retrigger restarts the amp envelope from where it is
    let lvl = s.aeg.level;
    s.retrigger = true;
    s.render(&p, Ov::NONE, 440.0, SR, true, 1.0);
    assert!((lvl - 0.5).abs() < 0.01);
    assert_eq!(s.aeg.stage, ATTACK);
    assert!(s.aeg.level > lvl && s.aeg.level - lvl < 0.02, "no jump");
}

#[test]
fn mod_routes_move_knobs() {
    // velocity → cutoff: a soft note is darker
    let mut p = PolyP::default();
    p.set(&[(CUTOFF, 300.0)]);
    p.route(0, ModSrc::Vel, CUTOFF, 0.8);
    let hf = |v: f32| {
        let (l, _, _) = note(&p, 110.0, 12000, 12000, v);
        l.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>()
    };
    let (hard, soft) = (hf(1.0), hf(0.1));
    assert!(hard > soft * 1.5, "{hard} {soft}");
}

#[test]
fn presets_render_clean_and_round_trip() {
    let ps = presets::presets();
    assert_eq!(ps[0].0, concat!(poly_name!(), " Init"));
    assert_eq!(ps.len(), 11);
    for (name, _, p) in &ps {
        // round trip through the file format
        let j = serde_json::to_string(p).unwrap();
        let back: PolyP = serde_json::from_str(&j).unwrap();
        assert_eq!(back.p, p.p, "{name}: bit-exact knobs");
        assert_eq!(back.blend, p.blend);
        assert_eq!(back.filter, p.filter);
        assert_eq!(back.mode, p.mode);
        assert_eq!(back.routes, p.routes);
        // it sounds, stays bounded and ends
        let held = 96000;
        let n = held + 48000 * 6;
        let (l, r, s) = note(p, 130.81, n, held, 0.9);
        let pk = l.iter().chain(&r).fold(0.0f32, |a, x| a.max(x.abs()));
        assert!(pk.is_finite() && pk < 2.0, "{name} peak {pk}");
        assert!(pk > 0.02, "{name} is silent ({pk})");
        assert!(s.done, "{name} never ends");
    }
}

#[test]
fn file_format_ignores_unknown_and_defaults_missing() {
    let j = r#"{"knobs":{"cutoff":0.25,"bogus":3.0},"filter":"Hp2","polyphony":3}"#;
    let p: PolyP = serde_json::from_str(j).unwrap();
    assert_eq!(p.p[CUTOFF], 0.25);
    assert_eq!(p.filter, PolyFilter::Hp2);
    assert_eq!(p.polyphony, 3);
    assert_eq!(p.p[GAIN], PolyP::default().p[GAIN]);
    // out-of-range values are clamped, never NaN
    let j = r#"{"knobs":{"cutoff":7.0,"res":-4.0},"polyphony":99}"#;
    let p: PolyP = serde_json::from_str(j).unwrap();
    assert_eq!(p.p[CUTOFF], 1.0);
    assert_eq!(p.p[RES], 0.0);
    assert_eq!(p.polyphony, MAX_POLY);
}

#[test]
fn render_does_not_allocate() {
    let p = presets::by_name(concat!(poly_name!(), " Supersaw Pad")).unwrap();
    let mut s = PolyState::default();
    s.render(&p, Ov::NONE, 220.0, SR, true, 1.0);
    let n = crate::engine::alloc_count::count(|| {
        for i in 0..48000 {
            s.render(&p, Ov::NONE, 220.0, SR, i < 24000, 1.0);
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn songs_with_a_polysynth_need_reader_5_and_round_trip() {
    use crate::model::InstrumentKind;
    let mut s =
        crate::fileio::song_from_text(include_str!("../../demos/first_run.cwt.toml")).unwrap();
    assert!(
        !crate::fileio::needs_reader_5(&s),
        "old songs keep their version"
    );
    let mut g = crate::gen::preset(concat!(poly_name!(), " Pluck")).unwrap();
    if let crate::gen::GenEngine::Poly(x) = &mut g.engine {
        x.route(2, ModSrc::Key, OSC[0] + PW, -0.4);
        x.locks = 0b101;
    }
    s.instruments[0].kind = InstrumentKind::Gen(g);
    assert!(crate::fileio::needs_reader_5(&s));
    let dir = std::env::temp_dir().join(format!("cw-poly-rt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("poly.cwt");
    crate::fileio::save_cwt(&s, &f).unwrap();
    let l = crate::fileio::load_cwt(&f).unwrap();
    assert_eq!(s.instruments[0].kind, l.instruments[0].kind);
    let mut z = zip::ZipArchive::new(std::fs::File::open(&f).unwrap()).unwrap();
    let mut m = String::new();
    std::io::Read::read_to_string(&mut z.by_name("manifest.toml").unwrap(), &mut m).unwrap();
    assert!(m.contains("min_reader_version = 5"), "{m}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn display_name_lives_in_one_constant() {
    assert_eq!(POLY_DISPLAY_NAME, poly_name!());
    for (n, tip, p) in presets::presets() {
        assert!(tip.starts_with(POLY_DISPLAY_NAME), "{n}");
        // the file tag is the neutral "Poly", never the display name
        let g = crate::gen::GenEngine::Poly(p);
        let j = serde_json::to_string(&g).unwrap();
        assert!(j.starts_with("{\"Poly\":"), "{j}");
        if POLY_DISPLAY_NAME != "Poly" {
            assert!(!j.contains(POLY_DISPLAY_NAME));
        }
    }
}
