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
    /// the lower panel shows Devices: the selected track's instrument and
    /// effect chain (0.2.4)
    #[serde(default)]
    pub lower_dev: bool,
    /// the lower panel follows what you click on the Timeline: a track
    /// header shows its instrument's editor, a clip its clip editor
    /// (docs/unified/PLAN.md 3.2)
    #[serde(default = "yes")]
    pub follow_select: bool,
    /// right-click on a clip, launcher clip or drum step grid opens a list
    /// menu instead of the pie (Settings › Clip menus)
    #[serde(default)]
    pub clip_list_menu: bool,
    /// Full-height pattern: the lower panel is hidden
    #[serde(default)]
    pub full_height: bool,
    /// StringWave panel text size in % (100 / 125 / 150 / 200; 0 = 100):
    /// scales only that panel, on top of the UI zoom
    #[serde(default)]
    pub sw_text: u16,
    /// Settings › Look › Text size in % (90–150, 5 % steps; 0 = 100):
    /// every font, on top of the UI scale (docs/UI_SCALING.md)
    #[serde(default)]
    pub text_size: u16,
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
    /// Opening a tracker-mode song (a .cwt, a demo) switches it to timeline
    /// mode on load, without the dialog (Del's choice, 19 Oct 2026: "default
    /// is timeline mode"). Off: it stays in tracker mode and the first
    /// timeline edit asks, as before.
    #[serde(default = "yes")]
    pub open_in_timeline: bool,
    /// Settings › Look › Reduce motion: pie menus (the track pie) appear
    /// and close without animation, and About opens without its rain
    #[serde(default)]
    pub reduce_motion: bool,
    /// Settings › Look › Pie menus: 0 Normal (30 % bigger than before 25
    /// Oct 2026, the default), 1 Small (the old size), 2 Large
    #[serde(default)]
    pub pie_size: u8,
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
    /// Plugins tab: show editor thumbnails (captured or generated cards)
    #[serde(default = "yes")]
    pub browser_thumbs: bool,
    /// Drum Patterns preview volume 0..1.5 (docs/BROWSER.md)
    #[serde(default = "pattern_vol")]
    pub pattern_vol: f32,
    /// the Files tab's last folder (empty = Home)
    #[serde(default)]
    pub files_dir: String,
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
    /// the tracker's Advanced Edit panel is shown (Ctrl+Shift+A; docs/ADVANCED_EDIT.md)
    #[serde(default)]
    pub adv_edit_open: bool,
    /// ... as a floating window instead of docked right of the pattern
    #[serde(default)]
    pub adv_edit_float: bool,
    /// where the Generators panel was dragged to (its top-left, in
    /// points); None: top-centre (docs/CHORDS.md)
    #[serde(default)]
    pub gen_pos: Option<[f32; 2]>,
    /// the timeline's default track height in points (0 = the built-in one)
    #[serde(default)]
    pub track_h: f32,
    /// Keys → Notes (musical typing) as last switched by the user; None:
    /// never switched, so it starts on when no MIDI input is found
    #[serde(default)]
    pub keys_notes: Option<bool>,
    /// the Clip Arranger replaces the left Instruments list on the Timeline
    /// (Ctrl+8, View menu; docs/CLIP_ARRANGER.md)
    #[serde(default = "yes")]
    pub clip_arranger: bool,
    /// the Timeline's left panel (Clip Arranger / Instruments list) hidden
    /// for room: the Timeline's "Arranger" button and Ctrl+Shift+8
    #[serde(default)]
    pub side_hidden: bool,
    /// Settings › Show Tracker (experimental), off by default (Del, 20 Oct
    /// 2026: focus on the Timeline and the piano roll). Off: the TRACKER /
    /// ORDER tabs and the tracker-mode menu items are hidden (View › Tracker
    /// and Ctrl+1 still open the Tracker) and every song opens in timeline
    /// mode. The code and the file format stay.
    #[serde(default)]
    pub show_tracker: bool,
    /// metronome click while the timeline plays (and the record count-in)
    #[serde(default)]
    pub metronome: bool,
    /// older settings: Record from stopped counts in one bar (with the
    /// metronome on). Kept so old files load and older versions can read
    /// new ones; `count_in_bars` is the setting now
    #[serde(default = "yes")]
    pub count_in: bool,
    /// bars Record from stopped counts in, clicking even with the metronome
    /// off: 0 (off), 1 or 2. None (older files): 1 bar, or off when the old
    /// `count_in` was off. Read it with [`Extra::count_in_bars`]
    #[serde(default)]
    pub count_in_bars: Option<u8>,
    /// recorded note starts snap to the piano roll's grid
    #[serde(default)]
    pub rec_quantise: bool,
    /// Clip Arranger launch quantise: 0 one bar, 1 half a bar, 2 a beat, 3 off
    #[serde(default)]
    pub launch_q: u8,
    /// selecting a track header or an instrument opens Nested Device
    /// Chains (docs/racks/PLAN.md D2); off: the Instrument tab, as in 0.2.6
    #[serde(default = "yes")]
    pub select_chains: bool,
    /// Nested Device Chains: the tree column's width in points
    #[serde(default = "tree_w")]
    pub chains_tree_w: f32,
    /// ... folded to a slim strip of names
    #[serde(default)]
    pub chains_tree_folded: bool,
    /// ... shows the card row instead of the tree; Cards is the default
    /// (Del, 22 Oct 2026; was the tree in 0.2.7.0)
    #[serde(default = "yes")]
    pub chains_cards: bool,
    /// the user picked Cards or Tree themselves (kept); without it a saved
    /// 0.2.7.0 file's chains_cards = false gives way to the Cards default
    #[serde(default)]
    pub chains_view_chosen: bool,
    /// the Master panel's LEVEL history: 0 scrolling, 1 paused, 2 hidden
    #[serde(default)]
    pub level_hist: u8,
    /// tree rows the user closed ("i3" instrument 3, "s3.1" a Split, ...)
    #[serde(default)]
    pub chains_closed: Vec<String>,
    /// the toolbar's LCD display with the compact Master (0.2.10); off:
    /// the classic toolbar and the Master panel under the Clip Arranger
    #[serde(default = "yes")]
    pub lcd_toolbar: bool,
    /// the LCD's colour: a preset name ("Cyan", "Amber", ...) or "#RRGGBB";
    /// "" = Cyan
    #[serde(default)]
    pub lcd_colour: String,
    /// the top bar's panel order, left to right ("display", "transport",
    /// "master"); empty: the default (Display, Transport centred, Master)
    #[serde(default)]
    pub topbar: Vec<String>,
}

fn tree_w() -> f32 {
    370.0
}

fn pattern_vol() -> f32 {
    0.8
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
            lower_dev: false,
            follow_select: true,
            clip_list_menu: false,
            full_height: false,
            sw_text: 0,
            text_size: 0,
            classic_editor: false,
            cards_closed: vec![],
            new_in_timeline: true,
            open_in_timeline: true,
            reduce_motion: false,
            pie_size: 0,
            dblclick_roll: true,
            flat_ui: false,
            chord_list_menu: false,
            browser_open: true,
            browser_w: browser_w(),
            browser_dirs: vec![],
            browser_favs: vec![],
            browser_ratings: Default::default(),
            browser_autoplay: true,
            browser_thumbs: true,
            pattern_vol: pattern_vol(),
            files_dir: String::new(),
            plugin_setup_done: false,
            inspector_open: false,
            view_bar: true,
            adv_edit_open: false,
            adv_edit_float: false,
            gen_pos: None,
            track_h: 0.0,
            keys_notes: None,
            clip_arranger: true,
            side_hidden: false,
            show_tracker: false,
            metronome: false,
            count_in: true,
            count_in_bars: None,
            rec_quantise: false,
            launch_q: 0,
            select_chains: true,
            chains_tree_w: tree_w(),
            chains_tree_folded: false,
            chains_cards: true,
            chains_view_chosen: false,
            level_hist: 0,
            chains_closed: vec![],
            lcd_toolbar: true,
            lcd_colour: String::new(),
            topbar: vec![],
        }
    }
}

impl Extra {
    /// the record count-in in bars: 0 (off), 1 or 2
    pub fn count_in_bars(&self) -> u8 {
        self.count_in_bars
            .unwrap_or(if self.count_in { 1 } else { 0 })
            .min(2)
    }

    /// set the record count-in (0 off, 1 or 2 bars); the old flag follows
    pub fn set_count_in_bars(&mut self, bars: u8) {
        let b = bars.min(2);
        self.count_in_bars = Some(b);
        self.count_in = b > 0;
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

    /// no saved choice of Nested Device Chains view: Cards (0.2.7.0 files
    /// always wrote chains_cards = false, the old Tree default)
    pub fn default_chains_view(&mut self) {
        if !self.chains_view_chosen {
            self.chains_cards = true;
        }
    }
}

/// Settings › Look › UI scale range (on top of the OS display scale)
pub const ZOOM_MIN: f32 = 0.9;
pub const ZOOM_MAX: f32 = 1.5;
/// UI scale and Text size step
pub const SCALE_STEP: f32 = 0.05;

/// a scale snapped to the 5 % steps and the UI scale range
pub fn snap_zoom(z: f32) -> f32 {
    if !z.is_finite() {
        return 1.0;
    }
    ((z / SCALE_STEP).round() * SCALE_STEP).clamp(ZOOM_MIN, ZOOM_MAX)
}

impl Extra {
    /// the Text size as a factor (1.0 = 100 %)
    pub fn text_scale(&self) -> f32 {
        if self.text_size == 0 {
            1.0
        } else {
            (self.text_size.clamp(90, 150) as f32 / 5.0).round() * 0.05
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            theme: crate::theme::SLATE.name.into(),
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
            // "Dark Circuit (default)" (the 0.2.4.1 default, renamed) and
            // unknown names: the new default
            .unwrap_or(crate::theme::SLATE)
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
    s.extra.default_chains_view();
    Settings {
        zoom: snap_zoom(s.zoom),
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

    #[test]
    fn nested_device_chains_open_on_cards_unless_the_user_chose() {
        assert!(Extra::default().chains_cards && !Extra::default().chains_view_chosen);
        // 0.2.7.0 settings always wrote chains_cards = false
        let mut e: Extra = toml::from_str("chains_cards = false").unwrap();
        e.default_chains_view();
        assert!(e.chains_cards, "no saved choice: Cards");
        let mut e: Extra = toml::from_str("").unwrap();
        e.default_chains_view();
        assert!(e.chains_cards);
        // a choice made in this version is kept, both ways
        for cards in [false, true] {
            let mut s = Settings::default();
            s.extra.chains_cards = cards;
            s.extra.chains_view_chosen = true;
            let mut back: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
            back.extra.default_chains_view();
            assert_eq!(back.extra.chains_cards, cards);
        }
    }
}
