//! Melody generator: a tune over a chord progression (docs/CHORDS.md
//! "Melody generator", the Generators panel's Melody tab). Rule based and
//! seeded, so the same settings and seed always give the same notes:
//!
//! 1. **Rhythm**: a grid preset thinned / filled by Density (strong beats
//!    first), or a Euclidean rhythm (Bjorklund's algorithm, as described by
//!    Toussaint 2005), then rests.
//! 2. **Pitch**: chord tones on the strong beats (beats 1 and 3) with the
//!    chance Chord-tone bias, scale steps between them; a note outside the
//!    chord (a passing or neighbour tone) resolves by step; Leap sets how
//!    often the line jumps, and a leap is followed by a step back
//!    (leap recovery). A contour (Arch, Rise, Fall, Wave, Walk) pulls the
//!    line inside Range.
//! 3. **Motif**: AABA / ABAB / call and response repeat the first phrase's
//!    rhythm and its notes, moved by a whole number of semitones that keeps
//!    every note in the scale (exact at Variation 0).
//! 4. **End on tonic**: the last note is the key's tonic.
//!
//! Ideas from the Computoser paper (Bozhanov), the Euclidean rhythm paper
//! (Toussaint) and the pattern libraries total-serialism and isobar, and
//! the lock / mutate-a-loop and quantiser ideas in Emilie Gillet's
//! eurorack firmware; no code was copied (credits.rs, Research).
use crate::chordgen::{ChordMark, Rng};
use crate::chords::{pc_of, Key};
use crate::timeline::{Tick, PPQ};

pub const BAR: Tick = 4 * PPQ;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rhythm {
    Quarters,
    #[default]
    Eighths,
    Syncopated,
    Sixteenths,
    Long,
    /// the Euclid settings (pulses / steps / rotate)
    Euclid,
}

impl Rhythm {
    pub const ALL: [Rhythm; 6] = [
        Rhythm::Quarters,
        Rhythm::Eighths,
        Rhythm::Syncopated,
        Rhythm::Sixteenths,
        Rhythm::Long,
        Rhythm::Euclid,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Rhythm::Quarters => "Quarters",
            Rhythm::Eighths => "Eighths",
            Rhythm::Syncopated => "Syncopated",
            Rhythm::Sixteenths => "Sixteenths",
            Rhythm::Long => "Long notes",
            Rhythm::Euclid => "Euclid",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Contour {
    #[default]
    Arch,
    Rise,
    Fall,
    Wave,
    Walk,
}

impl Contour {
    pub const ALL: [Contour; 5] = [
        Contour::Arch,
        Contour::Rise,
        Contour::Fall,
        Contour::Wave,
        Contour::Walk,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Contour::Arch => "Arch",
            Contour::Rise => "Rise",
            Contour::Fall => "Fall",
            Contour::Wave => "Wave",
            Contour::Walk => "Walk",
        }
    }
    /// 0..1 height at `u` (0..1 through the melody); None: free (Walk)
    fn at(self, u: f32) -> Option<f32> {
        use std::f32::consts::PI;
        match self {
            Contour::Arch => Some(0.2 + 0.7 * (PI * u).sin()),
            Contour::Rise => Some(0.15 + 0.75 * u),
            Contour::Fall => Some(0.9 - 0.75 * u),
            Contour::Wave => Some(0.5 + 0.35 * (2.0 * PI * 2.0 * u).sin()),
            Contour::Walk => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motif {
    Off,
    #[default]
    Aaba,
    Abab,
    CallResponse,
}

impl Motif {
    pub const ALL: [Motif; 4] = [Motif::Off, Motif::Aaba, Motif::Abab, Motif::CallResponse];
    pub fn name(self) -> &'static str {
        match self {
            Motif::Off => "Off",
            Motif::Aaba => "AABA",
            Motif::Abab => "ABAB",
            Motif::CallResponse => "Call & response",
        }
    }
    /// the four phrases: the same letter repeats; R and S answer A with
    /// its rhythm and new notes
    fn plan(self) -> &'static [u8; 4] {
        match self {
            Motif::Off => b"ABCD",
            Motif::Aaba => b"AABA",
            Motif::Abab => b"ABAB",
            Motif::CallResponse => b"ARAS",
        }
    }
}

/// the Melody tab's settings
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MelOpts {
    pub seed: u64,
    pub rhythm: Rhythm,
    /// 0..1: how many of the grid's steps play (strong beats first)
    pub density: f32,
    /// semitones from the lowest to the highest note (12..=36)
    pub range: u8,
    /// octave of the lowest note (the key's tonic in that octave)
    pub octave: u8,
    pub contour: Contour,
    pub motif: Motif,
    /// Euclid: onsets / steps per bar / rotation
    pub pulses: u8,
    pub steps: u8,
    pub rotate: u8,
    /// 0..1: how often the line jumps instead of stepping
    pub leap: f32,
    /// 0..1: chance a strong beat is a chord tone
    pub chord_bias: f32,
    /// 0..1: repeated phrases change a few notes
    pub variation: f32,
    pub end_tonic: bool,
    /// 0..1: chance an onset becomes a rest
    pub rest: f32,
    /// 0.1..1: note length as a share of the gap to the next note
    pub legato: f32,
}

impl Default for MelOpts {
    fn default() -> Self {
        Self {
            seed: 1,
            rhythm: Rhythm::Eighths,
            density: 0.6,
            range: 19,
            octave: 5,
            contour: Contour::Arch,
            motif: Motif::Aaba,
            pulses: 5,
            steps: 8,
            rotate: 0,
            leap: 0.25,
            chord_bias: 0.75,
            variation: 0.3,
            end_tonic: true,
            rest: 0.1,
            legato: 0.9,
        }
    }
}

/// a generated note (times relative to the note data, as `ChordMark`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MNote {
    pub start: Tick,
    pub len: Tick,
    /// tracker note (1 = C-0)
    pub key: u8,
    pub vel: f32,
}

/// parts kept from the last result (Lock rhythm / Lock notes)
#[derive(Clone, Copy, Debug, Default)]
pub struct Locks<'a> {
    /// (start, len) of every note
    pub rhythm: Option<&'a [(Tick, Tick)]>,
    /// the notes in order (reused in order, cycling)
    pub pitches: Option<&'a [u8]>,
}

/// Bjorklund's algorithm: `pulses` onsets spread as evenly as possible over
/// `steps`, rotated left by `rotate` (Toussaint 2005: E(3,8) = x..x..x.)
pub fn euclid(pulses: u8, steps: u8, rotate: u8) -> Vec<bool> {
    let n = steps.max(1) as usize;
    let k = (pulses as usize).min(n);
    let mut out = if k == 0 {
        vec![false; n]
    } else if k == n {
        vec![true; n]
    } else {
        let mut a: Vec<Vec<bool>> = vec![vec![true]; k];
        let mut b: Vec<Vec<bool>> = vec![vec![false]; n - k];
        while b.len() > 1 {
            let m = a.len().min(b.len());
            let mut na = Vec::with_capacity(m);
            for i in 0..m {
                let mut x = a[i].clone();
                x.extend_from_slice(&b[i]);
                na.push(x);
            }
            let rest = if a.len() > m {
                a[m..].to_vec()
            } else {
                b[m..].to_vec()
            };
            a = na;
            b = rest;
        }
        a.into_iter().chain(b).flatten().collect()
    };
    out.rotate_left(rotate as usize % n);
    out
}

/// "x..x..x." for a pattern (tests, tooltips)
pub fn euclid_str(p: &[bool]) -> String {
    p.iter().map(|&b| if b { 'x' } else { '.' }).collect()
}

/// the lowest and highest note the melody may use
pub fn bounds(key: Key, o: &MelOpts) -> (u8, u8) {
    let range = o.range.clamp(12, 36) as i32;
    let mut lo = o.octave.min(9) as i32 * 12 + 1 + (key.root % 12) as i32;
    while lo + range > 120 {
        lo -= 12;
    }
    (lo as u8, (lo + range) as u8)
}

/// a strong beat: beat 1 or 3 of a 4/4 bar
pub fn strong(t: Tick) -> bool {
    t.rem_euclid(2 * PPQ) == 0
}

/// metric weight of a position: bar 1, beat 3, beats 2/4, 8ths, the rest
fn strength(t: Tick) -> f32 {
    let t = t.rem_euclid(BAR);
    if t == 0 {
        1.0
    } else if t % (2 * PPQ) == 0 {
        0.8
    } else if t % PPQ == 0 {
        0.6
    } else if t % (PPQ / 2) == 0 {
        0.4
    } else {
        0.2
    }
}

/// one bar's onsets from `t0`
fn bar_onsets(o: &MelOpts, rng: &mut Rng, t0: Tick) -> Vec<Tick> {
    if o.rhythm == Rhythm::Euclid {
        let steps = o.steps.clamp(1, 32);
        let step = BAR / steps as Tick;
        return euclid(o.pulses, steps, o.rotate)
            .iter()
            .enumerate()
            .filter(|(_, &on)| on)
            .map(|(i, _)| t0 + i as Tick * step)
            .collect();
    }
    let g: Tick = match o.rhythm {
        Rhythm::Quarters => 4,
        Rhythm::Sixteenths => 16,
        Rhythm::Long => 2,
        _ => 8,
    };
    const SYNC: [f32; 8] = [1.0, 0.3, 0.35, 0.9, 0.3, 0.35, 0.85, 0.4];
    let d = o.density.clamp(0.0, 1.0);
    let mut v = vec![];
    for i in 0..g {
        let t = i * BAR / g;
        let w = if o.rhythm == Rhythm::Syncopated {
            SYNC[i as usize]
        } else {
            strength(t)
        };
        let p = (d * 1.5 - 0.3 + (w - 0.5) * 0.9).clamp(0.0, 1.0);
        if rng.f() < p {
            v.push(t0 + t);
        }
    }
    if v.is_empty() {
        v.push(t0);
    }
    v
}

/// onsets from `a` for `bars` bars, with rests
fn onsets(o: &MelOpts, rng: &mut Rng, a: Tick, bars: Tick) -> Vec<Tick> {
    let mut v = vec![];
    for b in 0..bars {
        let bar = bar_onsets(o, rng, a + b * BAR);
        for t in bar {
            if v.is_empty() || rng.f() >= o.rest.clamp(0.0, 0.9) {
                v.push(t);
            }
        }
    }
    v
}

/// the chord sounding at `t` (else the key's tonic triad)
fn chord_at(key: Key, marks: &[ChordMark], t: Tick) -> Vec<u8> {
    marks
        .iter()
        .find(|m| t >= m.start && t < m.end())
        .map(|m| m.pcs())
        .unwrap_or_else(|| {
            let s = key.scale.steps();
            [0usize, 2, 4]
                .iter()
                .map(|&d| (key.root + s[d % s.len()]) % 12)
                .collect()
        })
}

struct Walker<'a> {
    key: Key,
    marks: &'a [ChordMark],
    lo: u8,
    hi: u8,
    /// scale notes lo..=hi
    pool: Vec<u8>,
    o: MelOpts,
    prev: Option<u8>,
    /// the last move in semitones
    last: i32,
    /// the previous note was outside its chord
    nct: bool,
    walk: f32,
}

impl Walker<'_> {
    fn idx(&self, n: u8) -> usize {
        (0..self.pool.len())
            .min_by_key(|&i| (self.pool[i] as i32 - n as i32).abs())
            .unwrap_or(0)
    }
    fn chord_tones(&self, pcs: &[u8]) -> Vec<u8> {
        let all: Vec<u8> = (self.lo..=self.hi)
            .filter(|&n| pcs.contains(&pc_of(n)))
            .collect();
        let scale: Vec<u8> = all
            .iter()
            .copied()
            .filter(|&n| self.key.contains(pc_of(n)))
            .collect();
        if scale.is_empty() {
            all
        } else {
            scale
        }
    }
    fn target(&self, u: f32) -> f32 {
        let h = self.o.contour.at(u).unwrap_or(self.walk);
        self.lo as f32 + h * (self.hi - self.lo) as f32
    }
    /// the next note at `t`
    fn next(&mut self, rng: &mut Rng, t: Tick, u: f32) -> u8 {
        let pcs = chord_at(self.key, self.marks, t);
        let target = self.target(u);
        let chord = self.chord_tones(&pcs);
        let bias = self.o.chord_bias.clamp(0.0, 1.0);
        let want_chord = strong(t) && rng.f() < bias;
        let n = match self.prev {
            None => *chord
                .iter()
                .min_by_key(|&&c| ((c as f32 - target).abs() * 10.0 + rng.f() * 30.0) as i32)
                .unwrap_or(&self.lo),
            Some(p) => {
                if want_chord {
                    let back = self.last.abs() > 4;
                    *chord
                        .iter()
                        .min_by_key(|&&c| {
                            let d = c as i32 - p as i32;
                            let mut s = d.abs() as f32 * (1.6 - self.o.leap)
                                + (c as f32 - target).abs() * 0.5
                                + rng.f() * 4.0;
                            // a passing tone resolves by step; a leap turns back
                            if self.nct && d.abs() > 2 {
                                s += 8.0;
                            }
                            if back && d.signum() == self.last.signum() {
                                s += 6.0;
                            }
                            (s * 10.0) as i32
                        })
                        .unwrap_or(&p)
                } else {
                    let i = self.idx(p) as i32;
                    let toward = if (target - p as f32).abs() < 1.5 {
                        if rng.f() < 0.5 {
                            1
                        } else {
                            -1
                        }
                    } else if target > p as f32 {
                        1
                    } else {
                        -1
                    };
                    let (dir, size) = if self.nct {
                        // resolve by step, to a chord tone when one is next door
                        let up = self.pool.get((i + 1) as usize).copied();
                        let dn = (i > 0).then(|| self.pool[(i - 1) as usize]);
                        let ct = |n: Option<u8>| n.is_some_and(|n| pcs.contains(&pc_of(n)));
                        match (ct(up), ct(dn)) {
                            (true, false) => (1, 1),
                            (false, true) => (-1, 1),
                            _ => (toward, 1),
                        }
                    } else if self.last.abs() > 4 && rng.f() < 0.85 {
                        (-self.last.signum(), 1)
                    } else {
                        let dir = if rng.f() < 0.75 { toward } else { -toward };
                        let size = if rng.f() < self.o.leap.clamp(0.0, 1.0) * 0.6 {
                            2 + rng.pick(3) as i32
                        } else if rng.f() < 0.1 {
                            0
                        } else {
                            1
                        };
                        (dir, size)
                    };
                    let mut j = i + dir * size;
                    let top = self.pool.len() as i32 - 1;
                    if j > top {
                        j = (2 * top - j).max(0);
                    }
                    if j < 0 {
                        j = (-j).min(top);
                    }
                    self.pool[j as usize]
                }
            }
        };
        self.walk = ((self.walk + (rng.f() - 0.5) * 0.25).clamp(0.1, 0.9)
            + (n as f32 - self.lo as f32) / (self.hi - self.lo).max(1) as f32)
            * 0.5;
        self.push(n, &pcs);
        n
    }
    fn push(&mut self, n: u8, pcs: &[u8]) {
        if let Some(p) = self.prev {
            self.last = n as i32 - p as i32;
        }
        self.prev = Some(n);
        self.nct = !pcs.contains(&pc_of(n));
    }
    /// the semitone move for a repeated phrase: every note stays in the
    /// scale and in range; prefers the first note on a chord tone, a small
    /// move and the contour
    fn best_shift(&self, src: &[u8], first_t: Tick, mid: f32) -> i32 {
        let mean = src.iter().map(|&n| n as f32).sum::<f32>() / src.len().max(1) as f32;
        let target = self.target(mid.clamp(0.0, 1.0));
        let pcs = chord_at(self.key, self.marks, first_t);
        (-7..=7)
            .filter(|s| {
                src.iter().all(|&n| {
                    let m = n as i32 + s;
                    m >= self.lo as i32 && m <= self.hi as i32 && self.key.contains(pc_of(m as u8))
                })
            })
            .min_by_key(|&s| {
                let first = src[0] as i32 + s;
                let ct = if pcs.contains(&pc_of(first as u8)) {
                    0.0
                } else {
                    4.0
                };
                let c = if self.o.contour == Contour::Walk {
                    0.0
                } else {
                    (mean + s as f32 - target).abs() * 0.15
                };
                ((ct + s.abs() as f32 * 0.3 + c) * 100.0) as i32
            })
            .unwrap_or(0)
    }
}

/// a melody over `marks` from `start` to `end` (ticks of the note data,
/// whole bars from `start`). Same inputs, same notes.
pub fn generate(
    key: Key,
    marks: &[ChordMark],
    start: Tick,
    end: Tick,
    o: &MelOpts,
    locks: Locks,
) -> Vec<MNote> {
    if end <= start {
        return vec![];
    }
    let seed = o.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut rr = Rng((seed ^ 0x5151_7A7A) | 1);
    let mut pr = Rng((seed.rotate_left(17) ^ 0xC0FF_EE11) | 1);
    let mut vr = Rng((seed.rotate_left(33) ^ 0x0BAD_F00D) | 1);
    let bars = ((end - start + BAR - 1) / BAR).max(1);
    // the phrases: (start, bars, letter)
    let mut secs: Vec<(Tick, Tick, u8)> = vec![];
    if o.motif == Motif::Off || bars < 4 {
        secs.push((start, bars, b'A'));
    } else {
        let sb = bars / 4;
        for (i, &l) in o.motif.plan().iter().enumerate() {
            secs.push((start + i as Tick * sb * BAR, sb, l));
        }
        if bars > sb * 4 {
            secs.push((start + sb * 4 * BAR, bars - sb * 4, b'T'));
        }
    }
    // 1. rhythm
    let times: Vec<Tick> = match locks.rhythm {
        Some(r) => {
            let mut v: Vec<Tick> = r
                .iter()
                .map(|x| x.0)
                .filter(|&t| t >= start && t < end)
                .collect();
            v.sort();
            v.dedup();
            v
        }
        None => {
            let mut first: Vec<(u8, Vec<Tick>)> = vec![];
            let mut v = vec![];
            for &(a, nb, l) in &secs {
                // R and S answer A with its rhythm
                let src = if l == b'R' || l == b'S' { b'A' } else { l };
                let rel = match first.iter().find(|f| f.0 == src) {
                    Some(f) => f.1.clone(),
                    None => {
                        let r: Vec<Tick> = onsets(o, &mut rr, 0, nb);
                        first.push((src, r.clone()));
                        r
                    }
                };
                v.extend(rel.into_iter().map(|t| a + t).filter(|&t| t < end));
            }
            v
        }
    };
    if times.is_empty() {
        return vec![];
    }
    let mut lens: Vec<Tick> = match locks.rhythm {
        Some(r) => times
            .iter()
            .map(|t| r.iter().find(|x| x.0 == *t).map_or(PPQ, |x| x.1))
            .collect(),
        None => times
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let next = times.get(i + 1).copied().unwrap_or(end);
                (((next - t) as f32 * o.legato.clamp(0.1, 1.0)) as Tick).max(PPQ / 8)
            })
            .collect(),
    };
    // 2. pitch
    let (lo, hi) = bounds(key, o);
    let pool: Vec<u8> = (lo..=hi).filter(|&n| key.contains(pc_of(n))).collect();
    let mut w = Walker {
        key,
        marks,
        lo,
        hi,
        pool,
        o: *o,
        prev: None,
        last: 0,
        nct: false,
        walk: 0.5,
    };
    let span = (end - start).max(1) as f32;
    let mut keys: Vec<u8> = Vec::with_capacity(times.len());
    match locks.pitches.filter(|p| !p.is_empty()) {
        Some(p) => {
            for i in 0..times.len() {
                keys.push(p[i % p.len()].clamp(1, 120));
            }
        }
        None => {
            // the notes of each letter's first phrase
            let mut first: Vec<(u8, Vec<u8>)> = vec![];
            for &(a, nb, l) in &secs {
                let b = a + nb * BAR;
                let idx: Vec<usize> = (0..times.len())
                    .filter(|&i| times[i] >= a && times[i] < b)
                    .collect();
                let copy = first
                    .iter()
                    .find(|f| f.0 == l)
                    .filter(|f| f.1.len() == idx.len() && !idx.is_empty())
                    .map(|f| f.1.clone());
                match copy {
                    Some(src) => {
                        let mid = ((a + b) / 2 - start) as f32 / span;
                        let shift = w.best_shift(&src, times[idx[0]], mid);
                        for (k, &i) in idx.iter().enumerate() {
                            let mut n = (src[k] as i32 + shift) as u8;
                            let pcs = chord_at(key, marks, times[i]);
                            if o.variation > 0.0 && pr.f() < o.variation.clamp(0.0, 1.0) * 0.5 {
                                let j = w.idx(n) as i32 + if pr.f() < 0.5 { 1 } else { -1 };
                                n = w.pool[j.clamp(0, w.pool.len() as i32 - 1) as usize];
                                if strong(times[i]) && pr.f() < o.chord_bias {
                                    let ct = w.chord_tones(&pcs);
                                    if let Some(&c) =
                                        ct.iter().min_by_key(|&&c| (c as i32 - n as i32).abs())
                                    {
                                        n = c;
                                    }
                                }
                            }
                            w.push(n, &pcs);
                            keys.push(n);
                        }
                    }
                    None => {
                        let mut mine = vec![];
                        for &i in &idx {
                            let u = (times[i] - start) as f32 / span;
                            let n = w.next(&mut pr, times[i], u);
                            mine.push(n);
                            keys.push(n);
                        }
                        if !first.iter().any(|f| f.0 == l) {
                            first.push((l, mine));
                        }
                    }
                }
            }
        }
    }
    // 3. end on the tonic, held to the end
    if o.end_tonic {
        let last = keys.len() - 1;
        let near = if last > 0 { keys[last - 1] } else { keys[last] };
        if let Some(t) = (lo..=hi)
            .filter(|&n| pc_of(n) == key.root % 12)
            .min_by_key(|&n| (n as i32 - near as i32).abs())
        {
            keys[last] = t;
        }
        if locks.rhythm.is_none() {
            lens[last] = (end - times[last]).max(lens[last]);
        }
    }
    times
        .iter()
        .zip(lens)
        .zip(keys)
        .map(|((&t, len), key)| MNote {
            start: t,
            len,
            key,
            vel: (0.6 + 0.25 * strength(t) + vr.f() * 0.08).min(1.0),
        })
        .collect()
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

    /// C major I-vi-IV-V, A minor i-VI-III-VII (both diatonic), 2 bars each
    fn cases() -> Vec<(Key, Vec<ChordMark>)> {
        let c = Key::new(0, Scale::Major);
        let a = Key::new(9, Scale::Minor);
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
        vec![(c, prog(c, &cp, 2)), (a, prog(a, &ap, 2))]
    }

    fn opts_grid() -> Vec<MelOpts> {
        let mut v = vec![];
        for (i, r) in Rhythm::ALL.iter().enumerate() {
            for (j, c) in Contour::ALL.iter().enumerate() {
                for m in Motif::ALL {
                    v.push(MelOpts {
                        seed: (i * 31 + j * 7) as u64 + 3,
                        rhythm: *r,
                        contour: *c,
                        motif: m,
                        density: 0.2 + 0.15 * j as f32,
                        leap: 0.1 * i as f32,
                        range: [12, 19, 24, 36][(i + j) % 4],
                        ..MelOpts::default()
                    });
                }
            }
        }
        v
    }

    fn gen(key: Key, marks: &[ChordMark], o: &MelOpts) -> Vec<MNote> {
        generate(key, marks, 0, 8 * BAR, o, Locks::default())
    }

    #[test]
    fn euclid_matches_toussaint() {
        assert_eq!(euclid_str(&euclid(3, 8, 0)), "x..x..x.");
        // the Cuban cinquillo
        assert_eq!(euclid_str(&euclid(5, 8, 0)), "x.xx.xx.");
        assert_eq!(euclid_str(&euclid(2, 5, 0)), "x.x..");
        assert_eq!(euclid_str(&euclid(3, 4, 0)), "xxx.");
        assert_eq!(euclid_str(&euclid(4, 12, 0)), "x..x..x..x..");
        assert_eq!(euclid_str(&euclid(5, 16, 0)), "x..x..x..x..x...");
        assert_eq!(euclid_str(&euclid(3, 8, 1)), "..x..x.x");
        assert_eq!(euclid_str(&euclid(0, 4, 0)), "....");
        assert_eq!(euclid_str(&euclid(9, 4, 0)), "xxxx");
        for k in 0..=16u8 {
            assert_eq!(euclid(k, 16, 3).iter().filter(|&&b| b).count(), k as usize);
        }
    }

    #[test]
    fn same_seed_same_melody_other_seed_differs() {
        for (key, marks) in cases() {
            for o in opts_grid() {
                let a = gen(key, &marks, &o);
                assert_eq!(a, gen(key, &marks, &o));
                assert!(!a.is_empty());
            }
            let o = MelOpts::default();
            let a = gen(key, &marks, &o);
            let differ = (2..12)
                .filter(|s| gen(key, &marks, &MelOpts { seed: *s, ..o }) != a)
                .count();
            assert_eq!(differ, 10, "every other seed gives another melody");
        }
    }

    #[test]
    fn notes_in_scale_and_range_and_end_on_tonic() {
        for (key, marks) in cases() {
            for o in opts_grid() {
                let (lo, hi) = bounds(key, &o);
                assert_eq!(hi - lo, o.range);
                let m = gen(key, &marks, &o);
                for n in &m {
                    assert!(key.contains(pc_of(n.key)), "{:?} {} out of scale", o, n.key);
                    assert!(n.key >= lo && n.key <= hi, "{} outside {lo}..{hi}", n.key);
                    assert!(n.start >= 0 && n.start < 8 * BAR && n.len > 0);
                }
                assert_eq!(pc_of(m.last().unwrap().key), key.root, "ends on the tonic");
                assert!(m.windows(2).all(|w| w[0].start < w[1].start));
            }
        }
    }

    #[test]
    fn strong_beats_are_chord_tones_at_full_bias() {
        for (key, marks) in cases() {
            for o in opts_grid() {
                let o = MelOpts {
                    chord_bias: 1.0,
                    motif: Motif::Off,
                    end_tonic: false,
                    ..o
                };
                let m = gen(key, &marks, &o);
                let mut strong_n = 0;
                for n in m.iter().filter(|n| strong(n.start)) {
                    let pcs = chord_at(key, &marks, n.start);
                    assert!(
                        pcs.contains(&pc_of(n.key)),
                        "{o:?}: {} at {}",
                        n.key,
                        n.start
                    );
                    strong_n += 1;
                }
                assert!(strong_n > 0);
            }
        }
    }

    #[test]
    fn motif_repeats_are_exact_transpositions_at_variation_0() {
        for (key, marks) in cases() {
            for seed in 1..20u64 {
                for motif in [Motif::Aaba, Motif::Abab] {
                    let o = MelOpts {
                        seed,
                        motif,
                        variation: 0.0,
                        end_tonic: false,
                        ..MelOpts::default()
                    };
                    let m = gen(key, &marks, &o);
                    let sec = |k: Tick| -> Vec<MNote> {
                        m.iter()
                            .filter(|n| n.start >= k * 2 * BAR && n.start < (k + 1) * 2 * BAR)
                            .copied()
                            .collect()
                    };
                    let a = sec(0);
                    let again: &[Tick] = if motif == Motif::Aaba { &[1, 3] } else { &[2] };
                    for &k in again {
                        let r = sec(k);
                        assert_eq!(a.len(), r.len(), "same rhythm");
                        let shift = r[0].key as i32 - a[0].key as i32;
                        for (x, y) in a.iter().zip(&r) {
                            assert_eq!(y.start - x.start, k * 2 * BAR);
                            assert_eq!(y.len, x.len);
                            assert_eq!(y.key as i32 - x.key as i32, shift, "exact transposition");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn locks_keep_rhythm_or_notes() {
        let (key, marks) = cases().remove(0);
        let o = MelOpts::default();
        let a = gen(key, &marks, &o);
        let r: Vec<(Tick, Tick)> = a.iter().map(|n| (n.start, n.len)).collect();
        let p: Vec<u8> = a.iter().map(|n| n.key).collect();
        let o2 = MelOpts { seed: 99, ..o };
        let lr = Locks {
            rhythm: Some(&r),
            pitches: None,
        };
        let b = generate(key, &marks, 0, 8 * BAR, &o2, lr);
        assert_eq!(b.iter().map(|n| (n.start, n.len)).collect::<Vec<_>>(), r);
        let lp = Locks {
            rhythm: None,
            pitches: Some(&p),
        };
        let c = generate(key, &marks, 0, 8 * BAR, &o2, lp);
        let ck: Vec<u8> = c.iter().map(|n| n.key).collect();
        let n = ck.len().min(p.len()) - 1;
        assert_eq!(ck[..n], p[..n], "the notes in order");
    }

    #[test]
    fn follows_generated_progressions_without_panicking() {
        let high = MelOpts {
            rhythm: Rhythm::Euclid,
            octave: 9,
            range: 36,
            ..MelOpts::default()
        };
        for style in 0..cg::STYLES.len() {
            for seed in 0..4 {
                let g = cg::generate(&cg::GenOpts {
                    style,
                    seed,
                    ..Default::default()
                });
                let end = g.marks.iter().map(|m| m.end()).max().unwrap();
                for o in [MelOpts::default(), high] {
                    let m = generate(g.key, &g.marks, 0, end, &o, Locks::default());
                    assert!(!m.is_empty());
                    assert!(m.iter().all(|n| (1..=120).contains(&n.key)));
                }
            }
        }
        let blues = Key::new(0, Scale::Blues);
        let d = MelOpts::default();
        assert!(generate(blues, &[], 0, 0, &d, Locks::default()).is_empty());
        assert!(!generate(blues, &[], 0, BAR, &d, Locks::default()).is_empty());
    }
}
