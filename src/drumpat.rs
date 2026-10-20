//! Drum Patterns (docs/BROWSER.md "Drum Patterns"): a built-in library of
//! ready-made grooves, sorted by style, each paired with a drum kit made by
//! ChipWave's own Drum Synth (kit mode: the note picks the drum). No samples
//! are used, so every kit is ours to ship (GPL-3, like the rest of ChipWave);
//! the drum recipes follow the Sonic Pi sc808 SynthDefs and the Plaits /
//! DaisySP drum models (see docs/GENERATORS.md and the About box credits).
//!
//! A pattern is written as step lanes of 16 steps per bar, one lane per drum:
//! `x` = hit, `X` = accent, `o` = ghost, `.` = rest (`|` is ignored). A lane
//! shorter than the pattern repeats. Patterns turn into timeline notes in
//! ticks (`timeline::ROW` per 16th), so a placed clip follows the song tempo.
use crate::gen::{DrumKind, DrumP, GenEngine, GenPatch, KitP};
use crate::model::{Instrument, InstrumentKind};
use crate::timeline::{vel_from_7, Note, Tick, ROW};

/// music styles (the Browser's folders), in display order
pub const STYLES: [&str; 14] = [
    "Psytrance / Dark psy",
    "Techno",
    "House",
    "Trance",
    "DnB / Jungle",
    "Breakbeat",
    "Hip-hop / Lo-fi",
    "Trap",
    "Disco / Funk",
    "Synthwave / 80s",
    "Reggaeton / Dembow",
    "Amiga / Chip",
    "Afro / Latin",
    "Fills",
];

/// lane letter -> (kit note, lane name). C-4 = 49 plays the kick.
pub const LANES: [(char, u8, &str); 10] = [
    ('K', 49, "Kick"),
    ('P', 50, "Perc"),
    ('S', 51, "Snare"),
    ('C', 52, "Clap"),
    ('L', 53, "Low tom"),
    ('H', 55, "Closed hat"),
    ('M', 56, "Mid tom"),
    ('T', 58, "High tom"),
    ('O', 59, "Open hat"),
    ('Z', 60, "Perc 2"),
];

pub fn lane_note(c: char) -> Option<u8> {
    LANES.iter().find(|l| l.0 == c).map(|l| l.1)
}

/// one built-in groove
#[derive(Clone, Copy, Debug)]
pub struct Pat {
    pub name: &'static str,
    /// index into `STYLES`
    pub style: usize,
    pub bars: u8,
    /// suggested tempo range
    pub bpm: (u16, u16),
    /// the paired kit (`kits()` name)
    pub kit: &'static str,
    /// 0..0.5: odd 16ths are late by this part of a 16th (shuffle)
    pub swing: f32,
    pub lanes: &'static [(char, &'static str)],
}

/// one step of a lane: velocity 0 (rest) or 1..=127
fn step_vel(c: char) -> u8 {
    match c {
        'X' => 127,
        'x' => 100,
        'o' => 58,
        _ => 0,
    }
}

impl Pat {
    pub fn steps(&self) -> usize {
        self.bars.max(1) as usize * 16
    }
    pub fn len(&self) -> Tick {
        self.steps() as Tick * ROW
    }
    /// lane `l` as velocities, repeated to the pattern length
    pub fn lane_steps(&self, l: &str) -> Vec<u8> {
        let src: Vec<u8> = l
            .chars()
            .filter(|c| *c != '|' && *c != ' ')
            .map(step_vel)
            .collect();
        if src.is_empty() {
            return vec![0; self.steps()];
        }
        (0..self.steps()).map(|k| src[k % src.len()]).collect()
    }
    /// tick offset of step `k` (swing delays the odd 16ths)
    pub fn step_tick(&self, k: usize) -> Tick {
        let base = k as Tick * ROW;
        if k % 2 == 1 {
            base + (self.swing.clamp(0.0, 0.5) * ROW as f32).round() as Tick
        } else {
            base
        }
    }
    /// (tick, kit note, velocity 1..=127), sorted by tick
    pub fn events(&self) -> Vec<(Tick, u8, u8)> {
        let mut v = vec![];
        for (c, l) in self.lanes {
            let Some(n) = lane_note(*c) else { continue };
            for (k, vel) in self.lane_steps(l).into_iter().enumerate() {
                if vel > 0 {
                    v.push((self.step_tick(k), n, vel));
                }
            }
        }
        v.sort_by_key(|e| (e.0, e.1));
        v
    }
    /// timeline notes (instrument `instr`, 1-based; 0 = the track's)
    pub fn notes(&self, instr: u8) -> Vec<Note> {
        self.events()
            .into_iter()
            .map(|(t, key, vel)| Note {
                start: t,
                len: ROW,
                key,
                vel: vel_from_7(vel),
                instr,
                ..Default::default()
            })
            .collect()
    }
    pub fn hits(&self) -> usize {
        self.events().len()
    }
    /// thumbnail: per lane (in `LANES` order), the first bar's 16 steps as a bit mask
    pub fn thumb(&self) -> Vec<(char, u16)> {
        let mut v = vec![];
        for (c, _, _) in LANES {
            let mut m = 0u16;
            for (lc, l) in self.lanes {
                if *lc == c {
                    for (k, vel) in self.lane_steps(l).into_iter().take(16).enumerate() {
                        if vel > 0 {
                            m |= 1 << k;
                        }
                    }
                }
            }
            if m != 0 {
                v.push((c, m));
            }
        }
        v
    }
    pub fn bpm_tag(&self) -> String {
        if self.bpm.0 == self.bpm.1 {
            format!("{} BPM", self.bpm.0)
        } else {
            format!("{}–{} BPM", self.bpm.0, self.bpm.1)
        }
    }
    /// stable Browser key (favourites)
    pub fn key(&self) -> String {
        format!("pat:{}/{}", STYLES[self.style], self.name)
    }
}

// ------------------------------------------------------------------ kits

/// the kits patterns pair with: (name, description)
pub const KIT_NAMES: [(&str, &str); 13] = [
    ("Psy Kit", "zap kick, 808 snare, metal hats, cowbell, zap"),
    ("808 Kit", "sc808: long boom kick, rimshot, claves"),
    ("909 Kit", "punchy 909 kick and snare, ride cymbal"),
    ("Techno Kit", "driven 909 kick, tight clap, comb hats, zap"),
    ("Trap Kit", "long 808 sub kick, crisp hats, rim, cowbell"),
    (
        "Lo-fi Kit",
        "soft kick, ringing shell snare, dull hats, through a low-pass",
    ),
    (
        "Chip Kit",
        "8-bit style: zap kick, noisy snare, FM hats, laser",
    ),
    (
        "Acoustic-ish Kit",
        "modal shell snare, comb hats, toms, ride",
    ),
    (
        "Disco Kit",
        "round kick, bright snare, open hats, congas, cowbell",
    ),
    ("Latin Kit", "congas, cowbell, claves, rimshot"),
    ("DnB Kit", "tight kick, cracking snare, ring hats, ride"),
    ("Synthwave Kit", "big 80s snare, boomy toms, crisp hats"),
    (
        "Dembow Kit",
        "808 kick, snappy snare, rim, timbale-ish toms",
    ),
];

fn kit(pieces: [DrumP; 8]) -> KitP {
    KitP { pieces }
}

/// the kit named `name` (None = unknown)
pub fn kit_patch(name: &str) -> Option<GenPatch> {
    use DrumKind::*;
    let d = DrumP::new;
    let k = match name {
        "Psy Kit" | "808 Kit" => return crate::gen::preset(name),
        "909 Kit" => kit([
            d(Kick, 2, &[52.0, 28.0, 0.025, 0.4, 0.5, 0.15, 0.35, 0.05]),
            d(Snare, 1, &[220.0, 0.6, 0.18, 0.1, 7500.0, 0.5, 0.0, 0.2]),
            d(Clap, 0, &[1300.0, 0.3, 0.009, 4.0, 0.35, 0.2, 0.5, 0.0]),
            d(Hat, 1, &[1.2, 0.06, 0.85, 9000.0, 0.4, 0.5, 0.1, 0.0]),
            d(Hat, 1, &[1.2, 0.35, 0.85, 8500.0, 0.4, 0.5, 0.1, 0.0]),
            d(Tom, 0, &[140.0, 5.0, 0.07, 0.45, 0.25, 0.05, 0.1, 0.0]),
            d(Perc, 2, &[1.0, 0.07, 0.6, 0.0, 0.3, 0.4, 0.0, 0.0]),
            d(Perc, 3, &[1.1, 0.9, 0.7, 0.4, 0.3, 0.1, 0.5, 0.0]),
        ]),
        "Techno Kit" => kit([
            d(Kick, 2, &[48.0, 30.0, 0.03, 0.5, 0.45, 0.25, 0.65, 0.1]),
            d(Snare, 1, &[200.0, 0.5, 0.25, 0.12, 6500.0, 0.4, 0.0, 0.3]),
            d(Clap, 1, &[1600.0, 0.15, 0.008, 3.0, 0.4, 0.0, 0.3, 0.1]),
            d(Hat, 2, &[1.0, 0.05, 0.7, 7000.0, 0.6, 0.5, 0.2, 0.0]),
            d(Hat, 2, &[1.0, 0.3, 0.7, 6500.0, 0.6, 0.5, 0.2, 0.0]),
            d(Tom, 0, &[90.0, 4.0, 0.08, 0.6, 0.15, 0.1, 0.3, 0.2]),
            d(Perc, 2, &[1.0, 0.06, 0.6, 0.0, 0.3, 0.4, 0.0, 0.1]),
            d(Perc, 4, &[1.0, 0.2, 0.5, 0.6, 0.2, 0.0, 0.3, 0.0]),
        ]),
        "Trap Kit" => kit([
            d(Kick, 1, &[42.0, 34.0, 0.03, 2.0, 0.2, 0.0, 0.2, 0.0]),
            d(Snare, 0, &[210.0, 0.7, 0.3, 0.14, 6000.0, 0.4, 0.1, 0.1]),
            d(Clap, 0, &[1400.0, 0.25, 0.01, 4.0, 0.35, 0.25, 0.6, 0.0]),
            d(Hat, 0, &[1.1, 0.03, 0.8, 9500.0, 0.3, 0.3, 0.3, 0.0]),
            d(Hat, 0, &[1.1, 0.3, 0.8, 9000.0, 0.3, 0.4, 0.1, 0.0]),
            d(Tom, 0, &[110.0, 5.0, 0.08, 0.6, 0.2, 0.05, 0.1, 0.0]),
            d(Perc, 2, &[1.0, 0.07, 0.6, 0.0, 0.3, 0.4, 0.0, 0.0]),
            d(Perc, 0, &[1.0, 0.25, 0.6, 0.3, 0.3, 0.2, 0.0, 0.0]),
        ]),
        "Lo-fi Kit" => {
            let k = kit([
                d(Kick, 0, &[48.0, 30.0, 0.03, 0.45, 0.15, 0.4, 0.15, 0.1]),
                d(Snare, 2, &[165.0, 0.4, 0.2, 0.25, 3800.0, 0.2, 0.5, 0.15]),
                d(Clap, 0, &[1100.0, 0.25, 0.01, 4.0, 0.3, 0.3, 0.4, 0.0]),
                d(Hat, 0, &[0.9, 0.04, 0.6, 5500.0, 0.2, 0.2, 0.2, 0.1]),
                d(Hat, 0, &[0.9, 0.3, 0.6, 5200.0, 0.2, 0.3, 0.1, 0.1]),
                d(Tom, 1, &[230.0, 5.0, 0.03, 0.3, 0.2, 0.0, 0.2, 0.0]),
                d(Perc, 2, &[0.9, 0.06, 0.5, 0.0, 0.3, 0.3, 0.0, 0.1]),
                d(Perc, 3, &[0.85, 0.8, 0.45, 0.3, 0.3, 0.1, 0.4, 0.1]),
            ]);
            let mut g = GenPatch {
                engine: GenEngine::Kit(k),
                ..Default::default()
            };
            // a dull, dusty top end
            g.filter_on = true;
            g.base.cutoff = 4200.0;
            g.base.resonance = 0.1;
            return Some(g);
        }
        "Chip Kit" => kit([
            d(Kick, 3, &[55.0, 48.0, 0.02, 0.25, 0.6, 0.0, 0.5, 0.1]),
            d(Snare, 1, &[260.0, 0.3, 0.6, 0.12, 9000.0, 0.6, 0.0, 0.5]),
            d(Clap, 1, &[1800.0, 0.12, 0.006, 2.0, 0.5, 0.0, 0.2, 0.4]),
            d(Hat, 2, &[1.4, 0.035, 0.9, 10000.0, 0.8, 0.2, 0.4, 0.3]),
            d(Hat, 2, &[1.4, 0.2, 0.9, 9500.0, 0.8, 0.2, 0.3, 0.3]),
            d(Tom, 0, &[150.0, 8.0, 0.05, 0.3, 0.4, 0.0, 0.0, 0.4]),
            d(Perc, 4, &[1.4, 0.15, 0.6, 0.7, 0.2, 0.2, 0.0, 0.3]),
            d(Perc, 4, &[0.8, 0.3, 0.4, 0.5, 0.3, 0.0, 0.2, 0.2]),
        ]),
        "Acoustic-ish Kit" => kit([
            d(Kick, 2, &[56.0, 22.0, 0.03, 0.4, 0.3, 0.55, 0.05, 0.1]),
            d(Snare, 2, &[185.0, 0.5, 0.35, 0.28, 5200.0, 0.4, 0.45, 0.05]),
            d(Clap, 0, &[1200.0, 0.3, 0.012, 4.0, 0.3, 0.35, 0.5, 0.0]),
            d(Hat, 2, &[1.0, 0.07, 0.65, 7500.0, 0.5, 0.6, 0.15, 0.0]),
            d(Hat, 2, &[1.0, 0.5, 0.65, 7000.0, 0.5, 0.6, 0.1, 0.0]),
            d(Tom, 0, &[120.0, 3.0, 0.1, 0.6, 0.2, 0.15, 0.4, 0.0]),
            d(Perc, 2, &[1.0, 0.08, 0.55, 0.0, 0.35, 0.4, 0.0, 0.0]),
            d(Perc, 3, &[1.0, 1.2, 0.6, 0.35, 0.3, 0.1, 0.5, 0.0]),
        ]),
        "Disco Kit" => kit([
            d(Kick, 1, &[50.0, 32.0, 0.03, 0.55, 0.3, 0.1, 0.1, 0.0]),
            d(Snare, 1, &[230.0, 0.6, 0.25, 0.12, 7500.0, 0.5, 0.1, 0.1]),
            d(Clap, 0, &[1300.0, 0.3, 0.009, 4.0, 0.35, 0.3, 0.5, 0.0]),
            d(Hat, 0, &[1.0, 0.05, 0.75, 8500.0, 0.3, 0.3, 0.2, 0.0]),
            d(Hat, 0, &[1.0, 0.5, 0.75, 8000.0, 0.3, 0.4, 0.1, 0.0]),
            d(Tom, 1, &[260.0, 6.0, 0.03, 0.35, 0.3, 0.0, 0.2, 0.0]),
            d(Perc, 0, &[1.0, 0.25, 0.6, 0.3, 0.3, 0.2, 0.0, 0.0]),
            d(Perc, 1, &[1.0, 0.08, 0.5, 0.3, 0.4, 0.3, 0.0, 0.0]),
        ]),
        "Latin Kit" => kit([
            d(Kick, 1, &[48.0, 30.0, 0.03, 0.5, 0.2, 0.2, 0.05, 0.0]),
            d(Snare, 2, &[200.0, 0.5, 0.2, 0.2, 5000.0, 0.4, 0.5, 0.0]),
            d(Clap, 0, &[1300.0, 0.3, 0.009, 4.0, 0.35, 0.25, 0.5, 0.0]),
            d(Hat, 0, &[1.0, 0.04, 0.7, 8000.0, 0.3, 0.3, 0.2, 0.0]),
            d(Hat, 0, &[1.0, 0.35, 0.7, 7500.0, 0.3, 0.4, 0.1, 0.0]),
            d(Tom, 1, &[250.0, 6.0, 0.03, 0.4, 0.3, 0.0, 0.25, 0.0]),
            d(Perc, 0, &[1.0, 0.3, 0.6, 0.3, 0.3, 0.2, 0.0, 0.0]),
            d(Perc, 1, &[1.0, 0.08, 0.5, 0.3, 0.4, 0.3, 0.0, 0.0]),
        ]),
        "DnB Kit" => kit([
            d(Kick, 0, &[52.0, 42.0, 0.02, 0.35, 0.65, 0.3, 0.3, 0.08]),
            d(Snare, 1, &[245.0, 0.55, 0.3, 0.12, 8000.0, 0.6, 0.1, 0.25]),
            d(Clap, 1, &[1600.0, 0.15, 0.008, 3.0, 0.4, 0.0, 0.3, 0.1]),
            d(Hat, 1, &[1.3, 0.04, 0.85, 9500.0, 0.4, 0.5, 0.2, 0.0]),
            d(Hat, 1, &[1.3, 0.3, 0.85, 9000.0, 0.4, 0.5, 0.1, 0.0]),
            d(Tom, 0, &[150.0, 5.0, 0.06, 0.4, 0.3, 0.05, 0.2, 0.1]),
            d(Perc, 2, &[1.0, 0.07, 0.6, 0.0, 0.3, 0.4, 0.0, 0.0]),
            d(Perc, 3, &[1.2, 0.7, 0.7, 0.45, 0.3, 0.1, 0.5, 0.0]),
        ]),
        "Synthwave Kit" => kit([
            d(Kick, 2, &[50.0, 26.0, 0.03, 0.5, 0.4, 0.3, 0.25, 0.05]),
            d(Snare, 1, &[190.0, 0.55, 0.35, 0.35, 6000.0, 0.4, 0.3, 0.2]),
            d(Clap, 0, &[1250.0, 0.5, 0.012, 5.0, 0.3, 0.6, 0.7, 0.0]),
            d(Hat, 0, &[1.0, 0.05, 0.75, 8500.0, 0.3, 0.3, 0.2, 0.0]),
            d(Hat, 0, &[1.0, 0.45, 0.75, 8000.0, 0.3, 0.4, 0.1, 0.0]),
            d(Tom, 0, &[110.0, 6.0, 0.1, 0.8, 0.2, 0.05, 0.4, 0.1]),
            d(Perc, 2, &[1.0, 0.07, 0.6, 0.0, 0.3, 0.4, 0.0, 0.0]),
            d(Perc, 3, &[0.9, 1.4, 0.6, 0.35, 0.3, 0.1, 0.6, 0.0]),
        ]),
        "Dembow Kit" => kit([
            d(Kick, 1, &[46.0, 32.0, 0.03, 0.9, 0.2, 0.0, 0.15, 0.0]),
            d(Snare, 0, &[230.0, 0.7, 0.35, 0.1, 6500.0, 0.45, 0.1, 0.15]),
            d(Clap, 1, &[1600.0, 0.15, 0.008, 3.0, 0.4, 0.0, 0.3, 0.1]),
            d(Hat, 0, &[1.0, 0.04, 0.75, 8500.0, 0.3, 0.3, 0.2, 0.0]),
            d(Hat, 0, &[1.0, 0.35, 0.75, 8000.0, 0.3, 0.4, 0.1, 0.0]),
            d(Tom, 1, &[300.0, 6.0, 0.03, 0.3, 0.4, 0.0, 0.2, 0.0]),
            d(Perc, 2, &[1.0, 0.07, 0.6, 0.0, 0.3, 0.4, 0.0, 0.0]),
            d(Perc, 1, &[1.0, 0.08, 0.5, 0.3, 0.4, 0.3, 0.0, 0.0]),
        ]),
        _ => return None,
    };
    Some(GenPatch {
        engine: GenEngine::Kit(k),
        ..Default::default()
    })
}

/// the kit as a song instrument (named after the kit)
#[cfg(test)]
pub fn kit_instrument(name: &str) -> Option<Instrument> {
    let mut g = kit_patch(name)?;
    g.base.name = name.to_string();
    Some(Instrument {
        name: name.to_string(),
        volume: 0.85,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Gen(g),
    })
}

/// the kit as a Drum Kit instrument (16 pads, `drumkit::from_library_kit`):
/// what a pattern dropped on a new track gets
pub fn drumkit_instrument(name: &str) -> Option<Instrument> {
    let mut g = crate::drumkit::preset(name)?;
    g.base.name = name.to_string();
    Some(Instrument {
        name: name.to_string(),
        volume: 0.85,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Gen(g),
    })
}

pub fn kit_desc(name: &str) -> &'static str {
    KIT_NAMES
        .iter()
        .find(|k| k.0 == name)
        .map(|k| k.1)
        .unwrap_or("")
}

mod library;
pub use library::PATTERNS;

/// patterns of style `s`
pub fn in_style(s: usize) -> impl Iterator<Item = (usize, &'static Pat)> {
    PATTERNS
        .iter()
        .enumerate()
        .filter(move |(_, p)| p.style == s)
}

#[cfg(test)]
mod tests;
