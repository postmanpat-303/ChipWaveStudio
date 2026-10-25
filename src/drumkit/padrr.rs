//! Pad Round Robin (docs/ROUND_ROBIN.md, part B): up to four sounds per
//! pad that take turns hit by hit, and a Variation amount that nudges each
//! hit's pitch, level, tone, decay, timing and pan a little, so repeated
//! hits stop sounding like a machine gun.
//!
//! Everything a hit does comes from [`hit_mod`]: the pad's seed and the hit
//! number (counted per key on the timeline, see `timeline::plan`), never a
//! clock, so playback, seeking and export always agree. Off by default and
//! not written to files while off.
use crate::gen::DrumP;
use crate::timeline::rr::{self, Order};
use crate::timeline::ClipRng;
use serde::{Deserialize, Serialize};

/// most sounds a pad can rotate through (its own + three more)
pub const MAX_SOUNDS: usize = 4;

/// how far each part of a hit may drift at Variation 100 %
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Drift {
    /// semitones either way
    pub pitch: f32,
    /// dB either way
    pub level: f32,
    /// tone macro, 0..1 either way
    pub tone: f32,
    /// decay length, a fraction either way (0.2 = ±20 %)
    pub decay: f32,
    /// milliseconds late (hits only ever land later, never early)
    pub timing: f32,
    /// pan either way (-1..1 scale)
    pub pan: f32,
}

impl Default for Drift {
    fn default() -> Self {
        Drift {
            pitch: 0.3,
            level: 2.0,
            tone: 0.08,
            decay: 0.15,
            timing: 6.0,
            pan: 0.15,
        }
    }
}

impl Drift {
    /// the largest range per part, in the order hits draw them
    pub const MAX: [f32; 6] = [2.0, 6.0, 0.3, 0.5, 20.0, 0.5];
    pub fn get(&self, i: usize) -> f32 {
        [
            self.pitch,
            self.level,
            self.tone,
            self.decay,
            self.timing,
            self.pan,
        ][i]
    }
    pub fn set(&mut self, i: usize, v: f32) {
        let v = if v.is_finite() {
            v.clamp(0.0, Self::MAX[i])
        } else {
            0.0
        };
        match i {
            0 => self.pitch = v,
            1 => self.level = v,
            2 => self.tone = v,
            3 => self.decay = v,
            4 => self.timing = v,
            _ => self.pan = v,
        }
    }
}

/// names, units and beginner tooltips of the drift ranges
pub const DRIFT_UI: [(&str, &str, &str); 6] = [
    (
        "Pitch",
        "st",
        "how far a hit's pitch may move, in semitones",
    ),
    ("Level", "dB", "how much louder or softer a hit may be"),
    ("Tone", "", "how much brighter or darker a hit may sound"),
    ("Decay", "%", "how much shorter or longer a hit may ring"),
    (
        "Timing",
        "ms",
        "how late a hit may land, like a human drummer",
    ),
    ("Pan", "", "how far left or right a hit may sit"),
];

/// a pad's Round Robin settings
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PadRr {
    /// which sound each hit plays
    pub order: Order,
    /// the extra sounds (the pad's own drum is sound A)
    pub alts: Vec<DrumP>,
    /// 0..1: how much of the drift ranges each hit uses
    pub variation: f32,
    pub drift: Drift,
    pub seed: u32,
}

impl Default for PadRr {
    fn default() -> Self {
        PadRr {
            order: Order::RandomNoRepeat,
            alts: vec![],
            variation: 0.0,
            drift: Drift::default(),
            seed: 0x5244_4D31,
        }
    }
}

impl PadRr {
    /// sounds in the rotation (the pad's own counts)
    pub fn sounds(&self) -> usize {
        (1 + self.alts.len()).min(MAX_SOUNDS)
    }
    pub fn fix(&mut self) {
        self.alts.truncate(MAX_SOUNDS - 1);
        self.alts.iter_mut().for_each(DrumP::fix);
        self.variation = if self.variation.is_finite() {
            self.variation.clamp(0.0, 1.0)
        } else {
            0.0
        };
        for i in 0..6 {
            let v = self.drift.get(i);
            self.drift.set(i, v);
        }
        if let Order::EveryN { n, which } = self.order {
            self.order = Order::EveryN {
                n: n.clamp(2, 16),
                which: which.min(MAX_SOUNDS as u8 - 1),
            };
        }
    }
}

/// what one hit plays: its sound and its drift
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitMod {
    /// 0 = the pad's own drum, 1.. = `alts[i - 1]`
    pub sound: u8,
    pub semis: f32,
    /// linear gain
    pub gain: f32,
    /// added to the tone macro (0..1 scale)
    pub tone: f32,
    /// decay scale (1 = as set)
    pub decay: f32,
    /// seconds of delay before the hit
    pub delay: f32,
    /// added to the pad's pan
    pub pan: f32,
}

/// hit number `hit` on a pad with these settings (pure)
pub fn hit_mod(r: &PadRr, hit: u32) -> HitMod {
    let sound = rr::pick(r.order, r.sounds(), r.seed, rr::SALT_HIT, hit) as u8;
    let mut g = ClipRng::new(r.seed ^ rr::SALT_DRIFT, hit);
    // the six draws are always taken, in order, so no range moves another
    let mut d = [0.0f32; 6];
    for x in &mut d {
        *x = g.next_f32() * 2.0 - 1.0;
    }
    let a = r.variation;
    let dr = &r.drift;
    if a <= 0.0 {
        return HitMod {
            sound,
            gain: 1.0,
            decay: 1.0,
            ..Default::default()
        };
    }
    HitMod {
        sound,
        semis: d[0] * dr.pitch * a,
        gain: 10f32.powf(d[1] * dr.level * a / 20.0),
        tone: d[2] * dr.tone * a,
        decay: (1.0 + d[3] * dr.decay * a).max(0.1),
        delay: (d[4] * 0.5 + 0.5) * dr.timing * a / 1000.0,
        pan: d[5] * dr.pan * a,
    }
}

/// the macro a hit's Tone drift moves on drum kind `k` (None: no such macro)
pub fn tone_macro(k: crate::gen::DrumKind) -> Option<usize> {
    let m = k.macros();
    ["Tone", "Color", "Body", "Metal"]
        .iter()
        .find_map(|n| m.iter().position(|d| d.name == *n))
}

/// the drum hit `m` plays on a pad (its sound, with the Tone drift)
pub fn hit_drum(own: &DrumP, r: &PadRr, m: &HitMod) -> DrumP {
    let mut d = match m.sound {
        0 => *own,
        i => r.alts.get(i as usize - 1).copied().unwrap_or(*own),
    };
    if m.tone != 0.0 {
        if let Some(i) = tone_macro(d.kind) {
            d.macros[i] = (d.macros[i] + m.tone).clamp(0.0, 1.0);
        }
    }
    d
}

/// sound letters for the next `n` hits from hit `from` (the "Next 8 hits" row)
pub fn preview(r: &PadRr, from: u32, n: u32) -> Vec<u8> {
    (from..from + n).map(|k| hit_mod(r, k).sound).collect()
}

/// hit numbers per key for notes that are not on the timeline (tracker
/// rows, live keys, rack layers, previews): counted from the start of play,
/// so an export from the start always plays the same hits
#[derive(Clone, Copy, Debug)]
pub struct HitCount([u32; 128]);

impl Default for HitCount {
    fn default() -> Self {
        HitCount([0; 128])
    }
}

impl HitCount {
    /// the next hit number for `key`
    pub fn next(&mut self, key: u8) -> u32 {
        let c = &mut self.0[(key as usize).min(127)];
        let h = *c;
        *c = c.wrapping_add(1);
        h
    }
}
