// ChipWave Tracker - GPL-3.0-or-later
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// Display version (Cargo: 0.2.3+fix.1).
pub const VERSION: &str = "0.2.3.9";

mod app;
mod arrange;
mod audio;
mod automation;
mod chords;
mod engine;
mod export;
mod fileio;
mod fx;
mod input;
mod instfile;
mod keymap;
mod master;
mod midi;
mod model;
mod presets;
mod prompt;
mod recovery;
mod sample;
mod settings;
mod speech;
mod synth;
mod theme;
mod update;
mod voice;

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
    // MIDI: raw bytes -> hub -> lock-free queue -> engine live voices
    let midi_ok = {
        let (hub, rx) = midi::Hub::new();
        hub.cfg.instr.store(1, std::sync::atomic::Ordering::Relaxed);
        let mut e = engine::Engine::new(48000.0);
        e.set_live_input(Some(rx));
        for k in [60u8, 64, 67] {
            hub.inject(&[0x90, k, 100]);
        }
        hub.inject(&[0xE0, 0x00, 0x60]); // bend up
        let mut buf = vec![0.0f32; 4096];
        e.render(&song, &mut buf);
        let voices = e.live_voices();
        let lvl = buf.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        for k in [60u8, 64, 67] {
            hub.inject(&[0x80, k, 0]);
        }
        for _ in 0..100 {
            e.render(&song, &mut buf);
        }
        let ok = voices == 3 && lvl > 0.001 && e.live_voices() == 0;
        let ports = match midi::list_ports() {
            Ok(p) => format!("{} input(s)", p.len()),
            Err(e) => format!("inputs not available ({e})"),
        };
        report.push_str(&format!(
            "  midi live play: {voices} voices, peak {lvl:.3}, released {} ({}); {ports}\n",
            e.live_voices() == 0,
            if ok { "ok" } else { "FAILED" }
        ));
        ok
    };
    // 0.2.3.9: .cwi instrument file round trip, key detection, chord writing
    let inst_ok = {
        let dir = std::env::temp_dir().join("chipwave_selftest_lib");
        let _ = std::fs::create_dir_all(&dir);
        let mut all = true;
        let mut n = 0;
        for i in &song.instruments {
            let p = instfile::free_path(&dir, &i.name);
            let back = instfile::save_cwi(i, &p).and_then(|_| instfile::load_cwi(&p));
            all &= matches!(&back, Ok(b) if b.name == i.name);
            n += 1;
        }
        let listed = instfile::scan(&dir).len();
        let _ = std::fs::remove_dir_all(&dir);
        let key = chords::detect(&song).first().map(|(k, _)| *k);
        let mut pat = song.patterns[0].clone();
        let w = key.and_then(|k| {
            let prog: Vec<chords::Step> = [0usize, 5, 2, 6]
                .iter()
                .map(|&d| chords::Step::new(d, chords::ChordKind::Triad))
                .collect();
            chords::write(
                &mut pat,
                k,
                &prog,
                &chords::WriteOpts {
                    style: chords::Style::Chords,
                    channel: 1,
                    instr: 1,
                    octave: 3,
                    rows_per_beat: song.lines_per_beat.max(1) as usize,
                    start_row: 0,
                    fill: true,
                    vol: 0,
                    seed: 1,
                },
            )
            .ok()
        });
        let ok = all && listed == n && key.is_some() && w.as_ref().is_some_and(|w| w.notes > 0);
        report.push_str(&format!(
            "  instruments + chords: {n} .cwi saved/loaded/listed {listed}, key {}, {} chord note(s) ({})\n",
            key.map(|k| k.name()).unwrap_or_else(|| "?".into()),
            w.map(|w| w.notes).unwrap_or(0),
            if ok { "ok" } else { "FAILED" }
        ));
        ok
    };
    let ok = fill_ok
        && inst_ok
        && midi_ok
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

/// `--screenshot <out.png> [--view v] [--theme dark|amiga] [--size WxH]`
/// (developer flag for the README screenshots; see src/app/shot.rs)
fn screenshot(args: &[String], i: usize) -> i32 {
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|k| args.get(k + 1))
            .cloned()
    };
    let Some(out) = args.get(i + 1).filter(|a| !a.starts_with("--")) else {
        eprintln!(
            "usage: --screenshot <out.png> [--view {}] [--theme dark|amiga] [--size 1600x900] [--song file.cwt|file.cwt.toml]",
            app::shot::VIEWS.join("|")
        );
        return 2;
    };
    let view = flag("--view").unwrap_or_else(|| "pattern".into());
    if !app::shot::VIEWS.contains(&view.as_str()) {
        eprintln!(
            "unknown view {view}; one of {}",
            app::shot::VIEWS.join(", ")
        );
        return 2;
    }
    let size = flag("--size")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1600.0, 900.0]);
    let amiga = flag("--theme").is_some_and(|t| t.to_ascii_lowercase().starts_with('a'));
    // isolated settings / recovery folder, so the user's own settings are untouched
    let tmp = std::env::temp_dir().join(format!("chipwave-shot-{}", std::process::id()));
    std::env::set_var("XDG_CONFIG_HOME", &tmp);
    std::env::set_var("APPDATA", &tmp);
    let mut st = settings::Settings {
        theme: if amiga {
            theme::AMIGA.name
        } else {
            theme::DARK.name
        }
        .into(),
        maximized: false,
        ..Default::default()
    };
    st.extra.update_check = false;
    settings::save(&st);
    let mut shot = app::shot::Shot::new(PathBuf::from(out), &view);
    shot.hold = flag("--hold").and_then(|s| s.parse().ok());
    let r = app::screenshot(shot, size, flag("--song").map(PathBuf::from));
    let _ = std::fs::remove_dir_all(&tmp);
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("screenshot failed: {e}");
            1
        }
    }
}

/// `--midi-list`: print the MIDI input devices (for checking a keyboard)
fn midi_list() -> i32 {
    match midi::list_ports() {
        Ok(p) if p.is_empty() => {
            println!("MIDI inputs: none found (plug the keyboard in, then run again)");
            0
        }
        Ok(p) => {
            println!("MIDI inputs ({}):", p.len());
            for (i, n) in p.iter().enumerate() {
                println!("  {}: {n}", i + 1);
            }
            0
        }
        Err(e) => {
            println!("MIDI: cannot list inputs: {e}");
            2
        }
    }
}

/// `--midi-monitor [secs]`: connect to every input and print what arrives
fn midi_monitor(secs: u64) -> i32 {
    let (mut hub, mut audio) = midi::Hub::new();
    hub.connect("*");
    if let Some(e) = &hub.error {
        println!("MIDI: {e}");
    }
    let names = hub.connected().join(", ");
    if names.is_empty() {
        println!("MIDI monitor: no inputs connected");
        return 2;
    }
    println!("MIDI monitor: listening to {names} for {secs} s (play some keys)...");
    let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let mut n = 0;
    while std::time::Instant::now() < end {
        while let Ok(e) = hub.ui_rx.pop() {
            n += 1;
            match e {
                midi::UiEv::On { key, note, vel, .. } => {
                    println!(
                        "  note on  key {key:3} = {} velocity {:.0} %",
                        model::note_name(note),
                        vel * 100.0
                    )
                }
                midi::UiEv::Off { key } => println!("  note off key {key:3}"),
                midi::UiEv::Cc { cc, val } => println!("  CC {cc:3} = {val}"),
            }
        }
        while let Ok(e) = audio.pop() {
            match e {
                midi::LiveEv::Sustain(on) => {
                    println!("  sustain pedal {}", if on { "down" } else { "up" })
                }
                midi::LiveEv::Bend(s) => println!("  pitch bend {s:+.2} semitones"),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    println!("MIDI monitor: {n} note/CC message(s)");
    0
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
    if let Some(i) = args.iter().position(|a| a == "--screenshot") {
        std::process::exit(screenshot(&args, i));
    }
    if args.iter().any(|a| a == "--check-updates") {
        // same check as Help > Check for updates (5 s timeout); exit 0 = ran
        println!("ChipWave Tracker {}: checking for updates…", crate::VERSION);
        match update::evaluate(update::fetch_latest(), crate::VERSION) {
            update::Outcome::Newer(r) => println!(
                "  newer version {} available
  release page: {}
  installer:    {}",
                r.version,
                r.page,
                r.download_url()
            ),
            update::Outcome::Latest(v) => {
                println!("  you're on the latest version (latest published: {v})")
            }
            update::Outcome::Failed(e) => {
                println!("  couldn't check for updates: {e}");
                std::process::exit(2);
            }
        }
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
            eprintln!("usage: chipwave --render <song.cwt|demo> <out.wav|out.mp3>");
            std::process::exit(2);
        };
        let song = if inp == "demo" {
            Ok(fileio::demo_song())
        } else if inp.ends_with(".toml") {
            // single-file text song (demos/*.cwt.toml)
            std::fs::read_to_string(inp)
                .map_err(|e| e.to_string())
                .and_then(|t| fileio::song_from_text(&t))
        } else {
            fileio::load_cwt(std::path::Path::new(inp))
        };
        // same path as File > Export (limiter on, 2 s tail); .mp3 = LAME V2
        let opts = export::Opts {
            format: if out.to_ascii_lowercase().ends_with(".mp3") {
                export::Format::Mp3
            } else {
                export::Format::Wav
            },
            ..Default::default()
        };
        match song.and_then(|s| {
            export::export(
                &s,
                &opts,
                std::path::Path::new(out),
                &export::Progress::default(),
            )
        }) {
            Ok(msg) => println!("{msg}"),
            Err(e) => {
                eprintln!("render failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|a| a == "--midi-list") {
        std::process::exit(midi_list());
    }
    if let Some(i) = args.iter().position(|a| a == "--midi-monitor") {
        let secs: u64 = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(15);
        std::process::exit(midi_monitor(secs));
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
