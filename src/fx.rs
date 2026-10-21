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
    /// 0.2.3.11 Transform stages as devices (see crate::transform)
    Resonator,
    Digital,
    Sub,
    /// all-pass phaser (crate::fxmod)
    Phaser,
    /// Bode-style frequency shifter (crate::fxmod)
    FreqShift,
    /// 4-band parametric EQ: low shelf, two peaks, high shelf (Devices, 0.2.4)
    Eq,
    /// a modular Grid patch (`Device::grid`, crate::grid) fed by the audio
    Grid,
    /// a CLAP or VST3 effect plugin (`Device::plugin`); not in `ALL` (its menu lists
    /// the scanned effects instead). Runs in `crate::plugin::fxrack`.
    Plugin,
    /// Nested Device Chains: the sound goes down 2-4 parallel paths
    /// (`Device::paths`, each its own chain with level, pan and on / off)
    /// and is added back together. Not in `ALL`: Add Device offers it.
    Split,
}

impl FxKind {
    pub const ALL: [FxKind; 14] = [
        FxKind::Eq,
        FxKind::Delay,
        FxKind::Reverb,
        FxKind::Drive,
        FxKind::Chorus,
        FxKind::Filter,
        FxKind::Bitcrusher,
        FxKind::Compressor,
        FxKind::Resonator,
        FxKind::Digital,
        FxKind::Sub,
        FxKind::Phaser,
        FxKind::FreqShift,
        FxKind::Grid,
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
            FxKind::Resonator => "Metal resonator",
            FxKind::Digital => "Digital distortion",
            FxKind::Sub => "Sub bass",
            FxKind::Phaser => "Phaser",
            FxKind::FreqShift => "Frequency shifter",
            FxKind::Grid => "Grid FX",
            FxKind::Eq => "EQ",
            FxKind::Plugin => "Plugin effect",
            FxKind::Split => "Split",
        }
    }
    /// one-line description (Browser › Effects)
    pub fn blurb(self) -> &'static str {
        match self {
            FxKind::Delay => "echo · tempo sync · ping-pong",
            FxKind::Reverb => "space · size, damping, pre-delay",
            FxKind::Drive => "saturation · soft, hard, fold",
            FxKind::Chorus => "modulation · chorus and flanger",
            FxKind::Filter => "LP / HP / BP with an LFO",
            FxKind::Bitcrusher => "lo-fi bits and rate",
            FxKind::Compressor => "dynamics · threshold, ratio, makeup",
            FxKind::Resonator => "metallic ringing tuned to the note",
            FxKind::Digital => "digital drive, crush and ring mod",
            FxKind::Sub => "adds a sub octave under the note",
            FxKind::Phaser => "sweeping notches · 4-12 stages",
            FxKind::FreqShift => "inharmonic shift up / down",
            FxKind::Grid => "your own modular effect patch",
            FxKind::Eq => "4-band EQ · shelves and two peaks",
            FxKind::Plugin => "a CLAP or VST3 effect",
            FxKind::Split => "parallel paths · 2-4 chains mixed back together",
        }
    }
    /// muted strip tint per device type
    pub fn tint(self) -> u32 {
        match self {
            FxKind::Delay | FxKind::Chorus | FxKind::Phaser => 0x6B5E8A, // time-based: violet
            FxKind::Reverb => 0x5E5A86,
            FxKind::Drive | FxKind::Bitcrusher | FxKind::Digital => 0x8A5A4E, // warm
            FxKind::Resonator | FxKind::FreqShift => 0x7A6E4E,
            FxKind::Sub => 0x4E5E7A,
            FxKind::Filter => 0x4E6A86,     // cool
            FxKind::Compressor => 0x5E6B7A, // neutral
            FxKind::Plugin => 0x4E7A66,     // plugins: green-grey
            FxKind::Grid => 0x8A6A4E,       // the Grid's amber
            FxKind::Eq => 0x4E7A86,         // EQ: teal
            FxKind::Split => 0x7A7A5E,      // containers: olive
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
const REVERB: [ParamInfo; 7] = [
    p("size", "Size", 0.0, 1.0, 0.6, false, "%"),
    p("damping", "Damping", 0.0, 1.0, 0.5, false, "%"),
    p("predelay", "Pre-delay", 0.0, 200.0, 10.0, false, "ms"),
    p("width", "Width", 0.0, 1.0, 1.0, false, "%"),
    p("mix", "Mix", 0.0, 1.0, 0.25, false, "%"),
    // on the reverb signal only (12 dB/oct): keeps the low end out of the
    // tail (no mud under the kick and bass) and tames fizz
    p("lowcut", "Low cut", 20.0, 1000.0, 200.0, true, "Hz"),
    p("highcut", "High cut", 1000.0, 20000.0, 16000.0, true, "Hz"),
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
const RESONATOR: [ParamInfo; 8] = [
    p("mix", "Mix", 0.0, 1.0, 0.5, false, "%"),
    c(
        "ratios",
        "Ratios",
        &["harmonic", "metal", "bell", "bar", "odd"],
        1.0,
    ),
    c("pitch", "Pitch", &["note", "fixed"], 0.0),
    p("tune", "Tune", -24.0, 24.0, 0.0, false, "st"),
    p("hz", "Fixed pitch", 30.0, 4000.0, 220.0, true, "Hz"),
    p("decay", "Decay", 20.0, 4000.0, 400.0, true, "ms"),
    p("damp", "Damping", 0.0, 1.0, 0.4, false, "%"),
    p("spread", "Spread", 0.0, 1.0, 0.3, false, "%"),
];
const DIGITAL: [ParamInfo; 10] = [
    p("mix", "Mix", 0.0, 1.0, 0.5, false, "%"),
    p("drive", "Drive", 0.0, 36.0, 12.0, false, "dB"),
    c("shape", "Shape", &["clip", "fold"], 0.0),
    p("bits", "Bits", 1.0, 16.0, 8.0, false, "bit"),
    p("rate", "Rate", 300.0, 48000.0, 12000.0, true, "Hz"),
    c("alias", "Aliasing", &["off", "on"], 1.0),
    p("ring", "Ring mod", 0.0, 1.0, 0.0, false, "%"),
    p("ring_ratio", "Ring ratio", 0.25, 8.0, 1.5, true, "x"),
    p("hp", "Pre HP", 20.0, 2000.0, 80.0, true, "Hz"),
    p("lp", "Post LP", 1000.0, 20000.0, 12000.0, true, "Hz"),
];
const SUB: [ParamInfo; 6] = [
    p("mix", "Level", 0.0, 1.0, 0.5, false, "%"),
    c("mode", "Mode", &["sine", "divider", "sine + divider"], 0.0),
    c("octave", "Octave", &["-1", "-2"], 0.0),
    p("drive", "Drive", 0.0, 1.0, 0.3, false, "%"),
    p("growl", "Growl", 0.0, 1.0, 0.3, false, "%"),
    p("xover", "Crossover", 60.0, 250.0, 120.0, true, "Hz"),
];

/// Phaser LFO sync: "Hz" = the Rate knob, else one LFO cycle per this many beats
pub const PHASER_SYNC: [&str; 7] = ["Hz", "1/4", "1/2", "1 bar", "2 bars", "4 bars", "8 bars"];
const PHASER_SYNC_BEATS: [f32; 6] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
pub const PHASER: [ParamInfo; 8] = [
    c("stages", "Stages", &["4", "6", "8", "12"], 1.0),
    c("sync", "Sync", &PHASER_SYNC, 0.0),
    p("rate", "Rate", 0.02, 10.0, 0.35, true, "Hz"),
    p("depth", "Depth", 0.0, 1.0, 0.7, false, "%"),
    p("center", "Centre", 80.0, 5000.0, 800.0, true, "Hz"),
    p("feedback", "Feedback", -0.95, 0.95, 0.4, false, "%"),
    p("stereo", "Stereo phase", 0.0, 180.0, 90.0, false, "°"),
    p("mix", "Mix", 0.0, 1.0, 0.5, false, "%"),
];

/// Grid FX: the patch's 8 macros (read by its Macro modules) + dry / wet
pub const GRID_FX: [ParamInfo; 9] = [
    p("m1", "Macro 1", 0.0, 1.0, 0.0, false, "%"),
    p("m2", "Macro 2", 0.0, 1.0, 0.0, false, "%"),
    p("m3", "Macro 3", 0.0, 1.0, 0.0, false, "%"),
    p("m4", "Macro 4", 0.0, 1.0, 0.0, false, "%"),
    p("m5", "Macro 5", 0.0, 1.0, 0.0, false, "%"),
    p("m6", "Macro 6", 0.0, 1.0, 0.0, false, "%"),
    p("m7", "Macro 7", 0.0, 1.0, 0.0, false, "%"),
    p("m8", "Macro 8", 0.0, 1.0, 0.0, false, "%"),
    p("mix", "Mix", 0.0, 1.0, 1.0, false, "%"),
];

pub const FREQ_SHIFT: [ParamInfo; 5] = [
    p("coarse", "Shift", -2000.0, 2000.0, 100.0, false, "Hz"),
    p("fine", "Fine", -50.0, 50.0, 0.0, false, "Hz"),
    c(
        "output",
        "Output",
        &["up", "down", "both (L up, R down)"],
        0.0,
    ),
    p("feedback", "Feedback", 0.0, 0.95, 0.0, false, "%"),
    p("mix", "Mix", 0.0, 1.0, 0.5, false, "%"),
];

/// EQ: low shelf, peak 1, peak 2, high shelf (RBJ Audio EQ Cookbook biquads)
pub const EQ: [ParamInfo; 10] = [
    p("lo_freq", "Low freq", 20.0, 1000.0, 100.0, true, "Hz"),
    p("lo_gain", "Low gain", -18.0, 18.0, 0.0, false, "dB"),
    p("m1_freq", "Mid 1 freq", 40.0, 8000.0, 400.0, true, "Hz"),
    p("m1_gain", "Mid 1 gain", -18.0, 18.0, 0.0, false, "dB"),
    p("m1_q", "Mid 1 Q", 0.3, 8.0, 1.0, true, ""),
    p("m2_freq", "Mid 2 freq", 200.0, 16000.0, 2500.0, true, "Hz"),
    p("m2_gain", "Mid 2 gain", -18.0, 18.0, 0.0, false, "dB"),
    p("m2_q", "Mid 2 Q", 0.3, 8.0, 1.0, true, ""),
    p("hi_freq", "High freq", 1000.0, 20000.0, 8000.0, true, "Hz"),
    p("hi_gain", "High gain", -18.0, 18.0, 0.0, false, "dB"),
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
        FxKind::Resonator => &RESONATOR,
        FxKind::Digital => &DIGITAL,
        FxKind::Sub => &SUB,
        FxKind::Phaser => &PHASER,
        FxKind::FreqShift => &FREQ_SHIFT,
        FxKind::Grid => &GRID_FX,
        FxKind::Eq => &EQ,
        // plugin parameters come from the instance (ids, not indices)
        FxKind::Plugin => &[],
        // a Split's settings are per path (`FxPath`)
        FxKind::Split => &[],
    }
}

pub const MAX_PARAMS: usize = 10;

/// devices per insert chain
pub const MAX_DEVICES: usize = 16;

/// paths of a Split (D6: 2-4; one is allowed in files, as the plain sound)
pub const MAX_PATHS: usize = 4;
/// Splits inside Splits: one Split may sit inside another, no deeper
pub const MAX_SPLIT_DEPTH: usize = 2;
/// a new Split's path level: two empty paths at -6 dB add up to the
/// unchanged sound (0.5 + 0.5)
pub const NEW_PATH_DB: f32 = -6.020_6;

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
            "Hz" if v.abs() >= 1000.0 => format!("{:.2} kHz", v / 1000.0),
            "Hz" => format!("{v:.1} Hz"),
            "ms" => format!("{v:.1} ms"),
            "dB" => format!("{v:+.1} dB"),
            ":1" => format!("{v:.1}:1"),
            "bit" => format!("{v:.0} bit"),
            "°" => format!("{v:.0}°"),
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
    /// `FxKind::Plugin`: the CLAP / VST3 effect (id, path, saved state)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<Box<crate::plugin::PluginPatch>>,
    /// `FxKind::Plugin`: stable id of this device in its chain, so a moved
    /// device keeps its running instance (0 = none)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub uid: u32,
    /// `FxKind::Grid`: the patch (its AudioIn module carries the input)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<Box<crate::grid::GridP>>,
    /// `FxKind::Split`: its parallel paths (Nested Device Chains); empty
    /// for every other kind, and then not written
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<FxPath>,
}

/// one parallel path of a Split: its own chain, level, pan and on / off
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FxPath {
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default = "yes")]
    pub on: bool,
    /// level in dB (-60..+6; 0 = unchanged)
    #[serde(default)]
    pub level_db: f32,
    /// -1 left .. +1 right
    #[serde(default)]
    pub pan: f32,
}

impl FxPath {
    /// (left, right) gain: the level, then a balance pan (centre = exactly
    /// the level on both sides); 0 when off
    pub fn gains(&self) -> [f32; 2] {
        if !self.on {
            return [0.0, 0.0];
        }
        let g = 10f32.powf(self.level_db / 20.0);
        let p = self.pan.clamp(-1.0, 1.0);
        [g * (1.0 - p).min(1.0), g * (1.0 + p).min(1.0)]
    }
}

impl Default for FxPath {
    fn default() -> Self {
        Self {
            devices: vec![],
            on: true,
            level_db: 0.0,
            pan: 0.0,
        }
    }
}

fn is_zero(v: &u32) -> bool {
    *v == 0
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
            plugin: None,
            uid: 0,
            grid: (kind == FxKind::Grid).then(|| Box::new(crate::grid::fx_default())),
            paths: vec![],
        }
    }
    /// a plugin effect device for `p` (a fresh uid)
    pub fn plugin(p: crate::plugin::PluginPatch) -> Self {
        let mut d = Device::new(FxKind::Plugin);
        d.plugin = Some(Box::new(p));
        d.uid = new_uid();
        d
    }
    /// the device's display name (a plugin's own name)
    pub fn title(&self) -> String {
        match &self.plugin {
            Some(p) if self.kind == FxKind::Plugin => p.name.clone(),
            _ => self.kind.name().to_string(),
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
        if self.kind == FxKind::Grid {
            self.grid
                .get_or_insert_with(|| Box::new(crate::grid::fx_default()))
                .fix();
        } else {
            self.grid = None;
        }
        self.fix_paths(1);
    }

    /// a Split's paths after loading: 1-4 paths, levels and pans in range,
    /// no plugin effects and no Splits deeper than allowed (those are
    /// bypassed, never dropped); other kinds have none
    fn fix_paths(&mut self, depth: usize) {
        if self.kind != FxKind::Split {
            self.paths.clear();
            return;
        }
        if self.paths.is_empty() {
            self.paths.push(FxPath::default());
        }
        self.paths.truncate(MAX_PATHS);
        for p in &mut self.paths {
            p.level_db = if p.level_db.is_finite() {
                p.level_db.clamp(-60.0, 6.0)
            } else {
                0.0
            };
            p.pan = if p.pan.is_finite() {
                p.pan.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            p.devices.truncate(MAX_DEVICES);
            for d in &mut p.devices {
                let deep = d.kind == FxKind::Split && depth >= MAX_SPLIT_DEPTH;
                if d.kind == FxKind::Plugin || deep {
                    d.enabled = false;
                }
                let info = params(d.kind);
                let mut v: Vec<f32> = info.iter().map(|i| i.to_norm(i.def)).collect();
                for (x, s) in v.iter_mut().zip(&d.params) {
                    *x = s.clamp(0.0, 1.0);
                }
                d.params = v;
                if d.kind == FxKind::Grid {
                    d.grid
                        .get_or_insert_with(|| Box::new(crate::grid::fx_default()))
                        .fix();
                } else {
                    d.grid = None;
                }
                if deep {
                    // kept with its settings, but not run (no deeper nesting)
                    d.paths.truncate(MAX_PATHS);
                } else {
                    d.fix_paths(depth + 1);
                }
            }
        }
    }

    /// a new Split: two paths at -6 dB (the sound unchanged until devices
    /// go in)
    pub fn split() -> Self {
        let mut d = Device::new(FxKind::Split);
        d.paths = (0..2)
            .map(|_| FxPath {
                level_db: NEW_PATH_DB,
                ..Default::default()
            })
            .collect();
        d
    }

    /// the deepest Split nesting at and under this device (0 = none)
    pub fn split_depth(&self) -> usize {
        if self.kind != FxKind::Split {
            return 0;
        }
        1 + self
            .paths
            .iter()
            .flat_map(|p| &p.devices)
            .map(|d| d.split_depth())
            .max()
            .unwrap_or(0)
    }
    /// a plugin effect at or under this device
    pub fn has_plugin(&self) -> bool {
        self.kind == FxKind::Plugin
            || self
                .paths
                .iter()
                .flat_map(|p| &p.devices)
                .any(|d| d.has_plugin())
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
    /// a copy of a plugin device gets its own instance (built-in devices
    /// keep uid 0)
    pub fn renew_uid(&mut self) {
        if self.plugin.is_some() {
            self.uid = new_uid();
        }
    }

    pub fn param_index(kind: FxKind, key: &str) -> Option<usize> {
        params(kind).iter().position(|p| p.key == key)
    }
}

/// Where a device sits in an instrument's chain (Nested Device Chains,
/// docs/racks/PLAN.md): `[i]` is device `i` of the chain, `[i, p, j]` is
/// device `j` of path `p` of the Split at `i`, and so on (at most two
/// Splits deep). Copy, so the UI can keep it as a selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct DevPath {
    n: u8,
    s: [u8; 5],
}

impl DevPath {
    /// device `i` of the instrument's own chain
    pub fn top(i: usize) -> Self {
        let mut s = [0; 5];
        s[0] = i.min(255) as u8;
        Self { n: 1, s }
    }
    /// from a raw index list (None when it is too long or even)
    pub fn from_slice(v: &[u8]) -> Option<Self> {
        if v.is_empty() || v.len() > 5 || v.len().is_multiple_of(2) {
            return None;
        }
        let mut s = [0; 5];
        s[..v.len()].copy_from_slice(v);
        Some(Self {
            n: v.len() as u8,
            s,
        })
    }
    /// device `j` of path `p` of the Split this path points at
    pub fn child(self, p: usize, j: usize) -> Option<Self> {
        let n = self.n as usize;
        if n + 2 > 5 {
            return None;
        }
        let mut s = self.s;
        s[n] = p.min(255) as u8;
        s[n + 1] = j.min(255) as u8;
        Some(Self { n: self.n + 2, s })
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.s[..self.n as usize]
    }
    /// the device's index in its own chain
    pub fn index(&self) -> usize {
        self.s[self.n as usize - 1] as usize
    }
    /// the chain it is in: `[]` the instrument's, `[i, p]` path p of Split i
    pub fn chain(&self) -> &[u8] {
        &self.s[..self.n as usize - 1]
    }
    /// how many Splits it sits inside (0 = the instrument's own chain)
    pub fn depth(&self) -> usize {
        self.n as usize / 2
    }
}

/// the chain at `prefix` (`[]`, `[i, p]`, `[i, p, j, q]`)
pub fn chain_at<'a>(c: &'a [Device], prefix: &[u8]) -> Option<&'a [Device]> {
    match prefix {
        [] => Some(c),
        [i, p, rest @ ..] => {
            let d = c.get(*i as usize)?;
            chain_at(&d.paths.get(*p as usize)?.devices, rest)
        }
        _ => None,
    }
}

/// the chain at `prefix`, mutable
pub fn chain_at_mut<'a>(c: &'a mut Vec<Device>, prefix: &[u8]) -> Option<&'a mut Vec<Device>> {
    match prefix {
        [] => Some(c),
        [i, p, rest @ ..] => {
            let d = c.get_mut(*i as usize)?;
            chain_at_mut(&mut d.paths.get_mut(*p as usize)?.devices, rest)
        }
        _ => None,
    }
}

/// the device at `at`
pub fn dev_at<'a>(c: &'a [Device], at: &DevPath) -> Option<&'a Device> {
    chain_at(c, at.chain())?.get(at.index())
}

/// a fresh non-zero device uid (time + counter; unique within a session and
/// practically unique across songs)
pub fn new_uid() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
        .unwrap_or(1);
    let v = t
        .wrapping_mul(2654435761)
        .wrapping_add(N.fetch_add(0x9E37, Ordering::Relaxed));
    v.max(1)
}

/// Device presets (folder icon): name + real values by key.
pub fn presets(k: FxKind) -> &'static [(&'static str, &'static [(&'static str, f32)])] {
    match k {
        // Grid FX: its patches are in the patch editor (crate::grid::fx_presets)
        FxKind::Plugin | FxKind::Grid | FxKind::Split => &[],
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
        FxKind::Resonator => &[
            (
                "Robot ring",
                &[
                    ("ratios", 0.0),
                    ("decay", 250.0),
                    ("damp", 0.3),
                    ("mix", 0.5),
                ],
            ),
            (
                "Cyber metal",
                &[
                    ("ratios", 1.0),
                    ("decay", 600.0),
                    ("spread", 0.6),
                    ("mix", 0.6),
                ],
            ),
            (
                "Temple bell",
                &[
                    ("ratios", 2.0),
                    ("decay", 2500.0),
                    ("damp", 0.2),
                    ("mix", 0.4),
                ],
            ),
        ],
        FxKind::Digital => &[
            (
                "8-bit grit",
                &[
                    ("drive", 6.0),
                    ("bits", 6.0),
                    ("rate", 11025.0),
                    ("mix", 0.7),
                ],
            ),
            (
                "Ring robot",
                &[
                    ("drive", 3.0),
                    ("bits", 16.0),
                    ("rate", 48000.0),
                    ("ring", 0.8),
                    ("ring_ratio", 1.0),
                    ("mix", 0.8),
                ],
            ),
            (
                "Broken DAC",
                &[
                    ("drive", 24.0),
                    ("shape", 1.0),
                    ("bits", 4.0),
                    ("rate", 4000.0),
                    ("mix", 0.6),
                ],
            ),
        ],
        FxKind::Sub => &[
            (
                "Clean sine sub",
                &[("mode", 0.0), ("drive", 0.0), ("growl", 0.0), ("mix", 0.6)],
            ),
            (
                "Octaver growl",
                &[("mode", 1.0), ("drive", 0.6), ("growl", 0.5), ("mix", 0.6)],
            ),
        ],
        FxKind::Phaser => &[
            (
                "Slow swirl",
                &[
                    ("stages", 1.0),
                    ("sync", 0.0),
                    ("rate", 0.25),
                    ("depth", 0.8),
                    ("feedback", 0.5),
                    ("mix", 0.5),
                ],
            ),
            (
                "Disco strings (1 bar)",
                &[
                    ("stages", 2.0),
                    ("sync", 3.0),
                    ("depth", 0.7),
                    ("center", 900.0),
                    ("feedback", 0.6),
                    ("stereo", 120.0),
                    ("mix", 0.5),
                ],
            ),
            (
                "Jet 12-stage",
                &[
                    ("stages", 3.0),
                    ("sync", 0.0),
                    ("rate", 0.12),
                    ("depth", 1.0),
                    ("center", 1200.0),
                    ("feedback", 0.85),
                    ("mix", 0.5),
                ],
            ),
            (
                "Vibe (4-stage)",
                &[
                    ("stages", 0.0),
                    ("sync", 0.0),
                    ("rate", 3.5),
                    ("depth", 0.4),
                    ("feedback", -0.3),
                    ("stereo", 0.0),
                    ("mix", 0.5),
                ],
            ),
        ],
        FxKind::FreqShift => &[
            (
                "Barber pole",
                &[
                    ("coarse", 0.0),
                    ("fine", 0.4),
                    ("output", 0.0),
                    ("feedback", 0.7),
                    ("mix", 0.5),
                ],
            ),
            (
                "Metallic +170 Hz",
                &[
                    ("coarse", 170.0),
                    ("fine", 0.0),
                    ("output", 0.0),
                    ("mix", 1.0),
                ],
            ),
            (
                "Stereo detune",
                &[
                    ("coarse", 0.0),
                    ("fine", 3.0),
                    ("output", 2.0),
                    ("feedback", 0.0),
                    ("mix", 0.4),
                ],
            ),
            (
                "Alien down -600 Hz",
                &[
                    ("coarse", 600.0),
                    ("output", 1.0),
                    ("feedback", 0.4),
                    ("mix", 0.8),
                ],
            ),
        ],
        FxKind::Eq => &[
            (
                "Warm low end",
                &[
                    ("lo_freq", 120.0),
                    ("lo_gain", 4.0),
                    ("m1_freq", 350.0),
                    ("m1_gain", -3.0),
                ],
            ),
            (
                "Air",
                &[
                    ("hi_freq", 9000.0),
                    ("hi_gain", 5.0),
                    ("m2_freq", 3000.0),
                    ("m2_gain", -2.0),
                ],
            ),
            (
                "Telephone",
                &[
                    ("lo_freq", 400.0),
                    ("lo_gain", -18.0),
                    ("m1_freq", 1500.0),
                    ("m1_gain", 8.0),
                    ("m1_q", 1.2),
                    ("hi_freq", 3500.0),
                    ("hi_gain", -18.0),
                ],
            ),
            (
                "Flat",
                &[
                    ("lo_gain", 0.0),
                    ("m1_gain", 0.0),
                    ("m2_gain", 0.0),
                    ("hi_gain", 0.0),
                ],
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
        "resonator" => FxKind::Resonator,
        "digital" => FxKind::Digital,
        "sub" | "subbass" => FxKind::Sub,
        "phaser" => FxKind::Phaser,
        "freqshift" | "frequency_shifter" | "freq_shift" => FxKind::FreqShift,
        "eq" | "equalizer" | "equaliser" => FxKind::Eq,
        _ => return None,
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
        FxKind::Phaser => {
            if let Some(n) = num("stages") {
                // 2..12 stages in the factory format: the nearest choice
                let i = [4.0, 6.0, 8.0, 12.0]
                    .iter()
                    .enumerate()
                    .min_by(|a, b| (a.1 - n).abs().total_cmp(&(b.1 - n).abs()))
                    .map_or(1, |x| x.0);
                d.set_real("stages", i as f32);
            }
            if let Some(tm) = t.get("rate").and_then(|x| x.as_str()) {
                if let Some(i) = PHASER_SYNC.iter().position(|s| *s == tm) {
                    d.set_real("sync", i as f32);
                }
            } else if let Some(hz) = num("rate_hz").or(num("rate")) {
                d.set_real("sync", 0.0);
                d.set_real("rate", hz);
            }
            if let Some(v) = num("depth") {
                d.set_real("depth", v);
            }
            if let Some(v) = num("feedback") {
                d.set_real("feedback", v);
            }
        }
        FxKind::FreqShift => {
            if let Some(v) = num("shift_hz").or(num("shift")) {
                d.set_real("coarse", v.round());
                d.set_real("fine", v - v.round());
            }
            if let Some(v) = num("feedback") {
                d.set_real("feedback", v);
            }
            let o = match t.get("output").and_then(|x| x.as_str()) {
                Some("down") => 1.0,
                Some("both") => 2.0,
                _ => 0.0,
            };
            d.set_real("output", o);
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

/// biquad coefficients (normalised: a0 = 1)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biquad {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Biquad {
    const FLAT: Biquad = Biquad {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };
    /// RBJ cookbook shapes: 0 low shelf, 1 peak, 2 high shelf (S = 1 for
    /// the shelves). Gain 0 dB is exactly flat.
    pub fn rbj(shape: u8, f: f32, gain_db: f32, q: f32, sr: f32) -> Biquad {
        if gain_db.abs() < 1e-4 {
            return Biquad::FLAT;
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w = std::f32::consts::TAU * f.clamp(10.0, sr * 0.49) / sr;
        let (sn, cs) = w.sin_cos();
        let (b0, b1, b2, a0, a1, a2) = match shape {
            1 => {
                let al = sn / (2.0 * q.max(0.05));
                (
                    1.0 + al * a,
                    -2.0 * cs,
                    1.0 - al * a,
                    1.0 + al / a,
                    -2.0 * cs,
                    1.0 - al / a,
                )
            }
            _ => {
                // shelf slope S = 1
                let al = sn / 2.0 * std::f32::consts::SQRT_2;
                let sq = 2.0 * a.sqrt() * al;
                if shape == 0 {
                    (
                        a * ((a + 1.0) - (a - 1.0) * cs + sq),
                        2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                        a * ((a + 1.0) - (a - 1.0) * cs - sq),
                        (a + 1.0) + (a - 1.0) * cs + sq,
                        -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                        (a + 1.0) + (a - 1.0) * cs - sq,
                    )
                } else {
                    (
                        a * ((a + 1.0) + (a - 1.0) * cs + sq),
                        -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                        a * ((a + 1.0) + (a - 1.0) * cs - sq),
                        (a + 1.0) - (a - 1.0) * cs + sq,
                        2.0 * ((a - 1.0) - (a + 1.0) * cs),
                        (a + 1.0) - (a - 1.0) * cs - sq,
                    )
                }
            }
        };
        Biquad {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
    /// |H| at frequency `f` (for the EQ curve)
    pub fn mag(&self, f: f32, sr: f32) -> f32 {
        let w = std::f32::consts::TAU * f / sr;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = -(self.b1 * s1 + self.b2 * s2);
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = -(self.a1 * s1 + self.a2 * s2);
        ((nr * nr + ni * ni) / (dr * dr + di * di).max(1e-12)).sqrt()
    }
    /// transposed direct form II, state `z`
    #[inline]
    fn run(&self, x: f32, z: &mut [f32; 2]) -> f32 {
        let y = self.b0 * x + z[0];
        z[0] = self.b1 * x - self.a1 * y + z[1];
        z[1] = self.b2 * x - self.a2 * y;
        y
    }
}

/// the 4 EQ bands for real parameter values `p` (`EQ` order)
pub fn eq_bands(p: &[f32], sr: f32) -> [Biquad; 4] {
    let g = |i: usize| p.get(i).copied().unwrap_or(0.0);
    [
        Biquad::rbj(0, g(0), g(1), 0.707, sr),
        Biquad::rbj(1, g(2), g(3), g(4), sr),
        Biquad::rbj(1, g(5), g(6), g(7), sr),
        Biquad::rbj(2, g(8), g(9), 0.707, sr),
    ]
}

/// the EQ's response in dB at `f` for a device's normalised params
#[cfg(test)]
pub fn eq_response_db(d: &Device, f: f32, sr: f32) -> f32 {
    let real: Vec<f32> = (0..EQ.len()).map(|i| d.real(i)).collect();
    let m: f32 = eq_bands(&real, sr).iter().map(|b| b.mag(f, sr)).product();
    20.0 * m.max(1e-6).log10()
}

enum Dsp {
    /// a plugin effect: runs in `crate::plugin::fxrack` (see ChainState::process_with)
    Plugin,
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
        /// wet low cut / high cut (L, R)
        cut: [[Svf; 2]; 2],
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
    Stage(Box<crate::transform::FxStage>),
    Phaser(crate::fxmod::Phaser),
    FreqShift(crate::fxmod::FreqShifter),
    /// 4 biquads, (L, R) state per band, coefficients for `last`, and a
    /// countdown to the next coefficient check (every 16 samples)
    Eq {
        c: [Biquad; 4],
        z: [[[f32; 2]; 2]; 4],
        last: [f32; 10],
        n: u32,
    },
    /// the device's patch (a copy: structure changes rebuild the chain,
    /// knob values are copied in place) and its state
    Grid(Box<(crate::grid::GridP, crate::grid::GridState)>),
    /// a Split: one chain of states per path
    Split(Box<SplitDsp>),
}

/// Cache of pure per-parameter maths (`f(x)` with `sr` fixed per state),
/// keyed on the input's bits: an unchanged parameter skips the libm call.
#[derive(Default, Clone, Copy)]
struct Memo {
    key: [u32; MEMO_SLOTS],
    val: [f32; MEMO_SLOTS],
    set: u16,
    /// tests: always recompute (the reference the cache must match)
    #[cfg(test)]
    off: bool,
}

const MEMO_SLOTS: usize = 6;

impl Memo {
    #[inline]
    fn get(&mut self, slot: usize, x: f32, f: impl FnOnce(f32) -> f32) -> f32 {
        #[cfg(test)]
        if self.off {
            return f(x);
        }
        let b = x.to_bits();
        if self.set & (1 << slot) != 0 && self.key[slot] == b {
            return self.val[slot];
        }
        let v = f(x);
        self.key[slot] = b;
        self.val[slot] = v;
        self.set |= 1 << slot;
        v
    }
}

/// a Split's paths (built with the device, never in the audio callback)
pub struct SplitDsp {
    paths: Vec<PathDsp>,
    /// gain smoothing per sample (5 ms)
    k: f32,
}

struct PathDsp {
    states: Vec<FxState>,
    g: [f32; 2],
    tg: [f32; 2],
}

impl SplitDsp {
    fn new(d: &Device, sr: f32) -> Self {
        Self {
            paths: d
                .paths
                .iter()
                .map(|p| {
                    let tg = p.gains();
                    PathDsp {
                        states: p.devices.iter().map(|x| FxState::new(x, sr)).collect(),
                        g: tg,
                        tg,
                    }
                })
                .collect(),
            k: 1.0 - (-1.0 / (0.005 * sr)).exp(),
        }
    }
    fn set_targets(&mut self, d: &Device) {
        for (pd, p) in self.paths.iter_mut().zip(&d.paths) {
            pd.tg = p.gains();
            for (st, x) in pd.states.iter_mut().zip(&p.devices) {
                st.set_targets_plain(x);
            }
        }
    }
    fn reset(&mut self, d: &Device) {
        for (pd, p) in self.paths.iter_mut().zip(&d.paths) {
            pd.tg = p.gains();
            pd.g = pd.tg;
            for (st, x) in pd.states.iter_mut().zip(&p.devices) {
                if st.kind == x.kind {
                    st.reset(x);
                }
            }
        }
    }
    /// every path on the same input, added back together (one path at
    /// 0 dB, centre, is bit for bit its own chain)
    fn run(&mut self, l: f32, r: f32, ctx: &Ctx) -> (f32, f32) {
        let (mut ol, mut or) = (0.0, 0.0);
        let k = self.k;
        for pd in &mut self.paths {
            for c in 0..2 {
                if pd.g[c] != pd.tg[c] {
                    pd.g[c] += (pd.tg[c] - pd.g[c]) * k;
                    if (pd.g[c] - pd.tg[c]).abs() < 1e-6 {
                        pd.g[c] = pd.tg[c];
                    }
                }
            }
            if pd.g == [0.0, 0.0] && pd.tg == [0.0, 0.0] {
                continue;
            }
            let (mut a, mut b) = (l, r);
            for st in &mut pd.states {
                (a, b) = st.process(a, b, ctx);
            }
            ol += a * pd.g[0];
            or += b * pd.g[1];
        }
        (ol, or)
    }
}

/// no automation override (nested devices, D8)
fn no_over(_: usize) -> Option<f32> {
    None
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
    /// last inputs/outputs of the per-sample coefficient maths (tan, exp,
    /// powf of a parameter). A parameter that holds still reuses its value:
    /// same bits in, same bits out, so the sound is unchanged.
    memo: Memo,
    /// plugin effects: the device uid and its slot in the engine's
    /// `FxRack` (`u16::MAX` = no running instance: passed through)
    pub uid: u32,
    pub plug: u16,
}

pub struct Ctx {
    pub bpm: f32,
    /// pitch of the note playing on this channel (0 = none); the
    /// Resonator, Digital ring mod and Sub sine follow it
    pub note: f32,
}

impl FxState {
    pub fn new(d: &Device, sr: f32) -> Self {
        let n = |s: f32| (s * sr) as usize + 4;
        let scale = sr / 44100.0;
        let dsp = match d.kind {
            FxKind::Plugin => Dsp::Plugin,
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
                    cut: Default::default(),
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
            FxKind::Resonator | FxKind::Digital | FxKind::Sub => Dsp::Stage(Box::default()),
            FxKind::Phaser => Dsp::Phaser(Default::default()),
            FxKind::FreqShift => Dsp::FreqShift(Default::default()),
            FxKind::Eq => Dsp::Eq {
                c: [Biquad::FLAT; 4],
                z: [[[0.0; 2]; 2]; 4],
                last: [f32::NAN; 10],
                n: 0,
            },
            FxKind::Grid => Dsp::Grid(Box::new((
                d.grid
                    .as_deref()
                    .cloned()
                    .unwrap_or_else(crate::grid::fx_default),
                Default::default(),
            ))),
            FxKind::Split => Dsp::Split(Box::new(SplitDsp::new(d, sr))),
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
            memo: Memo::default(),
            uid: d.uid,
            plug: u16::MAX,
        };
        s.set_targets(d, |_| None);
        s.cur = s.tgt;
        s
    }

    /// Make this state sound like `FxState::new(d, sr)` again without
    /// allocating: buffers are zeroed in place (kind must match).
    pub fn reset(&mut self, d: &Device) {
        debug_assert_eq!(self.kind, d.kind);
        self.uid = d.uid;
        self.plug = u16::MAX;
        match &mut self.dsp {
            Dsp::Plugin => {}
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
                cut,
            } => {
                *cut = Default::default();
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
            Dsp::Stage(st) => **st = crate::transform::FxStage::default(),
            Dsp::Phaser(ph) => *ph = Default::default(),
            Dsp::FreqShift(f) => *f = Default::default(),
            Dsp::Grid(b) => b.1 = Default::default(),
            Dsp::Split(sp) => sp.reset(d),
            Dsp::Eq { c, z, last, n } => {
                *c = [Biquad::FLAT; 4];
                *z = [[[0.0; 2]; 2]; 4];
                *last = [f32::NAN; 10];
                *n = 0;
            }
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
        // the Grid's knob values (same length: no allocation)
        if let (Dsp::Grid(b), Some(g)) = (&mut self.dsp, d.grid.as_deref()) {
            if b.0.p.len() == g.p.len() {
                b.0.p.copy_from_slice(&g.p);
            }
            b.0.level = g.level;
        }
        // a Split's paths: their gains and their devices' own values
        if let Dsp::Split(sp) = &mut self.dsp {
            sp.set_targets(d);
        }
    }

    /// `set_targets` without automation (devices inside a Split, D8)
    pub fn set_targets_plain(&mut self, d: &Device) {
        self.set_targets(d, no_over);
    }

    /// a compressor's current gain reduction in dB (0 when idle or not a
    /// compressor); read by the Devices meter, never changes the state
    pub fn comp_gr_db(&self) -> Option<f32> {
        let Dsp::Comp { env } = &self.dsp else {
            return None;
        };
        if !self.on {
            return Some(0.0);
        }
        let lvl = 20.0 * env.max(1e-6).log10();
        let over = lvl - self.cur[0];
        Some(if over > 0.0 {
            over * (1.0 - 1.0 / self.cur[1].max(1.0))
        } else {
            0.0
        })
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
        let memo = &mut self.memo;
        match &mut self.dsp {
            Dsp::Plugin => (l, r),
            Dsp::Split(sp) => sp.run(l, r, ctx),
            Dsp::Eq { c, z, last, n } => {
                if *n == 0 {
                    *n = 16;
                    if *last != p {
                        *last = p;
                        *c = eq_bands(&p, sr);
                    }
                } else {
                    *n -= 1;
                }
                let (mut yl, mut yr) = (l, r);
                for (b, zz) in c.iter().zip(z.iter_mut()) {
                    yl = b.run(yl, &mut zz[0]);
                    yr = b.run(yr, &mut zz[1]);
                }
                (yl, yr)
            }
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
                let alp = memo.get(0, p[5], |f| pole_coef(f, sr));
                let ahp = memo.get(1, p[4], |f| pole_coef(f, sr));
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
                cut,
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
                // wet-only low cut / high cut (Butterworth, k = sqrt 2)
                let gl = memo.get(0, p[5], |f| {
                    (std::f32::consts::PI * f.clamp(10.0, sr * 0.45) / sr).tan()
                });
                let gh = memo.get(1, p[6], |f| {
                    (std::f32::consts::PI * f.clamp(10.0, sr * 0.45) / sr).tan()
                });
                let k = std::f32::consts::SQRT_2;
                let hl = cut[0][0].run(yl, gl, k).2;
                let hr = cut[0][1].run(yr, gl, k).2;
                let yl = cut[1][0].run(hl, gh, k).0;
                let yr = cut[1][1].run(hr, gh, k).0;
                let m = p[4];
                (l * (1.0 - m) + yl * m, r * (1.0 - m) + yr * m)
            }
            Dsp::Drive { tone } => {
                let g = memo.get(0, p[1], db);
                let shape = |x: f32| match p[0] as usize {
                    0 => (x * g).tanh(),
                    1 => (x * g).clamp(-1.0, 1.0),
                    _ => {
                        // triangle fold, in phase with the input (up to
                        // 0.2.3.6 this was inverted, so a dry/wet blend
                        // cancelled instead of thickening)
                        let y = x * g * 0.5;
                        1.0 - ((y + 1.0).rem_euclid(4.0) - 2.0).abs()
                    }
                };
                let a = memo.get(1, p[2], |f| pole_coef(f, sr));
                let out = memo.get(2, p[3], db);
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
                // no sweep: 2^(0 * sin) is exactly 1, skip the sin/powf
                let sweep = if p[4] == 0.0 {
                    1.0
                } else {
                    2f32.powf(p[4] * (*ph * std::f32::consts::TAU).sin())
                };
                let fc = (p[1] * sweep).clamp(20.0, sr * 0.45);
                let g = memo.get(0, fc, |fc| (std::f32::consts::PI * fc / sr).tan());
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
                    memo.get(0, p[2], |ms| 1.0 - (-1.0 / (ms / 1000.0 * sr)).exp())
                } else {
                    memo.get(1, p[3], |ms| 1.0 - (-1.0 / (ms / 1000.0 * sr)).exp())
                };
                *env += (x - *env) * a;
                let lvl = 20.0 * env.max(1e-6).log10();
                let over = lvl - p[0];
                let gr = if over > 0.0 {
                    over * (1.0 - 1.0 / p[1])
                } else {
                    0.0
                };
                let g = memo.get(2, p[4] - gr, db);
                (l * g, r * g)
            }
            Dsp::Phaser(ph) => {
                let sync = p[1] as usize;
                let rate = if sync == 0 {
                    p[2]
                } else {
                    ctx.bpm.max(20.0) / 60.0 / PHASER_SYNC_BEATS[(sync - 1).min(5)]
                };
                let pp = crate::fxmod::PhaserParams {
                    stages: crate::fxmod::STAGES[(p[0] as usize).min(3)],
                    rate,
                    depth: p[3],
                    center: p[4],
                    feedback: p[5],
                    stereo: p[6],
                    mix: p[7],
                };
                ph.process(l, r, &pp, sr)
            }
            Dsp::FreqShift(f) => {
                use crate::fxmod::ShiftOut;
                let sp = crate::fxmod::ShiftParams {
                    hz: p[0] + p[1],
                    out: match p[2] as usize {
                        0 => ShiftOut::Up,
                        1 => ShiftOut::Down,
                        _ => ShiftOut::Both,
                    },
                    feedback: p[3],
                    mix: p[4],
                };
                f.process(l, r, &sp, sr)
            }
            Dsp::Grid(b) => {
                let (g, st) = &mut **b;
                let mut mac = [0.0f32; 8];
                mac.copy_from_slice(&p[..8]);
                let ov = crate::gen::Ov {
                    mac: &mac,
                    knob: &crate::gen::NO_KNOBS,
                };
                let f = if ctx.note > 0.0 {
                    ctx.note
                } else {
                    crate::grid::C4
                };
                let (wl, wr, _) = st.render(g, ov, f, sr, true, 1.0, (l, r));
                (l + (wl - l) * p[8], r + (wr - r) * p[8])
            }
            Dsp::Stage(st) => match self.kind {
                FxKind::Resonator => st.resonator(l, r, &p, ctx.note, sr),
                FxKind::Digital => st.digital(l, r, &p, ctx.note, sr),
                _ => st.sub(l, r, &p, ctx.note, sr),
            },
        }
    }
}

/// a chain's structure: per device its kind and Grid patch fingerprint
type Sig = Vec<(FxKind, u64)>;

/// A chain of device states mirroring a `[Device]` list.
#[derive(Default)]
pub struct ChainState {
    /// structure signature (kinds) the states were built for
    sig: Sig,
    pub states: Vec<FxState>,
    pub owner: usize,
    /// chains of instruments that played on this channel before, kept so a
    /// mid-song instrument switch reuses their buffers instead of allocating
    spare: Vec<(usize, Sig, Vec<FxState>)>,
}

/// spare chains kept per channel
const SPARE_CHAINS: usize = 6;

/// a device's structure: its kind (+ a Grid patch's modules and cables)
fn dev_sig(d: &Device) -> (FxKind, u64) {
    let fp = match (&d.kind, &d.grid) {
        (FxKind::Grid, Some(g)) => g.fingerprint(),
        _ if d.kind == FxKind::Split => split_fp(d),
        _ => 0,
    };
    (d.kind, fp)
}

/// a Split's structure: its paths and the kinds (and patches) in them
fn split_fp(d: &Device) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    mix(d.paths.len() as u64);
    for p in &d.paths {
        mix(0xA5 ^ p.devices.len() as u64);
        for x in &p.devices {
            let (k, fp) = dev_sig(x);
            mix(k as u64);
            mix(fp);
            mix(x.uid as u64);
        }
    }
    h
}

fn same_sig(sig: &[(FxKind, u64)], devs: &[Device]) -> bool {
    sig.len() == devs.len() && sig.iter().zip(devs).all(|(k, d)| *k == dev_sig(d))
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
        if self.owner == owner && !self.states.is_empty() {
            // the chain was edited (a device inserted, removed or moved):
            // the devices that stay keep their state (delay and reverb
            // tails, plugin slots); only new devices are built
            let sig = std::mem::take(&mut self.sig);
            let mut old: Vec<Option<FxState>> = std::mem::take(&mut self.states)
                .into_iter()
                .map(Some)
                .collect();
            self.states = devs
                .iter()
                .map(|d| {
                    let ds = dev_sig(d);
                    let k = (0..old.len())
                        .find(|&k| sig[k] == ds && old[k].as_ref().is_some_and(|s| s.uid == d.uid));
                    match k.and_then(|k| old[k].take()) {
                        Some(s) => s,
                        None => FxState::new(d, sr),
                    }
                })
                .collect();
            self.sig = devs.iter().map(dev_sig).collect();
            return;
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
            self.sig = devs.iter().map(dev_sig).collect();
            self.states = devs.iter().map(|d| FxState::new(d, sr)).collect();
        }
        self.owner = owner;
    }
    /// Instruments were removed or inserted: follow the new indices so each
    /// chain stays with its instrument (`None` = instrument gone).
    pub fn remap(&mut self, f: &dyn Fn(usize) -> Option<usize>) {
        if self.owner != usize::MAX {
            match f(self.owner) {
                Some(o) => self.owner = o,
                None => {
                    // the states cannot belong to anyone else: rebuild
                    self.owner = usize::MAX;
                    self.sig.clear();
                    self.states.clear();
                }
            }
        }
        self.spare.retain_mut(|(o, _, _)| match f(*o) {
            Some(n) => {
                *o = n;
                true
            }
            None => false,
        });
    }
    pub fn process(&mut self, mut l: f32, mut r: f32, ctx: &Ctx) -> (f32, f32) {
        for s in &mut self.states {
            (l, r) = s.process(l, r, ctx);
        }
        (l, r)
    }
    /// like `process`, with plugin devices running in `rack` (a plugin device
    /// without a running instance passes the audio through)
    pub fn process_with(
        &mut self,
        mut l: f32,
        mut r: f32,
        ctx: &Ctx,
        rack: &mut crate::plugin::fxrack::FxRack,
    ) -> (f32, f32) {
        for s in &mut self.states {
            (l, r) = if s.kind == FxKind::Plugin {
                if s.plug == u16::MAX {
                    (l, r)
                } else {
                    let step = 1.0 / (0.005 * s.sr);
                    rack.process(s.plug, l, r, s.on, step)
                }
            } else {
                s.process(l, r, ctx)
            };
        }
        (l, r)
    }
}

#[cfg(test)]
mod tests {
    /// the coefficient cache changes nothing: same bits with and without it,
    /// for still and for moving (automated) parameters
    #[test]
    fn coefficient_memo_is_bit_identical() {
        let kinds = [
            FxKind::Filter,
            FxKind::Compressor,
            FxKind::Delay,
            FxKind::Drive,
            FxKind::Reverb,
        ];
        for kind in kinds {
            for sweep in [0.0f32, 0.6] {
                let mut d = Device::new(kind);
                for (i, v) in d.params.iter_mut().enumerate() {
                    *v = (0.3 + 0.13 * i as f32).fract();
                }
                if kind == FxKind::Filter {
                    d.params[4] = sweep;
                }
                let mut a = FxState::new(&d, 48000.0);
                let mut b = FxState::new(&d, 48000.0);
                b.memo.off = true;
                let ctx = Ctx {
                    bpm: 140.0,
                    note: 220.0,
                };
                let mut seed = 0x1234_5678u32;
                for n in 0..30_000 {
                    if n % 7_000 == 3_500 {
                        // automation: a parameter moves for a while
                        let m = (n / 7_000) as f32 * 0.21;
                        let over = |i: usize| (i == 1).then_some((0.2 + m).fract());
                        a.set_targets(&d, over);
                        b.set_targets(&d, over);
                    }
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let x = (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
                    let (al, ar) = a.process(x, -x * 0.5, &ctx);
                    let (bl, br) = b.process(x, -x * 0.5, &ctx);
                    assert_eq!(
                        (al.to_bits(), ar.to_bits()),
                        (bl.to_bits(), br.to_bits()),
                        "{kind:?} sweep {sweep} sample {n}"
                    );
                }
            }
        }
    }

    use super::*;

    #[test]
    fn eq_flat_is_transparent_and_bands_shape_the_curve() {
        let d = Device::new(FxKind::Eq);
        let mut st = FxState::new(&d, 48000.0);
        let ctx = Ctx {
            bpm: 120.0,
            note: 0.0,
        };
        for i in 0..2000 {
            let x = ((i as f32) * 0.07).sin() * 0.5;
            let (l, r) = st.process(x, -x, &ctx);
            assert_eq!((l, r), (x, -x), "0 dB on every band passes bit-exact");
        }
        let mut b = Device::new(FxKind::Eq);
        b.set_real("m1_freq", 1000.0);
        b.set_real("m1_gain", 12.0);
        b.set_real("lo_gain", -12.0);
        let at = |f| eq_response_db(&b, f, 48000.0);
        assert!((at(1000.0) - 12.0).abs() < 0.3, "peak {}", at(1000.0));
        assert!(at(25.0) < -10.0, "low shelf cut {}", at(25.0));
        assert!(at(15000.0).abs() < 0.5);
        // the DSP follows the curve: a 1 kHz sine comes out ~4x louder
        let mut st = FxState::new(&b, 48000.0);
        let mut peak = 0.0f32;
        for i in 0..48000 {
            let x = (i as f32 / 48000.0 * 1000.0 * std::f32::consts::TAU).sin() * 0.1;
            let (l, _) = st.process(x, x, &ctx);
            if i > 24000 {
                peak = peak.max(l.abs());
            }
        }
        assert!((peak / 0.1 - 3.98).abs() < 0.2, "gain {}", peak / 0.1);
    }

    #[test]
    fn editing_a_chain_keeps_the_states_of_devices_that_stay() {
        let ctx = Ctx {
            bpm: 120.0,
            note: 0.0,
        };
        let delay = Device::new(FxKind::Delay);
        let mut c = ChainState {
            sig: vec![],
            states: vec![],
            owner: usize::MAX,
            spare: vec![],
        };
        c.sync(std::slice::from_ref(&delay), 2, 48000.0);
        // feed an impulse so the delay holds a tail
        c.process(1.0, 1.0, &ctx);
        for _ in 0..100 {
            c.process(0.0, 0.0, &ctx);
        }
        // insert an EQ in front: the delay's buffer must survive
        let devs = [Device::new(FxKind::Eq), delay.clone()];
        c.sync(&devs, 2, 48000.0);
        assert_eq!(c.states.len(), 2);
        assert_eq!(c.states[0].kind, FxKind::Eq);
        let mut heard = 0.0f32;
        for _ in 0..48000 {
            let (l, _) = c.process(0.0, 0.0, &ctx);
            heard = heard.max(l.abs());
        }
        assert!(heard > 0.01, "the echo of the impulse still comes out");
        // remove the EQ again: still the same delay state, no rebuild
        c.sync(std::slice::from_ref(&delay), 2, 48000.0);
        assert_eq!(c.states.len(), 1);
        assert_eq!(c.states[0].kind, FxKind::Delay);
    }

    #[test]
    fn chain_remap_follows_instrument_indices() {
        let d = [Device::new(FxKind::Delay)];
        let mut c = ChainState {
            sig: vec![],
            states: vec![],
            owner: usize::MAX,
            spare: vec![],
        };
        c.sync(&d, 3, 48000.0);
        c.sync(&d, 5, 48000.0); // 3 goes to the spares
                                // instrument 1 deleted: 3 -> 2, 5 -> 4
        c.remap(&|i| {
            if i == 1 {
                None
            } else {
                Some(if i > 1 { i - 1 } else { i })
            }
        });
        assert_eq!(c.owner, 4);
        assert_eq!(c.spare[0].0, 2);
        // the owner itself deleted: chain dropped, spares kept
        c.remap(&|i| (i != 4).then_some(i));
        assert_eq!(c.owner, usize::MAX);
        assert!(c.states.is_empty());
        assert_eq!(c.spare.len(), 1);
    }

    #[test]
    fn reset_sounds_like_new() {
        for kind in FxKind::ALL {
            let d = Device::new(kind);
            let mut a = FxState::new(&d, 48000.0);
            let ctx = Ctx {
                bpm: 140.0,
                note: 0.0,
            };
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
        let ctx = Ctx {
            bpm: 145.0,
            note: 0.0,
        };
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
            let ctx = Ctx {
                bpm: 145.0,
                note: 0.0,
            };
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
        let ctx = Ctx {
            bpm: 120.0,
            note: 0.0,
        };
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

    #[test]
    fn reverb_low_cut_keeps_the_bass_out_of_the_tail() {
        let sine = |hz: f32| -> Vec<f32> {
            (0..48000)
                .map(|i| (i as f32 * hz / 48000.0 * std::f32::consts::TAU).sin() * 0.5)
                .collect()
        };
        let tail_rms = |d: &Device, x: &[f32]| {
            let o = run(d, x);
            let s: f32 = o[24000..].iter().map(|(l, r)| l * l + r * r).sum();
            (s / 24000.0).sqrt()
        };
        let mut wet = Device::new(FxKind::Reverb);
        wet.set_real("mix", 1.0);
        assert!((wet.real(5) - 200.0).abs() < 0.5, "default low cut 200 Hz");
        let mut open = wet.clone();
        open.set_real("lowcut", 20.0);
        open.set_real("highcut", 20000.0);
        let bass = sine(60.0);
        let db = 20.0 * (tail_rms(&wet, &bass) / tail_rms(&open, &bass)).log10();
        assert!(
            db < -12.0,
            "60 Hz in the tail {db:.1} dB with the 200 Hz low cut"
        );
        let mid = sine(1000.0);
        let db = 20.0 * (tail_rms(&wet, &mid) / tail_rms(&open, &mid)).log10();
        assert!(db.abs() < 1.5, "1 kHz untouched ({db:.1} dB)");
        let mut dark = wet.clone();
        dark.set_real("highcut", 1500.0);
        let air = sine(8000.0);
        let db = 20.0 * (tail_rms(&dark, &air) / tail_rms(&wet, &air)).log10();
        assert!(db < -12.0, "high cut {db:.1} dB");
    }

    #[test]
    fn old_reverbs_load_with_the_new_cut_params() {
        // a 0.2.3.6 reverb has 5 params; fix() appends the defaults
        let mut d = Device {
            kind: FxKind::Reverb,
            enabled: true,
            collapsed: false,
            params: vec![0.5, 0.5, 0.1, 1.0, 1.0],
            plugin: None,
            uid: 0,
            grid: None,
            paths: vec![],
        };
        d.fix();
        assert_eq!(d.params.len(), 7);
        assert_eq!(d.params[..5], [0.5, 0.5, 0.1, 1.0, 1.0]);
        assert!((d.real(5) - 200.0).abs() < 0.5 && (d.real(6) - 16000.0).abs() < 5.0);
    }

    #[test]
    fn fold_is_in_phase_so_blends_do_not_cancel() {
        let x: Vec<f32> = (0..4800)
            .map(|i| (i as f32 * 220.0 / 48000.0 * std::f32::consts::TAU).sin() * 0.3)
            .collect();
        let fold = |drive: f32, mix: f32| {
            let mut d = Device::new(FxKind::Drive);
            d.set_real("mode", 2.0);
            d.set_real("drive", drive);
            d.set_real("tone", 18000.0);
            d.set_real("output", 0.0);
            d.set_real("mix", mix);
            run(&d, &x)
        };
        let corr = |o: &[(f32, f32)]| {
            o.iter()
                .zip(&x)
                .skip(480)
                .map(|((l, _), v)| l * v)
                .sum::<f32>()
        };
        let e = |o: &[(f32, f32)]| o.iter().skip(480).map(|(l, _)| l * l).sum::<f32>();
        // below the fold point the fold is the identity (was: inverted)
        assert!(corr(&fold(0.0, 1.0)) > 0.0);
        // 50 % blend at low drive adds up instead of cancelling (the fold
        // stage halves the level: wet = 0.5 x, blend = 0.75 x; it was 0.25 x)
        let blend = e(&fold(0.0, 0.5));
        let dry = x.iter().skip(480).map(|v| v * v).sum::<f32>();
        assert!(blend > 0.5 * dry, "{blend} vs {dry}");
        // 100 % wet: same sound as before up to polarity (old songs keep
        // their sound): |new| == |old fold|
        let o = fold(12.0, 1.0);
        let g = 10f32.powf(12.0 / 20.0);
        let mut tone = OnePole::default();
        let a = pole_coef(18000.0, 48000.0);
        for (i, ((l, _), v)) in o.iter().zip(&x).enumerate().skip(2000) {
            let y = v * g * 0.5;
            let old = ((y + 1.0).rem_euclid(4.0) - 2.0).abs() - 1.0;
            let want = -tone.lp(old, a);
            let _ = i;
            assert!((l - want).abs() < 0.05, "{l} vs {want}");
        }
    }

    #[test]
    fn compressor_reports_gain_reduction_for_its_meter() {
        let mut d = Device::new(FxKind::Compressor);
        d.set_real("threshold", -30.0);
        d.set_real("ratio", 8.0);
        let mut st = FxState::new(&d, 48000.0);
        let ctx = Ctx {
            bpm: 120.0,
            note: 0.0,
        };
        assert_eq!(st.comp_gr_db(), Some(0.0));
        for n in 0..9600 {
            let x = 0.8 * (n as f32 * 0.05).sin();
            st.process(x, x, &ctx);
        }
        let gr = st.comp_gr_db().unwrap();
        assert!(gr > 6.0, "{gr}");
        // other devices have no meter
        assert_eq!(
            FxState::new(&Device::new(FxKind::Delay), 48000.0).comp_gr_db(),
            None
        );
    }
}
