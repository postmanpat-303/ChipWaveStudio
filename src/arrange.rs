//! Fill to length (Auto Arrange), docs/AUTO_ARRANGE.md.
//!
//! 1. A style is a weighted role-transition grammar (Markov FSM); only end roles (Outro) may end.
//! 2. A constrained shortest path (dynamic programming over states (role, last block, repeat
//!    count, pads used) × rows elapsed) lands exactly on the target length.
//!    Cost = −ln p(transition) + energy-curve error + repetition penalty + seeded noise.
//! 3. A seeded simulated-annealing pass adds variations without changing the length; locked
//!    sections are frozen.
//! 4. Fallbacks: 1–2-bar pad fills inside the DP, then ±1 bar or a ≤2 % tempo nudge, reported.
//!
//! Inspired by music retargeting (Rubin & Agrawala, UIST 2014 / radiotool, ISC; Wenner et al.,
//! Eurographics 2013). No code was copied; see docs/research/fill_to_length_algorithms.md.
use crate::model::{Cell, Pattern, Song, NOTE_OFF};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Role {
    Intro,
    Verse,
    Chorus,
    Bridge,
    Breakdown,
    Outro,
    Fill,
}

pub const ROLES: [Role; 7] = [
    Role::Intro,
    Role::Verse,
    Role::Chorus,
    Role::Bridge,
    Role::Breakdown,
    Role::Outro,
    Role::Fill,
];

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Intro => "Intro",
            Role::Verse => "Verse",
            Role::Chorus => "Chorus",
            Role::Bridge => "Bridge",
            Role::Breakdown => "Breakdown",
            Role::Outro => "Outro",
            Role::Fill => "Fill",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Role::Intro => "IN",
            Role::Verse => "VS",
            Role::Chorus => "CH",
            Role::Bridge => "BR",
            Role::Breakdown => "BD",
            Role::Outro => "OUT",
            Role::Fill => "FL",
        }
    }
    pub fn from_name(s: &str) -> Option<Role> {
        ROLES
            .into_iter()
            .find(|r| r.name().eq_ignore_ascii_case(s.trim()))
    }
    /// muted badge palette
    pub fn colour(self) -> [u8; 3] {
        match self {
            Role::Intro => [0x6f, 0x8f, 0xa8],
            Role::Verse => [0x7f, 0xa0, 0x7a],
            Role::Chorus => [0xc0, 0x8f, 0x5a],
            Role::Bridge => [0x9a, 0x82, 0xb0],
            Role::Breakdown => [0x5f, 0x9e, 0x9a],
            Role::Outro => [0x8a, 0x8a, 0x8a],
            Role::Fill => [0xb0, 0x76, 0x7a],
        }
    }
    fn prior(self) -> f32 {
        match self {
            Role::Intro => 0.3,
            Role::Verse => 0.5,
            Role::Chorus => 0.9,
            Role::Bridge => 0.55,
            Role::Breakdown => 0.25,
            Role::Outro => 0.2,
            Role::Fill => 0.7,
        }
    }
}

/// Non-destructive variation transforms, applied in order to the source block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VarOp {
    /// keep only the first n rows (length fitting, short pad fills)
    TakeRows(u16),
    /// semitones, melodic channels only
    Transpose(i8),
    /// octave shift on the channels in the mask (melodic only)
    Octave { mask: u8, oct: i8 },
    /// replace the last bar of a drum channel with a seeded roll
    DrumFill { ch: u8 },
    /// clear channels (note-off at row 0)
    Mute(u8),
    /// drop notes on the masked channels, off-beats first; keep = fraction kept
    Thin { mask: u8, keep: f32 },
    /// <1 removes weak-beat notes, >1 adds ghost repeats on empty even rows (melodic)
    Density(f32),
    /// key change (from the final chorus on), melodic only
    KeyChange(i8),
    /// volume ramp over the last n bars
    FadeOut { bars: u8 },
}

impl VarOp {
    pub fn label(&self) -> String {
        match self {
            VarOp::TakeRows(n) => format!("first {n} rows"),
            VarOp::Transpose(s) => format!("transpose {s:+}"),
            VarOp::Octave { oct, .. } => format!("octave {oct:+}"),
            VarOp::DrumFill { .. } => "drum fill".into(),
            VarOp::Mute(_) => "mute drums".into(),
            VarOp::Thin { keep, .. } => format!("thin {:.0} %", keep * 100.0),
            VarOp::Density(f) if *f > 1.0 => "density +".into(),
            VarOp::Density(_) => "density −".into(),
            VarOp::KeyChange(s) => format!("key change {s:+}"),
            VarOp::FadeOut { bars } => format!("fade {bars} bars"),
        }
    }
    fn energy(&self) -> f32 {
        match self {
            VarOp::Thin { keep, .. } => -(1.0 - keep) * 0.4,
            VarOp::Mute(_) => -0.25,
            VarOp::Density(f) => (f - 1.0) * 0.25,
            VarOp::DrumFill { .. } => 0.05,
            VarOp::Octave { .. } => 0.05,
            VarOp::KeyChange(_) => 0.05,
            VarOp::FadeOut { .. } => -0.1,
            _ => 0.0,
        }
    }
}

/// A derived block: rendered from `source` + `ops`; follows edits of the source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Derived {
    pub source: usize,
    pub ops: Vec<VarOp>,
    pub seed: u64,
    /// drum channel mask (skipped by transpose/key change)
    #[serde(default)]
    pub drums: u8,
}

// ------------------------------------------------------------------ rng

#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
fn noise(seed: u64, a: u64, b: u64) -> f32 {
    (mix(seed ^ mix(a.wrapping_mul(31).wrapping_add(b))) >> 40) as f32 / (1u64 << 24) as f32
}

// ------------------------------------------------------------------ styles

#[derive(Clone, Debug, Deserialize)]
pub struct Style {
    pub name: String,
    #[serde(default = "def_len")]
    pub default_length: String,
    #[serde(default = "def_rep")]
    pub max_repeat: u8,
    pub start: Vec<String>,
    pub end: Vec<String>,
    pub energy: Vec<[f32; 2]>,
    #[serde(default)]
    pub favour: Vec<String>,
    pub next: BTreeMap<String, Vec<(String, f32)>>,
    #[serde(skip)]
    pub file: String,
}
fn def_len() -> String {
    "3:30".into()
}
fn def_rep() -> u8 {
    2
}

pub const FACTORY_STYLES: &[(&str, &str)] = &[
    ("pop.toml", include_str!("../presets/styles/pop.toml")),
    (
        "chiptune.toml",
        include_str!("../presets/styles/chiptune.toml"),
    ),
    ("club.toml", include_str!("../presets/styles/club.toml")),
    (
        "ambient.toml",
        include_str!("../presets/styles/ambient.toml"),
    ),
    (
        "psytrance.toml",
        include_str!("../presets/styles/psytrance.toml"),
    ),
];

impl Style {
    pub fn parse(text: &str, file: &str) -> Result<Style, String> {
        let mut s: Style = toml::from_str(text).map_err(|e| format!("{file}: {e}"))?;
        s.file = file.into();
        s.max_repeat = s.max_repeat.clamp(1, 8);
        Ok(s)
    }
    pub fn factory(i: usize) -> Style {
        let (f, t) = FACTORY_STYLES[i];
        Style::parse(t, f).expect("factory style")
    }
    fn favours(&self, k: &str) -> bool {
        self.favour.iter().any(|f| f == k)
    }
    fn trans(&self, from: Option<Role>) -> Vec<(Role, f32)> {
        let list: Vec<(String, f32)> = match from {
            None => self.start.iter().map(|s| (s.clone(), 1.0)).collect(),
            Some(r) => self.next.get(r.name()).cloned().unwrap_or_default(),
        };
        let mut v: Vec<(Role, f32)> = list
            .iter()
            .filter_map(|(n, p)| Role::from_name(n).map(|r| (r, p.max(0.0))))
            .filter(|(_, p)| *p > 0.0)
            .collect();
        let sum: f32 = v.iter().map(|x| x.1).sum();
        if sum > 0.0 {
            v.iter_mut().for_each(|x| x.1 /= sum);
        }
        v
    }
    fn is_end(&self, r: Role) -> bool {
        self.end.iter().any(|e| Role::from_name(e) == Some(r))
    }
}

/// Styles from the user folder (copied from the factory ones on first use), else the factory set.
pub fn load_styles(dir: Option<&std::path::Path>) -> (Vec<Style>, Vec<String>) {
    let mut errs = vec![];
    if let Some(d) = dir {
        if !d.exists() {
            let _ = std::fs::create_dir_all(d);
            for (f, t) in FACTORY_STYLES {
                let _ = std::fs::write(d.join(f), t);
            }
        }
        let mut files: Vec<_> = std::fs::read_dir(d)
            .map(|r| r.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.retain(|p: &std::path::PathBuf| p.extension().is_some_and(|e| e == "toml"));
        files.sort();
        let mut out = vec![];
        for p in files {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match std::fs::read_to_string(&p)
                .map_err(|e| e.to_string())
                .and_then(|t| Style::parse(&t, &name))
            {
                Ok(s) => out.push(s),
                Err(e) => errs.push(e),
            }
        }
        if !out.is_empty() {
            // factory order first, then user styles
            out.sort_by_key(|s| {
                FACTORY_STYLES
                    .iter()
                    .position(|(f, _)| *f == s.file)
                    .unwrap_or(99)
            });
            return (out, errs);
        }
    }
    (
        (0..FACTORY_STYLES.len()).map(Style::factory).collect(),
        errs,
    )
}

pub fn energy_at(curve: &[[f32; 2]], x: f32) -> f32 {
    if curve.is_empty() {
        return 0.5;
    }
    let mut pts = curve.to_vec();
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
    if x <= pts[0][0] {
        return pts[0][1];
    }
    for w in pts.windows(2) {
        if x <= w[1][0] {
            let f = (x - w[0][0]) / (w[1][0] - w[0][0]).max(1e-6);
            return w[0][1] + (w[1][1] - w[0][1]) * f;
        }
    }
    pts[pts.len() - 1][1]
}

// ------------------------------------------------------------------ proposal

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub role: Role,
    pub block: usize,
    pub rows: usize,
    pub ops: Vec<VarOp>,
    pub locked: bool,
    /// short pad fill inserted by the fitter
    pub pad: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub sections: Vec<Section>,
    pub rows: usize,
    pub target_rows: usize,
    pub rows_per_bar: usize,
    /// tempo after a fallback nudge (None = unchanged)
    pub bpm: Option<f32>,
    pub notes: Vec<String>,
    pub cost: f32,
    pub drums: u8,
}

impl Proposal {
    pub fn seconds(&self, song: &Song) -> f64 {
        self.rows as f64 * 60.0
            / (self.bpm.unwrap_or(song.bpm) as f64 * song.lines_per_beat.max(1) as f64)
    }
    pub fn bars(&self) -> f32 {
        self.rows as f32 / self.rows_per_bar as f32
    }
}

pub struct Params<'a> {
    pub style: &'a Style,
    pub target_bars: usize,
    pub energy: &'a [[f32; 2]],
    /// 0..=1
    pub variation: f32,
    pub seed: u64,
    /// sections to keep (from the previous proposal), matched by their start row
    pub locks: Vec<(usize, Section)>,
}

pub fn rows_per_bar(song: &Song) -> usize {
    (song.lines_per_beat.max(1) * 4) as usize
}
pub fn bar_secs(song: &Song) -> f64 {
    4.0 * 60.0 / song.bpm as f64
}

/// drum channels: those mostly playing instruments whose name looks like a drum
pub fn drum_mask(song: &Song) -> u8 {
    let is_drum = |i: u8| {
        song.instruments
            .get((i as usize).wrapping_sub(1))
            .is_some_and(|ins| {
                let n = ins.name.to_lowercase();
                [
                    "kick", "hat", "snare", "clap", "drum", "perc", "tom", "cym", "rim", "shaker",
                ]
                .iter()
                .any(|k| n.contains(k))
            })
    };
    let mut mask = 0u8;
    for ch in 0..song.channels.min(8) {
        let (mut d, mut all) = (0, 0);
        for p in &song.patterns {
            for r in &p.cells {
                if let Some(c) = r.get(ch) {
                    if c.note > 0 && c.note != NOTE_OFF {
                        all += 1;
                        if is_drum(c.instr) {
                            d += 1;
                        }
                    }
                }
            }
        }
        if all > 0 && d * 2 > all {
            mask |= 1 << ch;
        }
    }
    mask
}

fn block_energy(song: &Song, b: usize, role: Role, maxd: f32) -> f32 {
    let p = &song.patterns[b];
    let notes = p
        .cells
        .iter()
        .flatten()
        .filter(|c| c.note > 0 && c.note != NOTE_OFF)
        .count();
    let d = notes as f32 / p.rows.max(1) as f32;
    0.5 * (d / maxd.max(1e-6)).min(1.0) + 0.5 * role.prior()
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Key {
    role: Option<Role>,
    last: usize,
    rep: u8,
    pads: u8,
}

#[derive(Clone, Copy)]
struct Item {
    role: Role,
    block: usize,
    rows: usize,
    pad: bool,
}

/// Generate a proposal. Deterministic in (song blocks, params). If the style grammar can't
/// reach the length with the tagged blocks, a relaxed pass lets any tagged role follow.
pub fn generate(song: &Song, p: &Params) -> Result<Proposal, String> {
    match generate_with(song, p, false) {
        Ok(r) => Ok(r),
        Err(_) => {
            let mut r = generate_with(song, p, true)?;
            r.notes.insert(
                0,
                "style rules relaxed: tag more sections for a closer match".into(),
            );
            Ok(r)
        }
    }
}

fn generate_with(song: &Song, p: &Params, relaxed: bool) -> Result<Proposal, String> {
    let rpb = rows_per_bar(song);
    let target = p.target_bars.max(1) * rpb;
    let mut notes = vec![];
    // tagged, non-derived blocks
    let tagged: Vec<(usize, Role)> = song
        .patterns
        .iter()
        .enumerate()
        .filter(|(_, b)| b.derived.is_none())
        .filter_map(|(i, b)| b.role.map(|r| (i, r)))
        .collect();
    if tagged.is_empty() {
        return Err(
            "No blocks have a role yet. Tag blocks in the arranger: keys 1–7 or right-click."
                .into(),
        );
    }
    let maxd = tagged
        .iter()
        .map(|&(b, _)| {
            let pt = &song.patterns[b];
            pt.cells
                .iter()
                .flatten()
                .filter(|c| c.note > 0 && c.note != NOTE_OFF)
                .count() as f32
                / pt.rows.max(1) as f32
        })
        .fold(0.0f32, f32::max);
    let energy_of: BTreeMap<usize, f32> = tagged
        .iter()
        .map(|&(b, r)| (b, block_energy(song, b, r, maxd)))
        .collect();
    for r in ROLES {
        let used = p.style.next.contains_key(r.name())
            || p.style
                .next
                .values()
                .flatten()
                .any(|(n, _)| Role::from_name(n) == Some(r));
        if used && !tagged.iter().any(|t| t.1 == r) && r != Role::Fill {
            notes.push(format!("no {} tagged: skipped", r.name()));
        }
    }
    let has_end = tagged.iter().any(|t| p.style.is_end(t.1));
    if !has_end {
        notes.push("no Outro tagged: the song may end on any section".into());
    }
    // items: full blocks + 1/2-bar pad fills (from Fill-tagged blocks, else any block)
    let mut items: Vec<Item> = tagged
        .iter()
        .map(|&(b, r)| Item {
            role: r,
            block: b,
            rows: song.patterns[b].rows.max(1),
            pad: false,
        })
        .collect();
    let pad_src: Vec<usize> = {
        let f: Vec<usize> = tagged
            .iter()
            .filter(|t| t.1 == Role::Fill)
            .map(|t| t.0)
            .collect();
        if f.is_empty() {
            tagged
                .iter()
                .filter(|t| t.1 != Role::Outro && t.1 != Role::Intro)
                .map(|t| t.0)
                .take(1)
                .collect()
        } else {
            f
        }
    };
    for &b in &pad_src {
        let full = song.patterns[b].rows;
        for bars in [1usize, 2] {
            if full > bars * rpb {
                items.push(Item {
                    role: Role::Fill,
                    block: b,
                    rows: bars * rpb,
                    pad: true,
                });
            }
        }
        if full <= 4 * rpb && song.patterns[b].role == Some(Role::Fill) {
            items.push(Item {
                role: Role::Fill,
                block: b,
                rows: full,
                pad: true,
            });
        }
    }
    let avail: Vec<Role> = ROLES
        .into_iter()
        .filter(|r| tagged.iter().any(|t| t.1 == *r))
        .collect();
    let locks: BTreeMap<usize, &Section> = p.locks.iter().map(|(t, s)| (*t, s)).collect();
    let horizon = target + rpb;
    let noise_amp = 0.35 + 0.65 * p.variation;
    // dp[t]: key -> (cost, back (t, key), item index)
    type Back = Option<(usize, Key, usize)>;
    let mut dp: Vec<BTreeMap<Key, (f32, Back)>> = vec![BTreeMap::new(); horizon + 1];
    dp[0].insert(
        Key {
            role: None,
            last: usize::MAX,
            rep: 0,
            pads: 0,
        },
        (0.0, None),
    );
    for t in 0..horizon {
        if dp[t].is_empty() {
            continue;
        }
        let states: Vec<(Key, f32)> = dp[t].iter().map(|(k, v)| (*k, v.0)).collect();
        for (k, cost) in states {
            if k.role.is_some_and(|r| p.style.is_end(r)) {
                continue; // end roles are terminal
            }
            let mut trans = skip_untagged(p.style, k.role, &avail);
            if relaxed && k.role.is_some() {
                for r in &avail {
                    if !trans.iter().any(|x| x.0 == *r) {
                        trans.push((*r, 0.03));
                    }
                }
            }
            if trans.is_empty() {
                // the grammar's next roles aren't tagged: skip them, any tagged role may follow
                trans = avail
                    .iter()
                    .filter(|r| k.role.is_some() || !p.style.is_end(**r))
                    .map(|r| (*r, 0.05))
                    .collect();
            }
            for (ii, it) in items.iter().enumerate() {
                let t2 = t + it.rows;
                if t2 > horizon {
                    continue;
                }
                // locks: at a lock start only that block; nothing may straddle a lock
                if let Some(l) = locks.get(&t) {
                    if l.block != it.block || l.rows != it.rows {
                        continue;
                    }
                } else if locks.range(t + 1..t2).next().is_some() {
                    continue;
                }
                let (prob, next_key) = if it.pad {
                    if k.pads >= 3 || k.role.is_none() {
                        continue;
                    }
                    (
                        0.12,
                        Key {
                            pads: k.pads + 1,
                            ..k
                        },
                    )
                } else {
                    let pr = trans.iter().find(|x| x.0 == it.role).map(|x| x.1);
                    let pr = match pr {
                        Some(v) => v,
                        None if locks.contains_key(&t) => 0.05,
                        // no outro tagged: any section may follow weakly
                        None if !has_end && k.role.is_some() => 0.02,
                        None => continue,
                    };
                    let rep = if k.last == it.block { k.rep + 1 } else { 1 };
                    if rep > p.style.max_repeat + if relaxed { 2 } else { 0 } {
                        continue;
                    }
                    (
                        pr.max(0.02),
                        Key {
                            role: Some(it.role),
                            last: it.block,
                            rep,
                            pads: k.pads,
                        },
                    )
                };
                let mid = (t as f32 + it.rows as f32 / 2.0) / target as f32;
                let e = energy_of.get(&it.block).copied().unwrap_or(0.5);
                let eerr = (e - energy_at(p.energy, mid.min(1.0))).powi(2) * 6.0 * it.rows as f32
                    / target as f32
                    * 8.0;
                let rep_pen = if next_key.rep > 1 {
                    0.6 * (next_key.rep - 1) as f32
                } else {
                    0.0
                };
                let nz =
                    noise(p.seed, t as u64, (ii as u64) << 8 | next_key.rep as u64) * noise_amp;
                let c = cost - prob.ln() + eerr + rep_pen + nz;
                let e = dp[t2].entry(next_key).or_insert((f32::INFINITY, None));
                if c < e.0 {
                    *e = (c, Some((t, k, ii)));
                }
            }
        }
    }
    // pick end: exact first, then ±1 bar
    let end_ok = |k: &Key| {
        k.role
            .is_some_and(|r| if has_end { p.style.is_end(r) } else { true })
    };
    let best_at = |t: usize| {
        dp.get(t).and_then(|m| {
            m.iter()
                .filter(|(k, _)| end_ok(k))
                .min_by(|a, b| a.1 .0.total_cmp(&b.1 .0))
                .map(|(k, v)| (*k, v.0))
        })
    };
    let mut pick = best_at(target).map(|x| (target, x));
    if pick.is_none() {
        for d in 1..=rpb {
            for t in [target.wrapping_sub(d), target + d] {
                if t <= horizon {
                    if let Some(x) = best_at(t) {
                        pick = Some((t, x));
                        break;
                    }
                }
            }
            if pick.is_some() {
                break;
            }
        }
    }
    let Some((tend, (kend, cost))) = pick else {
        return Err("Could not fit the tagged blocks to this length; try a longer target or tag more blocks.".into());
    };
    // backtrack
    let mut secs = vec![];
    let (mut t, mut k) = (tend, kend);
    while let Some(&(_, Some((pt, pk, ii)))) = dp[t].get(&k) {
        let it = items[ii];
        let mut ops = vec![];
        if it.pad {
            ops.push(VarOp::TakeRows(it.rows as u16));
        }
        let mut s = Section {
            role: it.role,
            block: it.block,
            rows: it.rows,
            ops,
            locked: false,
            pad: it.pad,
        };
        if let Some(l) = locks.get(&pt) {
            s = (*l).clone();
            s.locked = true;
        }
        secs.push(s);
        t = pt;
        k = pk;
    }
    secs.reverse();
    let drums = drum_mask(song);
    let mut prop = Proposal {
        sections: secs,
        rows: tend,
        target_rows: target,
        rows_per_bar: rpb,
        bpm: None,
        notes,
        cost,
        drums,
    };
    if tend != target {
        let ratio = tend as f32 / target as f32;
        if (ratio - 1.0).abs() <= 0.02 {
            prop.bpm = Some(song.bpm * ratio);
            prop.notes.push(format!(
                "tempo nudged {:+.1} % to land on the target",
                (ratio - 1.0) * 100.0
            ));
        } else {
            let d = (tend as f64 - target as f64) * 60.0
                / (song.bpm as f64 * song.lines_per_beat.max(1) as f64);
            prop.notes
                .push(format!("{:+.1} s from the target (no exact fit)", d));
        }
    }
    anneal(song, &mut prop, p, &energy_of);
    Ok(prop)
}

/// grammar successors restricted to tagged roles; an untagged role is skipped and
/// replaced by its own successors (up to two levels), weights multiplied
fn skip_untagged(style: &Style, from: Option<Role>, avail: &[Role]) -> Vec<(Role, f32)> {
    let mut out: BTreeMap<Role, f32> = BTreeMap::new();
    let mut frontier: Vec<(Role, f32)> = style.trans(from);
    for depth in 0..3 {
        let mut next = vec![];
        for (r, p) in frontier {
            if avail.contains(&r) {
                *out.entry(r).or_insert(0.0) += p;
            } else if depth < 2 && Some(r) != from {
                next.extend(
                    style
                        .trans(Some(r))
                        .into_iter()
                        .map(|(r2, p2)| (r2, p * p2)),
                );
            }
        }
        frontier = next;
    }
    out.into_iter().filter(|x| x.1 > 0.0).collect()
}

// ------------------------------------------------------------------ annealing

fn melodic(song: &Song, drums: u8) -> Vec<u8> {
    (0..song.channels.min(8) as u8)
        .filter(|c| drums & (1 << c) == 0)
        .collect()
}

fn plan_cost(prop: &Proposal, p: &Params, energy_of: &BTreeMap<usize, f32>) -> f32 {
    let v = p.variation;
    let mut c = 0.0;
    let mut seen: Vec<(usize, &Vec<VarOp>)> = vec![];
    let mut t = 0usize;
    let n = prop.sections.len();
    for (i, s) in prop.sections.iter().enumerate() {
        let base = energy_of.get(&s.block).copied().unwrap_or(0.5);
        let e = (base + s.ops.iter().map(|o| o.energy()).sum::<f32>()).clamp(0.0, 1.0);
        let mid = (t as f32 + s.rows as f32 / 2.0) / prop.rows.max(1) as f32;
        c += (e - energy_at(p.energy, mid)).powi(2) * 4.0;
        t += s.rows;
        let real_ops: Vec<&VarOp> = s
            .ops
            .iter()
            .filter(|o| !matches!(o, VarOp::TakeRows(_)))
            .collect();
        // novelty: identical repeat of an earlier section
        if seen.iter().any(|(b, o)| *b == s.block && **o == s.ops) {
            c += 0.4 + 1.6 * v;
        }
        seen.push((s.block, &s.ops));
        c += real_ops.len() as f32 * (0.15 + (1.0 - v) * 0.9);
        let next = prop.sections.get(i + 1).map(|x| x.role);
        for o in real_ops {
            let fav = |k: &str| if p.style.favours(k) { -0.35 } else { 0.1 };
            c += match o {
                VarOp::DrumFill { .. } if next == Some(Role::Chorus) => -0.4 + fav("drum_fill"),
                VarOp::DrumFill { .. } => 0.5,
                VarOp::Mute(_) if s.role == Role::Breakdown || s.role == Role::Intro => {
                    -0.3 + fav("mute_drums")
                }
                VarOp::Mute(_) => 0.6,
                VarOp::FadeOut { .. } if i + 1 == n => -0.5 + fav("fade"),
                VarOp::FadeOut { .. } => 5.0,
                VarOp::KeyChange(_) => fav("key_change"),
                VarOp::Transpose(_) => fav("transpose"),
                VarOp::Octave { .. } => fav("octave"),
                VarOp::Thin { .. } => fav("thin"),
                VarOp::Density(_) => fav("density"),
                VarOp::TakeRows(_) => 0.0,
            };
        }
    }
    // key change must run from one chorus to the end, all with the same amount
    if let Some(first) = prop
        .sections
        .iter()
        .position(|s| s.ops.iter().any(|o| matches!(o, VarOp::KeyChange(_))))
    {
        let tail_ok = prop.sections[first..]
            .iter()
            .all(|s| s.ops.iter().any(|o| matches!(o, VarOp::KeyChange(_))));
        let at_chorus = prop.sections[first].role == Role::Chorus
            && prop.sections[..first]
                .iter()
                .any(|s| s.role == Role::Chorus);
        if !tail_ok || !at_chorus {
            c += 10.0;
        }
    }
    c
}

fn random_op(song: &Song, prop: &Proposal, i: usize, rng: &mut Rng) -> Option<VarOp> {
    let mel = melodic(song, prop.drums);
    let drums: Vec<u8> = (0..8u8).filter(|c| prop.drums & (1 << c) != 0).collect();
    let s = &prop.sections[i];
    let bars = (s.rows / prop.rows_per_bar).max(1);
    Some(match rng.below(8) {
        0 => VarOp::Transpose([2i8, -2, 5, 7, -5][rng.below(5)]),
        1 if !mel.is_empty() => VarOp::Octave {
            mask: 1 << mel[rng.below(mel.len())],
            oct: if rng.f32() < 0.6 { 1 } else { -1 },
        },
        2 if !drums.is_empty() => VarOp::DrumFill {
            ch: drums[rng.below(drums.len())],
        },
        3 if prop.drums != 0 => VarOp::Mute(prop.drums),
        4 => {
            let m = if mel.is_empty() {
                0xff
            } else {
                mel.iter().fold(0u8, |a, c| a | 1 << c)
            };
            VarOp::Thin {
                mask: m,
                keep: [0.5f32, 0.65, 0.8][rng.below(3)],
            }
        }
        5 => VarOp::Density([0.6f32, 1.4][rng.below(2)]),
        6 => VarOp::FadeOut {
            bars: bars.min(4) as u8,
        },
        7 => VarOp::KeyChange([1i8, 2][rng.below(2)]),
        _ => return None,
    })
}

fn anneal(song: &Song, prop: &mut Proposal, p: &Params, energy_of: &BTreeMap<usize, f32>) {
    let n = prop.sections.len();
    if n == 0 || p.variation <= 0.0 {
        return;
    }
    let mut rng = Rng::new(p.seed ^ 0x00A1_1EA1);
    let mut cur = plan_cost(prop, p, energy_of);
    let mut best = (cur, prop.sections.clone());
    let iters = 2000;
    for it in 0..iters {
        let temp = 1.0 * (1.0 - it as f32 / iters as f32) + 0.01;
        let free: Vec<usize> = (0..n)
            .filter(|&i| !prop.sections[i].locked && !prop.sections[i].pad)
            .collect();
        if free.is_empty() {
            break;
        }
        let i = free[rng.below(free.len())];
        let backup = prop.sections.clone();
        match rng.below(4) {
            // add a variation
            0 | 1 => {
                if let Some(op) = random_op(song, prop, i, &mut rng) {
                    if let VarOp::KeyChange(k) = op {
                        // key change from this chorus to the end
                        for s in prop.sections[i..].iter_mut().filter(|s| !s.locked) {
                            s.ops.retain(|o| !matches!(o, VarOp::KeyChange(_)));
                            s.ops.push(VarOp::KeyChange(k));
                        }
                    } else if !prop.sections[i]
                        .ops
                        .iter()
                        .any(|o| std::mem::discriminant(o) == std::mem::discriminant(&op))
                    {
                        prop.sections[i].ops.push(op);
                    }
                }
            }
            // remove one
            2 => {
                let s = &mut prop.sections[i];
                let removable: Vec<usize> = (0..s.ops.len())
                    .filter(|&j| !matches!(s.ops[j], VarOp::TakeRows(_)))
                    .collect();
                if !removable.is_empty() {
                    let j = removable[rng.below(removable.len())];
                    if matches!(s.ops[j], VarOp::KeyChange(_)) {
                        for s in prop.sections.iter_mut().filter(|s| !s.locked) {
                            s.ops.retain(|o| !matches!(o, VarOp::KeyChange(_)));
                        }
                    } else {
                        s.ops.remove(j);
                    }
                }
            }
            // swap to another block of the same role and length (length unchanged)
            _ => {
                let s = &prop.sections[i];
                let alts: Vec<usize> = song
                    .patterns
                    .iter()
                    .enumerate()
                    .filter(|(j, b)| {
                        *j != s.block
                            && b.derived.is_none()
                            && b.role == Some(s.role)
                            && b.rows == s.rows
                    })
                    .map(|(j, _)| j)
                    .collect();
                if !alts.is_empty() {
                    prop.sections[i].block = alts[rng.below(alts.len())];
                }
            }
        }
        let c = plan_cost(prop, p, energy_of);
        if c <= cur || rng.f32() < ((cur - c) / temp).exp() {
            cur = c;
            if c < best.0 {
                best = (c, prop.sections.clone());
            }
        } else {
            prop.sections = backup;
        }
    }
    prop.sections = best.1;
    prop.cost += best.0;
}

// ------------------------------------------------------------------ transforms

/// Render a derived block from its source pattern.
pub fn apply_ops(src: &Pattern, d: &Derived, rpb: usize) -> (usize, Vec<Vec<Cell>>) {
    let mut cells = src.cells.clone();
    let mut rows = src.rows;
    let chans = cells.first().map(|r| r.len()).unwrap_or(0);
    let mut rng = Rng::new(d.seed);
    let is_note = |c: &Cell| c.note > 0 && c.note != NOTE_OFF;
    let melodic = |ch: usize| d.drums & (1 << ch) == 0;
    let shift = |cells: &mut Vec<Vec<Cell>>, s: i32, mask: u8| {
        for r in cells.iter_mut() {
            for (ch, c) in r.iter_mut().enumerate() {
                if ch < 8 && mask & (1 << ch) != 0 && melodic(ch) && is_note(c) {
                    c.note = (c.note as i32 + s).clamp(1, 120) as u8;
                }
            }
        }
    };
    for op in &d.ops {
        match op {
            VarOp::TakeRows(n) => {
                rows = (*n as usize).clamp(1, rows);
                cells.truncate(rows);
            }
            VarOp::Transpose(s) | VarOp::KeyChange(s) => shift(&mut cells, *s as i32, 0xff),
            VarOp::Octave { mask, oct } => shift(&mut cells, *oct as i32 * 12, *mask),
            VarOp::Mute(mask) => {
                for (ri, r) in cells.iter_mut().enumerate() {
                    for (ch, c) in r.iter_mut().enumerate() {
                        if ch < 8 && mask & (1 << ch) != 0 {
                            *c = Cell::default();
                            if ri == 0 {
                                c.note = NOTE_OFF;
                            }
                        }
                    }
                }
            }
            VarOp::Thin { mask, keep } => {
                for (ri, r) in cells.iter_mut().enumerate() {
                    let strong = ri % (rpb / 4).max(1) == 0;
                    for (ch, c) in r.iter_mut().enumerate() {
                        if ch < 8 && mask & (1 << ch) != 0 && is_note(c) {
                            let p_drop = if strong {
                                (1.0 - keep) * 0.3
                            } else {
                                1.0 - keep
                            };
                            if rng.f32() < p_drop {
                                *c = Cell::default();
                            }
                        }
                    }
                }
            }
            VarOp::Density(f) => {
                if *f < 1.0 {
                    for (ri, r) in cells.iter_mut().enumerate() {
                        if ri % (rpb / 4).max(1) == 0 {
                            continue;
                        }
                        for c in r.iter_mut() {
                            if is_note(c) && rng.f32() < 1.0 - f {
                                *c = Cell::default();
                            }
                        }
                    }
                } else {
                    for ch in (0..chans).filter(|&c| c < 8 && melodic(c)) {
                        let mut last: Option<Cell> = None;
                        #[allow(clippy::needless_range_loop)]
                        for ri in 0..rows {
                            let c = cells[ri][ch];
                            if is_note(&c) {
                                last = Some(c);
                            } else if c.note == NOTE_OFF {
                                last = None;
                            } else if c.is_empty() && ri % 2 == 0 {
                                if let Some(l) = last {
                                    if rng.f32() < (f - 1.0).min(1.0) {
                                        cells[ri][ch] = Cell {
                                            vol: 0x18,
                                            fx: 0,
                                            fxp: 0,
                                            ..l
                                        };
                                    }
                                }
                            }
                        }
                    }
                }
            }
            VarOp::DrumFill { ch } => {
                let ch = *ch as usize;
                if ch >= chans || rows == 0 {
                    continue;
                }
                let start = rows.saturating_sub(rpb);
                let used: Vec<Cell> = cells.iter().map(|r| r[ch]).filter(is_note).collect();
                if used.is_empty() {
                    continue;
                }
                let half = start + (rows - start) / 2;
                let step = if rng.f32() < 0.5 { 1 } else { 2 };
                #[allow(clippy::needless_range_loop)]
                for ri in start..rows {
                    let c = &mut cells[ri][ch];
                    if ri < half {
                        continue;
                    }
                    *c = Cell::default();
                    if (ri - half).is_multiple_of(step) {
                        let src = used[rng.below(used.len())];
                        let f = (ri - half) as f32 / (rows - half).max(1) as f32;
                        *c = Cell {
                            vol: (0x14 as f32 + f * 0x2c as f32) as u8 + 1,
                            fx: 0,
                            fxp: 0,
                            ..src
                        };
                    }
                }
            }
            VarOp::FadeOut { bars } => {
                let n = (*bars as usize * rpb).min(rows).max(1);
                let start = rows - n;
                for (ri, r) in cells.iter_mut().enumerate().skip(start) {
                    let g = 1.0 - (ri - start) as f32 / n as f32;
                    for c in r.iter_mut() {
                        if is_note(c) {
                            let v = if c.vol == 0 { 0x40 } else { c.vol as i32 - 1 } as f32;
                            c.vol = (v * g).round() as u8 + 1;
                        }
                    }
                }
            }
        }
    }
    (rows, cells)
}

/// Re-render every derived block from its (current) source. A derived block whose cells were
/// edited by hand since the last render is baked (becomes an ordinary block). Returns the
/// number of blocks baked.
pub fn refresh_derived(song: &mut Song) -> usize {
    let rpb = rows_per_bar(song);
    let mut baked = 0;
    for i in 0..song.patterns.len() {
        let Some(d) = song.patterns[i].derived.clone() else {
            continue;
        };
        let pt = &song.patterns[i];
        if !pt.cache.is_empty() && pt.cache != pt.cells {
            song.patterns[i].derived = None;
            song.patterns[i].cache.clear();
            baked += 1;
            continue;
        }
        let Some(src) = song
            .patterns
            .get(d.source)
            .filter(|s| s.derived.is_none() && d.source != i)
        else {
            song.patterns[i].derived = None;
            continue;
        };
        let (rows, mut cells) = apply_ops(src, &d, rpb);
        for r in cells.iter_mut() {
            r.resize(song.channels, Cell::default());
        }
        let p = &mut song.patterns[i];
        if p.cells != cells || p.rows != rows {
            p.cells = cells.clone();
            p.rows = rows;
        }
        p.cache = cells;
    }
    baked
}

fn derived_name(song: &Song, s: &Section) -> String {
    let base = &song.patterns[s.block].name;
    if s.pad {
        format!("{base} fill")
    } else {
        format!("{base}'")
    }
}

/// Apply a proposal: derived blocks for varied sections are appended (reusing identical ones),
/// the order is replaced. Returns the new order.
pub fn apply(song: &mut Song, prop: &Proposal, seed: u64) {
    let mut order = vec![];
    for (i, s) in prop.sections.iter().enumerate() {
        if s.ops.is_empty() {
            order.push(s.block);
            continue;
        }
        let d = Derived {
            source: s.block,
            ops: s.ops.clone(),
            seed: seed ^ (i as u64).wrapping_mul(0x51),
            drums: prop.drums,
        };
        let existing = song.patterns.iter().position(|p| {
            p.derived
                .as_ref()
                .is_some_and(|x| x.source == d.source && x.ops == d.ops)
        });
        let idx = match existing {
            Some(j) => j,
            None => {
                let mut p = Pattern::new(
                    &derived_name(song, s),
                    song.patterns[s.block].rows,
                    song.channels,
                );
                p.role = Some(s.role);
                p.derived = Some(d);
                song.patterns.push(p);
                song.patterns.len() - 1
            }
        };
        order.push(idx);
    }
    song.order = order;
    if let Some(b) = prop.bpm {
        song.bpm = b;
    }
    refresh_derived(song);
}

/// Song copy with the proposal applied (preview and export of the proposal).
pub fn preview_song(song: &Song, prop: &Proposal, seed: u64) -> Song {
    let mut s = song.clone();
    apply(&mut s, prop, seed);
    s
}

/// "3:29.1" style text
pub fn mmss(secs: f64) -> String {
    let m = (secs / 60.0).floor();
    format!("{}:{:04.1}", m as i64, secs - m * 60.0)
}

/// parse "3:30", "210", "3.5m"
pub fn parse_mmss(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some((m, sec)) = s.split_once(':') {
        let m: f64 = m.trim().parse().ok()?;
        let sec: f64 = if sec.trim().is_empty() {
            0.0
        } else {
            sec.trim().parse().ok()?
        };
        return (m >= 0.0 && (0.0..60.0).contains(&sec)).then_some(m * 60.0 + sec);
    }
    s.parse::<f64>().ok().filter(|v| *v > 0.0)
}

/// target in bars, snapped to `snap` bars (at least one snap unit)
pub fn target_bars(song: &Song, secs: f64, snap: usize) -> usize {
    let snap = snap.max(1);
    let bars = secs / bar_secs(song);
    ((bars / snap as f64).round() as usize).max(1) * snap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pattern;

    fn song() -> Song {
        let mut s = Song::empty();
        s.bpm = 125.0;
        s.channels = 4;
        s.instruments = vec![];
        s.patterns.clear();
        let mk = |name: &str, role: Role, rows: usize, base: u8| {
            let mut p = Pattern::new(name, rows, 4);
            p.role = Some(role);
            for r in (0..rows).step_by(2) {
                p.cells[r][1] = Cell {
                    note: base + (r % 7) as u8,
                    instr: 1,
                    ..Default::default()
                };
                if r % 4 == 0 {
                    p.cells[r][0] = Cell {
                        note: 25,
                        instr: 2,
                        ..Default::default()
                    };
                }
            }
            p
        };
        s.patterns.push(mk("Intro", Role::Intro, 64, 40));
        s.patterns.push(mk("Verse", Role::Verse, 128, 45));
        s.patterns.push(mk("Chorus A", Role::Chorus, 128, 50));
        s.patterns.push(mk("Chorus B", Role::Chorus, 128, 52));
        s.patterns.push(mk("Bridge", Role::Bridge, 64, 48));
        s.patterns.push(mk("Outro", Role::Outro, 64, 40));
        s.patterns.push(mk("Fill", Role::Fill, 32, 30));
        s.order = vec![0, 1, 2];
        s
    }

    fn gen(s: &Song, style: usize, bars: usize, seed: u64, v: f32) -> Proposal {
        let st = Style::factory(style);
        generate(
            s,
            &Params {
                style: &st,
                target_bars: bars,
                energy: &st.energy,
                variation: v,
                seed,
                locks: vec![],
            },
        )
        .unwrap()
    }

    #[test]
    fn deterministic_same_seed_same_result() {
        let s = song();
        for st in 0..FACTORY_STYLES.len() {
            let a = gen(&s, st, 100, 42, 0.6);
            let b = gen(&s, st, 100, 42, 0.6);
            assert_eq!(a, b);
            assert_eq!(
                preview_song(&s, &a, 42).order,
                preview_song(&s, &b, 42).order
            );
        }
        // a different seed usually gives a different plan
        let differs =
            (1..20).any(|k| gen(&s, 0, 100, k, 0.6).sections != gen(&s, 0, 100, 0, 0.6).sections);
        assert!(differs);
    }

    #[test]
    fn exact_length_for_many_targets() {
        let s = song();
        let rpb = rows_per_bar(&s);
        let (mut exact, mut total) = (0, 0);
        for st in 0..FACTORY_STYLES.len() {
            for bars in (24..=160).step_by(3) {
                let p = gen(&s, st, bars, bars as u64, 0.5);
                total += 1;
                if p.rows == bars * rpb {
                    exact += 1;
                } else {
                    // grammar can't reach it: nearest within one bar, tempo nudge or reported
                    assert!(
                        p.rows.abs_diff(bars * rpb) <= rpb,
                        "style {st} bars {bars}: got {}",
                        p.bars()
                    );
                    assert!(!p.notes.is_empty());
                }
                assert_eq!(p.sections.iter().map(|x| x.rows).sum::<usize>(), p.rows);
                assert_eq!(
                    p.sections.last().unwrap().role,
                    Role::Outro,
                    "only outro may end"
                );
                assert!(p.bpm.is_none());
                let ap = preview_song(&s, &p, 1);
                let ap_rows: usize = ap.order.iter().map(|&i| ap.patterns[i].rows).sum();
                assert_eq!(ap_rows, p.rows);
            }
        }
        assert!(exact * 100 >= total * 95, "exact {exact}/{total}");
        eprintln!("exact length hits: {exact}/{total}");
    }

    #[test]
    fn length_within_one_snap_unit() {
        let s = song();
        for snap in [1usize, 4, 8, 16] {
            for secs in [95.0, 150.0, 209.0, 333.0] {
                let tb = target_bars(&s, secs, snap);
                let p = gen(&s, 0, tb, 7, 0.4);
                let got = p.seconds(&s);
                assert!(
                    (got - secs).abs() <= snap as f64 * bar_secs(&s) + 1e-6,
                    "{secs} snap {snap}: {got}"
                );
            }
        }
    }

    #[test]
    fn locks_are_kept_on_reroll() {
        let s = song();
        let st = Style::factory(0);
        let a = gen(&s, 0, 96, 1, 0.7);
        let mut t = 0;
        let mut locks = vec![];
        for (i, x) in a.sections.iter().enumerate() {
            if i == 1 || i == 2 {
                locks.push((t, x.clone()));
            }
            t += x.rows;
        }
        for seed in 2..12 {
            let b = generate(
                &s,
                &Params {
                    style: &st,
                    target_bars: 96,
                    energy: &st.energy,
                    variation: 0.7,
                    seed,
                    locks: locks.clone(),
                },
            )
            .unwrap();
            let mut t = 0;
            let mut found = 0;
            for x in &b.sections {
                if let Some((_, l)) = locks.iter().find(|(lt, _)| *lt == t) {
                    assert_eq!((x.block, &x.ops), (l.block, &l.ops));
                    assert!(x.locked);
                    found += 1;
                }
                t += x.rows;
            }
            assert_eq!(found, 2);
            assert_eq!(b.rows, 96 * 16);
        }
    }

    #[test]
    fn variations_transform_correctly() {
        let s = song();
        let src = &s.patterns[2];
        let d = |ops| Derived {
            source: 2,
            ops,
            seed: 3,
            drums: 0b0001,
        };
        // transpose skips the drum channel
        let (_, c) = apply_ops(src, &d(vec![VarOp::Transpose(2)]), 16);
        assert_eq!(c[0][1].note, src.cells[0][1].note + 2);
        assert_eq!(c[0][0].note, src.cells[0][0].note);
        let (_, c) = apply_ops(
            src,
            &d(vec![VarOp::Octave {
                mask: 0b0010,
                oct: -1,
            }]),
            16,
        );
        assert_eq!(c[2][1].note, src.cells[2][1].note - 12);
        // mute: channel cleared, note-off at row 0
        let (_, c) = apply_ops(src, &d(vec![VarOp::Mute(0b0001)]), 16);
        assert_eq!(c[0][0].note, NOTE_OFF);
        assert!(c[1..].iter().all(|r| r[0].is_empty()));
        // thin / density
        let count = |c: &Vec<Vec<Cell>>, ch: usize| {
            c.iter()
                .filter(|r| r[ch].note > 0 && r[ch].note != NOTE_OFF)
                .count()
        };
        let (_, c) = apply_ops(
            src,
            &d(vec![VarOp::Thin {
                mask: 0b0010,
                keep: 0.5,
            }]),
            16,
        );
        assert!(count(&c, 1) < count(&src.cells, 1));
        assert_eq!(count(&c, 0), count(&src.cells, 0));
        let mut sparse = src.clone();
        for r in (0..sparse.rows).filter(|r| r % 8 != 0) {
            sparse.cells[r][1] = Cell::default();
        }
        let (_, c) = apply_ops(&sparse, &d(vec![VarOp::Density(2.0)]), 16);
        assert!(count(&c, 1) > count(&sparse.cells, 1));
        // drum fill: last bar half rebuilt, earlier bars untouched
        let (_, c) = apply_ops(src, &d(vec![VarOp::DrumFill { ch: 0 }]), 16);
        assert_eq!(&c[..120], &src.cells[..120]);
        assert!(c[120..].iter().filter(|r| r[0].note > 0).count() >= 4);
        // fade: last note quieter than the first one of the faded range
        let (_, c) = apply_ops(src, &d(vec![VarOp::FadeOut { bars: 2 }]), 16);
        assert!(c[126][1].vol < c[96][1].vol);
        // key change and take rows
        let (rows, c) = apply_ops(src, &d(vec![VarOp::TakeRows(16), VarOp::KeyChange(1)]), 16);
        assert_eq!((rows, c.len()), (16, 16));
        assert_eq!(c[0][1].note, src.cells[0][1].note + 1);
    }

    #[test]
    fn derived_follow_source_and_bake_on_edit() {
        let mut s = song();
        let p = gen(&s, 0, 96, 5, 1.0);
        apply(&mut s, &p, 5);
        let di = s
            .patterns
            .iter()
            .position(|p| {
                p.derived.as_ref().is_some_and(|d| {
                    d.ops.iter().any(|o| {
                        matches!(
                            o,
                            VarOp::Transpose(_) | VarOp::KeyChange(_) | VarOp::Octave { .. }
                        )
                    })
                })
            })
            .or_else(|| s.patterns.iter().position(|p| p.derived.is_some()));
        let di = di.expect("some derived block");
        let src = s.patterns[di].derived.as_ref().unwrap().source;
        // edit the source: the derived block follows
        s.patterns[src].cells[0][3] = Cell {
            note: 61,
            instr: 1,
            ..Default::default()
        };
        refresh_derived(&mut s);
        assert!(s.patterns[di].cells[0][3].note > 0);
        // edit the derived block: it is baked
        s.patterns[di].cells[1][3] = Cell {
            note: 70,
            instr: 1,
            ..Default::default()
        };
        assert_eq!(refresh_derived(&mut s), 1);
        assert!(s.patterns[di].derived.is_none());
        assert_eq!(s.patterns[di].cells[1][3].note, 70);
    }

    #[test]
    fn roles_and_derived_saved_in_cwt() {
        let mut s = song();
        let p = gen(&s, 1, 80, 9, 0.8);
        apply(&mut s, &p, 9);
        let path = std::env::temp_dir().join(format!("cw_arr_{}.cwt", std::process::id()));
        crate::fileio::save_cwt(&s, &path).unwrap();
        let mut l = crate::fileio::load_cwt(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(l.order, s.order);
        for (a, b) in l.patterns.iter().zip(&s.patterns) {
            assert_eq!(a.role, b.role);
            assert_eq!(a.derived, b.derived);
            assert_eq!(a.cells, b.cells);
        }
        assert_eq!(refresh_derived(&mut l), 0, "nothing baked after load");
    }

    /// CHIPWAVE_FILL_DEMO=<path>: write a demo song with tagged blocks (screenshots / manual testing)
    #[test]
    fn fill_demo_song() {
        let mut s = crate::fileio::demo_song();
        let base = s.patterns[0].clone();
        let mk = |name: &str, role: Role, f: &dyn Fn(&mut Pattern)| {
            let mut p = base.clone();
            p.name = name.into();
            p.role = Some(role);
            f(&mut p);
            p
        };
        let ch = s.channels;
        let keep = |p: &mut Pattern, chans: &[usize]| {
            for r in p.cells.iter_mut() {
                for (c, cell) in r.iter_mut().enumerate() {
                    if !chans.contains(&c) {
                        *cell = Cell::default();
                    }
                }
            }
        };
        let all: Vec<usize> = (0..ch).collect();
        s.patterns = vec![
            mk("Intro", Role::Intro, &|p| keep(p, &[0, 1])),
            mk("Groove", Role::Verse, &|p| keep(p, &all[..ch.min(3)])),
            mk("Drop", Role::Chorus, &|_| {}),
            mk("Break", Role::Breakdown, &|p| keep(p, &all[1..])),
            mk("Build", Role::Bridge, &|p| keep(p, &[0, 2])),
            mk("Outro", Role::Outro, &|p| keep(p, &[0, 1])),
            mk("Roll", Role::Fill, &|p| {
                p.rows = 32;
                p.cells.truncate(32);
            }),
        ];
        s.order = vec![0, 1, 2];
        s.automation.clear();
        s.title = "Fill demo".into();
        let st = Style::factory(4);
        let pr = generate(
            &s,
            &Params {
                style: &st,
                target_bars: 120,
                energy: &st.energy,
                variation: 0.5,
                seed: 1,
                locks: vec![],
            },
        )
        .unwrap();
        assert_eq!(pr.rows, 120 * rows_per_bar(&s));
        if let Ok(path) = std::env::var("CHIPWAVE_FILL_DEMO") {
            crate::fileio::save_cwt(&s, std::path::Path::new(&path)).unwrap();
        }
    }

    #[test]
    fn accepted_result_renders_for_export() {
        let mut s = crate::fileio::demo_song();
        let mut p2 = s.patterns[0].clone();
        s.patterns[0].role = Some(Role::Intro);
        p2.role = Some(Role::Verse);
        s.patterns.push(p2.clone());
        p2.role = Some(Role::Outro);
        s.patterns.push(p2);
        let st = Style::factory(0);
        let pr = generate(
            &s,
            &Params {
                style: &st,
                target_bars: 24,
                energy: &st.energy,
                variation: 0.6,
                seed: 3,
                locks: vec![],
            },
        )
        .unwrap();
        apply(&mut s, &pr, 3);
        let data = crate::engine::render_song(&s, 22050);
        let secs = data.len() as f64 / 2.0 / 22050.0;
        // render adds a 3 s tail
        assert!(
            (secs - 3.0 - pr.seconds(&s)).abs() < 0.5,
            "{secs} vs {}",
            pr.seconds(&s)
        );
        assert!(data.iter().any(|x| x.abs() > 0.05));
    }

    #[test]
    fn styles_parse_and_mmss() {
        for i in 0..FACTORY_STYLES.len() {
            let s = Style::factory(i);
            assert!(s.is_end(Role::Outro));
            assert!(!s.trans(None).is_empty());
        }
        assert_eq!(parse_mmss("3:30"), Some(210.0));
        assert_eq!(mmss(209.1), "3:29.1");
    }

    #[test]
    fn fallback_nudges_or_reports() {
        // only 4-bar blocks and no Fill: pads come from a 4-bar block (1 or 2 bars) so odd targets fit
        let mut s = song();
        s.patterns.retain(|p| p.rows == 64);
        let st = Style::factory(0);
        let p = generate(
            &s,
            &Params {
                style: &st,
                target_bars: 61,
                energy: &st.energy,
                variation: 0.0,
                seed: 1,
                locks: vec![],
            },
        )
        .unwrap();
        assert_eq!(p.rows, 61 * 16);
        // only one 8-bar Intro + 8-bar Outro: 17 bars can't fit exactly (pads need a middle block)
        let mut s = song();
        s.patterns
            .retain(|p| matches!(p.role, Some(Role::Intro) | Some(Role::Outro)));
        let p = generate(
            &s,
            &Params {
                style: &st,
                target_bars: 9,
                energy: &st.energy,
                variation: 0.0,
                seed: 1,
                locks: vec![],
            },
        );
        if let Ok(p) = p {
            assert!(p.bpm.is_some() || !p.notes.is_empty());
        }
    }
}
