//! Automation lanes: breakpoints over song position (lines), evaluated by the
//! engine while playing (and in WAV export). See docs/AUTOMATION.md.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    #[default]
    Linear,
    Step,
    Smooth,
}

impl Shape {
    pub const ALL: [Shape; 3] = [Shape::Linear, Shape::Step, Shape::Smooth];
    pub fn name(self) -> &'static str {
        match self {
            Shape::Linear => "Linear",
            Shape::Step => "Step",
            Shape::Smooth => "Smooth",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// song position in lines (rows), fractional allowed
    pub pos: f32,
    /// normalised 0..=1
    pub value: f32,
    /// shape of the segment from this point to the next
    #[serde(default)]
    pub shape: Shape,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lane {
    /// target id, e.g. "instrument.2.cutoff", "mixer.ch.1.volume", "mixer.master", "tempo"
    pub target: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub points: Vec<Point>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

impl Lane {
    pub fn new(target: Target, label: String) -> Self {
        Self {
            target: target.id(),
            label,
            points: vec![],
            enabled: true,
        }
    }
    pub fn sort(&mut self) {
        self.points.sort_by(|a, b| {
            a.pos
                .partial_cmp(&b.pos)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// Normalised value at `pos`: holds the first/last value outside the points.
pub fn value_at(points: &[Point], pos: f32) -> Option<f32> {
    let first = points.first()?;
    if pos <= first.pos {
        return Some(first.value);
    }
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        if pos < b.pos {
            let span = (b.pos - a.pos).max(1e-6);
            let t = ((pos - a.pos) / span).clamp(0.0, 1.0);
            let t = match a.shape {
                Shape::Linear => t,
                Shape::Step => 0.0,
                Shape::Smooth => t * t * (3.0 - 2.0 * t),
            };
            return Some(a.value + (b.value - a.value) * t);
        }
    }
    points.last().map(|p| p.value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// instrument index (0-based)
    Cutoff(usize),
    Resonance(usize),
    Drive(usize),
    Level(usize),
    ChanVolume(usize),
    ChanPan(usize),
    Master,
    Tempo,
    /// instrument, device, param index (normalised value = device param)
    Fx(usize, usize, usize),
    /// return bus (0 = A, 1 = B), param index
    Ret(usize, usize),
    /// channel, return bus
    Send(usize, usize),
}

impl Target {
    pub fn parse(s: &str) -> Option<Target> {
        let p: Vec<&str> = s.split('.').collect();
        let num = |x: &str| x.parse::<usize>().ok().filter(|n| *n >= 1).map(|n| n - 1);
        match p.as_slice() {
            ["tempo"] => Some(Target::Tempo),
            ["mixer", "master"] | ["mixer", "master", "volume"] => Some(Target::Master),
            ["mixer", "ch", n, "volume"] => num(n).map(Target::ChanVolume),
            ["mixer", "ch", n, "pan"] => num(n).map(Target::ChanPan),
            ["mixer", "ch", n, s] if s.starts_with("send_") => {
                let r = match *s {
                    "send_a" => 0,
                    "send_b" => 1,
                    _ => return None,
                };
                num(n).map(|c| Target::Send(c, r))
            }
            ["mixer", r, key] if r.starts_with("return_") => {
                let ri = match *r {
                    "return_a" => 0,
                    "return_b" => 1,
                    _ => return None,
                };
                let kind = if ri == 0 {
                    crate::fx::FxKind::Delay
                } else {
                    crate::fx::FxKind::Reverb
                };
                param_ix(kind, key).map(|p| Target::Ret(ri, p))
            }
            ["instrument", n, "fx", d, key] => {
                let i = num(n)?;
                let d = num(d)?;
                let p = key.strip_prefix('p').and_then(|x| x.parse().ok())?;
                Some(Target::Fx(i, d, p))
            }
            ["instrument", n, rest @ ..] => {
                let i = num(n)?;
                match rest.last().copied()? {
                    "cutoff" => Some(Target::Cutoff(i)),
                    "resonance" => Some(Target::Resonance(i)),
                    "drive" => Some(Target::Drive(i)),
                    "level" => Some(Target::Level(i)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn id(self) -> String {
        match self {
            Target::Cutoff(i) => format!("instrument.{}.cutoff", i + 1),
            Target::Resonance(i) => format!("instrument.{}.resonance", i + 1),
            Target::Drive(i) => format!("instrument.{}.drive", i + 1),
            Target::Level(i) => format!("instrument.{}.level", i + 1),
            Target::ChanVolume(c) => format!("mixer.ch.{}.volume", c + 1),
            Target::ChanPan(c) => format!("mixer.ch.{}.pan", c + 1),
            Target::Master => "mixer.master".into(),
            Target::Tempo => "tempo".into(),
            Target::Fx(i, d, p) => format!("instrument.{}.fx.{}.p{}", i + 1, d + 1, p),
            Target::Ret(r, p) => {
                let kind = if r == 0 {
                    crate::fx::FxKind::Delay
                } else {
                    crate::fx::FxKind::Reverb
                };
                format!(
                    "mixer.return_{}.{}",
                    if r == 0 { "a" } else { "b" },
                    crate::fx::params(kind)[p].key
                )
            }
            Target::Send(c, r) => {
                format!("mixer.ch.{}.send_{}", c + 1, if r == 0 { "a" } else { "b" })
            }
        }
    }

    /// Parse with the song, so `instrument.N.fx.D.<key>` (named param) resolves.
    pub fn parse_in(s: &str, song: &crate::model::Song) -> Option<Target> {
        if let Some(t) = Target::parse(s) {
            return Some(t);
        }
        let p: Vec<&str> = s.split('.').collect();
        if let ["instrument", n, "fx", d, key] = p.as_slice() {
            let i = n.parse::<usize>().ok()?.checked_sub(1)?;
            let d = d.parse::<usize>().ok()?.checked_sub(1)?;
            let kind = song.instruments.get(i)?.fx.get(d)?.kind;
            return param_ix(kind, key).map(|p| Target::Fx(i, d, p));
        }
        None
    }

    /// value text; FX params use their own units (`kinds` = FX kinds per instrument)
    pub fn format_in(self, n: f32, kinds: &[Vec<crate::fx::FxKind>]) -> String {
        use crate::fx::FxKind;
        let info = |k: FxKind, p: usize| crate::fx::params(k).get(p).map(|i| i.format(n));
        match self {
            Target::Fx(i, d, p) => kinds
                .get(i)
                .and_then(|x| x.get(d))
                .and_then(|&k| info(k, p))
                .unwrap_or_default(),
            Target::Ret(r, p) => info(
                if r == 0 {
                    FxKind::Delay
                } else {
                    FxKind::Reverb
                },
                p,
            )
            .unwrap_or_default(),
            Target::Send(..) => format!("{:.0} %", n * 100.0),
            _ => self.format(n),
        }
    }

    /// (min, max, logarithmic)
    fn range(self) -> (f32, f32, bool) {
        match self {
            Target::Fx(..) | Target::Ret(..) | Target::Send(..) => (0.0, 1.0, false),
            Target::Cutoff(_) => (20.0, 18000.0, true),
            Target::Resonance(_) => (0.0, 0.98, false),
            Target::Drive(_) => (0.0, 1.0, false),
            Target::Level(_) => (0.0, 1.5, false),
            Target::ChanVolume(_) | Target::Master => (0.0, 2.0, false),
            Target::ChanPan(_) => (-1.0, 1.0, false),
            Target::Tempo => (40.0, 300.0, false),
        }
    }

    pub fn to_real(self, n: f32) -> f32 {
        let (lo, hi, log) = self.range();
        let n = n.clamp(0.0, 1.0);
        if log {
            lo * (hi / lo).powf(n)
        } else {
            lo + (hi - lo) * n
        }
    }

    pub fn to_norm(self, v: f32) -> f32 {
        let (lo, hi, log) = self.range();
        let v = v.clamp(lo, hi);
        if log {
            (v / lo).ln() / (hi / lo).ln()
        } else {
            (v - lo) / (hi - lo)
        }
    }

    pub fn param_name(self) -> &'static str {
        match self {
            Target::Cutoff(_) => "Filter cutoff",
            Target::Resonance(_) => "Resonance",
            Target::Drive(_) => "Drive",
            Target::Level(_) => "Amp level",
            Target::ChanVolume(_) => "Volume",
            Target::ChanPan(_) => "Pan",
            Target::Master => "Master volume",
            Target::Tempo => "Tempo",
            Target::Fx(..) | Target::Ret(..) => "FX param",
            Target::Send(..) => "Send",
        }
    }

    pub fn format(self, n: f32) -> String {
        let v = self.to_real(n);
        match self {
            Target::Cutoff(_) if v >= 1000.0 => format!("{:.2} kHz", v / 1000.0),
            Target::Cutoff(_) => format!("{v:.0} Hz"),
            Target::ChanVolume(_) | Target::Master | Target::Level(_) => crate::model::db_text(v),
            Target::ChanPan(_) if v.abs() < 0.01 => "C".into(),
            Target::ChanPan(_) if v < 0.0 => format!("L{:.0}", -v * 100.0),
            Target::ChanPan(_) => format!("R{:.0}", v * 100.0),
            Target::Tempo => format!("{v:.1} BPM"),
            _ => format!("{:.0} %", n * 100.0),
        }
    }
}

fn param_ix(kind: crate::fx::FxKind, key: &str) -> Option<usize> {
    crate::fx::Device::param_index(kind, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pos: f32, value: f32, shape: Shape) -> Point {
        Point { pos, value, shape }
    }

    #[test]
    fn interpolation_shapes() {
        let lin = [p(0.0, 0.0, Shape::Linear), p(10.0, 1.0, Shape::Linear)];
        assert_eq!(value_at(&lin, -5.0), Some(0.0));
        assert!((value_at(&lin, 5.0).unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(value_at(&lin, 20.0), Some(1.0));
        let step = [p(0.0, 0.2, Shape::Step), p(10.0, 0.8, Shape::Step)];
        assert_eq!(value_at(&step, 9.9), Some(0.2));
        assert_eq!(value_at(&step, 10.0), Some(0.8));
        let sm = [p(0.0, 0.0, Shape::Smooth), p(10.0, 1.0, Shape::Linear)];
        let a = value_at(&sm, 2.5).unwrap();
        assert!(a < 0.25 && a > 0.1, "{a}");
        assert!((value_at(&sm, 5.0).unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(value_at(&[], 1.0), None);
    }

    #[test]
    fn targets_roundtrip_and_units() {
        for t in [
            Target::Cutoff(1),
            Target::Resonance(0),
            Target::Drive(3),
            Target::Level(2),
            Target::ChanVolume(0),
            Target::ChanPan(3),
            Target::Master,
            Target::Tempo,
        ] {
            assert_eq!(Target::parse(&t.id()), Some(t));
            for n in [0.0, 0.3, 1.0] {
                assert!((t.to_norm(t.to_real(n)) - n).abs() < 1e-4);
            }
        }
        // the demo's id form
        assert_eq!(
            Target::parse("instrument.2.filter1.cutoff"),
            Some(Target::Cutoff(1))
        );
        let c = Target::Cutoff(0);
        assert!((c.to_real(c.to_norm(180.0)) - 180.0).abs() < 0.1);
        assert_eq!(c.format(c.to_norm(2400.0)), "2.40 kHz");
    }
}
