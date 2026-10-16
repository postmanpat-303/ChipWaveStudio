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
}
