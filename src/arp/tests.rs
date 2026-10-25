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
