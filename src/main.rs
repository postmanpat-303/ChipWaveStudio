// ChipWave Studio - GPL-3.0-or-later
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

/// Display version (Cargo: 0.2.17+fix.0 -> "0.2.17.0"; the updater compares it part by part).
pub const VERSION: &str = "0.2.17.0";

/// The version as shown (About, window title): a test build (the pack
/// scripts set CW_BUILD, e.g. "combo2 8bdf40c") reads
/// "0.2.13.0 · test build (combo2 8bdf40c)"; a release build just "0.2.17.0".
pub fn version_label() -> String {
    version_label_for(option_env!("CW_BUILD"))
}

fn version_label_for(build: Option<&str>) -> String {
    match build.map(str::trim) {
        Some(b) if !b.is_empty() => format!("{VERSION} · test build ({b})"),
        _ => VERSION.to_string(),
    }
}

#[cfg(test)]
#[test]
fn a_test_build_says_so_next_to_the_version() {
    assert_eq!(version_label_for(None), VERSION);
    assert_eq!(version_label_for(Some(" ")), VERSION);
    assert_eq!(
        version_label_for(Some("combo2 8bdf40c")),
        format!("{VERSION} · test build (combo2 8bdf40c)")
    );
}

mod advedit;
mod app;
mod arrange;
mod audio;
mod audiofile;
mod automation;
mod bassgen;
mod browser;
mod chanops;
mod chipfx;
mod chordgen;
mod chords;
mod credits;
mod ctlmap;
mod drives;
mod drumkit;
mod drumpat;
mod ember;
mod engine;
mod export;
mod fileio;
mod fx;
mod fxmod;
mod gen;
mod gen2;
mod gen_wave;
mod grid;
mod input;
mod instfile;
mod keymap;
mod knobs;
mod ladder;
mod master;
mod melodygen;
mod midi;
mod model;
mod modulation;
mod plugin;
mod poly;
mod presets;
mod prompt;
mod quickmod;
mod racks;
mod recovery;
#[cfg(test)]
mod render_hashes;
#[cfg(test)]
mod render_hashes_gen;
mod sample;
mod settings;
mod showcase;
mod smf;
mod speech;
mod strings;
mod synth;
mod theme;
mod timeline;
mod titlebar;
mod transform;
mod update;
mod voice;
mod wavetable;
// only the windows-gnu (mingw) build: MSVC's ucrt maths is fast already,
// and its import library defines the same symbols (LNK2005)
#[cfg(all(windows, target_env = "gnu"))]
mod winmath;

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
    // Timeline (M0): order list -> clips -> order list renders bit-exact,
    // and a song that needs a newer reader is refused
    let timeline_ok = {
        let (tl, _) = timeline::from_order(&song);
        let back = timeline::consolidated_song(&song, &tl);
        let want = engine::render_song(&song, 22050);
        let same = back
            .as_ref()
            .is_ok_and(|b| engine::render_song(b, 22050) == want);
        // timeline mode (M2): the engine plays the clips directly, the same
        // samples; saving needs reader version 2 and loads back
        let mut m = song.clone();
        timeline::enter_timeline_mode(&mut m);
        let mode_same = engine::render_song(&m, 22050) == want;
        let tmp = std::env::temp_dir().join("chipwave_selftest_tl.cwt");
        let mode_saved = fileio::save_cwt(&m, &tmp)
            .and_then(|_| fileio::load_cwt(&tmp))
            .is_ok_and(|b| {
                b.timeline
                    .as_deref()
                    .zip(m.timeline.as_deref())
                    .is_some_and(|(x, y)| x.same_content(y))
            });
        std::fs::remove_file(&tmp).ok();
        report.push_str(&format!(
            "  timeline mode: plays bit-exact {}, timeline.toml round trip {} ({})\n",
            if mode_same { "yes" } else { "NO" },
            if mode_saved { "yes" } else { "NO" },
            if mode_same && mode_saved {
                "ok"
            } else {
                "FAILED"
            }
        ));
        // note clips (M3b): a chord in a note clip plays, the same every time
        let notes_ok = {
            let mut n = m.clone();
            let tr = n.channels.saturating_sub(1);
            let id = timeline::new_note_clip(&mut n, tr, 0);
            let t = n.timeline.as_deref_mut();
            if let (Some(id), Some(t)) = (id, t) {
                if let Some(timeline::ClipKind::Notes { data }) =
                    t.clips.iter().find(|c| c.id == id).map(|c| c.kind.clone())
                {
                    t.notes[data as usize].make_mut().notes = [49u8, 53, 56]
                        .iter()
                        .map(|&key| timeline::Note {
                            len: 4 * timeline::PPQ,
                            key,
                            instr: 1,
                            ..Default::default()
                        })
                        .collect();
                }
            }
            let a = engine::render_song(&n, 22050);
            let b = engine::render_song(&n, 22050);
            let plays = a != want && a.iter().all(|x| x.is_finite());
            let ok = plays && a == b;
            report.push_str(&format!(
                "  note clips: a chord plays {}, repeatable {} ({})\n",
                if plays { "yes" } else { "NO" },
                if a == b { "yes" } else { "NO" },
                if ok { "ok" } else { "FAILED" }
            ));
            ok
        };
        // audio clips (M4): a decoded WAV plays at its own speed at another
        // output rate, embedded in the song file and loaded back
        let audio_ok = {
            let mut n = m.clone();
            let tr = n.channels.saturating_sub(1);
            let data: Vec<f32> = (0..11025)
                .map(|i| (i as f32 * 441.0 * std::f32::consts::TAU / 11025.0).sin() * 0.25)
                .collect();
            let bytes = audiofile::wav_f32(11025, 1, &data);
            let decoded = audiofile::decode(&bytes, "wav").ok();
            let placed = decoded.is_some_and(|pcm| {
                let t = n.timeline.as_deref_mut().unwrap();
                let a = timeline::AudioAsset::from_pcm("tone.wav", "", "wav", pcm, bytes);
                let k = timeline::add_audio_asset(t, a);
                timeline::new_audio_clip(&mut n, tr, 0, k).is_some()
            });
            let a = engine::render_song(&n, 22050);
            let plays = placed && a != want && a.iter().all(|x| x.is_finite());
            let tmp = std::env::temp_dir().join("chipwave_selftest_audio.cwt");
            let saved = fileio::save_cwt(&n, &tmp)
                .and_then(|_| fileio::load_cwt_warn(&tmp))
                .is_ok_and(|(b, w)| w.is_empty() && engine::render_song(&b, 22050) == a);
            std::fs::remove_file(&tmp).ok();
            let ok = plays && saved;
            report.push_str(&format!(
                "  audio clips: a WAV plays {}, embedded round trip {} ({})\n",
                if plays { "yes" } else { "NO" },
                if saved { "yes" } else { "NO" },
                if ok { "ok" } else { "FAILED" }
            ));
            ok
        };
        // automation lanes (M5): track lanes turn every fader down, a clip
        // lane wins inside its clip, and both survive a save and load
        let lanes_ok = {
            let lane = |ch: usize, v: f32| automation::Lane {
                target: format!("mixer.ch.{}.volume", ch + 1),
                label: String::new(),
                points: vec![automation::Point {
                    pos: 0.0,
                    value: v,
                    shape: automation::Shape::Linear,
                    curve: 0.0,
                }],
                enabled: true,
                mode: Default::default(),
            };
            let peak = |a: &[f32]| a.iter().fold(0.0f32, |p, x| p.max(x.abs()));
            let mut n = m.clone();
            if let Some(t) = n.timeline.as_deref_mut() {
                for (i, tr) in t.tracks.iter_mut().enumerate() {
                    tr.auto.push(lane(i, 0.0));
                }
            }
            let quiet = engine::render_song(&n, 22050);
            if let Some(t) = n.timeline.as_deref_mut() {
                for c in t.clips.iter_mut() {
                    c.auto.push(lane(c.track as usize, 0.8));
                }
            }
            let loud = engine::render_song(&n, 22050);
            let (pq, pl, pw) = (peak(&quiet), peak(&loud), peak(&want));
            // (sends and their returns keep some level when the faders are down)
            let plays = pq < pw * 0.9 && pl > pq * 1.1 && loud.iter().all(|x| x.is_finite());
            let tmp = std::env::temp_dir().join("chipwave_selftest_lanes.cwt");
            let saved = fileio::save_cwt(&n, &tmp)
                .and_then(|_| fileio::load_cwt_warn(&tmp))
                .is_ok_and(|(b, w)| w.is_empty() && engine::render_song(&b, 22050) == loud);
            std::fs::remove_file(&tmp).ok();
            let ok = plays && saved;
            report.push_str(&format!(
                "  automation lanes: track and clip lanes play {} (peak {:.3} -> {:.3}, song {:.3}), round trip {} ({})\n",
                if plays { "yes" } else { "NO" },
                pq,
                pl,
                pw,
                if saved { "yes" } else { "NO" },
                if ok { "ok" } else { "FAILED" }
            ));
            ok
        };
        let same = same && mode_same && mode_saved && notes_ok && audio_ok && lanes_ok;
        let refused = fileio::song_from_text("min_reader_version = 999\n")
            .err()
            .is_some_and(|e| fileio::is_too_new(&e));
        report.push_str(&format!(
            "  timeline: {} tracks, {} clips, bit-exact round trip {}, newer song refused {} ({})\n",
            tl.tracks.len(),
            tl.clips.len(),
            if same { "yes" } else { "NO" },
            if refused { "yes" } else { "NO" },
            if same && refused { "ok" } else { "FAILED" }
        ));
        same && refused
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
                    rows_per_beat: crate::model::ROWS_PER_BEAT as usize,
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
    // 0.2.3.10: spoken words (formant + throat tract) and sample slicing
    let speech_ok = {
        let mut s = song.clone();
        for row in s.patterns[0].cells.iter_mut() {
            row.iter_mut().for_each(|c| *c = model::Cell::default());
        }
        s.order = vec![0];
        let base = s.instruments.len() as u8;
        for name in ["Talk Ka-Ta", "Throat Speech"] {
            if let Some(p) = voice::preset(name) {
                s.instruments.push(model::Instrument {
                    name: name.into(),
                    volume: 1.0,
                    transpose: 0,
                    fx: vec![],
                    kind: model::InstrumentKind::Voice(p),
                });
            }
        }
        for (r, ch, i) in [(0usize, 0usize, 1u8), (8, 0, 1), (16, 1, 2), (24, 1, 2)] {
            s.patterns[0].cells[r][ch] = model::Cell {
                note: 37,
                instr: base + i,
                ..Default::default()
            };
        }
        let pcm = engine::render_song(&s, 44100);
        let fin = pcm.iter().all(|x| x.is_finite());
        let half = pcm.len() / 2;
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt();
        let (a, b) = (rms(&pcm[..half]), rms(&pcm[half..]));
        // a 2 s loop of 8 hits must slice into 8
        let mut n = 0x2545_F491u32;
        let mut d = vec![0.0f32; 96000];
        for h in 0..8 {
            for k in 0..6000 {
                n ^= n << 13;
                n ^= n >> 17;
                n ^= n << 5;
                d[h * 12000 + k] +=
                    (n as f32 / u32::MAX as f32 - 0.5) * (-(k as f32) / 900.0).exp();
            }
        }
        let sl = sample::transient_slices(&d, 48000, 0.5, 50.0);
        let ok = fin
            && a > 0.003
            && b > 0.003
            && sl.len() == 8
            && s.instruments.len() == base as usize + 2;
        report.push_str(&format!(
            "  voice words + slicing: formant rms {a:.4}, throat rms {b:.4}, {} slice(s) ({})\n",
            sl.len(),
            if ok { "ok" } else { "FAILED" }
        ));
        ok
    };
    // 0.2.3.12: generators (every drum preset + kits) and the ladder filter
    let gen_ok = {
        let mut s = song.clone();
        for row in s.patterns[0].cells.iter_mut() {
            row.iter_mut().for_each(|c| *c = model::Cell::default());
        }
        s.order = vec![0];
        let base = s.instruments.len();
        let presets = gen::presets();
        let rows = s.patterns[0].cells.len();
        for (k, (name, _, g)) in presets.iter().enumerate() {
            s.instruments.push(model::Instrument {
                name: (*name).into(),
                volume: 0.8,
                transpose: 0,
                fx: vec![],
                kind: model::InstrumentKind::Gen(g.clone()),
            });
            let r = (k * 2) % rows;
            s.patterns[0].cells[r][k % 6] = model::Cell {
                note: 49,
                instr: (base + k + 1) as u8,
                fx: if k % 3 == 0 { 0x43 } else { 0 },
                fxp: 0x2C,
                ..Default::default()
            };
        }
        // a synth through each filter model
        for (m, fm) in ladder::FilterModel::ALL.iter().enumerate() {
            let p = synth::Patch {
                filter_model: *fm,
                resonance: 0.8,
                ..Default::default()
            };
            s.instruments.push(model::Instrument {
                name: fm.name().into(),
                volume: 0.6,
                transpose: 0,
                fx: vec![],
                kind: model::InstrumentKind::Synth(p),
            });
            s.patterns[0].cells[1 + m * 8][6] = model::Cell {
                note: 25,
                instr: s.instruments.len() as u8,
                ..Default::default()
            };
        }
        let pcm = engine::render_song(&s, 44100);
        let fin = pcm.iter().all(|x| x.is_finite());
        let tp = master::true_peak(&pcm);
        let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
        let ok = fin && tp < 1.0 && rms > 0.003 && s.instruments.len() == base + presets.len() + 3;
        report.push_str(&format!(
            "  generators: {} presets (drums, kits, chip FX, analog, FM-4, noise) + 3 filter models, rms {rms:.4}, true peak {:.2} dBTP ({})\n",
            presets.len(),
            20.0 * tp.max(1e-9).log10(),
            if ok { "ok" } else { "FAILED" }
        ));
        ok
    };
    // 0.2.3.11: Voice Transform presets + the three FX devices
    let tf_ok = {
        let mut s = song.clone();
        for row in s.patterns[0].cells.iter_mut() {
            row.iter_mut().for_each(|c| *c = model::Cell::default());
        }
        s.order = vec![0];
        let names = [
            "Metal Demon",
            "Digital Ghost",
            "Sub Beast",
            "Cyber Choir",
            "Ritual Growl",
        ];
        let base = s.instruments.len() as u8;
        for (k, name) in names.iter().enumerate() {
            if let Some(p) = voice::preset(name) {
                s.instruments.push(model::Instrument {
                    name: (*name).into(),
                    volume: 0.8,
                    transpose: 0,
                    fx: vec![],
                    kind: model::InstrumentKind::Voice(p),
                });
            }
            s.patterns[0].cells[k * 6][k % 4] = model::Cell {
                note: 25,
                instr: base + k as u8 + 1,
                ..Default::default()
            };
        }
        if let Some(i) = s.instruments.get_mut(1) {
            i.fx = vec![
                fx::Device::new(fx::FxKind::Resonator),
                fx::Device::new(fx::FxKind::Digital),
                fx::Device::new(fx::FxKind::Sub),
            ];
        }
        s.patterns[0].cells[2][4] = model::Cell {
            note: 37,
            instr: 2,
            fx: 0x3F,
            fxp: 0x80,
            ..Default::default()
        };
        let pcm = engine::render_song(&s, 44100);
        let fin = pcm.iter().all(|x| x.is_finite());
        let tp = master::true_peak(&pcm);
        let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
        let ok = fin && tp < 1.0 && rms > 0.003 && s.instruments.len() == base as usize + 5;
        report.push_str(&format!(
            "  voice transform: 5 presets + 3 FX devices, rms {rms:.4}, true peak {:.2} dBTP ({})\n",
            20.0 * tp.max(1e-9).log10(),
            if ok { "ok" } else { "FAILED" }
        ));
        ok
    };
    // plugin host (0.2.3.16): a missing plugin stays a silent placeholder;
    // the scan cache is listed; CHIPWAVE_SELFTEST_PLUGIN=<file.clap|.vst3>
    // also plays a note through that plugin and needs sound
    let plug_ok = {
        let mut s = fileio::demo_song();
        s.instruments[0].kind = model::InstrumentKind::Plugin(plugin::PluginPatch {
            format: "clap".into(),
            id: "org.chipwave.self-test.missing".into(),
            name: "Missing".into(),
            vendor: String::new(),
            path: "/nonexistent/missing.clap".into(),
            state: String::new(),
            params: vec![],
            macros: vec![],
        });
        // and a missing VST3 one (VST3 hosting, Unreleased)
        if s.instruments.len() > 1 {
            s.instruments[1].kind = model::InstrumentKind::Plugin(plugin::PluginPatch {
                format: "vst3".into(),
                id: "00000000000000000000000000C0FFEE".into(),
                name: "Missing VST3".into(),
                vendor: String::new(),
                path: "/nonexistent/missing.vst3".into(),
                state: String::new(),
                params: vec![],
                macros: vec![],
            });
        }
        s.order.truncate(1);
        let warn = plugin::missing_warnings(&s);
        let pcm = engine::render_song(&s, 22050);
        let want = s.instruments.len().min(2);
        let missing_ok = warn.len() == want && pcm.iter().all(|x| x.is_finite());
        let found = plugin::scan::cache().instruments();
        report.push_str(&format!(
            "  plugins: missing plugin = silent placeholder + warning ({}); {} cached plugin instrument(s){}\n",
            if missing_ok { "ok" } else { "FAILED" },
            found.len(),
            found
                .iter()
                .take(6)
                .map(|f| format!(", {} ({})", f.desc.name, f.format.to_uppercase()))
                .collect::<String>()
        ));
        let mut ok = missing_ok;
        if let Some(p) = std::env::var_os("CHIPWAVE_SELFTEST_PLUGIN") {
            let p = PathBuf::from(p);
            let r = plugin::scan::check_file(&p);
            let inst = r.plugins.iter().find(|x| x.desc.is_instrument());
            let rendered = inst.map(|f| {
                let mut s = fileio::demo_song();
                s.instruments[0].kind = model::InstrumentKind::Plugin(plugin::PluginPatch {
                    format: f.format.clone(),
                    id: f.desc.id.clone(),
                    name: f.desc.name.clone(),
                    vendor: f.desc.vendor.clone(),
                    path: p.to_string_lossy().into_owned(),
                    state: String::new(),
                    params: vec![],
                    macros: vec![],
                });
                s.order.truncate(1);
                let pcm = engine::render_song(&s, 44100);
                let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
                (f.desc.name.clone(), rms, pcm.iter().all(|x| x.is_finite()))
            });
            let good = matches!(&rendered, Some((_, rms, fin)) if *rms > 1e-4 && *fin);
            report.push_str(&format!(
                "  plugin {}: {} ({})\n",
                p.display(),
                match &rendered {
                    Some((n, rms, _)) => format!("{n}, demo block rms {rms:.4}"),
                    None => format!("no instrument ({})", r.message),
                },
                if good { "ok" } else { "FAILED" }
            ));
            ok &= good;
        }
        // plugin effects: a missing one is bypassed (the song still plays);
        // CHIPWAVE_SELFTEST_PLUGIN_FX=<file.clap|.vst3> runs the demo through that
        // effect and needs sound
        let fx_song = |patch: plugin::PluginPatch| {
            let mut s = fileio::demo_song();
            s.instruments[0].fx.push(fx::Device::plugin(patch));
            s.order.truncate(1);
            s
        };
        let missing = fx_song(plugin::PluginPatch {
            format: "clap".into(),
            id: "org.chipwave.self-test.missing-fx".into(),
            name: "Missing FX".into(),
            vendor: String::new(),
            path: "/nonexistent/missing-fx.clap".into(),
            state: String::new(),
            params: vec![],
            macros: vec![],
        });
        let warn = plugin::fxrack::missing_warnings(&missing);
        let pcm = engine::render_song(&missing, 22050);
        let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
        let fx_ok = warn.len() == 1 && rms > 0.003 && pcm.iter().all(|x| x.is_finite());
        let found = plugin::scan::cache().effects();
        report.push_str(&format!(
            "  plugin effects: missing effect = bypassed + warning, rms {rms:.4} ({}); {} cached plugin effect(s)\n",
            if fx_ok { "ok" } else { "FAILED" },
            found.len()
        ));
        ok &= fx_ok;
        if let Some(p) = std::env::var_os("CHIPWAVE_SELFTEST_PLUGIN_FX") {
            let p = PathBuf::from(p);
            let r = plugin::scan::check_file(&p);
            let rendered = r.plugins.iter().find(|x| x.desc.is_effect()).map(|f| {
                let s = fx_song(plugin::PluginPatch {
                    format: f.format.clone(),
                    id: f.desc.id.clone(),
                    name: f.desc.name.clone(),
                    vendor: f.desc.vendor.clone(),
                    path: p.to_string_lossy().into_owned(),
                    state: String::new(),
                    params: vec![],
                    macros: vec![],
                });
                let pcm = engine::render_song(&s, 44100);
                let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
                (f.desc.name.clone(), rms, pcm.iter().all(|x| x.is_finite()))
            });
            let good = matches!(&rendered, Some((_, rms, fin)) if *rms > 1e-4 && *fin);
            report.push_str(&format!(
                "  plugin effect {}: {} ({})\n",
                p.display(),
                match &rendered {
                    Some((n, rms, _)) => format!("{n}, demo block rms {rms:.4}"),
                    None => format!("no effect ({})", r.message),
                },
                if good { "ok" } else { "FAILED" }
            ));
            ok &= good;
        }
        ok
    };
    // drag-to-modulate: an LFO on a synth's cutoff changes the render, zero
    // depth doesn't, and the routing survives a .cwt round trip
    let mod_ok = {
        let mut s = fileio::demo_song();
        s.order.truncate(1);
        s.automation.clear();
        let synth = s
            .instruments
            .iter()
            .position(|i| matches!(i.kind, model::InstrumentKind::Synth(_)))
            .unwrap_or(0);
        let plain = engine::render_song(&s, 22050);
        let k = s
            .modulation
            .add(synth, modulation::ModKind::Lfo)
            .unwrap_or(0);
        s.modulation.mods[k].shape = modulation::LfoShape::Square;
        let id = s.modulation.mods[k].id;
        s.modulation
            .connect(id, &format!("instrument.{}.cutoff", synth + 1), 0.5);
        let modded = engine::render_song(&s, 22050);
        let diff = plain
            .iter()
            .zip(&modded)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        let p = std::env::temp_dir().join("chipwave_selftest_mod.cwt");
        let rt = fileio::save_cwt(&s, &p)
            .and_then(|_| fileio::load_cwt(&p))
            .map(|b| b.modulation == s.modulation)
            .unwrap_or(false);
        let _ = std::fs::remove_file(&p);
        let good = diff > 1e-3 && modded.iter().all(|x| x.is_finite()) && rt;
        report.push_str(&format!(
            "  modulation: LFO on instrument {:02X} cutoff, max diff {diff:.3}, modulation.toml round trip {} ({})\n",
            synth + 1,
            if rt { "ok" } else { "FAILED" },
            if good { "ok" } else { "FAILED" }
        ));
        good
    };
    // wavetable: built-ins ready, a high note stays alias-free, a
    // Serum-style 3-frame WAV imports and survives a .cwt round trip
    let wt_ok = {
        let tables = wavetable::init().len();
        let mut w = gen_wave::WaveP {
            filter_on: false,
            ..Default::default()
        };
        w.p[0] = gen_wave::WAVE[0].unmap(0.67); // saw
        let mut st = gen_wave::WaveState::default();
        let (mac, knob) = ([-1.0f32; 8], gen::NO_KNOBS);
        let ov = gen::Ov {
            mac: &mac,
            knob: &knob,
        };
        let (sr, f) = (48000.0f32, 3100.0f32);
        let y: Vec<f32> = (0..24000)
            .map(|_| st.render(&w, ov, f, sr, true).0)
            .collect();
        let bin = |hz: f32| {
            let (mut a, mut b) = (0.0f64, 0.0f64);
            for (i, x) in y[4800..].iter().enumerate() {
                let p = std::f64::consts::TAU * (hz * i as f32 / sr) as f64;
                a += *x as f64 * p.cos();
                b += *x as f64 * p.sin();
            }
            ((a * a + b * b).sqrt() / (y.len() - 4800) as f64) as f32
        };
        let fund = bin(f);
        let alias = bin(sr - 8.0 * f); // where harmonic 8 would fold
        let mut bytes = std::io::Cursor::new(Vec::new());
        if let Ok(mut wr) = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        ) {
            for fr in 0..3 {
                for i in 0..wavetable::FRAME {
                    let t = i as f32 / wavetable::FRAME as f32;
                    let v = (t * std::f32::consts::TAU * (fr + 1) as f32).sin();
                    let _ = wr.write_sample((v * 20000.0) as i16);
                }
            }
            let _ = wr.finalize();
        }
        let imp = wavetable::import_wav_bytes("self-test", bytes.get_ref());
        let frames = imp.as_ref().map(|u| u.table.frames).unwrap_or(0);
        let rt = imp
            .map(|u| {
                let mut s = fileio::demo_song();
                w.user[1] = Some(u);
                w.tables[1] = gen_wave::USER;
                s.instruments.push(model::Instrument {
                    name: "Wave".into(),
                    volume: 0.8,
                    transpose: 0,
                    fx: vec![],
                    kind: model::InstrumentKind::Gen(gen::GenPatch {
                        engine: gen::GenEngine::Wave(w.clone()),
                        ..Default::default()
                    }),
                });
                let p = std::env::temp_dir().join("chipwave_selftest_wt.cwt");
                let ok = fileio::save_cwt(&s, &p)
                    .and_then(|_| fileio::load_cwt(&p))
                    .map(|b| {
                        b.instruments.last().map(|i| &i.kind)
                            == s.instruments.last().map(|i| &i.kind)
                    })
                    .unwrap_or(false);
                let _ = std::fs::remove_file(&p);
                ok
            })
            .unwrap_or(false);
        let good = tables == wavetable::BUILTIN.len()
            && y.iter().all(|x| x.is_finite())
            && fund > 0.05
            && alias < 1e-3
            && frames == 3
            && rt;
        report.push_str(&format!(
            "  wavetable: {tables} built-in tables, 3.1 kHz saw fundamental {fund:.3} alias {alias:.6}, import {frames} frames, .cwt round trip {} ({})\n",
            if rt { "ok" } else { "FAILED" },
            if good { "ok" } else { "FAILED" }
        ));
        good
    };
    // the Grid: every starter patch renders, a Grid FX device processes
    // audio, and both survive a .cwt round trip
    let grid_ok = {
        wavetable::init();
        let mut s = fileio::demo_song();
        let mut peaks = vec![];
        for (name, _, g, _, _) in grid::presets() {
            let p = gen::GenPatch {
                engine: gen::GenEngine::Grid(g),
                ..Default::default()
            };
            let y = gen::bake(&p, 220.0, 48000.0, 1.0);
            let pk = y.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            peaks.push((name, pk, y.iter().all(|x| x.is_finite())));
            s.instruments.push(model::Instrument {
                name: name.into(),
                volume: 0.8,
                transpose: 0,
                fx: vec![fx::Device::new(fx::FxKind::Grid)],
                kind: model::InstrumentKind::Gen(p),
            });
        }
        let p = std::env::temp_dir().join("chipwave_selftest_grid.cwt");
        let rt = fileio::save_cwt(&s, &p)
            .and_then(|_| fileio::load_cwt(&p))
            .map(|b| {
                b.instruments.len() == s.instruments.len()
                    && b.instruments
                        .iter()
                        .zip(&s.instruments)
                        .all(|(x, y)| x.kind == y.kind && x.fx == y.fx)
            })
            .unwrap_or(false);
        let _ = std::fs::remove_file(&p);
        let audible = peaks
            .iter()
            .all(|(_, pk, fin)| *fin && *pk > 0.05 && *pk < 2.0);
        let good = !peaks.is_empty() && audible && rt;
        report.push_str(&format!(
            "  grid: {} starter patches (peaks {}), + Grid FX devices, .cwt round trip {} ({})\n",
            peaks.len(),
            peaks
                .iter()
                .map(|(_, p, _)| format!("{p:.2}"))
                .collect::<Vec<_>>()
                .join(" "),
            if rt { "ok" } else { "FAILED" },
            if good { "ok" } else { "FAILED" }
        ));
        good
    };
    let ok = fill_ok
        && grid_ok
        && wt_ok
        && mod_ok
        && plug_ok
        && inst_ok
        && tf_ok
        && gen_ok
        && speech_ok
        && midi_ok
        && peak > 0.05
        && rms > 0.005
        && !nan
        && wrote.is_ok()
        && rt_ok
        && tempo_ok
        && auto_ok
        && timeline_ok
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

/// `--screenshot <out.png> [--view v] [--theme slate|dark|amiga] [--size WxH]`
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
            "usage: --screenshot <out.png> [--view {}] [--theme slate|dark|amiga] [--size 1600x900] [--zoom 0.9..1.5] [--text 0.9..1.5] [--song file.cwt|file.cwt.toml]",
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
    let theme_flag = flag("--theme").map(|t| t.to_ascii_lowercase());
    let amiga = theme_flag.as_deref().is_some_and(|t| t.starts_with('a'));
    let dark = theme_flag.as_deref().is_some_and(|t| t.starts_with('d'));
    // isolated settings / recovery folder, so the user's own settings are untouched
    let tmp = std::env::temp_dir().join(format!("chipwave-shot-{}", std::process::id()));
    std::env::set_var("XDG_CONFIG_HOME", &tmp);
    std::env::set_var("APPDATA", &tmp);
    // macOS keeps settings under $HOME/Library/Application Support
    #[cfg(target_os = "macos")]
    std::env::set_var("HOME", &tmp);
    let mut st = settings::Settings {
        theme: if amiga {
            theme::AMIGA.name
        } else if dark {
            theme::DARK.name
        } else {
            theme::SLATE.name
        }
        .into(),
        maximized: false,
        ..Default::default()
    };
    st.extra.update_check = false;
    // --zoom 1.5 (UI scale) / --text 1.2 (Text size)
    if let Some(z) = flag("--zoom").and_then(|s| s.parse::<f32>().ok()) {
        st.zoom = settings::snap_zoom(z);
    }
    if let Some(t) = flag("--text").and_then(|s| s.parse::<f32>().ok()) {
        st.extra.text_size = ((t * 20.0).round() * 5.0).clamp(90.0, 150.0) as u16;
    }
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
                midi::UiEv::Pressure { key, val } if key == midi::ALL_KEYS => {
                    println!("  channel pressure {:.0} %", val * 100.0)
                }
                midi::UiEv::Pressure { key, val } => {
                    println!("  key pressure key {key:3} = {:.0} %", val * 100.0)
                }
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
    // before --version: capture-thumb takes --version <plugin ver>
    if args.iter().any(|a| a == "--capture-thumb") {
        std::process::exit(plugin::thumb::cli_capture(&args));
    }
    if args.iter().any(|a| a == "--version") {
        println!("ChipWave Studio {}", crate::VERSION);
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--screenshot") {
        std::process::exit(screenshot(&args, i));
    }
    if args.iter().any(|a| a == "--check-updates") {
        // same check as Help > Check for updates (5 s timeout); exit 0 = ran
        println!("ChipWave Studio {}: checking for updates…", crate::VERSION);
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
    if let Some(i) = args.iter().position(|a| a == "--scan-plugin") {
        // child process of the plugin scanner: check one file, print JSON
        let Some(p) = args.get(i + 1) else {
            std::process::exit(2);
        };
        // a crashing plugin must not pop up an error dialog / debugger:
        // the parent sees the exit code and lists it as crashed
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Diagnostics::Debug::*;
            SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
        }
        let rep = plugin::scan::check_file(std::path::Path::new(p));
        println!("{}", serde_json::to_string(&rep).unwrap_or_default());
        std::process::exit(0);
    }
    if let Some(i) = args.iter().position(|a| a == "--install-showcase") {
        std::process::exit(app::showcase::cli_install(&args, i));
    }
    if let Some(i) = args.iter().position(|a| a == "--scan-plugins") {
        // scan every plugin folder (plus an optional extra one) and list instruments
        let mut dirs = plugin::scan::all_dirs(&settings::load().extra.plugin_dirs);
        if let Some(d) = args.get(i + 1).filter(|a| !a.starts_with("--")) {
            dirs.push(PathBuf::from(d));
        }
        let exe = std::env::current_exe().unwrap_or_default();
        let c = plugin::scan::scan(&exe, &dirs, &plugin::scan::cache(), true, &|k, n, f| {
            if !f.is_empty() {
                println!("[{}/{n}] {f}", k + 1);
            }
        });
        for e in &c.entries {
            println!("  {} {}: {} {}", e.format, e.status, e.path, e.message);
            for p in &e.plugins {
                println!(
                    "      {} — {} ({}){}",
                    p.desc.name,
                    p.desc.vendor,
                    p.desc.id,
                    if p.desc.is_instrument() {
                        " [instrument]"
                    } else {
                        ""
                    }
                );
            }
        }
        plugin::scan::set_cache(c);
        std::process::exit(0);
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
