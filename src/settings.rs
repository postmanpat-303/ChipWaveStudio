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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            theme: crate::theme::DARK.name.into(),
            buffer: 0,
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
