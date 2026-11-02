//! Memory soak measurements (run alone: `cargo test --bin chipwave memsoak -- --ignored
//! --nocapture --test-threads=1`). Heap numbers come from the test allocator.
use super::ui_tests::{frame, harness_with};
use super::*;
use crate::engine::alloc_count;

fn rss_kb() -> usize {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<usize>().ok())
        })
        .map_or(0, |p| p * 4)
}

fn mb(b: isize) -> f64 {
    b as f64 / 1048576.0
}

#[test]
#[ignore]
fn memsoak_engine_demos() {
    for (i, (name, _, _)) in crate::fileio::DEMOS.iter().enumerate() {
        let s = crate::fileio::demo_song_n(i);
        let mut e = crate::engine::Engine::new(48000.0);
        e.start(&s, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 512];
        for _ in 0..375 {
            e.render(&s, &mut buf);
        }
        let l0 = alloc_count::live();
        let n = alloc_count::count(|| {
            for _ in 0..(30 * 48000 / 256) {
                e.render(&s, &mut buf);
            }
        });
        let l1 = alloc_count::live();
        // play / stop loop
        let n2 = alloc_count::count(|| {
            for _ in 0..100 {
                e.stop();
                e.start(&s, 0, 0, false, 0);
                for _ in 0..20 {
                    e.render(&s, &mut buf);
                }
            }
        });
        let l2 = alloc_count::live();
        e.stop();
        e.start(&s, 0, 0, false, 0);
        let n3 = alloc_count::count(|| {
            for _ in 0..(32 * 48000 / 256) {
                e.render(&s, &mut buf);
            }
        });
        println!(
            "ENG {name:24} second pass allocs {n3} live {:+.3} MB",
            mb(alloc_count::live() - l2)
        );
        println!(
            "ENG {name:24} allocs/30s {n:6} live d {:+.3} MB | play/stop x100 allocs {n2:6} live d {:+.3} MB",
            mb(l1 - l0),
            mb(l2 - l1)
        );
    }
}

fn render(h: &mut egui_kittest::Harness<'static, App>, chunks: usize) {
    let shared = h.state().shared.clone();
    let mut buf = vec![0.0f32; 512];
    for _ in 0..chunks {
        crate::audio::render_chunk(&shared, &mut buf, 48000.0);
    }
}

#[test]
#[ignore]
fn memsoak_app() {
    let rounds: usize = std::env::var("SOAK_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);
    let base = alloc_count::live();
    let rss0 = rss_kb();
    let mut h = harness_with(crate::fileio::demo_song());
    println!(
        "MEM start live {:.1} MB rss {} MB",
        mb(alloc_count::live() - base),
        rss_kb() / 1024
    );
    let views = [
        View::Pattern,
        View::Arranger,
        View::Timeline,
        View::Mixer,
        View::Fx,
        View::Sample,
        View::Settings,
    ];
    for r in 0..rounds {
        alloc_count::reset_peak();
        let t = std::time::Instant::now();
        for d in 0..crate::fileio::DEMOS.len() {
            h.state_mut().load_demo(d);
            frame(&mut h);
            // play / stop
            h.state_mut().play(0);
            for _ in 0..10 {
                render(&mut h, 8);
                frame(&mut h);
            }
            h.state_mut().play(0);
            frame(&mut h);
            // every view
            for v in views {
                h.state_mut().view = v;
                frame(&mut h);
                frame(&mut h);
            }
            h.state_mut().view = View::Timeline;
            // open every instrument in the lower panel
            let n = h.state().with(|s| s.song.instruments.len()).min(12);
            for k in 1..=n {
                h.state_mut().instr = k as u8;
                frame(&mut h);
            }
            // 30 edits, undo all, redo all
            for e in 0..30usize {
                h.state_mut().with(|s| {
                    if let Some(p) = s.song.patterns.first_mut() {
                        let rows = p.cells.len();
                        if rows > 0 {
                            p.cells[e % rows][0].note = 30 + (e % 40) as u8;
                        }
                    }
                    s.song.bpm += 0.5;
                });
                h.state_mut().snapshot();
                frame(&mut h);
            }
            for _ in 0..30 {
                h.state_mut().undo(false);
                frame(&mut h);
            }
            for _ in 0..30 {
                h.state_mut().undo(true);
                frame(&mut h);
            }
        }
        println!(
            "MEM round {r} live {:.1} MB peak {:.1} MB rss {} MB undo {} steps {:.1} MB ({:.1}s)",
            mb(alloc_count::live() - base),
            mb(alloc_count::peak() - base),
            rss_kb() / 1024,
            h.state().hist.len(),
            h.state().hist.bytes() as f64 / 1048576.0,
            t.elapsed().as_secs_f32()
        );
    }
    let _ = rss0;
}

#[test]
#[ignore]
fn memsoak_sizes() {
    let l = alloc_count::live;
    let a = l();
    let e = crate::engine::Engine::new(48000.0);
    println!(
        "SIZE Engine::new(48k) heap {:.2} MB, inline {} B",
        mb(l() - a),
        std::mem::size_of_val(&e)
    );
    for (i, (name, _, _)) in crate::fileio::DEMOS.iter().enumerate() {
        let a = l();
        let s = crate::fileio::demo_song_n(i);
        let b = l();
        let h = undo::History::new(&s);
        let c = l();
        let mut e2 = crate::engine::Engine::new(48000.0);
        let d = l();
        e2.start(&s, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 512];
        for _ in 0..(40 * 48000 / 256) {
            e2.render(&s, &mut buf);
        }
        let f = l();
        println!(
            "SIZE {name:22} song {:.2} MB | history {:.2} MB | engine new {:.2} MB, after 40 s play +{:.2} MB",
            mb(b - a), mb(c - b), mb(d - c), mb(f - d)
        );
        drop((h, e2));
    }
    let a = l();
    let shared: SharedRef = Arc::new(Mutex::new(Shared {
        song: crate::fileio::demo_song(),
        engine: Engine::new(48000.0),
        preview: None,
    }));
    let b = l();
    let app = App::new(shared, None, None, None, String::new(), None, vec![]);
    let c = l();
    println!(
        "SIZE Shared {:.2} MB | App::new {:.2} MB, App inline {} B",
        mb(b - a),
        mb(c - b),
        std::mem::size_of_val(&app)
    );
    drop(app);
}
