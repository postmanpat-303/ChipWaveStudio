//! Custom racks (`.cwr`, docs/racks/FORMAT.md): a rack instrument with its
//! layer instruments, every setting, in one file in the Racks folder
//! (`<config>/Racks`). Save, check, load into a song, and the library
//! operations of My Racks (rename, duplicate, update with a backup, delete
//! to Racks/Deleted). Loading checks everything before anything changes
//! and never panics on a broken file.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Instrument, InstrumentKind, Layer, RackP, Song, MAX_LAYERS};

pub const EXT: &str = "cwr";
pub const FORMAT: &str = "chipwave-rack";
pub const FORMAT_VERSION: u32 = 1;
const MIME: &[u8] = b"application/x-chipwave-rack";
const MAX_ENTRY: u64 = 256 * 1024 * 1024;
/// backups kept per rack by Update
pub const BACKUPS: usize = 3;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RackInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// "Keys", "Bass", "Pad", ... (free text, the Browser's sound type)
    #[serde(default)]
    pub sound_type: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// UTC date, YYYY-MM-DD
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

/// a plugin a rack uses (so the Browser can warn without opening it all)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginRef {
    pub format: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    /// "layer B › EQ" style: where it sits
    #[serde(default)]
    pub used_by: String,
}

#[derive(Serialize, Deserialize)]
struct RackFile {
    format: String,
    format_version: u32,
    #[serde(default)]
    app: String,
    rack: RackInfo,
    #[serde(default)]
    instruments: Vec<Instrument>,
    #[serde(default)]
    plugins: Vec<PluginRef>,
}

/// A rack as saved: `instruments[0]` is the rack, its layers point at
/// entries 1..n.
#[derive(Clone, Debug, PartialEq)]
pub struct Rack {
    pub info: RackInfo,
    pub instruments: Vec<Instrument>,
    pub plugins: Vec<PluginRef>,
}

/// the Racks folder (tests: a folder of their own)
pub fn dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        TEST_DIR.with(|d| d.borrow().clone())
    }
    #[cfg(not(test))]
    {
        crate::settings::config_dir().map(|d| d.join("Racks"))
    }
}

#[cfg(test)]
thread_local! {
    pub static TEST_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// today, UTC, YYYY-MM-DD
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The rack of instrument `idx` in `song`. A plain instrument becomes a
/// 1-layer rack (D13): the rack keeps its name, volume, transpose and
/// effects, the layer its sound. Plugin instruments can't be layers yet.
pub fn from_song(song: &Song, idx: usize, info: RackInfo) -> Result<Rack, String> {
    let inst = song
        .instruments
        .get(idx)
        .ok_or_else(|| "that instrument isn't in the song".to_string())?;
    let mut out = vec![];
    match &inst.kind {
        InstrumentKind::Rack(r) => {
            if r.layers.is_empty() {
                return Err("the rack has no layers yet: add one first".into());
            }
            let mut rack = inst.clone();
            let mut layers = vec![];
            for (k, l) in r.layers.iter().enumerate() {
                let li = song
                    .instruments
                    .get(l.slot)
                    .ok_or_else(|| format!("layer {} points at a missing instrument", k + 1))?;
                out.push(li.clone());
                layers.push(Layer {
                    slot: k + 1,
                    ..l.clone()
                });
            }
            rack.kind = InstrumentKind::Rack(RackP { layers });
            out.insert(0, rack);
        }
        InstrumentKind::Plugin(_) => {
            return Err(
                "plugin instruments can't be saved as a rack yet (they can't play as a layer)"
                    .into(),
            )
        }
        _ => {
            let mut s = Song::empty();
            s.instruments = vec![inst.clone()];
            let slot = s
                .make_rack(0)
                .ok_or("this instrument can't become a rack")?;
            debug_assert_eq!(slot, 1);
            out = s.instruments;
        }
    }
    let plugins = plugin_refs(&out);
    let mut info = info;
    info.name = clean_name(&info.name, &inst.name);
    if info.created.is_empty() {
        info.created = today();
    }
    info.updated = today();
    Ok(Rack {
        info,
        instruments: out,
        plugins,
    })
}

fn clean_name(name: &str, fallback: &str) -> String {
    let n: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    if n.is_empty() {
        fallback.chars().take(64).collect()
    } else {
        n
    }
}

/// every plugin effect (and plugin instrument) in the rack
pub fn plugin_refs(instruments: &[Instrument]) -> Vec<PluginRef> {
    let mut v: Vec<PluginRef> = vec![];
    fn walk(fx: &[crate::fx::Device], at: &str, v: &mut Vec<PluginRef>) {
        for d in fx {
            if let Some(p) = &d.plugin {
                v.push(PluginRef {
                    format: p.format.clone(),
                    id: p.id.clone(),
                    name: p.name.clone(),
                    vendor: p.vendor.clone(),
                    used_by: format!("{at} › effects"),
                });
            }
            for p in &d.paths {
                walk(&p.devices, at, v);
            }
        }
    }
    for (k, i) in instruments.iter().enumerate() {
        let at = if k == 0 {
            "the rack".to_string()
        } else {
            format!("layer {}", (b'A' + (k - 1).min(25) as u8) as char)
        };
        if let InstrumentKind::Plugin(p) = &i.kind {
            v.push(PluginRef {
                format: p.format.clone(),
                id: p.id.clone(),
                name: p.name.clone(),
                vendor: p.vendor.clone(),
                used_by: at.clone(),
            });
        }
        walk(&i.fx, &at, &mut v);
    }
    v
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

/// Write `rack` to `path` (through a temp file: a failed save never leaves
/// half a file behind).
pub fn save(rack: &Rack, path: &Path) -> Result<(), String> {
    validate(rack)?;
    let tmp = path.with_extension("cwr.tmp");
    let r = (|| {
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(f);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let defl = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let e = |e: zip::result::ZipError| e.to_string();
        z.start_file("mimetype", stored).map_err(e)?;
        z.write_all(MIME).map_err(|e| e.to_string())?;
        let mut insts = rack.instruments.clone();
        for (k, i) in insts.iter_mut().enumerate() {
            if let InstrumentKind::Sample {
                file, data, rate, ..
            } = &mut i.kind
            {
                let name = format!("samples/{k:02}.wav");
                *file = name.clone();
                z.start_file(&name, stored).map_err(e)?;
                let d: Vec<f32> = data.iter().copied().collect();
                z.write_all(&wav_bytes(&d, *rate)?)
                    .map_err(|e| e.to_string())?;
            }
        }
        let f = RackFile {
            format: FORMAT.into(),
            format_version: FORMAT_VERSION,
            app: format!("ChipWave Studio {}", crate::VERSION),
            rack: rack.info.clone(),
            instruments: insts,
            plugins: plugin_refs(&rack.instruments),
        };
        z.start_file("rack.toml", defl).map_err(e)?;
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
    let f = z.by_name(name).map_err(|_| format!("{name} is missing"))?;
    if f.size() > MAX_ENTRY {
        return Err(format!("{name} is too large"));
    }
    let mut v = Vec::new();
    f.take(MAX_ENTRY)
        .read_to_end(&mut v)
        .map_err(|e| e.to_string())?;
    Ok(v)
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "The rack".into())
}

fn read_file(path: &Path) -> Result<(RackFile, zip::ZipArchive<std::fs::File>), String> {
    let label = file_label(path);
    let f = std::fs::File::open(path).map_err(|e| format!("{label} can't be opened ({e})"))?;
    let mut z = zip::ZipArchive::new(f)
        .map_err(|_| format!("{label} is damaged (it isn't a rack file)"))?;
    let t = entry(&mut z, "rack.toml")
        .map_err(|_| format!("{label} is damaged (rack.toml is missing)"))?;
    let v: toml::Value = toml::from_str(&String::from_utf8_lossy(&t))
        .map_err(|_| format!("{label} is damaged (rack.toml can't be read)"))?;
    // the version first: a newer file may not parse as this version does
    if v.get("format").and_then(|x| x.as_str()) != Some(FORMAT) {
        return Err(format!("{label} isn't a ChipWave rack"));
    }
    let ver = v
        .get("format_version")
        .and_then(|x| x.as_integer())
        .unwrap_or(0);
    if ver > FORMAT_VERSION as i64 {
        return Err(format!(
            "{label} was made by a newer ChipWave (rack format {ver}). Please update to load it."
        ));
    }
    let f: RackFile = v
        .try_into()
        .map_err(|_| format!("{label} is damaged (rack.toml can't be read)"))?;
    Ok((f, z))
}

/// name, description, tags and plugins only (the Browser list)
pub fn read_info(path: &Path) -> Result<(RackInfo, Vec<PluginRef>, usize), String> {
    let (f, _) = read_file(path)?;
    let layers = match f.instruments.first().map(|i| &i.kind) {
        Some(InstrumentKind::Rack(r)) => r.layers.len(),
        _ => 0,
    };
    Ok((f.rack, f.plugins, layers))
}

/// Read and check a rack file, samples included. Nothing is loaded if
/// any part fails; the error is one plain sentence.
pub fn load(path: &Path) -> Result<Rack, String> {
    let label = file_label(path);
    let (f, mut z) = read_file(path)?;
    let mut rack = Rack {
        info: f.rack,
        instruments: f.instruments,
        plugins: f.plugins,
    };
    for (k, i) in rack.instruments.iter_mut().enumerate() {
        if let InstrumentKind::Sample {
            file,
            data,
            rate,
            lp,
            ..
        } = &mut i.kind
        {
            let b = entry(&mut z, file)
                .map_err(|_| format!("{label} is damaged (the sample of entry {k} is missing)"))?;
            let (d, r) = crate::fileio::decode_wav(std::io::Cursor::new(b)).map_err(|_| {
                format!("{label} is damaged (the sample of entry {k} can't be read)")
            })?;
            *data = d.into();
            *rate = r;
            lp.fix(data.len());
        }
    }
    fix(&mut rack);
    validate(&rack).map_err(|e| format!("{label} can't be loaded: {e}"))?;
    Ok(rack)
}

/// clamp what a hand-edited file could get wrong (parameters, names)
fn fix(rack: &mut Rack) {
    rack.info.name = clean_name(&rack.info.name, "Rack");
    rack.info.description = rack.info.description.chars().take(2000).collect();
    rack.info.tags.truncate(16);
    for i in &mut rack.instruments {
        i.name = i.name.chars().take(64).collect();
        i.volume = if i.volume.is_finite() {
            i.volume.clamp(0.0, 2.0)
        } else {
            1.0
        };
        i.transpose = i.transpose.clamp(-48, 48);
        i.fx.truncate(crate::fx::MAX_DEVICES);
        i.fx.iter_mut().for_each(|d| d.fix());
        match &mut i.kind {
            InstrumentKind::Voice(v) => v.fix(),
            InstrumentKind::Gen(g) => g.fix(),
            _ => {}
        }
    }
    let n = rack.instruments.len();
    if let Some(InstrumentKind::Rack(r)) = rack.instruments.first_mut().map(|i| &mut i.kind) {
        for l in &mut r.layers {
            l.level_db = if l.level_db.is_finite() {
                l.level_db.clamp(-60.0, 6.0)
            } else {
                0.0
            };
            l.pan = if l.pan.is_finite() {
                l.pan.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            l.transpose = l.transpose.clamp(-48, 48);
            l.key_hi = l.key_hi.min(127);
            if l.key_lo > l.key_hi {
                std::mem::swap(&mut l.key_lo, &mut l.key_hi);
            }
        }
        let _ = n;
    }
}

/// The checks before anything is applied: entry 0 a rack of 1-4 layers,
/// every layer reference in range and used once, no rack inside a rack,
/// Splits within limits, plugin ids present.
pub fn validate(rack: &Rack) -> Result<(), String> {
    let Some(first) = rack.instruments.first() else {
        return Err("it holds no instruments".into());
    };
    let InstrumentKind::Rack(r) = &first.kind else {
        return Err("its first instrument isn't a rack".into());
    };
    if r.layers.is_empty() || r.layers.len() > MAX_LAYERS {
        return Err(format!("it has {} layers (1 to 4 allowed)", r.layers.len()));
    }
    let n = rack.instruments.len();
    let mut seen = vec![false; n];
    for (k, l) in r.layers.iter().enumerate() {
        if l.slot == 0 || l.slot >= n {
            return Err(format!(
                "layer {} points at an instrument that isn't there",
                k + 1
            ));
        }
        if seen[l.slot] {
            return Err(format!("layer {} repeats another layer", k + 1));
        }
        seen[l.slot] = true;
    }
    for (k, i) in rack.instruments.iter().enumerate().skip(1) {
        if matches!(i.kind, InstrumentKind::Rack(_)) {
            return Err("it has a rack inside a rack".into());
        }
        if !seen[k] {
            return Err(format!("instrument {k} isn't used by any layer"));
        }
    }
    for i in &rack.instruments {
        for d in &i.fx {
            if d.split_depth() > crate::fx::MAX_SPLIT_DEPTH {
                return Err("a Split sits more than 2 deep".into());
            }
            if d.kind == crate::fx::FxKind::Split
                && !(1..=crate::fx::MAX_PATHS).contains(&d.paths.len())
            {
                return Err("a Split has the wrong number of paths".into());
            }
        }
        if let InstrumentKind::Plugin(p) = &i.kind {
            if p.id.trim().is_empty() {
                return Err("a plugin has no id".into());
            }
        }
    }
    for p in plugin_refs(&rack.instruments) {
        if p.id.trim().is_empty() {
            return Err("a plugin effect has no id".into());
        }
    }
    Ok(())
}

/// Put `rack` into `song` (D9): its instruments become new slots at the
/// end; track `track` (if any) switches to the rack, every other track and
/// the slot it used before are untouched. Returns the rack's slot.
pub fn load_into(song: &mut Song, rack: &Rack, track: Option<usize>) -> Result<usize, String> {
    validate(rack)?;
    let base = song.instruments.len();
    if base + rack.instruments.len() > 255 {
        return Err(format!(
            "the rack needs {} instrument slots and only {} are free",
            rack.instruments.len(),
            255 - base
        ));
    }
    for (k, i) in rack.instruments.iter().enumerate() {
        let mut i = i.clone();
        if k == 0 {
            if let InstrumentKind::Rack(r) = &mut i.kind {
                for l in &mut r.layers {
                    l.slot += base;
                }
            }
            if i.name.trim().is_empty() {
                i.name = rack.info.name.clone();
            }
        }
        song.instruments.push(i);
    }
    if let Some(tr) = track {
        if tr < song.channels {
            crate::chanops::assign_track_instrument(song, tr, (base + 1) as u8);
        }
    }
    Ok(base)
}

/// "Glass Keys.cwr" for a name (no path separators or odd characters)
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
    format!("{}.{EXT}", if s.is_empty() { "Rack" } else { &s })
}

/// a path in `dir` for `name` that doesn't exist yet ("Pad.cwr", "Pad 2.cwr")
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

/// one rack in the library
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub info: RackInfo,
    pub plugins: Vec<PluginRef>,
    pub layers: usize,
    /// why it can't be read (listed anyway, so it can be deleted)
    pub error: Option<String>,
}

/// the racks in `dir` (not Deleted/ or .backup/), by name
pub fn scan(dir: &Path) -> Vec<Entry> {
    let mut v = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else {
        return v;
    };
    for e in rd.flatten().take(4000) {
        let p = e.path();
        if !p.is_file() || p.extension().and_then(|x| x.to_str()) != Some(EXT) {
            continue;
        }
        match read_info(&p) {
            Ok((info, plugins, layers)) => v.push(Entry {
                path: p,
                info,
                plugins,
                layers,
                error: None,
            }),
            Err(err) => v.push(Entry {
                info: RackInfo {
                    name: p
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    ..Default::default()
                },
                path: p,
                plugins: vec![],
                layers: 0,
                error: Some(err),
            }),
        }
    }
    v.sort_by_key(|e| e.info.name.to_lowercase());
    v
}

/// the search: every word in the name, description, sound type or tags
pub fn matches(e: &Entry, query: &str) -> bool {
    let hay = format!(
        "{} {} {} {}",
        e.info.name,
        e.info.description,
        e.info.sound_type,
        e.info.tags.join(" ")
    )
    .to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|w| hay.contains(w))
}

/// rename: the name inside and the file name; returns the new path
pub fn rename(path: &Path, name: &str) -> Result<PathBuf, String> {
    let mut r = load(path)?;
    let name = clean_name(name, &r.info.name);
    r.info.name = name.clone();
    r.info.updated = today();
    let dir = path.parent().ok_or("no folder")?;
    let new = if file_name(&name) == file_label(path) {
        path.to_path_buf()
    } else {
        free_path(dir, &name)
    };
    save(&r, &new)?;
    if new != path {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(new)
}

/// a copy named "<name> copy"; returns its path
pub fn duplicate(path: &Path) -> Result<PathBuf, String> {
    let mut r = load(path)?;
    r.info.name = format!("{} copy", r.info.name).chars().take(64).collect();
    r.info.created = today();
    r.info.updated = today();
    let dir = path.parent().ok_or("no folder")?;
    let new = free_path(dir, &r.info.name);
    save(&r, &new)?;
    Ok(new)
}

/// Update the file with a new sound, keeping its name, description, tags
/// and created date. The old file goes to `.backup/` first (the newest
/// 3 backups of each rack are kept).
pub fn update(path: &Path, mut rack: Rack) -> Result<(), String> {
    let old = read_info(path).map(|x| x.0).ok();
    if let Some(o) = old {
        rack.info = RackInfo {
            updated: today(),
            ..o
        };
    }
    backup(path)?;
    save(&rack, path)
}

fn backup(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let dir = path.parent().ok_or("no folder")?.join(".backup");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    std::fs::copy(path, dir.join(format!("{stem}.{stamp}.{EXT}"))).map_err(|e| e.to_string())?;
    // the newest BACKUPS of this rack stay
    let mut mine: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().is_some_and(|n| {
                        let n = n.to_string_lossy();
                        n.starts_with(&format!("{stem}."))
                            && n.ends_with(&format!(".{EXT}"))
                            && n[stem.len() + 1..n.len() - EXT.len() - 1]
                                .chars()
                                .all(|c| c.is_ascii_digit())
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    mine.sort();
    while mine.len() > BACKUPS {
        let _ = std::fs::remove_file(mine.remove(0));
    }
    Ok(())
}

/// Delete (D10): the file moves to `Racks/Deleted`, never gone for good
/// from here. Returns where it went.
pub fn delete(path: &Path) -> Result<PathBuf, String> {
    let dir = path.parent().ok_or("no folder")?.join("Deleted");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = file_label(path);
    let mut to = dir.join(&name);
    let mut k = 2;
    while to.exists() {
        let stem = name.trim_end_matches(&format!(".{EXT}"));
        to = dir.join(format!("{stem} {k}.{EXT}"));
        k += 1;
    }
    std::fs::rename(path, &to).map_err(|e| e.to_string())?;
    Ok(to)
}

#[cfg(test)]
#[path = "racks_tests.rs"]
pub(crate) mod tests;
