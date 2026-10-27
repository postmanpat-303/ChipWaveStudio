//! Arp: a note stepper in front of a track's instrument (0.2.16).
//!
//! The held notes of a track (note clips on the timeline, or keys played
//! live) go into a [`Stepper`]; it plays them one step at a time on a grid
//! locked to the song position, shaped by a pattern of up to 32 steps
//! (velocity, gate, accent, rest / tie, ratchet, transpose). A Drum Kit gets
//! Note repeat instead: every held pad hits on each step.
//!
//! Everything here is allocation-free and a pure function of the song
//! position and the held notes, so a seek lands on the same notes as a full
//! play, and Random mode is seeded. Original ChipWave code, asked for by
//! Del (postmanpat).
use serde::{Deserialize, Serialize};

/// held notes kept per stepper
pub const MAX_HELD: usize = 16;
/// pattern steps kept (the grid shows 16; the model allows 32)
pub const MAX_STEPS: usize = 32;
/// notes one call can fire (a chord on a step, or several hits)
pub const MAX_OUT: usize = 32;
/// ticks per quarter note (as the timeline)
const PPQ: f64 = crate::timeline::PPQ as f64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Up,
    Down,
    UpDown,
    DownUp,
    Random,
    /// in the order the keys went down
    Played,
    /// every held note together on each step
    Chord,
    /// outside in: lowest, highest, second lowest, second highest ...
    Converge,
    /// inside out: from the middle note outwards
    Diverge,
}

impl Mode {
    pub const ALL: [Mode; 9] = [
        Mode::Up,
        Mode::Down,
        Mode::UpDown,
        Mode::DownUp,
        Mode::Converge,
        Mode::Diverge,
        Mode::Random,
        Mode::Played,
        Mode::Chord,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Mode::Up => "Up",
            Mode::Down => "Down",
            Mode::UpDown => "Up-down",
            Mode::DownUp => "Down-up",
            Mode::Random => "Random",
            Mode::Played => "As played",
            Mode::Chord => "Chord",
            Mode::Converge => "Converge",
            Mode::Diverge => "Diverge",
        }
    }
    /// the readout's words
    fn words(self) -> &'static str {
        match self {
            Mode::Up => "going up",
            Mode::Down => "going down",
            Mode::UpDown => "up then down",
            Mode::DownUp => "down then up",
            Mode::Random => "in a random order",
            Mode::Played => "in the order you played them",
            Mode::Chord => "all together",
            Mode::Converge => "from the outside in",
            Mode::Diverge => "from the middle out",
        }
    }
}

/// how the octave range is walked
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OctMode {
    /// the run goes up an octave each pass
    #[default]
    Up,
    /// each pass goes an octave down
    Down,
    /// up the octaves and back down (0, 1, 2, 1, 0 ...)
    Bounce,
}

impl OctMode {
    pub const ALL: [OctMode; 3] = [OctMode::Up, OctMode::Down, OctMode::Bounce];
    pub fn name(self) -> &'static str {
        match self {
            OctMode::Up => "Up",
            OctMode::Down => "Down",
            OctMode::Bounce => "Bounce",
        }
    }
    /// the passes one cycle has for `oct` octaves
    fn passes(self, oct: i64) -> i64 {
        match self {
            OctMode::Bounce if oct > 1 => 2 * oct - 2,
            _ => oct.max(1),
        }
    }
    /// semitones for pass `q` (0 .. passes)
    fn shift(self, q: i64, oct: i64) -> i32 {
        let q = q.rem_euclid(self.passes(oct));
        (match self {
            OctMode::Up => q,
            OctMode::Down => -q,
            OctMode::Bounce => {
                if q < oct {
                    q
                } else {
                    2 * oct - 2 - q
                }
            }
        } * 12) as i32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Feel {
    #[default]
    Straight,
    Triplet,
    Dotted,
}

/// the rates, slowest first: (division, feel). The last three are the
/// chip "fast chord" rates: at 125 BPM 1/64T (= 1/96) is one 50 Hz frame,
/// the C64 / tracker `0xy` speed.
pub const RATES: [(u8, Feel); 15] = [
    (4, Feel::Dotted),
    (4, Feel::Straight),
    (4, Feel::Triplet),
    (8, Feel::Dotted),
    (8, Feel::Straight),
    (8, Feel::Triplet),
    (16, Feel::Dotted),
    (16, Feel::Straight),
    (16, Feel::Triplet),
    (32, Feel::Dotted),
    (32, Feel::Straight),
    (32, Feel::Triplet),
    (64, Feel::Straight),
    (64, Feel::Triplet),
    (128, Feel::Straight),
];

pub fn rate_name(div: u8, feel: Feel) -> String {
    if div == 64 && feel == Feel::Triplet {
        return "1/96".into();
    }
    match feel {
        Feel::Straight => format!("1/{div}"),
        Feel::Triplet => format!("1/{div}T"),
        Feel::Dotted => format!("1/{div}."),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    #[default]
    Play,
    Rest,
    /// holds the note before it on (nothing new plays)
    Tie,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Step {
    /// 0..1 (scaled by the Arp's velocity depth)
    pub vel: f32,
    /// 0.1..1, times the Arp's Gate
    pub gate: f32,
    pub accent: bool,
    pub kind: StepKind,
    /// hits in the step, 1..4
    pub ratchet: u8,
    /// semitones, -24..24 (melodic only); scale steps with Scale lock
    pub transpose: i8,
    /// chance the step plays, 0..1 (seeded: the same every play)
    pub chance: f32,
}

impl Default for Step {
    fn default() -> Self {
        Step {
            vel: 1.0,
            gate: 1.0,
            accent: false,
            kind: StepKind::Play,
            ratchet: 1,
            transpose: 0,
            chance: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Arp {
    pub mode: Mode,
    /// octave range 1..4
    pub octaves: u8,
    /// note division: 4, 8, 16 or 32
    pub div: u8,
    pub feel: Feel,
    /// note length, 0.05..1 of a step
    pub gate: f32,
    /// 0..1: every second step is late by up to half a step
    pub swing: f32,
    /// keep playing after the keys are up, until a new chord
    pub latch: bool,
    /// how much the step velocities count, 0..1
    pub vel_depth: f32,
    /// Random mode's seed
    pub seed: u32,
    /// pattern steps used, 1..32
    pub len: u8,
    /// the grid shows the steps (else the simple controls only)
    pub show_steps: bool,
    pub steps: Vec<Step>,
    /// how the octave range is walked
    pub oct_mode: OctMode,
    /// a new note restarts the pattern and the run (else the pattern runs
    /// free on the song grid)
    pub retrig: bool,
    /// notes and step transposes stay in the song key (transpose counts
    /// scale steps)
    pub scale_lock: bool,
    /// seeded velocity variation, 0..1
    pub humanise: f32,
    /// the Advanced fold-out is open
    pub show_more: bool,
}

impl Default for Arp {
    fn default() -> Self {
        Arp {
            mode: Mode::Up,
            octaves: 1,
            div: 16,
            feel: Feel::Straight,
            gate: 0.5,
            swing: 0.0,
            latch: false,
            vel_depth: 1.0,
            seed: 1,
            len: 16,
            show_steps: true,
            steps: vec![Step::default(); 16],
            oct_mode: OctMode::Up,
            retrig: false,
            scale_lock: false,
            humanise: 0.0,
            show_more: false,
        }
    }
}

/// modulation offsets (automation lanes and modulators on the Arp's
/// parameters): added to the Arp's own values. `key` is the song key's
/// pitch-class mask for Scale lock (0 = no key).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mods {
    /// rate table steps (+ = faster)
    pub rate: f32,
    pub gate: f32,
    pub swing: f32,
    pub octaves: f32,
    pub humanise: f32,
    pub key: u16,
}

/// the Arp parameters automation and modulators reach
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Param {
    Rate,
    Gate,
    Swing,
    Octaves,
    Humanise,
}

impl Param {
    pub const ALL: [Param; 5] = [
        Param::Rate,
        Param::Gate,
        Param::Swing,
        Param::Octaves,
        Param::Humanise,
    ];
    /// the target id's last part
    pub fn key(self) -> &'static str {
        match self {
            Param::Rate => "rate",
            Param::Gate => "gate",
            Param::Swing => "swing",
            Param::Octaves => "octaves",
            Param::Humanise => "humanise",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Param::Rate => "Arp rate",
            Param::Gate => "Arp gate",
            Param::Swing => "Arp swing",
            Param::Octaves => "Arp octaves",
            Param::Humanise => "Arp humanise",
        }
    }
    /// the parameter's value on the 0..1 automation scale
    pub fn norm(self, a: &Arp) -> f32 {
        match self {
            Param::Rate => a.rate_index() as f32 / (RATES.len() - 1) as f32,
            Param::Gate => (a.gate - 0.05) / 0.95,
            Param::Swing => a.swing,
            Param::Octaves => (a.octaves.clamp(1, 4) - 1) as f32 / 3.0,
            Param::Humanise => a.humanise,
        }
        .clamp(0.0, 1.0)
    }
    /// set the Arp's own value from normalised `v` (a knob, a learned CC)
    pub fn set(self, a: &mut Arp, v: f32) {
        let v = v.clamp(0.0, 1.0);
        match self {
            Param::Rate => a.set_rate_index((v * (RATES.len() - 1) as f32).round() as usize),
            Param::Gate => a.gate = 0.05 + v * 0.95,
            Param::Swing => a.swing = v,
            Param::Octaves => a.octaves = 1 + (v * 3.0).round() as u8,
            Param::Humanise => a.humanise = v,
        }
    }
    /// add the offset that takes `a` to normalised value `v`
    pub fn apply(self, a: &Arp, v: f32, m: &mut Mods) {
        let d = v.clamp(0.0, 1.0) - self.norm(a);
        match self {
            Param::Rate => m.rate += d * (RATES.len() - 1) as f32,
            Param::Gate => m.gate += d * 0.95,
            Param::Swing => m.swing += d,
            Param::Octaves => m.octaves += d * 3.0,
            Param::Humanise => m.humanise += d,
        }
    }
    /// a readout of normalised value `v`
    pub fn show(self, v: f32) -> String {
        let v = v.clamp(0.0, 1.0);
        match self {
            Param::Rate => {
                let (d, f) = RATES[(v * (RATES.len() - 1) as f32).round() as usize];
                rate_name(d, f)
            }
            Param::Gate => format!("{:.0}%", (0.05 + v * 0.95) * 100.0),
            Param::Swing | Param::Humanise => format!("{:.0}%", v * 100.0),
            Param::Octaves => format!("{}", 1 + (v * 3.0).round() as u8),
        }
    }
}

/// the modulation of instrument `i`'s Arp `a`: the song key mask `key`
/// and the automated / modulated Arp values `over` (instrument, param,
/// normalised value)
pub fn mods_for(a: &Arp, i: usize, key: u16, over: &[(usize, usize, f32)]) -> Mods {
    let mut m = Mods {
        key,
        ..Default::default()
    };
    for &(ii, k, v) in over {
        if ii == i {
            if let Some(p) = Param::ALL.get(k) {
                p.apply(a, v, &mut m);
            }
        }
    }
    m
}

/// Euclidean rhythm (Bjorklund / Toussaint 2005): `hits` onsets spread as
/// evenly as they go over `len` steps, turned right by `rot`
pub fn euclid(hits: usize, len: usize, rot: usize) -> Vec<bool> {
    let len = len.max(1);
    let hits = hits.min(len);
    // the bucket form: step i is an onset when the running sum wraps
    let mut v: Vec<bool> = (0..len)
        .map(|i| hits > 0 && (i * hits) % len < hits)
        .collect();
    v.rotate_right(rot % len);
    v
}

/// the note `deg` scale steps from `note` in the key with pitch-class
/// mask `mask` (`note` snaps into the key first)
fn scale_step(note: i32, deg: i32, mask: u16) -> i32 {
    let in_key = |n: i32| mask & (1 << ((n - 1).rem_euclid(12))) != 0;
    let mut n = note;
    for d in 0..12 {
        if in_key(note - d) {
            n = note - d;
            break;
        }
        if in_key(note + d) {
            n = note + d;
            break;
        }
    }
    let dir = deg.signum();
    for _ in 0..deg.abs() {
        n += dir;
        while !in_key(n) {
            n += dir;
        }
    }
    n
}

impl Arp {
    pub fn rate_index(&self) -> usize {
        RATES
            .iter()
            .position(|&(d, f)| d == self.div && f == self.feel)
            .unwrap_or(7)
    }
    pub fn set_rate_index(&mut self, i: usize) {
        let (d, f) = RATES[i.min(RATES.len() - 1)];
        self.div = d;
        self.feel = f;
    }
    pub fn step(&self, k: usize) -> Step {
        self.steps.get(k).copied().unwrap_or_default()
    }
    fn len(&self) -> i64 {
        self.len.clamp(1, MAX_STEPS as u8) as i64
    }
    /// the step data for pattern step `k`, made sane
    fn pat(&self, k: i64) -> Step {
        let mut s = self.step(k.rem_euclid(self.len()) as usize);
        s.ratchet = s.ratchet.clamp(1, 4);
        s.transpose = s.transpose.clamp(-24, 24);
        s
    }
    /// Euclid fill: `hits` plays spread over the first `len` steps (the
    /// rest rest), turned by `rot`; the pattern length becomes `len`
    pub fn euclid_fill(&mut self, hits: usize, len: usize, rot: usize) {
        let len = len.clamp(1, MAX_STEPS);
        if self.steps.len() < len {
            self.steps.resize(len, Step::default());
        }
        for (s, on) in self.steps.iter_mut().zip(euclid(hits, len, rot)) {
            s.kind = if on { StepKind::Play } else { StepKind::Rest };
        }
        self.len = len as u8;
    }
    /// the effective values with modulation: (step ticks, gate, swing,
    /// octaves)
    pub fn eff(&self, m: &Mods) -> (f64, f32, f32, u8) {
        let ri = (self.rate_index() as f32 + m.rate)
            .round()
            .clamp(0.0, (RATES.len() - 1) as f32) as usize;
        let (div, feel) = RATES[ri];
        (
            step_ticks(div, feel),
            (self.gate + m.gate).clamp(0.05, 1.0),
            (self.swing + m.swing).clamp(0.0, 1.0),
            (self.octaves as f32 + m.octaves).round().clamp(1.0, 4.0) as u8,
        )
    }
    /// a plain-words readout
    pub fn describe(&self, kit: bool) -> String {
        let rate = rate_name(self.div, self.feel);
        let n = self.len() as usize;
        let mut rests = 0;
        let mut ties = 0;
        let mut rolls = 0;
        let mut accents = 0;
        for k in 0..n {
            let s = self.pat(k as i64);
            match s.kind {
                StepKind::Rest => rests += 1,
                StepKind::Tie => ties += 1,
                StepKind::Play => {}
            }
            rolls += (s.ratchet > 1 && s.kind == StepKind::Play) as usize;
            accents += s.accent as usize;
        }
        let mut s = if kit {
            format!("Repeats the held pads every {rate}")
        } else {
            let oct = if self.octaves > 1 {
                format!(" over {} octaves", self.octaves)
            } else {
                String::new()
            };
            format!(
                "Plays the held notes {}{oct}, one every {rate}",
                self.mode.words()
            )
        };
        s += &format!(", {}% long", (self.gate * 100.0).round());
        if self.swing > 0.005 {
            s += &format!(", swing {}%", (self.swing * 100.0).round());
        }
        if self.latch {
            s += ", keeps going after you let go";
        }
        if self.scale_lock {
            s += ", in the song key";
        }
        if self.retrig {
            s += ", restarts on each new note";
        }
        let mut extra = vec![];
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        if rests > 0 {
            extra.push(plural(rests, "rest", "rests"));
        }
        if ties > 0 {
            extra.push(plural(ties, "tie", "ties"));
        }
        if accents > 0 {
            extra.push(plural(accents, "accent", "accents"));
        }
        if rolls > 0 {
            extra.push(plural(rolls, "ratchet", "ratchets"));
        }
        s += &format!(" · {n} steps");
        if !extra.is_empty() {
            s += &format!(": {}", extra.join(", "));
        }
        s
    }
}

/// one step's length in ticks
pub fn step_ticks(div: u8, feel: Feel) -> f64 {
    let base = PPQ * 4.0 / div.max(1) as f64;
    match feel {
        Feel::Straight => base,
        Feel::Triplet => base * 2.0 / 3.0,
        Feel::Dotted => base * 1.5,
    }
}

/// step `k`'s start and length on the grid (`len` ticks, swing 0..1: odd
/// steps start late by up to half a step)
fn grid(k: i64, len: f64, swing: f32) -> (f64, f64) {
    let d = swing as f64 * len * 0.5;
    let base = k as f64 * len;
    if k.rem_euclid(2) == 1 {
        (base + d, len - d)
    } else {
        (base, len + d)
    }
}

fn hash(seed: u32, k: i64) -> u32 {
    let mut x = (k as u64) ^ ((seed as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (x ^ (x >> 31)) as u32
}

/// a held note
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held {
    /// tracker note (1 = C-0)
    pub key: u8,
    pub vel: f32,
    /// instrument slot as the note had it (1-based, 0 = the track's)
    pub instr: u8,
    pub tune: f32,
    pub start: f64,
    /// when it lets go (f64::INFINITY: a live key, until its Off)
    pub end: f64,
    /// a live key's id (the On's `key`)
    pub id: u8,
    /// let go, kept by Latch
    pub up: bool,
    /// let go while the sustain pedal is down (kept until it comes up)
    pub sus: bool,
    pub ord: u32,
}

/// one note the stepper plays
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hit {
    pub start: f64,
    pub end: f64,
    pub key: u8,
    pub vel: f32,
    pub instr: u8,
    pub tune: f32,
    /// the hit's number (drum pads' Round Robin)
    pub n: u32,
}

/// the hits of one call (fixed size: no allocation)
#[derive(Clone, Copy, Debug)]
pub struct Hits {
    pub n: usize,
    pub h: [Hit; MAX_OUT],
}

impl Default for Hits {
    fn default() -> Self {
        Hits {
            n: 0,
            h: [Hit::default(); MAX_OUT],
        }
    }
}

impl Hits {
    fn push(&mut self, h: Hit) {
        if self.n < MAX_OUT {
            self.h[self.n] = h;
            self.n += 1;
        }
    }
    pub fn as_slice(&self) -> &[Hit] {
        &self.h[..self.n]
    }
}

/// a track's stepper: its held notes and the last time it ran
#[derive(Clone, Copy, Debug)]
pub struct Stepper {
    held: [Held; MAX_HELD],
    n: usize,
    ord: u32,
    /// the position (ticks) of the last call; NaN = not running
    pub last: f64,
    /// the instrument (0-based) whose Arp plays
    pub idx: usize,
    /// it runs on the launch clock (a launcher clip's notes, tracker cells)
    pub launch: bool,
    /// the sustain pedal is down (live keys)
    pub pedal: bool,
}

impl Default for Stepper {
    fn default() -> Self {
        Stepper {
            held: [Held::default(); MAX_HELD],
            n: 0,
            ord: 0,
            last: f64::NAN,
            idx: 0,
            launch: false,
            pedal: false,
        }
    }
}

impl Stepper {
    pub fn clear(&mut self) {
        self.n = 0;
        self.last = f64::NAN;
        self.pedal = false;
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    #[allow(dead_code)]
    pub fn held(&self) -> &[Held] {
        &self.held[..self.n]
    }
    /// run from `t` on (after a seek): steps after `t` play
    pub fn reset(&mut self, t: f64) {
        self.last = t - 1e-6;
    }

    /// the clock jumped (live keys): the held notes start again at `t`
    pub fn rebase(&mut self, t: f64) {
        for h in &mut self.held[..self.n] {
            h.start = t;
        }
        self.last = t - 1e-6;
    }

    /// notes that ended by `t` let go (Latch keeps them, marked up)
    pub fn expire(&mut self, t: f64, latch: bool) {
        let mut k = 0;
        while k < self.n {
            let h = &mut self.held[k];
            if !h.up && h.end <= t {
                if latch {
                    h.up = true;
                } else {
                    self.remove(k);
                    continue;
                }
            }
            k += 1;
        }
    }

    fn remove(&mut self, k: usize) {
        self.held.copy_within(k + 1..self.n, k);
        self.n -= 1;
    }

    /// a note goes down (Latch: a new chord after every key was up starts
    /// afresh)
    pub fn add(&mut self, latch: bool, h: Held) {
        self.expire(h.start, latch);
        if latch && self.n > 0 && self.held[..self.n].iter().all(|x| x.up) {
            self.n = 0;
        }
        if self.n == 0 && self.last.is_nan() {
            // a step on the note's start plays it
            self.last = h.start - 1e-6;
        }
        if self.n == MAX_HELD {
            // full: the oldest goes
            self.remove(0);
        }
        self.ord = self.ord.wrapping_add(1);
        self.held[self.n] = Held {
            up: false,
            ord: self.ord,
            ..h
        };
        self.n += 1;
    }

    /// live key `id` is up (Latch keeps it; the sustain pedal holds it
    /// until it comes up)
    pub fn release(&mut self, id: u8, latch: bool) -> bool {
        let mut hit = false;
        let mut k = 0;
        while k < self.n {
            if self.held[k].id == id
                && !self.held[k].up
                && !self.held[k].sus
                && self.held[k].end.is_infinite()
            {
                hit = true;
                if self.pedal {
                    self.held[k].sus = true;
                } else if latch {
                    self.held[k].up = true;
                } else {
                    self.remove(k);
                    continue;
                }
            }
            k += 1;
        }
        hit
    }

    /// the sustain pedal (CC 64) goes down / up: up lets go of the notes
    /// it held (Latch keeps them)
    pub fn sustain(&mut self, down: bool, latch: bool) {
        self.pedal = down;
        if down {
            return;
        }
        let mut k = 0;
        while k < self.n {
            if self.held[k].sus {
                self.held[k].sus = false;
                if latch {
                    self.held[k].up = true;
                } else {
                    self.remove(k);
                    continue;
                }
            }
            k += 1;
        }
    }

    /// every held note with no end (tracker cells) lets go at `t`
    pub fn release_open(&mut self, t: f64) {
        for h in &mut self.held[..self.n] {
            if h.end.is_infinite() && !h.up {
                h.end = t.max(h.start);
            }
        }
    }

    /// the step the held notes start on: the first one at or after the
    /// earliest held note (Retrigger: the newest)
    fn anchor(&self, len: f64, swing: f32, newest: bool) -> i64 {
        let it = self.held[..self.n].iter().map(|h| h.start);
        let first = if newest {
            it.fold(f64::NEG_INFINITY, f64::max)
        } else {
            it.fold(f64::INFINITY, f64::min)
        };
        let mut k = (first / len).floor() as i64;
        while grid(k, len, swing).0 < first - 1e-6 {
            k += 1;
        }
        k
    }

    /// The hits between the last call and `t` (ticks): ended notes let go
    /// first, then each step in (last, t] plays.
    pub fn advance(&mut self, a: &Arp, m: &Mods, kit: bool, t: f64, out: &mut Hits) {
        out.n = 0;
        self.expire(t, a.latch);
        if self.n == 0 {
            self.last = f64::NAN;
            return;
        }
        if self.last.is_nan() {
            self.last = t - 1e-6;
        }
        let last = self.last;
        self.last = t;
        if t <= last {
            return;
        }
        let (len, gate, swing, oct) = a.eff(m);
        let anchor = self.anchor(len, swing, a.retrig);
        // Retrigger: the pattern counts from the anchor, else the song grid
        let base = if a.retrig { anchor } else { 0 };
        let hum = (a.humanise + m.humanise).clamp(0.0, 1.0);
        // a long gap (a jump the caller did not reset) plays only its end
        let from = ((last / len).floor() as i64 - 1).max((t / len).floor() as i64 - 64);
        let to = (t / len).floor() as i64;
        for k in from.max(anchor)..=to {
            let s = a.pat(k - base);
            if s.kind != StepKind::Play {
                continue;
            }
            // step probability, seeded on the step's song position
            if s.chance < 1.0
                && (hash(a.seed ^ 0xC4A2_5E11, k) % 10_000) as f32 >= s.chance.max(0.0) * 10_000.0
            {
                continue;
            }
            let (st, span) = grid(k, len, swing);
            let r = s.ratchet as i64;
            let sub = span / r as f64;
            let g = (gate * s.gate.clamp(0.1, 1.0)).clamp(0.02, 1.0) as f64;
            for j in 0..r {
                let tm = st + j as f64 * sub;
                if !(tm > last && tm <= t) {
                    continue;
                }
                let mut end = tm + (sub * g).max(1.0);
                if j == r - 1 {
                    // ties hold the last hit on through them
                    let mut e = k;
                    for _ in 1..a.len() {
                        if a.pat(e + 1 - base).kind != StepKind::Tie {
                            break;
                        }
                        e += 1;
                    }
                    if e > k {
                        let (es, el) = grid(e, len, swing);
                        end = es + (el * g).max(1.0);
                    }
                }
                let n = (k.max(0) as u32).wrapping_mul(4).wrapping_add(j as u32);
                let mut s = s;
                if hum > 0.0 {
                    let r = (hash(a.seed ^ 0x4855_4D41, k * 4 + j) % 2001) as f32 / 1000.0 - 1.0;
                    s.vel = (s.vel * (1.0 + r * hum * 0.4)).clamp(0.0, 1.0);
                }
                let key = if a.scale_lock { m.key } else { 0 };
                self.notes(
                    a,
                    kit,
                    oct,
                    k - base,
                    anchor - base,
                    &s,
                    tm,
                    end,
                    n,
                    key,
                    out,
                );
            }
        }
    }

    /// the notes step `k` plays (its `ni`-th note of the sequence)
    #[allow(clippy::too_many_arguments)]
    fn notes(
        &self,
        a: &Arp,
        kit: bool,
        oct: u8,
        k: i64,
        anchor: i64,
        s: &Step,
        start: f64,
        end: f64,
        n: u32,
        key: u16,
        out: &mut Hits,
    ) {
        // the held notes by pitch (one of each), or as played
        let mut ix = [0usize; MAX_HELD];
        let mut m = 0;
        let by_order = a.mode == Mode::Played;
        let mut order: [usize; MAX_HELD] = [0; MAX_HELD];
        for (i, o) in order.iter_mut().enumerate().take(self.n) {
            *o = i;
        }
        let held = &self.held[..self.n];
        order[..self.n].sort_unstable_by(|&x, &y| {
            if by_order {
                held[x].ord.cmp(&held[y].ord)
            } else {
                held[x]
                    .key
                    .cmp(&held[y].key)
                    .then(held[x].ord.cmp(&held[y].ord))
            }
        });
        for &i in &order[..self.n] {
            if ix[..m].iter().any(|&j| held[j].key == held[i].key) {
                continue;
            }
            ix[m] = i;
            m += 1;
        }
        if m == 0 {
            return;
        }
        let dv = a.vel_depth.clamp(0.0, 1.0);
        let push = |out: &mut Hits, h: &Held, shift: i32| {
            let mut v = h.vel * (1.0 - dv + dv * s.vel.clamp(0.0, 1.0));
            if s.accent {
                v = (v * 1.3).min(1.0);
            }
            let key = if kit {
                h.key
            } else if key != 0 {
                (scale_step(h.key as i32, s.transpose as i32, key) + shift).clamp(1, 127) as u8
            } else {
                (h.key as i32 + shift + s.transpose as i32).clamp(1, 127) as u8
            };
            out.push(Hit {
                start,
                end,
                key,
                vel: v.clamp(0.0, 1.0),
                instr: h.instr,
                tune: h.tune,
                n,
            });
        };
        let oct = if kit { 1 } else { oct.max(1) as i64 };
        let om = a.oct_mode;
        let ni = self.count_plays(a, anchor, k);
        if kit || a.mode == Mode::Chord {
            let shift = if kit { 0 } else { om.shift(ni, oct) };
            for &i in &ix[..m] {
                push(out, &held[i], shift);
            }
            return;
        }
        let total = m as i64 * om.passes(oct);
        // position `j` of the run: (note, octave shift)
        let up = |j: i64| -> (usize, i32) {
            let j = j.rem_euclid(total);
            (ix[(j % m as i64) as usize], om.shift(j / m as i64, oct))
        };
        // Converge: 0, n-1, 1, n-2 ...; Diverge: that backwards
        let conv = |j: i64| -> i64 {
            let j = j.rem_euclid(total);
            if j % 2 == 0 {
                j / 2
            } else {
                total - 1 - j / 2
            }
        };
        let bounce = |j: i64| -> i64 {
            if total < 2 {
                return 0;
            }
            let p = 2 * total - 2;
            let j = j.rem_euclid(p);
            if j < total {
                j
            } else {
                p - j
            }
        };
        let (i, sh) = match a.mode {
            Mode::Up | Mode::Played => up(ni),
            Mode::Down => up(total - 1 - ni.rem_euclid(total)),
            Mode::UpDown => up(bounce(ni)),
            Mode::DownUp => up(total - 1 - bounce(ni)),
            Mode::Random => up((hash(a.seed, k) % total as u32) as i64),
            Mode::Converge => up(conv(ni)),
            Mode::Diverge => up(conv(total - 1 - ni.rem_euclid(total))),
            Mode::Chord => unreachable!(),
        };
        push(out, &held[i], sh);
    }

    /// Play steps in [from, k): the sequence moves on only on played steps
    fn count_plays(&self, a: &Arp, from: i64, k: i64) -> i64 {
        let len = a.len();
        let per: i64 = (0..len)
            .filter(|&i| a.pat(i).kind == StepKind::Play)
            .count() as i64;
        let c = |x: i64| -> i64 {
            let q = x.div_euclid(len);
            let r = x.rem_euclid(len);
            q * per + (0..r).filter(|&i| a.pat(i).kind == StepKind::Play).count() as i64
        };
        c(k) - c(from)
    }
}

/// Bake: the notes the Arp plays for `notes` (start, end, key, vel), held
/// from song tick `origin` (the clip's start), until `until` ticks after it.
/// Note clips get these instead of their held notes.
pub fn bake(
    a: &Arp,
    kit: bool,
    origin: f64,
    notes: &[(f64, f64, u8, f32)],
    until: f64,
) -> Vec<(f64, f64, u8, f32)> {
    let mut st = Stepper::default();
    let mut sorted: Vec<_> = notes.to_vec();
    sorted.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = vec![];
    let mut hits = Hits::default();
    let m = Mods::default();
    let (len, ..) = a.eff(&m);
    // walk in small steps: every hit lands on its own (sub-)step time
    let dt = len / 64.0;
    let mut t = origin - 1e-6;
    st.reset(t);
    let mut i = 0;
    let end = origin + until;
    while t < end {
        let nt = (t + dt).min(end);
        while i < sorted.len() && origin + sorted[i].0 <= nt {
            let (s, e, key, vel) = sorted[i];
            st.add(
                a.latch,
                Held {
                    key,
                    vel,
                    start: origin + s,
                    end: origin + e,
                    ..Default::default()
                },
            );
            i += 1;
        }
        st.advance(a, &m, kit, nt, &mut hits);
        for h in hits.as_slice() {
            if h.start < end {
                out.push((h.start - origin, h.end.min(end) - origin, h.key, h.vel));
            }
        }
        t = nt;
    }
    out
}

/// the starting patterns: (name, settings)
/// the named preset (melodic first, then the drum ones)
#[cfg(test)]
pub fn preset(name: &str) -> Option<Arp> {
    presets(false)
        .into_iter()
        .chain(presets(true))
        .find(|p| p.0 == name)
        .map(|p| p.1)
}

pub fn presets(kit: bool) -> Vec<(&'static str, Arp)> {
    let base = Arp::default();
    let with = |f: &dyn Fn(&mut Arp)| {
        let mut a = base.clone();
        f(&mut a);
        a
    };
    if kit {
        return vec![
            ("Repeat 1/16", with(&|a| a.gate = 0.4)),
            (
                "Roll 1/32",
                with(&|a| {
                    a.div = 32;
                    a.gate = 0.35;
                    for (k, s) in a.steps.iter_mut().enumerate() {
                        s.vel = 0.55 + 0.45 * (k as f32 / 15.0);
                    }
                }),
            ),
            (
                "Ratchet fill",
                with(&|a| {
                    for k in [3, 7, 11] {
                        a.steps[k].ratchet = 2;
                    }
                    a.steps[14].ratchet = 3;
                    a.steps[15].ratchet = 4;
                    for k in [0, 4, 8, 12] {
                        a.steps[k].accent = true;
                    }
                }),
            ),
            (
                "Triplet stutter",
                with(&|a| {
                    a.feel = Feel::Triplet;
                    a.gate = 0.3;
                    for k in [2, 5, 8, 11, 14] {
                        a.steps[k].kind = StepKind::Rest;
                    }
                }),
            ),
        ];
    }
    vec![
        (
            "Trance Gate 16ths",
            with(&|a| {
                a.mode = Mode::Up;
                a.octaves = 2;
                a.gate = 0.55;
                a.retrig = true;
                for (k, s) in a.steps.iter_mut().enumerate() {
                    s.accent = k % 4 == 0;
                    s.vel = if k % 4 == 0 { 1.0 } else { 0.75 };
                    s.gate = if k % 4 == 3 { 0.4 } else { 1.0 };
                }
            }),
        ),
        (
            "Chip Fast Chord",
            with(&|a| {
                // the C64 / tracker 0xy chord: one note per 50 Hz frame
                a.mode = Mode::Up;
                a.div = 64;
                a.feel = Feel::Triplet;
                a.gate = 1.0;
                a.vel_depth = 0.0;
                a.retrig = true;
            }),
        ),
        (
            "Synthwave Octaves",
            with(&|a| {
                a.mode = Mode::Up;
                a.div = 8;
                a.octaves = 2;
                a.oct_mode = OctMode::Bounce;
                a.gate = 0.6;
                a.retrig = true;
                for (k, s) in a.steps.iter_mut().enumerate() {
                    s.transpose = if k % 2 == 1 { 12 } else { 0 };
                    s.accent = k % 8 == 0;
                }
            }),
        ),
        (
            "Euclid 5/8 Pluck",
            with(&|a| {
                a.mode = Mode::UpDown;
                a.octaves = 2;
                a.gate = 0.25;
                a.euclid_fill(5, 8, 0);
                a.humanise = 0.3;
            }),
        ),
        (
            "Converge Glass",
            with(&|a| {
                a.mode = Mode::Converge;
                a.octaves = 2;
                a.feel = Feel::Triplet;
                a.gate = 0.45;
                a.humanise = 0.2;
            }),
        ),
        (
            "In-key Runs",
            with(&|a| {
                // step transposes count scale steps of the song key
                a.mode = Mode::Played;
                a.scale_lock = true;
                a.gate = 0.5;
                for (k, s) in a.steps.iter_mut().enumerate() {
                    s.transpose = [0, 1, 2, 4][k % 4];
                }
            }),
        ),
        (
            "Probability Sparkle",
            with(&|a| {
                a.mode = Mode::Random;
                a.octaves = 3;
                a.gate = 0.3;
                a.humanise = 0.5;
                for (k, s) in a.steps.iter_mut().enumerate() {
                    s.chance = if k % 4 == 0 { 1.0 } else { 0.55 };
                }
            }),
        ),
        ("Straight 16ths", base.clone()),
        (
            "Up-down 8ths",
            with(&|a| {
                a.mode = Mode::UpDown;
                a.div = 8;
                a.octaves = 2;
                a.gate = 0.7;
            }),
        ),
        (
            "Gated trance",
            with(&|a| {
                a.octaves = 2;
                a.gate = 0.8;
                for (k, s) in a.steps.iter_mut().enumerate() {
                    s.gate = if k % 2 == 0 { 1.0 } else { 0.4 };
                    s.accent = k % 4 == 0;
                    s.vel = if k % 4 == 0 { 1.0 } else { 0.7 };
                }
            }),
        ),
        (
            "Octave bounce",
            with(&|a| {
                a.mode = Mode::Played;
                for (k, s) in a.steps.iter_mut().enumerate() {
                    s.transpose = if k % 2 == 1 { 12 } else { 0 };
                }
            }),
        ),
        (
            "Swing pluck",
            with(&|a| {
                a.div = 16;
                a.swing = 0.4;
                a.gate = 0.3;
                a.steps[3].kind = StepKind::Rest;
                a.steps[6].kind = StepKind::Tie;
                a.steps[11].kind = StepKind::Rest;
                a.steps[14].kind = StepKind::Tie;
            }),
        ),
        (
            "Ratchet run",
            with(&|a| {
                a.mode = Mode::Up;
                a.octaves = 3;
                for k in [3, 7, 11, 15] {
                    a.steps[k].ratchet = if k == 15 { 4 } else { 2 };
                }
            }),
        ),
        (
            "Random sparkle",
            with(&|a| {
                a.mode = Mode::Random;
                a.octaves = 2;
                a.gate = 0.35;
                for (k, s) in a.steps.iter_mut().enumerate() {
                    if hash(7, k as i64).is_multiple_of(5) {
                        s.kind = StepKind::Rest;
                    }
                    s.vel = 0.5 + (hash(9, k as i64) % 50) as f32 / 100.0;
                }
            }),
        ),
    ]
}

#[cfg(test)]
#[path = "arp/tests.rs"]
mod tests;
