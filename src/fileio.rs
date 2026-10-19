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
        if f.len() == 4 && f.is_ascii() && !f.starts_with('.') {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<crate::arrange::Role>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    derived: Option<crate::arrange::Derived>,
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
    #[serde(default)]
    info: SongInfo,
}

/// Write a .cwt straight to `path` and flush it to disk. No temp file or
/// backups here: `recovery::atomic_save` owns the tmp + rename + .bak scheme.
pub fn save_cwt(song: &Song, path: &Path) -> Result<(), String> {
    if song.patterns.len() > MAX_BLOCKS || song.automation.len() > MAX_LANES {
        return Err(format!(
            "too many blocks or lanes (max {MAX_BLOCKS} blocks, {MAX_LANES} lanes)"
        ));
    }
    {
        let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
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
        // version 2: timeline mode, or plugin effect devices (an older
        // reader can't parse `FxKind::Plugin`)
        // version 3: audio clips (an older reader can't parse `ClipKind::Audio`)
        let has_audio = song
            .timeline
            .as_deref()
            .is_some_and(|t| !t.audio.is_empty() || t.clips.iter().any(|c| c.audio().is_some()));
        let min_reader = if has_audio {
            3
        } else if song.timeline.is_some() || crate::plugin::fxrack::song_has_fx(song) {
            2
        } else {
            1
        };
        z.write_all(format!("format = \"chipwave-song\"\nformat_version = 1\nmin_reader_version = {min_reader}\napp = \"ChipWave Studio {}\"\n", crate::VERSION).as_bytes()).map_err(|e| e.to_string())?;
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
            info: song.info.clone(),
        };
        z.start_file("song.toml", defl).map_err(e)?;
        z.write_all(toml::to_string(&sf).map_err(|e| e.to_string())?.as_bytes())
            .map_err(|e| e.to_string())?;
        for (i, p) in song.patterns.iter().enumerate() {
            let b = BlockFile {
                name: p.name.clone(),
                lines: p.rows,
                tracks: song.channels,
                role: p.role,
                derived: p.derived.clone(),
                rows: rows_to_text(p),
            };
            z.start_file(format!("blocks/{:03}.toml", i), defl)
                .map_err(e)?;
            z.write_all(toml::to_string(&b).map_err(|e| e.to_string())?.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        z.start_file("mixer.toml", defl).map_err(e)?;
        z.write_all(
            toml::to_string(&song.mixer)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        for (i, lane) in song.automation.iter().enumerate() {
            z.start_file(format!("automation/{i:02}.toml"), defl)
                .map_err(e)?;
            z.write_all(toml::to_string(lane).map_err(|e| e.to_string())?.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        if !song.modulation.is_empty() {
            z.start_file("modulation.toml", defl).map_err(e)?;
            z.write_all(
                toml::to_string(&song.modulation)
                    .map_err(|e| e.to_string())?
                    .as_bytes(),
            )
            .map_err(|e| e.to_string())?;
        }
        if let Some(tl) = &song.timeline {
            z.start_file("timeline.toml", defl).map_err(e)?;
            z.write_all(crate::timeline::to_toml(tl)?.as_bytes())
                .map_err(|e| e.to_string())?;
            // embedded audio files: the original bytes (compressed already)
            for (i, a) in tl.audio.iter().enumerate() {
                if let (true, Some(b)) = (a.embed, &a.bytes) {
                    if b.len() as u64 > MAX_ENTRY {
                        return Err(format!(
                            "audio file '{}' is too large to embed: reference it instead",
                            a.name
                        ));
                    }
                    z.start_file(audio_entry(i, &a.ext), stored).map_err(e)?;
                    z.write_all(b).map_err(|e| e.to_string())?;
                }
            }
        }
        z.start_file("arranger.toml", defl).map_err(e)?;
        z.write_all(
            toml::to_string(&ArrangerFile {
                zoom: song.arranger_zoom,
            })
            .map_err(|e| e.to_string())?
            .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        let f = z.finish().map_err(e)?;
        // flush through the writable handle (FlushFileBuffers needs write access on Windows)
        f.sync_all().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// zip entry of embedded audio file `i`
fn audio_entry(i: usize, ext: &str) -> String {
    let ext: String = ext
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();
    format!("audio/{i:03}.{}", if ext.is_empty() { "bin" } else { &ext })
}

/// Decode the audio files of a loaded timeline: embedded ones through
/// `embedded(entry name)`, referenced ones from their path, else from the
/// song's folder (`base`) by file name. A file that can't be found or read
/// stays missing (its clips are silent, keep their place, and can be
/// relinked); each gives a warning.
pub fn load_audio_assets(
    tl: &mut crate::timeline::Timeline,
    mut embedded: impl FnMut(&str) -> Result<Vec<u8>, String>,
    base: Option<&Path>,
) -> Vec<String> {
    let mut warnings = vec![];
    for (i, a) in tl.audio.iter_mut().enumerate() {
        let got: Result<(Vec<u8>, bool), String> = if a.embed {
            embedded(&audio_entry(i, &a.ext)).map(|b| (b, true))
        } else {
            let p = Path::new(&a.path);
            let local = base.zip(p.file_name()).map(|(d, f)| d.join(f));
            match std::fs::read(p) {
                Ok(b) => Ok((b, false)),
                Err(e) => match local.as_deref().map(std::fs::read) {
                    Some(Ok(b)) => {
                        a.path = local.unwrap().to_string_lossy().to_string();
                        Ok((b, false))
                    }
                    _ => Err(if a.path.is_empty() {
                        "no file".to_string()
                    } else {
                        format!("{}: {e}", a.path)
                    }),
                },
            }
        };
        match got.and_then(|(b, keep)| {
            crate::audiofile::decode(&b, &a.ext).map(|p| (p, keep.then_some(b)))
        }) {
            Ok((pcm, bytes)) => a.attach(pcm, bytes),
            Err(e) => warnings.push(format!(
                "audio file '{}' is missing ({e}): its clips are silent until you locate it (right-click the clip)",
                a.name
            )),
        }
    }
    warnings
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

/// keep loaded modulation within the engine's limits and the song's
/// instruments (bad entries are dropped)
pub fn sanitize_modulation(
    mut m: crate::modulation::Modulation,
    ninst: usize,
) -> crate::modulation::Modulation {
    use crate::modulation::{MAX_MODS, MAX_ROUTES};
    m.mods.retain(|x| x.owner < ninst);
    m.mods.truncate(MAX_MODS);
    let mut seen = vec![];
    m.mods.retain(|x| {
        let fresh = !seen.contains(&x.id);
        seen.push(x.id);
        fresh
    });
    for x in &mut m.mods {
        for v in [&mut x.rate, &mut x.sync, &mut x.a, &mut x.d, &mut x.r] {
            if !v.is_finite() || *v < 0.0 {
                *v = 0.0;
            }
        }
        x.rate = x.rate.min(100.0);
        x.sync = x.sync.min(64.0);
        x.phase = if x.phase.is_finite() {
            x.phase.clamp(0.0, 1.0)
        } else {
            0.0
        };
        x.s = if x.s.is_finite() {
            x.s.clamp(0.0, 1.0)
        } else {
            0.5
        };
        x.smooth = if x.smooth.is_finite() {
            x.smooth.clamp(0.0, 1.0)
        } else {
            0.0
        };
        x.value = if x.value.is_finite() {
            x.value.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }
    let ids: Vec<u32> = m.mods.iter().map(|x| x.id).collect();
    m.routes
        .retain(|r| ids.contains(&r.src) && crate::automation::Target::parse(&r.target).is_some());
    for r in &mut m.routes {
        r.amount = if r.amount.is_finite() {
            r.amount.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
    m.routes.truncate(MAX_ROUTES);
    m
}

/// Newest `min_reader_version` this build can read. Tracker-mode songs write
/// 1 (byte-identical to older versions); timeline-mode songs (docs/TIMELINE.md)
/// write 2 plus `timeline.toml`, so builds older than the timeline refuse
/// them instead of playing a stale order list. Songs with plugin effect
/// devices also write 2; songs with audio clips (M4) write 3.
pub const READER_VERSION: u32 = 3;

/// Start of the error for a song that needs a newer app (see [`is_too_new`]).
const TOO_NEW: &str = "This song was made with a newer version of ChipWave Studio";

/// The message for a file that needs reader version `need`.
pub fn too_new_message(need: i64) -> String {
    format!(
        "{TOO_NEW} (it needs file reader version {need}; ChipWave Studio {} reads up to version {READER_VERSION}). Please update ChipWave Studio (Help > Check for updates) to open it.",
        crate::VERSION
    )
}

/// the open failed because the song needs a newer ChipWave Studio
pub fn is_too_new(err: &str) -> bool {
    err.starts_with(TOO_NEW)
}

/// Refuse a song whose `min_reader_version` is newer than this build reads.
/// A missing or unreadable value counts as 1 (every file written so far).
fn check_reader_version(v: Option<&toml::Value>) -> Result<(), String> {
    match v.and_then(|x| x.as_integer()) {
        Some(n) if n > READER_VERSION as i64 => Err(too_new_message(n)),
        _ => Ok(()),
    }
}

/// Most blocks / automation lanes a song may have (save and load agree).
pub const MAX_BLOCKS: usize = 10_000;
pub const MAX_LANES: usize = 4096;

pub fn load_cwt(path: &Path) -> Result<Song, String> {
    load_cwt_warn(path).map(|(s, _)| s)
}

/// Load a song; the second value lists parts that could not be loaded
/// (shown to the user instead of silently dropping data).
pub fn load_cwt_warn(path: &Path) -> Result<(Song, Vec<String>), String> {
    let mut warnings: Vec<String> = Vec::new();
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    if z.len() > 10_000 {
        return Err("too many entries".into());
    }
    // manifest first: a song from a newer app must not half-load
    if let Ok(m) = read_entry(&mut z, "manifest.toml") {
        let m = toml::from_str::<toml::Value>(&String::from_utf8_lossy(&m)).ok();
        check_reader_version(m.as_ref().and_then(|m| m.get("min_reader_version")))?;
    }
    let sf: SongFile = toml::from_str(&String::from_utf8_lossy(&read_entry(&mut z, "song.toml")?))
        .map_err(|e| e.to_string())?;
    let channels = sf.channels.clamp(1, MAX_CHANNELS);
    let mut patterns = Vec::new();
    for i in 0..=MAX_BLOCKS {
        let name = format!("blocks/{:03}.toml", i);
        if i == MAX_BLOCKS {
            if read_entry(&mut z, &name).is_ok() {
                warnings.push(format!(
                    "more than {MAX_BLOCKS} blocks: the rest were skipped"
                ));
            }
            break;
        }
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
            role: b.role,
            derived: b.derived,
            cache: vec![],
        });
    }
    if patterns.is_empty() {
        patterns.push(Pattern::new("Block 00", ROWS, channels));
    }
    let mut instruments = sf.instruments;
    for inst in instruments.iter_mut() {
        inst.fx.truncate(16);
        inst.fx.iter_mut().for_each(|d| d.fix());
    }
    for inst in instruments.iter_mut() {
        if let InstrumentKind::Sample {
            file, data, rate, ..
        } = &mut inst.kind
        {
            match read_entry(&mut z, &format!("instruments/{}", file))
                .and_then(|b| decode_wav(std::io::Cursor::new(b)))
            {
                Ok((d, r)) => {
                    *data = d.into();
                    *rate = r;
                }
                Err(e) => warnings.push(format!(
                    "sample \"{}\" could not be loaded ({e})",
                    inst.name
                )),
            }
        }
        if let InstrumentKind::Sample { data, lp, .. } = &mut inst.kind {
            lp.fix(data.len());
        }
        if let InstrumentKind::Voice(v) = &mut inst.kind {
            v.fix();
        }
    }
    let order: Vec<usize> = sf
        .order
        .into_iter()
        .filter(|&o| o < patterns.len())
        .collect();
    let mut automation = vec![];
    for i in 0..=MAX_LANES {
        let Ok(b) = read_entry(&mut z, &format!("automation/{i:02}.toml")) else {
            break;
        };
        if i == MAX_LANES {
            warnings.push(format!(
                "more than {MAX_LANES} automation lanes: the rest were skipped"
            ));
            break;
        }
        let parsed = toml::from_str::<crate::automation::Lane>(&String::from_utf8_lossy(&b));
        if parsed.is_err() {
            warnings.push(format!("automation lane {} could not be read", i + 1));
        }
        if let Ok(mut lane) = parsed {
            lane.points.truncate(100_000);
            for p in &mut lane.points {
                p.value = p.value.clamp(0.0, 1.0);
                p.pos = p.pos.max(0.0);
            }
            lane.sort();
            automation.push(lane);
        }
    }
    let mut names = sf.channel_names;
    names.resize(channels, "Ch".into());
    let mut song = Song {
        title: sf.title,
        bpm: sf.bpm.clamp(20.0, 400.0),
        speed: sf.speed.clamp(1, 32),
        lines_per_beat: sf.lines_per_beat.clamp(1, 32),
        channels,
        channel_names: names,
        patterns,
        order: if order.is_empty() { vec![0] } else { order },
        instruments,
        automation,
        info: sf.info,
        mixer: read_entry(&mut z, "mixer.toml")
            .ok()
            .and_then(|b| toml::from_str::<Mixer>(&String::from_utf8_lossy(&b)).ok())
            .map(|mut m| {
                m.fix();
                m
            })
            .unwrap_or_default(),
        arranger_zoom: read_entry(&mut z, "arranger.toml")
            .ok()
            .and_then(|b| toml::from_str::<ArrangerFile>(&String::from_utf8_lossy(&b)).ok())
            .map(|a| a.zoom.clamp(0.25, 16.0))
            .unwrap_or(2.0),
        timeline: None,
        modulation: Default::default(),
    };
    // drag-to-modulate (docs/MODULATION.md)
    if let Ok(b) = read_entry(&mut z, "modulation.toml") {
        match toml::from_str::<crate::modulation::Modulation>(&String::from_utf8_lossy(&b)) {
            Ok(m) => song.modulation = sanitize_modulation(m, song.instruments.len()),
            Err(e) => warnings.push(format!(
                "modulation.toml could not be read ({e}): the song opens without modulation"
            )),
        }
    }
    // timeline mode (docs/TIMELINE.md)
    if let Ok(b) = read_entry(&mut z, "timeline.toml") {
        match crate::timeline::from_toml(&String::from_utf8_lossy(&b)) {
            Ok(mut tl) => {
                warnings.extend(load_audio_assets(
                    &mut tl,
                    |name| read_entry(&mut z, name),
                    path.parent(),
                ));
                warnings.extend(tl.fix(&song));
                song.timeline = Some(Box::new(tl));
            }
            Err(e) => warnings.push(format!(
                "timeline.toml could not be read ({e}): the song opens in tracker mode"
            )),
        }
    }
    // hand-written lanes may name FX params by key ("instrument.2.fx.1.feedback")
    for k in 0..song.automation.len() {
        let id = song.automation[k].target.clone();
        if crate::automation::Target::parse(&id).is_none() {
            if let Some(t) = crate::automation::Target::parse_in(&id, &song) {
                song.automation[k].target = t.id();
            }
        }
    }
    // plugin instruments that are not installed here: placeholder + warning
    warnings.extend(crate::plugin::missing_warnings(&song));
    // plugin effects that are not installed here: bypassed, state kept
    warnings.extend(crate::plugin::fxrack::missing_warnings(&song));
    Ok((song, warnings))
}

#[derive(Serialize, Deserialize)]
struct ArrangerFile {
    zoom: f32,
}

/// Decode any hound-readable WAV to mono f32.
pub fn decode_wav<R: std::io::Read>(r: R) -> Result<(Vec<f32>, u32), String> {
    let mut rd = hound::WavReader::new(r).map_err(|e| e.to_string())?;
    let spec = rd.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => rd.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
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
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Sample {
            file: String::new(),
            rate,
            data: data.into(),
            base_note: parse_note("C-4").unwrap(),
            lp: Default::default(),
        },
    })
}

/// a single-file text song (`.toml`, e.g. demos/dark_psy.cwt.toml)?
pub fn is_text_song(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
}

/// load a single-file text song from disk (File > Open of a .cwt.toml)
pub fn load_text_song(path: &Path) -> Result<Song, String> {
    let t = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut song = song_from_text(&t)?;
    // audio clips of a text song reference files next to it
    if let Some(tl) = song.timeline.as_deref_mut() {
        if !tl.audio.is_empty() {
            for w in load_audio_assets(
                tl,
                |_| Err("a text song can't embed audio".into()),
                path.parent(),
            ) {
                eprintln!("{}: {w}", path.display());
            }
        }
    }
    Ok(song)
}

/// The first-run demo (demos/first_run.cwt.toml, generated by
/// scripts/make_demo.py). Also File > New from demo.
pub fn demo_song() -> Song {
    demo_song_n(0)
}

/// Built-in demo songs (File > Demo songs): (menu name, description, text song).
pub const DEMOS: &[(&str, &str, &str)] = &[
    (
        "Lumen Drift",
        "uplifting trance, 138 BPM (the first-run demo)",
        include_str!("../demos/first_run.cwt.toml"),
    ),
    (
        "Hollow Ritual",
        "dark psytrance, E Phrygian, 147 BPM",
        include_str!("../demos/dark_psy.cwt.toml"),
    ),
    (
        "Velvet Mirrorball",
        "disco, F minor, 104 BPM (voice, phaser strings, filter sweeps)",
        include_str!("../demos/disco.cwt.toml"),
    ),
];

/// demo `i` of `DEMOS` (falls back to the first)
pub fn demo_song_n(i: usize) -> Song {
    let (_, _, text) = DEMOS.get(i).unwrap_or(&DEMOS[0]);
    song_from_text(text).expect("demo toml")
}

fn num_of(v: &toml::Value) -> Option<f32> {
    v.as_float()
        .or(v.as_integer().map(|i| i as f64))
        .map(|f| f as f32)
}

/// set every numeric key of `t` (except `kind`) as a real param value
fn apply_fx_params(d: &mut crate::fx::Device, t: &toml::Value) {
    if let Some(tab) = t.as_table() {
        for (k, v) in tab {
            if let (false, Some(x)) = (k == "kind", num_of(v)) {
                d.set_real(k, x);
            }
        }
    }
}

fn wave_of(s: &str) -> Option<crate::synth::Wave> {
    use crate::synth::Wave;
    Some(match s.to_ascii_lowercase().as_str() {
        "saw" => Wave::Saw,
        "square" => Wave::Square,
        "pulse" => Wave::Pulse,
        "triangle" | "tri" => Wave::Triangle,
        "sine" => Wave::Sine,
        _ => return None,
    })
}

/// optional synth parameter overrides on a text-song instrument
fn patch_overrides(p: &mut crate::synth::Patch, t: &toml::Value) {
    let r = |k: &str| t.get(k).and_then(num_of);
    let set = |k: &str, f: &mut f32| {
        if let Some(v) = r(k) {
            *f = v;
        }
    };
    set("cutoff", &mut p.cutoff);
    set("resonance", &mut p.resonance);
    set("env_amount", &mut p.env_amount);
    set("key_track", &mut p.key_track);
    set("attack", &mut p.amp.a);
    set("decay", &mut p.amp.d);
    set("sustain", &mut p.amp.s);
    set("release", &mut p.amp.r);
    set("filter_attack", &mut p.fenv.a);
    set("filter_decay", &mut p.fenv.d);
    set("filter_sustain", &mut p.fenv.s);
    set("detune", &mut p.detune_cents);
    set("width", &mut p.width);
    set("drive", &mut p.drive);
    set("gain", &mut p.gain);
    set("osc1_level", &mut p.osc1_level);
    set("sub", &mut p.sub_level);
    set("noise", &mut p.noise_level);
    set("metal", &mut p.metal);
    set("vel_cutoff", &mut p.vel_cutoff);
    if let Some(ms) = r("start_delay_ms") {
        p.start_delay = (ms / 1000.0).clamp(0.0, 0.05);
    }
    if let Some(ms) = r("glide_ms") {
        p.glide = (ms / 1000.0).clamp(0.001, 2.0);
    }
    if let Some(b) = t.get("legato").and_then(|x| x.as_bool()) {
        p.legato = b;
    }
    set("accent", &mut p.accent);
    set("accent_vel", &mut p.accent_vel);
    set("pulse_width", &mut p.pulse_width);
    if let Some(v) = r("unison") {
        p.unison = (v as u32).clamp(1, 8);
    }
    if let Some(v) = r("octave") {
        p.osc1_oct = v as i32;
    }
    if let Some(w) = t.get("wave").and_then(|x| x.as_str()).and_then(wave_of) {
        p.osc1 = w;
    }
    if let Some(o) = t.get("osc2") {
        if o.as_bool() == Some(false) {
            p.osc2 = None;
        } else {
            let w = o.get("wave").and_then(|x| x.as_str()).and_then(wave_of);
            let semi = o.get("semi").and_then(num_of).unwrap_or(0.0) as i32;
            let lvl = o.get("level").and_then(num_of).unwrap_or(0.5);
            p.osc2 = Some((w.unwrap_or(crate::synth::Wave::Saw), semi, lvl));
        }
    }
    if let Some(pe) = t.get("pitch_env").and_then(|x| x.as_array()) {
        if let (Some(a), Some(b)) = (pe.first().and_then(num_of), pe.get(1).and_then(num_of)) {
            p.pitch_env = (a, b);
        }
    }
    if let Some(l) = t.get("lfo") {
        let n = |k: &str| l.get(k).and_then(num_of);
        if let Some(w) = l.get("shape").and_then(|x| x.as_str()).and_then(wave_of) {
            p.lfo.shape = w;
        }
        p.lfo.rate = n("rate").unwrap_or(p.lfo.rate);
        p.lfo.sync = n("sync").unwrap_or(p.lfo.sync);
        p.lfo.cutoff = n("cutoff").unwrap_or(p.lfo.cutoff);
        p.lfo.amp = n("amp").unwrap_or(p.lfo.amp);
        p.lfo.pitch = n("pitch").unwrap_or(p.lfo.pitch);
    }
    // filter_model = "svf" | "ladder" | "vintage"; slope = 12 | 24 (ladder)
    if let Some(m) = t.get("filter_model").and_then(|x| x.as_str()) {
        use crate::ladder::FilterModel;
        p.filter_model = match m.to_ascii_lowercase().as_str() {
            "ladder" | "moog" => FilterModel::Ladder,
            "vintage" => FilterModel::Vintage,
            _ => FilterModel::Svf,
        };
    }
    if let Some(v) = r("slope") {
        p.ladder_12db = v < 18.0;
    }
    if let Some(f) = t.get("filter").and_then(|x| x.as_str()) {
        use crate::synth::FilterMode;
        p.filter = match f.to_ascii_lowercase().as_str() {
            "hp" => FilterMode::Hp,
            "bp" => FilterMode::Bp,
            _ => FilterMode::Lp,
        };
    }
}

/// block role by name ("drop" = Chorus, "build" = Bridge)
fn role_of(r: &str) -> Option<crate::arrange::Role> {
    use crate::arrange::{Role, ROLES};
    match r.to_ascii_lowercase().as_str() {
        "drop" => Some(Role::Chorus),
        "build" => Some(Role::Bridge),
        l => ROLES
            .into_iter()
            .find(|x| format!("{x:?}").to_ascii_lowercase() == l),
    }
}

/// overlay `o` on `m`; tables merge key by key (so `[instrument.voice.transform]`
/// changes only the keys it names)
fn merge_toml(m: &mut toml::Value, o: &toml::Value) {
    match (m.as_table_mut(), o.as_table()) {
        (Some(mt), Some(ot)) => {
            for (k, v) in ot {
                match mt.get_mut(k) {
                    Some(dst) if dst.is_table() && v.is_table() => merge_toml(dst, v),
                    _ => {
                        mt.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        _ => *m = o.clone(),
    }
}

fn fx_kind(name: &str) -> Option<crate::fx::FxKind> {
    crate::fx::FxKind::ALL.into_iter().find(|k| {
        format!("{k:?}").eq_ignore_ascii_case(name) || k.name().eq_ignore_ascii_case(name)
    })
}

/// The single-file text song form used by the demo (docs/FIRST_RUN.md).
pub fn song_from_text(text: &str) -> Result<Song, String> {
    use crate::automation::{Lane, Point, Shape, Target};
    let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
    check_reader_version(
        v.get("min_reader_version")
            .or_else(|| v.get("song").and_then(|s| s.get("min_reader_version"))),
    )?;
    let s = &v["song"];
    let int = |t: &toml::Value, k: &str, d: i64| t.get(k).and_then(|x| x.as_integer()).unwrap_or(d);
    let real = |t: &toml::Value, k: &str| t.get(k).and_then(num_of);
    let channels = (int(s, "tracks", 4) as usize).clamp(1, MAX_CHANNELS);
    let mut song = Song::empty();
    song.title = s
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("Demo")
        .into();
    song.bpm = real(s, "bpm").unwrap_or(145.0);
    song.speed = int(s, "speed", 6) as u32;
    song.channels = channels;
    if let Some(k) = s.get("key").and_then(|x| x.as_str()) {
        song.info.key =
            Some(crate::chords::parse_key(k).ok_or_else(|| format!("unknown key {k:?}"))?);
    }
    // instruments
    song.instruments.clear();
    for i in v
        .get("instrument")
        .and_then(|x| x.as_array())
        .into_iter()
        .flatten()
    {
        let name = i.get("name").and_then(|x| x.as_str()).unwrap_or("?");
        let src = i.get("patch").and_then(|x| x.as_str()).unwrap_or("");
        // patch = "voice:<preset>": a Voice (glitch) instrument; an optional
        // [instrument.voice] table overrides its fields
        if let Some(vname) = src.strip_prefix("voice:") {
            let mut vp = crate::voice::preset(vname)
                .ok_or_else(|| format!("instrument {name}: unknown voice preset {vname:?}"))?;
            if let Some(t) = i.get("voice") {
                let mut merged = toml::Value::try_from(&vp).map_err(|e| e.to_string())?;
                merge_toml(&mut merged, t);
                vp = merged
                    .try_into()
                    .map_err(|e| format!("instrument {name}: voice: {e}"))?;
            }
            vp.base.name = name.into();
            patch_overrides(&mut vp.base, i);
            vp.fix();
            let mut fx = vec![];
            for f in i.get("fx").and_then(|x| x.as_array()).into_iter().flatten() {
                let kind = f
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .and_then(fx_kind)
                    .ok_or_else(|| format!("instrument {name}: unknown fx kind"))?;
                let mut d = crate::fx::Device::new(kind);
                apply_fx_params(&mut d, f);
                fx.push(d);
            }
            song.instruments.push(Instrument {
                name: name.into(),
                volume: real(i, "volume").unwrap_or(1.0),
                transpose: int(i, "transpose", 0) as i8,
                fx,
                kind: InstrumentKind::Voice(vp),
            });
            continue;
        }
        // patch = "gen:<preset>": a Generator (drum synth / kit); an optional
        // [instrument.gen] table overrides its fields (deep merge)
        if let Some(gname) = src.strip_prefix("gen:") {
            let mut gp = crate::gen::preset(gname)
                .ok_or_else(|| format!("instrument {name}: unknown generator preset {gname:?}"))?;
            if let Some(t) = i.get("gen") {
                let mut merged = toml::Value::try_from(&gp).map_err(|e| e.to_string())?;
                merge_toml(&mut merged, t);
                gp = merged
                    .try_into()
                    .map_err(|e| format!("instrument {name}: gen: {e}"))?;
            }
            gp.base.name = name.into();
            patch_overrides(&mut gp.base, i);
            gp.fix();
            let mut fx = vec![];
            for f in i.get("fx").and_then(|x| x.as_array()).into_iter().flatten() {
                let kind = f
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .and_then(fx_kind)
                    .ok_or_else(|| format!("instrument {name}: unknown fx kind"))?;
                let mut d = crate::fx::Device::new(kind);
                apply_fx_params(&mut d, f);
                fx.push(d);
            }
            song.instruments.push(Instrument {
                name: name.into(),
                volume: real(i, "volume").unwrap_or(1.0),
                transpose: int(i, "transpose", 0) as i8,
                fx,
                kind: InstrumentKind::Gen(gp),
            });
            continue;
        }
        // patch = "clap:<plugin id>": a CLAP plugin instrument; optional
        // `plugin_path`, `state` (base64) and `macros` = [param ids]
        if let Some(pid) = src.strip_prefix("clap:") {
            let mut fx = vec![];
            for f in i.get("fx").and_then(|x| x.as_array()).into_iter().flatten() {
                let kind = f
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .and_then(fx_kind)
                    .ok_or_else(|| format!("instrument {name}: unknown fx kind"))?;
                let mut d = crate::fx::Device::new(kind);
                apply_fx_params(&mut d, f);
                fx.push(d);
            }
            let st = |k: &str| i.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let pp = crate::plugin::PluginPatch {
                format: "clap".into(),
                id: pid.into(),
                name: name.into(),
                vendor: String::new(),
                path: st("plugin_path"),
                state: st("state"),
                params: vec![],
                macros: i
                    .get("macros")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .map(|v| {
                                v.as_integer()
                                    .map(|n| n as u32)
                                    .unwrap_or(crate::plugin::NO_PARAM)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            song.instruments.push(Instrument {
                name: name.into(),
                volume: real(i, "volume").unwrap_or(1.0),
                transpose: int(i, "transpose", 0) as i8,
                fx,
                kind: InstrumentKind::Plugin(pp),
            });
            continue;
        }
        let (patch, mut fx) = if let Some(p) = src.strip_prefix("factory:") {
            (presets::factory(p), presets::factory_fx(p))
        } else if let Some(b) = src.strip_prefix("builtin:") {
            (presets::builtin(b), vec![])
        } else {
            (None, vec![])
        };
        let mut patch = patch.ok_or_else(|| format!("instrument {name}: unknown patch {src:?}"))?;
        patch.name = name.into();
        patch_overrides(&mut patch, i);
        if i.get("factory_fx").and_then(|x| x.as_bool()) == Some(false) {
            fx.clear();
        }
        for f in i.get("fx").and_then(|x| x.as_array()).into_iter().flatten() {
            let kind = f
                .get("kind")
                .and_then(|x| x.as_str())
                .and_then(fx_kind)
                .ok_or_else(|| format!("instrument {name}: unknown fx kind"))?;
            let mut d = crate::fx::Device::new(kind);
            apply_fx_params(&mut d, f);
            fx.push(d);
        }
        song.instruments.push(Instrument {
            name: name.into(),
            volume: real(i, "volume").unwrap_or(1.0),
            transpose: int(i, "transpose", 0) as i8,
            fx,
            kind: InstrumentKind::Synth(patch),
        });
    }
    // mixer
    let mix = v.get("mixer");
    let chans = mix
        .and_then(|m| m.get("channel"))
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    song.channel_names = (0..channels)
        .map(|k| {
            chans
                .get(k)
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("Ch")
                .to_string()
        })
        .collect();
    let gain = |db: f32| 10f32.powf(db / 20.0).clamp(0.0, 2.0);
    for (k, c) in chans.iter().enumerate().take(channels) {
        let m = &mut song.mixer.channels[k];
        if let Some(db) = real(c, "volume_db") {
            m.volume = gain(db);
        }
        m.pan = real(c, "pan").unwrap_or(0.0).clamp(-1.0, 1.0);
        m.send = [
            real(c, "send_a").unwrap_or(0.0).clamp(0.0, 1.0),
            real(c, "send_b").unwrap_or(0.0).clamp(0.0, 1.0),
        ];
        m.duck = real(c, "duck").unwrap_or(0.0).clamp(0.0, 1.0);
    }
    if let Some(m) = mix {
        if let Some(db) = real(m, "master_db") {
            song.mixer.master = gain(db);
        }
        if let Some(k) = m.get("duck_key").and_then(|x| x.as_integer()) {
            song.mixer.duck_key = (k.max(1) as usize - 1).min(channels - 1);
        }
        if let Some(l) = m.get("limiter") {
            if let Ok(mut ls) = l.clone().try_into::<crate::master::LimiterSettings>() {
                ls.fix();
                song.mixer.limiter = ls;
            }
        }
        if let Some(ms) = real(m, "duck_release_ms") {
            song.mixer.duck_release = (ms / 1000.0).clamp(0.02, 1.0);
        }
        for (k, key) in ["return_a", "return_b"].iter().enumerate() {
            if let (Some(r), Some(ret)) = (m.get(*key), song.mixer.returns.get_mut(k)) {
                apply_fx_params(&mut ret.device, r);
                if let Some(l) = real(r, "level") {
                    ret.level = l;
                }
            }
        }
    }
    // blocks
    song.patterns.clear();
    for b in v
        .get("block")
        .and_then(|x| x.as_array())
        .into_iter()
        .flatten()
    {
        let name = b.get("name").and_then(|x| x.as_str()).unwrap_or("Block");
        let lines = (int(b, "lines", ROWS as i64) as usize).clamp(1, 256);
        let rows_text = b.get("rows").and_then(|x| x.as_str()).unwrap_or("");
        let src = rows_from_text(rows_text, channels, lines);
        let mut p = Pattern::new(name, lines, channels);
        p.cells = src;
        p.role = b.get("role").and_then(|x| x.as_str()).and_then(role_of);
        song.patterns.push(p);
    }
    if song.patterns.is_empty() {
        song.patterns.push(Pattern::new("Block 00", ROWS, channels));
    }
    song.order = v
        .get("sequence")
        .and_then(|q| q.get("sections"))
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .flat_map(|sec| {
                    let b = int(sec, "block", 0) as usize;
                    let n = int(sec, "repeat", 1).max(1) as usize;
                    std::iter::repeat_n(b, n)
                })
                .filter(|&b| b < song.patterns.len())
                .collect()
        })
        .unwrap_or_default();
    if song.order.is_empty() {
        song.order = vec![0];
    }
    // automation (positions in lines from the song start; values in real units)
    for a in v
        .get("automation")
        .and_then(|x| x.as_array())
        .into_iter()
        .flatten()
    {
        let id = a.get("target").and_then(|x| x.as_str()).unwrap_or("");
        let Some(target) = Target::parse_in(id, &song) else {
            return Err(format!("automation: unknown target {id:?}"));
        };
        let fxinfo = match target {
            Target::Fx(i, d, p) => song.instruments[i]
                .fx
                .get(d)
                .and_then(|dev| crate::fx::params(dev.kind).get(p)),
            Target::Ret(r, p) => Some(
                &crate::fx::params(if r == 0 {
                    crate::fx::FxKind::Delay
                } else {
                    crate::fx::FxKind::Reverb
                })[p],
            ),
            _ => None,
        };
        let shape_of = |s: Option<&str>| match s {
            Some("step") => Shape::Step,
            Some("smooth") | Some("bezier") => Shape::Smooth,
            _ => Shape::Linear,
        };
        let deflt = a.get("curve").and_then(|x| x.as_str());
        let label = a.get("label").and_then(|x| x.as_str()).unwrap_or(id);
        let mut lane = Lane::new(target, label.to_string());
        for p in a
            .get("points")
            .and_then(|x| x.as_array())
            .into_iter()
            .flatten()
        {
            let pos = real(p, "pos").unwrap_or(0.0);
            let val = real(p, "value").unwrap_or(0.0);
            let value = match fxinfo {
                Some(info) => info.to_norm(val),
                None => target.to_norm(val),
            };
            let shape = shape_of(p.get("shape").and_then(|x| x.as_str()).or(deflt));
            lane.points.push(Point { pos, value, shape });
        }
        lane.sort();
        song.automation.push(lane);
    }
    // optional [timeline] table: the song opens in timeline mode
    if let Some(t) = v.get("timeline") {
        let text = toml::to_string(t).map_err(|e| e.to_string())?;
        let mut tl = crate::timeline::from_toml(&text)?;
        tl.fix(&song);
        song.timeline = Some(Box::new(tl));
    }
    // optional [modulation] table: [[modulation.modulator]] / [[modulation.route]]
    if let Some(m) = v.get("modulation") {
        let text = toml::to_string(m).map_err(|e| e.to_string())?;
        let m: crate::modulation::Modulation =
            toml::from_str(&text).map_err(|e| format!("modulation: {e}"))?;
        song.modulation = sanitize_modulation(m, song.instruments.len());
    }
    Ok(song)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// the extra dark psy demo parses, and File > Open's text path loads it
    #[test]
    fn dark_psy_demo_loads() {
        let s = song_from_text(include_str!("../demos/dark_psy.cwt.toml")).expect("dark psy toml");
        assert!(s.title.starts_with("Hollow Ritual"));
        assert_eq!(s.bpm, 147.0);
        assert_eq!(s.channels, 8);
        assert_eq!(s.mixer.duck_key, 0);
        assert!(s.mixer.channels[1].duck > 0.5, "bass ducked off the kick");
        assert!(s.automation.len() >= 8);
        // ~3:30: 34 blocks of 64 lines at 147 BPM
        let secs = s.order.len() as f32 * 64.0 * 60.0 / (147.0 * 4.0);
        assert!((200.0..240.0).contains(&secs), "{secs}");
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("demos/dark_psy.cwt.toml");
        assert!(is_text_song(&p) && !is_text_song(std::path::Path::new("a.cwt")));
        assert_eq!(load_text_song(&p).unwrap().order, s.order);
    }
    #[test]
    fn disco_demo_loads_from_menu() {
        let i = DEMOS
            .iter()
            .position(|d| d.0 == "Velvet Mirrorball")
            .expect("disco demo registered in File > Demo songs");
        let s = demo_song_n(i);
        assert!(s.title.starts_with("Velvet Mirrorball"));
        assert_eq!(s.bpm, 104.0);
        assert_eq!(s.channels, 15);
        assert_eq!(s.mixer.duck_key, 0);
        assert!(s.mixer.limiter.enabled && s.mixer.limiter.ceiling_db <= -1.0);
        assert!(s
            .instruments
            .iter()
            .any(|i| matches!(i.kind, InstrumentKind::Voice(_))));
        assert!(s.automation.len() >= 8);
        // ~3:25: 22 blocks of 64 lines at 104 BPM
        let secs = s.order.len() as f32 * 64.0 * 60.0 / (104.0 * 4.0);
        assert!((180.0..240.0).contains(&secs), "{secs}");
    }
    /// arbitrary (non-ASCII, garbage) text must never panic the parsers
    #[test]
    fn cell_parsers_fuzz_no_panic() {
        let alphabet: Vec<char> = "0123456789ABCDEFabcdef.-#=^| \t\nCHIPWAVEéß€😀ÿ\u{0}\u{301}"
            .chars()
            .collect();
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..20_000 {
            let len = (next() % 24) as usize;
            let s: String = (0..len)
                .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
                .collect();
            let _ = cell_from_text(&s);
            let _ = rows_from_text(&s, 4, 8);
            let _ = crate::input::clip_from_text(&format!("{}\n{s}", crate::input::CLIP_HEADER));
        }
        // the case found in review: 4 bytes, char boundary inside a multibyte char
        let _ = cell_from_text("C-3 01 40 aé1");
    }

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

    /// copy of the .cwt at `src` with manifest.toml replaced (None = removed)
    fn with_manifest(src: &Path, dst: &Path, manifest: Option<&str>) {
        let mut a = zip::ZipArchive::new(std::fs::File::open(src).unwrap()).unwrap();
        let mut w = zip::ZipWriter::new(std::fs::File::create(dst).unwrap());
        for i in 0..a.len() {
            let f = a.by_index(i).unwrap();
            if f.name().unwrap() == "manifest.toml" {
                drop(f);
                if let Some(m) = manifest {
                    w.start_file("manifest.toml", zip::write::SimpleFileOptions::default())
                        .unwrap();
                    w.write_all(m.as_bytes()).unwrap();
                }
            } else {
                w.raw_copy_file(f).unwrap();
            }
        }
        w.finish().unwrap();
    }

    #[test]
    fn songs_from_a_newer_reader_version_are_refused_with_a_clear_message() {
        let dir = std::env::temp_dir().join(format!("cw_reader_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("ok.cwt");
        save_cwt(&demo_song(), &src).unwrap();
        // what we write today is readable
        let m = {
            let mut z = zip::ZipArchive::new(std::fs::File::open(&src).unwrap()).unwrap();
            String::from_utf8(read_entry(&mut z, "manifest.toml").unwrap()).unwrap()
        };
        assert!(m.contains("min_reader_version = 1"), "{m}");
        assert!(load_cwt(&src).is_ok());
        let newer = dir.join("newer.cwt");
        with_manifest(
            &src,
            &newer,
            Some("format = \"chipwave-song\"\nformat_version = 4\nmin_reader_version = 4\n"),
        );
        let e = load_cwt(&newer).unwrap_err();
        assert!(is_too_new(&e), "{e}");
        assert!(
            e.contains("reader version 4") && e.contains("Check for updates"),
            "{e}"
        );
        // a missing or odd manifest is treated as version 1
        let none = dir.join("none.cwt");
        with_manifest(&src, &none, None);
        assert!(load_cwt(&none).is_ok());
        with_manifest(&src, &none, Some("min_reader_version = \"x\"\n"));
        assert!(load_cwt(&none).is_ok());
        // text songs: the same rule
        let t = include_str!("../demos/first_run.cwt.toml");
        assert!(song_from_text(t).is_ok());
        let e = song_from_text(&format!("min_reader_version = 4\n{t}")).unwrap_err();
        assert!(is_too_new(&e), "{e}");
        assert!(!is_too_new("zip error"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// timeline-mode songs save timeline.toml and need reader version 2;
    /// tracker songs still write reader version 1 and no timeline.toml
    #[test]
    fn timeline_mode_round_trips_and_needs_reader_version_2() {
        let dir = std::env::temp_dir().join(format!("cw_tlsave_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let entries = |p: &std::path::Path| {
            let mut z = zip::ZipArchive::new(std::fs::File::open(p).unwrap()).unwrap();
            let m = String::from_utf8(read_entry(&mut z, "manifest.toml").unwrap()).unwrap();
            let names: Vec<String> = z
                .file_names()
                .filter_map(|n| n.ok())
                .map(|n| n.to_string())
                .collect();
            (m, names)
        };
        let plain = dir.join("plain.cwt");
        save_cwt(&demo_song(), &plain).unwrap();
        let (m, names) = entries(&plain);
        assert!(m.contains("min_reader_version = 1"), "{m}");
        assert!(!names.iter().any(|n| n == "timeline.toml"), "{names:?}");
        let mut song = demo_song();
        crate::timeline::enter_timeline_mode(&mut song);
        {
            let t = song.timeline.as_mut().unwrap();
            let id = t.clips[0].id;
            crate::timeline::split(t, id, t.clips[0].start + crate::timeline::ROW * 4);
            t.markers.push(crate::timeline::Marker {
                tick: 960 * 8,
                name: "Hook".into(),
                ..Default::default()
            });
            t.loop_range = Some((0, 960 * 16));
            t.loop_on = true;
            t.tracks[1].speed = 3;
        }
        let tlp = dir.join("tl.cwt");
        save_cwt(&song, &tlp).unwrap();
        let (m, names) = entries(&tlp);
        assert!(m.contains("min_reader_version = 2"), "{m}");
        assert!(names.iter().any(|n| n == "timeline.toml"), "{names:?}");
        let back = load_cwt(&tlp).unwrap();
        let (a, b) = (
            song.timeline.as_ref().unwrap(),
            back.timeline.as_ref().unwrap(),
        );
        assert!(
            a.same_content(b),
            "timeline changed on the way through the file"
        );
        assert!(b.markers.iter().any(|m| m.name == "Hook"));
        assert!(b.loop_on && b.tracks[1].speed == 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// the demo is a real arrangement: 1:30-2:00, every block has a role,
    /// intro / build / drop / breakdown / outro all present, 8 tracks, in tempo
    #[test]
    fn demo_song_arrangement() {
        use crate::arrange::Role;
        let s = demo_song();
        let secs = s.length_seconds();
        assert!((120.0..=150.0).contains(&secs), "{secs} s");
        assert_eq!((s.bpm, s.channels), (140.0, 8));
        assert!(s.patterns.iter().all(|p| p.role.is_some()));
        for r in [
            Role::Intro,
            Role::Verse,
            Role::Bridge,
            Role::Chorus,
            Role::Breakdown,
            Role::Outro,
        ] {
            assert!(s.patterns.iter().any(|p| p.role == Some(r)), "{r:?}");
        }
        // every played block makes sound, and every note uses an existing instrument
        for &b in &s.order {
            let p = &s.patterns[b];
            assert!(p
                .cells
                .iter()
                .flatten()
                .any(|c| c.note > 0 && c.note != NOTE_OFF));
            for c in p.cells.iter().flatten() {
                assert!(c.instr as usize <= s.instruments.len());
            }
        }
        assert!(s.instruments.len() >= 11);
        assert_eq!(s.instruments[0].name, "Trance Kick");
        assert_eq!(s.instruments[1].name, "Gallop Bass");
        // sidechain: the kick (channel 1) keys the ducking of bass, arp and pads
        assert_eq!(s.mixer.duck_key, 0);
        assert!(s.mixer.channels[0].duck == 0.0);
        assert!(s.mixer.channels[1].duck > 0.3 && s.mixer.channels[5].duck > 0.9);
        // hats (channel 3): classic offbeat hat only, one hit on the "and" of
        // every beat (rows 2, 6, 10, 14 of each bar), in every block; on the grid
        for p in &s.patterns {
            for (i, r) in p.cells.iter().enumerate() {
                let c = &r[2];
                if c.note > 0 && c.note != crate::model::NOTE_OFF {
                    assert_eq!(i % 4, 2, "{}: hat-channel note on row {i}", p.name);
                    assert_eq!((c.instr, c.fx, c.fxp), (3, 0x08, 0x14), "{}", p.name);
                }
            }
        }
        let drop = s.patterns.iter().find(|p| p.name == "Drop A").unwrap();
        let hats = drop.cells.iter().filter(|r| r[2].note > 0).count();
        assert_eq!(hats, 16, "4 per bar");
        let InstrumentKind::Synth(hat) = &s.instruments[2].kind else {
            panic!("hat is a synth")
        };
        assert!(hat.metal > 0.0 && hat.start_delay == 0.0);
    }

    #[test]
    fn automation_demo_lanes_and_save_load() {
        use crate::automation::{Shape, Target};
        let s = demo_song();
        assert!(s.automation.len() >= 6, "{}", s.automation.len());
        // lane 1: pad filter cutoff, real Hz values, smooth
        let c = &s.automation[0];
        assert_eq!(Target::parse(&c.target), Some(Target::Cutoff(6)));
        assert!((Target::Cutoff(6).to_real(c.points[0].value) - 350.0).abs() < 0.5);
        assert_eq!(c.points[0].shape, Shape::Smooth);
        // arp cutoff opens through the pre-break (rising sweep into the drop)
        let arp = s
            .automation
            .iter()
            .find(|l| Target::parse(&l.target) == Some(Target::Cutoff(5)))
            .expect("arp cutoff lane");
        let hz = |pos: f32| {
            Target::Cutoff(5).to_real(crate::automation::value_at(&arp.points, pos).unwrap())
        };
        assert!(hz(2.0 * 64.0 + 62.0) > 2.0 * hz(2.0 * 64.0));
        // the pad breathes with a tempo-synced LFO and is spread in stereo
        let InstrumentKind::Synth(pad) = &s.instruments[6].kind else {
            panic!("pad is a synth")
        };
        assert!(pad.lfo.active() && pad.lfo.sync > 0.0 && pad.width > 0.9);
        assert_eq!(s.instruments[6].fx[0].kind, crate::fx::FxKind::Filter);
        let fb = s
            .automation
            .iter()
            .find(|l| l.target == "mixer.return_a.feedback")
            .expect("delay feedback lane");
        let info = &crate::fx::params(crate::fx::FxKind::Delay)[2];
        assert!((info.to_real(fb.points[0].value) - 0.3).abs() < 1e-3);
        let f = std::env::temp_dir().join("chipwave_auto_test.cwt");
        save_cwt(&s, &f).unwrap();
        let l = load_cwt(&f).unwrap();
        assert_eq!(l.automation, s.automation);
        // FX chains and return buses survive save/load
        for (a, b) in l.instruments.iter().zip(&s.instruments) {
            assert_eq!(a.fx, b.fx);
        }
        assert!(s.instruments.iter().filter(|i| !i.fx.is_empty()).count() >= 3);
        assert_eq!(l.mixer.returns, s.mixer.returns);
    }

    #[test]
    fn sample_loop_and_edits_saved_in_cwt() {
        use crate::sample::{LoopMode, SampleLoop};
        let mut s = demo_song();
        // a decaying plucked tone with a loop in the sustain part
        let data: Vec<f32> = (0..22050)
            .map(|i| {
                let t = i as f32 / 22050.0;
                ((t * 220.0 * std::f32::consts::TAU).sin() * 0.7
                    + (t * 663.0 * std::f32::consts::TAU).sin() * 0.2)
                    * (-t * 2.5).exp()
            })
            .collect();
        s.instruments.push(Instrument {
            name: "Pluck sample".into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Sample {
                file: String::new(),
                rate: 22050,
                data: data.into(),
                base_note: parse_note("A-3").unwrap(),
                lp: SampleLoop {
                    mode: LoopMode::PingPong,
                    start: 6000,
                    end: 15000,
                    finetune: -12,
                },
            },
        });
        let f = std::env::temp_dir().join("chipwave_sample_test.cwt");
        save_cwt(&s, &f).unwrap();
        let l = load_cwt(&f).unwrap();
        let (
            InstrumentKind::Sample {
                data: a,
                lp: la,
                base_note: ba,
                ..
            },
            InstrumentKind::Sample {
                data: b,
                lp: lb,
                base_note: bb,
                ..
            },
        ) = (
            &l.instruments.last().unwrap().kind,
            &s.instruments.last().unwrap().kind,
        )
        else {
            panic!()
        };
        assert_eq!(a, b);
        assert_eq!(la, lb);
        assert_eq!(ba, bb);
        // handy for screenshots: CHIPWAVE_SAMPLE_DEMO=<path> writes this song
        if let Some(p) = std::env::var_os("CHIPWAVE_SAMPLE_DEMO") {
            save_cwt(&s, std::path::Path::new(&p)).unwrap();
        }
    }

    #[test]
    fn corrupt_files_error_not_panic() {
        let dir = std::env::temp_dir().join(format!("chipwave_corrupt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.cwt");
        save_cwt(&demo_song(), &good).unwrap();
        let bytes = std::fs::read(&good).unwrap();
        let bad = dir.join("bad.cwt");
        // truncations and byte flips: must return Ok or Err, never panic
        for cut in [0, 10, 100, bytes.len() / 2, bytes.len() - 1] {
            std::fs::write(&bad, &bytes[..cut]).unwrap();
            let _ = load_cwt_warn(&bad);
        }
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..200 {
            let mut b = bytes.clone();
            for _ in 0..8 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let i = (seed as usize) % b.len();
                b[i] ^= (seed >> 32) as u8 | 1;
            }
            std::fs::write(&bad, &b).unwrap();
            let _ = load_cwt_warn(&bad);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_sample_gives_load_warning() {
        let dir = std::env::temp_dir().join(format!("chipwave_warn_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("w.cwt");
        let mut s = demo_song();
        s.instruments.push(Instrument {
            name: "Broken".into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Sample {
                file: String::new(),
                data: vec![0.1; 100].into(),
                rate: 44100,
                base_note: 48,
                lp: Default::default(),
            },
        });
        save_cwt(&s, &f).unwrap();
        // rewrite the zip without the sample entry
        let src = std::fs::File::open(&f).unwrap();
        let mut za = zip::ZipArchive::new(src).unwrap();
        let out = dir.join("w2.cwt");
        let mut zw = zip::ZipWriter::new(std::fs::File::create(&out).unwrap());
        for i in 0..za.len() {
            let e = za.by_index(i).unwrap();
            if e.name().unwrap().starts_with("instruments/") {
                continue;
            }
            zw.raw_copy_file(e).unwrap();
        }
        zw.finish().unwrap();
        let (l, w) = load_cwt_warn(&out).unwrap();
        assert_eq!(l.instruments.len(), s.instruments.len());
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("Broken"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn voice_instrument_round_trips_through_a_cwt() {
        let mut s = demo_song();
        for (name, _, p) in crate::voice::presets() {
            s.instruments.push(Instrument {
                name: name.into(),
                volume: 0.7,
                transpose: -12,
                fx: vec![crate::fx::Device::new(crate::fx::FxKind::Delay)],
                kind: InstrumentKind::Voice(p),
            });
        }
        let f = std::env::temp_dir().join(format!("chipwave_voice_{}.cwt", std::process::id()));
        save_cwt(&s, &f).unwrap();
        let back = load_cwt(&f).unwrap();
        let _ = std::fs::remove_file(&f);
        assert_eq!(back.instruments, s.instruments);
    }

    /// a song saved by 0.2.3.5 (before Voice existed) still loads unchanged
    #[test]
    fn song_saved_by_0_2_3_5_still_loads() {
        let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/chipwave-0.2.3.5-song.cwt");
        let s = load_cwt(&f).expect("old song");
        assert_eq!(s.title, "Saved by 0.2.3.5");
        let demo = demo_song();
        assert_eq!(s.instruments, demo.instruments);
        assert_eq!(s.order, demo.order);
        assert!(s
            .instruments
            .iter()
            .all(|i| !matches!(i.kind, InstrumentKind::Voice(_))));
    }

    /// a song saved by 0.2.3.6 (Hollow Ritual + a Voice) loads in 0.2.3.7:
    /// new limiter defaults, reverb cuts appended, patches get glide/accent
    /// defaults, and it renders under the ceiling
    #[test]
    fn song_saved_by_0_2_3_6_still_loads() {
        let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/chipwave-0.2.3.6-song.cwt");
        let mut s = load_cwt(&f).expect("old song");
        assert_eq!(s.title, "Saved by 0.2.3.6");
        let demo = demo_song_n(1);
        assert_eq!(
            s.instruments[..demo.instruments.len()],
            demo.instruments[..]
        );
        assert!(matches!(
            s.instruments.last().unwrap().kind,
            InstrumentKind::Voice(_)
        ));
        assert_eq!(s.mixer.limiter, crate::master::LimiterSettings::default());
        let rv = &s.mixer.returns[1].device;
        assert_eq!(rv.params.len(), 7);
        assert!((rv.real(5) - 200.0).abs() < 0.5);
        let p = s.instruments[0].kind.patch().unwrap();
        assert!(!p.legato && p.accent_vel == 0.0 && (p.glide - 0.06).abs() < 1e-6);
        s.order.truncate(1);
        let out = crate::engine::render_song(&s, 44100);
        assert!(out.iter().all(|x| x.is_finite()));
        let tp = crate::master::lin_to_db(crate::master::true_peak(&out));
        assert!(tp <= -1.0 + 1e-3, "{tp}");
    }

    #[test]
    fn text_song_limiter_and_acid_keys() {
        let t = r#"
[song]
title = "acid"
bpm = 138
[[instrument]]
name = "303"
patch = "builtin:init"
glide_ms = 80
legato = true
accent = 0.9
accent_vel = 0.75
[mixer]
[mixer.limiter]
gain_db = 6.0
ceiling_db = -0.5
"#;
        let s = song_from_text(t).unwrap();
        let p = s.instruments[0].kind.patch().unwrap();
        assert!((p.glide - 0.08).abs() < 1e-6 && p.legato && p.accent == 0.9);
        assert_eq!(p.accent_vel, 0.75);
        assert_eq!(s.mixer.limiter.gain_db, 6.0);
        assert_eq!(s.mixer.limiter.ceiling_db, -0.5);
        assert!(s.mixer.limiter.enabled);
    }

    #[test]
    fn text_song_voice_instrument_with_overrides() {
        let t = r#"
[song]
title = "v"
bpm = 147
[[instrument]]
name = "Ritual voice"
patch = "voice:Demon Ahh"
cutoff = 3000
[instrument.voice]
vowels = ["Er", "A"]
shift = 0.9
"#;
        let s = song_from_text(t).unwrap();
        let InstrumentKind::Voice(v) = &s.instruments[0].kind else {
            panic!("not a voice")
        };
        assert_eq!(
            v.vowels,
            vec![crate::voice::Vowel::Er, crate::voice::Vowel::A]
        );
        assert_eq!(v.shift, 0.9);
        assert_eq!(v.base.cutoff, 3000.0);
        assert_eq!(v.register, crate::voice::Register::Demon);
        assert!(song_from_text(&t.replace("Demon Ahh", "Nope")).is_err());
    }

    #[test]
    fn text_song_voice_transform_overrides_merge_into_the_preset() {
        let t = r#"
[song]
title = "tf"
[[instrument]]
name = "Beast"
patch = "voice:Sub Beast"
[instrument.voice.transform]
blend = 0.5
[instrument.voice.transform.sub]
octave = 2
[[instrument]]
name = "Dry"
patch = "voice:Demon Ahh"
[[instrument.fx]]
kind = "Metal resonator"
mix = 0.7
"#;
        let s = song_from_text(t).unwrap();
        let InstrumentKind::Voice(v) = &s.instruments[0].kind else {
            panic!("not a voice")
        };
        let pre = crate::voice::preset("Sub Beast").unwrap().transform;
        assert_eq!(v.transform.blend, 0.5);
        assert_eq!(v.transform.sub.octave, 2);
        // everything else stays the preset's
        assert_eq!(v.transform.sub.formant_drop, pre.sub.formant_drop);
        assert_eq!(v.transform.sub.mode, pre.sub.mode);
        assert_eq!(v.transform.dig, pre.dig);
        let InstrumentKind::Voice(d) = &s.instruments[1].kind else {
            panic!("not a voice")
        };
        assert_eq!(d.transform, crate::transform::Transform::default());
        assert_eq!(s.instruments[1].fx[0].kind, crate::fx::FxKind::Resonator);
    }

    #[test]
    fn text_song_stage2_generators() {
        let t = r#"
[song]
title = "g2"
[[instrument]]
name = "Bass"
patch = "gen:Psy Rolling Bass"
[instrument.gen.engine.Analog]
sync = true
level = 0.9
[[instrument]]
name = "Bell"
patch = "gen:Metallic Bell"
[[instrument]]
name = "Riser"
patch = "gen:White Riser"
"#;
        let s = song_from_text(t).unwrap();
        let kind = |i: usize| match &s.instruments[i].kind {
            InstrumentKind::Gen(g) => g.engine.clone(),
            _ => panic!("not a generator"),
        };
        let crate::gen::GenEngine::Analog(a) = kind(0) else {
            panic!()
        };
        let crate::gen::GenEngine::Analog(pre) =
            crate::gen::preset("Psy Rolling Bass").unwrap().engine
        else {
            panic!()
        };
        assert!(a.sync && a.level == 0.9 && a.p == pre.p && a.waves == pre.waves);
        assert!(matches!(kind(1), crate::gen::GenEngine::Fm(_)));
        assert!(matches!(kind(2), crate::gen::GenEngine::Noise(_)));
        let dir = std::env::temp_dir().join("cw_g2_rt.cwt");
        save_cwt(&s, &dir).unwrap();
        let back = load_cwt(&dir).unwrap();
        for i in 0..3 {
            assert_eq!(back.instruments[i].kind, s.instruments[i].kind);
        }
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn text_song_chip_fx() {
        let t = r#"
[song]
title = "fx"
[[instrument]]
name = "Coin"
patch = "gen:Coin"
[instrument.gen.engine.ChipFx]
wave = "Triangle"
level = 0.5
"#;
        let s = song_from_text(t).unwrap();
        let InstrumentKind::Gen(g) = &s.instruments[0].kind else {
            panic!("not a generator")
        };
        let crate::gen::GenEngine::ChipFx(x) = &g.engine else {
            panic!("not chip fx")
        };
        let crate::gen::GenEngine::ChipFx(pre) = crate::gen::preset("Coin").unwrap().engine else {
            panic!()
        };
        assert_eq!(x.wave, crate::chipfx::Wave::Triangle);
        assert_eq!(x.level, 0.5);
        assert_eq!(x.p, pre.p, "the other fields come from the preset");
        // .cwt round trip
        let dir = std::env::temp_dir().join("cw_chipfx_rt.cwt");
        save_cwt(&s, &dir).unwrap();
        let back = load_cwt(&dir).unwrap();
        assert_eq!(back.instruments[0].kind, s.instruments[0].kind);
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn text_song_generators_and_ladder_filters() {
        let t = r#"
[song]
title = "gen"
[[instrument]]
name = "Zap"
patch = "gen:Psy Zap Kick"
[instrument.gen.engine.Drum]
model = 0
level = 0.7
[[instrument]]
name = "Kit"
patch = "gen:Psy Kit"
filter_model = "vintage"
cutoff = 3000
[instrument.gen]
filter_on = true
[[instrument]]
name = "Bass"
patch = "builtin:init"
filter_model = "ladder"
slope = 12
"#;
        let s = song_from_text(t).unwrap();
        let InstrumentKind::Gen(g) = &s.instruments[0].kind else {
            panic!("not a generator")
        };
        let crate::gen::GenEngine::Drum(d) = &g.engine else {
            panic!("not a drum")
        };
        let pre = crate::gen::preset("Psy Zap Kick").unwrap();
        let crate::gen::GenEngine::Drum(pd) = &pre.engine else {
            panic!()
        };
        assert_eq!((d.model, d.level), (0, 0.7));
        assert_eq!(d.macros, pd.macros, "the rest stays the preset's");
        let InstrumentKind::Gen(k) = &s.instruments[1].kind else {
            panic!()
        };
        assert!(k.filter_on && matches!(k.engine, crate::gen::GenEngine::Kit(_)));
        assert_eq!(k.base.filter_model, crate::ladder::FilterModel::Vintage);
        assert_eq!(k.base.cutoff, 3000.0);
        let InstrumentKind::Synth(p) = &s.instruments[2].kind else {
            panic!()
        };
        assert_eq!(p.filter_model, crate::ladder::FilterModel::Ladder);
        assert!(p.ladder_12db);
        // and the whole song survives a .cwt save / load
        let path = std::env::temp_dir().join(format!("cw_gen_{}.cwt", std::process::id()));
        save_cwt(&s, &path).unwrap();
        let back = load_cwt(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(back.instruments[0].kind, s.instruments[0].kind);
        assert_eq!(back.instruments[1].kind, s.instruments[1].kind);
        assert_eq!(back.instruments[2].kind, s.instruments[2].kind);
        // old synth patches (no filter_model) keep the SVF
        let old: crate::synth::Patch = toml::from_str(
            &toml::to_string(&crate::synth::Patch::default())
                .unwrap()
                .lines()
                .filter(|l| !l.starts_with("filter_model") && !l.starts_with("ladder_12db"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        assert_eq!(old.filter_model, crate::ladder::FilterModel::Svf);
    }

    #[test]
    fn thirty_two_channel_song_round_trips() {
        let mut s = demo_song();
        let n0 = s.channels;
        s.channels = MAX_CHANNELS;
        s.channel_names.resize(MAX_CHANNELS, "Ch".into());
        s.channel_names[31] = "Last".into();
        for p in &mut s.patterns {
            for r in &mut p.cells {
                r.resize(MAX_CHANNELS, Cell::default());
            }
        }
        s.patterns[0].cells[5][31] = Cell {
            note: 49,
            instr: 2,
            vol: 0x31,
            fx: 0x38,
            fxp: 0x80,
        };
        s.patterns[0].cells[6][17] = Cell {
            note: 37,
            instr: 1,
            vol: 0,
            fx: 0,
            fxp: 0,
        };
        s.mixer.channels[31].volume = 0.25;
        s.mixer.channels[31].pan = -0.5;
        s.mixer.channels[20].mute = true;
        s.mixer.duck_key = 30;
        let dir = std::env::temp_dir().join(format!("chipwave_32ch_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("wide.cwt");
        save_cwt(&s, &f).unwrap();
        let (l, warn) = load_cwt_warn(&f).unwrap();
        assert!(warn.is_empty(), "{warn:?}");
        assert_eq!(l.channels, 32);
        assert_eq!(l.channel_names[31], "Last");
        assert_eq!(l.patterns[0].cells[5][31], s.patterns[0].cells[5][31]);
        assert_eq!(l.patterns[0].cells[6][17], s.patterns[0].cells[6][17]);
        assert_eq!(l.patterns[0].cells[0][..n0], s.patterns[0].cells[0][..n0]);
        assert_eq!(l.mixer.channels[31].volume, 0.25);
        assert_eq!(l.mixer.channels[31].pan, -0.5);
        assert!(l.mixer.channels[20].mute);
        assert_eq!(l.mixer.duck_key, 30);
        // a channel count above the limit is clamped, not rejected
        let mut big = s.clone();
        big.channels = 40;
        for p in &mut big.patterns {
            for r in &mut p.cells {
                r.resize(40, Cell::default());
            }
        }
        save_cwt(&big, &f).unwrap();
        let l = load_cwt(&f).unwrap();
        assert_eq!(l.channels, MAX_CHANNELS);
        assert!(l
            .patterns
            .iter()
            .all(|p| p.cells.iter().all(|r| r.len() == MAX_CHANNELS)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_eight_channel_songs_keep_their_layout() {
        let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/chipwave-0.2.3.6-song.cwt");
        let s = load_cwt(&f).unwrap();
        let demo = demo_song_n(1);
        assert_eq!(s.channels, demo.channels);
        assert!(s
            .patterns
            .iter()
            .all(|p| p.cells.iter().all(|r| r.len() == s.channels)));
        // the mixer has a strip for every possible channel; old ones unchanged
        assert_eq!(s.mixer.channels.len(), MAX_CHANNELS);
        assert_eq!(s.mixer.channels[..8], demo.mixer.channels[..8]);
    }

    #[test]
    fn text_song_with_sixteen_tracks() {
        let mut t = String::from("[song]\ntitle = \"wide\"\ntracks = 16\n[[instrument]]\nname = \"a\"\npatch = \"builtin:init\"\n[mixer]\n");
        for k in 0..16 {
            t += &format!(
                "[[mixer.channel]]\nname = \"T{k}\"\npan = {}\n",
                if k == 15 { -1.0 } else { 0.0 }
            );
        }
        t += "[[block]]\nname = \"b\"\nlines = 16\nrows = \"\"\"\n00";
        for k in 0..16 {
            t += if k == 15 {
                " | C-4 01 .. ...."
            } else {
                " | --- .. .. ...."
            };
        }
        t += "\n\"\"\"\n";
        let s = song_from_text(&t).unwrap();
        assert_eq!(s.channels, 16);
        assert_eq!(s.channel_names[15], "T15");
        assert_eq!(s.mixer.channels[15].pan, -1.0);
        assert_eq!(s.patterns[0].cells[0][15].note, 49);
    }

    #[test]
    fn song_key_in_text_songs_and_cwt_round_trip() {
        let t = "[song]\ntitle = \"k\"\nkey = \"F# dorian\"\n[[instrument]]\nname = \"a\"\npatch = \"builtin:init\"\n[[block]]\nname = \"b\"\nlines = 4\nrows = \"\"\"\n00 | C-4 01 .. ....\n\"\"\"\n";
        let s = song_from_text(t).unwrap();
        let k = s.info.key.expect("key parsed");
        assert_eq!(k.name(), "F# dorian");
        assert!(song_from_text(&t.replace("F# dorian", "H lydian")).is_err());
        // .cwt keeps the key; songs without one stay keyless
        let p = std::env::temp_dir().join(format!("cw-key-{}.cwt", std::process::id()));
        save_cwt(&s, &p).unwrap();
        assert_eq!(load_cwt(&p).unwrap().info.key, Some(k));
        let mut s2 = s.clone();
        s2.info.key = None;
        save_cwt(&s2, &p).unwrap();
        assert_eq!(load_cwt(&p).unwrap().info.key, None);
        let _ = std::fs::remove_file(&p);
        // a 0.2.3.6 song (no key field) still loads
        let old = load_cwt(std::path::Path::new(
            "tests/fixtures/chipwave-0.2.3.6-song.cwt",
        ))
        .unwrap();
        assert_eq!(old.info.key, None);
    }
}
