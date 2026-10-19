//! Automation lanes: breakpoints over song position (lines), evaluated by the
//! engine while playing (and in WAV export). See docs/AUTOMATION.md.

mod ux;
pub use ux::{
    combine_clip, AutoTool, ClipMode, WriteMode, POINT_HIT, POINT_R, POINT_R_HOT, SCALE_W,
};

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
    /// bend of a Linear segment to the next point, -1..1 (0 = straight;
    /// > 0 rises early, < 0 late). Timeline lanes draw it with Alt-drag.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub curve: f32,
}

fn is_zero(x: &f32) -> bool {
    *x == 0.0
}

/// Bend `t` (0..1) by `curve` (-1..1): a power curve, exactly `t` at 0.
pub fn bend(t: f32, curve: f32) -> f32 {
    if curve == 0.0 {
        return t;
    }
    t.powf(2f32.powf(-2.5 * curve.clamp(-1.0, 1.0)))
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
    /// clip envelope combine mode (track lanes ignore this; Absolute = bit-exact)
    #[serde(default, skip_serializing_if = "ClipMode::is_absolute")]
    pub mode: ClipMode,
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
            mode: ClipMode::Absolute,
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
                Shape::Linear => bend(t, a.curve),
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
    /// plugin instrument, macro 0-7 (0.2.3.16)
    Macro(usize, usize),
    /// generator instrument, knob (`crate::gen::gen_knobs`; 0-based, the id
    /// is 1-based: `instrument.N.gen.K`), normalised 0..1 knob value
    Gen(usize, usize),
    /// synth patch knob of an instrument (Synth / Voice / generator base
    /// patch): instrument, `modulation::PATCH_PARAMS` index; the id is
    /// `instrument.N.patch.<key>` with `.` in the key written as `-`
    Patch(usize, usize),
    /// depth of modulation route K (0-based; id `mod.K.amount`, 1-based),
    /// -1..1 (normalised 0..1 = -100 %..+100 %)
    ModAmount(usize),
}

/// The value a target has in the song, normalised 0..1 (the base that
/// modulation moves around). None for values that live in a plugin (plugin
/// FX parameters, plugin macros) or an unknown target. No allocation.
pub fn song_norm(song: &crate::model::Song, t: Target) -> Option<f32> {
    use crate::model::InstrumentKind;
    Some(match t {
        Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) => {
            let p = song.instruments.get(i)?.kind.patch()?;
            t.to_norm(match t {
                Target::Cutoff(_) => p.cutoff,
                Target::Resonance(_) => p.resonance,
                _ => p.drive,
            })
        }
        Target::Patch(i, k) => {
            let p = song.instruments.get(i)?.kind.patch()?;
            let pp = crate::modulation::PATCH_PARAMS.get(k)?;
            t.to_norm((pp.get)(p))
        }
        Target::Level(_) => t.to_norm(1.0),
        Target::ChanVolume(c) => t.to_norm(song.mixer.channels.get(c)?.volume),
        Target::ChanPan(c) => t.to_norm(song.mixer.channels.get(c)?.pan),
        Target::Master => t.to_norm(song.mixer.master),
        Target::Tempo => t.to_norm(song.bpm),
        Target::Fx(i, d, p) => {
            let dev = song.instruments.get(i)?.fx.get(d)?;
            if dev.plugin.is_some() {
                return None;
            }
            *dev.params.get(p)?
        }
        Target::Ret(r, p) => *song.mixer.returns.get(r)?.device.params.get(p)?,
        Target::Send(c, r) => *song.mixer.channels.get(c)?.send.get(r)?,
        Target::Macro(..) => return None,
        Target::Gen(i, k) => match &song.instruments.get(i)?.kind {
            InstrumentKind::Gen(g) => crate::gen::gen_knob_norm(g, k)?,
            _ => return None,
        },
        Target::ModAmount(k) => t.to_norm(song.modulation.routes.get(k)?.amount),
    })
}

/// can a modulation route drive this target (tempo and route depths can't)
pub fn modulatable(t: Target) -> bool {
    !matches!(t, Target::Tempo | Target::ModAmount(_) | Target::Macro(..))
}

/// per-instrument value units for lane text: FX device kinds and the
/// generator knob definitions (`Target::Gen` index -> MacroDef)
#[derive(Clone, Default)]
pub struct Units {
    pub fx: Vec<crate::fx::FxKind>,
    pub gen: Vec<Option<&'static crate::gen::MacroDef>>,
}

impl Units {
    pub fn gen_of(
        kind: &crate::model::InstrumentKind,
    ) -> Vec<Option<&'static crate::gen::MacroDef>> {
        match kind {
            crate::model::InstrumentKind::Gen(g) => (0..crate::gen::GEN_KNOBS)
                .map(|k| crate::gen::gen_knob_def(g, k))
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl Target {
    pub fn parse(s: &str) -> Option<Target> {
        // no allocation: split into a fixed array (targets have <= 6 parts)
        let mut parts = [""; 8];
        let mut n = 0;
        for x in s.split('.') {
            if n == parts.len() {
                return None;
            }
            parts[n] = x;
            n += 1;
        }
        let p = &parts[..n];
        let num = |x: &str| x.parse::<usize>().ok().filter(|n| *n >= 1).map(|n| n - 1);
        match p {
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
            ["instrument", n, "macro", k] => {
                let i = num(n)?;
                let k = num(k).filter(|k| *k < 8)?;
                Some(Target::Macro(i, k))
            }
            ["instrument", n, "gen", k] => {
                let i = num(n)?;
                let k = num(k).filter(|k| *k < crate::gen::GEN_KNOBS)?;
                Some(Target::Gen(i, k))
            }
            ["mod", k, "amount"] => num(k)
                .filter(|k| *k < crate::modulation::MAX_ROUTES)
                .map(Target::ModAmount),
            ["instrument", n, "patch", key] => {
                let i = num(n)?;
                crate::modulation::patch_param_id(key).map(|k| Target::Patch(i, k))
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
            Target::Macro(i, k) => format!("instrument.{}.macro.{}", i + 1, k + 1),
            Target::Gen(i, k) => format!("instrument.{}.gen.{}", i + 1, k + 1),
            Target::Patch(i, k) => format!(
                "instrument.{}.patch.{}",
                i + 1,
                crate::modulation::PATCH_PARAMS
                    .get(k)
                    .map(|p| p.key.replace('.', "-"))
                    .unwrap_or_default()
            ),
            Target::ModAmount(k) => format!("mod.{}.amount", k + 1),
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

    /// value text; FX params and generator knobs use their own units
    /// (`units` = per instrument)
    pub fn format_in(self, n: f32, units: &[Units]) -> String {
        use crate::fx::FxKind;
        let info = |k: FxKind, p: usize| crate::fx::params(k).get(p).map(|i| i.format(n));
        match self {
            Target::Fx(i, d, p) => match units.get(i).and_then(|x| x.fx.get(d)) {
                // plugin parameters: normalised over the parameter's range
                Some(FxKind::Plugin) => format!("{:.0} %", n * 100.0),
                Some(&k) => info(k, p).unwrap_or_default(),
                None => String::new(),
            },
            Target::Gen(i, k) => match units.get(i).and_then(|u| u.gen.get(k)) {
                Some(Some(d)) => d.show(n),
                _ => format!("{:.0} %", n * 100.0),
            },
            Target::Ret(r, p) => info(
                if r == 0 {
                    FxKind::Delay
                } else {
                    FxKind::Reverb
                },
                p,
            )
            .unwrap_or_default(),
            Target::Send(..) | Target::Macro(..) => format!("{:.0} %", n * 100.0),
            _ => self.format(n),
        }
    }

    /// (min, max, logarithmic)
    fn range(self) -> (f32, f32, bool) {
        match self {
            Target::Fx(..)
            | Target::Ret(..)
            | Target::Send(..)
            | Target::Macro(..)
            | Target::Gen(..)
            | Target::Patch(..) => (0.0, 1.0, false),
            Target::ModAmount(_) => (-1.0, 1.0, false),
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
        if let Target::Patch(_, k) = self {
            return crate::modulation::PATCH_PARAMS
                .get(k)
                .map(|p| crate::modulation::from_norm(n, p.lo, p.hi, p.log))
                .unwrap_or(0.0);
        }
        let (lo, hi, log) = self.range();
        let n = n.clamp(0.0, 1.0);
        if log {
            lo * (hi / lo).powf(n)
        } else {
            lo + (hi - lo) * n
        }
    }

    pub fn to_norm(self, v: f32) -> f32 {
        if let Target::Patch(_, k) = self {
            return crate::modulation::PATCH_PARAMS
                .get(k)
                .map(|p| crate::modulation::to_norm(v, p.lo, p.hi, p.log))
                .unwrap_or(0.0);
        }
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
            Target::Macro(..) => "Macro",
            Target::Gen(..) => "Generator knob",
            Target::Patch(_, k) => crate::modulation::PATCH_PARAMS
                .get(k)
                .map(|p| p.name)
                .unwrap_or("Patch"),
            Target::ModAmount(_) => "Modulation amount",
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
            Target::ModAmount(_) => format!("{:+.0} %", v * 100.0),
            Target::Patch(..) if v.abs() >= 100.0 => format!("{v:.0}"),
            Target::Patch(..) if v.abs() >= 1.0 => format!("{v:.2}"),
            Target::Patch(..) => format!("{v:.3}"),
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

    #[test]
    fn curves_bend_linear_segments_only_when_set() {
        let mut lin = [p(0.0, 0.0, Shape::Linear), p(10.0, 1.0, Shape::Linear)];
        let straight = value_at(&lin, 2.5).unwrap();
        assert_eq!(straight, 0.25);
        lin[0].curve = 0.6;
        let up = value_at(&lin, 2.5).unwrap();
        lin[0].curve = -0.6;
        let down = value_at(&lin, 2.5).unwrap();
        assert!(up > 0.4 && down < 0.1, "{up} {down}");
        // the end points stay put
        assert_eq!(value_at(&lin, 0.0), Some(0.0));
        assert!((value_at(&lin, 9.9999).unwrap() - 1.0).abs() < 1e-3);
        assert_eq!(bend(0.37, 0.0), 0.37);
    }

    fn p(pos: f32, value: f32, shape: Shape) -> Point {
        Point {
            pos,
            value,
            shape,
            curve: 0.0,
        }
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
