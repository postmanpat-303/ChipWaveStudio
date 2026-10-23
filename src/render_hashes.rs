//! Render fingerprints of every demo and old-song fixture (tracker form and
//! converted to timeline mode). Not an expected-audio test: run it on two
//! commits and diff the output to show UI work left the sound alone.
//!   cargo test --bin chipwave render_hashes -- --ignored --nocapture
use std::hash::{Hash, Hasher};

fn fingerprint(s: &crate::model::Song) -> (u64, usize) {
    let v = crate::engine::render_song(s, 11025);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for x in &v {
        x.to_bits().hash(&mut h);
    }
    (h.finish(), v.len())
}

#[test]
#[ignore]
fn render_hashes() {
    let mut songs: Vec<(String, crate::model::Song)> = vec![];
    for (i, d) in crate::fileio::DEMOS.iter().enumerate() {
        songs.push((format!("demo {}", d.0), crate::fileio::demo_song_n(i)));
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for f in ["chipwave-0.2.3.5-song.cwt", "chipwave-0.2.3.6-song.cwt"] {
        songs.push((
            f.to_string(),
            crate::fileio::load_cwt(&dir.join(f)).expect(f),
        ));
    }
    // StringWave (new in file version 8: not in the older lists)
    songs.push((
        "stringwave Velvet Ensemble".to_string(),
        crate::engine::strings_tests::sw_song(),
    ));
    for (name, s) in songs {
        let a = fingerprint(&s);
        let mut t = s.clone();
        crate::timeline::enter_timeline_mode(&mut t);
        let b = fingerprint(&t);
        println!(
            "RENDER {name}: tracker {:016x} ({}) timeline {:016x} ({})",
            a.0, a.1, b.0, b.1
        );
    }
}
