//! Render fingerprints of every generator preset and every Drum Patterns
//! kit, note by note (keys 13..=97, plus an automated-knob and a 43xy pass).
//! Not an expected-audio test: run it on two commits and diff the output.
//!   cargo test --bin chipwave render_hashes_gen -- --ignored --nocapture
use std::hash::{Hash, Hasher};

fn voice_hash(g: &crate::gen::GenPatch, h: &mut impl Hasher) {
    let sr = 24000.0;
    let mut ko = crate::gen::NO_KNOBS;
    for (k, v) in ko.iter_mut().enumerate() {
        if k % 3 == 1 {
            *v = (k % 7) as f32 / 7.0;
        }
    }
    for pass in 0..3 {
        for key in 13u8..=97 {
            let freq = crate::model::note_freq(key);
            let mut v = crate::synth::SynthVoice::seeded(key as u32 * 2_654_435_761 | 1);
            if pass == 2 {
                v.gmo = [0.2, -1.0, 0.7, -1.0, 0.4, -1.0, -1.0, 0.9];
            }
            let ko = if pass == 1 {
                &ko
            } else {
                &crate::gen::NO_KNOBS
            };
            for n in 0..(sr * 0.6) as usize {
                if n == (sr * 0.3) as usize {
                    v.amp.release();
                }
                let (l, r) = v.render_gen_ko(&g.base, g, ko, freq, sr, 120.0);
                l.to_bits().hash(h);
                r.to_bits().hash(h);
            }
        }
    }
}

#[test]
#[ignore]
fn render_hashes_gen() {
    let mut all: Vec<(String, crate::gen::GenPatch)> = crate::gen::presets()
        .into_iter()
        .map(|(n, _, g)| (n.to_string(), g))
        .collect();
    for (n, _) in crate::drumpat::KIT_NAMES {
        all.push((format!("kit {n}"), crate::drumpat::kit_patch(n).unwrap()));
    }
    for (name, g) in all {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        voice_hash(&g, &mut h);
        println!("GEN {name}: {:016x}", h.finish());
    }
}
