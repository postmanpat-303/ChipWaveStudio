//! Remappable global shortcuts (docs/SETTINGS.md §2.4). Overrides are stored in
//! keymap.toml; at runtime a remapped key is translated to the action's
//! default key before the normal key handling runs, so all existing code paths
//! keep working.
use eframe::egui::{Event, Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Action {
    PlayBlock,
    PlaySong,
    PlayFromRow,
    ToggleEdit,
    Undo,
    Redo,
    New,
    Open,
    Save,
    SaveAs,
    ExportWav,
    ExportDialog,
    Settings,
    Help,
    Quit,
    ViewPattern,
    ViewArranger,
    ViewMixer,
    ViewTimeline,
    ViewFx,
    ViewSample,
    ScaleUp,
    ScaleDown,
    ScaleReset,
    Mute,
    Solo,
    Panic,
    BlockLonger,
    BlockShorter,
    FullHeight,
    InstrumentPanel,
    ParamSearch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Combo {
    #[serde(with = "key_name")]
    pub key: Key,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl Combo {
    pub const fn new(key: Key, ctrl: bool, shift: bool) -> Self {
        Self {
            key,
            ctrl,
            shift,
            alt: false,
        }
    }
    pub fn from_event(e: &Event) -> Option<Combo> {
        match e {
            Event::Key {
                key,
                pressed: true,
                modifiers: m,
                ..
            } => Some(Combo {
                key: *key,
                ctrl: m.command || m.ctrl,
                shift: m.shift,
                alt: m.alt,
            }),
            _ => None,
        }
    }
    pub fn mods(&self) -> Modifiers {
        let mut m = Modifiers::NONE;
        if self.ctrl {
            m |= Modifiers::COMMAND;
        }
        if self.shift {
            m |= Modifiers::SHIFT;
        }
        if self.alt {
            m |= Modifiers::ALT;
        }
        m
    }
    pub fn text(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push_str("Ctrl+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        s.push_str(self.key.name());
        s
    }
}

mod key_name {
    use eframe::egui::Key;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(k: &Key, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(k.name())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Key, D::Error> {
        let t = String::deserialize(d)?;
        Key::from_name(&t).ok_or_else(|| serde::de::Error::custom(format!("unknown key {t}")))
    }
}

use Action as A;
pub const DEFAULTS: [(Action, &str, Combo); 32] = [
    (
        A::PlayBlock,
        "Play / stop block",
        Combo::new(Key::Space, false, false),
    ),
    (
        A::PlaySong,
        "Play / stop song",
        Combo::new(Key::Space, false, true),
    ),
    (
        A::PlayFromRow,
        "Play from cursor row",
        Combo::new(Key::Enter, true, false),
    ),
    (
        A::ToggleEdit,
        "Edit mode on / off",
        Combo::new(Key::Escape, false, false),
    ),
    (A::Undo, "Undo", Combo::new(Key::Z, true, false)),
    (A::Redo, "Redo", Combo::new(Key::Y, true, false)),
    (A::New, "New song", Combo::new(Key::N, true, false)),
    (A::Open, "Open", Combo::new(Key::O, true, false)),
    (A::Save, "Save", Combo::new(Key::S, true, false)),
    (A::SaveAs, "Save as", Combo::new(Key::S, true, true)),
    (
        A::ExportWav,
        "Quick export WAV",
        Combo::new(Key::E, true, false),
    ),
    (
        A::ExportDialog,
        "Export dialog (WAV / MP3)",
        Combo::new(Key::E, true, true),
    ),
    (A::Settings, "Settings", Combo::new(Key::Comma, true, false)),
    (A::Help, "Key list", Combo::new(Key::F12, false, false)),
    (A::Quit, "Quit", Combo::new(Key::Q, true, false)),
    (
        A::ViewPattern,
        "View: pattern",
        Combo::new(Key::Num1, true, false),
    ),
    (
        A::ViewArranger,
        "View: arranger",
        Combo::new(Key::Num2, true, false),
    ),
    (
        A::ViewTimeline,
        "View: timeline",
        Combo::new(Key::T, true, false),
    ),
    (
        A::ViewMixer,
        "View: mixer",
        Combo::new(Key::Num3, true, false),
    ),
    (A::ViewFx, "View: FX", Combo::new(Key::Num4, true, false)),
    (
        A::ViewSample,
        "View: sample",
        Combo::new(Key::Num5, true, false),
    ),
    (
        A::ScaleUp,
        "UI scale up",
        Combo::new(Key::Equals, true, false),
    ),
    (
        A::ScaleDown,
        "UI scale down",
        Combo::new(Key::Minus, true, false),
    ),
    (
        A::ScaleReset,
        "UI scale 100 %",
        Combo::new(Key::Num0, true, false),
    ),
    (A::Mute, "Mute channel", Combo::new(Key::F9, false, false)),
    (A::Solo, "Solo channel", Combo::new(Key::F10, false, false)),
    (
        A::Panic,
        "Panic: stop all sound",
        Combo::new(Key::Period, true, false),
    ),
    (
        A::BlockLonger,
        "Double block length",
        Combo::new(Key::PageDown, true, true),
    ),
    (
        A::BlockShorter,
        "Halve block length",
        Combo::new(Key::PageUp, true, true),
    ),
    (
        A::FullHeight,
        "Full-height pattern (hide the lower panel)",
        Combo::new(Key::F11, false, false),
    ),
    (
        A::InstrumentPanel,
        "Edit instrument in the lower panel",
        Combo::new(Key::I, true, false),
    ),
    (
        A::ParamSearch,
        "Search instrument parameters",
        Combo::new(Key::F, true, false),
    ),
];

pub fn default_of(a: Action) -> Combo {
    DEFAULTS.iter().find(|d| d.0 == a).map(|d| d.2).unwrap()
}

pub fn label_of(a: Action) -> &'static str {
    DEFAULTS
        .iter()
        .find(|d| d.0 == a)
        .map(|d| d.1)
        .unwrap_or("")
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Keymap {
    #[serde(default)]
    pub overrides: BTreeMap<Action, Combo>,
}

#[derive(Debug, PartialEq)]
pub enum Bind {
    Ok,
    /// already used by another action
    Conflict(Action),
}

impl Keymap {
    pub fn binding(&self, a: Action) -> Combo {
        self.overrides
            .get(&a)
            .copied()
            .unwrap_or_else(|| default_of(a))
    }
    pub fn action_for(&self, c: Combo) -> Option<Action> {
        DEFAULTS.iter().map(|d| d.0).find(|a| self.binding(*a) == c)
    }
    /// Bind `a` to `c`; refuses (and reports) a combo used by another action.
    pub fn set(&mut self, a: Action, c: Combo) -> Bind {
        if let Some(o) = self.action_for(c) {
            if o != a {
                return Bind::Conflict(o);
            }
        }
        if c == default_of(a) {
            self.overrides.remove(&a);
        } else {
            self.overrides.insert(a, c);
        }
        Bind::Ok
    }
    pub fn reset(&mut self, a: Action) -> Bind {
        let c = default_of(a);
        match self.action_for(c) {
            Some(o) if o != a => Bind::Conflict(o),
            _ => {
                self.overrides.remove(&a);
                Bind::Ok
            }
        }
    }
    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }
    /// Rewrite a key event: a remapped combo becomes its action's default combo;
    /// the default combo of a remapped action no longer does anything (None).
    pub fn translate(&self, e: &Event) -> Option<Event> {
        if self.overrides.is_empty() {
            return Some(e.clone());
        }
        let Some(c) = Combo::from_event(e) else {
            return Some(e.clone());
        };
        let Event::Key {
            physical_key,
            repeat,
            ..
        } = e
        else {
            return Some(e.clone());
        };
        if let Some(a) = self.action_for(c) {
            let d = default_of(a);
            return Some(Event::Key {
                key: d.key,
                physical_key: *physical_key,
                pressed: true,
                repeat: *repeat,
                modifiers: d.mods(),
            });
        }
        if self.overrides.keys().any(|a| default_of(*a) == c) {
            return None;
        }
        Some(e.clone())
    }

    pub fn path() -> Option<std::path::PathBuf> {
        crate::settings::config_dir().map(|d| d.join("keymap.toml"))
    }
    pub fn load() -> Keymap {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str::<KeymapFile>(&t).ok())
            .map(|f| {
                let mut k = Keymap::default();
                for (a, c) in f.keys {
                    let _ = k.set(a, c);
                }
                k
            })
            .unwrap_or_default()
    }
    pub fn save(&self) {
        let Some(p) = Self::path() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let f = KeymapFile {
            keys: self.overrides.clone().into_iter().collect(),
        };
        if let Ok(t) = toml::to_string(&f) {
            let tmp = p.with_extension("toml.tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
}

/// keymap.toml: `[[keys]]` pairs so the file stays readable
#[derive(Serialize, Deserialize)]
struct KeymapFile {
    #[serde(default)]
    keys: Vec<(Action, Combo)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(c: Combo) -> Event {
        Event::Key {
            key: c.key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: c.mods(),
        }
    }

    #[test]
    fn defaults_have_no_conflicts() {
        let k = Keymap::default();
        for (a, _, c) in DEFAULTS {
            assert_eq!(k.action_for(c), Some(a), "{a:?}");
        }
    }

    #[test]
    fn remap_translates_and_conflicts() {
        let mut k = Keymap::default();
        let f5 = Combo::new(Key::F5, false, false);
        assert_eq!(k.set(Action::PlaySong, f5), Bind::Ok);
        assert_eq!(k.binding(Action::PlaySong), f5);
        // F5 now produces the default play-song event (Shift+Space)
        let t = k.translate(&ev(f5)).unwrap();
        assert_eq!(Combo::from_event(&t), Some(default_of(Action::PlaySong)));
        // the old Shift+Space does nothing now
        assert!(k.translate(&ev(default_of(Action::PlaySong))).is_none());
        // unrelated keys pass through
        let z = Combo::new(Key::Z, false, false);
        assert_eq!(Combo::from_event(&k.translate(&ev(z)).unwrap()), Some(z));
        // conflict: Ctrl+S is Save
        assert_eq!(
            k.set(Action::Open, Combo::new(Key::S, true, false)),
            Bind::Conflict(Action::Save)
        );
        assert_eq!(k.binding(Action::Open), default_of(Action::Open));
        // reset
        assert_eq!(k.reset(Action::PlaySong), Bind::Ok);
        assert!(k.overrides.is_empty());
        // roundtrip through the file format
        k.set(Action::Help, Combo::new(Key::F1, true, false));
        let f = KeymapFile {
            keys: k.overrides.clone().into_iter().collect(),
        };
        let t = toml::to_string(&f).unwrap();
        let back: KeymapFile = toml::from_str(&t).unwrap();
        assert_eq!(back.keys, f.keys);
    }
}
