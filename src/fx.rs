//! Insert / return effects: devices with normalised params (0..1, mapped to
//! real units through `ParamInfo`) and their realtime DSP state.
//! DSP states allocate their buffers once when built (engine thread, never in
//! the audio callback); `process` never allocates. Params are smoothed.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxKind {
    Delay,
    Reverb,
    Drive,
    Chorus,
    Filter,
    Bitcrusher,
    Compressor,
}

impl FxKind {
    pub const ALL: [FxKind; 7] = [
        FxKind::Delay,
        FxKind::Reverb,
        FxKind::Drive,
        FxKind::Chorus,
        FxKind::Filter,
        FxKind::Bitcrusher,
        FxKind::Compressor,
    ];
    pub fn name(self) -> &'static str {
        match self {
            FxKind::Delay => "Delay",
            FxKind::Reverb => "Reverb",
            FxKind::Drive => "Drive",
            FxKind::Chorus => "Chorus / Flanger",
            FxKind::Filter => "Filter",
            FxKind::Bitcrusher => "Bitcrusher",
            FxKind::Compressor => "Compressor",
        }
    }
    /// muted strip tint per device type
    pub fn tint(self) -> u32 {
        match self {
            FxKind::Delay | FxKind::Chorus => 0x6B5E8A, // time-based: violet
            FxKind::Reverb => 0x5E5A86,
            FxKind::Drive | FxKind::Bitcrusher => 0x8A5A4E, // warm
            FxKind::Filter => 0x4E6A86,                     // cool
            FxKind::Compressor => 0x5E6B7A,                 // neutral
        }
    }
}

pub struct ParamInfo {
    pub key: &'static str,
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub def: f32,
    pub log: bool,
    pub unit: &'static str,
    /// named steps (the value is an index)
    pub choices: &'static [&'static str],
}

const fn p(
    key: &'static str,
    name: &'static str,
    min: f32,
    max: f32,
    def: f32,
    log: bool,
    unit: &'static str,
) -> ParamInfo {
    ParamInfo {
        key,
        name,
        min,
        max,
        def,
        log,
        unit,
        choices: &[],
    }
}

const fn c(
    key: &'static str,
    name: &'static str,
    choices: &'static [&'static str],
    def: f32,
) -> ParamInfo {
    ParamInfo {
        key,
        name,
        min: 0.0,
        max: (choices.len() - 1) as f32,
        def,
        log: false,
        unit: "",
        choices,
    }
}

pub const SYNC: [&str; 7] = ["ms", "1/16", "1/8", "1/8 dot", "1/4", "1/4 dot", "1/2"];
const SYNC_BEATS: [f32; 6] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0];

const DELAY: [ParamInfo; 7] = [
    c("sync", "Sync", &SYNC, 3.0),
    p("time", "Time", 1.0, 2000.0, 350.0, true, "ms"),
    p("feedback", "Feedback", 0.0, 0.95, 0.35, false, "%"),
    c("pingpong", "Ping-pong", &["off", "on"], 1.0),
    p("hp", "Loop HP", 20.0, 2000.0, 120.0, true, "Hz"),
    p("lp", "Loop LP", 500.0, 18000.0, 6000.0, true, "Hz"),
    p("mix", "Mix", 0.0, 1.0, 0.3, false, "%"),
];
const REVERB: [ParamInfo; 5] = [
    p("size", "Size", 0.0, 1.0, 0.6, false, "%"),
    p("damping", "Damping", 0.0, 1.0, 0.5, false, "%"),
    p("predelay", "Pre-delay", 0.0, 200.0, 10.0, false, "ms"),
    p("width", "Width", 0.0, 1.0, 1.0, false, "%"),
    p("mix", "Mix", 0.0, 1.0, 0.25, false, "%"),
];
const DRIVE: [ParamInfo; 5] = [
    c("mode", "Mode", &["soft", "hard", "fold"], 0.0),
    p("drive", "Drive", 0.0, 36.0, 12.0, false, "dB"),
    p("tone", "Tone", 500.0, 18000.0, 9000.0, true, "Hz"),
    p("output", "Output", -24.0, 6.0, -6.0, false, "dB"),
    p("mix", "Mix", 0.0, 1.0, 1.0, false, "%"),
];
const CHORUS: [ParamInfo; 5] = [
    p("rate", "Rate", 0.05, 5.0, 0.6, true, "Hz"),
    p("depth", "Depth", 0.0, 8.0, 2.5, false, "ms"),
    p("delay", "Delay", 1.0, 25.0, 12.0, false, "ms"),
    p("feedback", "Feedback", -0.9, 0.9, 0.0, false, "%"),
    p("mix", "Mix", 0.0, 1.0, 0.5, false, "%"),
];
const FILTER: [ParamInfo; 6] = [
    c("mode", "Mode", &["LP", "HP", "BP"], 0.0),
    p("cutoff", "Cutoff", 20.0, 18000.0, 1200.0, true, "Hz"),
    p("resonance", "Resonance", 0.0, 0.98, 0.3, false, "%"),
    p("lfo_rate", "LFO rate", 0.01, 10.0, 0.25, true, "Hz"),
    p("lfo_depth", "LFO depth", 0.0, 4.0, 0.0, false, "oct"),
    p("mix", "Mix", 0.0, 1.0, 1.0, false, "%"),
];
const CRUSH: [ParamInfo; 3] = [
    p("bits", "Bits", 1.0, 16.0, 8.0, false, "bit"),
    p("rate", "Rate", 500.0, 48000.0, 11025.0, true, "Hz"),
    p("mix", "Mix", 0.0, 1.0, 1.0, false, "%"),
];
const COMP: [ParamInfo; 5] = [
    p("threshold", "Threshold", -48.0, 0.0, -18.0, false, "dB"),
    p("ratio", "Ratio", 1.0, 20.0, 4.0, true, ":1"),
    p("attack", "Attack", 0.1, 100.0, 5.0, true, "ms"),
    p("release", "Release", 10.0, 1000.0, 120.0, true, "ms"),
    p("makeup", "Makeup", 0.0, 24.0, 4.0, false, "dB"),
];

pub fn params(k: FxKind) -> &'static [ParamInfo] {
    match k {
        FxKind::Delay => &DELAY,
        FxKind::Reverb => &REVERB,
        FxKind::Drive => &DRIVE,
        FxKind::Chorus => &CHORUS,
        FxKind::Filter => &FILTER,
        FxKind::Bitcrusher => &CRUSH,
        FxKind::Compressor => &COMP,
    }
}

pub const MAX_PARAMS: usize = 8;

impl ParamInfo {
    pub fn to_real(&self, n: f32) -> f32 {
        let n = n.clamp(0.0, 1.0);
        let v = if self.log {
            self.min * (self.max / self.min).powf(n)
        } else {
            self.min + (self.max - self.min) * n
        };
        if self.choices.is_empty() {
            v
        } else {
            v.round()
        }
    }
    pub fn to_norm(&self, v: f32) -> f32 {
        let v = v.clamp(self.min, self.max);
        if self.max <= self.min {
            0.0
        } else if self.log {
            (v / self.min).ln() / (self.max / self.min).ln()
        } else {
            (v - self.min) / (self.max - self.min)
        }
    }
    pub fn format(&self, n: f32) -> String {
        let v = self.to_real(n);
        if !self.choices.is_empty() {
            return self.choices[(v as usize).min(self.choices.len() - 1)].into();
        }
        match self.unit {
            "%" => format!("{:.0} %", v * 100.0),
            "Hz" if v >= 1000.0 => format!("{:.2} kHz", v / 1000.0),
            "Hz" => format!("{v:.1} Hz"),
            "ms" => format!("{v:.1} ms"),
            "dB" => format!("{v:+.1} dB"),
            ":1" => format!("{v:.1}:1"),
            "bit" => format!("{v:.0} bit"),
            u => format!("{v:.2} {u}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub kind: FxKind,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub collapsed: bool,
    /// normalised parameter values, indexed like `params(kind)`
    #[serde(default)]
    pub params: Vec<f32>,
}

fn yes() -> bool {
    true
}

impl Device {
    pub fn new(kind: FxKind) -> Self {
        let params = params(kind).iter().map(|i| i.to_norm(i.def)).collect();
        Self {
            kind,
            enabled: true,
            collapsed: false,
            params,
        }
    }
    /// fix lengths / ranges after loading
    pub fn fix(&mut self) {
        let info = params(self.kind);
        let mut v: Vec<f32> = info.iter().map(|i| i.to_norm(i.def)).collect();
        for (d, s) in v.iter_mut().zip(&self.params) {
            *d = s.clamp(0.0, 1.0);
        }
        self.params = v;
    }
    #[allow(dead_code)]
    pub fn real(&self, i: usize) -> f32 {
        let info = &params(self.kind)[i];
        info.to_real(
            self.params
                .get(i)
                .copied()
                .unwrap_or_else(|| info.to_norm(info.def)),
        )
    }
    pub fn set_real(&mut self, key: &str, v: f32) {
        if let Some(i) = params(self.kind).iter().position(|p| p.key == key) {
            self.params[i] = params(self.kind)[i].to_norm(v);
        }
    }
    pub fn param_index(kind: FxKind, key: &str) -> Option<usize> {
        params(kind).iter().position(|p| p.key == key)
    }
}

/// Device presets (folder icon): name + real values by key.
pub fn presets(k: FxKind) -> &'static [(&'static str, &'static [(&'static str, f32)])] {
    match k {
        FxKind::Delay => &[
            (
                "1/8 dotted ping-pong",
                &[
                    ("sync", 3.0),
                    ("feedback", 0.4),
                    ("pingpong", 1.0),
                    ("mix", 0.25),
                ],
            ),
            (
                "Slapback",
                &[
                    ("sync", 0.0),
                    ("time", 90.0),
                    ("feedback", 0.1),
                    ("pingpong", 0.0),
                    ("mix", 0.3),
                ],
            ),
            (
                "Dub 1/4 dotted",
                &[
                    ("sync", 5.0),
                    ("feedback", 0.7),
                    ("lp", 2500.0),
                    ("hp", 300.0),
                    ("mix", 0.35),
                ],
            ),
        ],
        FxKind::Reverb => &[
            (
                "Small room",
                &[
                    ("size", 0.3),
                    ("damping", 0.6),
                    ("predelay", 5.0),
                    ("mix", 0.2),
                ],
            ),
            (
                "Big hall",
                &[
                    ("size", 0.9),
                    ("damping", 0.35),
                    ("predelay", 30.0),
                    ("mix", 0.35),
                ],
            ),
            (
                "Space wash",
                &[
                    ("size", 1.0),
                    ("damping", 0.2),
                    ("predelay", 80.0),
                    ("mix", 0.55),
                ],
            ),
        ],
        FxKind::Drive => &[
            (
                "Warm tape",
                &[
                    ("mode", 0.0),
                    ("drive", 6.0),
                    ("tone", 8000.0),
                    ("output", -3.0),
                ],
            ),
            (
                "Hard clip",
                &[("mode", 1.0), ("drive", 18.0), ("output", -9.0)],
            ),
            (
                "Wavefolder",
                &[("mode", 2.0), ("drive", 14.0), ("output", -8.0)],
            ),
        ],
        FxKind::Chorus => &[
            (
                "Wide chorus",
                &[
                    ("rate", 0.5),
                    ("depth", 3.0),
                    ("delay", 14.0),
                    ("feedback", 0.0),
                    ("mix", 0.5),
                ],
            ),
            (
                "Jet flanger",
                &[
                    ("rate", 0.15),
                    ("depth", 2.0),
                    ("delay", 1.5),
                    ("feedback", 0.75),
                    ("mix", 0.5),
                ],
            ),
        ],
        FxKind::Filter => &[
            (
                "Slow LP sweep",
                &[
                    ("mode", 0.0),
                    ("cutoff", 900.0),
                    ("resonance", 0.5),
                    ("lfo_rate", 0.1),
                    ("lfo_depth", 2.0),
                ],
            ),
            (
                "HP thin",
                &[
                    ("mode", 1.0),
                    ("cutoff", 400.0),
                    ("resonance", 0.2),
                    ("lfo_depth", 0.0),
                ],
            ),
        ],
        FxKind::Bitcrusher => &[
            (
                "Amiga 8-bit",
                &[("bits", 8.0), ("rate", 16726.0), ("mix", 1.0)],
            ),
            (
                "Lo-fi 4-bit",
                &[("bits", 4.0), ("rate", 6000.0), ("mix", 0.7)],
            ),
        ],
        FxKind::Compressor => &[
            (
                "Glue",
                &[
                    ("threshold", -14.0),
                    ("ratio", 2.0),
                    ("attack", 20.0),
                    ("release", 200.0),
                    ("makeup", 2.0),
                ],
            ),
            (
                "Punch",
                &[
                    ("threshold", -22.0),
                    ("ratio", 6.0),
                    ("attack", 8.0),
                    ("release", 80.0),
                    ("makeup", 6.0),
                ],
            ),
        ],
    }
}

/// Parse a preset's `[[fx]]` table (factory .toml format) into a device.
pub fn from_preset(t: &toml::Value) -> Option<Device> {
    let ty = t.get("type")?.as_str()?;
    let kind = match ty {
        "delay" => FxKind::Delay,
        "reverb" => FxKind::Reverb,
        "distortion" | "drive" => FxKind::Drive,
        "chorus" | "flanger" => FxKind::Chorus,
        "filter" => FxKind::Filter,
        "bitcrush" | "bitcrusher" => FxKind::Bitcrusher,
        "compressor" => FxKind::Compressor,
        _ => return None, // e.g. phaser: not yet
    };
    let num = |k: &str| {
        t.get(k)
            .and_then(|x| x.as_float().or(x.as_integer().map(|i| i as f64)))
            .map(|f| f as f32)
    };
    let mut d = Device::new(kind);
    if let Some(m) = num("mix") {
        d.set_real("mix", m);
    }
    match kind {
        FxKind::Delay => {
            if let Some(tm) = t.get("time").and_then(|x| x.as_str()) {
                let beats = match tm {
                    "1/16" => Some(1.0),
                    "1/8" => Some(2.0),
                    "3/16" | "1/8 dotted" => Some(3.0),
                    "1/4" => Some(4.0),
                    "3/8" | "1/4 dotted" => Some(5.0),
                    "1/2" => Some(6.0),
                    _ => None,
                };
                if let Some(b) = beats {
                    d.set_real("sync", b);
                }
            } else if let Some(ms) = num("time_ms") {
                d.set_real("sync", 0.0);
                d.set_real("time", ms);
            }
            if let Some(f) = num("feedback") {
                d.set_real("feedback", f);
            }
            let pp = t
                .get("ping_pong")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            d.set_real("pingpong", if pp { 1.0 } else { 0.0 });
        }
        FxKind::Reverb => {
            if let Some(s) = num("size") {
                d.set_real("size", s);
            }
            if let Some(s) = num("damping") {
                d.set_real("damping", s);
            }
        }
        FxKind::Drive => {
            let mode = match t.get("mode").and_then(|x| x.as_str()) {
                Some("hard") => 1.0,
                Some("fold") | Some("foldback") => 2.0,
                _ => 0.0,
            };
            d.set_real("mode", mode);
            if let Some(v) = num("drive") {
                d.set_real("drive", v * 36.0);
            }
        }
        FxKind::Chorus => {
            if let Some(v) = num("rate_hz") {
                d.set_real("rate", v);
            }
            if let Some(v) = num("depth") {
                d.set_real("depth", v * 8.0);
            }
        }
        FxKind::Bitcrusher => {
            if let Some(v) = num("bits") {
                d.set_real("bits", v);
            }
            if let Some(v) = num("rate_reduce") {
                d.set_real("rate", 44100.0 / v.max(1.0));
            }
        }
        _ => {}
    }
    Some(d)
}

// ------------------------------------------------------------------ DSP

fn db(v: f32) -> f32 {
    10f32.powf(v / 20.0)
}

/// fractional read `d` samples behind write index `w`
fn read(buf: &[f32], w: usize, d: f32) -> f32 {
    let n = buf.len();
    let d = d.clamp(1.0, (n - 2) as f32);
    let pos = w as f32 + n as f32 - d;
    let i = pos as usize;
    let fr = pos - i as f32;
    let a = buf[i % n];
    let b = buf[(i + 1) % n];
    a + (b - a) * fr
}

#[derive(Default)]
struct OnePole {
    z: f32,
}
impl OnePole {
    fn lp(&mut self, x: f32, a: f32) -> f32 {
        self.z += (x - self.z) * a;
        self.z
    }
}
fn pole_coef(fc: f32, sr: f32) -> f32 {
    1.0 - (-2.0 * std::f32::consts::PI * fc / sr).exp()
}

#[derive(Default, Clone, Copy)]
struct Svf {
    ic1: f32,
    ic2: f32,
}
impl Svf {
    /// returns (lp, bp, hp)
    fn run(&mut self, x: f32, g: f32, k: f32) -> (f32, f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - k * v1 - v2)
    }
}

struct Comb {
    buf: Vec<f32>,
    i: usize,
    filt: f32,
}
impl Comb {
    fn new(n: usize) -> Self {
        Self {
            buf: vec![0.0; n.max(2)],
            i: 0,
            filt: 0.0,
        }
    }
    fn run(&mut self, x: f32, fb: f32, damp: f32) -> f32 {
        let out = self.buf[self.i];
        self.filt = out * (1.0 - damp) + self.filt * damp;
        self.buf[self.i] = x + self.filt * fb;
        self.i = (self.i + 1) % self.buf.len();
        out
    }
}
struct Allpass {
    buf: Vec<f32>,
    i: usize,
}
impl Allpass {
    fn new(n: usize) -> Self {
        Self {
            buf: vec![0.0; n.max(2)],
            i: 0,
        }
    }
    fn run(&mut self, x: f32) -> f32 {
        let b = self.buf[self.i];
        self.buf[self.i] = x + b * 0.5;
        self.i = (self.i + 1) % self.buf.len();
        b - x
    }
}

enum Dsp {
    Delay {
        l: Vec<f32>,
        r: Vec<f32>,
        w: usize,
        d: f32,
        lp: [OnePole; 2],
        hp: [OnePole; 2],
    },
    Reverb {
        pre: Vec<f32>,
        pw: usize,
        combs: Vec<[Comb; 2]>,
        aps: Vec<[Allpass; 2]>,
    },
    Drive {
        tone: [OnePole; 2],
    },
    Chorus {
        l: Vec<f32>,
        r: Vec<f32>,
        w: usize,
        ph: f32,
    },
    Filter {
        s: [Svf; 2],
        ph: f32,
    },
    Crush {
        ph: f32,
        hold: [f32; 2],
    },
    Comp {
        env: f32,
    },
}

/// realtime state for one device instance
pub struct FxState {
    pub kind: FxKind,
    sr: f32,
    on: bool,
    on_gain: f32,
    tgt: [f32; MAX_PARAMS],
    cur: [f32; MAX_PARAMS],
    k: f32,
    dsp: Dsp,
}

pub struct Ctx {
    pub bpm: f32,
}

impl FxState {
    pub fn new(d: &Device, sr: f32) -> Self {
        let n = |s: f32| (s * sr) as usize + 4;
        let scale = sr / 44100.0;
        let dsp = match d.kind {
            FxKind::Delay => Dsp::Delay {
                l: vec![0.0; n(2.1)],
                r: vec![0.0; n(2.1)],
                w: 0,
                d: -1.0,
                lp: Default::default(),
                hp: Default::default(),
            },
            FxKind::Reverb => {
                const C: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
                const A: [usize; 4] = [556, 441, 341, 225];
                let sz = |t: usize, sp: usize| ((t + sp) as f32 * scale) as usize;
                Dsp::Reverb {
                    pre: vec![0.0; n(0.25)],
                    pw: 0,
                    combs: C
                        .iter()
                        .map(|&t| [Comb::new(sz(t, 0)), Comb::new(sz(t, 23))])
                        .collect(),
                    aps: A
                        .iter()
                        .map(|&t| [Allpass::new(sz(t, 0)), Allpass::new(sz(t, 23))])
                        .collect(),
                }
            }
            FxKind::Drive => Dsp::Drive {
                tone: Default::default(),
            },
            FxKind::Chorus => Dsp::Chorus {
                l: vec![0.0; n(0.05)],
                r: vec![0.0; n(0.05)],
                w: 0,
                ph: 0.0,
            },
            FxKind::Filter => Dsp::Filter {
                s: Default::default(),
                ph: 0.0,
            },
            FxKind::Bitcrusher => Dsp::Crush {
                ph: 1.0,
                hold: [0.0; 2],
            },
            FxKind::Compressor => Dsp::Comp { env: 0.0 },
        };
        let mut s = Self {
            kind: d.kind,
            sr,
            on: d.enabled,
            on_gain: if d.enabled { 1.0 } else { 0.0 },
            tgt: [0.0; MAX_PARAMS],
            cur: [0.0; MAX_PARAMS],
            k: 1.0 - (-1.0 / (0.01 * sr)).exp(),
            dsp,
        };
        s.set_targets(d, |_| None);
        s.cur = s.tgt;
        s
    }

    /// Make this state sound like `FxState::new(d, sr)` again without
    /// allocating: buffers are zeroed in place (kind must match).
    pub fn reset(&mut self, d: &Device) {
        debug_assert_eq!(self.kind, d.kind);
        match &mut self.dsp {
            Dsp::Delay {
                l,
                r,
                w,
                d: dl,
                lp,
                hp,
            } => {
                l.fill(0.0);
                r.fill(0.0);
                *w = 0;
                *dl = -1.0;
                *lp = Default::default();
                *hp = Default::default();
            }
            Dsp::Reverb {
                pre,
                pw,
                combs,
                aps,
            } => {
                pre.fill(0.0);
                *pw = 0;
                for c in combs.iter_mut().flatten() {
                    c.buf.fill(0.0);
                    c.i = 0;
                    c.filt = 0.0;
                }
                for a in aps.iter_mut().flatten() {
                    a.buf.fill(0.0);
                    a.i = 0;
                }
            }
            Dsp::Drive { tone } => *tone = Default::default(),
            Dsp::Chorus { l, r, w, ph } => {
                l.fill(0.0);
                r.fill(0.0);
                *w = 0;
                *ph = 0.0;
            }
            Dsp::Filter { s, ph } => {
                *s = Default::default();
                *ph = 0.0;
            }
            Dsp::Crush { ph, hold } => {
                *ph = 1.0;
                *hold = [0.0; 2];
            }
            Dsp::Comp { env } => *env = 0.0,
        }
        self.on_gain = if d.enabled { 1.0 } else { 0.0 };
        self.set_targets(d, |_| None);
        self.cur = self.tgt;
    }

    /// update parameter targets (with optional normalised overrides from automation)
    pub fn set_targets(&mut self, d: &Device, over: impl Fn(usize) -> Option<f32>) {
        self.on = d.enabled;
        for (i, info) in params(self.kind).iter().enumerate().take(MAX_PARAMS) {
            let n = over(i).unwrap_or_else(|| d.params.get(i).copied().unwrap_or(0.0));
            self.tgt[i] = info.to_real(n);
            if !info.choices.is_empty() {
                self.cur[i] = self.tgt[i];
            }
        }
    }

    pub fn process(&mut self, l: f32, r: f32, ctx: &Ctx) -> (f32, f32) {
        let step = 1.0 / (0.005 * self.sr);
        let g_t = if self.on { 1.0 } else { 0.0 };
        if self.on_gain != g_t {
            self.on_gain = if g_t > self.on_gain {
                (self.on_gain + step).min(1.0)
            } else {
                (self.on_gain - step).max(0.0)
            };
        }
        if self.on_gain <= 0.0 {
            return (l, r);
        }
        for i in 0..MAX_PARAMS {
            self.cur[i] += (self.tgt[i] - self.cur[i]) * self.k;
        }
        let (wl, wr) = self.wet(l, r, ctx);
        let (wl, wr) = (
            if wl.is_finite() {
                wl.clamp(-8.0, 8.0)
            } else {
                0.0
            },
            if wr.is_finite() {
                wr.clamp(-8.0, 8.0)
            } else {
                0.0
            },
        );
        let g = self.on_gain;
        (l + (wl - l) * g, r + (wr - r) * g)
    }

    /// device output including its own dry/wet mix
    fn wet(&mut self, l: f32, r: f32, ctx: &Ctx) -> (f32, f32) {
        let p = self.cur;
        let sr = self.sr;
        match &mut self.dsp {
            Dsp::Delay {
                l: bl,
                r: br,
                w,
                d,
                lp,
                hp,
            } => {
                let sync = p[0] as usize;
                let secs = if sync == 0 {
                    p[1] / 1000.0
                } else {
                    60.0 / ctx.bpm.max(20.0) * SYNC_BEATS[(sync - 1).min(5)]
                };
                let target = (secs * sr).clamp(1.0, (bl.len() - 3) as f32);
                if *d < 0.0 {
                    *d = target;
                }
                *d += (target - *d) * 0.0008;
                let dl = read(bl, *w, *d);
                let dr = read(br, *w, *d);
                let (alp, ahp) = (pole_coef(p[5], sr), pole_coef(p[4], sr));
                let fl = lp[0].lp(dl, alp);
                let fl = fl - hp[0].lp(fl, ahp);
                let fr = lp[1].lp(dr, alp);
                let fr = fr - hp[1].lp(fr, ahp);
                let fb = p[2];
                let (inl, inr) = if p[3] >= 0.5 {
                    ((l + r) * 0.5 + fb * fr, fb * fl)
                } else {
                    (l + fb * fl, r + fb * fr)
                };
                bl[*w] = inl.clamp(-4.0, 4.0);
                br[*w] = inr.clamp(-4.0, 4.0);
                *w = (*w + 1) % bl.len();
                let m = p[6];
                (l * (1.0 - m) + dl * m, r * (1.0 - m) + dr * m)
            }
            Dsp::Reverb {
                pre,
                pw,
                combs,
                aps,
            } => {
                let pd = (p[2] / 1000.0 * sr).clamp(0.0, (pre.len() - 2) as f32);
                pre[*pw] = (l + r) * 0.5;
                let x = read(pre, *pw + 1, pd + 1.0) * 0.015;
                *pw = (*pw + 1) % pre.len();
                let fb = 0.7 + p[0] * 0.28;
                let damp = p[1] * 0.4;
                let (mut ol, mut or) = (0.0, 0.0);
                for c in combs.iter_mut() {
                    ol += c[0].run(x, fb, damp);
                    or += c[1].run(x, fb, damp);
                }
                for a in aps.iter_mut() {
                    ol = a[0].run(ol);
                    or = a[1].run(or);
                }
                let wid = p[3];
                let (w1, w2) = ((wid / 2.0 + 0.5) * 3.0, ((1.0 - wid) / 2.0) * 3.0);
                let (yl, yr) = (ol * w1 + or * w2, or * w1 + ol * w2);
                let m = p[4];
                (l * (1.0 - m) + yl * m, r * (1.0 - m) + yr * m)
            }
            Dsp::Drive { tone } => {
                let g = db(p[1]);
                let shape = |x: f32| match p[0] as usize {
                    0 => (x * g).tanh(),
                    1 => (x * g).clamp(-1.0, 1.0),
                    _ => {
                        let y = x * g * 0.5;
                        ((y + 1.0).rem_euclid(4.0) - 2.0).abs() - 1.0
                    }
                };
                let a = pole_coef(p[2], sr);
                let out = db(p[3]);
                let yl = tone[0].lp(shape(l), a) * out;
                let yr = tone[1].lp(shape(r), a) * out;
                let m = p[4];
                (l * (1.0 - m) + yl * m, r * (1.0 - m) + yr * m)
            }
            Dsp::Chorus {
                l: bl,
                r: br,
                w,
                ph,
            } => {
                *ph = (*ph + p[0] / sr).fract();
                let tau = std::f32::consts::TAU;
                let ml = 0.5 + 0.5 * (*ph * tau).sin();
                let mr = 0.5 + 0.5 * (*ph * tau + tau / 4.0).sin();
                let dl = (p[2] + p[1] * ml) / 1000.0 * sr;
                let dr = (p[2] + p[1] * mr) / 1000.0 * sr;
                let yl = read(bl, *w, dl);
                let yr = read(br, *w, dr);
                bl[*w] = (l + yl * p[3]).clamp(-4.0, 4.0);
                br[*w] = (r + yr * p[3]).clamp(-4.0, 4.0);
                *w = (*w + 1) % bl.len();
                let m = p[4];
                (l * (1.0 - m) + yl * m, r * (1.0 - m) + yr * m)
            }
            Dsp::Filter { s, ph } => {
                *ph = (*ph + p[3] / sr).fract();
                let fc = (p[1] * 2f32.powf(p[4] * (*ph * std::f32::consts::TAU).sin()))
                    .clamp(20.0, sr * 0.45);
                let g = (std::f32::consts::PI * fc / sr).tan();
                let k = (2.0 - 2.0 * p[2]).max(0.05);
                let pick = |o: (f32, f32, f32)| match p[0] as usize {
                    0 => o.0,
                    1 => o.2,
                    _ => o.1,
                };
                let yl = pick(s[0].run(l, g, k));
                let yr = pick(s[1].run(r, g, k));
                let m = p[5];
                (l * (1.0 - m) + yl * m, r * (1.0 - m) + yr * m)
            }
            Dsp::Crush { ph, hold } => {
                *ph += p[1] / sr;
                if *ph >= 1.0 {
                    *ph -= ph.floor();
                    let lv = 2f32.powf(p[0] - 1.0).max(1.0);
                    hold[0] = (l.clamp(-1.0, 1.0) * lv).round() / lv;
                    hold[1] = (r.clamp(-1.0, 1.0) * lv).round() / lv;
                }
                let m = p[2];
                (l * (1.0 - m) + hold[0] * m, r * (1.0 - m) + hold[1] * m)
            }
            Dsp::Comp { env } => {
                let x = l.abs().max(r.abs());
                let a = if x > *env {
                    1.0 - (-1.0 / (p[2] / 1000.0 * sr)).exp()
                } else {
                    1.0 - (-1.0 / (p[3] / 1000.0 * sr)).exp()
                };
                *env += (x - *env) * a;
                let lvl = 20.0 * env.max(1e-6).log10();
                let over = lvl - p[0];
                let gr = if over > 0.0 {
                    over * (1.0 - 1.0 / p[1])
                } else {
                    0.0
                };
                let g = db(p[4] - gr);
                (l * g, r * g)
            }
        }
    }
}

/// A chain of device states mirroring a `[Device]` list.
#[derive(Default)]
pub struct ChainState {
    /// structure signature (kinds) the states were built for
    sig: Vec<FxKind>,
    pub states: Vec<FxState>,
    pub owner: usize,
    /// chains of instruments that played on this channel before, kept so a
    /// mid-song instrument switch reuses their buffers instead of allocating
    spare: Vec<(usize, Vec<FxKind>, Vec<FxState>)>,
}

/// spare chains kept per channel
const SPARE_CHAINS: usize = 6;

fn same_sig(sig: &[FxKind], devs: &[Device]) -> bool {
    sig.len() == devs.len() && sig.iter().zip(devs).all(|(k, d)| *k == d.kind)
}

impl ChainState {
    /// Bring the states in line with `devs` (engine thread). On an instrument
    /// switch the previous chain is parked and a parked chain for the new owner
    /// is reset and reused, so steady playback doesn't allocate. Only a chain
    /// structure change (or an instrument's first use on this channel) builds.
    pub fn sync(&mut self, devs: &[Device], owner: usize, sr: f32) {
        if self.owner == owner && same_sig(&self.sig, devs) {
            return;
        }
        if self.owner != owner && !self.states.is_empty() {
            if self.spare.len() >= SPARE_CHAINS {
                self.spare.remove(0);
            }
            let sig = std::mem::take(&mut self.sig);
            let states = std::mem::take(&mut self.states);
            self.spare.push((self.owner, sig, states));
        }
        let hit = self
            .spare
            .iter()
            .position(|(o, sig, _)| *o == owner && same_sig(sig, devs));
        if let Some(i) = hit {
            let (_, sig, mut states) = self.spare.swap_remove(i);
            for (s, d) in states.iter_mut().zip(devs) {
                s.reset(d);
            }
            self.sig = sig;
            self.states = states;
        } else {
            self.sig = devs.iter().map(|d| d.kind).collect();
            self.states = devs.iter().map(|d| FxState::new(d, sr)).collect();
        }
        self.owner = owner;
    }
    pub fn process(&mut self, mut l: f32, mut r: f32, ctx: &Ctx) -> (f32, f32) {
        for s in &mut self.states {
            (l, r) = s.process(l, r, ctx);
        }
        (l, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_sounds_like_new() {
        for kind in FxKind::ALL {
            let d = Device::new(kind);
            let mut a = FxState::new(&d, 48000.0);
            let ctx = Ctx { bpm: 140.0 };
            for i in 0..20000 {
                a.process((i as f32 * 0.37).sin(), (i as f32 * 0.21).cos(), &ctx);
            }
            a.reset(&d);
            let mut b = FxState::new(&d, 48000.0);
            for i in 0..5000 {
                let x = ((i * 7919) % 101) as f32 / 50.0 - 1.0;
                let (al, ar) = a.process(x, -x, &ctx);
                let (bl, br) = b.process(x, -x, &ctx);
                assert!(
                    (al - bl).abs() < 1e-6 && (ar - br).abs() < 1e-6,
                    "{kind:?} at {i}"
                );
            }
        }
    }

    fn noise(n: usize) -> Vec<f32> {
        let mut x = 12345u32;
        (0..n)
            .map(|i| {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                let v = (x >> 8) as f32 / (1 << 24) as f32 * 2.0 - 1.0;
                // bursts + silence + a loud impulse
                if i % 20000 < 8000 {
                    v
                } else if i % 20000 == 15000 {
                    4.0
                } else {
                    0.0
                }
            })
            .collect()
    }

    fn run(d: &Device, input: &[f32]) -> Vec<(f32, f32)> {
        let mut s = FxState::new(d, 48000.0);
        let ctx = Ctx { bpm: 145.0 };
        input
            .iter()
            .map(|&x| s.process(x, -x * 0.5, &ctx))
            .collect()
    }

    #[test]
    fn every_device_is_finite_and_bounded_with_extreme_params() {
        let input = noise(96000);
        for k in FxKind::ALL {
            for pv in [0.0, 0.5, 1.0] {
                let mut d = Device::new(k);
                d.params.iter_mut().for_each(|p| *p = pv);
                let out = run(&d, &input);
                for (l, r) in out {
                    assert!(l.is_finite() && r.is_finite(), "{k:?} {pv}");
                    assert!(l.abs() <= 8.0 && r.abs() <= 8.0, "{k:?} {pv} {l} {r}");
                }
            }
            // defaults must not blow up either and must decay after input stops
            let d = Device::new(k);
            let mut s = FxState::new(&d, 48000.0);
            let ctx = Ctx { bpm: 145.0 };
            for &x in &input[..8000] {
                s.process(x, x, &ctx);
            }
            let mut tail = 0.0f32;
            for i in 0..(48000 * 8) {
                let (l, _) = s.process(0.0, 0.0, &ctx);
                if i > 48000 * 7 {
                    tail = tail.max(l.abs());
                }
            }
            assert!(tail < 0.05, "{k:?} tail {tail}");
        }
    }

    #[test]
    fn bypass_is_dry() {
        let input = noise(20000);
        for k in FxKind::ALL {
            let mut d = Device::new(k);
            d.enabled = false;
            for (i, (l, r)) in run(&d, &input).into_iter().enumerate() {
                assert_eq!(l, input[i], "{k:?}");
                assert_eq!(r, -input[i] * 0.5, "{k:?}");
            }
        }
    }

    #[test]
    fn delay_repeats_at_tempo() {
        let mut d = Device::new(FxKind::Delay);
        d.set_real("sync", 4.0); // 1/4 at 120 BPM = 0.5 s
        d.set_real("pingpong", 0.0);
        d.set_real("mix", 1.0);
        d.set_real("lp", 18000.0);
        d.set_real("hp", 20.0);
        let mut s = FxState::new(&d, 48000.0);
        let ctx = Ctx { bpm: 120.0 };
        let mut peak_at = 0;
        let mut peak = 0.0;
        for i in 0..40000 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (l, _) = s.process(x, x, &ctx);
            if l.abs() > peak {
                peak = l.abs();
                peak_at = i;
            }
        }
        assert!((peak_at - 24000i32).abs() < 3, "{peak_at}");
    }

    #[test]
    fn params_roundtrip_and_presets_parse() {
        for k in FxKind::ALL {
            for info in params(k) {
                let n = info.to_norm(info.def);
                assert!((info.to_real(n) - info.def).abs() < 1e-2 * info.def.abs().max(1.0));
            }
            for (_, vals) in presets(k) {
                for (key, _) in *vals {
                    assert!(Device::param_index(k, key).is_some(), "{k:?} {key}");
                }
            }
        }
        let t: toml::Value =
            toml::from_str("type='delay'\ntime='1/8'\nfeedback=0.4\nping_pong=true\nmix=0.25")
                .unwrap();
        let d = from_preset(&t).unwrap();
        assert_eq!(d.real(0), 2.0);
        assert!((d.real(2) - 0.4).abs() < 1e-3);
    }
}
