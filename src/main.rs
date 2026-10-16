// ChipWave Tracker - GPL-3.0-or-later
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod audio;
mod engine;
mod fileio;
mod model;
mod presets;
mod synth;
mod theme;

use std::path::PathBuf;

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
        env!("CARGO_PKG_VERSION"), song.title, song.instruments.len(), song.patterns.len(), secs, peak, rms, nan, out.display(), wrote
    );
    let tmp = std::env::temp_dir().join("chipwave_selftest.cwt");
    let rt = fileio::save_cwt(&song, &tmp).and_then(|_| fileio::load_cwt(&tmp));
    let rt_ok = matches!(&rt, Ok(s) if s.patterns[0].cells == song.patterns[0].cells);
    report.push_str(&format!(
        "  .cwt round-trip: {}\n",
        if rt_ok { "ok" } else { "FAILED" }
    ));
    let ok = peak > 0.05 && rms > 0.005 && !nan && wrote.is_ok() && rt_ok;
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

fn main() {
    #[cfg(windows)]
    if std::env::args().len() > 1 {
        attach_console();
    }
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version") {
        println!("ChipWave Tracker {}", env!("CARGO_PKG_VERSION"));
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
    let open = args
        .get(1)
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from);
    if let Err(e) = app::run(open) {
        eprintln!("ChipWave failed to start: {e}");
        std::process::exit(1);
    }
}
