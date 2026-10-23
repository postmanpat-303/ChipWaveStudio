//! StringWave engine tests (ported from the stand-alone build): every
//! control is live, edits don't click, levels match, nothing blows up.
#![allow(clippy::field_reassign_with_default, clippy::type_complexity)]
use super::engine::{Engine, Snap};
use super::*;

const SR: f32 = 48000.0;
const BLK: usize = 64;

fn hf_rms(x: &[f32]) -> f32 {
    let mut acc = 0.0f64;
    for w in x.windows(2) {
        let d = (w[1] - w[0]) as f64;
        acc += d * d;
    }
    (acc / x.len() as f64).sqrt() as f32
}
fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>() / x.len() as f64).sqrt() as f32
}
fn max_step(x: &[f32]) -> f32 {
    x.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max)
}
fn seg(x: &[f32], a: f32, b: f32) -> &[f32] {
    &x[(a * SR) as usize..(b * SR) as usize]
}
fn snap(p: &StringsP) -> Snap {
    Snap::new(p, &|_| None)
}

/// Render `secs` of a held chord (left channel). `edit(t, patch)` runs
/// before every block and may change the patch; the snapshot is re-read
/// every block, as the app does.
fn run(
    p0: &StringsP,
    secs: f32,
    notes: &[u8],
    mut edit: impl FnMut(f32, &mut StringsP),
) -> Vec<f32> {
    let mut p = p0.clone();
    let mut e = Engine::new(SR);
    let n = (secs * SR) as usize;
    let mut out = vec![0.0f32; n];
    let mut r = vec![0.0f32; BLK];
    let s = snap(&p);
    for &k in notes {
        e.note_on(&s, k, 0.8, None, 0);
    }
    let mut pos = 0;
    while pos < n {
        edit(pos as f32 / SR, &mut p);
        let s = snap(&p);
        let end = (pos + BLK).min(n);
        e.render(&s, &mut out[pos..end], &mut r[..end - pos]);
        pos = end;
    }
    out
}

/// peak (dBFS) and deepest limiter reduction of a 5-note chord
fn chord_level(p: &StringsP, secs: f32) -> (f32, f32) {
    let mut e = Engine::new(SR);
    let s = snap(p);
    for k in [48, 55, 60, 64, 67] {
        e.note_on(&s, k, 0.8, None, 0);
    }
    let (mut l, mut r) = (vec![0.0; BLK], vec![0.0; BLK]);
    let (mut peak, mut gr) = (0.0f32, 0.0f32);
    for _ in 0..(secs * SR) as usize / BLK {
        e.render(&s, &mut l, &mut r);
        peak = l.iter().chain(r.iter()).fold(peak, |a, &x| a.max(x.abs()));
        gr = gr.min(e.meter_gr);
    }
    (20.0 * peak.log10(), gr)
}

#[test]
fn cutoff_change_mid_note_changes_output() {
    let p = StringsP::default();
    let reference = run(&p, 2.0, &[48, 60, 64], |_, _| {});
    let live = run(&p, 2.0, &[48, 60, 64], |t, p| {
        if t >= 1.0 {
            p.set(&[(CUTOFF, 250.0)]);
        }
    });
    let (hr, hl) = (
        hf_rms(seg(&reference, 1.3, 1.9)),
        hf_rms(seg(&live, 1.3, 1.9)),
    );
    assert!(hl < hr * 0.5, "cutoff change mid-note had no effect");
    // the edit lands within ~50 ms
    let (h0, h1) = (
        hf_rms(seg(&reference, 1.05, 1.1)),
        hf_rms(seg(&live, 1.05, 1.1)),
    );
    assert!(h1 < h0 * 0.8, "cutoff change too slow to take effect");
    // no click: around the edit the steps stay like the steady signal's
    let ms = max_step(seg(&live, 0.98, 1.1));
    let ref_ms = max_step(seg(&reference, 0.5, 1.9));
    assert!(ms <= ref_ms * 1.5 + 1e-3, "cutoff edit clicks");
}

#[test]
fn a_change_lands_within_one_block() {
    // the engine reads the patch every block (64 samples): the output
    // differs from the reference inside the block after the edit
    let p = StringsP::default();
    let reference = run(&p, 0.6, &[57], |_, _| {});
    let at = 0.5;
    let live = run(&p, 0.6, &[57], |t, p| {
        if t >= at {
            p.set(&[(LAYER[0] + L_LVL, 0.0), (LAYER[1] + L_LVL, 0.0)]);
        }
    });
    let i = (at * SR) as usize / BLK * BLK + BLK;
    let d: f32 = reference[i..i + BLK]
        .iter()
        .zip(&live[i..i + BLK])
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(d > 1e-4, "no change within 64 samples of the edit ({d})");
}

#[test]
fn level_and_power_changes_mid_note() {
    let mut p = StringsP::default();
    // effects off so their tails do not mask the dry level
    p.cho_on = false;
    p.echo_on = false;
    p.rev_on = false;
    let mut stage = 0;
    let live = run(&p, 3.0, &[55, 62], |t, p| {
        if t >= 1.0 && stage == 0 {
            stage = 1;
            for b in LAYER {
                p.p[b + L_LVL] = 0.0;
            }
            p.und_on = false;
        }
        if t >= 2.0 && stage == 1 {
            stage = 2;
            p.p[LAYER[1] + L_LVL] = 1.0; // layer 2 back, and solo it
            p.layer_solo[1] = true;
        }
    });
    let (a, b, c) = (
        rms(seg(&live, 0.6, 0.95)),
        rms(seg(&live, 1.3, 1.9)),
        rms(seg(&live, 2.3, 2.9)),
    );
    assert!(b < a * 0.1, "level change mid-note had no effect");
    assert!(
        c > b * 5.0,
        "turning a level back up mid-note had no effect"
    );
    let st = max_step(seg(&live, 0.98, 1.1));
    let st_ref = max_step(seg(&live, 0.5, 0.95));
    assert!(st <= st_ref * 1.2 + 1e-3, "level change clicks");
}

#[test]
fn every_continuous_control_is_live() {
    // a held note; each knob is moved after 0.5 s: the output must differ
    let p = StringsP::default();
    let reference = run(&p, 2.0, &[57], |_, _| {});
    let mut dead = vec![];
    for (k, key) in KEYS.iter().enumerate() {
        // values that only act at note start/end, during the mod envelope,
        // with the mod wheel up, or on more voices than one note uses
        if [
            "aa", "ar", "fa", "fr", "me_a", "me_d", "me_amt", "vel", "wheel", "voices",
        ]
        .contains(key)
        {
            continue;
        }
        let target = if p.p[k] > 0.5 { 0.0 } else { 1.0 };
        let live = run(&p, 2.0, &[57], |t, p| {
            if t >= 0.5 {
                p.p[k] = target;
            }
        });
        let a = seg(&reference, 0.6, 2.0);
        let b = seg(&live, 0.6, 2.0);
        let diff = a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32;
        if diff < 1e-4 {
            dead.push(*key);
        }
    }
    // the switches too
    let flips: [(&str, fn(&mut StringsP)); 8] = [
        ("layer_on", |p| p.layer_on[0] = false),
        ("wave", |p| p.wave[0] = Wave::Plucked),
        ("und_on", |p| p.und_on = false),
        ("filter_on", |p| p.filter_on = false),
        ("f24", |p| p.f24 = true),
        ("lfo_on", |p| p.lfo_on[1] = false),
        ("cho_on", |p| p.cho_on = false),
        ("rev_on", |p| p.rev_on = false),
    ];
    for (name, f) in flips {
        let live = run(&p, 2.0, &[57], |t, p| {
            if t >= 0.5 {
                f(p);
            }
        });
        let a = seg(&reference, 0.6, 2.0);
        let b = seg(&live, 0.6, 2.0);
        let diff = a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32;
        if diff < 1e-4 {
            dead.push(name);
        }
    }
    assert!(
        dead.is_empty(),
        "these controls did nothing to a sounding note: {dead:?}"
    );
}

#[test]
fn an_override_applies_mid_note() {
    // automation / 43xy / MIDI learn arrive as knob overrides (0..1)
    let p = StringsP::default();
    let reference = run(&p, 2.0, &[48, 60], |_, _| {});
    let mut e = Engine::new(SR);
    let s = snap(&p);
    for k in [48, 60] {
        e.note_on(&s, k, 0.8, None, 0);
    }
    let n = (2.0 * SR) as usize;
    let mut out = vec![0.0f32; n];
    let mut r = vec![0.0f32; BLK];
    let mut pos = 0;
    while pos < n {
        let ov = |k: usize| (pos as f32 >= SR && k == CUTOFF).then_some(0.05);
        let s = Snap::new(&p, &ov);
        let end = (pos + BLK).min(n);
        e.render(&s, &mut out[pos..end], &mut r[..end - pos]);
        pos = end;
    }
    let (hr, hl) = (
        hf_rms(seg(&reference, 1.3, 1.9)),
        hf_rms(seg(&out, 1.3, 1.9)),
    );
    assert!(hl < hr * 0.5, "an override had no effect mid-note");
}

#[test]
fn default_chord_loudness() {
    let (db, gr) = chord_level(&StringsP::default(), 4.0);
    assert!(
        (-4.5..=-2.5).contains(&db),
        "default chord peak {db:.2} dBFS out of range"
    );
    assert!(gr > -1.5, "the limiter is doing the work ({gr:.2} dB)");
}

#[test]
fn a_full_velocity_cluster_does_not_clip() {
    let mut e = Engine::new(SR);
    let s = snap(&StringsP::default());
    for k in 0..16u8 {
        e.note_on(&s, 40 + k, 1.0, None, 0);
    }
    let (mut l, mut r) = (vec![0.0; BLK], vec![0.0; BLK]);
    let mut peak = 0.0f32;
    for _ in 0..(4.0 * SR) as usize / BLK {
        e.render(&s, &mut l, &mut r);
        peak = l.iter().chain(r.iter()).fold(peak, |a, &x| a.max(x.abs()));
    }
    let db = 20.0 * peak.log10();
    assert!(db < -0.1, "16-note cluster peak {db:.2} dBFS");
}

#[test]
fn presets_are_level_matched() {
    for (name, _, p) in presets::presets() {
        let (db, gr) = chord_level(&p, 6.0);
        assert!((-5.0..=-2.5).contains(&db), "{name}: peak {db:.2} dBFS");
        assert!(gr > -2.0, "{name}: limiter working too hard ({gr:.2} dB)");
    }
}

struct Stats {
    peak: f32,
    rms: f32,
    nan: bool,
}

/// events: (time_s, note, vel); vel 0 = off
fn render(s: &Snap, secs: f32, events: &[(f32, u8, f32)]) -> (Stats, Vec<f32>) {
    let mut e = Engine::new(SR);
    let n = (secs * SR) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let mut ev = events.to_vec();
    ev.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut k = 0;
    let mut pos = 0;
    while pos < n {
        while k < ev.len() && (ev[k].0 * SR) as usize <= pos {
            let (_, note, vel) = ev[k];
            if vel > 0.0 {
                e.note_on(s, note, vel, None, 0);
            } else {
                e.note_off(note, 0);
            }
            k += 1;
        }
        let end = (pos + BLK).min(n);
        e.render(s, &mut l[pos..end], &mut r[pos..end]);
        pos = end;
    }
    let mut st = Stats {
        peak: 0.0,
        rms: 0.0,
        nan: false,
    };
    let mut acc = 0.0f64;
    for &x in l.iter().chain(r.iter()) {
        st.nan |= !x.is_finite();
        st.peak = st.peak.max(x.abs());
        acc += (x as f64) * (x as f64);
    }
    st.rms = (acc / (2.0 * n as f64)).sqrt() as f32;
    (st, l)
}

fn chord(t_on: f32, t_off: f32, notes: &[u8], vel: f32) -> Vec<(f32, u8, f32)> {
    notes
        .iter()
        .flat_map(|&n| [(t_on, n, vel), (t_off, n, 0.0)])
        .collect()
}

fn check(name: &str, st: &Stats) {
    assert!(!st.nan, "{name}: NaN/inf in output");
    assert!(st.rms > 0.003, "{name}: output is (nearly) silent");
    assert!(st.peak < 0.995, "{name}: output clipping");
}

#[test]
fn render_default_and_waves() {
    let base = snap(&StringsP::default());
    let ev = chord(0.05, 3.0, &[48, 55, 60, 64, 67], 0.8);
    check("default chord", &render(&base, 6.0, &ev).0);
    for w in Wave::ALL {
        let mut s = base;
        for l in s.layers.iter_mut() {
            l.on = true;
            l.wave = w;
            l.lvl = 0.8;
        }
        s.cutoff = 9000.0;
        s.reso = 0.6;
        s.a_adsr = [0.002, 1.0, 0.8, 0.5];
        check(w.name(), &render(&s, 4.0, &ev).0);
    }
    // 40-note pile-up with voice stealing, loud velocities, low and high notes
    let mut ev = vec![];
    for i in 0..40u8 {
        let t = 0.02 + i as f32 * 0.05;
        let note = 28 + ((i as u32 * 7) % 72) as u8;
        ev.push((t, note, 1.0));
        ev.push((t + 1.5, note, 0.0));
    }
    let mut s = base;
    s.a_adsr = [0.002, 0.5, 1.0, 1.0];
    s.reso = 0.9;
    s.echo.2 = 0.95;
    check("40-note steal stress", &render(&s, 6.0, &ev).0);
}

#[test]
fn a4_is_440_hz() {
    let mut s = snap(&StringsP::default());
    s.layers[0].wave = Wave::Ensemble;
    s.layers[0].oct = 0;
    s.layers[0].det = 0.0;
    s.layers[1].on = false;
    s.layers[2].on = false;
    s.und = false;
    s.sub_lvl = 0.0;
    s.drone_lvl = 0.0;
    s.lfo[0].2 = 0.0;
    s.lfo[1].2 = 0.0;
    s.cho.0 = false;
    s.echo.0 = false;
    s.rev.0 = false;
    s.cutoff = 12000.0;
    s.a_adsr = [0.002, 1.0, 1.0, 0.3];
    let (st, l) = render(&s, 1.5, &chord(0.0, 1.4, &[69], 0.8));
    assert!(!st.nan && st.rms > 0.001, "single note silent");
    // the period by autocorrelation (the first strong peak, in samples)
    let x = seg(&l, 0.3, 0.8);
    let ac = |lag: usize| -> f32 { x.iter().zip(&x[lag..]).map(|(a, b)| a * b).sum::<f32>() };
    let (best, _) = (60..200)
        .map(|lag| (lag, ac(lag)))
        .fold((0, f32::MIN), |m, x| if x.1 > m.1 { x } else { m });
    // refine between samples (parabola through the peak)
    let (a, b, c) = (ac(best - 1), ac(best), ac(best + 1));
    let lag = best as f32 + 0.5 * (a - c) / (a - 2.0 * b + c);
    let hz = SR / lag;
    assert!((hz - 440.0).abs() < 2.0, "A4 measured {hz:.1} Hz");
    for w in Wave::ALL {
        let mut s2 = s;
        s2.layers[0].wave = w;
        let (st, _) = render(&s2, 1.0, &chord(0.0, 0.8, &[69], 0.8));
        assert!(!st.nan && st.rms > 0.001, "{} single note silent", w.name());
    }
}

#[test]
fn the_voices_knob_limits_polyphony() {
    let mut p = StringsP::default();
    p.set(&[(VOICES, 4.0)]);
    let s = snap(&p);
    assert_eq!(s.voices, 4);
    let mut e = Engine::new(SR);
    for k in 0..8u8 {
        e.note_on(&s, 48 + k, 0.8, None, 0);
    }
    let (mut l, mut r) = (vec![0.0; BLK], vec![0.0; BLK]);
    for _ in 0..200 {
        e.render(&s, &mut l, &mut r);
    }
    assert!(e.active_voices() <= 4, "{} voices", e.active_voices());
}

#[test]
fn patches_round_trip_through_files_by_name() {
    for (name, _, p) in presets::presets() {
        let t = toml::to_string(&p).unwrap();
        let back: StringsP = toml::from_str(&t).unwrap();
        assert_eq!(back, p, "{name}");
    }
    // unknown keys are ignored, missing ones keep their defaults
    let p: StringsP = toml::from_str("[knobs]\ncutoff = 0.25\nzzz = 1.0\n").unwrap();
    assert_eq!(p.p[CUTOFF], 0.25);
    assert_eq!(p.p[RESO], StringsP::default().p[RESO]);
    assert_eq!(STRINGS.len(), KEYS.len());
    const _: () = assert!(SN <= crate::gen::GEN_KNOBS);
}

#[test]
fn cpu_16_voices() {
    let s = snap(&StringsP::default());
    let mut e = Engine::new(SR);
    for n in 0..16u8 {
        e.note_on(&s, 40 + n * 2, 0.8, None, 0);
    }
    let secs = 2.0;
    let (mut l, mut r) = (vec![0.0; BLK], vec![0.0; BLK]);
    let t = std::time::Instant::now();
    for _ in 0..((secs * SR) as usize / BLK) {
        e.render(&s, &mut l, &mut r);
    }
    let el = t.elapsed().as_secs_f32();
    println!(
        "StringWave 16 voices: {:.1}% of one core ({:.0}x realtime)",
        100.0 * el / secs,
        secs / el
    );
}
