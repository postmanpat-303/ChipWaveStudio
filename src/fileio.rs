//! .cwt save/load (ZIP: mimetype, manifest.toml, song.toml, blocks/NNN.toml, instruments/NN.wav)
//! and the first-run demo loader.
use crate::model::*;
use crate::presets;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;

const MAX_ENTRY: u64 = 256 * 1024 * 1024;

fn hex2(s: &str) -> Option<u8> {
    if s.chars().all(|c| c == '.') {
        return Some(0);
    }
    u8::from_str_radix(s, 16).ok()
}

pub fn cell_to_text(c: &Cell) -> String {
    let ins = if c.instr == 0 {
        "..".into()
    } else {
        format!("{:02X}", c.instr)
    };
    let vol = if c.vol == 0 {
        "..".into()
    } else {
        format!("{:02X}", c.vol - 1)
    };
    let fx = if c.fx == 0 && c.fxp == 0 {
        "....".into()
    } else {
        format!("{:02X}{:02X}", c.fx, c.fxp)
    };
    format!("{} {} {} {}", note_name(c.note), ins, vol, fx)
}

/// Accepts "C-3 01 0C40" (note ins cmd) or "C-3 01 40 0C40" (note ins vol cmd).
pub fn cell_from_text(t: &str) -> Cell {
    let parts: Vec<&str> = t.split_whitespace().collect();
    let mut c = Cell::default();
    if parts.is_empty() {
        return c;
    }
    c.note = parse_note(parts[0]).unwrap_or(0);
    if let Some(i) = parts.get(1) {
        c.instr = hex2(i).unwrap_or(0);
    }
    let (vol, fx) = match parts.len() {
        3 => (None, parts.get(2)),
        4 => (parts.get(2), parts.get(3)),
        _ => (None, None),
    };
    if let Some(v) = vol {
        if let Some(x) = hex2(v) {
            if !v.starts_with('.') {
                c.vol = x.min(0x40) + 1;
            }
        }
    }
    if let Some(f) = fx {
        if f.len() == 4 && !f.starts_with('.') {
            c.fx = hex2(&f[0..2]).unwrap_or(0);
            c.fxp = hex2(&f[2..4]).unwrap_or(0);
        }
    }
    c
}

pub fn rows_from_text(text: &str, channels: usize, rows: usize) -> Vec<Vec<Cell>> {
    let mut cells = vec![vec![Cell::default(); channels]; rows];
    for line in text.lines() {
        let mut it = line.split('|');
        let Some(r) = it
            .next()
            .and_then(|r| usize::from_str_radix(r.trim(), 16).ok())
        else {
            continue;
        };
        if r >= rows {
            continue;
        }
        for (ch, t) in it.enumerate().take(channels) {
            cells[r][ch] = cell_from_text(t);
        }
    }
    cells
}

pub fn rows_to_text(p: &Pattern) -> String {
    let mut s = String::new();
    for (r, row) in p.cells.iter().enumerate() {
        if row.iter().all(|c| c.is_empty()) {
            continue;
        }
        s.push_str(&format!("{:02X}", r));
        for c in row {
            s.push_str(" | ");
            s.push_str(&cell_to_text(c));
        }
        s.push('\n');
    }
    s
}

#[derive(Serialize, Deserialize)]
struct BlockFile {
    name: String,
    lines: usize,
    tracks: usize,
    rows: String,
}

#[derive(Serialize, Deserialize)]
struct SongFile {
    format: String,
    format_version: u32,
    title: String,
    bpm: f32,
    speed: u32,
    lines_per_beat: u32,
    channels: usize,
    channel_names: Vec<String>,
    order: Vec<usize>,
    instruments: Vec<Instrument>,
}

pub fn save_cwt(song: &Song, path: &Path) -> Result<(), String> {
    let tmp = path.with_extension("cwt.tmp");
    {
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(f);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let defl = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let e = |e: zip::result::ZipError| e.to_string();
        z.start_file("mimetype", stored).map_err(e)?;
        z.write_all(b"application/x-chipwave-song")
            .map_err(|e| e.to_string())?;
        z.start_file("manifest.toml", defl).map_err(e)?;
        z.write_all(format!("format = \"chipwave-song\"\nformat_version = 1\nmin_reader_version = 1\napp = \"ChipWave Tracker {}\"\n", env!("CARGO_PKG_VERSION")).as_bytes()).map_err(|e| e.to_string())?;
        let mut instruments = song.instruments.clone();
        for (i, inst) in instruments.iter_mut().enumerate() {
            if let InstrumentKind::Sample {
                file, data, rate, ..
            } = &mut inst.kind
            {
                *file = format!("{:02X}.wav", i + 1);
                z.start_file(format!("instruments/{}", file), stored)
                    .map_err(e)?;
                let mut buf = std::io::Cursor::new(Vec::new());
                {
                    let spec = hound::WavSpec {
                        channels: 1,
                        sample_rate: *rate,
                        bits_per_sample: 32,
                        sample_format: hound::SampleFormat::Float,
                    };
                    let mut w = hound::WavWriter::new(&mut buf, spec).map_err(|e| e.to_string())?;
                    for s in data.iter() {
                        w.write_sample(*s).map_err(|e| e.to_string())?;
                    }
                    w.finalize().map_err(|e| e.to_string())?;
                }
                z.write_all(buf.get_ref()).map_err(|e| e.to_string())?;
            }
        }
        let sf = SongFile {
            format: "chipwave-song".into(),
            format_version: 1,
            title: song.title.clone(),
            bpm: song.bpm,
            speed: song.speed,
            lines_per_beat: song.lines_per_beat,
            channels: song.channels,
            channel_names: song.channel_names.clone(),
            order: song.order.clone(),
            instruments,
        };
        z.start_file("song.toml", defl).map_err(e)?;
        z.write_all(toml::to_string(&sf).map_err(|e| e.to_string())?.as_bytes())
            .map_err(|e| e.to_string())?;
        for (i, p) in song.patterns.iter().enumerate() {
            let b = BlockFile {
                name: p.name.clone(),
                lines: p.rows,
                tracks: song.channels,
                rows: rows_to_text(p),
            };
            z.start_file(format!("blocks/{:03}.toml", i), defl)
                .map_err(e)?;
            z.write_all(toml::to_string(&b).map_err(|e| e.to_string())?.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        z.finish().map_err(e)?;
    }
    if path.exists() {
        let _ = std::fs::rename(path, path.with_extension("cwt.bak"));
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn read_entry(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>, String> {
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

pub fn load_cwt(path: &Path) -> Result<Song, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    if z.len() > 10_000 {
        return Err("too many entries".into());
    }
    let sf: SongFile = toml::from_str(&String::from_utf8_lossy(&read_entry(&mut z, "song.toml")?))
        .map_err(|e| e.to_string())?;
    let channels = sf.channels.clamp(1, MAX_CHANNELS);
    let mut patterns = Vec::new();
    for i in 0..1000 {
        let name = format!("blocks/{:03}.toml", i);
        let Ok(b) = read_entry(&mut z, &name) else {
            break;
        };
        let b: BlockFile =
            toml::from_str(&String::from_utf8_lossy(&b)).map_err(|e| e.to_string())?;
        let rows = b.lines.clamp(1, 3200);
        patterns.push(Pattern {
            name: b.name,
            rows,
            cells: rows_from_text(&b.rows, channels, rows),
        });
    }
    if patterns.is_empty() {
        patterns.push(Pattern::new("Block 00", ROWS, channels));
    }
    let mut instruments = sf.instruments;
    for inst in instruments.iter_mut() {
        if let InstrumentKind::Sample {
            file, data, rate, ..
        } = &mut inst.kind
        {
            if let Ok(bytes) = read_entry(&mut z, &format!("instruments/{}", file)) {
                if let Ok((d, r)) = decode_wav(std::io::Cursor::new(bytes)) {
                    *data = d;
                    *rate = r;
                }
            }
        }
    }
    let order: Vec<usize> = sf
        .order
        .into_iter()
        .filter(|&o| o < patterns.len())
        .collect();
    let mut names = sf.channel_names;
    names.resize(channels, "Ch".into());
    Ok(Song {
        title: sf.title,
        bpm: sf.bpm.clamp(20.0, 400.0),
        speed: sf.speed.clamp(1, 32),
        lines_per_beat: sf.lines_per_beat.max(1),
        channels,
        channel_names: names,
        patterns,
        order: if order.is_empty() { vec![0] } else { order },
        instruments,
    })
}

/// Decode any hound-readable WAV to mono f32.
pub fn decode_wav<R: std::io::Read>(r: R) -> Result<(Vec<f32>, u32), String> {
    let mut rd = hound::WavReader::new(r).map_err(|e| e.to_string())?;
    let spec = rd.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => rd.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            rd.samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 * scale)
                .collect()
        }
    };
    let mono = raw
        .chunks(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect();
    Ok((mono, spec.sample_rate))
}

pub fn import_wav(path: &Path) -> Result<Instrument, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let (data, rate) = decode_wav(std::io::BufReader::new(f))?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Sample".into());
    Ok(Instrument {
        name,
        volume: 1.0,
        kind: InstrumentKind::Sample {
            file: String::new(),
            rate,
            data,
            base_note: parse_note("C-4").unwrap(),
        },
    })
}

/// The first-run demo (demos/first_run.cwt.toml).
pub fn demo_song() -> Song {
    let text = include_str!("../demos/first_run.cwt.toml");
    let v: toml::Value = toml::from_str(text).expect("demo toml");
    let s = &v["song"];
    let channels = s.get("tracks").and_then(|x| x.as_integer()).unwrap_or(4) as usize;
    let mut song = Song::empty();
    song.title = s
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("Demo")
        .into();
    song.bpm = s.get("bpm").and_then(|x| x.as_integer()).unwrap_or(145) as f32;
    song.speed = s.get("speed").and_then(|x| x.as_integer()).unwrap_or(6) as u32;
    song.channels = channels;
    song.channel_names = v
        .get("mixer")
        .and_then(|m| m.get("channel"))
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .map(|c| {
                    c.get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("Ch")
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default();
    song.channel_names.resize(channels, "Ch".into());
    // the demo's mixer list is Kick, Hats, Bass but rows are kick | hats | bass: keep names in row order
    if let Some(insts) = v.get("instrument").and_then(|x| x.as_array()) {
        for i in insts {
            let name = i.get("name").and_then(|x| x.as_str()).unwrap_or("?");
            let patch = if let Some(p) = i.get("patch").and_then(|x| x.as_str()) {
                presets::factory(p.trim_start_matches("factory:"))
            } else {
                Some(presets::hat(name.to_lowercase().contains("open")))
            };
            let p = patch.unwrap_or_default();
            song.instruments.push(Instrument {
                name: name.into(),
                volume: 1.0,
                kind: InstrumentKind::Synth(p),
            });
        }
    }
    // extra factory instruments to play with
    for n in [
        "psytrance/acid-squelch",
        "psytrance/psy-pluck",
        "psytrance/supersaw-lead",
        "alien/flutter-warble",
    ] {
        if let Some(p) = presets::factory(n) {
            song.instruments.push(Instrument {
                name: p.name.clone(),
                volume: 0.8,
                kind: InstrumentKind::Synth(p),
            });
        }
    }
    song.patterns.clear();
    if let Some(blocks) = v.get("block").and_then(|x| x.as_array()) {
        for b in blocks {
            let name = b
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("Block")
                .to_string();
            let src_lines = b.get("lines").and_then(|x| x.as_integer()).unwrap_or(16) as usize;
            let rows_text = b.get("rows").and_then(|x| x.as_str()).unwrap_or("");
            let src = rows_from_text(rows_text, channels, src_lines);
            // expand to a 64-row pattern by repeating the 16-row groove
            let mut p = Pattern::new(&name, ROWS, channels);
            for r in 0..ROWS {
                p.cells[r] = src[r % src_lines].clone();
            }
            song.patterns.push(p);
        }
    }
    if song.patterns.is_empty() {
        song.patterns.push(Pattern::new("Block 00", ROWS, channels));
    }
    song.order = vec![0; 2];
    song
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cell_text_roundtrip() {
        let c = Cell {
            note: parse_note("E-1").unwrap(),
            instr: 2,
            vol: 0x21,
            fx: 0x0C,
            fxp: 0x20,
        };
        assert_eq!(cell_from_text(&cell_to_text(&c)), c);
    }
    #[test]
    fn demo_loads_and_saves() {
        let s = demo_song();
        assert!(s.instruments.len() >= 4);
        assert!(s.patterns[0].cells[0][0].note > 0);
        let dir = std::env::temp_dir().join("chipwave_test.cwt");
        save_cwt(&s, &dir).unwrap();
        let l = load_cwt(&dir).unwrap();
        assert_eq!(l.patterns[0].cells, s.patterns[0].cells);
        assert_eq!(l.instruments.len(), s.instruments.len());
    }
}
