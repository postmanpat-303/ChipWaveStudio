//! "Get showcase instruments…" (docs/BROWSER.md): an optional, user-started
//! download of eight free, open-source instrument plugins from their
//! creators' own release pages. Nothing here is shipped with ChipWave: the
//! manifest pins each official release URL, its size and its SHA-256; the
//! installer downloads, checks the hash, extracts only the `.vst3` / `.clap`
//! bundles (plus the plugin's own data folder where it needs one) into the
//! per-user plugin folders, and copies the licence alongside. No admin
//! rights, no third-party installers are run: a release that only ships an
//! `.exe` / `.pkg` / `.dmg` installer is listed with "Open download page".

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod tests;

/// GPL-3.0 text (ChipWave's own LICENSE) for archives that don't carry one
const GPL3: &str = include_str!("../../LICENSE");
/// Boost Software License 1.0 text (ADLplug's licence; its archive has none)
const BSL1: &str = include_str!("BSL-1.0.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Zip,
    SevenZ,
    TarGz,
}

/// where an extracted piece lands
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dest {
    /// the per-user VST3 folder
    Vst3,
    /// the per-user CLAP folder
    Clap,
    /// a folder under the per-user app-data root (the plugin's own factory data)
    Data(&'static str),
    /// a licence file: copied next to every installed bundle as `<Name>-<name>`
    Licence,
}

/// one archive path (a file, or a folder and everything under it) to take
#[derive(Clone, Copy, Debug)]
pub struct Pick {
    pub from: &'static str,
    pub dest: Dest,
    /// the name it gets at the destination (a single-file VST3 may be renamed)
    pub name: &'static str,
}

const fn p(from: &'static str, dest: Dest, name: &'static str) -> Pick {
    Pick { from, dest, name }
}

#[derive(Clone, Copy, Debug)]
pub struct Asset {
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    pub kind: Kind,
    pub picks: &'static [Pick],
}

/// what ChipWave can do for an item on this OS
#[derive(Clone, Copy, Debug)]
pub enum Get {
    /// download and install these assets
    Auto(&'static [Asset]),
    /// the creator only ships an installer (or nothing for this OS): show
    /// the reason and an "Open download page" button
    Page(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub id: &'static str,
    pub name: &'static str,
    pub creator: &'static str,
    pub category: &'static str,
    pub licence: &'static str,
    pub version: &'static str,
    pub formats: &'static str,
    /// the creator's download / release page
    pub page: &'static str,
    /// source code (GPL: corresponding source)
    pub source: &'static str,
    /// licence text written next to the plugin when its archive has none
    pub licence_text: Option<&'static str>,
    /// big download: unticked by default
    pub big: bool,
    pub note: &'static str,
    pub windows: Get,
    pub macos: Get,
    pub linux: Get,
}

impl Item {
    /// the plan for the OS ChipWave is running on
    pub fn get(&self) -> Get {
        if cfg!(windows) {
            self.windows
        } else if cfg!(target_os = "macos") {
            self.macos
        } else {
            self.linux
        }
    }
    pub fn auto(&self) -> bool {
        matches!(self.get(), Get::Auto(_))
    }
    /// download size in bytes (0 when not auto-installable)
    pub fn size(&self) -> u64 {
        match self.get() {
            Get::Auto(a) => a.iter().map(|a| a.size).sum(),
            Get::Page(_) => 0,
        }
    }
}

const GH: &str = "https://github.com/";

// ---- the manifest (pinned versions, official release assets only) --------

const SURGE_WIN: &[Asset] = &[
    Asset {
        url: "https://github.com/surge-synthesizer/releases-xt/releases/download/1.3.4/surge-xt-win64-1.3.4-pluginsonly.zip",
        sha256: "564e162c560af07ad4ed47fe1bfcd827cf97a575de30d06c48249aad2e7c35e6",
        size: 49_093_875,
        kind: Kind::Zip,
        picks: &[
            p("Surge XT.vst3", Dest::Vst3, "Surge XT.vst3"),
            p("Surge XT.clap", Dest::Clap, "Surge XT.clap"),
            p("Surge XT Effects.vst3", Dest::Vst3, "Surge XT Effects.vst3"),
            p("Surge XT Effects.clap", Dest::Clap, "Surge XT Effects.clap"),
        ],
    },
    // factory patches, wavetables and skins: Surge looks in the per-user
    // "%LOCALAPPDATA%\Surge XT" when there is no system-wide install
    Asset {
        url: "https://github.com/surge-synthesizer/releases-xt/releases/download/1.3.4/surge-xt-portable-content-1.3.4.tar.gz",
        sha256: "7ed24822eab69f73101d8558222979b1bceb4882a780292bb7087309b7b101bd",
        size: 252_763_750,
        kind: Kind::TarGz,
        picks: &[p("Surge Synth Team/SurgeXTData", Dest::Data("Surge XT"), "")],
    },
];

const DEXED_WIN: &[Asset] = &[Asset {
    url: "https://github.com/asb2m10/dexed/releases/download/v1.0.1/Dexed-1.0.1-win.zip",
    sha256: "1f118445f2c5c411636bb41ddf8b456db09af74cd38f9ede8a93a01020505f05",
    size: 9_742_538,
    kind: Kind::Zip,
    picks: &[
        p("Dexed.vst3", Dest::Vst3, "Dexed.vst3"),
        p("Dexed.clap", Dest::Clap, "Dexed.clap"),
        p("LICENSE", Dest::Licence, "LICENSE.txt"),
    ],
}];
const DEXED_MAC: &[Asset] = &[Asset {
    url: "https://github.com/asb2m10/dexed/releases/download/v1.0.1/Dexed-1.0.1-macOS.zip",
    sha256: "4269448bc1312424cafd29aff615d891bc5905ebfc54b42ce70c56b9b10dc401",
    size: 16_939_688,
    kind: Kind::Zip,
    picks: &[
        p("Dexed.vst3", Dest::Vst3, "Dexed.vst3"),
        p("Dexed.clap", Dest::Clap, "Dexed.clap"),
        p("LICENSE", Dest::Licence, "LICENSE.txt"),
    ],
}];
const DEXED_LNX: &[Asset] = &[Asset {
    url: "https://github.com/asb2m10/dexed/releases/download/v1.0.1/Dexed-1.0.1-lnx.zip",
    sha256: "2bac3d0c4237e8c22c4274b6d5b59fe6329fcae72864f81b5830393eff354fc1",
    size: 7_725_160,
    kind: Kind::Zip,
    picks: &[
        p("Dexed.vst3", Dest::Vst3, "Dexed.vst3"),
        p("Dexed.clap", Dest::Clap, "Dexed.clap"),
        p("LICENSE", Dest::Licence, "LICENSE.txt"),
    ],
}];

const OBXF_ASSETS: Asset = Asset {
    url: "https://github.com/surge-synthesizer/OB-Xf/releases/download/v1.0.3/ob-xf-assets-v1.0.3.zip",
    sha256: "6d324e520694167f22dc97912088bafea52bb6e348916056482a5f79434c6ec0",
    size: 23_966_440,
    kind: Kind::Zip,
    // patches and themes: OB-Xf's per-user "local factory" folder
    picks: &[p("Surge Synth Team/OB-Xf", Dest::Data("Surge Synth Team/OB-Xf"), "")],
};
const OBXF_WIN: &[Asset] = &[
    Asset {
        url: "https://github.com/surge-synthesizer/OB-Xf/releases/download/v1.0.3/ob-xf-Windows-v1.0.3.zip",
        sha256: "149fda0649daf8a3aa0851801f92515f0dbf6203acf33c0edc7d3c445e80b1e7",
        size: 9_222_959,
        kind: Kind::Zip,
        picks: &[
            p("obxf_products/OB-Xf.vst3", Dest::Vst3, "OB-Xf.vst3"),
            p("obxf_products/OB-Xf.clap", Dest::Clap, "OB-Xf.clap"),
            p("obxf_products/LICENSE", Dest::Licence, "LICENSE.txt"),
        ],
    },
    OBXF_ASSETS,
];
const OBXF_LNX: &[Asset] = &[
    Asset {
        url: "https://github.com/surge-synthesizer/OB-Xf/releases/download/v1.0.3/ob-xf-Linux-v1.0.3.zip",
        sha256: "72b60c83cf6426337031df744c34a047104a9d95f1feaf6cd048ecfa39f74c96",
        size: 21_867_343,
        kind: Kind::Zip,
        picks: &[
            p("obxf_products/OB-Xf.vst3", Dest::Vst3, "OB-Xf.vst3"),
            p("obxf_products/OB-Xf.clap", Dest::Clap, "OB-Xf.clap"),
            p("obxf_products/LICENSE", Dest::Licence, "LICENSE.txt"),
        ],
    },
    OBXF_ASSETS,
];

const MONIQUE_WIN: &[Asset] = &[Asset {
    url: "https://github.com/surge-synthesizer/monique-monosynth/releases/download/Nightly/MoniqueMonosynth-2024-08-07-df7d339-Windows-64bit.zip",
    sha256: "483d957388e5655639a1a43092ce0f9677e512ee016186a3110909112d99b9f4",
    size: 11_473_649,
    kind: Kind::Zip,
    picks: &[
        p("monique_products/MoniqueMonosynth.vst3", Dest::Vst3, "MoniqueMonosynth.vst3"),
        p("monique_products/MoniqueMonosynth.clap", Dest::Clap, "MoniqueMonosynth.clap"),
        p("monique_products/LICENSE-gpl3", Dest::Licence, "LICENSE.txt"),
    ],
}];
const MONIQUE_LNX: &[Asset] = &[Asset {
    url: "https://github.com/surge-synthesizer/monique-monosynth/releases/download/Nightly/MoniqueMonosynth-2024-08-07-df7d339-Linux.zip",
    sha256: "a9a9a9b42e111bd41ee24157368ca2499cace806c505f7f8ccd9aeec03f251b7",
    size: 11_138_914,
    kind: Kind::Zip,
    picks: &[
        p("monique_products/MoniqueMonosynth.vst3", Dest::Vst3, "MoniqueMonosynth.vst3"),
        p("monique_products/MoniqueMonosynth.clap", Dest::Clap, "MoniqueMonosynth.clap"),
        p("monique_products/LICENSE-gpl3", Dest::Licence, "LICENSE.txt"),
    ],
}];

const ODIN_LNX: &[Asset] = &[Asset {
    url: "https://github.com/TheWaveWarden/odin2/releases/download/NightlyDevel/Odin2.4.1Linux.zip",
    sha256: "462c4fffda99ff91a6c362d8cf41616b50807778ae7a96471753046a799aed63",
    size: 66_588_004,
    kind: Kind::Zip,
    picks: &[
        p("Odin2.vst3", Dest::Vst3, "Odin2.vst3"),
        p("Odin2.clap", Dest::Clap, "Odin2.clap"),
    ],
}];

const GEONKICK_LNX: &[Asset] = &[Asset {
    url: "https://github.com/quamplex/geonkick/releases/download/v3.9.0/GNULinux_binaries_v3.9.0_basic.zip",
    sha256: "2ac941877bea77983677861f32cd35255fd502e2d96514702bf9e749aeb0c44c",
    size: 3_767_094,
    kind: Kind::Zip,
    picks: &[p(
        "GNULinux_binaries_v3.9.0_basic/lib/vst3/Geonkick.vst3",
        Dest::Vst3,
        "Geonkick.vst3",
    )],
}];

const ADL_WIN: &[Asset] = &[Asset {
    url: "https://github.com/jpcima/ADLplug/releases/download/v1.0.0/ADLplug-1.0.0-win64.7z",
    sha256: "82f32e08d1ba4cd154a8dea7ac7cc7d61ae2534fca8c252c7b71cc692b125cb5",
    size: 18_564_457,
    kind: Kind::SevenZ,
    // a single-file VST3 (the DLL itself), renamed to .vst3
    picks: &[p("ADLplug-1.0.0-win64/vst3/ADLplug.dll", Dest::Vst3, "ADLplug.vst3")],
}];
const ADL_MAC: &[Asset] = &[Asset {
    url: "https://github.com/jpcima/ADLplug/releases/download/v1.0.0/ADLplug-1.0.0-mac.7z",
    sha256: "2390f87bbd747b9d3a017c45c2bef3fda0d0e4e8efb5bcb9ff163c0aadbf3fe7",
    size: 7_079_858,
    kind: Kind::SevenZ,
    picks: &[p("ADLplug-1.0.0-mac/vst3/ADLplug.vst3", Dest::Vst3, "ADLplug.vst3")],
}];

const INSTALLER_ONLY: &str = "the creator ships this as an installer program only";

/// the eight showcase instruments (same order as the dialog)
pub const ITEMS: &[Item] = &[
    Item {
        id: "surge-xt",
        name: "Surge XT",
        creator: "Surge Synth Team",
        category: "Hybrid",
        licence: "GPL-3.0",
        version: "1.3.4",
        formats: "CLAP + VST3 (+ Surge XT Effects)",
        page: "https://surge-synthesizer.github.io/downloads/",
        source: "https://github.com/surge-synthesizer/surge",
        licence_text: Some(GPL3),
        big: true,
        note: "large: the factory patches and wavetables are most of the download",
        windows: Get::Auto(SURGE_WIN),
        macos: Get::Page(INSTALLER_ONLY),
        linux: Get::Page("install it with the creator's package for your distribution"),
    },
    Item {
        id: "dexed",
        name: "Dexed",
        creator: "Digital Suburban / Pascal Gauthier",
        category: "FM",
        licence: "GPL-3.0",
        version: "1.0.1",
        formats: "CLAP + VST3",
        page: "https://asb2m10.github.io/dexed/",
        source: "https://github.com/asb2m10/dexed",
        licence_text: None,
        big: false,
        note: "DX7-style FM",
        windows: Get::Auto(DEXED_WIN),
        macos: Get::Auto(DEXED_MAC),
        linux: Get::Auto(DEXED_LNX),
    },
    Item {
        id: "ob-xf",
        name: "OB-Xf",
        creator: "Surge Synth Team",
        category: "Analog",
        licence: "GPL-3.0",
        version: "1.0.3",
        formats: "CLAP + VST3",
        page: "https://surge-synth-team.org/ob-xf/",
        source: "https://github.com/surge-synthesizer/OB-Xf",
        licence_text: None,
        big: false,
        note: "with its factory patches",
        windows: Get::Auto(OBXF_WIN),
        macos: Get::Page(INSTALLER_ONLY),
        linux: Get::Auto(OBXF_LNX),
    },
    Item {
        id: "odin2",
        name: "Odin 2",
        creator: "The Wave Warden",
        category: "Pad",
        licence: "GPL-3.0",
        version: "2.4.1",
        formats: "CLAP + VST3",
        page: "https://thewavewarden.com/pages/odin-2",
        source: "https://github.com/TheWaveWarden/odin2",
        licence_text: Some(GPL3),
        big: false,
        note: "",
        windows: Get::Page(INSTALLER_ONLY),
        macos: Get::Page(INSTALLER_ONLY),
        linux: Get::Auto(ODIN_LNX),
    },
    Item {
        id: "monique",
        name: "Monique",
        creator: "Thomas Arndt / Surge Synth Team",
        category: "Bass",
        licence: "GPL-3.0",
        version: "nightly 2024-08-07",
        formats: "CLAP + VST3",
        page: "https://github.com/surge-synthesizer/monique-monosynth/releases/tag/Nightly",
        source: "https://github.com/surge-synthesizer/monique-monosynth",
        licence_text: None,
        big: false,
        note: "",
        windows: Get::Auto(MONIQUE_WIN),
        macos: Get::Page(INSTALLER_ONLY),
        linux: Get::Auto(MONIQUE_LNX),
    },
    Item {
        id: "geonkick",
        name: "Geonkick",
        creator: "Iurie Nistor (Quamplex)",
        category: "Drums",
        licence: "GPL-3.0",
        version: "3.9.0 basic",
        formats: "VST3",
        page: "https://quamplex.com/geonkick/",
        source: "https://github.com/quamplex/geonkick",
        licence_text: Some(GPL3),
        big: false,
        note: "",
        windows: Get::Page(INSTALLER_ONLY),
        macos: Get::Page("no macOS build"),
        linux: Get::Auto(GEONKICK_LNX),
    },
    Item {
        id: "sfizz",
        name: "sfizz",
        creator: "SFZTools",
        category: "Sampler",
        licence: "BSD-2-Clause",
        version: "1.2.3",
        formats: "VST3",
        page: "https://sfz.tools/sfizz/downloads/",
        source: "https://github.com/sfztools/sfizz",
        licence_text: None,
        big: false,
        note: "needs an SFZ sound set to make sound",
        windows: Get::Page(INSTALLER_ONLY),
        macos: Get::Page(INSTALLER_ONLY),
        linux: Get::Page("install it with your distribution's package"),
    },
    Item {
        id: "adlplug",
        name: "ADLplug",
        creator: "Jean Pierre Cimalando",
        category: "Chip",
        licence: "BSL-1.0",
        version: "1.0.0",
        formats: "VST3",
        page: "https://github.com/jpcima/ADLplug/releases/tag/v1.0.0",
        source: "https://github.com/jpcima/ADLplug",
        licence_text: Some(BSL1),
        big: false,
        note: "OPL3 FM chip",
        windows: Get::Auto(ADL_WIN),
        macos: Get::Auto(ADL_MAC),
        linux: Get::Page("the creator ships a .deb package only"),
    },
];

pub fn item(id: &str) -> Option<&'static Item> {
    ITEMS.iter().find(|i| i.id == id)
}

/// every URL in the manifest points at the creator's own GitHub releases
pub fn official(url: &str) -> bool {
    url.starts_with(GH) && url.contains("/releases/download/")
}

// ---- where things go ------------------------------------------------------

/// the folders an install writes to (per user; overridable for tests)
#[derive(Clone, Debug)]
pub struct Roots {
    pub vst3: PathBuf,
    pub clap: PathBuf,
    /// per-user app data (Windows %LOCALAPPDATA%, macOS Application Support,
    /// Linux $XDG_DATA_HOME or ~/.local/share)
    pub data: PathBuf,
    /// downloads in progress (deleted after each install)
    pub cache: PathBuf,
    /// what's installed: `showcase-installed.json`
    pub record: PathBuf,
}

impl Roots {
    /// the standard per-user folders for this OS (None if HOME / LOCALAPPDATA is unset)
    pub fn user() -> Option<Roots> {
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        let (vst3, clap, data) = if cfg!(windows) {
            let l = env("LOCALAPPDATA")?;
            let c = l.join("Programs").join("Common");
            (c.join("VST3"), c.join("CLAP"), l)
        } else if cfg!(target_os = "macos") {
            let h = env("HOME")?;
            let pl = h.join("Library/Audio/Plug-Ins");
            (pl.join("VST3"), pl.join("CLAP"), h.join("Library/Application Support"))
        } else {
            let h = env("HOME")?;
            let d = env("XDG_DATA_HOME").unwrap_or_else(|| h.join(".local/share"));
            (h.join(".vst3"), h.join(".clap"), d)
        };
        let cfg = crate::settings::config_dir()
            .or_else(|| Some(std::env::temp_dir().join("ChipWave")))?;
        Some(Roots {
            vst3,
            clap,
            data,
            cache: cfg.join("showcase-downloads"),
            record: cfg.join("showcase-installed.json"),
        })
    }
    /// the same layout under one folder (tests, `--install-showcase --root`)
    pub fn under(base: &Path) -> Roots {
        Roots {
            vst3: base.join("VST3"),
            clap: base.join("CLAP"),
            data: base.join("data"),
            cache: base.join("cache"),
            record: base.join("showcase-installed.json"),
        }
    }
    fn dir(&self, d: Dest) -> PathBuf {
        match d {
            Dest::Vst3 => self.vst3.clone(),
            Dest::Clap => self.clap.clone(),
            Dest::Data(sub) => self.data.join(sub),
            Dest::Licence => self.vst3.clone(),
        }
    }
}

// ---- install record ---------------------------------------------------------

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Installed {
    pub version: String,
    /// top-level paths written (bundles, data folders, licence files)
    pub files: Vec<String>,
}

pub fn load_record(r: &Roots) -> BTreeMap<String, Installed> {
    std::fs::read_to_string(&r.record)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_record(r: &Roots, m: &BTreeMap<String, Installed>) -> Result<(), String> {
    if let Some(d) = r.record.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let s = serde_json::to_string_pretty(m).unwrap_or_default();
    std::fs::write(&r.record, s).map_err(|e| format!("couldn't save the install list ({e})"))
}

/// this exact version is installed and all its files are still there
pub fn is_installed(r: &Roots, it: &Item) -> bool {
    load_record(r).get(it.id).is_some_and(|i| {
        i.version == it.version && !i.files.is_empty() && i.files.iter().all(|f| Path::new(f).exists())
    })
}

// ---- progress -----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Waiting,
    Downloading,
    Verifying,
    Installing,
    Done,
    /// already installed (same version)
    Skipped,
    Failed(String),
    Cancelled,
}

impl State {
    pub fn finished(&self) -> bool {
        matches!(
            self,
            State::Done | State::Skipped | State::Failed(_) | State::Cancelled
        )
    }
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub id: &'static str,
    pub state: State,
    pub done: u64,
    pub total: u64,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        match self.state {
            State::Done | State::Skipped => 1.0,
            _ if self.total > 0 => (self.done as f32 / self.total as f32).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

/// downloads a URL into `out`, reporting (bytes so far, expected total)
pub trait Fetch: Send + Sync {
    fn fetch(
        &self,
        url: &str,
        size: u64,
        out: &mut dyn Write,
        on: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<(), String>;
}

/// the real thing: HTTPS with ureq (follows GitHub's redirect to its CDN)
pub struct Http;

impl Fetch for Http {
    fn fetch(
        &self,
        url: &str,
        size: u64,
        out: &mut dyn Write,
        on: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        use std::time::Duration;
        if !official(url) {
            return Err(format!("not an official release URL: {url}"));
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .user_agent(format!("ChipWave-Studio/{}", crate::VERSION))
            .build()
            .into();
        let resp = agent.get(url).call().map_err(|e| match e {
            ureq::Error::StatusCode(404) => {
                "the creator's download is gone (HTTP 404): use Open download page".to_string()
            }
            ureq::Error::StatusCode(c) => format!("the download server answered HTTP {c}"),
            ureq::Error::Timeout(_) => "the download server did not answer in time".into(),
            e => format!("no connection ({e})"),
        })?;
        let mut rd = resp
            .into_body()
            .into_with_config()
            .limit(size + (1 << 20))
            .reader();
        let mut buf = vec![0u8; 256 * 1024];
        let mut n = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(CANCELLED.into());
            }
            let k = rd
                .read(&mut buf)
                .map_err(|e| format!("the download stopped ({e})"))?;
            if k == 0 {
                break;
            }
            out.write_all(&buf[..k])
                .map_err(|e| format!("couldn't write the download ({e})"))?;
            n += k as u64;
            on(n);
        }
        Ok(())
    }
}

const CANCELLED: &str = "cancelled";

/// a running install (background thread); the dialog polls `progress`
pub struct Job {
    pub progress: Arc<Mutex<Vec<Progress>>>,
    pub cancel: Arc<AtomicBool>,
    pub done: Arc<AtomicBool>,
}

impl Job {
    pub fn finished(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }
}

/// install `ids` one after another on a background thread
pub fn spawn(ids: Vec<&'static str>, roots: Roots, fetch: Arc<dyn Fetch>) -> Job {
    let progress = Arc::new(Mutex::new(
        ids.iter()
            .map(|id| Progress {
                id,
                state: State::Waiting,
                done: 0,
                total: item(id).map(|i| i.size()).unwrap_or(0),
            })
            .collect::<Vec<_>>(),
    ));
    let cancel = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let (p2, c2, d2) = (progress.clone(), cancel.clone(), done.clone());
    let ran = std::thread::Builder::new()
        .name("showcase-install".into())
        .spawn(move || {
            run(&ids, &roots, &*fetch, &c2, &p2);
            d2.store(true, Ordering::Relaxed);
        });
    if let Err(e) = ran {
        for pr in progress.lock().unwrap().iter_mut() {
            pr.state = State::Failed(format!("couldn't start the download ({e})"));
        }
        done.store(true, Ordering::Relaxed);
    }
    Job {
        progress,
        cancel,
        done,
    }
}

/// install each item in turn, updating `progress` (also the CLI path)
pub fn run(
    ids: &[&'static str],
    roots: &Roots,
    fetch: &dyn Fetch,
    cancel: &AtomicBool,
    progress: &Mutex<Vec<Progress>>,
) {
    for (k, id) in ids.iter().enumerate() {
        let set = |s: State| {
            if let Some(p) = progress.lock().unwrap().get_mut(k) {
                p.state = s;
            }
        };
        if cancel.load(Ordering::Relaxed) {
            set(State::Cancelled);
            continue;
        }
        let Some(it) = item(id) else {
            set(State::Failed(format!("unknown instrument {id}")));
            continue;
        };
        let r = install(it, roots, fetch, cancel, &mut |s, done| {
            if let Some(p) = progress.lock().unwrap().get_mut(k) {
                p.state = s;
                if let Some(d) = done {
                    p.done = d;
                }
            }
        });
        match r {
            Ok(true) => set(State::Done),
            Ok(false) => set(State::Skipped),
            Err(e) if e == CANCELLED => set(State::Cancelled),
            Err(e) => set(State::Failed(e)),
        }
    }
}

/// Download, verify and install one item. Ok(false) = already installed.
pub fn install(
    it: &Item,
    roots: &Roots,
    fetch: &dyn Fetch,
    cancel: &AtomicBool,
    on: &mut dyn FnMut(State, Option<u64>),
) -> Result<bool, String> {
    let Get::Auto(assets) = it.get() else {
        return Err(format!("{} can't be installed automatically here: use Open download page", it.name));
    };
    if is_installed(roots, it) {
        return Ok(false);
    }
    std::fs::create_dir_all(&roots.cache)
        .map_err(|e| format!("couldn't create the download folder {} ({e})", roots.cache.display()))?;
    // staging folders inside each destination root, so the final step is a rename
    let stage_of = |d: Dest| -> PathBuf {
        let base = match d {
            Dest::Vst3 | Dest::Licence => roots.vst3.clone(),
            Dest::Clap => roots.clap.clone(),
            Dest::Data(_) => roots.data.clone(),
        };
        base.join(format!(".chipwave-staging-{}", it.id))
    };
    let mut stages: Vec<PathBuf> = vec![];
    let cleanup = |stages: &[PathBuf]| {
        for s in stages {
            let _ = std::fs::remove_dir_all(s);
        }
    };
    let mut downloads: Vec<PathBuf> = vec![];
    let mut base = 0u64;
    // (staged path, final path)
    let mut moves: Vec<(PathBuf, PathBuf)> = vec![];
    let mut licences: Vec<(PathBuf, String)> = vec![]; // staged licence files
    let res = (|| -> Result<(), String> {
        for a in assets {
            // ---- download (or reuse a verified earlier download)
            let file = roots.cache.join(
                a.url.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("download"),
            );
            downloads.push(file.clone());
            let good = file.exists() && sha256_file(&file).is_ok_and(|h| h == a.sha256);
            if !good {
                on(State::Downloading, Some(base));
                let part = file.with_extension("part");
                let mut f = std::fs::File::create(&part)
                    .map_err(|e| format!("couldn't create {} ({e})", part.display()))?;
                let r = fetch.fetch(a.url, a.size, &mut f, &mut |n| on(State::Downloading, Some(base + n)), cancel);
                drop(f);
                if let Err(e) = r {
                    let _ = std::fs::remove_file(&part);
                    return Err(e);
                }
                on(State::Verifying, None);
                let h = sha256_file(&part).map_err(|e| format!("couldn't read the download ({e})"))?;
                if h != a.sha256 {
                    let _ = std::fs::remove_file(&part);
                    return Err(format!(
                        "the download didn't match the expected checksum (SHA-256): not installed. The creator may have replaced the file; use Open download page."
                    ));
                }
                std::fs::rename(&part, &file).map_err(|e| format!("couldn't keep the download ({e})"))?;
            }
            base += a.size;
            on(State::Installing, Some(base));
            // ---- extract the picks into staging
            for pk in a.picks {
                let st = stage_of(pk.dest);
                if !stages.contains(&st) {
                    let _ = std::fs::remove_dir_all(&st);
                    std::fs::create_dir_all(&st)
                        .map_err(|e| format!("couldn't write to {} ({e})", st.display()))?;
                    stages.push(st);
                }
            }
            let targets: Vec<(Pick, PathBuf)> = a
                .picks
                .iter()
                .map(|pk| {
                    let leaf = if pk.name.is_empty() {
                        match pk.dest {
                            Dest::Data(sub) => sub.replace('/', "_"),
                            _ => pk.from.rsplit('/').next().unwrap_or(pk.from).to_string(),
                        }
                    } else {
                        pk.name.to_string()
                    };
                    (*pk, stage_of(pk.dest).join(leaf))
                })
                .collect();
            let n = extract(&file, a.kind, &targets, cancel)?;
            for (pk, staged) in &targets {
                if !staged.exists() {
                    return Err(format!(
                        "the download didn't contain {} (unexpected layout)",
                        pk.from
                    ));
                }
                match pk.dest {
                    Dest::Licence => licences.push((staged.clone(), pk.name.to_string())),
                    d => {
                        let fin = match d {
                            Dest::Data(sub) => roots.data.join(sub),
                            _ => roots.dir(d).join(staged.file_name().unwrap()),
                        };
                        moves.push((staged.clone(), fin));
                    }
                }
            }
            let _ = n; // entries written (the existence check above is what matters)
        }
        Ok(())
    })();
    if let Err(e) = res {
        cleanup(&stages);
        return Err(e);
    }
    if cancel.load(Ordering::Relaxed) {
        cleanup(&stages);
        return Err(CANCELLED.into());
    }
    // ---- move into place (replacing an older version)
    let mut files = vec![];
    let r = (|| -> Result<(), String> {
        for (st, fin) in &moves {
            if let Some(par) = fin.parent() {
                std::fs::create_dir_all(par)
                    .map_err(|e| format!("couldn't create {} ({e})", par.display()))?;
            }
            if fin.is_dir() {
                std::fs::remove_dir_all(fin)
            } else if fin.exists() {
                std::fs::remove_file(fin)
            } else {
                Ok(())
            }
            .map_err(|e| {
                format!(
                    "couldn't replace {} — is it open in another program? ({e})",
                    fin.display()
                )
            })?;
            std::fs::rename(st, fin).or_else(|_| copy_tree(st, fin)).map_err(|e| {
                format!("couldn't install {} ({e})", fin.display())
            })?;
            files.push(fin.display().to_string());
        }
        // licence + source notes next to every plugin folder we wrote to
        let mut text: Vec<(String, Vec<u8>)> = vec![];
        for (st, name) in &licences {
            text.push((name.clone(), std::fs::read(st).unwrap_or_default()));
        }
        if text.is_empty() {
            if let Some(t) = it.licence_text {
                text.push(("LICENSE.txt".into(), t.as_bytes().to_vec()));
            }
        }
        let src = source_note(it);
        let mut roots_used: Vec<PathBuf> = vec![];
        for (_, fin) in &moves {
            for r in [&roots.vst3, &roots.clap] {
                if fin.starts_with(r) && !roots_used.contains(r) {
                    roots_used.push(r.clone());
                }
            }
        }
        let stem = it.name.replace(' ', "");
        for r in roots_used {
            for (name, bytes) in &text {
                let f = r.join(format!("{stem}-{name}"));
                std::fs::write(&f, bytes).map_err(|e| format!("couldn't write {} ({e})", f.display()))?;
                files.push(f.display().to_string());
            }
            let f = r.join(format!("{stem}-SOURCE.txt"));
            std::fs::write(&f, &src).map_err(|e| format!("couldn't write {} ({e})", f.display()))?;
            files.push(f.display().to_string());
        }
        Ok(())
    })();
    cleanup(&stages);
    r?;
    let mut rec = load_record(roots);
    rec.insert(
        it.id.to_string(),
        Installed {
            version: it.version.to_string(),
            files,
        },
    );
    save_record(roots, &rec)?;
    for d in downloads {
        let _ = std::fs::remove_file(d);
    }
    Ok(true)
}

/// the "where's the source" note written next to each plugin (GPL offer)
pub fn source_note(it: &Item) -> String {
    let mut s = format!(
        "{} {} by {}\nLicence: {}\nSource code: {}\nDownload page: {}\n",
        it.name, it.version, it.creator, it.licence, it.source, it.page
    );
    if let Get::Auto(a) = it.get() {
        for a in a {
            s += &format!("Downloaded from: {}\nSHA-256: {}\n", a.url, a.sha256);
        }
    }
    s += "\nInstalled by ChipWave Studio (Browser › Plugins › Get showcase instruments…).\n\
          ChipWave did not build or change this plugin; it is the creator's own release.\n";
    s
}

pub fn sha256_file(p: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// normalise an archive path ("./a\\b" → "a/b"); None if it escapes ("..", absolute)
pub fn clean(name: &str) -> Option<String> {
    let s = name.replace('\\', "/");
    let s = s.trim_start_matches("./");
    let mut out = vec![];
    for c in Path::new(s).components() {
        match c {
            Component::Normal(x) => out.push(x.to_str()?.to_string()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out.join("/"))
}

/// where an archive entry goes, if it is part of a pick
fn target_for(name: &str, targets: &[(Pick, PathBuf)]) -> Option<PathBuf> {
    let n = clean(name)?;
    for (pk, t) in targets {
        if n == pk.from {
            return Some(t.clone());
        }
        if let Some(rest) = n.strip_prefix(pk.from).and_then(|r| r.strip_prefix('/')) {
            if rest.is_empty() {
                return Some(t.clone());
            }
            return Some(t.join(rest));
        }
    }
    None
}

fn write_entry(
    to: &Path,
    dir: bool,
    mode: Option<u32>,
    rd: &mut dyn Read,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if dir {
        return std::fs::create_dir_all(to).map_err(|e| format!("couldn't create {} ({e})", to.display()));
    }
    if let Some(par) = to.parent() {
        std::fs::create_dir_all(par).map_err(|e| format!("couldn't create {} ({e})", par.display()))?;
    }
    let mut f = std::fs::File::create(to).map_err(|e| format!("couldn't write {} ({e})", to.display()))?;
    let mut buf = vec![0u8; 128 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        let k = rd.read(&mut buf).map_err(|e| format!("the archive is damaged ({e})"))?;
        if k == 0 {
            break;
        }
        f.write_all(&buf[..k]).map_err(|e| format!("couldn't write {} ({e})", to.display()))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // plugin binaries must stay executable; unknown modes get rwxr-xr-x
        let m = mode.map(|m| m & 0o777).filter(|m| *m != 0).unwrap_or(0o755);
        let _ = std::fs::set_permissions(to, std::fs::Permissions::from_mode(m | 0o600));
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

/// extract only the picked paths; returns the number of entries written
pub fn extract(
    file: &Path,
    kind: Kind,
    targets: &[(Pick, PathBuf)],
    cancel: &AtomicBool,
) -> Result<usize, String> {
    let mut n = 0;
    let damaged = |e: &dyn std::fmt::Display| format!("couldn't open the download as an archive ({e})");
    match kind {
        Kind::Zip => {
            let f = std::fs::File::open(file).map_err(|e| damaged(&e))?;
            let mut z = zip::ZipArchive::new(std::io::BufReader::new(f)).map_err(|e| damaged(&e))?;
            for i in 0..z.len() {
                let mut e = z.by_index(i).map_err(|e| damaged(&e))?;
                let name = e.name().map_err(|e| damaged(&e))?.to_string();
                let Some(to) = target_for(&name, targets) else {
                    continue;
                };
                let dir = e.is_dir();
                let mode = e.unix_mode();
                write_entry(&to, dir, mode, &mut e, cancel)?;
                n += 1;
            }
        }
        Kind::TarGz => {
            let f = std::fs::File::open(file).map_err(|e| damaged(&e))?;
            let gz = flate2::read::GzDecoder::new(std::io::BufReader::new(f));
            let mut t = tar::Archive::new(gz);
            for e in t.entries().map_err(|e| damaged(&e))? {
                let mut e = e.map_err(|e| damaged(&e))?;
                let name = e.path().map_err(|e| damaged(&e))?.to_string_lossy().to_string();
                let Some(to) = target_for(&name, targets) else {
                    continue;
                };
                let ty = e.header().entry_type();
                if !(ty.is_file() || ty.is_dir()) {
                    continue; // no links / devices
                }
                let mode = e.header().mode().ok();
                write_entry(&to, ty.is_dir(), mode, &mut e, cancel)?;
                n += 1;
            }
        }
        Kind::SevenZ => {
            let mut r = sevenz_rust2::ArchiveReader::open(file, sevenz_rust2::Password::empty())
                .map_err(|e| damaged(&e))?;
            let mut err: Option<String> = None;
            r.for_each_entries(|e, rd| {
                let Some(to) = target_for(e.name(), targets) else {
                    // drain: solid archives decode in order
                    std::io::copy(rd, &mut std::io::sink())?;
                    return Ok(true);
                };
                let attr = e.windows_attributes();
                let mode = (attr & 0x8000 != 0).then_some(attr >> 16);
                match write_entry(&to, e.is_directory(), mode, rd, cancel) {
                    Ok(()) => {
                        n += 1;
                        Ok(true)
                    }
                    Err(x) => {
                        err = Some(x);
                        Ok(false)
                    }
                }
            })
            .map_err(|e| damaged(&e))?;
            if let Some(e) = err {
                return Err(e);
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.into());
    }
    Ok(n)
}

/// human size ("9.7 MB")
pub fn mb(b: u64) -> String {
    if b >= 1_000_000_000 {
        format!("{:.1} GB", b as f64 / 1e9)
    } else {
        format!("{:.1} MB", b as f64 / 1e6)
    }
}
