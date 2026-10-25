//! Round Robin (docs/ROUND_ROBIN.md): step conditions, fills, the
//! "which one this time" orders, determinism, and files without any of it
//! staying exactly as before.
use super::rr::*;
use super::*;
use crate::model::{Pattern, Song};

fn song() -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.bpm = 125.0;
    s.speed = 6;
    s.channels = 2;
    s.channel_names = vec!["A".into(), "B".into()];
    s.patterns = vec![Pattern::new("A", 16, 2)];
    s.order = vec![0];
    s.mixer.channels.resize(2, Default::default());
    enter_timeline_mode(&mut s);
    s
}

fn tl(s: &mut Song) -> &mut Timeline {
    s.timeline.as_deref_mut().unwrap()
}

/// a looped note clip on track 1: `passes` passes of 4 rows
fn looped(s: &mut Song, notes: Vec<Note>, passes: Tick) -> u32 {
    let id = new_note_clip(s, 1, 0).unwrap();
    let t = tl(s);
    let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
    c.loop_len = Some(4 * ROW);
    c.len = passes * 4 * ROW;
    let ClipKind::Notes { data } = c.kind else {
        unreachable!()
    };
    t.notes[data as usize].make_mut().notes = notes;
    id
}

fn n(start: Tick, key: u8) -> Note {
    Note {
        start,
        len: ROW,
        key,
        vel: 50000,
        ..Default::default()
    }
}

/// the 0-based passes key `key` plays on
fn passes_of(s: &Song, key: u8) -> Vec<Tick> {
    note_spans(s.timeline.as_deref().unwrap(), 2)[1]
        .iter()
        .filter(|x| x.key == key)
        .map(|x| x.start / (4 * ROW))
        .collect()
}

#[test]
fn conditions_pick_their_passes() {
    let seq = |c: Cond, fill: bool| -> Vec<u32> { (0..9).filter(|&p| c.plays(p, fill)).collect() };
    assert_eq!(seq(Cond::Always, false), (0..9).collect::<Vec<_>>());
    assert_eq!(seq(Cond::Every { a: 1, b: 4 }, false), vec![0, 4, 8]);
    assert_eq!(seq(Cond::Every { a: 2, b: 4 }, false), vec![1, 5]);
    assert_eq!(seq(Cond::Every { a: 4, b: 4 }, false), vec![3, 7]);
    assert_eq!(seq(Cond::Every { a: 2, b: 2 }, false), vec![1, 3, 5, 7]);
    // out-of-range values are clamped, never panic
    assert_eq!(seq(Cond::Every { a: 9, b: 3 }, false), vec![2, 5, 8]);
    assert_eq!(
        seq(Cond::Every { a: 0, b: 0 }, false),
        (0..9).collect::<Vec<_>>()
    );
    assert_eq!(seq(Cond::First, false), vec![0]);
    assert_eq!(seq(Cond::NotFirst, false), (1..9).collect::<Vec<_>>());
    assert!(seq(Cond::Fill, false).is_empty());
    assert_eq!(seq(Cond::Fill, true).len(), 9);
    assert_eq!(seq(Cond::NotFill, false).len(), 9);
    assert!(seq(Cond::NotFill, true).is_empty());
}

#[test]
fn clip_passes_follow_the_conditions_and_the_last_pass_is_a_fill() {
    let mut s = song();
    looped(
        &mut s,
        vec![
            n(0, 49),
            Note {
                cond: Cond::Every { a: 2, b: 4 },
                ..n(ROW, 50)
            },
            Note {
                cond: Cond::First,
                ..n(2 * ROW, 51)
            },
            Note {
                cond: Cond::Fill,
                ..n(3 * ROW, 52)
            },
            Note {
                cond: Cond::NotFill,
                ..n(3 * ROW, 53)
            },
        ],
        8,
    );
    assert_eq!(passes_of(&s, 49), (0..8).collect::<Vec<_>>());
    assert_eq!(passes_of(&s, 50), vec![1, 5]);
    assert_eq!(passes_of(&s, 51), vec![0]);
    assert_eq!(passes_of(&s, 52), vec![7], "the last pass is a fill");
    assert_eq!(passes_of(&s, 53), (0..7).collect::<Vec<_>>());
    // a song marker inside pass 3 (the next section starts) makes it a fill
    tl(&mut s).markers.push(Marker {
        name: "Chorus".into(),
        tick: 4 * 4 * ROW,
        ..Default::default()
    });
    assert_eq!(passes_of(&s, 52), vec![3, 7]);
    // a track tag doesn't
    tl(&mut s).markers[0].track = Some(1);
    assert_eq!(passes_of(&s, 52), vec![7]);
    // the live Fill key: every pass, only in the live plan
    let t = s.timeline.as_deref().unwrap();
    let live = note_spans_live(t, 2, true);
    assert_eq!(live[1].iter().filter(|x| x.key == 52).count(), 8);
    assert_eq!(live[1].iter().filter(|x| x.key == 53).count(), 0);
}

#[test]
fn conditions_never_move_the_chance_draws() {
    let notes = vec![
        Note {
            chance: 50,
            ..n(0, 49)
        },
        Note {
            chance: 30,
            ..n(ROW, 50)
        },
        n(2 * ROW, 51),
    ];
    let mut a = song();
    looped(&mut a, notes.clone(), 64);
    let mut b = song();
    let mut nb = notes;
    nb[0].cond = Cond::Every { a: 1, b: 2 };
    nb[2].cond = Cond::NotFirst;
    looped(&mut b, nb, 64);
    let pa = passes_of(&a, 50);
    assert!(!pa.is_empty() && pa.len() < 64);
    assert_eq!(
        pa,
        passes_of(&b, 50),
        "the other note's draws are unchanged"
    );
    // "Sometimes" and a condition combine: plays when both say so
    let both: Vec<Tick> = passes_of(&a, 49)
        .into_iter()
        .filter(|p| p % 2 == 0)
        .collect();
    assert_eq!(passes_of(&b, 49), both);
    assert_eq!(passes_of(&b, 51), (1..64).collect::<Vec<_>>());
}

#[test]
fn unused_conditions_leave_files_and_plans_as_before() {
    let json = serde_json::to_string(&n(0, 49)).unwrap();
    assert!(!json.contains("cond"), "{json}");
    let c = Note {
        cond: Cond::Every { a: 3, b: 4 },
        ..n(0, 49)
    };
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("\"cond\""), "{json}");
    let back: Note = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c);
    for c in [Cond::First, Cond::NotFirst, Cond::Fill, Cond::NotFill] {
        let x = Note {
            cond: c,
            ..n(0, 49)
        };
        let back: Note = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
        assert_eq!(back.cond, c);
    }
    // without conditions the Fill key changes nothing (no plan rebuild)
    let mut s = song();
    looped(&mut s, vec![n(0, 49)], 4);
    let t = s.timeline.as_deref().unwrap();
    assert_eq!(plan_key_live(&s, t, true), plan_key(&s, t));
    assert_eq!(build_plan_live(&s, t, true).notes, build_plan(&s, t).notes);
    tl(&mut s).notes[0].make_mut().notes[0].cond = Cond::Fill;
    let t = s.timeline.as_deref().unwrap();
    assert_ne!(plan_key_live(&s, t, true), plan_key(&s, t));
}

#[test]
fn orders_give_their_sequences() {
    let seq = |o: Order, n: usize| -> Vec<usize> {
        (0..12).map(|k| pick(o, n, 7, SALT_VAR, k)).collect()
    };
    assert_eq!(
        seq(Order::InOrder, 3),
        vec![0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2]
    );
    assert_eq!(
        seq(Order::PingPong, 4),
        vec![0, 1, 2, 3, 2, 1, 0, 1, 2, 3, 2, 1]
    );
    assert_eq!(
        seq(Order::PingPong, 2),
        vec![0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1]
    );
    assert_eq!(
        seq(Order::EveryN { n: 4, which: 2 }, 3),
        vec![0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 2]
    );
    // one item: always it
    for o in Order::ALL {
        assert!(seq(o, 1).iter().all(|&i| i == 0));
    }
    for n in 2..=4 {
        for seed in 0..50u32 {
            let s: Vec<usize> = (0..400)
                .map(|k| pick(Order::RandomNoRepeat, n, seed, SALT_VAR, k))
                .collect();
            assert!(
                s.windows(2).all(|w| w[0] != w[1]),
                "no repeats n={n} seed={seed}"
            );
            // every bag of n holds each one once
            if n > 2 {
                for b in s.chunks_exact(n) {
                    let mut x = b.to_vec();
                    x.sort();
                    assert_eq!(x, (0..n).collect::<Vec<_>>());
                }
            }
            let r: Vec<usize> = (0..400)
                .map(|k| pick(Order::Random, n, seed, SALT_VAR, k))
                .collect();
            assert!(r.iter().all(|&i| i < n));
            for i in 0..n {
                assert!(r.contains(&i), "random reaches every item");
            }
        }
    }
    // pure: asking for one index alone gives the same answer (seeking)
    for k in [0u32, 5, 99, 1000] {
        let walk: Vec<usize> = (0..=k)
            .map(|j| pick(Order::RandomNoRepeat, 4, 3, SALT_HIT, j))
            .collect();
        assert_eq!(
            walk[k as usize],
            pick(Order::RandomNoRepeat, 4, 3, SALT_HIT, k)
        );
    }
    // different seeds and salts give different shuffles
    let a: Vec<usize> = (0..32)
        .map(|k| pick(Order::Random, 4, 1, SALT_VAR, k))
        .collect();
    let b: Vec<usize> = (0..32)
        .map(|k| pick(Order::Random, 4, 2, SALT_VAR, k))
        .collect();
    let c: Vec<usize> = (0..32)
        .map(|k| pick(Order::Random, 4, 1, SALT_HIT, k))
        .collect();
    assert_ne!(a, b);
    assert_ne!(a, c);
    // unit draws are in 0..1 and repeatable
    for k in 0..100 {
        let u = unit(5, SALT_DRIFT, k, 2);
        assert!((0.0..1.0).contains(&u));
        assert_eq!(u.to_bits(), unit(5, SALT_DRIFT, k, 2).to_bits());
    }
}
