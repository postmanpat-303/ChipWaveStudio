use super::*;
use crate::fileio::{demo_song_n, DEMOS};

fn songs() -> Vec<Song> {
    (0..DEMOS.len()).map(demo_song_n).collect()
}

#[test]
fn ids_are_unique_and_read_back() {
    for (n, song) in songs().iter().enumerate() {
        let all = all(song);
        assert!(all.len() > 50, "demo {n}: only {} knobs", all.len());
        let mut seen = std::collections::HashSet::new();
        for k in &all {
            let id = k.target.id();
            assert!(seen.insert(id.clone()), "demo {n}: two knobs are {id}");
            assert_eq!(resolve(song, &id), Some(k.target), "demo {n}: {id}");
            assert_eq!(get(song, k.target).as_ref(), Some(k), "demo {n}: {id}");
            assert!(!k.name.is_empty(), "demo {n}: {id} has no name");
            assert!(k.lo < k.hi, "demo {n}: {id} range {}..{}", k.lo, k.hi);
            assert_eq!(k.target.bounds(), (k.lo, k.hi, k.log), "{id}");
            // 0..1 -> real -> 0..1
            for x in [0.0, 0.25, 0.5, 1.0] {
                let back = k.target.to_norm(k.target.to_real(x));
                assert!((back - x).abs() < 1e-3, "{id}: {x} -> {back}");
            }
        }
        // every targets() entry is a knob
        assert_eq!(targets(song).len(), all.len(), "demo {n}");
    }
}

#[test]
fn every_saved_lane_and_route_resolves() {
    for (n, song) in songs().iter().enumerate() {
        let known: Vec<Target> = targets(song);
        for l in &song.automation {
            let t = resolve(song, &l.target);
            assert!(t.is_some(), "demo {n}: lane {} has no knob", l.target);
            assert!(known.contains(&t.unwrap()), "demo {n}: {}", l.target);
        }
        for r in &song.modulation.routes {
            let t = resolve(song, &r.target);
            assert!(t.is_some(), "demo {n}: route onto {} has no knob", r.target);
            assert!(known.contains(&t.unwrap()), "demo {n}: {}", r.target);
        }
    }
}

#[test]
fn set_norm_writes_what_the_song_reads() {
    for (n, song) in songs().iter().enumerate() {
        let mut s = song.clone();
        for k in all(song) {
            let t = k.target;
            // Level: a CC sets the instrument volume (0..1), while a Level
            // lane is a 0..1.5 gain on top of it, so the song value isn't the
            // lane base; Tempo is rounded to whole BPM
            if matches!(t, Target::Level(_) | Target::Tempo) || k.plugin {
                continue;
            }
            assert!(
                set_norm(&mut s, t, 0.3),
                "demo {n}: {} not set",
                k.target.id()
            );
            let got = crate::automation::song_norm(&s, t).expect("readable");
            let tol = if k.choice { 0.2 } else { 0.02 };
            assert!(
                (got - 0.3).abs() <= tol,
                "demo {n}: {} reads {got}",
                k.target.id()
            );
        }
        assert!(set_norm(&mut s, Target::Tempo, 0.5));
        assert_eq!(s.bpm, 170.0);
        assert!(set_norm(&mut s, Target::Level(0), 0.25));
        assert_eq!(s.instruments[0].volume, 0.25);
        // knobs the song doesn't have
        assert!(!set_norm(&mut s, Target::Level(250), 0.5));
        assert!(!set_norm(&mut s, Target::ChanVolume(250), 0.5));
    }
}

#[test]
fn ids_keep_their_old_spelling() {
    let song = demo_song_n(0);
    for (t, id) in [
        (Target::Cutoff(0), "instrument.1.cutoff"),
        (Target::Level(1), "instrument.2.level"),
        (Target::ChanVolume(0), "mixer.ch.1.volume"),
        (Target::ChanPan(2), "mixer.ch.3.pan"),
        (Target::Send(0, 1), "mixer.ch.1.send_b"),
        (Target::Master, "mixer.master"),
        (Target::Tempo, "tempo"),
        (Target::Fx(0, 1, 2), "instrument.1.fx.2.p2"),
        (Target::Gen(2, 3), "instrument.3.gen.4"),
        (Target::Macro(0, 7), "instrument.1.macro.8"),
        (Target::ModAmount(0), "mod.1.amount"),
    ] {
        assert_eq!(t.id(), id);
        assert_eq!(Target::parse(id), Some(t));
    }
    let amp_a = crate::modulation::patch_param("amp.a").unwrap();
    assert_eq!(Target::Patch(0, amp_a).id(), "instrument.1.patch.amp-a");
    assert_eq!(Target::Ret(0, 0).id(), "mixer.return_a.sync");
    // long forms that old files may hold still resolve
    assert_eq!(
        Target::parse("instrument.1.filter1.cutoff"),
        Some(Target::Cutoff(0))
    );
    assert_eq!(Target::parse("mixer.master.volume"), Some(Target::Master));
    let _ = song;
}
