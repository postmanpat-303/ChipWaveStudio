//! Round Robin for creative timelines (docs/ROUND_ROBIN.md): step
//! conditions on notes, and the shared "which one this time" orders used by
//! clip Variations and drum pad Round Robin.
//!
//! Everything here is a pure function of (seed, pass / hit index): nothing
//! counts at run time, so playback, seeking and export always agree. Each
//! feature draws from its own salted [`ClipRng`] stream, so none of them
//! moves the existing per-note chance draws.
use super::*;

/// stream salts: each feature has its own random stream
pub const SALT_VAR: u32 = 0x5641_5249; // "VARI"
pub const SALT_HIT: u32 = 0x4849_5452; // "HITR"
pub const SALT_DRIFT: u32 = 0x4452_4654; // "DRFT"
pub const SALT_FOLLOW: u32 = 0x464F_4C57; // "FOLW"

/// When a note plays, besides its chance ("Play this note…").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cond {
    /// every pass (the default; not written to files)
    #[default]
    Always,
    /// pass `a` of every `b` (1-based: Every{1,4} = passes 1, 5, 9…)
    Every { a: u8, b: u8 },
    /// only the first pass through the clip
    First,
    /// every pass but the first
    NotFirst,
    /// only in a fill (see [`is_fill`])
    Fill,
    /// never in a fill
    NotFill,
}

impl Cond {
    /// the note plays on loop pass `pass` (0-based); `fill` = it is a fill
    pub fn plays(self, pass: u32, fill: bool) -> bool {
        match self {
            Cond::Always => true,
            Cond::Every { a, b } => {
                let b = b.max(1) as u32;
                let a = (a.max(1) as u32 - 1).min(b - 1);
                pass % b == a
            }
            Cond::First => pass == 0,
            Cond::NotFirst => pass > 0,
            Cond::Fill => fill,
            Cond::NotFill => !fill,
        }
    }
    /// short chip text ("2:4", "1st", "Fill"…); empty for Always
    pub fn chip(self) -> String {
        match self {
            Cond::Always => String::new(),
            Cond::Every { a, b } => format!("{a}:{b}"),
            Cond::First => "1st".into(),
            Cond::NotFirst => "not 1st".into(),
            Cond::Fill => "Fill".into(),
            Cond::NotFill => "no Fill".into(),
        }
    }
    /// a sentence for tooltips and the status line
    pub fn describe(self) -> String {
        match self {
            Cond::Always => "plays every pass".into(),
            Cond::Every { a, b } => format!("plays on pass {a} of every {b}"),
            Cond::First => "plays on the first pass only".into(),
            Cond::NotFirst => "plays on every pass except the first".into(),
            Cond::Fill => "plays only in a fill".into(),
            Cond::NotFill => "plays except in a fill".into(),
        }
    }
}

/// Is loop pass `k` of clip `c` a fill? It is when the pass is the clip's
/// last (of at least two), or a song marker falls inside it (the next
/// section starts as it ends), or `live` (the Fill key is held while
/// playing; never in renders). `base` = the pass's start in clip time,
/// `l` = the loop length.
pub fn is_fill(
    tl: &Timeline,
    c: &Clip,
    k: Tick,
    passes: Tick,
    base: Tick,
    l: Tick,
    live: bool,
) -> bool {
    if live {
        return true;
    }
    if passes >= 2 && k == passes - 1 {
        return true;
    }
    let (a, b) = (c.start + base, c.start + base + l);
    tl.markers
        .iter()
        .any(|m| m.track.is_none() && m.tick > a && m.tick <= b && m.tick < c.end())
}

/// Which of several takes plays each time (clip Variations, pad Round
/// Robin).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    /// A B C D A B C D…
    InOrder,
    /// any of them, repeats allowed
    Random,
    /// shuffled, never the same twice in a row (the default)
    #[default]
    RandomNoRepeat,
    /// A B C D C B A B…
    PingPong,
    /// the first one, and take `which` every `n`th time
    EveryN { n: u8, which: u8 },
}

impl Order {
    pub const ALL: [Order; 5] = [
        Order::InOrder,
        Order::Random,
        Order::RandomNoRepeat,
        Order::PingPong,
        Order::EveryN { n: 4, which: 1 },
    ];
    pub fn label(self) -> &'static str {
        match self {
            Order::InOrder => "In order",
            Order::Random => "Random",
            Order::RandomNoRepeat => "Random no repeat",
            Order::PingPong => "Ping-pong",
            Order::EveryN { .. } => "Every N",
        }
    }
    pub fn tip(self) -> &'static str {
        match self {
            Order::InOrder => "one after the other: A, B, C, D, then A again",
            Order::Random => "a random pick each time (the same one can come twice)",
            Order::RandomNoRepeat => "shuffled, and never the same one twice in a row",
            Order::PingPong => "forwards then backwards: A, B, C, D, C, B, A…",
            Order::EveryN { .. } => "the first one, with another one every Nth time",
        }
    }
    pub fn same_kind(self, o: Order) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&o)
    }
}

fn rand_u32(seed: u32, salt: u32, k: u32) -> u32 {
    ClipRng::new(seed ^ salt, k).next_u32()
}

/// the shuffled bag `b` of `n` items (Fisher-Yates from its own stream)
fn bag(seed: u32, salt: u32, b: u32, n: usize) -> [u8; 8] {
    let mut p = [0u8, 1, 2, 3, 4, 5, 6, 7];
    let mut r = ClipRng::new(seed ^ salt ^ 0xBA6B_A600, b);
    for i in (1..n).rev() {
        let j = (r.next_u32() as usize) % (i + 1);
        p.swap(i, j);
    }
    p
}

/// Which of `n` items (0-based) plays at index `k` (a loop pass or a hit).
/// Pure: the same (order, n, seed, salt, k) always gives the same answer,
/// with no state carried from earlier indices.
pub fn pick(order: Order, n: usize, seed: u32, salt: u32, k: u32) -> usize {
    let n = n.clamp(1, 8);
    if n == 1 {
        return 0;
    }
    match order {
        Order::InOrder => k as usize % n,
        Order::Random => rand_u32(seed, salt, k) as usize % n,
        Order::RandomNoRepeat => {
            if n == 2 {
                // two items never repeating simply alternate
                return (bag(seed, salt, 0, 2)[0] as usize + k as usize) % 2;
            }
            let b = k / n as u32;
            let mut p = bag(seed, salt, b, n);
            if b > 0 {
                // a bag never starts with the one the last bag ended on
                // (with n >= 3, the fix below never moves a bag's last item)
                let prev = bag(seed, salt, b - 1, n)[n - 1];
                if p[0] == prev {
                    p.swap(0, 1);
                }
            }
            p[k as usize % n] as usize
        }
        Order::PingPong => {
            let per = 2 * n - 2;
            let p = k as usize % per;
            if p < n {
                p
            } else {
                per - p
            }
        }
        Order::EveryN { n: every, which } => {
            let every = every.max(1) as u32;
            if (k + 1) % every == 0 {
                (which as usize).min(n - 1)
            } else {
                0
            }
        }
    }
}

/// a 0..1 draw for index `k`, item `i` of a feature (drift amounts)
pub fn unit(seed: u32, salt: u32, k: u32, i: u32) -> f32 {
    let mut r = ClipRng::new(seed ^ salt ^ i.wrapping_mul(0x9E37_79B9), k);
    r.next_f32()
}
