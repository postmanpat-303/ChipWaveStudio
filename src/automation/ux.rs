//! Automation UX helpers (docs/research/automation-ux.md §3): colour by
//! parameter family, value-scale ticks, and Device > Parameter labels.
use super::Target;
use crate::fx::FxKind;
use crate::model::Song;

/// visual family for a lane colour (stable across lane index)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Cutoff,
    Resonance,
    Volume,
    Pan,
    Mix,
    Plugin,
    Modulation,
    Tempo,
    Other,
}

impl Family {
    /// RGB for dark / polished themes (saturated enough to read on CRT lanes)
    pub fn colour(self) -> u32 {
        match self {
            Family::Cutoff => 0x5CC6D8,     // cyan
            Family::Resonance => 0xD96FC4,  // magenta
            Family::Volume => 0x6FD07A,     // green
            Family::Pan => 0xE0A060,        // amber
            Family::Mix => 0xB07FD0,        // violet (FX mix / wet)
            Family::Plugin => 0x4FA3A0,     // teal
            Family::Modulation => 0xE6E8EB, // white-ish
            Family::Tempo => 0xE0A060,      // amber (with pan)
            Family::Other => 0x9AA8F0,      // soft blue
        }
    }
}

impl Target {
    pub fn family(self) -> Family {
        match self {
            Target::Cutoff(_) => Family::Cutoff,
            Target::Resonance(_) => Family::Resonance,
            Target::ChanVolume(_) | Target::Master | Target::Level(_) => Family::Volume,
            Target::ChanPan(_) => Family::Pan,
            Target::Tempo => Family::Tempo,
            Target::ModAmount(_) => Family::Modulation,
            Target::Macro(..) => Family::Plugin,
            Target::Fx(_, _, p) => {
                // common FX param indices: mix/wet often last; cutoff-like first
                // family is refined by `family_in` when the song is known
                let _ = p;
                Family::Mix
            }
            Target::Ret(_, 0) => Family::Mix,
            Target::Send(..) => Family::Mix,
            Target::Drive(_) | Target::Gen(..) | Target::Patch(..) | Target::Ret(..) => {
                Family::Other
            }
        }
    }

    /// family with FX kind / param name when available
    pub fn family_in(self, song: &Song) -> Family {
        match self {
            Target::Fx(i, d, p) => {
                let Some(dev) = song.instruments.get(i).and_then(|x| x.fx.get(d)) else {
                    return Family::Mix;
                };
                if dev.plugin.is_some() {
                    return Family::Plugin;
                }
                let name = crate::fx::params(dev.kind)
                    .get(p)
                    .map(|x| x.name.to_ascii_lowercase())
                    .unwrap_or_default();
                let key = crate::fx::params(dev.kind)
                    .get(p)
                    .map(|x| x.key)
                    .unwrap_or("");
                if name.contains("cutoff")
                    || name.contains("freq")
                    || key == "cutoff"
                    || key == "freq"
                    || key == "frequency"
                {
                    Family::Cutoff
                } else if name.contains("res") || key.starts_with("res") {
                    Family::Resonance
                } else if name.contains("mix")
                    || name.contains("wet")
                    || name.contains("dry")
                    || key == "mix"
                    || key == "wet"
                {
                    Family::Mix
                } else if matches!(
                    dev.kind,
                    FxKind::Filter | FxKind::Phaser | FxKind::Resonator | FxKind::FreqShift
                ) && p == 0
                {
                    Family::Cutoff
                } else {
                    Family::Mix
                }
            }
            Target::Gen(i, k) => {
                let Some(crate::model::InstrumentKind::Gen(g)) =
                    song.instruments.get(i).map(|x| &x.kind)
                else {
                    return Family::Other;
                };
                let name = crate::gen::gen_knobs(g)
                    .into_iter()
                    .find(|(id, _)| *id == k)
                    .map(|(_, n)| n.to_ascii_lowercase())
                    .unwrap_or_default();
                if name.contains("cutoff") || name.contains("filter") {
                    Family::Cutoff
                } else if name.contains("res") {
                    Family::Resonance
                } else {
                    Family::Other
                }
            }
            Target::Patch(_, k) => {
                let name = crate::modulation::PATCH_PARAMS
                    .get(k)
                    .map(|p| p.name.to_ascii_lowercase())
                    .unwrap_or_default();
                if name.contains("cutoff") {
                    Family::Cutoff
                } else if name.contains("res") {
                    Family::Resonance
                } else {
                    Family::Other
                }
            }
            _ => self.family(),
        }
    }

    /// (device, parameter) strings for the lane header picker
    pub fn device_param(self, song: &Song) -> (String, String) {
        let iname = |i: usize| {
            song.instruments
                .get(i)
                .map(|x| x.name.clone())
                .unwrap_or_else(|| format!("Instr {:02}", i + 1))
        };
        let cname = |c: usize| {
            song.channel_names
                .get(c)
                .cloned()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("Ch {}", c + 1))
        };
        match self {
            Target::Cutoff(i) => (iname(i), "Filter cutoff".into()),
            Target::Resonance(i) => (iname(i), "Resonance".into()),
            Target::Drive(i) => (iname(i), "Drive".into()),
            Target::Level(i) => (iname(i), "Amp level".into()),
            Target::ChanVolume(c) => (format!("Mixer · {}", cname(c)), "Volume".into()),
            Target::ChanPan(c) => (format!("Mixer · {}", cname(c)), "Pan".into()),
            Target::Send(c, r) => (
                format!("Mixer · {}", cname(c)),
                if r == 0 {
                    "Send A".into()
                } else {
                    "Send B".into()
                },
            ),
            Target::Master => ("Song".into(), "Master volume".into()),
            Target::Tempo => ("Song".into(), "Tempo".into()),
            Target::Ret(r, p) => {
                let k = if r == 0 {
                    FxKind::Delay
                } else {
                    FxKind::Reverb
                };
                (
                    format!("Return {}", if r == 0 { "A" } else { "B" }),
                    crate::fx::params(k)
                        .get(p)
                        .map(|x| x.name.to_string())
                        .unwrap_or_else(|| format!("Param {}", p + 1)),
                )
            }
            Target::Fx(i, d, p) => {
                let (dev, pn) = song
                    .instruments
                    .get(i)
                    .and_then(|x| x.fx.get(d))
                    .map(|dev| {
                        let title = if dev.plugin.is_some() {
                            format!("FX {} · {}", d + 1, dev.title())
                        } else {
                            format!("FX {} · {}", d + 1, dev.kind.name())
                        };
                        let pn = if dev.plugin.is_some() {
                            format!("Param {}", p + 1)
                        } else {
                            crate::fx::params(dev.kind)
                                .get(p)
                                .map(|x| x.name.to_string())
                                .unwrap_or_else(|| format!("Param {}", p + 1))
                        };
                        (title, pn)
                    })
                    .unwrap_or_else(|| (format!("FX {}", d + 1), format!("Param {}", p + 1)));
                (format!("{} · {dev}", iname(i)), pn)
            }
            Target::Macro(i, k) => (iname(i), format!("Macro {}", k + 1)),
            Target::Gen(i, k) => {
                let pn = match song.instruments.get(i).map(|x| &x.kind) {
                    Some(crate::model::InstrumentKind::Gen(g)) => crate::gen::gen_knobs(g)
                        .into_iter()
                        .find(|(id, _)| *id == k)
                        .map(|(_, n)| n)
                        .unwrap_or_else(|| format!("Knob {}", k + 1)),
                    _ => format!("Knob {}", k + 1),
                };
                (format!("{} · Generator", iname(i)), pn)
            }
            Target::Patch(i, k) => {
                let pn = crate::modulation::PATCH_PARAMS
                    .get(k)
                    .map(|p| p.name.to_string())
                    .unwrap_or_else(|| format!("Patch {}", k + 1));
                (iname(i), pn)
            }
            Target::ModAmount(r) => {
                let m = &song.modulation;
                let name = m
                    .routes
                    .get(r)
                    .map(|rt| {
                        format!(
                            "{} > depth",
                            m.find(rt.src).map(|k| m.title(k)).unwrap_or_default()
                        )
                    })
                    .unwrap_or_else(|| format!("Route {}", r + 1));
                ("Modulation".into(), name)
            }
        }
    }

    /// "Device > Parameter" for a lane header
    pub fn header_label(self, song: &Song) -> String {
        let (d, p) = self.device_param(song);
        format!("{d} > {p}")
    }

    /// value-scale ticks as (normalised 0..1, label). Empty = no special scale.
    pub fn scale_ticks(self) -> &'static [(f32, &'static str)] {
        match self {
            Target::Cutoff(_) => &[
                (0.0, "20"),
                (0.23, "100"),
                (0.46, "500"),
                (0.57, "1k"),
                (0.80, "5k"),
                (1.0, "20k"),
            ],
            Target::ChanVolume(_) | Target::Master | Target::Level(_) => &[
                (0.0, "-∞"),
                (0.33, "-12"),
                (0.5, "-6"),
                (0.67, "0"),
                (1.0, "+6"),
            ],
            Target::ChanPan(_) => &[(0.0, "L"), (0.5, "C"), (1.0, "R")],
            Target::Tempo => &[(0.0, "40"), (0.23, "100"), (0.5, "170"), (1.0, "300")],
            Target::Resonance(_)
            | Target::Drive(_)
            | Target::Send(..)
            | Target::Macro(..)
            | Target::Fx(..)
            | Target::Ret(..)
            | Target::Gen(..)
            | Target::Patch(..) => &[(0.0, "0"), (0.5, "50"), (1.0, "100")],
            Target::ModAmount(_) => &[(0.0, "-100"), (0.5, "0"), (1.0, "+100")],
        }
    }
}

/// axis width reserved for the value scale on a lane body
pub const SCALE_W: f32 = 34.0;
/// hit radius for points (design A)
pub const POINT_HIT: f32 = 10.0;
/// drawn point radius (idle / selected)
pub const POINT_R: f32 = 5.0;
pub const POINT_R_HOT: f32 = 7.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Instrument, InstrumentKind, Song};

    fn song() -> Song {
        Song::empty()
    }

    #[test]
    fn cutoff_is_cyan_and_log_ticks() {
        let t = Target::Cutoff(0);
        assert_eq!(t.family(), Family::Cutoff);
        assert_eq!(t.family().colour(), 0x5CC6D8);
        assert!(t.scale_ticks().len() >= 4);
        assert_eq!(t.scale_ticks()[0].1, "20");
    }

    #[test]
    fn header_label_uses_device_and_param() {
        let s = song();
        let (d, p) = Target::ChanVolume(0).device_param(&s);
        assert!(d.contains("Mixer"), "{d}");
        assert_eq!(p, "Volume");
        let h = Target::Tempo.header_label(&s);
        assert!(h.contains('>'), "{h}");
        assert!(h.contains("Tempo"), "{h}");
    }

    #[test]
    fn filter_fx_cutoff_family() {
        let mut s = song();
        s.instruments.push(Instrument {
            arp: None,
            name: "Bass".into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![crate::fx::Device::new(FxKind::Filter)],
            kind: InstrumentKind::Synth(Default::default()),
        });
        let t = Target::Fx(0, 0, 0);
        assert_eq!(t.family_in(&s), Family::Cutoff);
        assert!(
            t.header_label(&s).contains("Filter"),
            "{}",
            t.header_label(&s)
        );
    }
}

/// Timeline automation editing tool (design A §3.2)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AutoTool {
    #[default]
    Pointer,
    Pencil,
    Line,
    /// shape stamp: sine / tri / saw / square / random over a dragged range
    Shape,
}

impl AutoTool {
    pub const ALL: [AutoTool; 4] = [
        AutoTool::Pointer,
        AutoTool::Pencil,
        AutoTool::Line,
        AutoTool::Shape,
    ];
    pub fn name(self) -> &'static str {
        match self {
            AutoTool::Pointer => "Pointer",
            AutoTool::Pencil => "Pencil",
            AutoTool::Line => "Line",
            AutoTool::Shape => "Shape",
        }
    }
}

/// Write mode for recording knob moves into a lane (design A §3.3)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum WriteMode {
    #[default]
    Read,
    Touch,
    Latch,
    Write,
    Off,
}

impl WriteMode {
    pub const ALL: [WriteMode; 5] = [
        WriteMode::Read,
        WriteMode::Touch,
        WriteMode::Latch,
        WriteMode::Write,
        WriteMode::Off,
    ];
    pub fn name(self) -> &'static str {
        match self {
            WriteMode::Read => "Read",
            WriteMode::Touch => "Touch",
            WriteMode::Latch => "Latch",
            WriteMode::Write => "Write",
            WriteMode::Off => "Off",
        }
    }
}

/// How a clip lane combines with the track lane (design A §3.5)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ClipMode {
    /// replaces the track lane inside the clip (current behaviour)
    #[default]
    Absolute,
    /// adds ±50 % of the parameter range to the track value
    RelAdd,
    /// multiplies the track value (0..1 → 0 %..100 %)
    RelMul,
}

impl ClipMode {
    pub fn is_absolute(m: &ClipMode) -> bool {
        matches!(m, ClipMode::Absolute)
    }
    pub fn name(self) -> &'static str {
        match self {
            ClipMode::Absolute => "Absolute",
            ClipMode::RelAdd => "Relative +",
            ClipMode::RelMul => "Relative ×",
        }
    }
}

/// Combine track + clip values per clip mode.
pub fn combine_clip(track: Option<f32>, clip: Option<f32>, mode: ClipMode) -> Option<f32> {
    match (track, clip, mode) {
        (_, Some(c), ClipMode::Absolute) => Some(c),
        (Some(t), Some(c), ClipMode::RelAdd) => Some((t + (c - 0.5)).clamp(0.0, 1.0)),
        (Some(t), Some(c), ClipMode::RelMul) => Some((t * c).clamp(0.0, 1.0)),
        (t, None, _) => t,
        (None, c, _) => c,
    }
}

#[cfg(test)]
mod clip_mode_tests {
    use super::*;

    #[test]
    fn combine_clip_absolute_replaces() {
        assert_eq!(
            combine_clip(Some(0.2), Some(0.9), ClipMode::Absolute),
            Some(0.9)
        );
        assert_eq!(combine_clip(None, Some(0.9), ClipMode::Absolute), Some(0.9));
        assert_eq!(combine_clip(Some(0.2), None, ClipMode::Absolute), Some(0.2));
    }

    #[test]
    fn combine_clip_rel_add_and_mul() {
        assert_eq!(
            combine_clip(Some(0.5), Some(0.75), ClipMode::RelAdd),
            Some(0.75)
        );
        assert_eq!(
            combine_clip(Some(0.5), Some(0.25), ClipMode::RelAdd),
            Some(0.25)
        );
        assert_eq!(
            combine_clip(Some(0.8), Some(0.5), ClipMode::RelMul),
            Some(0.4)
        );
        assert_eq!(
            combine_clip(Some(0.8), Some(1.0), ClipMode::RelMul),
            Some(0.8)
        );
    }

    #[test]
    fn write_mode_names() {
        assert_eq!(WriteMode::Read.name(), "Read");
        assert_eq!(WriteMode::Touch.name(), "Touch");
        assert_eq!(ClipMode::RelMul.name(), "Relative ×");
    }
}
