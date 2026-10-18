//! Sample editing ops and loop playback maths (pure functions, unit tested).
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoopMode {
    #[default]
    Off,
    Forward,
    PingPong,
}

impl LoopMode {
    pub const ALL: [LoopMode; 3] = [LoopMode::Off, LoopMode::Forward, LoopMode::PingPong];
    pub fn name(self) -> &'static str {
        match self {
            LoopMode::Off => "Off",
            LoopMode::Forward => "Forward",
            LoopMode::PingPong => "Ping-pong",
        }
    }
}

/// Loop settings of a sample instrument (frames into `data`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SampleLoop {
    #[serde(default)]
    pub mode: LoopMode,
    #[serde(default)]
    pub start: usize,
    #[serde(default)]
    pub end: usize,
    /// cents, -100..=100
    #[serde(default)]
    pub finetune: i8,
}

impl SampleLoop {
    /// clamp to a sample of `len` frames; an empty / invalid loop turns off
    pub fn fix(&mut self, len: usize) {
        self.finetune = self.finetune.clamp(-100, 100);
        self.end = self.end.min(len);
        self.start = self.start.min(self.end);
        if self.end - self.start < 2 && self.mode != LoopMode::Off {
            self.mode = LoopMode::Off;
        }
    }
}

/// Map a monotonic play position (frames since note start) to a read position
/// in the data. `None` = past the end (voice finished).
pub fn read_pos(pos: f64, len: usize, lp: &SampleLoop) -> Option<f64> {
    let (s, e) = (lp.start as f64, lp.end.min(len) as f64);
    let looping = lp.mode != LoopMode::Off && e - s >= 2.0;
    if !looping || pos < e {
        return if pos + 1.0 < len as f64 || (looping && pos < e) {
            Some(pos)
        } else {
            None
        };
    }
    let l = e - s;
    Some(match lp.mode {
        LoopMode::Forward => s + (pos - s) % l,
        _ => {
            // ping-pong: forward s..e, then back e..s
            let u = (pos - s) % (2.0 * l);
            if u < l {
                s + u
            } else {
                e - (u - l)
            }
        }
    })
}

/// linear interpolation read; at the loop end, wraps to the loop start
pub fn read(data: &[f32], p: f64, lp: &SampleLoop) -> f32 {
    let i = p as usize;
    if i >= data.len() {
        return 0.0;
    }
    let fr = (p - i as f64) as f32;
    let mut j = i + 1;
    if lp.mode == LoopMode::Forward && j >= lp.end && lp.end > lp.start {
        j = lp.start;
    }
    let b = data.get(j).copied().unwrap_or(data[i]);
    data[i] * (1.0 - fr) + b * fr
}

/// Normalised half-open range inside `len`; an empty selection means "all".
pub fn range(sel: Option<(usize, usize)>, len: usize) -> (usize, usize) {
    match sel {
        Some((a, b)) if a.min(b) < b.max(a) => (a.min(b).min(len), a.max(b).min(len)),
        _ => (0, len),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// keep only the selection
    Crop,
    /// remove the selection
    Delete,
    /// strip leading / trailing silence (below -60 dBFS)
    TrimSilence,
    Normalise,
    Reverse,
    FadeIn,
    FadeOut,
    /// gain in dB
    Gain(f32),
}

impl Op {
    pub fn name(self) -> &'static str {
        match self {
            Op::Crop => "Crop",
            Op::Delete => "Delete",
            Op::TrimSilence => "Trim silence",
            Op::Normalise => "Normalise",
            Op::Reverse => "Reverse",
            Op::FadeIn => "Fade in",
            Op::FadeOut => "Fade out",
            Op::Gain(_) => "Gain",
        }
    }
}

/// Apply an op to `data` over the selection (all when None). Loop points are
/// moved with the audio. Returns the new selection.
pub fn apply(
    data: &mut Vec<f32>,
    lp: &mut SampleLoop,
    sel: Option<(usize, usize)>,
    op: Op,
) -> Option<(usize, usize)> {
    let len = data.len();
    let (a, b) = range(sel, len);
    let out = match op {
        Op::Crop => {
            data.truncate(b);
            data.drain(..a);
            lp.start = lp.start.saturating_sub(a);
            lp.end = lp.end.saturating_sub(a);
            None
        }
        Op::Delete => {
            if a == 0 && b == len {
                return sel; // never delete everything
            }
            data.drain(a..b);
            let n = b - a;
            let shift = |p: usize| {
                if p >= b {
                    p - n
                } else if p > a {
                    a
                } else {
                    p
                }
            };
            lp.start = shift(lp.start);
            lp.end = shift(lp.end);
            Some((a, a))
        }
        Op::TrimSilence => {
            let thr = 0.001;
            let s = data.iter().position(|x| x.abs() > thr).unwrap_or(0);
            let e = data
                .iter()
                .rposition(|x| x.abs() > thr)
                .map(|e| e + 1)
                .unwrap_or(len);
            if e > s {
                data.truncate(e);
                data.drain(..s);
                lp.start = lp.start.saturating_sub(s);
                lp.end = lp.end.saturating_sub(s);
            }
            None
        }
        Op::Normalise => {
            let pk = data[a..b].iter().fold(0.0f32, |m, x| m.max(x.abs()));
            if pk > 1e-6 {
                let g = 0.999 / pk;
                data[a..b].iter_mut().for_each(|x| *x *= g);
            }
            sel
        }
        Op::Reverse => {
            data[a..b].reverse();
            sel
        }
        Op::FadeIn | Op::FadeOut => {
            let n = (b - a).max(1) as f32;
            for (k, x) in data[a..b].iter_mut().enumerate() {
                let t = k as f32 / n;
                *x *= if op == Op::FadeIn { t } else { 1.0 - t };
            }
            sel
        }
        Op::Gain(db) => {
            let g = 10f32.powf(db / 20.0);
            data[a..b]
                .iter_mut()
                .for_each(|x| *x = (*x * g).clamp(-4.0, 4.0));
            sel
        }
    };
    lp.fix(data.len());
    out
}

// ------------------------------------------------------------ slicing (0.2.3.10)

/// most slices a sample is cut into
pub const MAX_SLICES: usize = 64;

/// nearest zero crossing to `p` within `radius` frames (else `p`)
pub fn snap_zero(data: &[f32], p: usize, radius: usize) -> usize {
    if p == 0 || p >= data.len() {
        return p.min(data.len());
    }
    let lo = p.saturating_sub(radius).max(1);
    let hi = (p + radius).min(data.len() - 1);
    (lo..=hi)
        .filter(|&i| (data[i - 1] <= 0.0) != (data[i] <= 0.0) || data[i] == 0.0)
        .min_by_key(|&i| i.abs_diff(p))
        .unwrap_or(p)
}

/// `n` equal slices (start frames, the first is 0), snapped to zero crossings
pub fn grid_slices(data: &[f32], n: usize) -> Vec<usize> {
    let n = n.clamp(1, MAX_SLICES);
    let len = data.len();
    let mut v: Vec<usize> = (0..n).map(|k| snap_zero(data, k * len / n, 64)).collect();
    v.dedup();
    v
}

/// Slice starts at transients (onsets). `sensitivity` 0..1 (1 = finds the
/// softest hits), `min_gap_ms` keeps slices apart. Energy-rise detector:
/// 5 ms frames, the level jump in dB against the previous 15 ms, a floor
/// 40 dB below the loudest frame, then the onset is moved to the zero
/// crossing just before the attack. The first slice is always 0.
pub fn transient_slices(data: &[f32], rate: u32, sensitivity: f32, min_gap_ms: f32) -> Vec<usize> {
    let hop = (rate as usize / 200).max(16);
    let n = data.len() / hop;
    let mut out = vec![0usize];
    if n < 4 {
        return out;
    }
    let lvl: Vec<f32> = (0..n)
        .map(|i| {
            let a = i * hop;
            let b = (a + hop * 2).min(data.len());
            let e = data[a..b].iter().map(|x| x * x).sum::<f32>() / (b - a).max(1) as f32;
            10.0 * (e + 1e-10).log10()
        })
        .collect();
    let top = lvl.iter().cloned().fold(f32::MIN, f32::max);
    let thr = 12.0 - 9.0 * sensitivity.clamp(0.0, 1.0);
    let gap = ((min_gap_ms.max(5.0) / 1000.0) * rate as f32) as usize;
    let rise = |i: usize| -> f32 {
        let prev = (i.saturating_sub(3)..i)
            .map(|j| lvl[j])
            .fold(f32::MIN, f32::max);
        lvl[i] - prev
    };
    #[allow(clippy::needless_range_loop)]
    for i in 1..n {
        if out.len() >= MAX_SLICES {
            break;
        }
        let r = rise(i);
        if r < thr || lvl[i] < top - 40.0 {
            continue;
        }
        // the local maximum of the rise
        if (i + 1 < n && rise(i + 1) > r) || rise(i - 1) > r {
            continue;
        }
        // attack start: the first sample above 25 % of the next 10 ms peak
        let a = (i * hop).saturating_sub(hop);
        let b = (i * hop + hop * 2).min(data.len());
        let pk = data[a..b].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let first = data[a..b]
            .iter()
            .position(|x| x.abs() > pk * 0.25)
            .map(|k| a + k)
            .unwrap_or(i * hop);
        // back to the zero crossing before it
        let mut p = first;
        while p > a && data[p - 1] != 0.0 && (data[p - 1] < 0.0) == (data[p] < 0.0) {
            p -= 1;
        }
        if p >= out.last().copied().unwrap_or(0) + gap {
            out.push(p);
        }
    }
    out
}

/// (start, end) of each slice of a `len`-frame sample
pub fn slice_bounds(slices: &[usize], len: usize) -> Vec<(usize, usize)> {
    let mut v: Vec<usize> = slices.iter().copied().filter(|&p| p < len).collect();
    if v.first() != Some(&0) {
        v.insert(0, 0);
    }
    v.sort_unstable();
    v.dedup();
    v.iter()
        .enumerate()
        .map(|(k, &a)| (a, v.get(k + 1).copied().unwrap_or(len)))
        .filter(|(a, b)| b > a)
        .collect()
}

/// A synthesised 2-bar breakbeat at 120 BPM (kick, snare, hats): used by
/// the slicing screenshot and the features demo. Nothing sampled.
pub fn demo_break(rate: u32) -> Vec<f32> {
    let sr = rate as f32;
    let step = (sr * 0.125) as usize; // 16ths at 120 BPM
    let mut d = vec![0.0f32; step * 32];
    let mut n = 0x1234_5678u32;
    let mut rnd = move || {
        n ^= n << 13;
        n ^= n >> 17;
        n ^= n << 5;
        n as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    // per bar: K = kick, S = snare, h = hat, g = ghost snare
    let bars = ["K.h.S.hKh.KhS.hg", "K.h.S.h.hKK.S.hS"];
    for (b, pat) in bars.iter().enumerate() {
        for (i, c) in pat.chars().enumerate() {
            let at = (b * 16 + i) * step;
            let len = (step * 4).min(d.len() - at);
            let mut ph = 0.0f32;
            let mut hp = 0.0f32;
            for k in 0..len {
                let t = k as f32 / sr;
                let v = match c {
                    'K' => {
                        let f = 50.0 + 110.0 * (-t * 28.0).exp();
                        ph += std::f32::consts::TAU * f / sr;
                        ph.sin() * (-t * 9.0).exp() * 0.9
                    }
                    'S' | 'g' => {
                        ph += std::f32::consts::TAU * 185.0 / sr;
                        let amp = if c == 'S' { 0.6 } else { 0.18 };
                        (rnd() * 0.8 + ph.sin() * 0.5) * (-t * 22.0).exp() * amp
                    }
                    'h' => {
                        let x = rnd();
                        let y = x - hp;
                        hp = x;
                        y * (-t * 70.0).exp() * 0.18
                    }
                    _ => 0.0,
                };
                d[at + k] += v;
            }
        }
    }
    d
}

/// copy frames `a..b` with short declick fades (1 ms in, 3 ms out)
pub fn extract(data: &[f32], a: usize, b: usize, rate: u32) -> Vec<f32> {
    let b = b.min(data.len());
    let mut v = data[a.min(b)..b].to_vec();
    let fi = (rate as usize / 1000).min(v.len() / 4);
    let fo = (rate as usize * 3 / 1000).min(v.len() / 4);
    for (k, x) in v.iter_mut().take(fi).enumerate() {
        *x *= k as f32 / fi as f32;
    }
    let n = v.len();
    for k in 0..fo {
        v[n - 1 - k] *= k as f32 / fo as f32;
    }
    v
}

/// Min/max peaks for `cols` columns covering frames `from..to`.
pub fn peaks(data: &[f32], from: f64, to: f64, cols: usize) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(cols);
    let span = (to - from).max(1e-9) / cols.max(1) as f64;
    for c in 0..cols {
        let a = (from + c as f64 * span).max(0.0) as usize;
        let b = ((from + (c + 1) as f64 * span).ceil() as usize)
            .max(a + 1)
            .min(data.len());
        if a >= data.len() {
            out.push((0.0, 0.0));
            continue;
        }
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for &x in &data[a..b] {
            lo = lo.min(x);
            hi = hi.max(x);
        }
        out.push((lo, hi));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lp(mode: LoopMode, s: usize, e: usize) -> SampleLoop {
        SampleLoop {
            mode,
            start: s,
            end: e,
            finetune: 0,
        }
    }

    #[test]
    fn loop_off_ends() {
        let l = lp(LoopMode::Off, 0, 0);
        assert_eq!(read_pos(5.0, 10, &l), Some(5.0));
        assert_eq!(read_pos(9.5, 10, &l), None);
    }

    #[test]
    fn loop_forward_wraps() {
        let l = lp(LoopMode::Forward, 4, 8);
        assert_eq!(read_pos(3.0, 10, &l), Some(3.0));
        assert_eq!(read_pos(8.0, 10, &l), Some(4.0));
        assert_eq!(read_pos(13.0, 10, &l), Some(5.0));
        assert!(read_pos(1e6, 10, &l).is_some(), "never ends");
    }

    #[test]
    fn loop_pingpong_bounces() {
        let l = lp(LoopMode::PingPong, 4, 8);
        assert_eq!(read_pos(9.0, 10, &l), Some(7.0)); // going back
        assert_eq!(read_pos(12.0, 10, &l), Some(4.0)); // back at start
        assert_eq!(read_pos(13.0, 10, &l), Some(5.0)); // forward again
        for k in 0..1000 {
            let p = read_pos(k as f64 * 0.37, 10, &l).unwrap();
            assert!((0.0..=8.0).contains(&p));
        }
    }

    #[test]
    fn ops_work_and_keep_loop_valid() {
        let base: Vec<f32> = (0..100).map(|i| (i as f32 / 100.0) - 0.25).collect();
        let mut l = lp(LoopMode::Forward, 20, 80);
        let mut d = base.clone();
        apply(&mut d, &mut l, Some((10, 60)), Op::Crop);
        assert_eq!(d.len(), 50);
        assert_eq!((l.start, l.end), (10, 50));
        let mut d = base.clone();
        let mut l = lp(LoopMode::Forward, 20, 80);
        apply(&mut d, &mut l, Some((0, 30)), Op::Delete);
        assert_eq!(d.len(), 70);
        assert_eq!((l.start, l.end), (0, 50));
        let mut d = base.clone();
        apply(&mut d, &mut l, None, Op::Normalise);
        assert!((d.iter().fold(0.0f32, |m, x| m.max(x.abs())) - 0.999).abs() < 1e-4);
        let mut d = base.clone();
        apply(&mut d, &mut l, None, Op::Reverse);
        assert_eq!(d[0], base[99]);
        let mut d = vec![1.0; 10];
        apply(&mut d, &mut l, None, Op::FadeIn);
        assert_eq!(d[0], 0.0);
        assert!(d[9] > 0.8);
        let mut d = vec![1.0; 10];
        apply(&mut d, &mut l, None, Op::FadeOut);
        assert_eq!(d[0], 1.0);
        assert!(d[9] < 0.2);
        let mut d = vec![0.5; 4];
        apply(&mut d, &mut l, None, Op::Gain(6.0206));
        assert!((d[0] - 1.0).abs() < 1e-3);
        let mut d = vec![0.0, 0.0, 0.5, -0.5, 0.0];
        apply(&mut d, &mut l, None, Op::TrimSilence);
        assert_eq!(d, vec![0.5, -0.5]);
        assert!(l.end <= d.len() && l.start <= l.end);
        // deleting everything is refused
        let mut d = vec![0.3; 8];
        apply(&mut d, &mut l, None, Op::Delete);
        assert_eq!(d.len(), 8);
    }

    #[test]
    fn peaks_cover_minmax() {
        let d: Vec<f32> = (0..1000).map(|i| ((i as f32) * 0.1).sin()).collect();
        let p = peaks(&d, 0.0, 1000.0, 10);
        assert_eq!(p.len(), 10);
        assert!(p.iter().all(|(lo, hi)| *lo < -0.9 && *hi > 0.9));
    }

    // ---- slicing
    /// four decaying noise-ish hits at known frames on a quiet bed
    fn hits(at: &[usize], len: usize) -> Vec<f32> {
        let mut v: Vec<f32> = (0..len).map(|i| ((i * 7919) % 13) as f32 * 1e-5).collect();
        let mut n = 0x1234_5678u32;
        for &a in at {
            for k in 0..4000.min(len - a) {
                n ^= n << 13;
                n ^= n >> 17;
                n ^= n << 5;
                let r = n as f32 / u32::MAX as f32 * 2.0 - 1.0;
                v[a + k] += r * 0.8 * (-(k as f32) / 700.0).exp();
            }
        }
        v
    }

    #[test]
    fn transient_slicing_finds_the_hits() {
        let at = [0, 12000, 24000, 30000, 42000];
        let d = hits(&at, 48000);
        let s = transient_slices(&d, 48000, 0.5, 30.0);
        assert_eq!(s.len(), at.len(), "{s:?}");
        for (got, want) in s.iter().zip(at) {
            assert!(got.abs_diff(want) < 200, "{got} vs {want}");
        }
        // low sensitivity + a long gap merge close hits
        let s2 = transient_slices(&d, 48000, 0.5, 200.0);
        assert_eq!(s2.len(), 4, "30000 is within 200 ms of 24000: {s2:?}");
        // silence: only the start
        assert_eq!(transient_slices(&vec![0.0; 48000], 48000, 1.0, 30.0), [0]);
    }

    #[test]
    fn grid_bounds_and_extract() {
        let d: Vec<f32> = (0..8000).map(|i| (i as f32 * 0.05).sin()).collect();
        let g = grid_slices(&d, 4);
        assert_eq!(g.len(), 4);
        assert_eq!(g[0], 0);
        for (k, p) in g.iter().enumerate().skip(1) {
            assert!(p.abs_diff(k * 2000) <= 64, "snapped near the grid");
        }
        let b = slice_bounds(&g, d.len());
        assert_eq!(b.len(), 4);
        assert_eq!(b[3].1, 8000);
        assert!(b.windows(2).all(|w| w[0].1 == w[1].0), "contiguous");
        let x = extract(&d, b[1].0, b[1].1, 48000);
        assert_eq!(x.len(), b[1].1 - b[1].0);
        assert_eq!(x[0], 0.0);
        assert!(x[x.len() - 1].abs() < 1e-6, "faded out");
        assert_eq!(
            slice_bounds(&[5000, 100, 100], 8000),
            [(0, 100), (100, 5000), (5000, 8000)]
        );
    }
}
