use super::*;
use crate::gen::{GenEngine, GenPatch, NO_KNOBS};

const SR: f32 = 48000.0;

/// one note on a fresh voice, sample by sample (bits), with optional knob
/// automation and 43xy macros
fn note(
    g: &GenPatch,
    key: u8,
    ko: &[f32; crate::gen::GEN_KNOBS],
    gmo: [f32; 8],
) -> Vec<(u32, u32)> {
    let freq = crate::model::note_freq(key);
    let mut v = crate::synth::SynthVoice::seeded(key as u32 * 77 + 5);
    v.gmo = gmo;
    (0..(SR * 0.5) as usize)
        .map(|n| {
            if n == (SR * 0.25) as usize {
                v.amp.release();
            }
            let (l, r) = v.render_gen_ko(&g.base, g, ko, freq, SR, 120.0);
            (l.to_bits(), r.to_bits())
        })
        .collect()
}

fn all_kits() -> Vec<(&'static str, GenPatch)> {
    crate::drumpat::KIT_NAMES
        .iter()
        .map(|(n, _)| (*n, crate::drumpat::kit_patch(n).unwrap()))
        .collect()
}

#[test]
fn a_converted_kit_renders_bit_identically_on_every_key() {
    let mut ko = NO_KNOBS;
    for (k, v) in ko.iter_mut().enumerate() {
        if k % 5 == 2 {
            *v = (k % 9) as f32 / 9.0;
        }
    }
    let gmo = [-1.0, 0.8, -1.0, 0.1, -1.0, -1.0, 0.5, -1.0];
    for (name, kit) in all_kits() {
        let dk = patch_from_kit(&kit, true).unwrap();
        assert!(matches!(dk.engine, GenEngine::Drums(_)));
        for key in 1u8..=96 {
            for (ko, gmo) in [(&NO_KNOBS, [-1.0; 8]), (&ko, [-1.0; 8]), (&NO_KNOBS, gmo)] {
                assert!(
                    note(&kit, key, ko, gmo) == note(&dk, key, ko, gmo),
                    "{name}: key {key} differs"
                );
            }
        }
    }
}

#[test]
fn the_16_pad_kit_plays_every_library_note_as_the_kit_did() {
    for (name, kit) in all_kits() {
        let dk = patch_from_kit(&kit, false).unwrap();
        let GenEngine::Drums(k) = &dk.engine else {
            panic!()
        };
        assert_eq!(k.pads.len(), DEFAULT_PADS, "{name}");
        for (_, key, lane) in crate::drumpat::LANES {
            assert!(
                note(&kit, key, &NO_KNOBS, [-1.0; 8]) == note(&dk, key, &NO_KNOBS, [-1.0; 8]),
                "{name}: {lane} ({key}) differs"
            );
        }
        // every pad has its own key, and the hats choke each other
        let mut keys: Vec<u8> = k.pads.iter().map(|p| p.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), k.pads.len(), "{name}: duplicate keys");
        let hats: Vec<&str> = k
            .pads
            .iter()
            .filter(|p| p.choke == 1)
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(hats, ["Closed hat", "Pedal hat", "Open hat"], "{name}");
        assert!(k.pads.iter().all(|p| p.choke == 0 || p.choke == 1));
    }
}

#[test]
fn notes_find_their_pad() {
    let k = DrumKitP::default();
    let pad = |key: i32| {
        k.pad_for_key(key)
            .map(|(i, o)| (k.pads[i].name.as_str(), o))
    };
    assert_eq!(pad(49), Some(("Kick", 0.0)));
    assert_eq!(pad(61), Some(("Crash", 0.0)));
    // another octave: the same note name, nearest pad
    assert_eq!(pad(37), Some(("Kick", 0.0)));
    assert_eq!(pad(73), Some(("Crash", 0.0)));
    assert_eq!(pad(75), Some(("Rim", 0.0)));
    // the old Kit map on a converted kit
    let c = from_kit(&KitP::default());
    let cp = |key: i32| {
        c.pad_for_key(key)
            .map(|(i, o)| (c.pads[i].name.as_str(), o))
    };
    assert_eq!(cp(53), Some(("Tom", -7.0)));
    assert_eq!(cp(54), Some(("Tom", -4.0)));
    assert_eq!(cp(58), Some(("Tom", 4.0)));
    assert_eq!(cp(57), Some(("Closed hat", 0.0)));
    assert_eq!(cp(61), Some(("Kick", 0.0)));
    assert_eq!(key_name(49), "C-4");
    assert_eq!(key_name(55), "F#4");
    assert_eq!(freq_key(crate::model::note_freq(70)), 70);
}

#[test]
fn mute_solo_tune_pan_decay_and_attack_shape_the_pad() {
    let base = GenPatch {
        engine: GenEngine::Drums(DrumKitP::default()),
        ..Default::default()
    };
    let with = |f: &dyn Fn(&mut Pad)| {
        let mut g = base.clone();
        if let GenEngine::Drums(k) = &mut g.engine {
            f(&mut k.pads[1]); // Snare
        }
        g
    };
    let render = |g: &GenPatch| -> Vec<(f32, f32)> {
        let freq = crate::model::note_freq(51);
        let mut v = crate::synth::SynthVoice::seeded(9);
        (0..(SR * 0.4) as usize)
            .map(|_| v.render_gen(&g.base, g, freq, SR, 120.0))
            .collect()
    };
    let energy = |o: &[(f32, f32)]| o.iter().map(|(l, r)| l * l + r * r).sum::<f32>();
    let plain = render(&base);
    assert!(energy(&plain) > 1.0);
    assert_eq!(energy(&render(&with(&|p| p.mute = true))), 0.0, "muted");
    // another pad's solo silences this one; its own solo doesn't
    let mut solo = base.clone();
    if let GenEngine::Drums(k) = &mut solo.engine {
        k.pads[0].solo = true;
    }
    assert_eq!(energy(&render(&solo)), 0.0, "soloed elsewhere");
    assert_eq!(render(&with(&|p| p.solo = true)), plain, "own solo");
    // pan right: the left side quieter
    let r = render(&with(&|p| p.pan = 0.8));
    let (le, re): (f32, f32) = r
        .iter()
        .fold((0.0, 0.0), |a, (l, r)| (a.0 + l * l, a.1 + r * r));
    assert!(le < re * 0.1, "{le} {re}");
    // decay: longer tail
    let tail = |o: &[(f32, f32)]| energy(&o[(SR * 0.2) as usize..]);
    assert!(tail(&render(&with(&|p| p.decay = 3.0))) > 2.0 * tail(&plain));
    // attack: a softer start
    let head = |o: &[(f32, f32)]| energy(&o[..(SR * 0.005) as usize]);
    assert!(head(&render(&with(&|p| p.attack = 0.02))) < 0.5 * head(&plain));
    // tune: a different sound
    assert_ne!(render(&with(&|p| p.tune = 5.0)), plain);
}

#[test]
fn every_kind_has_a_decay_macro() {
    for k in DrumKind::ALL {
        assert!(decay_macro(k).is_some(), "{k:?}");
    }
}

#[test]
fn pads_add_up_to_32_and_fix_clamps() {
    let mut k = DrumKitP::default();
    while k.add_pad(Some(0)).is_some() {}
    assert_eq!(k.pads.len(), MAX_PADS);
    let mut keys: Vec<u8> = k.pads.iter().map(|p| p.key).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), MAX_PADS, "each added pad gets a free key");
    k.pads.push(Pad::default());
    k.pads[0].pan = f32::NAN;
    k.pads[0].decay = 99.0;
    k.pads[0].choke = 40;
    k.fix();
    assert_eq!(k.pads.len(), MAX_PADS);
    assert_eq!(
        (k.pads[0].pan, k.pads[0].decay, k.pads[0].choke),
        (0.0, 4.0, 8)
    );
    let mut e = DrumKitP {
        pads: vec![],
        ..Default::default()
    };
    e.fix();
    assert_eq!(e.pads.len(), 1);
}

#[test]
fn velocity_depth() {
    let mut k = DrumKitP::default();
    assert_eq!(k.vel_gain(0, 0.5), 0.5, "default = linear");
    k.pads[0].vel = 0.0;
    assert_eq!(k.vel_gain(0, 0.2), 1.0);
    k.pads[0].vel = 0.5;
    assert!((k.vel_gain(0, 0.0) - 0.5).abs() < 1e-6);
}

#[test]
fn presets_cover_every_library_kit_and_finish() {
    let p = presets();
    assert_eq!(p.len(), crate::drumpat::KIT_NAMES.len());
    for (name, _, g) in &p {
        let GenEngine::Drums(k) = &g.engine else {
            panic!("{name}")
        };
        for pad in &k.pads {
            let freq = crate::model::note_freq(pad.key);
            let mut v = crate::synth::SynthVoice::seeded(3);
            let mut n = 0;
            let mut pk = 0.0f32;
            while !v.gen.done && n < (SR * 8.0) as usize {
                let (l, r) = v.render_gen(&g.base, g, freq, SR, 120.0);
                pk = pk.max(l.abs()).max(r.abs());
                n += 1;
            }
            assert!(v.gen.done, "{name} {} never ends", pad.name);
            assert!((0.05..=1.5).contains(&pk), "{name} {}: peak {pk}", pad.name);
        }
    }
    assert!(preset("psy kit").is_some());
}

#[test]
fn a_drum_kit_song_and_cwi_round_trip_and_need_reader_5() {
    use crate::model::{Instrument, InstrumentKind};
    let dir = std::env::temp_dir().join(format!("cw_drumkit_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = |p: &std::path::Path| {
        let mut z = zip::ZipArchive::new(std::fs::File::open(p).unwrap()).unwrap();
        let mut f = z.by_name("manifest.toml").unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut f, &mut s).unwrap();
        s
    };
    let mut song = crate::fileio::demo_song();
    let plain = dir.join("plain.cwt");
    crate::fileio::save_cwt(&song, &plain).unwrap();
    assert!(
        manifest(&plain).contains("min_reader_version = 1"),
        "no kit: as before"
    );
    let mut g = preset("909 Kit").unwrap();
    if let GenEngine::Drums(k) = &mut g.engine {
        k.pads[2].pan = -0.4;
        k.pads[3].mute = true;
        k.add_pad(Some(0));
    }
    let inst = Instrument {
        arp: None,
        name: "909 Kit".into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Gen(g),
    };
    song.instruments.push(inst.clone());
    let p = dir.join("kit.cwt");
    crate::fileio::save_cwt(&song, &p).unwrap();
    assert!(manifest(&p).contains("min_reader_version = 5"));
    let back = crate::fileio::load_cwt(&p).unwrap();
    assert_eq!(back.instruments.last(), Some(&inst));
    // .cwi
    let ip = dir.join("kit.cwi");
    crate::instfile::save_cwi(&inst, &ip).unwrap();
    assert_eq!(crate::instfile::load_cwi(&ip).unwrap(), inst);
    // the text form: patch = "drumkit:<kit>"
    let t = "[song]\ntitle = \"t\"\n[[instrument]]\nname = \"Kit\"\npatch = \"drumkit:Psy Kit\"\n";
    let s = crate::fileio::song_from_text(t).unwrap();
    let InstrumentKind::Gen(g) = &s.instruments[0].kind else {
        panic!()
    };
    assert!(matches!(&g.engine, GenEngine::Drums(k) if k.pads.len() == DEFAULT_PADS));
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------ step model

#[test]
fn steps_round_trip_with_swing() {
    for sw in [0.0f32, 0.1, 0.25, 0.5] {
        for k in 0..64 {
            assert_eq!(step_of(step_tick(k, sw)), k, "step {k} swing {sw}");
        }
    }
    assert_eq!(swing_percent(0.0), 50.0);
    assert_eq!(swing_percent(0.5), 75.0);
    assert_eq!(step_tick(1, 0.5), ROW + ROW / 2);
    assert_eq!(step_tick(2, 0.5), 2 * ROW);
}

#[test]
fn toggle_shift_and_swing_edit_the_right_notes() {
    let mut n = vec![];
    assert!(toggle(&mut n, 49, 0, true, 0.0, 30000, 0));
    assert!(toggle(&mut n, 55, 1, true, 0.0, 30000, 0));
    assert!(toggle(&mut n, 55, 1, true, 0.0, 30000, 0));
    assert_eq!(n.len(), 2, "a second on is a no-op");
    assert_eq!(hit_at(&n, 55, 1), Some(1));
    set_swing(&mut n, 0.25);
    assert_eq!(n[1].start, ROW + ROW / 4, "odd step moves");
    assert_eq!(n[0].start, 0, "even step stays");
    set_swing(&mut n, 0.0);
    assert_eq!(n[1].start, ROW);
    shift(&mut n, 55, -2, 16, 0.0);
    assert_eq!(
        step_of(n.iter().find(|x| x.key == 55).unwrap().start),
        15,
        "wraps"
    );
    shift(&mut n, 55, 1, 16, 0.0);
    assert_eq!(step_of(n.iter().find(|x| x.key == 55).unwrap().start), 0);
    assert!(!toggle(&mut n, 49, 0, false, 0.0, 0, 0));
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].key, 55);
}

#[test]
fn generate_is_reproducible_and_plays_on_library_pads() {
    let kit = from_library_kit(
        match &crate::drumpat::kit_patch("808 Kit").unwrap().engine {
            GenEngine::Kit(k) => k,
            _ => unreachable!(),
        },
    );
    for style in 0..crate::drumpat::STYLES.len() {
        for seed in [1u32, 2, 77] {
            for (e, fill) in [(0.0f32, 0u8), (0.5, 1), (1.0, 2)] {
                let a = generate(style, seed, e, fill, 3);
                let b = generate(style, seed, e, fill, 3);
                assert_eq!(a, b, "style {style} seed {seed} reproducible");
                let (notes, steps, swing, _) = a;
                assert!((1..=64).contains(&steps));
                assert!((0.0..=0.5).contains(&swing));
                assert!(
                    !notes.is_empty(),
                    "style {style} seed {seed} energy {e}: empty"
                );
                for n in &notes {
                    assert!(step_of(n.start) < steps);
                    assert_eq!(n.instr, 3);
                    assert!(
                        kit.pads.iter().any(|p| p.key == n.key),
                        "key {} has no pad (style {style})",
                        n.key
                    );
                }
            }
        }
    }
    // energy changes the density
    let lo: usize = (1..20).map(|s| generate(0, s, 0.0, 0, 0).0.len()).sum();
    let hi: usize = (1..20).map(|s| generate(0, s, 1.0, 0, 0).0.len()).sum();
    assert!(hi > lo, "energy: {lo} vs {hi}");
}

#[test]
fn drum_meta_saves_only_when_set() {
    let mut d = crate::timeline::NoteClipData::default();
    let a = toml::to_string(&d).unwrap();
    assert!(!a.contains("drum"), "{a}");
    d.drum = Some(DrumMeta { swing: 0.25 });
    let b = toml::to_string(&d).unwrap();
    let back: crate::timeline::NoteClipData = toml::from_str(&b).unwrap();
    assert_eq!(back.drum, Some(DrumMeta { swing: 0.25 }));
}
