//! Drum Kit (0.2.4.x): one instrument with up to 32 pads, each a Drum Synth
//! voice with its own trigger key, tune, pan, level, decay, attack, velocity
//! depth, choke group and mute / solo. It is a generator engine
//! (`GenEngine::Drums`), so automation, Dice / Mutate, Bake, `.cwi` presets
//! and the Browser work as for every other generator.
//!
//! The old Kit engine (`GenEngine::Kit`, eight pieces picked by the note
//! within the octave) is untouched. `from_kit` turns a Kit into a Drum Kit
//! that renders bit-identically (`KeyMap::Legacy12`, one pad per piece, no
//! choke): tested in `tests.rs`.
//!
//! Patterns are not part of the kit: they are note clips in the timeline's
//! pool (see `timeline::NoteClipData::drum`).
use crate::gen::{DrumKind, DrumP, GenEngine, GenPatch, KitP, KIT_MAP, KIT_NAMES};
use serde::{Deserialize, Serialize};

/// most pads a kit can hold
pub const MAX_PADS: usize = 32;
/// pads in a new kit
pub const DEFAULT_PADS: usize = 16;
/// C-4 in tracker note numbers (`model::note_freq(49)` = 261.63 Hz)
pub const C4_KEY: i32 = 49;
const C4_HZ: f32 = 261.625_58;

/// how a note that matches no pad key finds a pad
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyMap {
    /// the pad with the same note name nearest in pitch (any octave)
    #[default]
    Exact,
    /// as the old Kit: the note within the octave picks pad 0..7 (toms
    /// E/F/G/A transposed -7/-4/0/+4 semitones). Converted kits use this.
    Legacy12,
}

/// one pad of a kit
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pad {
    pub name: String,
    /// 0xRRGGBB, 0 = the theme's colour for the drum kind
    pub colour: u32,
    /// trigger note (tracker numbering, 49 = C-4)
    pub key: u8,
    /// the sound (kind, model, the 8 macros, level, locks)
    pub drum: DrumP,
    /// -1 (left) .. 1 (right), 0 = centre (untouched)
    pub pan: f32,
    /// semitones, -24..24
    pub tune: f32,
    /// scales the drum's Decay macro, 0.1..4 (1 = as designed)
    pub decay: f32,
    /// fade-in in seconds, 0..0.1 (0 = off)
    pub attack: f32,
    /// how much the note velocity sets the level, 0..1 (1 = linear, as other instruments)
    pub vel: f32,
    /// choke group: a hit cuts the other pads of the group (0 = none, 1..=8)
    pub choke: u8,
    pub mute: bool,
    pub solo: bool,
}

impl Default for Pad {
    fn default() -> Self {
        Pad {
            name: "Pad".into(),
            colour: 0,
            key: C4_KEY as u8,
            drum: DrumP::default(),
            pan: 0.0,
            tune: 0.0,
            decay: 1.0,
            attack: 0.0,
            vel: 1.0,
            choke: 0,
            mute: false,
            solo: false,
        }
    }
}

impl Pad {
    pub fn new(name: &str, key: u8, drum: DrumP) -> Self {
        Pad {
            name: name.into(),
            key,
            drum,
            ..Default::default()
        }
    }
    pub fn fix(&mut self) {
        self.drum.fix();
        let f = |v: f32, lo: f32, hi: f32, d: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                d
            }
        };
        self.pan = f(self.pan, -1.0, 1.0, 0.0);
        self.tune = f(self.tune, -24.0, 24.0, 0.0);
        self.decay = f(self.decay, 0.1, 4.0, 1.0);
        self.attack = f(self.attack, 0.0, 0.1, 0.0);
        self.vel = f(self.vel, 0.0, 1.0, 1.0);
        self.choke = self.choke.min(8);
        self.key = self.key.clamp(1, 120);
        self.colour &= 0xFF_FFFF;
    }
}

/// a Drum Kit instrument's engine data
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumKitP {
    pub pads: Vec<Pad>,
    pub keymap: KeyMap,
}

impl Default for DrumKitP {
    fn default() -> Self {
        from_library_kit(&KitP::default())
    }
}

/// note name of tracker key `k` ("C-4", "F#4")
pub fn key_name(k: u8) -> String {
    const N: [&str; 12] = [
        "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
    ];
    let n = k as i32 - 1;
    format!("{}{}", N[n.rem_euclid(12) as usize], n.div_euclid(12))
}

/// the key a frequency plays (the nearest semitone, as the Kit does)
#[inline]
pub fn freq_key(freq: f32) -> i32 {
    C4_KEY + (12.0 * (freq.max(1.0) / C4_HZ).log2()).round() as i32
}

impl DrumKitP {
    pub fn fix(&mut self) {
        if self.pads.is_empty() {
            self.pads.push(Pad::default());
        }
        self.pads.truncate(MAX_PADS);
        self.pads.iter_mut().for_each(Pad::fix);
    }

    /// the pad key `key` plays and its transposition in semitones (the
    /// legacy tom offsets), None = no pad
    pub fn pad_for_key(&self, key: i32) -> Option<(usize, f32)> {
        if let Some(i) = self.pads.iter().position(|p| p.key as i32 == key) {
            return Some((i, 0.0));
        }
        let pc = (key - C4_KEY).rem_euclid(12);
        match self.keymap {
            KeyMap::Legacy12 => {
                let (p, o) = KIT_MAP[pc as usize];
                (p < self.pads.len()).then_some((p, o))
            }
            KeyMap::Exact => self
                .pads
                .iter()
                .enumerate()
                .filter(|(_, p)| (p.key as i32 - C4_KEY).rem_euclid(12) == pc)
                .min_by_key(|(_, p)| (p.key as i32 - key).abs())
                .map(|(i, _)| (i, 0.0)),
        }
    }

    pub fn pad_for_freq(&self, freq: f32) -> Option<(usize, f32)> {
        self.pad_for_key(freq_key(freq))
    }

    /// pad `i` sounds (not muted, and not silenced by another pad's solo)
    pub fn audible(&self, i: usize) -> bool {
        let Some(p) = self.pads.get(i) else {
            return false;
        };
        if p.mute {
            return false;
        }
        p.solo || !self.pads.iter().any(|p| p.solo)
    }

    /// the note gain for velocity `vel` (0..1) on pad `i`: `vel` itself at
    /// the default depth 1, so a kit plays like every other instrument
    pub fn vel_gain(&self, i: usize, vel: f32) -> f32 {
        match self.pads.get(i) {
            Some(p) if p.vel != 1.0 => 1.0 - p.vel + p.vel * vel,
            _ => vel,
        }
    }

    /// a free key for a new pad: the next one above the highest in use
    pub fn free_key(&self) -> u8 {
        let mut k = self.pads.iter().map(|p| p.key).max().unwrap_or(48) + 1;
        while self.pads.iter().any(|p| p.key == k) && k < 120 {
            k += 1;
        }
        k.min(120)
    }

    /// add a pad (a copy of pad `from`, or a default kick); None = full
    pub fn add_pad(&mut self, from: Option<usize>) -> Option<usize> {
        if self.pads.len() >= MAX_PADS {
            return None;
        }
        let key = self.free_key();
        let mut p = match from.and_then(|i| self.pads.get(i)) {
            Some(p) => Pad {
                name: format!("{} 2", p.name),
                mute: false,
                solo: false,
                ..p.clone()
            },
            None => Pad::new("Pad", key, DrumP::default()),
        };
        p.key = key;
        self.pads.push(p);
        Some(self.pads.len() - 1)
    }
}

/// Convert a Kit: one pad per piece, the Kit's note map, no choke. Renders
/// bit-identically to the Kit and keeps its automation lanes (pad p macro m =
/// knob p*8+m, as piece p).
pub fn from_kit(k: &KitP) -> DrumKitP {
    // the key each piece answers on in octave 4 (KIT_KEYS' first note)
    const KEYS: [u8; 8] = [49, 51, 52, 55, 59, 56, 50, 60];
    DrumKitP {
        pads: k
            .pieces
            .iter()
            .enumerate()
            .map(|(i, d)| Pad::new(KIT_NAMES[i], KEYS[i], *d))
            .collect(),
        keymap: KeyMap::Legacy12,
    }
}

/// The 16-pad layout of a new Drum Kit, its sounds taken from Kit `k`: every
/// note the Drum Patterns library uses (`drumpat::LANES`) plays the same drum
/// as on the Kit, the toms are split into pads, and Crash, Ride, Rim and
/// Shaker are added. The hats share choke group 1.
pub fn from_library_kit(k: &KitP) -> DrumKitP {
    use DrumKind::*;
    let pc = &k.pieces;
    let tom = |name: &str, key: u8, tune: f32| Pad {
        tune,
        ..Pad::new(name, key, pc[5])
    };
    let hat = |name: &str, key: u8, d: DrumP| Pad {
        choke: 1,
        ..Pad::new(name, key, d)
    };
    let d = DrumP::new;
    let pads = vec![
        Pad::new("Kick", 49, pc[0]),
        Pad::new("Snare", 51, pc[1]),
        Pad::new("Clap", 52, pc[2]),
        hat("Closed hat", 55, pc[3]),
        hat("Pedal hat", 57, pc[3]),
        hat("Open hat", 59, pc[4]),
        tom("Low tom", 53, -7.0),
        tom("Mid tom", 56, 0.0),
        tom("High tom", 58, 4.0),
        tom("Floor tom", 54, -4.0),
        Pad::new("Perc", 50, pc[6]),
        Pad::new("Perc 2", 60, pc[7]),
        Pad::new(
            "Crash",
            61,
            d(Perc, 3, &[0.9, 1.6, 0.7, 0.45, 0.3, 0.1, 0.6, 0.0]),
        ),
        Pad::new(
            "Ride",
            62,
            d(Perc, 3, &[1.3, 0.9, 0.55, 0.2, 0.25, 0.1, 0.4, 0.0]),
        ),
        Pad::new(
            "Rim",
            63,
            d(Perc, 2, &[1.0, 0.07, 0.6, 0.0, 0.3, 0.4, 0.0, 0.0]),
        ),
        Pad::new("Shaker", 64, {
            let mut s = d(Hat, 0, &[1.6, 0.06, 0.4, 6500.0, 0.1, 0.5, 0.0, 0.0]);
            s.level = 1.3;
            s
        }),
    ];
    debug_assert_eq!(pads.len(), DEFAULT_PADS);
    DrumKitP {
        pads,
        keymap: KeyMap::Exact,
    }
}

/// a Drum Kit generator patch made from a Kit patch (filter, release and
/// gain carried over); `legacy` = `from_kit` (bit-identical), else the 16-pad
/// layout
pub fn patch_from_kit(g: &GenPatch, legacy: bool) -> Option<GenPatch> {
    let GenEngine::Kit(k) = &g.engine else {
        return None;
    };
    Some(GenPatch {
        engine: GenEngine::Drums(if legacy {
            from_kit(k)
        } else {
            from_library_kit(k)
        }),
        ..g.clone()
    })
}

/// the Drum Kit presets: one per Drum Patterns kit (16 pads each)
pub fn presets() -> Vec<(String, &'static str, GenPatch)> {
    crate::drumpat::KIT_NAMES
        .iter()
        .filter_map(|(name, desc)| {
            let g = crate::drumpat::kit_patch(name)?;
            let mut p = patch_from_kit(&g, false)?;
            p.base.name = name.to_string();
            Some((name.to_string(), *desc, p))
        })
        .collect()
}

/// the Drum Kit preset `name` ("Psy Kit" …)
pub fn preset(name: &str) -> Option<GenPatch> {
    presets()
        .into_iter()
        .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, _, p)| p)
}

/// index of the kind's "Decay" macro (what the pad's Decay scales)
pub fn decay_macro(k: DrumKind) -> Option<usize> {
    k.macros().iter().position(|m| m.name == "Decay")
}

#[cfg(test)]
mod tests;
