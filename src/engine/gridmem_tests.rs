//! Grid Delay lines are made on first use and kept by the voice that made
//! them: songs whose instruments are Grid patches with Delay modules render
//! the same audio, and allocate nothing once every voice has played.
use super::*;
use std::hash::{Hash, Hasher};

/// every demo with its instruments swapped for the Grid Delay presets
fn grid_songs() -> Vec<Song> {
    let pick = |name: &str| {
        crate::gen::presets()
            .into_iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, _, g)| g)
            .unwrap_or_else(|| panic!("preset {name}"))
    };
    let pad = pick("Grid Wavetable Pad");
    let pluck = pick("Grid Karplus Pluck");
    (0..crate::fileio::DEMOS.len())
        .map(|d| {
            let mut s = crate::fileio::demo_song_n(d);
            for (i, inst) in s.instruments.iter_mut().enumerate() {
                let g = if (i + d) % 2 == 0 { &pluck } else { &pad };
                inst.kind = InstrumentKind::Gen(g.clone());
            }
            s
        })
        .collect()
}

fn play(song: &Song, blocks: usize, e: &mut Engine, h: &mut impl Hasher) {
    // on the stack: the allocation test counts only the engine
    let mut buf = [0.0f32; 512];
    for _ in 0..blocks {
        e.render(song, &mut buf);
        for x in &buf {
            x.to_bits().hash(h);
        }
    }
}

/// hash of 12 s of every Grid Delay song (from 0 and from a later row)
fn grid_render_hash() -> u64 {
    crate::wavetable::init();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for song in grid_songs() {
        let mut e = Engine::new(48000.0);
        crate::strings::rack::prepare(&mut e.strings, &song, 48000.0);
        e.start(&song, 0, 0, false, 0);
        play(&song, 1125, &mut e, &mut h);
        e.start(&song, 0, 16, false, 0);
        play(&song, 375, &mut e, &mut h);
    }
    h.finish()
}

#[test]
fn grid_delay_songs_render_unchanged() {
    // recorded on 66bc633 (Grid Delay lines inline in every voice)
    let got = grid_render_hash();
    println!("GRIDHASH {got:016x}");
    assert_eq!(got, GRID_HASH, "Grid Delay render changed: {got:016x}");
}

const GRID_HASH: u64 = 0x2418_16e2_f40d_0f95;

#[test]
fn grid_delay_songs_play_allocation_free_after_warmup() {
    crate::wavetable::init();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (d, song) in grid_songs().iter().enumerate() {
        let mut e = Engine::new(48000.0);
        crate::strings::rack::prepare(&mut e.strings, song, 48000.0);
        // warm-up: every voice that plays makes its Delay lines once (two
        // passes, as voice rotation can reach a new voice on the second)
        for _ in 0..2 {
            e.start(song, 0, 0, false, 0);
            play(song, 750, &mut e, &mut h);
        }
        e.start(song, 0, 0, false, 0);
        let n = alloc_count::count(|| play(song, 750, &mut e, &mut h));
        assert_eq!(n, 0, "demo {d}: {n} allocations after warm-up");
    }
}

#[test]
fn voices_carry_no_delay_lines_until_a_grid_delay_plays() {
    // inline, the two lines were 32 KB of every voice (256 note voices,
    // 64 channel voices and the live ones in every engine)
    assert!(std::mem::size_of::<SynthVoice>() < 24 * 1024);
    let e = Engine::new(48000.0);
    assert!(e.chans.iter().all(|c| !c.synth.gen.grid_lines_made()));
    let song = &grid_songs()[0];
    let mut e = Engine::new(48000.0);
    e.start(song, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 512];
    for _ in 0..375 {
        e.render(song, &mut buf);
    }
    assert!(
        e.chans
            .iter()
            .any(|c| c.synth.gen.grid_lines_made() || c.tail.synth.gen.grid_lines_made()),
        "a Grid Delay that played made its lines"
    );
}
