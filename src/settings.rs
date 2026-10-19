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
    /// extra plugin folders to scan (CLAP / VST3), besides the standard ones (0.2.3.16)
    #[serde(default)]
    pub plugin_dirs: Vec<String>,
    /// pattern editor: tint every other channel column (0.2.3.16)
    #[serde(default = "yes")]
    pub alt_columns: bool,
    /// pattern row numbers in hex (00..3F) instead of decimal (00..63) (0.2.3.17)
    #[serde(default)]
    pub row_hex: bool,
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
    /// MIDI keyboard input (0.2.3)
    #[serde(default)]
    pub midi: crate::midi::MidiSettings,
    /// quiet update check at startup, at most once per 24 h (0.2.3.5)
    #[serde(default = "yes")]
    pub update_check: bool,
    /// Unix time of the last startup update check
    #[serde(default)]
    pub update_last: u64,
    /// version the user chose to skip ("Skip this version"), "" = none
    #[serde(default)]
    pub update_skip: String,
    /// instrument library folder, "" = <config>/Instruments (0.2.3.9)
    #[serde(default)]
    pub library_dir: String,
    /// snap typed and MIDI notes to the song key (chord helper)
    #[serde(default)]
    pub scale_lock: bool,
    /// lower panel height in points (0 = automatic) (0.2.3.19)
    #[serde(default)]
    pub lower_h: f32,
    /// the lower panel shows the instrument editor (else arranger / automation);
    /// the instrument editor is the default (Unreleased), see `lower_chosen`
    #[serde(default = "yes")]
    pub lower_inst: bool,
    /// the user picked the lower panel's view themselves (Arranger | Instrument |
    /// Automation switch, Ctrl+I, ...). Until then it opens on the instrument
    /// editor, also for settings files from 0.2.3.19 / 0.2.4.0, which always
    /// wrote `lower_inst = false` whether or not anyone chose the arranger
    #[serde(default)]
    pub lower_chosen: bool,
    /// Full-height pattern: the lower panel is hidden
    #[serde(default)]
    pub full_height: bool,
    /// the pre-0.2.3.19 instrument editor in the sidebar
    #[serde(default)]
    pub classic_editor: bool,
    /// collapsed instrument-editor cards, "Type/Card"
    #[serde(default)]
    pub cards_closed: Vec<String>,
    /// File > New opens the song on the Timeline view (Del's choice, 19 Oct
    /// 2026; docs/TIMELINE.md). Off: on the pattern editor, as before.
    #[serde(default = "yes")]
    pub new_in_timeline: bool,
    /// Timeline: double-clicking a clip opens it in the piano roll (Del's
    /// choice, 19 Oct 2026). Off: the block in the pattern editor, as
    /// before. Alt+double-click does the other one; Enter always opens the
    /// pattern editor.
    #[serde(default = "yes")]
    pub dblclick_roll: bool,
    /// Interface style: false = Polished (default), true = Flat (the look
    /// before the polish pass)
    #[serde(default)]
    pub flat_ui: bool,
    /// piano roll chord lane: right-click opens a list menu instead of the
    /// pie (Settings › Chord menus)
    #[serde(default)]
    pub chord_list_menu: bool,
    /// the file browser on the right is shown (Ctrl+B; docs/BROWSER.md)
    #[serde(default = "yes")]
    pub browser_open: bool,
    /// its width in points
    #[serde(default = "browser_w")]
    pub browser_w: f32,
    /// folders added to the browser's Locations (scanned in the background)
    #[serde(default)]
    pub browser_dirs: Vec<String>,
    /// favourite items (browser item keys)
    #[serde(default)]
    pub browser_favs: Vec<String>,
    /// star ratings 1-5 per browser item key
    #[serde(default)]
    pub browser_ratings: std::collections::BTreeMap<String, u8>,
    /// selecting a sample or sound plays it
    #[serde(default = "yes")]
    pub browser_autoplay: bool,
    /// the plugin setup ("Find plugins…") was shown once (first run, or
    /// the one-time prompt for existing users)
    #[serde(default)]
    pub plugin_setup_done: bool,
    /// the left Inspector is shown (I; docs/INSPECTOR.md)
    #[serde(default)]
    pub inspector_open: bool,
    /// the view tab bar at the bottom is shown (View menu)
    #[serde(default = "yes")]
    pub view_bar: bool,
}

fn browser_w() -> f32 {
    340.0
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
            plugin_dirs: vec![],
            alt_columns: true,
            row_hex: false,
            device: String::new(),
            rate: 0,
            accent: String::new(),
            autosave_mins: 2,
            backups: 1,
            song_dir: String::new(),
            export_dir: String::new(),
            midi: Default::default(),
            update_check: true,
            update_last: 0,
            update_skip: String::new(),
            library_dir: String::new(),
            scale_lock: false,
            lower_h: 0.0,
            lower_inst: true,
            lower_chosen: false,
            full_height: false,
            classic_editor: false,
            cards_closed: vec![],
            new_in_timeline: true,
            dblclick_roll: true,
            flat_ui: false,
            chord_list_menu: false,
            browser_open: true,
            browser_w: browser_w(),
            browser_dirs: vec![],
            browser_favs: vec![],
            browser_ratings: Default::default(),
            browser_autoplay: true,
            plugin_setup_done: false,
            inspector_open: false,
            view_bar: true,
        }
    }
}

fn yes() -> bool {
    true
}

impl Extra {
    /// no saved choice of lower-panel view: open on the instrument editor
    pub fn default_lower_view(&mut self) {
        if !self.lower_chosen {
            self.lower_inst = true;
        }
    }
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
    // unit / UI tests never read or write the real settings
    if cfg!(test) {
        return None;
    }
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
    let mut s: Settings = path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default();
    s.extra.default_lower_view();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_settings_roundtrip() {
        let mut s = Settings::default();
        s.extra.midi.device = "Keystation 49".into();
        s.extra.midi.curve = crate::midi::Curve::Soft;
        s.extra.midi.octave = -1;
        s.extra.midi.learn = vec![
            (1, "instrument.2.cutoff".into()),
            (7, "mixer.ch.1.volume".into()),
        ];
        let txt = toml::to_string(&s).unwrap();
        let back: Settings = toml::from_str(&txt).unwrap();
        assert_eq!(back.extra, s.extra);
        // old settings files without [midi] still load
        let old: Settings = toml::from_str("zoom = 1.0\ntheme = \"Dark\"\n").unwrap();
        assert_eq!(old.extra.midi, crate::midi::MidiSettings::default());
    }

    #[test]
    fn lower_panel_opens_on_the_instrument_editor_unless_the_user_chose() {
        // new users: no settings at all
        assert!(Extra::default().lower_inst && !Extra::default().lower_chosen);
        // 0.2.3.19 / 0.2.4.0 settings always wrote lower_inst = false
        let mut e: Extra = toml::from_str("lower_inst = false").unwrap();
        e.default_lower_view();
        assert!(e.lower_inst, "no saved choice: the instrument editor");
        // a choice made in this version is kept, both ways
        for inst in [false, true] {
            let mut s = Settings::default();
            s.extra.lower_inst = inst;
            s.extra.lower_chosen = true;
            let mut back: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
            back.extra.default_lower_view();
            assert_eq!(back.extra.lower_inst, inst);
        }
    }
}
