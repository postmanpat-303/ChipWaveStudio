//! The file browser's model (docs/BROWSER.md): every loadable thing as an
//! `Item` (built-in sounds and presets, the instrument library, the song's
//! own instruments, scanned plugins, demo songs, and files found in the
//! user's folders), the filters (tab, search, tags, category, creator,
//! location, favourites), the background folder scan and its cached index.
//! The panel itself is `app/browser.rs`.
use crate::instfile;
use crate::model::{Instrument, InstrumentKind, Song};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// file / item format: the badge and its colour
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Fmt {
    /// built-in sound, preset or library instrument (.cwi)
    Cwi,
    Vst3,
    Clap,
    Wav,
    Flac,
    Mp3,
    Ogg,
    /// .cwt / .cwt.toml
    Song,
    /// MOD / MED / XM / IT / S3M (import planned)
    Mod,
    Wavetable,
    Grid,
    /// .mid (import planned)
    Midi,
    /// a built-in drum pattern (Drum Patterns tab; not in the legend)
    Pattern,
    /// a built-in insert effect (Effects tab; not in the legend)
    Fx,
}

impl Fmt {
    pub const ALL: [Fmt; 12] = [
        Fmt::Cwi,
        Fmt::Vst3,
        Fmt::Clap,
        Fmt::Wav,
        Fmt::Flac,
        Fmt::Mp3,
        Fmt::Ogg,
        Fmt::Song,
        Fmt::Mod,
        Fmt::Wavetable,
        Fmt::Grid,
        Fmt::Midi,
    ];
    pub fn badge(self) -> &'static str {
        match self {
            Fmt::Cwi => "CWI",
            Fmt::Vst3 => "VST3",
            Fmt::Clap => "CLAP",
            Fmt::Wav => "WAV",
            Fmt::Flac => "FLAC",
            Fmt::Mp3 => "MP3",
            Fmt::Ogg => "OGG",
            Fmt::Song => "CWT",
            Fmt::Mod => "MOD",
            Fmt::Wavetable => "WT",
            Fmt::Grid => "GRID",
            Fmt::Midi => "MID",
            Fmt::Pattern => "PAT",
            Fmt::Fx => "FX",
        }
    }
    /// legend text
    pub fn what(self) -> &'static str {
        match self {
            Fmt::Cwi => "instrument",
            Fmt::Vst3 | Fmt::Clap => "plugin",
            Fmt::Wav | Fmt::Flac | Fmt::Mp3 | Fmt::Ogg => "sample",
            Fmt::Song => "song",
            Fmt::Mod => "MOD/MED*",
            Fmt::Wavetable => "wavetable",
            Fmt::Grid => "Grid patch",
            Fmt::Midi => "MIDI*",
            Fmt::Pattern => "drum pattern",
            Fmt::Fx => "effect",
        }
    }
    /// badge colour (0xRRGGBB), the same in every theme (dark text on it)
    pub fn colour(self) -> u32 {
        match self {
            Fmt::Cwi => 0x5CC07A,
            Fmt::Vst3 => 0xA98BE0,
            Fmt::Clap => 0xE07BB5,
            Fmt::Wav => 0x4FB8C9,
            Fmt::Flac => 0x4F9FD9,
            Fmt::Mp3 => 0x62C9B2,
            Fmt::Ogg => 0x7FC4E0,
            Fmt::Song => 0xE0B45C,
            Fmt::Mod => 0xE08A4F,
            Fmt::Wavetable => 0xC9CF52,
            Fmt::Grid => 0xE06C6C,
            Fmt::Midi => 0x6C8FE8,
            Fmt::Pattern => 0xE8A25C,
            Fmt::Fx => 0x5FC9C0,
        }
    }
    pub fn is_sample(self) -> bool {
        matches!(self, Fmt::Wav | Fmt::Flac | Fmt::Mp3 | Fmt::Ogg)
    }
    /// can't be loaded yet (shown, marked "import planned")
    pub fn planned(self) -> bool {
        matches!(self, Fmt::Mod | Fmt::Midi)
    }
}

/// the category tabs, in order
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Instruments,
    /// built-in insert effects, Grid FX patches and effect plugins (0.2.4
    /// Devices): drag onto the Devices row or a track
    Effects,
    Plugins,
    Samples,
    Songs,
    Presets,
    Grid,
    Wavetables,
    Midi,
    /// built-in drum grooves by style (src/drumpat.rs)
    Patterns,
    /// the file system: drives, quick locations, folders (src/drives.rs)
    Files,
}

impl Tab {
    pub const ALL: [Tab; 11] = [
        Tab::Instruments,
        Tab::Effects,
        Tab::Plugins,
        Tab::Samples,
        Tab::Songs,
        Tab::Presets,
        Tab::Grid,
        Tab::Wavetables,
        Tab::Midi,
        Tab::Patterns,
        Tab::Files,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Tab::Instruments => "Instruments",
            Tab::Effects => "Effects",
            Tab::Plugins => "Plugins",
            Tab::Samples => "Samples",
            Tab::Songs => "Songs",
            Tab::Presets => "Presets",
            Tab::Grid => "Grid",
            Tab::Wavetables => "Wavetables",
            Tab::Midi => "MIDI",
            Tab::Patterns => "Drum Patterns",
            Tab::Files => "Files",
        }
    }
    /// the tab's colour (the format it is mostly about)
    pub fn fmt(self) -> Fmt {
        match self {
            Tab::Instruments | Tab::Presets => Fmt::Cwi,
            Tab::Effects => Fmt::Fx,
            Tab::Plugins => Fmt::Vst3,
            Tab::Samples => Fmt::Wav,
            Tab::Songs => Fmt::Song,
            Tab::Grid => Fmt::Grid,
            Tab::Wavetables => Fmt::Wavetable,
            Tab::Midi => Fmt::Midi,
            Tab::Patterns => Fmt::Pattern,
            Tab::Files => Fmt::Song,
        }
    }
}

/// where an item lives (the Location column)
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Loc {
    /// built in, or the instrument library folder
    Library,
    /// the open song's instruments
    Song,
    /// a folder the user added (its path)
    Folder(String),
    /// the plugin folders (scanned plugins)
    Plugins,
}

/// what loading an item does
#[derive(Clone, Debug, PartialEq)]
pub enum Src {
    /// factory preset / built-in patch / Voice preset (instfile)
    Factory(instfile::Source),
    /// a blank instrument: "synth" or "voice"
    Init(&'static str),
    /// a generator preset (`gen::presets`, incl. Wave and Grid)
    Gen(String),
    /// a Grid FX patch (`grid::fx_presets`): an FX device
    GridFx(String),
    /// a built-in wavetable (`wavetable::BUILTIN` index)
    Table(u8),
    /// instrument `i` (0-based) of the open song
    SongInst(usize),
    /// a scanned plugin
    Plugin(crate::plugin::scan::Found),
    /// a demo song (`fileio::DEMOS` index)
    Demo(usize),
    /// a file on disk (.cwi, sample, wavetable WAV, song, MIDI, module)
    File(PathBuf),
    /// a built-in drum pattern (`drumpat::PATTERNS` index)
    Pattern(usize),
    /// a built-in insert effect
    Fx(crate::fx::FxKind),
    /// one of the ChipWave Instruments (the track pie's Add list, by index):
    /// a new instrument on a playable preset, given to a track
    Tool(u8),
}

/// the Instruments tab's first category: the built-in instruments, as the
/// track pie's Add lists them
pub const TOOLS_CAT: &str = "ChipWave Instruments";

/// the ChipWave Instruments rows: (label, tooltip) in the pie's order
pub fn tool_items(tools: &[(&str, &str)]) -> Vec<Item> {
    tools
        .iter()
        .enumerate()
        .map(|(i, (label, tip))| {
            item(
                format!("tool:{label}"),
                (*label).into(),
                Tab::Instruments,
                Fmt::Cwi,
                TOOLS_CAT,
                "ChipWave",
                Loc::Library,
                (*tip).into(),
                Src::Tool(i as u8),
            )
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// stable id (favourites and ratings): "kind:name" or the file path
    pub key: String,
    pub name: String,
    pub tab: Tab,
    pub fmt: Fmt,
    pub category: String,
    pub creator: String,
    pub loc: Loc,
    pub tags: Vec<&'static str>,
    /// second line: type, size, length…
    pub detail: String,
    pub src: Src,
}

impl Item {
    /// a plugin effect / Grid FX patch (goes into an FX chain)
    pub fn is_effect(&self) -> bool {
        match &self.src {
            Src::Plugin(f) => !f.desc.is_instrument(),
            Src::GridFx(_) | Src::Fx(_) => true,
            _ => false,
        }
    }
    /// listed under tab `t` (the Effects tab also lists effect plugins and
    /// Grid FX patches from their own tabs)
    pub fn in_tab(&self, t: Tab) -> bool {
        self.tab == t || (t == Tab::Effects && self.is_effect())
    }
    /// can be auditioned (played on C-3 through the current track)
    pub fn previewable(&self) -> bool {
        match &self.src {
            Src::Plugin(_) | Src::Demo(_) | Src::GridFx(_) | Src::Fx(_) => false,
            Src::Pattern(_) => true,
            Src::File(_) => {
                self.fmt == Fmt::Cwi || self.fmt.is_sample() || self.fmt == Fmt::Wavetable
            }
            _ => self.tab != Tab::Songs,
        }
    }
}

/// tag chips: keyword -> tag (searched in the name, category and kind)
const TAGS: [(&str, &[&str]); 15] = [
    ("Bass", &["bass", "sub", "reese", "808 boom"]),
    ("Lead", &["lead", "solo", "arp", "acid"]),
    ("Pad", &["pad", "string", "choir", "atmo", "drone"]),
    ("Keys", &["key", "piano", "organ", "bell", "e-piano", "ep "]),
    (
        "Drums",
        &[
            "kick", "snare", "hat", "clap", "tom", "perc", "drum", "kit", "cymbal", "cowbell",
            "conga", "rim", "shaker", "claves", "break", "loop",
        ],
    ),
    (
        "FX",
        &[
            "riser",
            "fx",
            "impact",
            "zap",
            "noise",
            "sweep",
            "downlifter",
            "laser",
            "wind",
            "vinyl",
            "coin",
            "jump",
        ],
    ),
    (
        "Chip",
        &[
            "chip", "pulse", "square", "nes", "sid", "8-bit", "8bit", "gameboy", "bit",
        ],
    ),
    ("Analog", &["analog", "moog", "ladder", "saw"]),
    ("FM", &["fm", "operator", "dx"]),
    (
        "Voice",
        &["voice", "vowel", "vocal", "vox", "speech", "choir", "robot"],
    ),
    ("Warm", &["warm", "soft", "mellow", "smooth"]),
    ("Bright", &["bright", "super", "hoover", "shimmer"]),
    ("Plucky", &["pluck", "stab", "staccato"]),
    (
        "Evolving",
        &["evolv", "morph", "motion", "scan", "formant", "sweep"],
    ),
    // applied by catalogue.rs for curated open-source plugins (not keyword-inferred)
    ("Showcase", &[]),
];

pub const TAG_NAMES: [&str; 15] = [
    "Bass", "Lead", "Pad", "Keys", "Drums", "FX", "Chip", "Analog", "FM", "Voice", "Warm",
    "Bright", "Plucky", "Evolving", "Showcase",
];

/// tags for an item named `name` with extra words `more`
pub fn tags_for(name: &str, more: &str) -> Vec<&'static str> {
    let hay = format!(" {} {} ", name, more).to_lowercase();
    TAGS.iter()
        .filter(|(_, words)| words.iter().any(|w| hay.contains(w)))
        .map(|(t, _)| *t)
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn item(
    key: String,
    name: String,
    tab: Tab,
    fmt: Fmt,
    category: &str,
    creator: &str,
    loc: Loc,
    detail: String,
    src: Src,
) -> Item {
    let tags = tags_for(&name, &format!("{category} {detail}"));
    Item {
        key,
        name,
        tab,
        fmt,
        category: category.to_string(),
        creator: creator.to_string(),
        loc,
        tags,
        detail,
        src,
    }
}

fn engine_name(e: &crate::gen::GenEngine) -> &'static str {
    use crate::gen::GenEngine as G;
    match e {
        G::Drum(_) => "Drums",
        G::Kit(_) => "Kit",
        G::ChipFx(_) => "Chip FX",
        G::Analog(_) => "Analog",
        G::Fm(_) => "FM",
        G::Noise(_) => "Noise",
        G::Wave(_) => "Wavetable",
        G::Grid(_) => "Grid",
        G::Poly(_) => crate::poly::POLY_DISPLAY_NAME,
        G::Drums(_) => "Drum Kit",
    }
}

/// Everything built in: instruments (blank synth / voice and every generator
/// preset), factory presets, Grid patches, wavetables and the demo songs.
pub fn builtin_items() -> Vec<Item> {
    let cw = "ChipWave";
    let mut v = vec![];
    for (name, k, detail) in [
        ("Synth (init)", "synth", "plain saw synth"),
        ("Voice (init)", "voice", "plain bass \"ahh\""),
    ] {
        v.push(item(
            format!("init:{k}"),
            name.into(),
            Tab::Instruments,
            Fmt::Cwi,
            if k == "synth" { "Synth" } else { "Voice" },
            cw,
            Loc::Library,
            detail.into(),
            Src::Init(k),
        ));
    }
    for (name, tip, p) in crate::gen::presets() {
        let eng = engine_name(&p.engine);
        let grid = eng == "Grid";
        v.push(item(
            format!("gen:{name}"),
            name.into(),
            if grid { Tab::Grid } else { Tab::Instruments },
            if grid { Fmt::Grid } else { Fmt::Cwi },
            if grid { "Generator" } else { eng },
            cw,
            Loc::Library,
            format!("{eng} generator · {tip}"),
            Src::Gen(name.into()),
        ));
    }
    for (name, tip, _) in crate::drumkit::presets() {
        v.push(item(
            format!("drumkit:{name}"),
            name.clone(),
            Tab::Instruments,
            Fmt::Cwi,
            "Drum Kit",
            cw,
            Loc::Library,
            format!("Drum Kit · 16 pads · {tip}"),
            Src::Gen(format!("drumkit:{name}")),
        ));
    }
    for k in crate::fx::FxKind::ALL {
        v.push(item(
            format!("fx:{k:?}"),
            k.name().into(),
            Tab::Effects,
            Fmt::Fx,
            "Built-in",
            cw,
            Loc::Library,
            k.blurb().into(),
            Src::Fx(k),
        ));
    }
    for (name, tip, _) in crate::grid::fx_presets() {
        v.push(item(
            format!("gridfx:{name}"),
            name.into(),
            Tab::Grid,
            Fmt::Grid,
            "FX",
            cw,
            Loc::Library,
            format!("Grid FX · {tip}"),
            Src::GridFx(name.into()),
        ));
    }
    for e in instfile::factory_entries() {
        if e.kind == "generator" {
            continue; // in Instruments (above)
        }
        let cat = match e.category.as_str() {
            "built-in" => "Built-in".to_string(),
            "voice" => "Voice".to_string(),
            c => {
                let mut s = c.replace('-', " ");
                if let Some(f) = s.get_mut(0..1) {
                    f.make_ascii_uppercase();
                }
                s
            }
        };
        v.push(item(
            format!("preset:{:?}", e.source),
            e.name.clone(),
            Tab::Presets,
            Fmt::Cwi,
            &cat,
            cw,
            Loc::Library,
            format!("{} preset", e.kind),
            Src::Factory(e.source.clone()),
        ));
    }
    for (i, name) in crate::wavetable::BUILTIN.iter().enumerate() {
        v.push(item(
            format!("table:{name}"),
            (*name).into(),
            Tab::Wavetables,
            Fmt::Wavetable,
            "Built-in",
            cw,
            Loc::Library,
            "built-in table · 32 frames".into(),
            Src::Table(i as u8),
        ));
    }
    v.extend(pattern_items());
    for (i, (name, tip, _)) in crate::fileio::DEMOS.iter().enumerate() {
        v.push(item(
            format!("demo:{name}"),
            (*name).into(),
            Tab::Songs,
            Fmt::Song,
            "Demo",
            cw,
            Loc::Library,
            (*tip).into(),
            Src::Demo(i),
        ));
    }
    v
}

/// the built-in drum patterns (Drum Patterns tab): category = style,
/// creator = ChipWave, details = kit · bars · BPM range
pub fn pattern_items() -> Vec<Item> {
    use crate::drumpat::{PATTERNS, STYLES};
    PATTERNS
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut it = item(
                p.key(),
                p.name.into(),
                Tab::Patterns,
                Fmt::Pattern,
                STYLES[p.style],
                "ChipWave",
                Loc::Library,
                format!(
                    "{} · {} bar{} · {}",
                    p.kit,
                    p.bars,
                    if p.bars == 1 { "" } else { "s" },
                    p.bpm_tag()
                ),
                Src::Pattern(i),
            );
            if !it.tags.contains(&"Drums") {
                it.tags.push("Drums");
            }
            it
        })
        .collect()
}

/// .cwi files of the instrument library (`instfile::scan`)
pub fn library_items(entries: &[instfile::Entry]) -> Vec<Item> {
    entries
        .iter()
        .filter_map(|e| {
            let instfile::Source::File(p) = &e.source else {
                return None;
            };
            Some(item(
                p.display().to_string(),
                e.name.clone(),
                Tab::Instruments,
                Fmt::Cwi,
                if e.category.is_empty() {
                    "Library"
                } else {
                    &e.category
                },
                "User",
                Loc::Library,
                format!("{} · .cwi", e.kind),
                Src::File(p.clone()),
            ))
        })
        .collect()
}

/// the open song's instruments
pub fn song_items(song: &Song) -> Vec<Item> {
    song.instruments
        .iter()
        .enumerate()
        .map(|(i, inst)| {
            let tag = kind_label(&inst.kind);
            item(
                format!("song:{i}"),
                format!("{:02X} {}", i + 1, inst.name),
                Tab::Instruments,
                Fmt::Cwi,
                tag,
                "This song",
                Loc::Song,
                format!("{tag} · in this song"),
                Src::SongInst(i),
            )
        })
        .collect()
}

fn kind_label(k: &InstrumentKind) -> &'static str {
    match k {
        InstrumentKind::Synth(_) => "Synth",
        InstrumentKind::Sample { .. } => "Sample",
        InstrumentKind::Voice(_) => "Voice",
        InstrumentKind::Gen(g) => engine_name(&g.engine),
        InstrumentKind::Plugin(_) => "Plugin",
        InstrumentKind::Rack(_) => "Rack",
    }
}

/// scanned CLAP / VST3 plugins (instruments and effects).
/// Known open-source instruments get a richer category / creator and a
/// Showcase tag from [`catalogue`] (Surge XT, Dexed, OB-Xf, …).
pub fn plugin_items(c: &crate::plugin::scan::Cache) -> Vec<Item> {
    let mut v = vec![];
    for f in c.instruments().into_iter().chain(c.effects()) {
        let inst = f.desc.is_instrument();
        let fmt = if f.format == "vst3" {
            Fmt::Vst3
        } else {
            Fmt::Clap
        };
        let feats: Vec<&str> = f
            .desc
            .features
            .iter()
            .map(|s| s.as_str())
            .filter(|s| !matches!(*s, "instrument" | "audio-effect" | "stereo" | "mono"))
            .take(3)
            .collect();
        let known = catalogue::lookup(&f.desc.id, &f.desc.name);
        let vendor = if let Some(e) = known {
            e.creator.to_string()
        } else if f.desc.vendor.is_empty() {
            "Unknown".to_string()
        } else {
            f.desc.vendor.clone()
        };
        let category = if let Some(e) = known {
            e.category
        } else if inst {
            "Instrument"
        } else {
            "Effect"
        };
        let detail = format!(
            "{} {}{}",
            f.format.to_uppercase(),
            if inst { "instrument" } else { "effect" },
            if feats.is_empty() {
                String::new()
            } else {
                format!(" · {}", feats.join(", "))
            }
        );
        let mut it = item(
            format!("plugin:{}:{}", f.format, f.desc.id),
            f.desc.name.clone(),
            Tab::Plugins,
            fmt,
            category,
            &vendor,
            Loc::Plugins,
            detail,
            Src::Plugin(f.clone()),
        );
        if let Some(e) = known {
            for t in e.tags {
                if !it.tags.contains(t) {
                    it.tags.push(*t);
                }
            }
        }
        v.push(it);
    }
    v
}

// ------------------------------------------------------------------ files

/// one file found in a user folder
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileHit {
    pub path: String,
    pub fmt: Fmt,
    pub size: u64,
}

/// the format of a file by its name (None = not shown); a WAV is a
/// wavetable when it sits in a folder named like "wavetables" or carries
/// a Serum `clm ` chunk
pub fn classify(path: &Path) -> Option<Fmt> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name.ends_with(".cwt.toml") || name.ends_with(".cwt") {
        return Some(Fmt::Song);
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    Some(match ext.as_str() {
        "cwi" => Fmt::Cwi,
        "wav" | "wave" => {
            let in_tables = path.parent().is_some_and(|d| {
                d.components().any(|c| {
                    let s = c.as_os_str().to_string_lossy().to_lowercase();
                    s.contains("wavetable") || s == "tables"
                })
            });
            if in_tables || has_clm(path) {
                Fmt::Wavetable
            } else {
                Fmt::Wav
            }
        }
        "flac" => Fmt::Flac,
        "mp3" => Fmt::Mp3,
        "ogg" | "oga" => Fmt::Ogg,
        "mid" | "midi" => Fmt::Midi,
        "mod" | "med" | "mmd0" | "mmd1" | "mmd2" | "mmd3" | "xm" | "it" | "s3m" => Fmt::Mod,
        _ => return None,
    })
}

/// a `clm ` chunk in the first 4 KB (Serum wavetable)
fn has_clm(path: &Path) -> bool {
    use std::io::Read;
    let mut b = [0u8; 4096];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut b))
        .unwrap_or(0);
    b[..n].windows(4).any(|w| w == b"clm ")
}

/// most files listed per folder
pub const MAX_FILES: usize = 50_000;

/// Walk `root` (sub-folders too, hidden ones and plugin bundles skipped) and
/// list the files the browser shows. `count` goes up per folder visited;
/// `stop` ends the walk early.
pub fn scan_folder(root: &Path, count: &AtomicUsize, stop: &AtomicBool) -> Vec<FileHit> {
    fn walk(
        d: &Path,
        depth: usize,
        out: &mut Vec<FileHit>,
        count: &AtomicUsize,
        stop: &AtomicBool,
    ) {
        if depth > 12 || out.len() >= MAX_FILES || stop.load(Ordering::Relaxed) {
            return;
        }
        count.fetch_add(1, Ordering::Relaxed);
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut items: Vec<(PathBuf, bool, u64)> = rd
            .flatten()
            .filter_map(|e| {
                let ft = e.file_type().ok()?;
                if ft.is_symlink() {
                    return None;
                }
                let size = if ft.is_file() {
                    e.metadata().map(|m| m.len()).unwrap_or(0)
                } else {
                    0
                };
                Some((e.path(), ft.is_dir(), size))
            })
            .collect();
        items.sort();
        for (p, dir, size) in items {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if name.starts_with('.') {
                continue;
            }
            if dir {
                if !(name.ends_with(".vst3") || name.ends_with(".clap") || name.ends_with(".app")) {
                    walk(&p, depth + 1, out, count, stop);
                }
            } else if let Some(fmt) = classify(&p) {
                if out.len() < MAX_FILES {
                    out.push(FileHit {
                        path: p.display().to_string(),
                        fmt,
                        size,
                    });
                }
            }
        }
    }
    let mut out = vec![];
    walk(root, 0, &mut out, count, stop);
    out
}

fn short_size(b: u64) -> String {
    if b >= 1 << 20 {
        format!("{:.1} MB", b as f64 / (1u64 << 20) as f64)
    } else {
        format!("{} kB", (b / 1024).max(1))
    }
}

/// the items for the files found under folder `root`
pub fn file_items(root: &str, files: &[FileHit]) -> Vec<Item> {
    let base = Path::new(root);
    files
        .iter()
        .map(|f| {
            let p = Path::new(&f.path);
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            // the category is the sub-folder under the root ("Drums/Kicks")
            let cat = p
                .parent()
                .and_then(|d| d.strip_prefix(base).ok())
                .map(|d| d.display().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    base.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| root.to_string())
                });
            let tab = match f.fmt {
                Fmt::Cwi => Tab::Instruments,
                Fmt::Song | Fmt::Mod => Tab::Songs,
                Fmt::Wavetable => Tab::Wavetables,
                Fmt::Midi => Tab::Midi,
                Fmt::Grid => Tab::Grid,
                Fmt::Vst3 | Fmt::Clap => Tab::Plugins,
                _ => Tab::Samples,
            };
            let what = if f.fmt.planned() {
                " · import planned"
            } else {
                ""
            };
            let name = name
                .strip_suffix(".cwt.toml")
                .map(|s| s.to_string())
                .unwrap_or(name);
            item(
                f.path.clone(),
                name,
                tab,
                f.fmt,
                &cat,
                "User",
                Loc::Folder(root.to_string()),
                format!("{} · {}{what}", f.fmt.badge(), short_size(f.size)),
                Src::File(p.to_path_buf()),
            )
        })
        .collect()
}

// ------------------------------------------------------------------ index cache

/// the folders' file lists as last scanned (shown at once on start, then
/// refreshed in the background)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Index {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub folders: BTreeMap<String, Vec<FileHit>>,
}

pub const INDEX_VERSION: u32 = 1;

fn index_path() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join("browser-index.json"))
}

pub fn load_index() -> Index {
    index_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Index>(&t).ok())
        .filter(|i| i.version == INDEX_VERSION)
        .unwrap_or_default()
}

pub fn save_index(i: &Index) {
    let Some(p) = index_path() else { return };
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut i = i.clone();
    i.version = INDEX_VERSION;
    if let Ok(t) = serde_json::to_string(&i) {
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, t).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}

// ------------------------------------------------------------------ filters

/// the Location filter
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LocFilter {
    #[default]
    All,
    Library,
    Song,
    Favourites,
    Folder(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Name,
    Type,
    Rating,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub tab: Tab,
    pub query: String,
    pub tags: Vec<&'static str>,
    pub category: Option<String>,
    pub creator: Option<String>,
    pub loc: LocFilter,
    pub sort: Sort,
}

/// every search word must appear in the name, category, creator, detail or tags
pub fn matches_query(it: &Item, q: &str) -> bool {
    if q.trim().is_empty() {
        return true;
    }
    let hay = format!(
        "{} {} {} {} {}",
        it.name,
        it.category,
        it.creator,
        it.detail,
        it.tags.join(" ")
    )
    .to_lowercase();
    q.to_lowercase().split_whitespace().all(|w| hay.contains(w))
}

fn loc_ok(it: &Item, l: &LocFilter, favs: &[String]) -> bool {
    match l {
        LocFilter::All => true,
        LocFilter::Library => matches!(it.loc, Loc::Library | Loc::Plugins),
        LocFilter::Song => it.loc == Loc::Song,
        LocFilter::Favourites => favs.contains(&it.key),
        LocFilter::Folder(f) => it.loc == Loc::Folder(f.clone()),
    }
}

/// indices into `items` the filter keeps, sorted
pub fn filter(
    items: &[Item],
    f: &Filter,
    favs: &[String],
    ratings: &BTreeMap<String, u8>,
) -> Vec<usize> {
    let mut v: Vec<usize> = (0..items.len())
        .filter(|&i| {
            let it = &items[i];
            it.in_tab(f.tab)
                && matches_query(it, &f.query)
                && f.tags.iter().all(|t| it.tags.contains(t))
                && f.category.as_ref().is_none_or(|c| &it.category == c)
                && f.creator.as_ref().is_none_or(|c| &it.creator == c)
                && loc_ok(it, &f.loc, favs)
        })
        .collect();
    let rating = |i: usize| ratings.get(&items[i].key).copied().unwrap_or(0);
    let name = |i: usize| items[i].name.to_lowercase();
    match f.sort {
        Sort::Name => v.sort_by_key(|&i| name(i)),
        Sort::Type => v.sort_by_key(|&i| (items[i].fmt, items[i].category.to_lowercase(), name(i))),
        Sort::Rating => v.sort_by_key(|&i| (std::cmp::Reverse(rating(i)), name(i))),
    }
    // the ChipWave Instruments come first, in their own (the pie's) order
    let tool = |i: usize| match items[i].src {
        Src::Tool(k) if items[i].category == TOOLS_CAT => Some(k),
        _ => None,
    };
    v.sort_by_key(|&i| tool(i).map_or((1, 0), |k| (0, k)));
    v
}

/// the values of one filter column within the tab (and the other filters
/// except this column), with counts, sorted by name
pub fn column_values(
    items: &[Item],
    f: &Filter,
    favs: &[String],
    col: fn(&Item) -> &str,
    skip_cat: bool,
    skip_creator: bool,
) -> Vec<(String, usize)> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for it in items.iter().filter(|it| {
        it.in_tab(f.tab)
            && matches_query(it, &f.query)
            && f.tags.iter().all(|t| it.tags.contains(t))
            && (skip_cat || f.category.as_ref().is_none_or(|c| &it.category == c))
            && (skip_creator || f.creator.as_ref().is_none_or(|c| &it.creator == c))
            && loc_ok(it, &f.loc, favs)
    }) {
        *m.entry(col(it).to_string()).or_default() += 1;
    }
    let mut v: Vec<(String, usize)> = m.into_iter().collect();
    // ChipWave Instruments heads the Category column
    v.sort_by_key(|(c, _)| c != TOOLS_CAT);
    v
}

// ------------------------------------------------------------------ loading

/// an instrument from a sample file (any decodable format), mono, root C-4
pub fn sample_instrument(path: &Path) -> Result<Instrument, String> {
    let ext = crate::audiofile::ext_of(path);
    if ext == "wav" || ext == "wave" {
        if let Ok(i) = crate::fileio::import_wav(path) {
            return Ok(i);
        }
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let pcm = crate::audiofile::decode(&bytes, &ext)?;
    let ch = pcm.channels.max(1) as usize;
    let data: Vec<f32> = pcm
        .data
        .chunks(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect();
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
            rate: pcm.rate,
            data: data.into(),
            base_note: crate::model::parse_note("C-4").unwrap_or(49),
            lp: Default::default(),
        },
    })
}

/// a Wave generator instrument playing table `t` (built-in index, or an
/// imported table) on oscillator A
pub fn wave_instrument(
    name: &str,
    t: Option<u8>,
    user: Option<crate::wavetable::UserTable>,
) -> Instrument {
    let mut g = crate::gen::presets()
        .into_iter()
        .find(|(_, _, p)| matches!(p.engine, crate::gen::GenEngine::Wave(_)))
        .map(|(_, _, p)| p)
        .expect("a Wave preset");
    if let crate::gen::GenEngine::Wave(w) = &mut g.engine {
        match (t, user) {
            (_, Some(u)) => {
                w.user[0] = Some(u);
                w.tables[0] = crate::gen_wave::USER;
            }
            (Some(i), None) => w.tables[0] = i,
            _ => {}
        }
    }
    g.base.name = name.into();
    Instrument {
        name: name.into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Gen(g),
    }
}

/// the instrument an item stands for (not plugins, songs, effects, MIDI)
pub fn instrument_of(it: &Item, song: &Song) -> Result<Instrument, String> {
    let plain = |name: &str, kind| Instrument {
        name: name.into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind,
    };
    match &it.src {
        Src::Factory(s) => instfile::instantiate(&instfile::Entry {
            name: it.name.clone(),
            kind: "synth",
            category: it.category.clone(),
            source: s.clone(),
        }),
        Src::Init("voice") => Ok(plain("Voice", InstrumentKind::Voice(Default::default()))),
        Src::Init(_) => Ok(plain("Synth", InstrumentKind::Synth(Default::default()))),
        Src::Gen(n) => {
            let (found, n) = match n.strip_prefix("drumkit:") {
                Some(k) => (crate::drumkit::preset(k), k),
                None => (crate::gen::preset(n), n.as_str()),
            };
            let mut g = found.ok_or("unknown generator preset")?;
            g.base.name = n.to_string();
            Ok(plain(n, InstrumentKind::Gen(g)))
        }
        Src::Table(i) => Ok(wave_instrument(&it.name, Some(*i), None)),
        Src::SongInst(i) => song
            .instruments
            .get(*i)
            .cloned()
            .ok_or_else(|| "that instrument is gone".to_string()),
        Src::File(p) => match it.fmt {
            Fmt::Cwi => instfile::load_cwi(p),
            Fmt::Wavetable => {
                let u = crate::wavetable::import_wav(p)?;
                let name = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Wavetable".into());
                Ok(wave_instrument(&name, None, Some(u)))
            }
            f if f.is_sample() => sample_instrument(p),
            _ => Err(format!(
                "{} files can't be loaded as an instrument",
                it.fmt.badge()
            )),
        },
        _ => Err("not an instrument".into()),
    }
}

/// the Grid FX device a Grid FX patch item stands for
pub fn grid_fx_device(name: &str) -> Option<crate::fx::Device> {
    let (_, _, p) = crate::grid::fx_presets()
        .into_iter()
        .find(|(n, _, _)| *n == name)?;
    let mut d = crate::fx::Device::new(crate::fx::FxKind::Grid);
    d.grid = Some(Box::new(p));
    Some(d)
}

/// A one-channel song playing `inst` on one note (C-3; samples at their
/// root note) through a copy of track `ch`'s mixer channel, for the preview.
pub fn preview_song(inst: &Instrument, base: &Song, ch: usize) -> Song {
    let mut s = instfile::audition_song(inst, base);
    if let Some(c) = base.mixer.channels.get(ch) {
        let mut c = *c;
        c.mute = false;
        c.solo = false;
        if let Some(d) = s.mixer.channels.get_mut(0) {
            *d = c;
        }
    }
    s.mixer.master = base.mixer.master;
    s.mixer.returns = base.mixer.returns.clone();
    let note = match &inst.kind {
        InstrumentKind::Sample { base_note, .. } => *base_note,
        _ => crate::model::parse_note("C-3").unwrap_or(37),
    };
    if let Some(p) = s.patterns.get_mut(0) {
        for r in p.cells.iter_mut() {
            r[0] = Default::default();
        }
        p.cells[0][0] = crate::model::Cell {
            note,
            instr: 1,
            ..Default::default()
        };
        let last = p.cells.len() - 1;
        p.cells[last][0].note = crate::model::NOTE_OFF;
    }
    s
}

pub(crate) mod catalogue;

#[cfg(test)]
mod tests;
