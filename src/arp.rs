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
}

impl Mode {
    pub const ALL: [Mode; 7] = [
        Mode::Up,
        Mode::Down,
        Mode::UpDown,
        Mode::DownUp,
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
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Feel {
    #[default]
    Straight,
    Triplet,
    Dotted,
}

/// the rates, slowest first: (division, feel)
pub const RATES: [(u8, Feel); 12] = [
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
];

pub fn rate_name(div: u8, feel: Feel) -> String {
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
    /// semitones, -24..24 (melodic only)
    pub transpose: i8,
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
        }
    }
}

/// modulation offsets (step 5): added to the Arp's own values
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mods {
    /// rate table steps (+ = faster)
    pub rate: f32,
    pub gate: f32,
    pub swing: f32,
    pub octaves: f32,
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
    /// it runs on the launch clock (a launcher clip's notes)
    pub launch: bool,
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
        }
    }
}

impl Stepper {
    pub fn clear(&mut self) {
        self.n = 0;
        self.last = f64::NAN;
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

    /// live key `id` is up (Latch keeps it)
    pub fn release(&mut self, id: u8, latch: bool) -> bool {
        let mut hit = false;
        let mut k = 0;
        while k < self.n {
            if self.held[k].id == id && !self.held[k].up && self.held[k].end.is_infinite() {
                hit = true;
                if latch {
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

    /// the step the held notes start on: the first one at or after the
    /// earliest held note
    fn anchor(&self, len: f64, swing: f32) -> i64 {
        let first = self.held[..self.n]
            .iter()
            .map(|h| h.start)
            .fold(f64::INFINITY, f64::min);
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
        let anchor = self.anchor(len, swing);
        // a long gap (a jump the caller did not reset) plays only its end
        let from = ((last / len).floor() as i64 - 1).max((t / len).floor() as i64 - 64);
        let to = (t / len).floor() as i64;
        for k in from.max(anchor)..=to {
            let s = a.pat(k);
            if s.kind != StepKind::Play {
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
                        if a.pat(e + 1).kind != StepKind::Tie {
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
                self.notes(a, kit, oct, k, anchor, &s, tm, end, n, out);
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
        let ni = self.count_plays(a, anchor, k);
        if kit || a.mode == Mode::Chord {
            let shift = (ni.rem_euclid(oct) * 12) as i32;
            for &i in &ix[..m] {
                push(out, &held[i], shift);
            }
            return;
        }
        let total = m as i64 * oct;
        // position `j` of the rising sequence: (note, octave)
        let up = |j: i64| -> (usize, i32) {
            let j = j.rem_euclid(total);
            (ix[(j % m as i64) as usize], (j / m as i64 * 12) as i32)
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
                    if hash(7, k as i64) % 5 == 0 {
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
