// ChipWave Tracker - GPL-3.0-or-later
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// Display version (Cargo: 0.2.2+fix.3; 0.2.3 is reserved for MIDI).
pub const VERSION: &str = "0.2.2.3";

mod app;
mod arrange;
mod audio;
mod automation;
mod engine;
mod export;
mod fileio;
mod fx;
mod input;
mod keymap;
mod model;
mod presets;
mod prompt;
mod recovery;
mod sample;
mod settings;
mod synth;
mod theme;

use std::path::PathBuf;

fn e_playing_ok(r1: u64, r2: u64) -> bool {
    (r2 as f64 - r1 as f64 * 1.5).abs() <= 2.0 && r1 > 0
}

fn self_test(out: Option<PathBuf>) -> i32 {
    let song = fileio::demo_song();
    let data = engine::render_song(&song, 44100);
    let peak = data.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let rms = (data.iter().map(|x| x * x).sum::<f32>() / data.len().max(1) as f32).sqrt();
    let nan = data.iter().any(|x| !x.is_finite());
    let out = out.unwrap_or_else(|| std::env::temp_dir().join("chipwave_selftest.wav"));
    let wrote = engine::write_wav(&out, &data, 44100);
    let secs = data.len() as f32 / 2.0 / 44100.0;
    let mut report = format!(
        "ChipWave {} self-test\n  demo: {} ({} instruments, {} patterns)\n  rendered {:.2} s, peak {:.3}, rms {:.4}, nan {}\n  wav: {} ({:?})\n",
        crate::VERSION, song.title, song.instruments.len(), song.patterns.len(), secs, peak, rms, nan, out.display(), wrote
    );
    let tmp = std::env::temp_dir().join("chipwave_selftest.cwt");
    let rt = fileio::save_cwt(&song, &tmp).and_then(|_| fileio::load_cwt(&tmp));
    let rt_ok = matches!(&rt, Ok(s) if s.patterns[0].cells == song.patterns[0].cells && s.mixer == song.mixer && s.automation == song.automation && s.order == song.order);
    report.push_str(&format!(
        "  .cwt round-trip: {}\n",
        if rt_ok { "ok" } else { "FAILED" }
    ));
    // tempo change while playing must apply at once (no restart)
    let (r1, r2, bpm_now) = {
        let mut e = engine::Engine::new(48000.0);
        e.start(&song, 0, 0, true, 0);
        let mut buf = vec![0.0f32; 512];
        let mut run = |e: &mut engine::Engine| {
            let b = e.rows_played;
            for _ in 0..(2 * 48000 / 256) {
                e.render(&song, &mut buf);
            }
            e.rows_played - b
        };
        let r1 = run(&mut e);
        e.set_tempo(song.bpm * 1.5, song.speed);
        let r2 = run(&mut e);
        (r1, r2, e.bpm())
    };
    let tempo_ok = e_playing_ok(r1, r2) && (bpm_now - song.bpm * 1.5).abs() < 0.01;
    report.push_str(&format!(
        "  tempo change while playing: {} -> {} BPM, rows/2 s {} -> {} ({})\n",
        song.bpm,
        bpm_now,
        r1,
        r2,
        if tempo_ok { "ok" } else { "FAILED" }
    ));
    // automation lanes must change the render (bass cutoff sweep + resonance)
    let auto_ok = {
        let mut s2 = song.clone();
        s2.automation.clear();
        let plain = engine::render_song(&s2, 22050);
        let with = engine::render_song(&song, 22050);
        let d: f32 = plain.iter().zip(&with).map(|(a, b)| (a - b).abs()).sum();
        report.push_str(&format!(
            "  automation: {} lanes, render difference {:.1} ({})\n",
            song.automation.len(),
            d,
            if d > 1.0 { "ok" } else { "FAILED" }
        ));
        d > 1.0
    };
    // MP3 export through the statically linked LAME, with ID3 tags
    let mp3_ok = {
        let p = std::env::temp_dir().join("chipwave_selftest.mp3");
        let o = export::Opts::default();
        let r = export::export(&song, &o, &p, &export::Progress::default());
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        let tag = id3::Tag::read_from_path(&p).ok();
        let tagged = tag
            .as_ref()
            .is_some_and(|t| t.comments().any(|c| c.text.contains("Made with ChipWave")));
        let ok = r.is_ok() && size > 10_000 && tagged;
        report.push_str(&format!(
            "  mp3 export: {} ({} bytes, tags {}) ({})\n",
            p.display(),
            size,
            if tagged { "ok" } else { "missing" },
            if ok { "ok" } else { "FAILED" }
        ));
        ok
    };
    // Fill to length: deterministic, exact length
    let fill_ok = {
        let mut s = song.clone();
        let mut b = s.patterns[0].clone();
        s.patterns[0].role = Some(arrange::Role::Intro);
        for r in [
            arrange::Role::Verse,
            arrange::Role::Chorus,
            arrange::Role::Outro,
        ] {
            b.role = Some(r);
            s.patterns.push(b.clone());
        }
        let st = arrange::Style::factory(4);
        let par = arrange::Params {
            style: &st,
            target_bars: 64,
            energy: &st.energy,
            variation: 0.5,
            seed: 7,
            locks: vec![],
        };
        let a = arrange::generate(&s, &par);
        let b = arrange::generate(&s, &par);
        let ok = matches!((&a, &b), (Ok(x), Ok(y)) if x == y && x.rows == 64 * arrange::rows_per_bar(&s));
        report.push_str(&format!(
            "  fill to length: {} ({})\n",
            a.as_ref()
                .map(|p| format!("{} sections, {:.0} bars", p.sections.len(), p.bars()))
                .unwrap_or_else(|e| e.clone()),
            if ok { "ok" } else { "FAILED" }
        ));
        ok
    };
    let ok = fill_ok
        && peak > 0.05
        && rms > 0.005
        && !nan
        && wrote.is_ok()
        && rt_ok
        && tempo_ok
        && auto_ok
        && mp3_ok;
    report.push_str(if ok {
        "RESULT: PASS\n"
    } else {
        "RESULT: FAIL\n"
    });
    print!("{report}");
    let _ = std::fs::write(std::env::temp_dir().join("chipwave_selftest.txt"), &report);
    if ok {
        0
    } else {
        1
    }
}

#[cfg(windows)]
fn attach_console() {
    // GUI-subsystem exe: reattach to the parent console so --self-test/--version print in cmd/PowerShell.
    extern "system" {
        fn AttachConsole(pid: u32) -> i32;
    }
    unsafe {
        AttachConsole(0xFFFF_FFFF);
    }
}

/// Real-time audio test: play the demo block for `secs` seconds and report underruns.
fn play_test(secs: u64) -> i32 {
    use std::sync::atomic::Ordering;
    let shared = std::sync::Arc::new(std::sync::Mutex::new(audio::Shared {
        song: fileio::demo_song(),
        engine: engine::Engine::new(48000.0),
        preview: None,
    }));
    let a = match audio::start(shared.clone(), settings::load().buffer) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("play-test: no audio: {e}");
            return 2;
        }
    };
    std::thread::sleep(std::time::Duration::from_millis(300));
    {
        let mut s = audio::lock(&shared);
        let audio::Shared { song, engine, .. } = &mut *s;
        engine.start(song, 0, 0, true, 0);
    }
    std::thread::sleep(std::time::Duration::from_secs(secs));
    let u = a.stats.underruns.load(Ordering::Relaxed);
    let msg = format!(
        "play-test: device '{}' {} Hz, buffer {}, callback {} frames, latency {:.1} ms, underruns {}\n",
        a.device_name,
        a.sample_rate,
        a.buffer,
        a.stats.callback_frames.load(Ordering::Relaxed),
        a.latency_ms(),
        u
    );
    print!("{msg}");
    let _ = std::fs::write(std::env::temp_dir().join("chipwave_playtest.txt"), &msg);
    if u == 0 {
        0
    } else {
        3
    }
}

fn main() {
    #[cfg(windows)]
    if std::env::args().len() > 1 {
        attach_console();
    }
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version") {
        println!("ChipWave Tracker {}", crate::VERSION);
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--self-test") {
        let out = args
            .get(i + 1)
            .filter(|a| !a.starts_with("--"))
            .map(PathBuf::from);
        std::process::exit(self_test(out));
    }
    if let Some(i) = args.iter().position(|a| a == "--render") {
        let (Some(inp), Some(out)) = (args.get(i + 1), args.get(i + 2)) else {
            eprintln!("usage: chipwave --render <song.cwt|demo> <out.wav>");
            std::process::exit(2);
        };
        let song = if inp == "demo" {
            Ok(fileio::demo_song())
        } else {
            fileio::load_cwt(std::path::Path::new(inp))
        };
        match song.and_then(|s| {
            engine::write_wav(
                std::path::Path::new(out),
                &engine::render_song(&s, 44100),
                44100,
            )
        }) {
            Ok(()) => println!("wrote {out}"),
            Err(e) => {
                eprintln!("render failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--play-test") {
        let secs: u64 = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(10);
        std::process::exit(play_test(secs));
    }
    let open = args
        .get(1)
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from);
    if let Err(e) = app::run(open) {
        eprintln!("ChipWave failed to start: {e}");
        std::process::exit(1);
    }
}
