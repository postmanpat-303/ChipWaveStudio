//! Tiny persisted app settings (scale, theme) in the per-OS config folder.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub zoom: f32,
    pub theme: String,
    /// device buffer request in frames, 0 = Auto
    #[serde(default)]
    pub buffer: u32,
    /// start maximised (default on first run)
    #[serde(default = "yes")]
    pub maximized: bool,
    /// last normal window position and inner size [x, y, w, h] (when not maximised)
    #[serde(default)]
    pub window: Option<[f32; 4]>,
    /// recent songs, newest first (max 20)
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Settings screen values (0.2.0)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extra {
    /// output device name, "" = system default
    #[serde(default)]
    pub device: String,
    /// sample rate, 0 = device default
    #[serde(default)]
    pub rate: u32,
    /// accent colour name ("" = theme default)
    #[serde(default)]
    pub accent: String,
    /// autosave interval in minutes, 0 = off
    #[serde(default = "two")]
    pub autosave_mins: u32,
    /// rotating .bak files kept on save
    #[serde(default = "one")]
    pub backups: u32,
    /// default folder for Open / Save
    #[serde(default)]
    pub song_dir: String,
    /// last export folder
    #[serde(default)]
    pub export_dir: String,
}

fn two() -> u32 {
    2
}
fn one() -> u32 {
    1
}

impl Default for Extra {
    fn default() -> Self {
        Self {
            device: String::new(),
            rate: 0,
            accent: String::new(),
            autosave_mins: 2,
            backups: 1,
            song_dir: String::new(),
            export_dir: String::new(),
        }
    }
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            theme: crate::theme::DARK.name.into(),
            buffer: 0,
            maximized: true,
            window: None,
            recent: vec![],
            extra: Extra::default(),
        }
    }
}

impl Settings {
    pub fn theme(&self) -> crate::theme::Theme {
        crate::theme::THEMES
            .iter()
            .copied()
            .find(|t| t.name == self.theme)
            .unwrap_or(crate::theme::DARK)
    }
}

/// the per-OS config folder (settings, recent files, recovery/)
pub fn config_dir() -> Option<PathBuf> {
    path().and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// Put `p` at the top of the recent list (deduplicated, max 20).
pub fn push_recent(list: &mut Vec<String>, p: &str) {
    list.retain(|x| x != p);
    list.insert(0, p.to_string());
    list.truncate(20);
}

fn path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("ChipWave"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/ChipWave"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("chipwave"))
    };
    base.map(|b| b.join("settings.toml"))
}

pub fn load() -> Settings {
    let s: Settings = path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default();
    Settings {
        zoom: if s.zoom.is_finite() {
            s.zoom.clamp(0.5, 3.0)
        } else {
            1.0
        },
        ..s
    }
}

pub fn save(s: &Settings) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(t) = toml::to_string(s) {
        let tmp = p.with_extension("toml.tmp");
        if std::fs::write(&tmp, t).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}
