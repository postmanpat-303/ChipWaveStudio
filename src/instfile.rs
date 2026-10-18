//! Instrument files (.cwi), the user instrument library and the factory list.
//!
//! A `.cwi` is a small ZIP like a `.cwt` song: `mimetype`, `instrument.toml`
//! (the instrument with its sound, FX chain, volume and transpose) and, for a
//! sample instrument, `sample.wav` (32-bit float). Loading never panics on a
//! broken file; it returns an error instead.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Cell, Instrument, InstrumentKind, Pattern, Song};

pub const EXT: &str = "cwi";
const FORMAT: &str = "chipwave-instrument";
const MAX_ENTRY: u64 = 256 * 1024 * 1024;
/// most library files listed (a huge folder must not freeze the UI)
pub const MAX_FILES: usize = 4000;

#[derive(Serialize, Deserialize)]
struct InstFile {
    format: String,
    format_version: u32,
    #[serde(default)]
    app: String,
    instrument: Instrument,
}

fn wav_bytes(data: &[f32], rate: u32) -> Result<Vec<u8>, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::new(&mut buf, spec).map_err(|e| e.to_string())?;
    for s in data {
        w.write_sample(*s).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

/// Write `inst` to `path` (via a temp file, so a failed save never leaves
/// half a file behind).
pub fn save_cwi(inst: &Instrument, path: &Path) -> Result<(), String> {
    let tmp = path.with_extension("cwi.tmp");
    let r = (|| {
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(f);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let defl = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let e = |e: zip::result::ZipError| e.to_string();
        z.start_file("mimetype", stored).map_err(e)?;
        z.write_all(b"application/x-chipwave-instrument")
            .map_err(|e| e.to_string())?;
        let mut i = inst.clone();
        if let InstrumentKind::Sample {
            file, data, rate, ..
        } = &mut i.kind
        {
            *file = "sample.wav".into();
            z.start_file("sample.wav", stored).map_err(e)?;
            let d: Vec<f32> = data.iter().copied().collect();
            z.write_all(&wav_bytes(&d, *rate)?)
                .map_err(|e| e.to_string())?;
        }
        let f = InstFile {
            format: FORMAT.into(),
            format_version: 1,
            app: format!("ChipWave Tracker {}", crate::VERSION),
            instrument: i,
        };
        z.start_file("instrument.toml", defl).map_err(e)?;
        z.write_all(toml::to_string(&f).map_err(|e| e.to_string())?.as_bytes())
            .map_err(|e| e.to_string())?;
        let f = z.finish().map_err(e)?;
        f.sync_all().map_err(|e| e.to_string())
    })();
    if let Err(e) = r {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn entry(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>, String> {
    let f = z.by_name(name).map_err(|e| format!("{name}: {e}"))?;
    if f.size() > MAX_ENTRY {
        return Err(format!("{name} too large"));
    }
    let mut v = Vec::new();
    f.take(MAX_ENTRY)
        .read_to_end(&mut v)
        .map_err(|e| e.to_string())?;
    Ok(v)
}

fn read_head(path: &Path) -> Result<(InstFile, zip::ZipArchive<std::fs::File>), String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("not a .cwi file ({e})"))?;
    let t = entry(&mut z, "instrument.toml")?;
    let f: InstFile = toml::from_str(&String::from_utf8_lossy(&t)).map_err(|e| e.to_string())?;
    if f.format != FORMAT {
        return Err(format!("not a ChipWave instrument (format {:?})", f.format));
    }
    if f.format_version > 1 {
        return Err(format!(
            "made by a newer ChipWave (instrument format {}); please update",
            f.format_version
        ));
    }
    Ok((f, z))
}

/// Read a `.cwi`, sample data included.
pub fn load_cwi(path: &Path) -> Result<Instrument, String> {
    let (f, mut z) = read_head(path)?;
    let mut inst = f.instrument;
    fix(&mut inst);
    if let InstrumentKind::Sample { data, rate, lp, .. } = &mut inst.kind {
        let b = entry(&mut z, "sample.wav")?;
        let (d, r) = crate::fileio::decode_wav(std::io::Cursor::new(b))?;
        *data = d.into();
        *rate = r;
        lp.fix(data.len());
    }
    Ok(inst)
}

/// clamp what a hand-edited file could get wrong
fn fix(inst: &mut Instrument) {
    inst.name = inst.name.chars().take(64).collect();
    inst.volume = if inst.volume.is_finite() {
        inst.volume.clamp(0.0, 1.0)
    } else {
        1.0
    };
    inst.transpose = inst.transpose.clamp(-48, 48);
    inst.fx.truncate(16);
    inst.fx.iter_mut().for_each(|d| d.fix());
    if let InstrumentKind::Voice(v) = &mut inst.kind {
        v.fix();
    }
}

/// short kind label
pub fn kind_name(k: &InstrumentKind) -> &'static str {
    match k {
        InstrumentKind::Synth(_) => "synth",
        InstrumentKind::Sample { .. } => "sample",
        InstrumentKind::Voice(_) => "voice",
    }
}

/// a file name for an instrument name (no path separators or odd characters)
pub fn file_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().trim_matches('.').to_string();
    format!("{}.{EXT}", if s.is_empty() { "Instrument" } else { &s })
}

/// a path in `dir` for `name` that does not exist yet ("Bass.cwi", "Bass 2.cwi", ...)
pub fn free_path(dir: &Path, name: &str) -> PathBuf {
    let base = file_name(name);
    let stem = base.trim_end_matches(&format!(".{EXT}")).to_string();
    let mut p = dir.join(&base);
    let mut k = 2;
    while p.exists() {
        p = dir.join(format!("{stem} {k}.{EXT}"));
        k += 1;
    }
    p
}

/// The user library folder: the setting, or `<config>/Instruments`.
pub fn library_dir(setting: &str) -> Option<PathBuf> {
    if !setting.trim().is_empty() {
        return Some(PathBuf::from(setting.trim()));
    }
    crate::settings::config_dir().map(|d| d.join("Instruments"))
}

/// One instrument offered by the browser.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    /// "synth" / "sample" / "voice"
    pub kind: &'static str,
    /// sub-folder (library) or preset group (factory)
    pub category: String,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// a .cwi in the user library
    File(PathBuf),
    /// FACTORY preset path ("psytrance/acid-squelch")
    Factory(String),
    /// built-in synth patch ("trance-kick")
    Builtin(String),
    /// Voice preset name
    Voice(String),
}

/// Built-in sounds: factory presets, built-in drum/synth patches, Voice presets.
pub fn factory_entries() -> Vec<Entry> {
    let mut v = vec![];
    for (path, _) in crate::presets::FACTORY {
        let (cat, name) = path.split_once('/').unwrap_or(("factory", path));
        v.push(Entry {
            name: name.replace('-', " "),
            kind: "synth",
            category: cat.into(),
            source: Source::Factory(path.to_string()),
        });
    }
    for b in [
        "init",
        "trance-kick",
        "hat-closed",
        "hat-open",
        "shaker",
        "clap",
        "riser",
    ] {
        v.push(Entry {
            name: b.replace('-', " "),
            kind: "synth",
            category: "built-in".into(),
            source: Source::Builtin(b.into()),
        });
    }
    for (name, _, _) in crate::voice::presets() {
        v.push(Entry {
            name: name.into(),
            kind: "voice",
            category: "voice".into(),
            source: Source::Voice(name.into()),
        });
    }
    // show the name the instrument gets in the song ("Punchy psy kick")
    for e in &mut v {
        if let Ok(i) = instantiate(e) {
            e.name = i.name;
        }
    }
    v
}

/// List the .cwi files in `dir` and its sub-folders (one level; the
/// sub-folder is the category). Unreadable files are skipped.
pub fn scan(dir: &Path) -> Vec<Entry> {
    let mut out = vec![];
    let visit = |d: &Path, cat: &str, out: &mut Vec<Entry>| {
        let Ok(rd) = std::fs::read_dir(d) else {
            return vec![];
        };
        let mut subs = vec![];
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for p in files {
            if out.len() >= MAX_FILES {
                break;
            }
            if p.is_dir() {
                subs.push(p);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXT)) {
                if let Ok((f, _)) = read_head(&p) {
                    out.push(Entry {
                        name: f.instrument.name.clone(),
                        kind: kind_name(&f.instrument.kind),
                        category: cat.into(),
                        source: Source::File(p),
                    });
                }
            }
        }
        subs
    };
    for sub in visit(dir, "", &mut out) {
        let cat = sub
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        visit(&sub, &cat, &mut out);
    }
    out
}

/// Make the instrument an entry stands for.
pub fn instantiate(e: &Entry) -> Result<Instrument, String> {
    let synth = |name: &str, p: crate::synth::Patch, fx| Instrument {
        name: name.into(),
        volume: 1.0,
        transpose: 0,
        fx,
        kind: InstrumentKind::Synth(p),
    };
    let title = |s: &str| {
        let mut c = s.chars();
        c.next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    };
    match &e.source {
        Source::File(p) => load_cwi(p),
        Source::Factory(f) => {
            let mut p = crate::presets::factory(f).ok_or("unknown factory preset")?;
            p.name = title(&e.name);
            Ok(synth(&p.name.clone(), p, crate::presets::factory_fx(f)))
        }
        Source::Builtin(b) => {
            let mut p = crate::presets::builtin(b).ok_or("unknown built-in patch")?;
            p.name = title(&e.name);
            Ok(synth(&p.name.clone(), p, vec![]))
        }
        Source::Voice(v) => {
            let mut vp = crate::voice::preset(v).ok_or("unknown voice preset")?;
            vp.base.name = v.clone();
            Ok(Instrument {
                name: v.clone(),
                volume: 1.0,
                transpose: 0,
                fx: vec![],
                kind: InstrumentKind::Voice(vp),
            })
        }
    }
}

/// search: every word must appear in the name, kind or category
pub fn matches(e: &Entry, query: &str) -> bool {
    let hay = format!("{} {} {}", e.name, e.kind, e.category).to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|w| hay.contains(w))
}

/// A one-channel song that plays `inst` for auditioning: a short phrase
/// (C-4 E-4 G-4 C-5), or two hits for a drum-like sound. Tempo and master
/// settings come from `base`.
pub fn audition_song(inst: &Instrument, base: &Song) -> Song {
    let mut s = Song::empty();
    s.bpm = base.bpm;
    s.speed = base.speed;
    s.lines_per_beat = base.lines_per_beat;
    s.mixer.limiter = base.mixer.limiter;
    s.channels = 1;
    s.channel_names = vec!["Preview".into()];
    s.instruments = vec![inst.clone()];
    let mut p = Pattern::new("Preview", 16, 1);
    let n = |note: u8| Cell {
        note,
        instr: 1,
        ..Default::default()
    };
    let drum = {
        let l = inst.name.to_lowercase();
        [
            "kick", "hat", "snare", "clap", "drum", "perc", "shaker", "tom", "rim",
        ]
        .iter()
        .any(|k| l.contains(k))
    };
    if drum {
        p.cells[0][0] = n(49);
        p.cells[4][0] = n(49);
    } else {
        for (r, note) in [(0, 49), (3, 53), (6, 56), (9, 61)] {
            p.cells[r][0] = n(note);
        }
    }
    p.cells[14][0].note = crate::model::NOTE_OFF;
    s.patterns = vec![p];
    s.order = vec![0];
    s.automation.clear();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("chipwave_cwi_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn every_kind_round_trips_through_a_cwi() {
        let d = tmpdir("rt");
        let demo = crate::fileio::demo_song();
        let mut insts: Vec<Instrument> = demo.instruments.iter().take(3).cloned().collect();
        insts.push(
            instantiate(
                &factory_entries()
                    .into_iter()
                    .find(|e| e.kind == "voice")
                    .unwrap(),
            )
            .unwrap(),
        );
        let wav: Vec<f32> = (0..2000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        insts.push(Instrument {
            name: "Saw/sample: odd*name".into(),
            volume: 0.7,
            transpose: -3,
            fx: vec![crate::fx::Device::new(crate::fx::FxKind::Delay)],
            kind: InstrumentKind::Sample {
                file: String::new(),
                rate: 22050,
                data: wav.clone().into(),
                base_note: 49,
                lp: Default::default(),
            },
        });
        for i in &insts {
            let p = free_path(&d, &i.name);
            assert!(!p.file_name().unwrap().to_string_lossy().contains('/'));
            save_cwi(i, &p).unwrap();
            let back = load_cwi(&p).unwrap();
            assert_eq!(back.name, i.name);
            assert_eq!(back.volume, i.volume);
            assert_eq!(back.transpose, i.transpose);
            assert_eq!(back.fx, i.fx);
            match (&back.kind, &i.kind) {
                (InstrumentKind::Synth(a), InstrumentKind::Synth(b)) => assert_eq!(a, b),
                (InstrumentKind::Voice(a), InstrumentKind::Voice(b)) => assert_eq!(a, b),
                (
                    InstrumentKind::Sample {
                        data: a, rate: ra, ..
                    },
                    InstrumentKind::Sample {
                        data: b, rate: rb, ..
                    },
                ) => {
                    assert_eq!(ra, rb);
                    assert_eq!(
                        a.iter().copied().collect::<Vec<_>>(),
                        b.iter().copied().collect::<Vec<_>>()
                    );
                }
                _ => panic!("kind changed"),
            }
        }
        // the library scan lists them all, sub-folders as categories
        std::fs::create_dir_all(d.join("Drums")).unwrap();
        save_cwi(&insts[0], &d.join("Drums").join("k.cwi")).unwrap();
        std::fs::write(d.join("junk.cwi"), b"not a zip").unwrap();
        let e = scan(&d);
        assert_eq!(e.len(), insts.len() + 1, "{e:?}");
        assert!(e.iter().any(|x| x.category == "Drums"));
        assert!(e.iter().any(|x| x.kind == "sample"));
        // free_path does not overwrite
        let p1 = free_path(&d, &insts[0].name);
        assert!(!p1.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn broken_or_foreign_files_are_errors_not_panics() {
        let d = tmpdir("bad");
        let good = d.join("g.cwi");
        save_cwi(&crate::fileio::demo_song().instruments[0], &good).unwrap();
        let bytes = std::fs::read(&good).unwrap();
        let bad = d.join("b.cwi");
        for cut in [0, 10, bytes.len() / 2, bytes.len() - 1] {
            std::fs::write(&bad, &bytes[..cut]).unwrap();
            assert!(load_cwi(&bad).is_err());
        }
        let mut seed = 0x1234_5678u64;
        for _ in 0..100 {
            let mut b = bytes.clone();
            for _ in 0..6 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let i = (seed as usize) % b.len();
                b[i] ^= (seed >> 32) as u8 | 1;
            }
            std::fs::write(&bad, &b).unwrap();
            let _ = load_cwi(&bad);
        }
        // a song is not an instrument
        let song = d.join("s.cwt");
        crate::fileio::save_cwt(&crate::fileio::demo_song(), &song).unwrap();
        assert!(load_cwi(&song).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn factory_entries_all_instantiate_search_and_audition() {
        let base = crate::fileio::demo_song();
        let all = factory_entries();
        assert!(all.len() >= 15);
        for e in &all {
            let i = instantiate(e).unwrap_or_else(|x| panic!("{e:?}: {x}"));
            let s = audition_song(&i, &base);
            let out = crate::engine::render_song(&s, 22050);
            let pk = out.iter().fold(0.0f32, |a, x| a.max(x.abs()));
            assert!(
                pk > 1e-3 && out.iter().all(|x| x.is_finite()),
                "{} silent",
                e.name
            );
        }
        assert!(all.iter().any(|e| matches(e, "acid")));
        assert!(all.iter().filter(|e| matches(e, "VOICE")).count() >= 6);
        assert!(all.iter().any(|e| matches(e, "kick built")));
        assert!(!all.iter().any(|e| matches(e, "zzzz")));
    }

    /// Builds the 0.2.3.9 features demo (scripts/make_features_0239.py):
    /// takes a drums-only text song and, with the real library and chord
    /// helper code, adds factory sounds, writes a progression as chords,
    /// bass and arp, saves a sound to a .cwi and swaps instruments in one
    /// block. Run: CW_F239_IN=.. CW_F239_OUT=.. cargo test -- --ignored features_0239
    #[test]
    #[ignore]
    fn features_0239_demo() {
        use crate::chords::{self, ArpPattern, BassRhythm, ChordKind, Step, Style, WriteOpts};
        let inp = std::env::var("CW_F239_IN").expect("CW_F239_IN");
        let out = std::env::var("CW_F239_OUT").expect("CW_F239_OUT");
        let mut s = crate::fileio::load_text_song(Path::new(&inp)).unwrap();
        let key = s
            .info
            .key
            .or_else(|| chords::detect(&s).first().map(|(k, _)| *k))
            .unwrap();
        let fac = factory_entries();
        let add = |s: &mut Song, q: &str, vol: f32| -> u8 {
            let e = fac
                .iter()
                .find(|e| matches(e, q))
                .unwrap_or_else(|| panic!("{q}"));
            let mut i = instantiate(e).unwrap();
            i.volume = vol;
            s.add_instrument(i).unwrap() as u8 + 1
        };
        let pad = add(&mut s, "trance gate pad", 0.55);
        let bass = add(&mut s, "rolling offbeat bass", 0.9);
        let arp = add(&mut s, "psy pluck", 0.5);
        let lead = add(&mut s, "supersaw lead", 0.4);
        let acid = add(&mut s, "acid squelch", 0.45);
        // a library round trip: the pad goes through a .cwi file
        let dir = std::env::temp_dir().join("cw-f239-lib");
        let _ = std::fs::create_dir_all(&dir);
        let f = free_path(&dir, "Gate Pad");
        save_cwi(&s.instruments[pad as usize - 1], &f).unwrap();
        s.instruments[pad as usize - 1] = load_cwi(&f).unwrap();
        let prog: Vec<Step> = [0usize, 5, 2, 6]
            .iter()
            .map(|&d| Step::new(d, ChordKind::Triad))
            .collect();
        let rpb = s.lines_per_beat.max(1) as usize;
        let w = |s: &mut Song, blk: usize, style: Style, channel: usize, instr: u8, octave: u8| {
            let n = chords::write(
                &mut s.patterns[blk],
                key,
                &prog,
                &WriteOpts {
                    style,
                    channel,
                    instr,
                    octave,
                    rows_per_beat: rpb,
                    start_row: 0,
                    fill: true,
                    vol: 0,
                    seed: blk as u64,
                },
            )
            .unwrap();
            assert!(n.notes > 0);
        };
        for b in 0..4 {
            w(&mut s, b, Style::Chords, 3, pad, 4);
            if b >= 1 {
                w(&mut s, b, Style::Bass(BassRhythm::Rolling), 2, bass, 4);
            }
            if b >= 2 {
                w(
                    &mut s,
                    b,
                    Style::Arp {
                        pattern: ArpPattern::UpDown,
                        rate: 1,
                        octaves: 2,
                    },
                    6,
                    arp,
                    5,
                );
            }
        }
        // block 3: "use this instead of" in this block only
        assert!(s.replace_instrument_uses(pad, lead, Some(3)) > 0);
        assert!(s.replace_instrument_uses(arp, acid, Some(3)) > 0);
        crate::fileio::save_cwt(&s, Path::new(&out)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        eprintln!("key {}", key.name());
    }
}
