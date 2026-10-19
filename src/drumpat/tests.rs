use super::*;

#[test]
fn library_has_60_plus_patterns_in_every_style() {
    assert!(PATTERNS.len() >= 60, "{} patterns", PATTERNS.len());
    for (s, name) in STYLES.iter().enumerate() {
        let n = in_style(s).count();
        assert!(n >= 4, "{name}: {n} patterns");
    }
}

#[test]
fn lanes_are_whole_bars_and_kits_exist() {
    let mut keys = std::collections::HashSet::new();
    for p in PATTERNS {
        assert!(keys.insert(p.key()), "duplicate {}", p.name);
        assert!(
            kit_patch(p.kit).is_some(),
            "{}: unknown kit {}",
            p.name,
            p.kit
        );
        assert!(p.bpm.0 <= p.bpm.1 && p.bpm.0 >= 60);
        assert!(!p.lanes.is_empty());
        for (c, l) in p.lanes {
            assert!(lane_note(*c).is_some(), "{}: lane {c}", p.name);
            let n = l.chars().filter(|c| *c != '|').count();
            assert!(
                n % 16 == 0 && p.steps() % n == 0,
                "{} lane {c}: {n} steps",
                p.name
            );
            assert!(
                l.chars().all(|c| "xXo.|".contains(c)),
                "{} lane {c}",
                p.name
            );
        }
        assert!(p.hits() > 0);
        assert!(p.events().iter().all(|e| e.0 >= 0 && e.0 < p.len()));
    }
}

#[test]
fn every_kit_is_a_kit_mode_generator() {
    for (k, _) in KIT_NAMES {
        let i = kit_instrument(k).unwrap();
        assert_eq!(i.name, k);
        match i.kind {
            InstrumentKind::Gen(g) => assert!(matches!(g.engine, GenEngine::Kit(_))),
            _ => panic!("{k} isn't a generator"),
        }
    }
}

#[test]
fn notes_are_ticks_with_swing_on_odd_sixteenths() {
    let p = PATTERNS.iter().find(|p| p.name == "Deep Shuffle").unwrap();
    let n = p.notes(3);
    assert!(n.iter().all(|x| x.instr == 3 && x.len == ROW));
    // closed hats on every 16th: the odd ones are late
    let hats: Vec<Tick> = n.iter().filter(|x| x.key == 55).map(|x| x.start).collect();
    assert_eq!(hats[0], 0);
    assert_eq!(hats[1], ROW + (0.18 * ROW as f32).round() as Tick);
    // the kick is on every beat
    let kicks: Vec<Tick> = n.iter().filter(|x| x.key == 49).map(|x| x.start).collect();
    assert_eq!(kicks, vec![0, 4 * ROW, 8 * ROW, 12 * ROW]);
}

#[test]
fn psy_hats_sit_on_the_offbeat() {
    for (_, p) in in_style(0) {
        for (c, l) in p.lanes {
            if *c == 'O' {
                let v = p.lane_steps(l);
                for (k, x) in v.iter().enumerate() {
                    if *x > 0 {
                        assert_eq!(k % 4, 2, "{} open hat on step {k}", p.name);
                    }
                }
            }
        }
    }
}

#[test]
fn thumbnail_masks_first_bar() {
    let p = PATTERNS.iter().find(|p| p.name == "Full-on Drive").unwrap();
    let t = p.thumb();
    let k = t.iter().find(|x| x.0 == 'K').unwrap().1;
    assert_eq!(k, 1 | 1 << 4 | 1 << 8 | 1 << 12);
}
