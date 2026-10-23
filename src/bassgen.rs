//! Bass generator: a bass line over a chord progression (docs/CHORDS.md
//! "Bass generator", the Generators panel's Bass tab). Rule based and
//! seeded, so the same settings and seed always give the same notes:
//!
//! 1. **Groove**: a one-bar pattern of steps on a 16th grid (two bars for
//!    Acid) picked by the style and Density, repeated through the line, so
//!    it grooves like a played part. Euclid spreads pulses over steps
//!    (Bjorklund's algorithm, as described by Toussaint 2005).
//! 2. **Chord start**: every chord starts with a note on its first beat,
//!    on the chord's root (or its slash bass).
//! 3. **Pitch**: each step is a role (root, fifth, octave, a scale step)
//!    read against the chord sounding there; Walking moves through chord
//!    and scale tones towards the next chord's root.
//! 4. **Approach**: the last note before a chord change becomes, with the
//!    chance Approach, a note leading into the next root: a half step
//!    below or above, a scale step, or its fifth.
//! 5. Every note is folded by octaves into Octave / Range; the line is
//!    monophonic (a note ends before the next starts). Acid adds accents
//!    (louder notes) and ties (a note held into the next).
//!
//! Ideas from acid-banger by David Whiting (CC BY 4.0: position-weighted
//! 16-step odds, root-heavy note sets, accents), the generator controls of
//! aciduino (midilab, MIT) and JC-303 (GPL-3.0), acid-test by Zack Scholl
//! (MIT), the walking-bass rules of WalkingBass (philxan, MIT; root on the
//! first beat, chord tones, a chromatic approach into the next root), the
//! Euclidean rhythm paper (Toussaint) and the pattern libraries
//! total-serialism and isobar. No code was copied (credits.rs, Research).
use crate::chordgen::{ChordMark, Rng};
use crate::chords::{pc_of, Key};
use crate::melodygen::{euclid, Locks, MNote, BAR};
use crate::timeline::{Tick, PPQ};

/// one step of the 16th grid
const STEP: Tick = PPQ / 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BassStyle {
    /// roots on the beat (halves, quarters or eighths by Density)
    #[default]
    Root,
    /// eighths: the root on the beat, its octave on the off-beat
    OctavePump,
    /// quarters through chord and scale tones into the next root
    Walking,
    /// 16ths with accents, ties and octave jumps
    Acid,
    /// 16th syncopation around the root and the fifth
    Syncopated,
    /// the Euclid settings (pulses / steps / rotate)
    Euclid,
}

impl BassStyle {
    pub const ALL: [BassStyle; 6] = [
        BassStyle::Root,
        BassStyle::OctavePump,
        BassStyle::Walking,
        BassStyle::Acid,
        BassStyle::Syncopated,
        BassStyle::Euclid,
    ];
    pub fn name(self) -> &'static str {
        match self {
            BassStyle::Root => "Root",
            BassStyle::OctavePump => "Octave pump",
            BassStyle::Walking => "Walking",
            BassStyle::Acid => "Acid",
            BassStyle::Syncopated => "Syncopated",
            BassStyle::Euclid => "Euclid",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            BassStyle::Root => {
                "the chord's root on the beat: halves, quarters or eighths by Density"
            }
            BassStyle::OctavePump => "eighths: the root on the beat, its octave on the off-beat",
            BassStyle::Walking => {
                "quarters through chord and scale tones, leading into each next root"
            }
            BassStyle::Acid => {
                "a looping 16th line with accents (louder notes), ties and octave jumps"
            }
            BassStyle::Syncopated => "16th syncopation around the root and the fifth",
            BassStyle::Euclid => {
                "pulses spread evenly over steps (Advanced), on roots, fifths and octaves"
            }
        }
    }
}

/// the Bass tab's settings
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BassOpts {
    pub seed: u64,
    pub style: BassStyle,
    /// 0..1: how many of the grid's steps play (strong beats first)
    pub density: f32,
    /// octave of the lowest note (the key's tonic near the C of it)
    pub octave: u8,
    /// semitones from the lowest to the highest note (12..=36)
    pub range: u8,
    /// Euclid: onsets / steps per bar / rotation
    pub pulses: u8,
    pub steps: u8,
    pub rotate: u8,
    /// 0..1: chance the note before a chord change leads into the next root
    pub approach: f32,
    /// 0..1: chance a step plays the fifth or the octave instead of the root
    pub fifths: f32,
    /// 0..1 (Acid): chance a note is accented (louder)
    pub accent: f32,
    /// 0..1 (Acid): chance a note is tied (held into the next one)
    pub tie: f32,
    /// 0.1..1: note length as a share of the gap to the next note
    pub legato: f32,
}

impl Default for BassOpts {
    fn default() -> Self {
        Self {
            seed: 1,
            style: BassStyle::Root,
            density: 0.5,
            octave: 2,
            range: 19,
            pulses: 3,
            steps: 8,
            rotate: 0,
            approach: 0.5,
            fifths: 0.3,
            accent: 0.3,
            tie: 0.2,
            legato: 0.85,
        }
    }
}

/// the lowest and highest note the bass may use: the key's tonic nearest
/// the C of Octave, then Range semitones up
pub fn bounds(key: Key, o: &BassOpts) -> (u8, u8) {
    let range = o.range.clamp(12, 36) as i32;
    let r = (key.root % 12) as i32;
    let mut lo = o.octave.min(8) as i32 * 12 + 1 + if r >= 6 { r - 12 } else { r };
    if lo < 1 {
        lo += 12;
    }
    while lo + range > 120 {
        lo -= 12;
    }
    (lo as u8, (lo + range) as u8)
}

/// what a step plays, read against the chord sounding there
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Root,
    Fifth,
    Octave,
    /// `n` scale steps above the root (Acid colour notes)
    Scale(u8),
}

#[derive(Clone, Copy, Debug)]
struct Step {
    /// position in the pattern (16ths)
    at: u8,
    role: Role,
    accent: bool,
    tie: bool,
}

/// the chord at `t`: (bass pitch class, root, chord tones)
fn chord_at(key: Key, marks: &[ChordMark], t: Tick) -> (u8, u8, Vec<u8>) {
    match marks.iter().find(|m| t >= m.start && t < m.end()) {
        Some(m) => (m.bass.unwrap_or(m.root) % 12, m.root % 12, m.pcs()),
        None => {
            let s = key.scale.steps();
            let pcs: Vec<u8> = [0usize, 2, 4]
                .iter()
                .map(|&d| (key.root + s[d % s.len()]) % 12)
                .collect();
            (key.root % 12, key.root % 12, pcs)
        }
    }
}

/// the fifth of a chord (its 5th-ish tone, else a perfect fifth)
fn fifth_of(root: u8, pcs: &[u8]) -> u8 {
    pcs.iter()
        .copied()
        .find(|&p| matches!((p + 12 - root) % 12, 7 | 6 | 8))
        .unwrap_or((root + 7) % 12)
}

/// the note with pitch class `pc` in lo..=hi nearest `near`
fn fold(pc: u8, near: i32, lo: u8, hi: u8) -> u8 {
    (lo..=hi)
        .filter(|&n| pc_of(n) == pc % 12)
        .min_by_key(|&n| ((n as i32 - near).abs(), n))
        .unwrap_or(lo)
}

/// position weights of a 16-step bar (acid-banger style: the beat, then
/// the off-beat 8ths, then the 16ths)
const ACID_W: [f32; 16] = [
    1.0, 0.35, 0.6, 0.45, 0.8, 0.35, 0.65, 0.5, 0.9, 0.35, 0.6, 0.45, 0.8, 0.4, 0.7, 0.55,
];
/// syncopated 16ths: the beat, the "and" of 1, the "e" of 3 ...
const SYNC_W: [f32; 16] = [
    1.0, 0.1, 0.2, 0.75, 0.15, 0.2, 0.8, 0.1, 0.35, 0.15, 0.75, 0.2, 0.15, 0.65, 0.2, 0.45,
];

fn keep(d: f32, w: f32) -> f32 {
    (d * 1.5 - 0.3 + (w - 0.5) * 0.9).clamp(0.0, 1.0)
}

/// the style's repeating pattern: its length in 16ths and its steps
fn pattern(o: &BassOpts, rng: &mut Rng) -> (u8, Vec<Step>) {
    let d = o.density.clamp(0.0, 1.0);
    let fif = o.fifths.clamp(0.0, 1.0);
    let st = |at: u8, role: Role| Step {
        at,
        role,
        accent: false,
        tie: false,
    };
    // a root, sometimes the fifth or the octave (never on beat 1)
    let mut colour = |rng: &mut Rng, at: u8| {
        if at == 0 || rng.f() >= fif {
            Role::Root
        } else if rng.f() < 0.6 {
            Role::Fifth
        } else {
            Role::Octave
        }
    };
    match o.style {
        BassStyle::Root => {
            let every: u8 = if d < 0.34 {
                8
            } else if d < 0.7 {
                4
            } else {
                2
            };
            let v = (0..16)
                .step_by(every as usize)
                .map(|at| st(at, colour(rng, at)))
                .collect();
            (16, v)
        }
        BassStyle::OctavePump => {
            let mut v = vec![];
            for at in (0..16u8).step_by(2) {
                let off = at % 4 == 2;
                let p = if off { keep(d, 0.75) } else { keep(d, 1.0) };
                if at == 0 || rng.f() < p {
                    v.push(st(at, if off { Role::Octave } else { Role::Root }));
                }
            }
            (16, v)
        }
        BassStyle::Walking => {
            // quarters; Density under a third: the two-feel (halves)
            let every = if d < 0.34 { 8 } else { 4 };
            (
                16,
                (0..16)
                    .step_by(every)
                    .map(|at| st(at, Role::Root))
                    .collect(),
            )
        }
        BassStyle::Acid => {
            let mut v = vec![];
            for at in 0..32u8 {
                let w = ACID_W[at as usize % 16];
                if at != 0 && rng.f() >= keep(d, w) {
                    continue;
                }
                // root-heavy note set: root, octave, fifth, scale colours
                let r = rng.f();
                let role = if at == 0 || r < 0.5 {
                    Role::Root
                } else if r < 0.7 {
                    Role::Octave
                } else if r < 0.7 + 0.15 * (0.5 + fif) {
                    Role::Fifth
                } else {
                    Role::Scale([1u8, 2, 3, 5, 6][rng.pick(5)])
                };
                let accent = rng.f() < o.accent.clamp(0.0, 1.0);
                let tie = rng.f() < o.tie.clamp(0.0, 1.0);
                v.push(Step {
                    at,
                    role,
                    accent,
                    tie,
                });
            }
            (32, v)
        }
        BassStyle::Syncopated => {
            let mut v = vec![];
            for at in 0..16u8 {
                if at == 0 || rng.f() < keep(d, SYNC_W[at as usize]) {
                    v.push(st(at, colour(rng, at)));
                }
            }
            (16, v)
        }
        BassStyle::Euclid => {
            let steps = o.steps.clamp(1, 16);
            let per = 16 / steps;
            let v = euclid(o.pulses, steps, o.rotate)
                .iter()
                .enumerate()
                .filter(|(_, &on)| on)
                .map(|(i, _)| {
                    let at = i as u8 * per;
                    st(at, colour(rng, at))
                })
                .collect();
            (16, v)
        }
    }
}

/// a bass line over `marks` from `start` to `end` (ticks of the note data,
/// whole bars from `start`). Same inputs, same notes.
pub fn generate(
    key: Key,
    marks: &[ChordMark],
    start: Tick,
    end: Tick,
    o: &BassOpts,
    locks: Locks,
) -> Vec<MNote> {
    if end <= start {
        return vec![];
    }
    let seed = o.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut rr = Rng((seed ^ 0xBA55_0001) | 1);
    let mut pr = Rng((seed.rotate_left(21) ^ 0xBA55_F00D) | 1);
    let (plen, pat) = pattern(o, &mut rr);
    // chord starts inside the span (always a note there)
    let mut starts: Vec<Tick> = marks
        .iter()
        .map(|m| m.start)
        .filter(|&t| t > start && t < end)
        .collect();
    starts.push(start);
    // 1. rhythm: (time, step)
    let mut ev: Vec<(Tick, Option<Step>)> = match locks.rhythm {
        Some(r) => r
            .iter()
            .map(|x| (x.0, None))
            .filter(|&(t, _)| t >= start && t < end)
            .collect(),
        None => {
            let mut v = vec![];
            let span = plen as Tick * STEP;
            let mut a = start;
            while a < end {
                for s in &pat {
                    let t = a + s.at as Tick * STEP;
                    if t < end {
                        v.push((t, Some(*s)));
                    }
                }
                a += span;
            }
            for &t in &starts {
                if !v.iter().any(|e| e.0 == t) {
                    v.push((t, None));
                }
            }
            v
        }
    };
    ev.sort_by_key(|e| e.0);
    ev.dedup_by_key(|e| e.0);
    if ev.is_empty() {
        return vec![];
    }
    let times: Vec<Tick> = ev.iter().map(|e| e.0).collect();
    // 2. pitch
    let (lo, hi) = bounds(key, o);
    let home = lo as i32 + 4;
    let scale: Vec<u8> = (lo..=hi).filter(|&n| key.contains(pc_of(n))).collect();
    let next_change = |t: Tick| -> Option<Tick> {
        marks
            .iter()
            .map(|m| m.start)
            .filter(|&s| s > t && s < end)
            .min()
    };
    let mut keys: Vec<u8> = Vec::with_capacity(times.len());
    match locks.pitches.filter(|p| !p.is_empty()) {
        Some(p) => {
            for i in 0..times.len() {
                keys.push(p[i % p.len()].clamp(1, 120));
            }
        }
        None => {
            let mut prev: Option<u8> = None;
            for (i, &(t, step)) in ev.iter().enumerate() {
                let (bass, root, pcs) = chord_at(key, marks, t);
                let at_start = starts.contains(&t) || marks.iter().any(|m| m.start == t);
                // the root near the bottom, drifting a little with the line
                // (Octave pump keeps its low note in place)
                let near = match prev {
                    Some(p) if o.style != BassStyle::OctavePump => (p as i32 + home) / 2,
                    _ => home,
                };
                let base = fold(bass, near, lo, hi);
                // the next onset starts a new chord: lead into it
                let nt = times.get(i + 1).copied();
                let change = next_change(t);
                let leads = !at_start
                    && nt.is_some()
                    && nt == change
                    && chord_at(key, marks, change.unwrap_or(t)).0 != bass;
                let n = if leads && pr.f() < o.approach.clamp(0.0, 1.0) {
                    let tb = chord_at(key, marks, change.unwrap_or(t)).0;
                    let near = prev.map_or(home, |p| p as i32);
                    let target = fold(tb, near, lo, hi) as i32;
                    let r = pr.f();
                    let cand = if r < 0.45 {
                        // chromatic: from below, or from above
                        if pr.f() < 0.6 {
                            target - 1
                        } else {
                            target + 1
                        }
                    } else if r < 0.75 {
                        // the scale step next to it, on the side we come from
                        let up = near > target;
                        scale
                            .iter()
                            .map(|&s| s as i32)
                            .filter(|&s| if up { s > target } else { s < target })
                            .min_by_key(|&s| (s - target).abs())
                            .unwrap_or(target + if up { 2 } else { -2 })
                    } else {
                        // its fifth (the dominant of the next chord)
                        fold((tb + 7) % 12, near, lo, hi) as i32
                    };
                    if cand < lo as i32 || cand > hi as i32 {
                        // out of range: from the other side
                        (2 * target - cand).clamp(lo as i32, hi as i32) as u8
                    } else {
                        cand as u8
                    }
                } else if at_start || o.style == BassStyle::Root && step.is_none() {
                    base
                } else if o.style == BassStyle::Walking {
                    walk(
                        prev.unwrap_or(base),
                        &pcs,
                        &scale,
                        change,
                        t,
                        key,
                        marks,
                        lo,
                        hi,
                        &mut pr,
                    )
                } else {
                    match step.map(|s| s.role).unwrap_or(Role::Root) {
                        Role::Root => base,
                        Role::Fifth => fold(fifth_of(root, &pcs), base as i32 + 4, lo, hi),
                        Role::Octave => {
                            if base + 12 <= hi {
                                base + 12
                            } else if base >= lo + 12 {
                                base - 12
                            } else {
                                base
                            }
                        }
                        Role::Scale(k) => {
                            let s = key.scale.steps();
                            // the scale degree nearest the root, k steps up
                            let d0 = (0..s.len())
                                .min_by_key(|&j| (key.root + s[j] + 12 - root) % 12)
                                .unwrap_or(0);
                            let pc = (key.root + s[(d0 + k as usize) % s.len()]) % 12;
                            fold(pc, base as i32 + 3, lo, hi)
                        }
                    }
                };
                keys.push(n);
                prev = Some(n);
            }
        }
    }
    // lengths: monophonic, legato or tied
    let mut out = Vec::with_capacity(times.len());
    for (i, &t) in times.iter().enumerate() {
        let next = times.get(i + 1).copied().unwrap_or(end);
        let gap = (next - t).max(1);
        let step = ev[i].1;
        let len = match locks.rhythm {
            Some(r) => r.iter().find(|x| x.0 == t).map_or(PPQ, |x| x.1).min(gap),
            None if step.is_some_and(|s| s.tie) => gap,
            None => {
                let l = o.legato.clamp(0.1, 1.0);
                // Acid: short notes unless tied
                let l = if o.style == BassStyle::Acid {
                    l.min(0.5)
                } else {
                    l
                };
                ((gap as f32 * l) as Tick).clamp(PPQ / 8, gap)
            }
        };
        let strong = t.rem_euclid(PPQ) == 0;
        let vel = if o.style == BassStyle::Acid {
            if step.is_some_and(|s| s.accent) {
                1.0
            } else {
                0.68
            }
        } else if t.rem_euclid(BAR) == 0 {
            0.9
        } else if strong {
            0.82
        } else {
            0.72
        };
        out.push(MNote {
            start: t,
            len: len.max(1),
            key: keys[i],
            vel,
        });
    }
    out
}

/// Walking: the next quarter, through chord tones (on beat 3) and scale
/// steps, heading for the next chord's root
#[allow(clippy::too_many_arguments)]
fn walk(
    prev: u8,
    pcs: &[u8],
    scale: &[u8],
    change: Option<Tick>,
    t: Tick,
    key: Key,
    marks: &[ChordMark],
    lo: u8,
    hi: u8,
    rng: &mut Rng,
) -> u8 {
    let p = prev as i32;
    let goal = match change {
        Some(c) => fold(chord_at(key, marks, c).0, p, lo, hi) as i32,
        None => p,
    };
    let beats_left = change.map_or(4, |c| ((c - t) / PPQ).max(1)) as i32;
    // where the line should be now to arrive in time
    let aim = p + (goal - p) / (beats_left + 1);
    let chord_beat = t.rem_euclid(2 * PPQ) == 0;
    let mut best = prev;
    let mut score = f32::MAX;
    for n in lo..=hi {
        let ni = n as i32;
        if ni == p || (ni - p).abs() > 7 {
            continue;
        }
        let ct = pcs.contains(&pc_of(n));
        if !ct && !scale.contains(&n) {
            continue;
        }
        let mut s = (ni - aim).abs() as f32 + rng.f() * 2.5;
        if ct {
            s -= if chord_beat { 2.5 } else { 1.0 };
        }
        if s < score {
            score = s;
            best = n;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chordgen::{self as cg, Quality};
    use crate::chords::Scale;

    fn prog(key: Key, degs: &[(u8, Quality)], bars_each: Tick) -> Vec<ChordMark> {
        degs.iter()
            .enumerate()
            .map(|(i, &(off, q))| ChordMark {
                start: i as Tick * bars_each * BAR,
                len: bars_each * BAR,
                root: (key.root + off) % 12,
                q,
                bass: None,
            })
            .collect()
    }

    /// C major I-vi-IV-V (2 bars each), A minor i-VI-III-VII (1 bar each),
    /// a chord every half bar in E minor
    fn cases() -> Vec<(Key, Vec<ChordMark>)> {
        let c = Key::new(0, Scale::Major);
        let a = Key::new(9, Scale::Minor);
        let e = Key::new(4, Scale::Minor);
        let cp = [
            (0, Quality::Maj),
            (9, Quality::Min),
            (5, Quality::Maj),
            (7, Quality::Maj),
        ];
        let ap = [
            (0, Quality::Min),
            (8, Quality::Maj),
            (3, Quality::Maj),
            (10, Quality::Maj),
        ];
        let mut half = prog(e, &[(0, Quality::Min), (7, Quality::Maj)], 1);
        half = half
            .iter()
            .cycle()
            .take(16)
            .enumerate()
            .map(|(i, m)| ChordMark {
                start: i as Tick * BAR / 2,
                len: BAR / 2,
                ..*m
            })
            .collect();
        vec![
            (c, prog(c, &cp, 2)),
            (
                a,
                [prog(a, &ap, 1), prog(a, &ap, 1)]
                    .concat()
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| ChordMark {
                        start: i as Tick * BAR,
                        ..m
                    })
                    .collect(),
            ),
            (e, half),
        ]
    }

    fn grid() -> Vec<BassOpts> {
        let mut v = vec![];
        for (i, s) in BassStyle::ALL.iter().enumerate() {
            for j in 0..6usize {
                v.push(BassOpts {
                    seed: (i * 31 + j * 7) as u64 + 3,
                    style: *s,
                    density: 0.1 + 0.17 * j as f32,
                    octave: [1, 2, 3][j % 3],
                    range: [12, 19, 24, 36][(i + j) % 4],
                    approach: [0.0, 0.5, 1.0][j % 3],
                    pulses: 2 + j as u8,
                    steps: [8, 16, 12][j % 3],
                    rotate: j as u8,
                    ..BassOpts::default()
                });
            }
        }
        v
    }

    fn gen(key: Key, marks: &[ChordMark], o: &BassOpts) -> Vec<MNote> {
        generate(key, marks, 0, 8 * BAR, o, Locks::default())
    }

    #[test]
    fn same_seed_same_line_and_seeds_differ() {
        for (key, marks) in cases() {
            for o in grid() {
                assert_eq!(gen(key, &marks, &o), gen(key, &marks, &o), "{o:?}");
            }
            for style in [BassStyle::Acid, BassStyle::Syncopated, BassStyle::Walking] {
                let lines: Vec<Vec<MNote>> = (1..6)
                    .map(|s| {
                        gen(
                            key,
                            &marks,
                            &BassOpts {
                                seed: s,
                                style,
                                density: 0.6,
                                ..BassOpts::default()
                            },
                        )
                    })
                    .collect();
                assert!(
                    lines.windows(2).any(|w| w[0] != w[1]),
                    "{style:?}: the seed changes the line"
                );
            }
        }
    }

    #[test]
    fn notes_stay_in_range_and_in_the_span_and_are_monophonic() {
        for (key, marks) in cases() {
            for o in grid() {
                let (lo, hi) = bounds(key, &o);
                assert!(hi - lo >= 12 && lo >= 1 && hi <= 120);
                let v = gen(key, &marks, &o);
                assert!(!v.is_empty(), "{o:?}");
                for n in &v {
                    assert!(
                        n.key >= lo && n.key <= hi,
                        "{} not in {lo}..={hi} {o:?}",
                        n.key
                    );
                    assert!(n.start >= 0 && n.start < 8 * BAR && n.len > 0);
                    assert!(n.vel > 0.0 && n.vel <= 1.0);
                }
                for w in v.windows(2) {
                    assert!(w[0].start < w[1].start, "sorted, one note at a time");
                    assert!(w[0].start + w[0].len <= w[1].start, "monophonic {o:?}");
                }
            }
        }
    }

    #[test]
    fn every_chord_starts_on_its_root_or_slash_bass() {
        let mut all = cases();
        // with a slash bass: C/E, Am, F, G/B
        let c = Key::new(0, Scale::Major);
        let mut sl = all[0].1.clone();
        sl[0].bass = Some(4);
        sl[3].bass = Some(11);
        all.push((c, sl));
        for (key, marks) in all {
            for o in grid() {
                let v = gen(key, &marks, &o);
                for m in marks.iter().filter(|m| m.start < 8 * BAR) {
                    let n = v
                        .iter()
                        .find(|n| n.start == m.start)
                        .unwrap_or_else(|| panic!("a note on {} {o:?}", m.start));
                    assert_eq!(pc_of(n.key), m.bass.unwrap_or(m.root), "{o:?} {m:?}");
                }
            }
        }
    }

    #[test]
    fn root_style_without_approach_plays_only_roots() {
        for (key, marks) in cases() {
            let o = BassOpts {
                style: BassStyle::Root,
                fifths: 0.0,
                approach: 0.0,
                density: 0.5,
                ..BassOpts::default()
            };
            let v = gen(key, &marks, &o);
            assert!(v.iter().all(|n| n.start % PPQ == 0), "quarters");
            for n in &v {
                let m = marks
                    .iter()
                    .find(|m| n.start >= m.start && n.start < m.end())
                    .unwrap();
                assert_eq!(pc_of(n.key), m.root);
            }
        }
    }

    #[test]
    fn walking_is_quarters_and_leads_into_the_next_root() {
        for (key, marks) in cases().into_iter().take(2) {
            let o = BassOpts {
                style: BassStyle::Walking,
                approach: 1.0,
                density: 0.6,
                seed: 9,
                ..BassOpts::default()
            };
            let v = gen(key, &marks, &o);
            let starts: Vec<Tick> = v.iter().map(|n| n.start).collect();
            let want: Vec<Tick> = (0..32).map(|i| i * PPQ).collect();
            assert_eq!(starts, want, "a note on every beat");
            for w in v.windows(2) {
                let Some(m) = marks.iter().find(|m| m.start == w[1].start) else {
                    continue;
                };
                if marks.iter().any(|p| p.end() == m.start && p.root == m.root) {
                    continue;
                }
                let d = (w[0].key as i32 - w[1].key as i32).rem_euclid(12);
                assert!(
                    [1, 2, 11, 10, 5, 7, 3, 9].contains(&d),
                    "an approach into {m:?}: {d}"
                );
            }
            // the line moves: no long run of the same note
            assert!(v
                .windows(3)
                .all(|w| !(w[0].key == w[1].key && w[1].key == w[2].key)));
        }
    }

    #[test]
    fn octave_pump_jumps_an_octave_on_the_off_beat() {
        let (key, marks) = cases().remove(0);
        let o = BassOpts {
            style: BassStyle::OctavePump,
            density: 1.0,
            approach: 0.0,
            // room for every root's octave (in 19 a high root stays put)
            range: 24,
            ..BassOpts::default()
        };
        let v = gen(key, &marks, &o);
        assert_eq!(v.len(), 64, "every eighth");
        for w in v.chunks(2) {
            assert_eq!((w[1].key as i32 - w[0].key as i32).abs(), 12, "{w:?}");
        }
    }

    #[test]
    fn acid_accents_ties_and_loops() {
        let (key, marks) = cases().remove(1);
        let o = BassOpts {
            style: BassStyle::Acid,
            density: 0.7,
            accent: 0.4,
            tie: 0.3,
            approach: 0.0,
            seed: 4,
            ..BassOpts::default()
        };
        let v = gen(key, &marks, &o);
        assert!(
            v.iter().any(|n| n.vel >= 1.0) && v.iter().any(|n| n.vel < 0.8),
            "accents"
        );
        assert!(v.iter().any(|n| n.start % PPQ != 0), "16ths");
        // two-bar loop: the rhythm of bars 1-2 comes back in bars 3-4
        let bars = |a: Tick| -> Vec<Tick> {
            v.iter()
                .filter(|n| n.start >= a && n.start < a + 2 * BAR)
                .map(|n| n.start - a)
                .collect()
        };
        assert_eq!(bars(0), bars(2 * BAR));
        // tied notes are held to the next one
        assert!(v.windows(2).any(|w| w[0].start + w[0].len == w[1].start));
        let ao = BassOpts { accent: 0.0, ..o };
        assert!(gen(key, &marks, &ao).iter().all(|n| n.vel < 0.8));
    }

    #[test]
    fn euclid_follows_pulses() {
        let (key, marks) = cases().remove(0);
        let o = BassOpts {
            style: BassStyle::Euclid,
            pulses: 3,
            steps: 8,
            rotate: 0,
            ..BassOpts::default()
        };
        let v = gen(key, &marks, &o);
        let bar: Vec<Tick> = v
            .iter()
            .filter(|n| n.start < BAR)
            .map(|n| n.start)
            .collect();
        assert_eq!(bar, vec![0, 3 * PPQ / 2, 3 * PPQ], "x..x..x.");
    }

    #[test]
    fn locks_keep_rhythm_or_notes() {
        let (key, marks) = cases().remove(0);
        let o = BassOpts {
            style: BassStyle::Syncopated,
            density: 0.6,
            ..BassOpts::default()
        };
        let first = gen(key, &marks, &o);
        let r: Vec<(Tick, Tick)> = first.iter().map(|n| (n.start, n.len)).collect();
        let o2 = BassOpts { seed: 77, ..o };
        let kept = generate(
            key,
            &marks,
            0,
            8 * BAR,
            &o2,
            Locks {
                rhythm: Some(&r),
                pitches: None,
            },
        );
        assert_eq!(kept.iter().map(|n| (n.start, n.len)).collect::<Vec<_>>(), r);
        let p: Vec<u8> = first.iter().map(|n| n.key).collect();
        let o3 = BassOpts {
            style: BassStyle::Acid,
            ..o2
        };
        let kn = generate(
            key,
            &marks,
            0,
            8 * BAR,
            &o3,
            Locks {
                rhythm: None,
                pitches: Some(&p),
            },
        );
        assert!(kn.iter().enumerate().all(|(i, n)| n.key == p[i % p.len()]));
    }

    #[test]
    fn follows_every_chord_generator_style() {
        // the Chords tab's progressions: chord starts get the bass note
        for st in 0..cg::STYLES.len() {
            let co = cg::GenOpts {
                style: st,
                seed: st as u64 + 1,
                ..cg::GenOpts::default()
            };
            let g = cg::generate(&co);
            for style in BassStyle::ALL {
                let o = BassOpts {
                    style,
                    ..BassOpts::default()
                };
                let end = g.marks.iter().map(|m| m.end()).max().unwrap_or(BAR);
                let v = generate(g.key, &g.marks, 0, end, &o, Locks::default());
                for m in &g.marks {
                    let n = v.iter().find(|n| n.start == m.start).expect("chord start");
                    assert_eq!(pc_of(n.key), m.bass.unwrap_or(m.root) % 12);
                }
            }
        }
    }
}
