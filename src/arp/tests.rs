use super::*;

/// run `a` over `notes` (start, end, key) from `from` to `to` ticks in
/// small steps; the hits
fn run(a: &Arp, kit: bool, notes: &[(f64, f64, u8)], from: f64, to: f64) -> Vec<Hit> {
    let mut st = Stepper::default();
    st.reset(from);
    // a seek: the notes held at `from` go in first (as the engine's restrike)
    let mut i = 0;
    while i < notes.len() && notes[i].0 < from {
        let (s, e, k) = notes[i];
        st.add(a.latch, held(s, e, k));
        i += 1;
    }
    st.expire(from, a.latch);
    let mut out = vec![];
    let mut hits = Hits::default();
    let mut t = from;
    while t < to {
        t += 0.37;
        while i < notes.len() && notes[i].0 <= t {
            let (s, e, k) = notes[i];
            st.add(a.latch, held(s, e, k));
            i += 1;
        }
        st.advance(a, &Mods::default(), kit, t, &mut hits);
        out.extend_from_slice(hits.as_slice());
    }
    out
}

fn held(s: f64, e: f64, key: u8) -> Held {
    Held {
        key,
        vel: 1.0,
        start: s,
        end: e,
        ..Default::default()
    }
}

fn keys(h: &[Hit]) -> Vec<u8> {
    h.iter().map(|h| h.key).collect()
}

/// C E G held for a bar
const CEG: [(f64, f64, u8); 3] = [(0.0, 3840.0, 49), (0.0, 3840.0, 53), (0.0, 3840.0, 56)];

#[test]
fn modes_play_their_orders() {
    let mut a = Arp::default();
    let take = |a: &Arp| keys(&run(a, false, &CEG, 0.0, 240.0 * 8.0 - 1.0));
    assert_eq!(take(&a), [49, 53, 56, 49, 53, 56, 49, 53]);
    a.mode = Mode::Down;
    assert_eq!(take(&a), [56, 53, 49, 56, 53, 49, 56, 53]);
    a.mode = Mode::UpDown;
    assert_eq!(take(&a), [49, 53, 56, 53, 49, 53, 56, 53]);
    a.mode = Mode::DownUp;
    assert_eq!(take(&a), [56, 53, 49, 53, 56, 53, 49, 53]);
    a.mode = Mode::Played;
    let played = [(0.0, 3840.0, 56), (0.0, 3840.0, 49), (0.0, 3840.0, 53)];
    assert_eq!(
        keys(&run(&a, false, &played, 0.0, 239.0 * 3.0)),
        [56, 49, 53]
    );
    a.mode = Mode::Chord;
    let h = run(&a, false, &CEG, 0.0, 239.0 * 2.0);
    assert_eq!(keys(&h), [49, 53, 56, 49, 53, 56]);
    assert_eq!(h[3].start, 240.0);
}

#[test]
fn octaves_extend_the_run() {
    let a = Arp {
        octaves: 2,
        ..Default::default()
    };
    let k = keys(&run(&a, false, &CEG, 0.0, 240.0 * 7.0 - 1.0));
    assert_eq!(k, [49, 53, 56, 61, 65, 68, 49]);
    let a = Arp {
        octaves: 3,
        mode: Mode::UpDown,
        ..Default::default()
    };
    let k = keys(&run(
        &a,
        false,
        &[(0.0, 9000.0, 49)],
        0.0,
        240.0 * 5.0 - 1.0,
    ));
    assert_eq!(k, [49, 61, 73, 61, 49]);
}

#[test]
fn rates_set_the_step_length() {
    for (div, feel, len) in [
        (16, Feel::Straight, 240.0),
        (16, Feel::Triplet, 160.0),
        (16, Feel::Dotted, 360.0),
        (8, Feel::Straight, 480.0),
        (4, Feel::Straight, 960.0),
        (32, Feel::Triplet, 80.0),
    ] {
        let a = Arp {
            div,
            feel,
            ..Default::default()
        };
        let h = run(&a, false, &CEG, 0.0, 3000.0);
        assert!(h.len() >= 3);
        assert!(
            (h[1].start - h[0].start - len).abs() < 1e-6,
            "{div} {feel:?}"
        );
        assert!((h[2].start - h[1].start - len).abs() < 1e-6);
    }
    let mut a = Arp::default();
    a.set_rate_index(a.rate_index() + 1);
    assert_eq!((a.div, a.feel), (16, Feel::Triplet));
}

#[test]
fn gate_sets_the_note_length() {
    let mut a = Arp {
        gate: 0.25,
        ..Default::default()
    };
    let h = run(&a, false, &CEG, 0.0, 239.0);
    assert!((h[0].end - h[0].start - 60.0).abs() < 1e-6);
    a.gate = 1.0;
    a.steps[0].gate = 0.5;
    let h = run(&a, false, &CEG, 0.0, 479.0);
    assert!((h[0].end - h[0].start - 120.0).abs() < 1e-6);
    assert!((h[1].end - h[1].start - 240.0).abs() < 1e-6);
}

#[test]
fn swing_delays_the_offbeats() {
    let a = Arp {
        swing: 0.5,
        ..Default::default()
    };
    let h = run(&a, false, &CEG, 0.0, 959.0);
    let starts: Vec<f64> = h.iter().map(|h| h.start).collect();
    assert_eq!(starts, [0.0, 300.0, 480.0, 780.0]);
}

#[test]
fn rests_ties_ratchets_and_transpose() {
    let mut a = Arp {
        gate: 0.5,
        ..Default::default()
    };
    a.steps[1].kind = StepKind::Rest;
    a.steps[3].kind = StepKind::Tie;
    a.steps[4].ratchet = 3;
    a.steps[5].transpose = 7;
    let h = run(&a, false, &CEG, 0.0, 240.0 * 6.0 - 1.0);
    // a rest plays nothing and does not use up a note
    assert_eq!(keys(&h), [49, 53, 56, 56, 56, 49 + 7]);
    let starts: Vec<f64> = h.iter().map(|h| h.start).collect();
    assert_eq!(starts, [0.0, 480.0, 960.0, 1040.0, 1120.0, 1200.0]);
    // the tie holds step 2's note through step 3
    assert!((h[1].end - (720.0 + 120.0)).abs() < 1e-6);
}

#[test]
fn velocity_and_accent() {
    let mut a = Arp::default();
    a.steps[0].vel = 0.5;
    a.steps[1].vel = 0.5;
    a.steps[1].accent = true;
    let h = run(&a, false, &CEG, 0.0, 479.0);
    assert!((h[0].vel - 0.5).abs() < 1e-6);
    assert!((h[1].vel - 0.65).abs() < 1e-6);
    a.vel_depth = 0.0;
    let h = run(&a, false, &CEG, 0.0, 239.0);
    assert!((h[0].vel - 1.0).abs() < 1e-6);
}

#[test]
fn latch_keeps_the_chord_until_a_new_one() {
    let a = Arp {
        latch: true,
        ..Default::default()
    };
    let notes = [
        (0.0, 300.0, 49),
        (0.0, 300.0, 53),
        // a new chord after every key was up
        (1920.0, 2000.0, 61),
    ];
    let h = run(&a, false, &notes, 0.0, 2400.0);
    // still playing C E after the keys were up at 300
    assert!(h.iter().any(|h| h.start >= 960.0 && h.key == 53));
    // from 1920 only the new note
    assert!(h.iter().filter(|h| h.start >= 1920.0).all(|h| h.key == 61));
    assert!(h.iter().filter(|h| h.start >= 1920.0).count() >= 2);
    // without Latch it stops when the keys go up
    let h = run(&Arp::default(), false, &notes[..2], 0.0, 2400.0);
    assert!(h.iter().all(|h| h.start < 300.0));
    // a live key with Latch
    let mut st = Stepper::default();
    let k = Held {
        id: 60,
        end: f64::INFINITY,
        ..held(0.0, 0.0, 49)
    };
    st.add(true, k);
    assert!(st.release(60, true));
    assert_eq!(st.held().len(), 1);
    st.add(
        true,
        Held {
            id: 61,
            key: 50,
            ..k
        },
    );
    assert_eq!(st.held().len(), 1);
    assert!(st.release(61, false));
    assert!(st.is_empty());
}

#[test]
fn seek_lands_on_the_same_notes() {
    let a = Arp {
        mode: Mode::UpDown,
        octaves: 2,
        swing: 0.3,
        ..Default::default()
    };
    let mut b = a.clone();
    b.steps[2].kind = StepKind::Rest;
    b.steps[5].ratchet = 2;
    b.mode = Mode::Random;
    let notes = [
        (0.0, 3840.0, 49),
        (480.0, 3840.0, 53),
        (960.0, 2900.0, 58),
        (3840.0, 7680.0, 51),
        (3840.0, 7680.0, 56),
    ];
    for a in [&a, &b] {
        let full = run(a, false, &notes, 0.0, 7680.0);
        for from in [960.0, 1440.0, 2000.0, 3840.0, 5000.0] {
            let part = run(a, false, &notes, from, 7680.0);
            let tail: Vec<_> = full.iter().filter(|h| h.start > from).copied().collect();
            let part: Vec<_> = part.iter().filter(|h| h.start > from).copied().collect();
            assert_eq!(keys(&tail), keys(&part), "from {from}");
            assert_eq!(tail, part);
        }
    }
}

#[test]
fn random_is_seeded() {
    let a = Arp {
        mode: Mode::Random,
        octaves: 3,
        seed: 5,
        ..Default::default()
    };
    let x = keys(&run(&a, false, &CEG, 0.0, 3839.0));
    let y = keys(&run(&a, false, &CEG, 0.0, 3839.0));
    assert_eq!(x, y);
    // a pinned sequence: the seed's notes never change between versions
    assert_eq!(x.len(), 16);
    let b = Arp {
        seed: 6,
        ..a.clone()
    };
    assert_ne!(x, keys(&run(&b, false, &CEG, 0.0, 3839.0)));
    let pinned: Vec<u8> = (0..16)
        .map(|k| {
            let j = (hash(5, k) % 9) as usize;
            [49u8, 53, 56][j % 3] + (j / 3) as u8 * 12
        })
        .collect();
    assert_eq!(x, pinned);
    assert_eq!(hash(5, 0), hash(5, 0));
}

#[test]
fn drum_kits_repeat_every_held_pad() {
    let mut a = Arp {
        mode: Mode::Up,
        octaves: 3,
        ..Default::default()
    };
    a.steps[1].ratchet = 4;
    a.steps[1].transpose = 5;
    let pads = [(0.0, 960.0, 37), (0.0, 960.0, 39)];
    let h = run(&a, true, &pads, 0.0, 479.0);
    // both pads on every hit, never moved
    assert_eq!(keys(&h), [37, 39, 37, 39, 37, 39, 37, 39, 37, 39]);
    assert_eq!(h[2].start, 240.0);
    assert_eq!(h[4].start, 300.0);
}

#[test]
fn nothing_stays_on_after_the_keys() {
    // every hit ends, and no hit starts after the last note's end
    let a = Arp {
        gate: 1.0,
        ..Default::default()
    };
    let h = run(&a, false, &[(0.0, 1000.0, 49)], 0.0, 4000.0);
    assert!(h.iter().all(|h| h.start < 1000.0 && h.end > h.start));
    assert!(h.iter().all(|h| h.end <= 1200.0));
}

#[test]
fn bake_writes_the_arp_notes() {
    let a = Arp::default();
    let n = bake(
        &a,
        false,
        3840.0,
        &[(0.0, 960.0, 49, 1.0), (0.0, 960.0, 56, 1.0)],
        960.0,
    );
    let ks: Vec<u8> = n.iter().map(|x| x.2).collect();
    assert_eq!(ks, [49, 56, 49, 56]);
    assert_eq!(n[1].0, 240.0);
    assert!((n[1].1 - 360.0).abs() < 1e-6);
}

#[test]
fn readout_in_plain_words() {
    let mut a = Arp::default();
    a.steps[2].kind = StepKind::Rest;
    a.steps[4].ratchet = 2;
    let s = a.describe(false);
    assert_eq!(
        s,
        "Plays the held notes going up, one every 1/16, 50% long · 16 steps: 1 rest, 1 ratchet"
    );
    assert!(a
        .describe(true)
        .starts_with("Repeats the held pads every 1/16"));
    assert!(presets(false).len() >= 5 && presets(true).len() >= 3);
}

#[test]
fn old_json_has_no_arp_and_defaults_fill_in() {
    let a: Arp = serde_json::from_str("{\"mode\":\"Down\"}").unwrap();
    assert_eq!(a.mode, Mode::Down);
    assert_eq!(a.steps.len(), 16);
}

// ---------------------------------------------------------------- arp2

fn preset(name: &str) -> Arp {
    presets(false)
        .into_iter()
        .find(|p| p.0 == name)
        .unwrap_or_else(|| panic!("no preset {name}"))
        .1
}

#[test]
fn converge_and_diverge() {
    let mut a = Arp {
        mode: Mode::Converge,
        ..Default::default()
    };
    let cegb = [
        (0.0, 3840.0, 49),
        (0.0, 3840.0, 53),
        (0.0, 3840.0, 56),
        (0.0, 3840.0, 60),
    ];
    let take = |a: &Arp| keys(&run(a, false, &cegb, 0.0, 240.0 * 8.0 - 1.0));
    assert_eq!(take(&a), [49, 60, 53, 56, 49, 60, 53, 56]);
    a.mode = Mode::Diverge;
    assert_eq!(take(&a), [56, 53, 60, 49, 56, 53, 60, 49]);
}

#[test]
fn octave_modes() {
    let mut a = Arp {
        octaves: 2,
        oct_mode: OctMode::Down,
        ..Default::default()
    };
    let k = keys(&run(&a, false, &CEG, 0.0, 240.0 * 7.0 - 1.0));
    assert_eq!(k, [49, 53, 56, 37, 41, 44, 49]);
    a.octaves = 3;
    a.oct_mode = OctMode::Bounce;
    let one = [(0.0, 9000.0, 49)];
    let k = keys(&run(&a, false, &one, 0.0, 240.0 * 6.0 - 1.0));
    assert_eq!(k, [49, 61, 73, 61, 49, 61]);
    // Chord mode walks the octaves too
    a.mode = Mode::Chord;
    a.oct_mode = OctMode::Down;
    a.octaves = 2;
    let k = keys(&run(&a, false, &CEG, 0.0, 240.0 * 2.0 - 1.0));
    assert_eq!(k, [49, 53, 56, 37, 41, 44]);
}

#[test]
fn fast_chord_rates_and_names() {
    for (div, feel, len) in [
        (64, Feel::Straight, 60.0),
        (64, Feel::Triplet, 40.0),
        (128, Feel::Straight, 30.0),
    ] {
        let a = Arp {
            div,
            feel,
            ..Default::default()
        };
        let h = run(&a, false, &CEG, 0.0, 400.0);
        assert!((h[1].start - h[0].start - len).abs() < 1e-6, "{div}");
    }
    assert_eq!(rate_name(64, Feel::Triplet), "1/96");
    // the old rate table is the start of the new one (saved rates stay)
    assert_eq!(RATES[7], (16, Feel::Straight));
    assert_eq!(RATES.len(), 15);
    // Chip Fast Chord: C E G cycling on every 1/96 (one 50 Hz frame at 125 BPM)
    let a = preset("Chip Fast Chord");
    let h = run(&a, false, &CEG, 0.0, 40.0 * 6.0 - 1.0);
    assert_eq!(keys(&h), [49, 53, 56, 49, 53, 56]);
    assert!((h[1].start - 40.0).abs() < 1e-6);
    let frame = 60.0 / 125.0 * (40.0 / 960.0);
    assert!((frame - 0.02f64).abs() < 1e-9, "1/96 at 125 BPM is 20 ms");
}

#[test]
fn step_probability_is_seeded() {
    let mut a = Arp::default();
    for s in &mut a.steps {
        s.chance = 0.5;
    }
    let long = [(0.0, 240.0 * 400.0, 49)];
    let h1 = run(&a, false, &long, 0.0, 240.0 * 400.0 - 1.0);
    let h2 = run(&a, false, &long, 0.0, 240.0 * 400.0 - 1.0);
    assert_eq!(h1, h2, "the same every play");
    assert!((150..250).contains(&h1.len()), "about half: {}", h1.len());
    // a seek lands on the same steps
    let tail = run(&a, false, &long, 240.0 * 100.0 - 1.0, 240.0 * 400.0 - 1.0);
    let want: Vec<_> = h1
        .iter()
        .filter(|h| h.start >= 240.0 * 100.0)
        .copied()
        .collect();
    assert_eq!(
        tail.iter().map(|h| h.start).collect::<Vec<_>>(),
        want.iter().map(|h| h.start).collect::<Vec<_>>()
    );
    // a new seed is another pattern; 0 % never plays, 100 % always
    a.seed = 99;
    assert_ne!(run(&a, false, &long, 0.0, 240.0 * 400.0 - 1.0), h1);
    a.steps[0].chance = 0.0;
    a.len = 1;
    assert!(run(&a, false, &long, 0.0, 9000.0).is_empty());
    a.steps[0].chance = 1.0;
    assert_eq!(run(&a, false, &long, 0.0, 240.0 * 10.0 - 1.0).len(), 10);
}

#[test]
fn humanise_varies_velocity_seeded() {
    let mut a = Arp::default();
    let h = run(&a, false, &CEG, 0.0, 3000.0);
    assert!(h.iter().all(|h| h.vel == 1.0));
    a.humanise = 1.0;
    for s in &mut a.steps {
        s.vel = 0.6;
    }
    let h = run(&a, false, &CEG, 0.0, 3000.0);
    let v: Vec<f32> = h.iter().map(|h| h.vel).collect();
    assert!(v.iter().any(|&x| (x - 0.6).abs() > 0.02), "{v:?}");
    assert!(v
        .iter()
        .all(|&x| (0.6 * 0.6 - 1e-4..=0.6 * 1.4 + 1e-4).contains(&x)));
    assert_eq!(run(&a, false, &CEG, 0.0, 3000.0), h);
    // modulation adds humanise
    let mut m = Mods::default();
    a.humanise = 0.0;
    Param::Humanise.apply(&a, 1.0, &mut m);
    assert!((m.humanise - 1.0).abs() < 1e-6);
}

#[test]
fn euclid_fills_steps() {
    let e = |h, l, r| -> String {
        euclid(h, l, r)
            .iter()
            .map(|&b| if b { 'x' } else { '.' })
            .collect()
    };
    assert_eq!(e(3, 8, 0), "x..x..x.");
    assert_eq!(e(4, 16, 0), "x...x...x...x...");
    assert_eq!(e(5, 8, 0).matches('x').count(), 5);
    assert_eq!(e(3, 8, 1), ".x..x..x");
    assert_eq!(e(0, 4, 0), "....");
    assert_eq!(e(9, 4, 0), "xxxx");
    let mut a = Arp::default();
    a.euclid_fill(3, 8, 0);
    assert_eq!(a.len, 8);
    let h = run(&a, false, &CEG, 0.0, 240.0 * 8.0 - 1.0);
    let starts: Vec<f64> = h.iter().map(|h| h.start).collect();
    assert_eq!(starts, [0.0, 720.0, 1440.0]);
    // the run moves on only on played steps
    assert_eq!(keys(&h), [49, 53, 56]);
    let p = preset("Euclid 5/8 Pluck");
    assert_eq!(p.len, 8);
}

#[test]
fn sustain_pedal_holds_released_keys() {
    let a = Arp::default();
    let mut st = Stepper::default();
    let mut hits = Hits::default();
    let key = |k: u8| Held {
        key: k,
        vel: 1.0,
        start: 0.0,
        end: f64::INFINITY,
        id: k,
        ..Default::default()
    };
    st.add(false, key(49));
    st.add(false, key(53));
    st.sustain(true, false);
    st.release(49, false);
    st.release(53, false);
    let mut out = vec![];
    let mut t = 0.0;
    while t < 950.0 {
        t += 10.0;
        st.advance(&a, &Mods::default(), false, t, &mut hits);
        out.extend_from_slice(hits.as_slice());
    }
    assert_eq!(keys(&out), [49, 53, 49, 53], "the pedal holds them");
    // a key played under the pedal joins
    st.add(
        false,
        Held {
            start: t,
            ..key(56)
        },
    );
    st.release(56, false);
    assert_eq!(st.held().len(), 3);
    st.sustain(false, false);
    assert!(st.is_empty(), "pedal up lets go");
    // with Latch the pedal-up notes stay (until a new chord)
    st.add(false, key(49));
    st.sustain(true, true);
    st.release(49, true);
    st.sustain(false, true);
    assert_eq!(st.held().len(), 1);
    assert!(st.held()[0].up);
}

#[test]
fn scale_lock_keeps_steps_in_key() {
    // C major
    let cmaj = crate::chords::Key::new(0, crate::chords::Scale::Major).mask();
    assert_eq!(scale_step(49, 1, cmaj), 51); // C -> D
    assert_eq!(scale_step(49, 2, cmaj), 53); // C -> E
    assert_eq!(scale_step(49, -1, cmaj), 48); // C -> B
    assert_eq!(scale_step(50, 0, cmaj), 49); // C# snaps to C
    assert_eq!(scale_step(49, 7, cmaj), 61); // an octave in steps
    let mut a = Arp {
        len: 4,
        ..Default::default()
    };
    for (k, s) in a.steps.iter_mut().enumerate().take(4) {
        s.transpose = k as i8;
    }
    let go = |a: &Arp, key: u16| {
        let mut st = Stepper::default();
        let mut hits = Hits::default();
        st.add(false, held(0.0, 9000.0, 49));
        let m = Mods {
            key,
            ..Default::default()
        };
        let mut out = vec![];
        let mut t = 0.0;
        while t < 959.0 {
            t += 7.0;
            st.advance(a, &m, false, t, &mut hits);
            out.extend_from_slice(hits.as_slice());
        }
        keys(&out)
    };
    assert_eq!(go(&a, cmaj), [49, 50, 51, 52], "lock off: semitones");
    a.scale_lock = true;
    assert_eq!(go(&a, cmaj), [49, 51, 53, 54], "C D E F");
    let amin = crate::chords::Key::new(9, crate::chords::Scale::MinorPentatonic).mask();
    assert_eq!(go(&a, amin), [49, 51, 53, 56], "C D E G");
    assert_eq!(go(&a, 0), [49, 50, 51, 52], "no song key: semitones");
}

#[test]
fn retrigger_restarts_the_pattern() {
    let mut a = Arp::default();
    for (k, s) in a.steps.iter_mut().enumerate() {
        s.vel = if k == 0 { 1.0 } else { 0.5 };
    }
    // a note from step 3 on
    let late = [(720.0, 3000.0, 49)];
    let free = run(&a, false, &late, 0.0, 1500.0);
    assert_eq!(free[0].start, 720.0);
    assert!((free[0].vel - 0.5).abs() < 1e-6, "free: song step 3");
    a.retrig = true;
    let re = run(&a, false, &late, 0.0, 1500.0);
    assert_eq!(re[0].start, 720.0);
    assert!((re[0].vel - 1.0).abs() < 1e-6, "retrigger: step 1");
    // a new note later restarts the run and the pattern
    let two = [(0.0, 3840.0, 49), (0.0, 3840.0, 53), (960.0, 3840.0, 56)];
    let h = run(&a, false, &two, 0.0, 960.0 + 239.0);
    let at = h.iter().find(|h| h.start == 960.0).unwrap();
    assert!((at.vel - 1.0).abs() < 1e-6);
    assert_eq!(at.key, 49, "the run starts again from the bottom");
}

#[test]
fn mod_params_round_trip() {
    let a = Arp::default();
    for p in Param::ALL {
        let mut m = Mods::default();
        p.apply(&a, p.norm(&a), &mut m);
        assert_eq!(a.eff(&m), a.eff(&Mods::default()), "{p:?} at its own value");
        assert!(!p.name().is_empty() && !p.show(0.5).is_empty());
    }
    let mut m = Mods::default();
    Param::Rate.apply(&a, 1.0, &mut m);
    assert!((a.eff(&m).0 - 30.0).abs() < 1e-6, "rate to the fastest");
    let mut m = Mods::default();
    Param::Octaves.apply(&a, 1.0, &mut m);
    assert_eq!(a.eff(&m).3, 4);
}

#[test]
fn named_presets_exist() {
    for n in [
        "Trance Gate 16ths",
        "Chip Fast Chord",
        "Synthwave Octaves",
        "Euclid 5/8 Pluck",
        "Converge Glass",
        "In-key Runs",
        "Probability Sparkle",
    ] {
        let a = preset(n);
        assert!(!run(&a, false, &CEG, 0.0, 3000.0).is_empty(), "{n}");
    }
}

/// Length 32 plays all 32 steps: a rest on step 21 is heard at 32 (and
/// skipped at 16, where the pattern wraps after step 16), and the pattern
/// starts over on step 33; it survives a save as JSON
#[test]
fn length_32_plays_and_saves_all_32_steps() {
    let notes = [(0.0, 240.0 * 40.0, 49)];
    let mut a = Arp::default();
    a.steps.resize(MAX_STEPS, Step::default());
    a.steps[20].kind = StepKind::Rest;
    a.steps[28].transpose = 12;
    a.len = 32;
    let h = run(&a, false, &notes, 0.0, 240.0 * 34.0 - 1.0);
    let starts: Vec<i64> = h.iter().map(|h| (h.start / 240.0).round() as i64).collect();
    assert!(!starts.contains(&20), "step 21 rests: {starts:?}");
    assert!(starts.contains(&32) && starts.contains(&33), "{starts:?}");
    let at = |k: i64| {
        h.iter()
            .find(|h| (h.start / 240.0).round() as i64 == k)
            .map(|h| h.key)
    };
    assert_eq!(at(28), Some(49 + 12), "step 29 transposed");
    assert_eq!(at(32), Some(49), "step 33 is step 1 again");
    assert_eq!(h.len(), 33);
    // at Length 16 steps 17-32 are not used: no rest at step 21
    a.len = 16;
    let h = run(&a, false, &notes, 0.0, 240.0 * 34.0 - 1.0);
    assert_eq!(h.len(), 34);
    // the JSON keeps the length and the steps past 16
    a.len = 32;
    let back: Arp = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    assert_eq!(back.len, 32);
    assert_eq!(back.steps.len(), 32);
    assert_eq!(back.steps[20].kind, StepKind::Rest);
    assert_eq!(back.steps[28].transpose, 12);
}
