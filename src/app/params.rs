//! Parameter descriptors for the lower-panel instrument editor (0.2.3.19).
//!
//! Every editable instrument property is described once here: which tab and
//! card it sits on, its range, default, unit, value text, automation target
//! and how to read / write it on an [`Instrument`]. The panel (instpanel.rs)
//! draws knobs, dropdowns and toggles from these, parameter search uses the
//! names, and the tests below enumerate them per instrument type so no
//! property of the old sidebar editor can get lost.
use crate::automation::Target;
use crate::gen::{DrumKind, DrumP, GenEngine, GenPatch, MacroDef};
use crate::model::{Instrument, InstrumentKind};
use crate::synth::{FilterMode, Patch, Wave};
use crate::voice::VoicePatch;
use std::sync::OnceLock;

pub(crate) type Get = Box<dyn Fn(&Instrument) -> f32>;
pub(crate) type Set = Box<dyn Fn(&mut Instrument, f32)>;
pub(crate) type Pred = Box<dyn Fn(&Instrument) -> bool>;
pub(crate) type Fmt = Box<dyn Fn(f32) -> String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub(crate) enum Tab {
    #[default]
    Main,
    Modulation,
    Fx,
    Advanced,
    /// the Grid generator's patching canvas
    Grid,
    /// the poly synth's own editor (named `poly::POLY_DISPLAY_NAME`)
    Synth,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Synth,
        Tab::Grid,
        Tab::Main,
        Tab::Modulation,
        Tab::Fx,
        Tab::Advanced,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Tab::Grid => "Grid",
            Tab::Synth => crate::poly::POLY_DISPLAY_NAME,
            Tab::Main => "Main",
            Tab::Modulation => "Modulation",
            Tab::Fx => "FX",
            Tab::Advanced => "Advanced",
        }
    }
}

/// widgets that are more than one value (drawn by instpanel.rs)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Custom {
    /// vowel palette + sequence chips
    Vowels,
    /// the Voice's words list
    Words,
    /// drum kit: which piece is shown
    KitPiece,
    /// Dice / Mutate / history / seed / Bake to sample
    Dice,
    /// Chip FX category buttons + Randomize
    ChipCategories,
    /// plugin: name, status, Configure…, Reload
    PluginStatus,
    /// plugin: the 8 macro knobs and their assignments
    PluginMacros,
    /// plugin: every parameter (searchable list)
    PluginParams,
    /// sample: file, length, base note, Open in Sample editor
    SampleInfo,
    /// FX chain: add effect, open the FX view
    FxChain,
    /// drag-to-modulate: the selected modulator's settings
    Modulators,
    /// Wave generator: import / clear a table for oscillator 0 or 1
    WaveTable(u8),
    /// Grid generator: the patching canvas (drawn over the whole tab)
    GridCanvas,
    /// the poly synth's editor (drawn over the whole tab)
    PolyPanel,
    /// drag-to-modulate: every routing with its depth
    Routings,
}

pub(crate) enum Ctl {
    /// a knob over `lo..hi` in real units; `step` 0 = continuous
    Knob {
        lo: f32,
        hi: f32,
        log: bool,
        step: f32,
        def: f32,
    },
    /// a dropdown / segmented choice: the value is the index
    Choice(Vec<String>),
    /// on / off (0 / 1)
    Toggle,
    Custom(Custom),
}

pub(crate) type LockFns = (Pred, Box<dyn Fn(&mut Instrument)>);

pub(crate) struct Param {
    pub tab: Tab,
    pub card: String,
    /// stable id (tests, search, jump)
    pub key: String,
    pub name: String,
    pub tip: String,
    pub unit: &'static str,
    pub ctl: Ctl,
    pub get: Get,
    pub set: Set,
    pub fmt: Option<Fmt>,
    pub target: Option<Target>,
    /// (is locked, toggle the lock) for the Dice locks
    pub lock: Option<LockFns>,
    pub enabled: Option<Pred>,
    /// small tag on the knob ("M1" = 43xy macro 1)
    pub badge: Option<String>,
}

impl Param {
    pub fn is_on(&self, i: &Instrument) -> bool {
        self.enabled.as_ref().is_none_or(|f| f(i))
    }
    /// value text in real units
    pub fn text(&self, v: f32) -> String {
        if let Some(f) = &self.fmt {
            return f(v);
        }
        match &self.ctl {
            Ctl::Choice(c) => c.get(v as usize).cloned().unwrap_or_default(),
            Ctl::Toggle => if v > 0.5 { "on" } else { "off" }.into(),
            Ctl::Knob { step, .. } if *step >= 1.0 => {
                format!("{}{}", v.round() as i32, unit_sfx(self.unit))
            }
            Ctl::Knob { .. } => fmt_unit(v, self.unit),
            Ctl::Custom(_) => String::new(),
        }
    }
    pub fn custom(&self) -> Option<Custom> {
        match self.ctl {
            Ctl::Custom(c) => Some(c),
            _ => None,
        }
    }
}

fn unit_sfx(u: &str) -> String {
    if u.is_empty() {
        String::new()
    } else {
        format!(" {u}")
    }
}

/// knob value text by unit
pub(crate) fn fmt_unit(v: f32, unit: &str) -> String {
    match unit {
        "Hz" if v >= 1000.0 => format!("{:.2} kHz", v / 1000.0),
        "Hz" if v < 10.0 => format!("{v:.2} Hz"),
        "Hz" => format!("{v:.0} Hz"),
        "s" if v < 1.0 => format!("{:.0} ms", v * 1000.0),
        "s" => format!("{v:.2} s"),
        "ms" if v >= 1000.0 => format!("{:.2} s", v / 1000.0),
        "ms" => format!("{v:.0} ms"),
        "st" => format!("{v:+.1} st"),
        "ct" => format!("{v:.1} ct"),
        "oct" => format!("{v:+.2} oct"),
        "dB" => format!("{v:.1} dB"),
        "x" => format!("{v:.2}x"),
        "bit" => format!("{v:.1} bit"),
        "%" => format!("{:.0} %", v * 100.0),
        "steps" => format!("{v:+.1}"),
        _ => format!("{v:.2}"),
    }
}

/// knob position (0..1) of a real value; log ranges may start at 0
pub(crate) fn to_norm(v: f32, lo: f32, hi: f32, log: bool) -> f32 {
    if hi <= lo || !v.is_finite() {
        return 0.0;
    }
    let v = v.clamp(lo, hi);
    if log {
        let e = if lo <= 0.0 { hi * 1e-4 } else { 0.0 };
        ((v + e) / (lo + e)).ln() / ((hi + e) / (lo + e)).ln()
    } else {
        (v - lo) / (hi - lo)
    }
}

pub(crate) fn from_norm(n: f32, lo: f32, hi: f32, log: bool) -> f32 {
    let n = n.clamp(0.0, 1.0);
    if log {
        if n <= 0.0 {
            return lo;
        }
        let e = if lo <= 0.0 { hi * 1e-4 } else { 0.0 };
        ((lo + e) * ((hi + e) / (lo + e)).powf(n) - e).clamp(lo, hi)
    } else {
        lo + (hi - lo) * n
    }
}

/// small pictures drawn in a card
pub(crate) enum Viz {
    /// attack, decay, sustain level, release (seconds)
    Adsr(f32, f32, f32, f32),
    /// cutoff Hz, resonance 0..1, 0 LP / 1 HP / 2 BP
    Filter(f32, f32, u8),
    /// one cycle, -1..1
    Wave(Vec<f32>),
    /// sample peaks (min, max)
    Peaks(Vec<(f32, f32)>),
    /// FM algorithm index
    Algo(u8),
}

pub(crate) type VizFn = Box<dyn Fn(&Instrument) -> Viz>;

pub(crate) struct Card {
    pub tab: Tab,
    pub name: String,
    pub viz: Option<VizFn>,
    /// short text on the right of the title
    pub note: String,
}

#[derive(Default)]
pub(crate) struct Layout {
    pub cards: Vec<Card>,
    pub params: Vec<Param>,
}

impl Layout {
    pub fn card_params<'a>(&'a self, c: &'a Card) -> impl Iterator<Item = &'a Param> + 'a {
        self.params
            .iter()
            .filter(move |p| p.tab == c.tab && p.card == c.name)
    }
    pub fn tab_has(&self, t: Tab) -> bool {
        self.cards.iter().any(|c| c.tab == t)
    }
    pub fn find(&self, key: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.key == key)
    }
}

/// short type tag of an instrument (list, header, card-state key)
pub(crate) fn type_tag(k: &InstrumentKind) -> &'static str {
    match k {
        InstrumentKind::Sample { .. } => "Sample",
        InstrumentKind::Voice(_) => "Voice",
        InstrumentKind::Plugin(p) if p.format == "vst3" => "VST3",
        InstrumentKind::Plugin(_) => "CLAP",
        InstrumentKind::Rack(_) => "Rack",
        InstrumentKind::Synth(p) => {
            if p.filter_model == crate::ladder::FilterModel::Svf {
                "Synth"
            } else {
                "Moog"
            }
        }
        InstrumentKind::Gen(g) => match g.engine {
            GenEngine::Drum(_) => "Drum",
            GenEngine::Kit(_) => "Kit",
            GenEngine::ChipFx(_) => "Chip FX",
            GenEngine::Analog(_) => "Analog",
            GenEngine::Fm(_) => "FM-4",
            GenEngine::Noise(_) => "Noise",
            GenEngine::Wave(_) => "Wave",
            GenEngine::Grid(_) => "Grid",
            GenEngine::Poly(_) => crate::poly::POLY_DISPLAY_NAME,
            GenEngine::Drums(_) => "Drum Kit",
            GenEngine::Strings(_) => crate::strings::STRINGS_DISPLAY_NAME,
        },
    }
}

// ---- accessors (a static default stands in for the wrong instrument type) ----

macro_rules! fallback {
    ($f:ident, $t:ty, $e:expr) => {
        fn $f() -> &'static $t {
            static D: OnceLock<$t> = OnceLock::new();
            D.get_or_init(|| $e)
        }
    };
}
fallback!(d_patch, Patch, Patch::default());
fallback!(d_voice, VoicePatch, VoicePatch::default());
fallback!(d_gen, GenPatch, GenPatch::default());
fallback!(d_an, crate::gen2::AnalogP, Default::default());
fallback!(d_fm, crate::gen2::FmP, Default::default());
fallback!(d_nz, crate::gen2::NoiseP, Default::default());
fallback!(d_wv, crate::gen_wave::WaveP, Default::default());
fallback!(d_ps, crate::poly::PolyP, Default::default());
fallback!(d_sw, crate::strings::StringsP, Default::default());
fallback!(d_gd, crate::grid::GridP, crate::grid::GridP::empty());
fallback!(d_sfx, crate::chipfx::SfxP, Default::default());

fn me(i: &Instrument) -> &Instrument {
    i
}
fn me_mut(i: &mut Instrument) -> Option<&mut Instrument> {
    Some(i)
}
fn pa(i: &Instrument) -> &Patch {
    i.kind.patch().unwrap_or(d_patch())
}
fn pa_mut(i: &mut Instrument) -> Option<&mut Patch> {
    match &mut i.kind {
        InstrumentKind::Synth(p) => Some(p),
        InstrumentKind::Voice(v) => Some(&mut v.base),
        InstrumentKind::Gen(g) => Some(&mut g.base),
        _ => None,
    }
}
fn vo(i: &Instrument) -> &VoicePatch {
    match &i.kind {
        InstrumentKind::Voice(v) => v,
        _ => d_voice(),
    }
}
fn vo_mut(i: &mut Instrument) -> Option<&mut VoicePatch> {
    match &mut i.kind {
        InstrumentKind::Voice(v) => Some(v),
        _ => None,
    }
}
fn ge(i: &Instrument) -> &GenPatch {
    match &i.kind {
        InstrumentKind::Gen(g) => g,
        _ => d_gen(),
    }
}
fn ge_mut(i: &mut Instrument) -> Option<&mut GenPatch> {
    match &mut i.kind {
        InstrumentKind::Gen(g) => Some(g),
        _ => None,
    }
}
macro_rules! engine_acc {
    ($r:ident, $m:ident, $v:ident, $t:ty, $d:ident) => {
        fn $r(i: &Instrument) -> &$t {
            match &ge(i).engine {
                GenEngine::$v(x) => x,
                _ => $d(),
            }
        }
        fn $m(i: &mut Instrument) -> Option<&mut $t> {
            match ge_mut(i).map(|g| &mut g.engine) {
                Some(GenEngine::$v(x)) => Some(x),
                _ => None,
            }
        }
    };
}
engine_acc!(an, an_mut, Analog, crate::gen2::AnalogP, d_an);
engine_acc!(fm, fm_mut, Fm, crate::gen2::FmP, d_fm);
engine_acc!(nz, nz_mut, Noise, crate::gen2::NoiseP, d_nz);
engine_acc!(wv, wv_mut, Wave, crate::gen_wave::WaveP, d_wv);
engine_acc!(gd, gd_mut, Grid, crate::grid::GridP, d_gd);
engine_acc!(ps, ps_mut, Poly, crate::poly::PolyP, d_ps);
engine_acc!(sw, sw_mut, Strings, crate::strings::StringsP, d_sw);
engine_acc!(sfx, sfx_mut, ChipFx, crate::chipfx::SfxP, d_sfx);

fn drum(i: &Instrument, sel: usize) -> DrumP {
    match &ge(i).engine {
        GenEngine::Drum(d) => *d,
        GenEngine::Kit(k) => k.pieces[sel.min(7)],
        GenEngine::Drums(k) => k
            .pads
            .get(sel.min(k.pads.len().max(1) - 1))
            .map(|p| p.drum)
            .unwrap_or_default(),
        _ => DrumP::default(),
    }
}
fn drum_mut(i: &mut Instrument, sel: usize) -> Option<&mut DrumP> {
    ge_mut(i).and_then(|g| g.drum_mut(sel))
}

/// float / int field: (get, set)
macro_rules! fld {
    ($r:ident, $m:ident, $($f:tt)+) => {(
        Box::new(move |i: &Instrument| $r(i).$($f)+ as f32) as Get,
        Box::new(move |i: &mut Instrument, v: f32| {
            if let Some(x) = $m(i) {
                x.$($f)+ = v as _;
            }
        }) as Set,
    )};
}
/// integer field (rounded)
macro_rules! fldi {
    ($r:ident, $m:ident, $($f:tt)+) => {(
        Box::new(move |i: &Instrument| $r(i).$($f)+ as f32) as Get,
        Box::new(move |i: &mut Instrument, v: f32| {
            if let Some(x) = $m(i) {
                x.$($f)+ = v.round() as _;
            }
        }) as Set,
    )};
}
/// field scaled for display (`k` = shown / stored, e.g. 1000 for ms)
macro_rules! flds {
    ($k:expr, $r:ident, $m:ident, $($f:tt)+) => {(
        Box::new(move |i: &Instrument| $r(i).$($f)+ * $k) as Get,
        Box::new(move |i: &mut Instrument, v: f32| {
            if let Some(x) = $m(i) {
                x.$($f)+ = v / $k;
            }
        }) as Set,
    )};
}
macro_rules! fldb {
    ($r:ident, $m:ident, $($f:tt)+) => {(
        Box::new(move |i: &Instrument| if $r(i).$($f)+ { 1.0f32 } else { 0.0 }) as Get,
        Box::new(move |i: &mut Instrument, v: f32| {
            if let Some(x) = $m(i) {
                x.$($f)+ = v > 0.5;
            }
        }) as Set,
    )};
}
/// enum field over the listed values (index)
macro_rules! fldc {
    ($r:ident, $m:ident, $all:expr, $($f:tt)+) => {{
        let a = $all.to_vec();
        let b = a.clone();
        (
            Box::new(move |i: &Instrument| {
                a.iter().position(|x| *x == $r(i).$($f)+).unwrap_or(0) as f32
            }) as Get,
            Box::new(move |i: &mut Instrument, v: f32| {
                if let (Some(x), Some(n)) = ($m(i), b.get(v.round().max(0.0) as usize)) {
                    x.$($f)+ = *n;
                }
            }) as Set,
        )
    }};
}

/// builder: params go on the current card
struct B {
    l: Layout,
    tab: Tab,
    card: String,
    /// the instrument (0-based), for automation targets
    idx: usize,
}

impl B {
    fn card(&mut self, tab: Tab, name: &str) -> &mut Card {
        self.tab = tab;
        self.card = name.to_string();
        if let Some(k) = self
            .l
            .cards
            .iter()
            .position(|c| c.tab == tab && c.name == name)
        {
            return &mut self.l.cards[k];
        }
        self.l.cards.push(Card {
            tab,
            name: name.into(),
            viz: None,
            note: String::new(),
        });
        self.l.cards.last_mut().unwrap()
    }
    fn push(
        &mut self,
        key: &str,
        name: &str,
        unit: &'static str,
        ctl: Ctl,
        gs: (Get, Set),
    ) -> &mut Param {
        self.l.params.push(Param {
            tab: self.tab,
            card: self.card.clone(),
            key: key.into(),
            name: name.into(),
            tip: String::new(),
            unit,
            ctl,
            get: gs.0,
            set: gs.1,
            fmt: None,
            target: None,
            lock: None,
            enabled: None,
            badge: None,
        });
        self.l.params.last_mut().unwrap()
    }
    #[allow(clippy::too_many_arguments)]
    fn knob(
        &mut self,
        key: &str,
        name: &str,
        lo: f32,
        hi: f32,
        log: bool,
        def: f32,
        unit: &'static str,
        gs: (Get, Set),
    ) -> &mut Param {
        let ctl = Ctl::Knob {
            lo,
            hi,
            log,
            step: 0.0,
            def,
        };
        self.push(key, name, unit, ctl, gs)
    }
    #[allow(clippy::too_many_arguments)]
    fn int(
        &mut self,
        key: &str,
        name: &str,
        lo: f32,
        hi: f32,
        def: f32,
        unit: &'static str,
        gs: (Get, Set),
    ) -> &mut Param {
        let ctl = Ctl::Knob {
            lo,
            hi,
            log: false,
            step: 1.0,
            def,
        };
        self.push(key, name, unit, ctl, gs)
    }
    fn choice(&mut self, key: &str, name: &str, opts: Vec<String>, gs: (Get, Set)) -> &mut Param {
        self.push(key, name, "", Ctl::Choice(opts), gs)
    }
    fn toggle(&mut self, key: &str, name: &str, gs: (Get, Set)) -> &mut Param {
        self.push(key, name, "", Ctl::Toggle, gs)
    }
    fn custom(&mut self, key: &str, name: &str, c: Custom) -> &mut Param {
        let gs: (Get, Set) = (Box::new(|_| 0.0), Box::new(|_, _| {}));
        self.push(key, name, "", Ctl::Custom(c), gs)
    }
    /// a 0..1 generator knob described by a MacroDef; `gk` = its
    /// automation knob index (`Target::Gen`)
    fn mac(
        &mut self,
        key: &str,
        def: &'static MacroDef,
        gs: (Get, Set),
        lock: LockFns,
        badge: Option<usize>,
        gk: usize,
    ) -> &mut Param {
        let idx = self.idx;
        let mid = (def.dice.0 + def.dice.1) * 0.5;
        let p = self.knob(key, def.name, 0.0, 1.0, false, mid, "", gs);
        p.tip = def.tip.to_string();
        p.fmt = Some(Box::new(move |v| def.show(v)));
        p.lock = Some(lock);
        p.badge = badge.map(|k| format!("M{}", k + 1));
        p.target = Some(Target::Gen(idx, gk));
        p
    }
}

impl Param {
    fn tip(&mut self, t: &str) -> &mut Self {
        self.tip = t.into();
        self
    }
    fn target(&mut self, t: Target) -> &mut Self {
        self.target = Some(t);
        self
    }
    fn when(&mut self, f: impl Fn(&Instrument) -> bool + 'static) -> &mut Self {
        self.enabled = Some(Box::new(f));
        self
    }
    fn fmt(&mut self, f: impl Fn(f32) -> String + 'static) -> &mut Self {
        self.fmt = Some(Box::new(f));
        self
    }
}

fn names<T: std::fmt::Debug>(v: &[T]) -> Vec<String> {
    v.iter().map(|x| format!("{x:?}")).collect()
}

fn wave_cycle(w: Wave, pw: f32) -> Vec<f32> {
    (0..64)
        .map(|k| {
            let t = k as f32 / 64.0;
            match w {
                Wave::Saw => 1.0 - 2.0 * t,
                Wave::Square => sq(t, 0.5),
                Wave::Pulse => sq(t, pw),
                Wave::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
                Wave::Sine => (t * std::f32::consts::TAU).sin(),
            }
        })
        .collect()
}

fn sq(t: f32, w: f32) -> f32 {
    if t < w {
        1.0
    } else {
        -1.0
    }
}

fn mode_ix(m: FilterMode) -> u8 {
    match m {
        FilterMode::Lp => 0,
        FilterMode::Hp => 1,
        FilterMode::Bp => 2,
    }
}

/// Build the editor layout for instrument `idx` (0-based); `kit_sel` = the kit
/// piece shown for drum kits.
pub(crate) fn build(inst: &Instrument, idx: usize, kit_sel: usize) -> Layout {
    let mut b = B {
        l: Layout::default(),
        tab: Tab::Main,
        card: String::new(),
        idx,
    };
    match &inst.kind {
        InstrumentKind::Synth(_) => synth(&mut b, idx, false),
        InstrumentKind::Voice(_) => {
            voice(&mut b);
            synth(&mut b, idx, true);
            transform(&mut b);
        }
        InstrumentKind::Gen(g) => generator(&mut b, g, kit_sel),
        InstrumentKind::Sample { .. } => {
            b.card(Tab::Main, "Sample").viz = Some(Box::new(|i| match &i.kind {
                InstrumentKind::Sample { data, .. } => {
                    Viz::Peaks(crate::sample::peaks(data, 0.0, data.len() as f64, 96))
                }
                _ => Viz::Peaks(vec![]),
            }));
            b.custom("sample.info", "Sample", Custom::SampleInfo)
                .tip("file, length, base note; Open in Sample editor (loop, slices, editing)");
        }
        InstrumentKind::Plugin(_) => {
            b.card(Tab::Main, "Plugin");
            b.custom("plugin.status", "Plugin", Custom::PluginStatus)
                .tip("Configure… opens the plugin's own window");
            b.card(Tab::Main, "Macros").note = "43xy · lanes".into();
            b.custom("plugin.macros", "Macros 1-8", Custom::PluginMacros)
                .tip("eight macros: 43xy, automation lanes and MIDI learn drive the parameter on each");
            b.card(Tab::Advanced, "All parameters");
            b.custom("plugin.params", "Parameters", Custom::PluginParams)
                .tip("every parameter of the plugin (right-click one to put it on a macro)");
        }
        InstrumentKind::Rack(r) => {
            b.card(Tab::Main, "Rack").note = format!(
                "{} layer{} · add, remove and set them up in Nested Device Chains (Ctrl+7)",
                r.layers.len(),
                if r.layers.len() == 1 { "" } else { "s" }
            );
        }
    }
    output(&mut b, inst, idx);
    fx_chain(&mut b, inst, idx);
    modulation(&mut b, inst, idx);
    b.l
}

/// drag-to-modulate: the Modulators and Routings cards lead the Modulation
/// tab; synth-patch knobs without a target of their own get one
/// (`Target::Patch`) so they can be modulated and automated
fn modulation(b: &mut B, inst: &Instrument, idx: usize) {
    let n0 = b.l.cards.len();
    b.card(Tab::Modulation, "Modulators").note = "drag a chip onto a knob".into();
    b.custom("mod.modulators", "Modulators", Custom::Modulators)
        .tip(
            "LFOs, envelopes, random, velocity, keytrack, mod wheel and macros of this instrument",
        );
    b.card(Tab::Modulation, "Routings");
    b.custom("mod.routings", "Routings", Custom::Routings)
        .tip("modulator > parameter connections with their depth");
    let ours: Vec<Card> = b.l.cards.drain(n0..).collect();
    let at =
        b.l.cards
            .iter()
            .position(|c| c.tab == Tab::Modulation)
            .unwrap_or(b.l.cards.len());
    for (k, c) in ours.into_iter().enumerate() {
        b.l.cards.insert(at + k, c);
    }
    let Some(patch) = inst.kind.patch() else {
        return;
    };
    for p in b.l.params.iter_mut() {
        if p.target.is_some() || !matches!(p.ctl, Ctl::Knob { .. }) {
            continue;
        }
        let Some(k) = crate::modulation::patch_param(&p.key) else {
            continue;
        };
        // only when the knob really is that patch field
        let reg = &crate::modulation::PATCH_PARAMS[k];
        let (a, c) = ((p.get)(inst), (reg.get)(patch));
        if (a - c).abs() <= 1e-4 * a.abs().max(1.0) {
            p.target = Some(crate::automation::Target::Patch(idx, k));
        }
    }
}

/// Volume / Transpose: on every instrument
fn output(b: &mut B, inst: &Instrument, idx: usize) {
    let card = match &inst.kind {
        InstrumentKind::Voice(_) => "Output",
        _ => "Voice",
    };
    b.card(Tab::Main, card);
    b.knob(
        "volume",
        "Volume",
        0.0,
        1.0,
        false,
        0.8,
        "",
        fld!(me, me_mut, volume),
    )
    .target(Target::Level(idx))
    .tip("instrument volume");
    b.int(
        "transpose",
        "Transpose",
        -48.0,
        48.0,
        0.0,
        "st",
        fldi!(me, me_mut, transpose),
    )
    .fmt(|v| format!("{:+} st", v.round() as i32))
    .tip("transpose every note of this instrument (semitones)");
}

/// the synth patch (Synth instruments; `voice` = the subset a Voice uses)
fn synth(b: &mut B, idx: usize, voice: bool) {
    let waves = [
        Wave::Saw,
        Wave::Square,
        Wave::Pulse,
        Wave::Triangle,
        Wave::Sine,
    ];
    if !voice {
        b.card(Tab::Main, "Oscillator").viz = Some(Box::new(|i| {
            Viz::Wave(wave_cycle(pa(i).osc1, pa(i).pulse_width))
        }));
        b.choice(
            "osc1",
            "Wave",
            names(&waves),
            fldc!(pa, pa_mut, waves, osc1),
        );
    } else {
        b.card(Tab::Main, "Output");
    }
    b.int(
        "osc1_oct",
        "Octave",
        -3.0,
        3.0,
        0.0,
        "",
        fldi!(pa, pa_mut, osc1_oct),
    )
    .fmt(|v| format!("{:+}", v.round() as i32));
    let pwp = b.knob(
        "pulse_width",
        "Pulse width",
        0.05,
        0.95,
        false,
        0.5,
        "",
        fld!(pa, pa_mut, pulse_width),
    );
    if voice {
        pwp.when(|i| vo(i).source == crate::voice::VoiceSource::Pulse)
            .tip("pulse width of the Pulse source");
    }
    if !voice {
        b.int(
            "unison",
            "Unison",
            1.0,
            8.0,
            1.0,
            "",
            fldi!(pa, pa_mut, unison),
        );
        b.knob(
            "detune_cents",
            "Detune",
            0.0,
            50.0,
            false,
            0.0,
            "ct",
            fld!(pa, pa_mut, detune_cents),
        )
        .tip("unison detune in cents");
    }
    // filter
    b.card(Tab::Main, "Filter").viz = Some(Box::new(|i| {
        let p = pa(i);
        Viz::Filter(p.cutoff, p.resonance, mode_ix(p.filter))
    }));
    let modes = [FilterMode::Lp, FilterMode::Hp, FilterMode::Bp];
    b.choice(
        "filter",
        "Mode",
        names(&modes),
        fldc!(pa, pa_mut, modes, filter),
    );
    filter_model(b);
    b.knob(
        "cutoff",
        "Cutoff",
        20.0,
        18000.0,
        true,
        4000.0,
        "Hz",
        fld!(pa, pa_mut, cutoff),
    )
    .target(Target::Cutoff(idx));
    b.knob(
        "resonance",
        "Res",
        0.0,
        0.98,
        false,
        0.2,
        "",
        fld!(pa, pa_mut, resonance),
    )
    .target(Target::Resonance(idx))
    .tip("resonance");
    b.knob(
        "env_amount",
        "Env amt",
        -1.0,
        1.0,
        false,
        0.0,
        "oct",
        fld!(pa, pa_mut, env_amount),
    )
    .tip("filter envelope amount");
    b.knob(
        "fenv.d",
        "F.Decay",
        0.001,
        3.0,
        true,
        0.3,
        "s",
        fld!(pa, pa_mut, fenv.d),
    )
    .tip("filter envelope decay");
    b.knob(
        "drive",
        "Drive",
        0.0,
        1.0,
        false,
        0.0,
        "",
        fld!(pa, pa_mut, drive),
    )
    .target(Target::Drive(idx));
    // envelope
    b.card(Tab::Main, "Envelope").viz = Some(Box::new(|i| {
        let a = pa(i).amp;
        Viz::Adsr(a.a, a.d, a.s, a.r)
    }));
    b.knob(
        "amp.a",
        "Attack",
        0.0,
        2.0,
        true,
        0.002,
        "s",
        fld!(pa, pa_mut, amp.a),
    );
    b.knob(
        "amp.d",
        "Decay",
        0.001,
        3.0,
        true,
        0.3,
        "s",
        fld!(pa, pa_mut, amp.d),
    );
    b.knob(
        "amp.s",
        "Sustain",
        0.0,
        1.0,
        false,
        0.7,
        "",
        fld!(pa, pa_mut, amp.s),
    );
    b.knob(
        "amp.r",
        "Release",
        0.001,
        4.0,
        true,
        0.15,
        "s",
        fld!(pa, pa_mut, amp.r),
    );
    // voice: width, glide, legato
    b.card(Tab::Main, if voice { "Output" } else { "Voice" });
    if !voice {
        b.knob("width", "Stereo", 0.0, 1.0, false, 0.0, "", fld!(pa, pa_mut, width)).tip(
            "spreads the unison voices left/right (Unison > 1); with Noise it makes the noise stereo (hats)",
        );
    }
    b.knob("glide", "Glide", 1.0, 1000.0, true, 60.0, "ms", flds!(1000.0, pa, pa_mut, glide)).tip(
        "glide time: how long a slide to the next note takes (effect 3C01, or every held note with Legato on)",
    );
    b.toggle("legato", "Legato", fldb!(pa, pa_mut, legato)).tip(
        "a new note while the last one still sounds glides to it without restarting the envelopes",
    );
    // modulation: LFO
    b.card(Tab::Modulation, "LFO").viz =
        Some(Box::new(|i| Viz::Wave(wave_cycle(pa(i).lfo.shape, 0.5))));
    let shapes = [Wave::Sine, Wave::Triangle, Wave::Square, Wave::Saw];
    b.choice(
        "lfo.shape",
        "Shape",
        names(&shapes),
        fldc!(pa, pa_mut, shapes, lfo.shape),
    );
    const SYNC: [(f32, &str); 7] = [
        (0.0, "free Hz"),
        (0.25, "1/16"),
        (0.5, "1/8"),
        (1.0, "1/4"),
        (2.0, "1/2"),
        (4.0, "1 bar"),
        (8.0, "2 bars"),
    ];
    let mut opts: Vec<String> = SYNC.iter().map(|s| s.1.to_string()).collect();
    opts.push("other".into());
    let gs: (Get, Set) = (
        Box::new(|i| {
            let s = pa(i).lfo.sync;
            SYNC.iter().position(|x| x.0 == s).unwrap_or(SYNC.len()) as f32
        }),
        Box::new(|i, v| {
            if let (Some(p), Some(s)) = (pa_mut(i), SYNC.get(v.round() as usize)) {
                p.lfo.sync = s.0;
            }
        }),
    );
    b.choice("lfo.sync", "Sync", opts, gs)
        .tip("free = Rate in Hz, else locked to the song tempo");
    b.knob(
        "lfo.rate",
        "Rate",
        0.01,
        30.0,
        true,
        1.0,
        "Hz",
        fld!(pa, pa_mut, lfo.rate),
    )
    .when(|i| pa(i).lfo.sync == 0.0)
    .tip("free LFO rate (Sync = free Hz)");
    b.knob(
        "lfo.cutoff",
        "> Cutoff",
        -4.0,
        4.0,
        false,
        0.0,
        "oct",
        fld!(pa, pa_mut, lfo.cutoff),
    );
    b.knob(
        "lfo.amp",
        "> Amp",
        0.0,
        1.0,
        false,
        0.0,
        "",
        fld!(pa, pa_mut, lfo.amp),
    )
    .tip("1 with a Square LFO synced to 1/16 = trance gate");
    b.knob(
        "lfo.pitch",
        "> Pitch",
        0.0,
        12.0,
        false,
        0.0,
        "st",
        fld!(pa, pa_mut, lfo.pitch),
    );
    // velocity + accent
    b.card(Tab::Modulation, "Velocity & accent");
    b.knob(
        "vel_cutoff",
        "Vel>Cut",
        0.0,
        4.0,
        false,
        0.0,
        "oct",
        fld!(pa, pa_mut, vel_cutoff),
    )
    .tip("softer notes (volume column) get a darker filter");
    b.knob("accent", "Accent", 0.0, 1.0, false, 0.6, "", fld!(pa, pa_mut, accent)).tip(
        "accented notes (effect 3D or high velocity) are louder, with a more open filter and a shorter decay",
    );
    let gs: (Get, Set) = (
        Box::new(|i| (pa(i).accent_vel * 64.0).round()),
        Box::new(|i, v| {
            if let Some(p) = pa_mut(i) {
                p.accent_vel = v.round() / 64.0;
            }
        }),
    );
    b.int("accent_vel", "Accent at", 0.0, 64.0, 0.0, "", gs)
        .fmt(|v| {
            if v < 0.5 {
                "off".into()
            } else {
                format!("vol ≥ {:02X}", v.round() as u32)
            }
        })
        .tip("notes with a volume-column value at or above this are accented (00 = off: only effect 3D)");
    // advanced
    b.card(Tab::Advanced, "Timing & character");
    b.knob(
        "start_delay",
        "Delay",
        0.0,
        50.0,
        false,
        0.0,
        "ms",
        flds!(1000.0, pa, pa_mut, start_delay),
    )
    .tip("plays every note this much behind the grid (laid-back hats / groove)");
    if !voice {
        b.knob(
            "metal",
            "Metal 808",
            0.0,
            1.0,
            false,
            0.0,
            "",
            fld!(pa, pa_mut, metal),
        )
        .tip("six square oscillators at 808 hi-hat ratios: the metallic part of a hat or cymbal");
    }
}

/// filter model + ladder slope (Synth, Voice, Generator filters)
fn filter_model(b: &mut B) {
    use crate::ladder::FilterModel;
    b.choice(
        "filter_model",
        "Model",
        FilterModel::ALL.iter().map(|m| m.name().to_string()).collect(),
        fldc!(pa, pa_mut, FilterModel::ALL, filter_model),
    )
    .tip("SVF: the original 12 dB filter. Ladder: Moog-style 24 dB transistor ladder; Vintage saturates (2x oversampled)");
    b.toggle("ladder_12db", "12 dB", fldb!(pa, pa_mut, ladder_12db))
        .when(|i| pa(i).filter_model != FilterModel::Svf)
        .tip("ladder slope: 12 dB/oct instead of 24");
}

fn voice(b: &mut B) {
    use crate::voice::{Register, VoiceSource};
    b.card(Tab::Main, "Voice");
    b.choice("throat", "Tract", vec!["Formant".into(), "Throat".into()], fldb!(vo, vo_mut, throat))
        .tip("Formant = five formant filters; Throat = a simulated vocal tract (inspired by Pink Trombone by Neil Thapen)");
    b.choice(
        "register",
        "Register",
        names(&Register::ALL),
        fldc!(vo, vo_mut, Register::ALL, register),
    )
    .tip("formant table: Bass / Tenor / Alto / Soprano, or Demon");
    b.choice("source", "Source", names(&VoiceSource::ALL), fldc!(vo, vo_mut, VoiceSource::ALL, source))
        .tip("Glottal = vocal-cord pulse, Saw = brighter, Pulse = robot / chip speech, Whisper = noise only");
    b.knob(
        "shift",
        "Formant",
        0.5,
        2.0,
        true,
        1.0,
        "x",
        fld!(vo, vo_mut, shift),
    )
    .tip("moves all formants up/down without changing the pitch (< 1 = bigger, darker)");
    b.knob(
        "sharp",
        "Sharp",
        0.5,
        3.0,
        false,
        1.0,
        "",
        fld!(vo, vo_mut, sharp),
    )
    .tip("narrower formants = whistly, robotic; wider = softer");
    b.knob(
        "breath",
        "Breath",
        0.0,
        1.0,
        false,
        0.0,
        "",
        fld!(vo, vo_mut, breath),
    )
    .tip("aspiration noise (35xx sets it per row)");
    b.knob(
        "tension",
        "Tension",
        0.0,
        1.0,
        false,
        0.5,
        "",
        fld!(vo, vo_mut, tension),
    )
    .tip("0 = soft, breathy pulse .. 1 = pressed, buzzy (Glottal source)");
    b.knob(
        "jitter",
        "Jitter",
        0.0,
        1.0,
        false,
        0.0,
        "",
        fld!(vo, vo_mut, jitter),
    )
    .tip("random pitch / level wobble per vocal-cord cycle");
    b.card(Tab::Main, "Vowels");
    b.custom("vowels", "Vowel sequence", Custom::Vowels)
        .tip("click a vowel to add it, click a chip in the sequence to remove it (31xx jumps)");
    len_choice(b, "step", "Step", 4.0, |v| v.step, |v, x| v.step = x)
        .tip("move to the next vowel every … (tempo-synced); off = hold the first");
    b.knob(
        "morph_ms",
        "Morph",
        0.0,
        1000.0,
        true,
        40.0,
        "ms",
        fld!(vo, vo_mut, morph_ms),
    )
    .tip("glide time between vowels (0 = hard cut)");
    b.knob(
        "babble",
        "Babble",
        0.0,
        1.0,
        false,
        0.0,
        "",
        fld!(vo, vo_mut, babble),
    )
    .tip("chance that a step jumps to a random vowel instead (needs Step)");
    b.card(Tab::Main, "Words");
    b.custom("words", "Words", Custom::Words).tip(
        "each note speaks the next word; 3Exx speaks word xx; right-click a word to remove it",
    );
    b.knob(
        "rate",
        "Speech rate",
        0.25,
        3.0,
        true,
        1.0,
        "x",
        fld!(vo, vo_mut, rate),
    )
    .tip("how fast words are spoken");
    b.card(Tab::Modulation, "LFO");
    b.knob(
        "lfo_vowel",
        "> Vowel",
        -4.0,
        4.0,
        false,
        0.0,
        "steps",
        fld!(vo, vo_mut, lfo_vowel),
    )
    .tip("the LFO sweeps through the vowel sequence");
    b.knob(
        "lfo_shift",
        "> Formant",
        -12.0,
        12.0,
        false,
        0.0,
        "st",
        fld!(vo, vo_mut, lfo_shift),
    )
    .tip("the LFO moves the formant shift (wah / throat wobble)");
    b.card(Tab::Modulation, "Stutter & chop");
    len_choice(
        b,
        "stutter_len",
        "Stutter",
        2.0,
        |v| v.stutter_len,
        |v, x| v.stutter_len = x,
    )
    .tip("every note: capture this slice, then repeat it (beat-repeat); 33xy per row");
    b.int(
        "stutter_repeats",
        "Repeats",
        0.0,
        16.0,
        0.0,
        "x",
        fldi!(vo, vo_mut, stutter_repeats),
    )
    .tip("repeats after the captured slice");
    b.knob(
        "stutter_pitch",
        "Rpt pitch",
        -12.0,
        12.0,
        false,
        0.0,
        "st",
        fld!(vo, vo_mut, stutter_pitch),
    )
    .tip("pitch step per repeat (tape-style)");
    len_choice(
        b,
        "chop_rate",
        "Chop",
        2.0,
        |v| v.chop_rate,
        |v, x| v.chop_rate = x,
    )
    .tip("tempo-synced gate: the voice opens and closes every … (37xy per row)");
    b.knob(
        "chop_duty",
        "Open",
        0.05,
        1.0,
        false,
        0.5,
        "",
        fld!(vo, vo_mut, chop_duty),
    )
    .tip("how much of each chop cycle is open");
}

/// beat lengths for step / stutter / chop
const LENS: [(f32, &str); 8] = [
    (0.0, "off"),
    (1.0 / 16.0, "1/64"),
    (0.125, "1/32"),
    (0.25, "1/16"),
    (0.5, "1/8"),
    (1.0, "1/4"),
    (2.0, "1/2"),
    (4.0, "1 bar"),
];

fn len_choice<'a>(
    b: &'a mut B,
    key: &str,
    name: &str,
    max: f32,
    get: fn(&VoicePatch) -> f32,
    set: fn(&mut VoicePatch, f32),
) -> &'a mut Param {
    let lens: Vec<(f32, &str)> = LENS.iter().copied().filter(|l| l.0 <= max).collect();
    let mut opts: Vec<String> = lens.iter().map(|l| l.1.to_string()).collect();
    opts.push("other".into());
    let l2 = lens.clone();
    let gs: (Get, Set) = (
        Box::new(move |i| {
            let v = get(vo(i));
            lens.iter()
                .position(|l| (l.0 - v).abs() < 1e-4)
                .unwrap_or(lens.len()) as f32
        }),
        Box::new(move |i, v| {
            if let (Some(p), Some(l)) = (vo_mut(i), l2.get(v.round() as usize)) {
                set(p, l.0);
            }
        }),
    );
    b.choice(key, name, opts, gs)
}

fn tf(i: &Instrument) -> &crate::transform::Transform {
    &vo(i).transform
}
fn tf_mut(i: &mut Instrument) -> Option<&mut crate::transform::Transform> {
    vo_mut(i).map(|v| &mut v.transform)
}

fn transform(b: &mut B) {
    use crate::transform::{Order, Ratios, SubMode};
    b.card(Tab::Fx, "Transform").note = "3F-42".into();
    b.knob(
        "tf.blend",
        "Blend",
        0.0,
        1.0,
        false,
        0.0,
        "",
        fld!(tf, tf_mut, blend),
    )
    .tip("dry voice (0) .. fully transformed (1); 3Fxx per row");
    let orders = [Order::DistRes, Order::ResDist];
    b.choice(
        "tf.order",
        "Order",
        vec!["Dist > Res".into(), "Res > Dist".into()],
        fldc!(tf, tf_mut, orders, order),
    );
    b.card(Tab::Fx, "Metallic resonance");
    b.knob(
        "tf.res.amount",
        "Amount",
        0.0,
        1.0,
        false,
        0.5,
        "",
        fld!(tf, tf_mut, res.amount),
    )
    .tip("resonator mix (40xx per row)");
    b.choice(
        "tf.res.ratios",
        "Ratios",
        Ratios::ALL.iter().map(|r| r.name().to_string()).collect(),
        fldc!(tf, tf_mut, Ratios::ALL, res.ratios),
    )
    .tip("harmonic = robotic comb, metal = 808 cymbal, bell, bar = glassy, odd = hollow square");
    b.choice(
        "tf.res.keyed",
        "Pitch",
        vec!["Fixed".into(), "Note".into()],
        fldb!(tf, tf_mut, res.keyed),
    )
    .tip("Note = tuned to the note played; Fixed = the fixed pitch");
    b.knob(
        "tf.res.tune",
        "Tune",
        -24.0,
        24.0,
        false,
        0.0,
        "st",
        fld!(tf, tf_mut, res.tune),
    )
    .when(|i| tf(i).res.keyed)
    .tip("semitones from the note (Pitch = Note)");
    b.knob(
        "tf.res.hz",
        "Fixed",
        30.0,
        4000.0,
        true,
        220.0,
        "Hz",
        fld!(tf, tf_mut, res.hz),
    )
    .when(|i| !tf(i).res.keyed)
    .tip("resonator base pitch (Pitch = Fixed)");
    b.knob(
        "tf.res.decay_ms",
        "Decay",
        20.0,
        4000.0,
        true,
        400.0,
        "ms",
        fld!(tf, tf_mut, res.decay_ms),
    )
    .tip("ring time: long = singing metal");
    b.knob(
        "tf.res.damp",
        "Damping",
        0.0,
        1.0,
        false,
        0.4,
        "",
        fld!(tf, tf_mut, res.damp),
    );
    b.knob(
        "tf.res.spread",
        "Spread",
        0.0,
        1.0,
        false,
        0.3,
        "",
        fld!(tf, tf_mut, res.spread),
    )
    .tip("detunes the modes apart, left vs right");
    b.card(Tab::Fx, "Digital distortion");
    b.knob(
        "tf.dig.amount",
        "Amount",
        0.0,
        1.0,
        false,
        0.5,
        "",
        fld!(tf, tf_mut, dig.amount),
    )
    .tip("distortion mix (41xx per row)");
    b.knob(
        "tf.dig.drive_db",
        "Drive",
        0.0,
        36.0,
        false,
        12.0,
        "dB",
        fld!(tf, tf_mut, dig.drive_db),
    );
    b.choice(
        "tf.dig.fold",
        "Shape",
        vec!["Clip".into(), "Fold".into()],
        fldb!(tf, tf_mut, dig.fold),
    );
    b.knob(
        "tf.dig.bits",
        "Bits",
        1.0,
        16.0,
        false,
        8.0,
        "bit",
        fld!(tf, tf_mut, dig.bits),
    )
    .tip("bit depth (16 = off)");
    b.knob(
        "tf.dig.rate",
        "Rate",
        300.0,
        48000.0,
        true,
        12000.0,
        "Hz",
        fld!(tf, tf_mut, dig.rate),
    )
    .tip("sample-rate reduction");
    b.toggle("tf.dig.alias", "Aliasing", fldb!(tf, tf_mut, dig.alias))
        .tip("on: plain sample-and-hold, metallic aliasing; off: filtered, cleaner");
    b.knob(
        "tf.dig.ring",
        "Ring mod",
        0.0,
        1.0,
        false,
        0.0,
        "",
        fld!(tf, tf_mut, dig.ring),
    );
    b.knob(
        "tf.dig.ring_ratio",
        "Ring ratio",
        0.25,
        8.0,
        true,
        1.5,
        "x",
        fld!(tf, tf_mut, dig.ring_ratio),
    );
    b.knob(
        "tf.dig.hp",
        "Pre HP",
        20.0,
        2000.0,
        true,
        80.0,
        "Hz",
        fld!(tf, tf_mut, dig.hp),
    );
    b.knob(
        "tf.dig.lp",
        "Post LP",
        1000.0,
        20000.0,
        true,
        12000.0,
        "Hz",
        fld!(tf, tf_mut, dig.lp),
    );
    b.card(Tab::Fx, "Sub bass");
    b.knob(
        "tf.sub.amount",
        "Level",
        0.0,
        1.0,
        false,
        0.5,
        "",
        fld!(tf, tf_mut, sub.amount),
    )
    .tip("sub layer level (42xx per row)");
    b.choice(
        "tf.sub.mode",
        "Mode",
        SubMode::ALL.iter().map(|m| m.name().to_string()).collect(),
        fldc!(tf, tf_mut, SubMode::ALL, sub.mode),
    );
    b.choice(
        "tf.sub.octave",
        "Octave",
        vec!["-1".into(), "-2".into()],
        fldc!(tf, tf_mut, [1u8, 2u8], sub.octave),
    );
    b.knob(
        "tf.sub.drive",
        "Drive",
        0.0,
        1.0,
        false,
        0.3,
        "",
        fld!(tf, tf_mut, sub.drive),
    );
    b.knob(
        "tf.sub.growl",
        "Growl",
        0.0,
        1.0,
        false,
        0.3,
        "",
        fld!(tf, tf_mut, sub.growl),
    );
    b.knob(
        "tf.sub.xover",
        "Crossover",
        60.0,
        250.0,
        true,
        120.0,
        "Hz",
        fld!(tf, tf_mut, sub.xover),
    );
    b.knob(
        "tf.sub.formant_drop",
        "Formant drop",
        0.0,
        12.0,
        false,
        0.0,
        "st",
        fld!(tf, tf_mut, sub.formant_drop),
    )
    .tip("lowers the formants with the blend: a monster voice");
}

/// a lockable knob `k` of a generator parameter array
macro_rules! gen_knob {
    ($b:expr, $tag:literal, $r:ident, $m:ident, $defs:expr, $k:expr) => {{
        let k: usize = $k;
        let bit = 1u64 << k;
        let gs: (Get, Set) = (
            Box::new(move |i| $r(i).p[k]),
            Box::new(move |i, v| {
                if let Some(x) = $m(i) {
                    x.p[k] = v.clamp(0.0, 1.0);
                }
            }),
        );
        let lock: LockFns = (
            Box::new(move |i| $r(i).locks & bit != 0),
            Box::new(move |i| {
                if let Some(x) = $m(i) {
                    x.locks ^= bit;
                }
            }),
        );
        $b.mac(
            &format!("{}.{}", $tag, k),
            &$defs[k],
            gs,
            lock,
            (k < 8).then_some(k),
            k,
        )
    }};
}

/// a lock for a discrete choice (bit `choice_lock(k)`)
macro_rules! choice_lock {
    ($p:expr, $r:ident, $m:ident, $k:expr) => {{
        let bit = crate::gen2::choice_lock($k);
        $p.lock = Some((
            Box::new(move |i| $r(i).locks & bit != 0),
            Box::new(move |i| {
                if let Some(x) = $m(i) {
                    x.locks ^= bit;
                }
            }),
        ));
    }};
}

fn generator(b: &mut B, g: &GenPatch, sel: usize) {
    // Dice + Bake on every generator
    b.card(Tab::Main, "Dice");
    b.custom("gen.dice", "Dice", Custom::Dice).tip(
        "Dice rolls the unlocked knobs, Mutate nudges them, < > walk the rolls, Bake to sample",
    );
    match &g.engine {
        GenEngine::Drum(_) | GenEngine::Kit(_) => drums(b, g, sel),
        GenEngine::ChipFx(_) => chipfx(b),
        GenEngine::Analog(_) => analog(b),
        GenEngine::Fm(_) => fm4(b),
        GenEngine::Noise(_) => noise(b),
        GenEngine::Wave(_) => wave(b),
        GenEngine::Grid(x) => grid(b, x),
        GenEngine::Poly(_) => polysynth(b),
        GenEngine::Drums(_) => drums(b, g, sel),
        GenEngine::Strings(_) => stringwave(b),
    }
    if !g.engine.own_env() {
        // release + optional filter (drums, kit, chip FX)
        b.card(Tab::Main, "Voice");
        b.knob(
            "gen.release",
            "Release",
            5.0,
            2000.0,
            true,
            40.0,
            "ms",
            flds!(1000.0, pa, pa_mut, amp.r),
        )
        .tip("how fast a note-off (===) fades the sound");
        b.card(Tab::Main, "Filter").viz = Some(Box::new(|i| {
            let p = pa(i);
            Viz::Filter(p.cutoff, p.resonance, mode_ix(p.filter))
        }));
        b.toggle("gen.filter_on", "Filter", fldb!(ge, ge_mut, filter_on))
            .tip("run the sound through a filter (Tracker effects 38-3B move it)");
        filter_model(b);
        let on = |i: &Instrument| ge(i).filter_on;
        let n = b.l.params.len();
        b.l.params[n - 2].enabled = Some(Box::new(on));
        b.l.params[n - 1].enabled = Some(Box::new(move |i| {
            on(i) && pa(i).filter_model != crate::ladder::FilterModel::Svf
        }));
        let modes = [FilterMode::Lp, FilterMode::Bp, FilterMode::Hp];
        b.choice(
            "filter",
            "Mode",
            names(&modes),
            fldc!(pa, pa_mut, modes, filter),
        )
        .when(on);
        b.knob(
            "cutoff",
            "Cutoff",
            20.0,
            18000.0,
            true,
            6000.0,
            "Hz",
            fld!(pa, pa_mut, cutoff),
        )
        .when(on);
        b.knob(
            "resonance",
            "Res",
            0.0,
            1.0,
            false,
            0.2,
            "",
            fld!(pa, pa_mut, resonance),
        )
        .when(on);
    }
}

fn drums(b: &mut B, g: &GenPatch, sel: usize) {
    let kit = matches!(g.engine, GenEngine::Kit(_));
    let drum_kit = match &g.engine {
        GenEngine::Drums(k) => Some(k),
        _ => None,
    };
    let d = match &g.engine {
        GenEngine::Drum(d) => *d,
        GenEngine::Kit(k) => k.pieces[sel.min(7)],
        GenEngine::Drums(k) => k
            .pads
            .get(sel.min(k.pads.len().max(1) - 1))
            .map(|p| p.drum)
            .unwrap_or_default(),
        _ => DrumP::default(),
    };
    if kit {
        b.card(Tab::Main, "Kit").note = "the note picks the drum".into();
        b.custom("kit.piece", "Kit piece", Custom::KitPiece)
            .tip("which kit piece the knobs edit: C kick, D snare, D# clap, F#/A# hats, E-A toms, C# B perc");
    }
    let title = if let Some(k) = drum_kit {
        let n = k.pads.len().max(1);
        format!(
            "Pad · {}",
            k.pads.get(sel.min(n - 1)).map_or("", |p| p.name.as_str())
        )
    } else if kit {
        format!("Drum · {}", crate::gen::KIT_NAMES[sel.min(7)])
    } else {
        "Drum".to_string()
    };
    b.card(Tab::Main, &title).note = "43xy macros".into();
    let gs: (Get, Set) = (
        Box::new(move |i| {
            let k = drum(i, sel).kind;
            DrumKind::ALL.iter().position(|x| *x == k).unwrap_or(0) as f32
        }),
        Box::new(move |i, v| {
            if let (Some(d), Some(k)) = (drum_mut(i, sel), DrumKind::ALL.get(v.round() as usize)) {
                if d.kind != *k {
                    let keep = (d.level, d.follow_note);
                    *d = DrumP::new(*k, 0, &[]);
                    (d.level, d.follow_note) = keep;
                }
            }
        }),
    );
    b.choice(
        "drum.kind",
        "Kind",
        DrumKind::ALL.iter().map(|k| k.name().to_string()).collect(),
        gs,
    )
    .tip("changing the kind starts from its default sound");
    let gs: (Get, Set) = (
        Box::new(move |i| drum(i, sel).model as f32),
        Box::new(move |i, v| {
            if let Some(d) = drum_mut(i, sel) {
                d.model = (v.round().max(0.0) as u8).min(d.kind.models().len() as u8 - 1);
            }
        }),
    );
    b.choice(
        "drum.model",
        "Model",
        d.kind.models().iter().map(|s| s.to_string()).collect(),
        gs,
    );
    if !kit && drum_kit.is_none() {
        let gs: (Get, Set) = (
            Box::new(move |i| if drum(i, sel).follow_note { 1.0 } else { 0.0 }),
            Box::new(move |i, v| {
                if let Some(d) = drum_mut(i, sel) {
                    d.follow_note = v > 0.5;
                }
            }),
        );
        b.toggle("drum.follow_note", "Follow note", gs)
            .tip("C-4 plays the drum as tuned; other notes transpose it");
    }
    let defs = d.kind.macros();
    for (k, def) in defs.iter().enumerate().take(8) {
        let bit = 1u8 << k;
        let gs: (Get, Set) = (
            Box::new(move |i| drum(i, sel).macros[k]),
            Box::new(move |i, v| {
                if let Some(d) = drum_mut(i, sel) {
                    d.macros[k] = v.clamp(0.0, 1.0);
                }
            }),
        );
        let lock: LockFns = (
            Box::new(move |i| drum(i, sel).locks & bit != 0),
            Box::new(move |i| {
                if let Some(d) = drum_mut(i, sel) {
                    d.locks ^= bit;
                }
            }),
        );
        let gk = if kit {
            sel.min(7) * 8 + k
        } else if drum_kit.is_some() {
            sel * 8 + k
        } else {
            k
        };
        let p = b.mac(&format!("drum.m{k}"), def, gs, lock, Some(k), gk);
        if gk >= crate::gen::GEN_KNOBS {
            // pads past GEN_KNOBS / 8 are not automatable yet
            p.target = None;
        }
    }
    let gs: (Get, Set) = (
        Box::new(move |i| drum(i, sel).level),
        Box::new(move |i, v| {
            if let Some(d) = drum_mut(i, sel) {
                d.level = v;
            }
        }),
    );
    b.knob("drum.level", "Level", 0.0, 1.5, false, 1.0, "", gs)
        .tip("drum level");
}

fn chipfx(b: &mut B) {
    use crate::chipfx::*;
    b.card(Tab::Main, "Sound");
    b.custom("chipfx.category", "Categories", Custom::ChipCategories)
        .tip("Pickup / Laser / Explosion / Power-up / Hit / Jump / Blip roll a sound; Randomize anything");
    let p = b.choice(
        "chipfx.wave",
        "Wave",
        Wave::ALL.iter().map(|w| w.name().to_string()).collect(),
        fldc!(sfx, sfx_mut, Wave::ALL, wave),
    );
    p.lock = Some((
        Box::new(|i| sfx(i).locks & WAVE_LOCK != 0),
        Box::new(|i| {
            if let Some(s) = sfx_mut(i) {
                s.locks ^= WAVE_LOCK;
            }
        }),
    ));
    b.toggle(
        "chipfx.follow_note",
        "Follow note",
        fldb!(sfx, sfx_mut, follow_note),
    )
    .tip("C-4 plays the sound as designed; other notes transpose it");
    b.knob(
        "chipfx.level",
        "Level",
        0.0,
        1.5,
        false,
        1.0,
        "",
        fld!(sfx, sfx_mut, level),
    );
    fn one(b: &mut B, k: usize, badge: Option<usize>) {
        let bit = 1u32 << k;
        let gs: (Get, Set) = (
            Box::new(move |i| PARAMS[k].unmap(sfx(i).p[k])),
            Box::new(move |i, v| {
                if let Some(s) = sfx_mut(i) {
                    s.p[k] = PARAMS[k].map(v);
                }
            }),
        );
        let lock: LockFns = (
            Box::new(move |i| sfx(i).locks & bit != 0),
            Box::new(move |i| {
                if let Some(s) = sfx_mut(i) {
                    s.locks ^= bit;
                }
            }),
        );
        let p = b.mac(&format!("sfx.{k}"), &PARAMS[k], gs, lock, badge, k);
        // double-click = the sfxr default of this parameter
        if let Ctl::Knob { def, .. } = &mut p.ctl {
            *def = PARAMS[k].unmap(SfxP::default().p[k]);
        }
    }
    b.card(Tab::Main, "Macros").note = "43xy".into();
    for (m, &k) in MACROS.iter().enumerate() {
        one(b, k, Some(m));
    }
    let groups: [(&str, &[usize]); 7] = [
        ("Pitch", &[BASE_FREQ, FREQ_LIMIT, FREQ_RAMP, FREQ_DRAMP]),
        ("Duty", &[DUTY, DUTY_RAMP]),
        ("Vibrato", &[VIB_STRENGTH, VIB_SPEED, VIB_DELAY]),
        ("Envelope", &[ENV_ATTACK, ENV_SUSTAIN, ENV_DECAY, ENV_PUNCH]),
        (
            "Filters",
            &[LPF_RES, LPF_FREQ, LPF_RAMP, HPF_FREQ, HPF_RAMP],
        ),
        ("Phaser", &[PHA_OFFSET, PHA_RAMP]),
        ("Repeat & arp", &[REPEAT, ARP_SPEED, ARP_MOD]),
    ];
    for (name, ks) in groups {
        let rest: Vec<usize> = ks.iter().copied().filter(|k| !MACROS.contains(k)).collect();
        if rest.is_empty() {
            continue;
        }
        b.card(Tab::Advanced, name);
        for k in rest {
            one(b, k, None);
        }
    }
}

fn analog(b: &mut B) {
    use crate::gen2::{AWave, ANALOG};
    b.card(Tab::Main, "Oscillators").viz = Some(Box::new(|i| {
        let a = an(i);
        let lv = [a.p[8], a.p[9], a.p[10]];
        let n = lv.iter().sum::<f32>().max(0.3);
        Viz::Wave(
            (0..64)
                .map(|k| {
                    let t = k as f32 / 64.0;
                    let s: f32 = a
                        .waves
                        .iter()
                        .zip(lv)
                        .map(|(w, l)| {
                            l * match w {
                                AWave::Saw => 1.0 - 2.0 * t,
                                AWave::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
                                AWave::Square => sq(t, 0.5),
                                AWave::Pulse => sq(t, 0.25),
                            }
                        })
                        .sum();
                    (s / n).clamp(-1.0, 1.0)
                })
                .collect(),
        )
    }));
    for o in 0..3 {
        let gs: (Get, Set) = (
            Box::new(move |i| {
                AWave::ALL
                    .iter()
                    .position(|w| *w == an(i).waves[o])
                    .unwrap_or(0) as f32
            }),
            Box::new(move |i, v| {
                if let (Some(a), Some(w)) = (an_mut(i), AWave::ALL.get(v.round() as usize)) {
                    a.waves[o] = *w;
                }
            }),
        );
        let opts = AWave::ALL.iter().map(|w| w.name().to_string()).collect();
        let p = b.choice(&format!("an.wave{o}"), &format!("Osc {}", o + 1), opts, gs);
        choice_lock!(p, an, an_mut, o as u32);
    }
    b.toggle("an.sync", "Sync 2>1", fldb!(an, an_mut, sync))
        .tip("hard-sync oscillator 2 to oscillator 1");
    for k in [8, 9, 10, 11, 12, 13, 14, 4, 5, 7, 23, 24] {
        gen_knob!(b, "an", an, an_mut, ANALOG, k);
    }
    b.card(Tab::Main, "Filter").viz = Some(Box::new(|i| {
        let a = an(i);
        Viz::Filter(
            ANALOG[0].map(a.p[0]),
            ANALOG[1].map(a.p[1]),
            mode_ix(a.mode),
        )
    }));
    let modes = [FilterMode::Lp, FilterMode::Bp, FilterMode::Hp];
    b.choice(
        "an.mode",
        "Mode",
        names(&modes),
        fldc!(an, an_mut, modes, mode),
    );
    b.toggle("an.vintage", "Vintage", fldb!(an, an_mut, vintage))
        .tip("saturated, 2x oversampled ladder (else clean)");
    b.toggle("an.pole4", "24 dB", fldb!(an, an_mut, pole4));
    for k in [0, 1, 2, 6, 22] {
        gen_knob!(b, "an", an, an_mut, ANALOG, k);
    }
    b.card(Tab::Main, "Amp envelope").viz = Some(Box::new(|i| {
        let a = an(i);
        let m = |k: usize| ANALOG[k].map(a.p[k]);
        Viz::Adsr(m(18), m(19), m(20), m(21))
    }));
    for k in 18..22 {
        gen_knob!(b, "an", an, an_mut, ANALOG, k);
    }
    b.card(Tab::Modulation, "Filter envelope").viz = Some(Box::new(|i| {
        let a = an(i);
        let m = |k: usize| ANALOG[k].map(a.p[k]);
        Viz::Adsr(m(15), m(3), m(16), m(17))
    }));
    for k in [15, 3, 16, 17] {
        gen_knob!(b, "an", an, an_mut, ANALOG, k);
    }
    b.card(Tab::Main, "Voice");
    b.knob(
        "an.level",
        "Level",
        0.0,
        4.0,
        false,
        1.0,
        "",
        fld!(an, an_mut, level),
    );
    b.knob(
        "gen.glide",
        "Glide",
        0.0,
        1000.0,
        false,
        60.0,
        "ms",
        flds!(1000.0, pa, pa_mut, glide),
    )
    .tip("portamento time (with Legato, or 3C in the Tracker)");
    b.toggle("gen.legato", "Legato", fldb!(pa, pa_mut, legato))
        .tip("overlapping notes glide instead of retriggering (mono)");
}

fn fm4(b: &mut B) {
    use crate::gen2::{fk, ALGOS, FM};
    b.card(Tab::Main, "Algorithm").viz = Some(Box::new(|i| Viz::Algo(fm(i).algo.min(7))));
    let gs: (Get, Set) = (
        Box::new(|i| fm(i).algo.min(7) as f32),
        Box::new(|i, v| {
            if let Some(f) = fm_mut(i) {
                f.algo = (v.round().max(0.0) as u8).min(7);
            }
        }),
    );
    let p = b.choice(
        "fm.algo",
        "Algorithm",
        ALGOS.iter().map(|a| a.2.to_string()).collect(),
        gs,
    );
    choice_lock!(p, fm, fm_mut, 0);
    b.card(Tab::Main, "Macros").note = "43xy".into();
    for k in 0..8 {
        gen_knob!(b, "fm", fm, fm_mut, FM, k);
    }
    b.card(Tab::Main, "Voice");
    b.knob(
        "fm.level",
        "Level",
        0.0,
        4.0,
        false,
        1.0,
        "",
        fld!(fm, fm_mut, level),
    );
    for o in 0..4 {
        b.card(Tab::Modulation, &format!("Operator {}", o + 1)).viz = Some(Box::new(move |i| {
            let f = fm(i);
            let m = |j: usize| FM[fk(o, j)].map(f.p[fk(o, j)]);
            Viz::Adsr(m(2), m(3), m(4), m(5))
        }));
        for j in 0..6 {
            gen_knob!(b, "fm", fm, fm_mut, FM, fk(o, j));
        }
    }
}

fn noise(b: &mut B) {
    use crate::gen2::{Color, NOISE};
    b.card(Tab::Main, "Noise");
    let opts = Color::ALL.iter().map(|c| c.name().to_string()).collect();
    let p = b.choice(
        "nz.color",
        "Colour",
        opts,
        fldc!(nz, nz_mut, Color::ALL, color),
    );
    choice_lock!(p, nz, nz_mut, 0);
    for k in [8, 14, 6, 7] {
        gen_knob!(b, "nz", nz, nz_mut, NOISE, k);
    }
    b.card(Tab::Main, "Filter").viz = Some(Box::new(|i| {
        let n = nz(i);
        Viz::Filter(NOISE[1].map(n.p[1]), NOISE[3].map(n.p[3]), mode_ix(n.mode))
    }));
    b.toggle("nz.ladder", "Ladder", fldb!(nz, nz_mut, ladder))
        .tip("Moog ladder (else the SVF)");
    let modes = [FilterMode::Lp, FilterMode::Bp, FilterMode::Hp];
    b.choice(
        "nz.mode",
        "Mode",
        names(&modes),
        fldc!(nz, nz_mut, modes, mode),
    );
    b.toggle("nz.key_track", "Key track", fldb!(nz, nz_mut, key_track))
        .tip("the note moves the cutoff (C-4 = as set)");
    for k in [1, 2, 3] {
        gen_knob!(b, "nz", nz, nz_mut, NOISE, k);
    }
    b.card(Tab::Main, "Shape");
    b.toggle("nz.hold", "Hold", fldb!(nz, nz_mut, hold))
        .tip("stay at the peak while the note is held (wind, vinyl); else one-shot");
    for k in [0, 4, 5, 11] {
        gen_knob!(b, "nz", nz, nz_mut, NOISE, k);
    }
    b.card(Tab::Main, "Voice");
    b.knob(
        "nz.level",
        "Level",
        0.0,
        4.0,
        false,
        1.0,
        "",
        fld!(nz, nz_mut, level),
    );
    b.card(Tab::Modulation, "Gusts");
    for k in [9, 10] {
        gen_knob!(b, "nz", nz, nz_mut, NOISE, k);
    }
    b.card(Tab::Advanced, "Thump & hiss");
    for k in [12, 13] {
        gen_knob!(b, "nz", nz, nz_mut, NOISE, k);
    }
}

/// a StringWave choice (index in, index out)
fn sw_choice<'b>(
    b: &'b mut B,
    key: &str,
    name: &str,
    opts: Vec<String>,
    get: impl Fn(&crate::strings::StringsP) -> usize + 'static,
    set: impl Fn(&mut crate::strings::StringsP, usize) + 'static,
) -> &'b mut Param {
    let n = opts.len();
    let gs: (Get, Set) = (
        Box::new(move |i| get(sw(i)) as f32),
        Box::new(move |i, v| {
            if let Some(x) = sw_mut(i) {
                set(x, (v.round().max(0.0) as usize).min(n - 1));
            }
        }),
    );
    b.choice(key, name, opts, gs)
}

/// a StringWave switch
fn sw_toggle<'b>(
    b: &'b mut B,
    key: &str,
    name: &str,
    get: impl Fn(&crate::strings::StringsP) -> bool + 'static,
    set: impl Fn(&mut crate::strings::StringsP, bool) + 'static,
) -> &'b mut Param {
    let gs: (Get, Set) = (
        Box::new(move |i| get(sw(i)) as u8 as f32),
        Box::new(move |i, v| {
            if let Some(x) = sw_mut(i) {
                set(x, v > 0.5);
            }
        }),
    );
    b.toggle(key, name, gs)
}

/// StringWave's placeholder editor: the generic cards (Layers on Main,
/// Filter & Mod on Modulation, FX & Master on FX). The styled panel waits
/// on the built-in instrument look.
fn stringwave(b: &mut B) {
    use crate::strings::*;
    let k = |b: &mut B, i: usize| {
        let p = gen_knob!(b, "sw", sw, sw_mut, STRINGS, i);
        // double-click resets to the factory default
        if let Ctl::Knob { def, .. } = &mut p.ctl {
            *def = StringsP::default().p[i];
        }
        p.badge = None;
    };
    let dests: Vec<String> = Dest::ALL.iter().map(|d| d.name().to_string()).collect();
    for l in 0..3usize {
        b.card(Tab::Main, ["Layer 1", "Layer 2", "Layer 3"][l]);
        sw_toggle(
            b,
            &format!("sw.on{l}"),
            "On",
            move |x| x.layer_on[l],
            move |x, v| x.layer_on[l] = v,
        );
        sw_choice(
            b,
            &format!("sw.wave{l}"),
            "Wave",
            Wave::ALL.iter().map(|w| w.name().to_string()).collect(),
            move |x| x.wave[l] as usize,
            move |x, v| x.wave[l] = Wave::ALL[v],
        )
        .tip("Ensemble: detuned saw section; Plucked: bright attack that dies down; Bowed: slow bow with rosin noise; Body: hollow wooden resonance");
        for j in [L_OCT, L_DET, L_TONE, L_LVL, L_PAN] {
            k(b, LAYER[l] + j);
        }
        sw_toggle(
            b,
            &format!("sw.mute{l}"),
            "Mute",
            move |x| x.layer_mute[l],
            move |x, v| x.layer_mute[l] = v,
        );
        sw_toggle(
            b,
            &format!("sw.solo{l}"),
            "Solo",
            move |x| x.layer_solo[l],
            move |x, v| x.layer_solo[l] = v,
        );
    }
    b.card(Tab::Main, "Undertones");
    sw_toggle(b, "sw.und", "On", |x| x.und_on, |x, v| x.und_on = v);
    sw_choice(
        b,
        "sw.suboct",
        "Sub octave",
        SubOct::ALL.iter().map(|w| w.name().to_string()).collect(),
        |x| x.sub_oct as usize,
        |x, v| x.sub_oct = SubOct::ALL[v],
    );
    for i in [SUB_LVL, DRONE_LVL, DRONE_TONE] {
        k(b, i);
    }
    sw_toggle(
        b,
        "sw.undmute",
        "Mute",
        |x| x.und_mute,
        |x, v| x.und_mute = v,
    );
    sw_toggle(
        b,
        "sw.undsolo",
        "Solo",
        |x| x.und_solo,
        |x, v| x.und_solo = v,
    );
    b.card(Tab::Main, "Amp envelope").viz = Some(Box::new(|i| {
        let x = sw(i);
        Viz::Adsr(x.get(AMP_A), x.get(AMP_D), x.get(AMP_S), x.get(AMP_R))
    }));
    for i in [AMP_A, AMP_D, AMP_S, AMP_R, VEL, VOICES] {
        k(b, i);
    }

    b.card(Tab::Modulation, "Filter").viz = Some(Box::new(|i| {
        let x = sw(i);
        Viz::Filter(x.get(CUTOFF), x.get(RESO), 0)
    }));
    sw_toggle(
        b,
        "sw.filter",
        "On",
        |x| x.filter_on,
        |x, v| x.filter_on = v,
    );
    sw_choice(
        b,
        "sw.slope",
        "Slope",
        vec!["12 dB".into(), "24 dB".into()],
        |x| x.f24 as usize,
        |x, v| x.f24 = v == 1,
    );
    for i in [CUTOFF, RESO, F_KEY] {
        k(b, i);
    }
    b.card(Tab::Modulation, "Filter envelope").viz = Some(Box::new(|i| {
        let x = sw(i);
        Viz::Adsr(x.get(F_A), x.get(F_D), x.get(F_S), x.get(F_R))
    }));
    for i in [F_ENV, F_A, F_D, F_S, F_R] {
        k(b, i);
    }
    for n in 0..2usize {
        b.card(Tab::Modulation, ["LFO 1", "LFO 2"][n]);
        sw_toggle(
            b,
            &format!("sw.lfo{n}"),
            "On",
            move |x| x.lfo_on[n],
            move |x, v| x.lfo_on[n] = v,
        );
        sw_choice(
            b,
            &format!("sw.lfow{n}"),
            "Shape",
            LfoWave::ALL.iter().map(|w| w.name().to_string()).collect(),
            move |x| x.lfo_wave[n] as usize,
            move |x, v| x.lfo_wave[n] = LfoWave::ALL[v],
        );
        k(b, LFO[n]);
        k(b, LFO[n] + 1);
        sw_choice(
            b,
            &format!("sw.lfod{n}"),
            "Target",
            dests.clone(),
            move |x| x.lfo_dest[n] as usize,
            move |x, v| x.lfo_dest[n] = Dest::ALL[v],
        );
    }
    b.card(Tab::Modulation, "Mod envelope");
    sw_toggle(b, "sw.me", "On", |x| x.me_on, |x, v| x.me_on = v);
    for i in [ME_A, ME_D, ME_AMT] {
        k(b, i);
    }
    sw_choice(
        b,
        "sw.med",
        "Target",
        dests,
        |x| x.me_dest as usize,
        |x, v| x.me_dest = Dest::ALL[v],
    );
    b.card(Tab::Modulation, "Mod wheel");
    k(b, WHEEL);

    b.card(Tab::Fx, "Chorus");
    sw_toggle(b, "sw.cho", "On", |x| x.cho_on, |x, v| x.cho_on = v);
    for i in [CHO_RATE, CHO_DEPTH, CHO_MIX] {
        k(b, i);
    }
    b.card(Tab::Fx, "Echo");
    sw_toggle(b, "sw.echo", "On", |x| x.echo_on, |x, v| x.echo_on = v);
    for i in [ECHO_TIME, ECHO_FB, ECHO_TONE, ECHO_MIX] {
        k(b, i);
    }
    b.card(Tab::Fx, "Reverb");
    sw_toggle(b, "sw.rev", "On", |x| x.rev_on, |x, v| x.rev_on = v);
    for i in [REV_SIZE, REV_DAMP, REV_MIX] {
        k(b, i);
    }
    b.card(Tab::Fx, "Master");
    k(b, MASTER);
}

/// a poly synth choice whose names come from `opts` (index in, index out)
fn ps_choice<'b>(
    b: &'b mut B,
    key: &str,
    name: &str,
    opts: Vec<String>,
    get: impl Fn(&crate::poly::PolyP) -> usize + 'static,
    set: impl Fn(&mut crate::poly::PolyP, usize) + 'static,
) -> &'b mut Param {
    let n = opts.len();
    let gs: (Get, Set) = (
        Box::new(move |i| get(ps(i)) as f32),
        Box::new(move |i, v| {
            let k = (v.round().max(0.0) as usize).min(n - 1);
            if let Some(g) = ge_mut(i) {
                if let GenEngine::Poly(x) = &mut g.engine {
                    set(x, k);
                }
                // the Tracker's glide / legato follow the voice mode
                g.sync_poly();
            }
        }),
    );
    b.choice(key, name, opts, gs)
}

/// the poly synth's cards
fn polysynth(b: &mut B) {
    use crate::poly::*;
    b.card(Tab::Synth, POLY_DISPLAY_NAME);
    b.custom("ps.panel", POLY_DISPLAY_NAME, Custom::PolyPanel)
        .tip("the synth's own editor: oscillators, blend, filter, envelopes, voice and routes");
    let k = |b: &mut B, i: usize| {
        gen_knob!(b, "ps", ps, ps_mut, POLY, i);
    };
    for o in 0..2usize {
        let base = OSC[o];
        b.card(Tab::Main, ["Osc 1", "Osc 2"][o]).viz = Some(Box::new(move |i| {
            let x = ps(i);
            let w = dsp::weights(x.get(base + SHAPE));
            let pw = x.get(base + PW);
            Viz::Wave((0..64).map(|s| dsp::wave(s as f32 / 64.0, pw, w)).collect())
        }));
        for j in [
            SHAPE, PW, OCT, PITCH, SUB, SUB_PW, SYNC, VOICES, DETUNE, WIDTH, OPAN,
        ] {
            k(b, base + j);
        }
        ps_choice(
            b,
            &format!("ps.reset{o}"),
            "Phase",
            vec!["Free".into(), "Reset".into()],
            move |x| x.reset[o] as usize,
            move |x, v| x.reset[o] = v == 1,
        )
        .tip("Reset: every note starts the oscillator at phase 0 (punchy); Free: random phases (wide unison)");
    }
    b.card(Tab::Main, "Blend & noise");
    ps_choice(
        b,
        "ps.blend",
        "Blend op",
        BlendOp::ALL.iter().map(|x| x.name().to_string()).collect(),
        |x| x.blend as usize,
        |x, v| x.blend = BlendOp::ALL[v],
    )
    .tip(&format!(
        "how oscillator 2 combines with oscillator 1. {}",
        BlendOp::ALL
            .iter()
            .map(|x| format!("{}: {}", x.name(), x.tip()))
            .collect::<Vec<_>>()
            .join("; ")
    ));
    for i in [BLEND, NOISE, COLOUR] {
        k(b, i);
    }
    b.card(Tab::Main, "Filter").viz = Some(Box::new(|i| {
        let x = ps(i);
        let m = match x.filter {
            PolyFilter::Hp2 | PolyFilter::Hp4 => 1,
            PolyFilter::Bp2 | PolyFilter::Bp4 => 2,
            _ => 0,
        };
        Viz::Filter(x.get(CUTOFF), x.get(RES), m)
    }));
    ps_choice(
        b,
        "ps.filter",
        "Mode",
        PolyFilter::ALL
            .iter()
            .map(|x| x.name().to_string())
            .collect(),
        |x| x.filter as usize,
        |x, v| x.filter = PolyFilter::ALL[v],
    )
    .tip("low-, band- or high-pass at 12 or 24 dB/octave, or band-reject");
    for i in [CUTOFF, RES, ENV_AMT, KEYTRK, VEL_CUT, FM_AMT, FM_FREQ] {
        k(b, i);
    }
    b.card(Tab::Main, "Drive & shaper");
    for i in [HP_CUT, HP_RES, DRIVE, FEEDBACK] {
        k(b, i);
    }
    ps_choice(
        b,
        "ps.shaper",
        "Shaper",
        ShaperMode::ALL
            .iter()
            .map(|x| x.name().to_string())
            .collect(),
        |x| x.shaper as usize,
        |x, v| x.shaper = ShaperMode::ALL[v],
    )
    .tip("the waveshaper after the filter: soft saturation, hard clip, wavefolder, asymmetric");
    for i in [SHAPER, SHAPER_MIX] {
        k(b, i);
    }
    b.card(Tab::Main, "Amp envelope").viz = Some(Box::new(|i| {
        let x = ps(i);
        Viz::Adsr(x.get(AEG), x.get(AEG + 1), x.get(AEG + 2), x.get(AEG + 3))
    }));
    for i in AEG..AEG + 4 {
        k(b, i);
    }
    b.card(Tab::Main, "Voice");
    ps_choice(
        b,
        "ps.mode",
        "Voice mode",
        VoiceMode::ALL.iter().map(|x| x.name().to_string()).collect(),
        |x| x.mode as usize,
        |x, v| x.mode = VoiceMode::ALL[v],
    )
    .tip("Poly: chords; Mono: one note, each retriggers; Legato: one note, overlapping notes glide without retriggering");
    ps_choice(
        b,
        "ps.polyphony",
        "Polyphony",
        (1..=MAX_POLY).map(|n| n.to_string()).collect(),
        |x| x.polyphony.clamp(1, MAX_POLY) as usize - 1,
        |x, v| x.polyphony = v as u8 + 1,
    )
    .tip("most notes at once per track (Poly mode)");
    for i in [GLIDE, VEL_AMP, GAIN, PAN] {
        k(b, i);
    }
    b.card(Tab::Modulation, "Filter envelope").viz = Some(Box::new(|i| {
        let x = ps(i);
        Viz::Adsr(x.get(FEG), x.get(FEG + 1), x.get(FEG + 2), x.get(FEG + 3))
    }));
    for i in FEG..FEG + 4 {
        k(b, i);
    }
    b.card(Tab::Modulation, "Mod routes");
    let dsts: Vec<String> = route_targets().map(knob_label).collect();
    for s in 0..MOD_SLOTS {
        ps_choice(
            b,
            &format!("ps.src{s}"),
            &format!("Src {}", s + 1),
            ModSrc::ALL.iter().map(|x| x.name().to_string()).collect(),
            move |x| x.routes[s].src as usize,
            move |x, v| x.routes[s].src = ModSrc::ALL[v],
        )
        .tip("modulation source: filter envelope, amp envelope, velocity or key");
        ps_choice(
            b,
            &format!("ps.dst{s}"),
            &format!("Dst {}", s + 1),
            dsts.clone(),
            move |x| x.routes[s].dst as usize,
            move |x, v| x.routes[s].dst = v as u8,
        )
        .tip("the knob this route moves");
        k(b, MOD_AMT + s);
    }
}

#[allow(clippy::needless_range_loop)] // `o` is captured by the closures
fn wave(b: &mut B) {
    use crate::gen_wave::{USER, WAVE};
    use crate::wavetable::BUILTIN;
    for o in 0..2usize {
        let (card, tag) = if o == 0 {
            ("Osc A", "A")
        } else {
            ("Osc B", "B")
        };
        b.card(Tab::Main, card).viz = Some(Box::new(move |i| {
            let w = wv(i);
            Viz::Wave(
                w.table(o)
                    .map(|t| t.frame_points(WAVE[o].map(w.p[o]), 64))
                    .unwrap_or_else(|| vec![0.0; 64]),
            )
        }));
        let gs: (Get, Set) = (
            Box::new(move |i| match wv(i).tables[o] {
                USER => BUILTIN.len() as f32,
                t => t as f32,
            }),
            Box::new(move |i, v| {
                if let Some(w) = wv_mut(i) {
                    let v = v.round() as usize;
                    if v < BUILTIN.len() {
                        w.tables[o] = v as u8;
                    } else if w.user[o].is_some() {
                        w.tables[o] = USER;
                    }
                }
            }),
        );
        let mut opts: Vec<String> = BUILTIN.iter().map(|n| n.to_string()).collect();
        opts.push("Imported".into());
        let p = b.choice(&format!("wv.table{o}"), &format!("Table {tag}"), opts, gs);
        p.tip = "the wavetable (built-in, or Imported: see the import button)".into();
        choice_lock!(p, wv, wv_mut, o as u32);
        b.custom(
            &format!("wv.import{o}"),
            "Import",
            Custom::WaveTable(o as u8),
        )
        .tip("import a single-cycle WAV or a Serum-style wavetable WAV (2048-sample frames)");
        let ks: &[usize] = if o == 0 {
            &[0, 8, 9, 10, 26]
        } else {
            &[1, 7, 11, 12, 27]
        };
        for &k in ks {
            gen_knob!(b, "wv", wv, wv_mut, WAVE, k);
        }
    }
    b.card(Tab::Main, "Unison & sub");
    for k in [13, 5, 6, 14, 15] {
        gen_knob!(b, "wv", wv, wv_mut, WAVE, k);
    }
    b.card(Tab::Main, "Filter").viz = Some(Box::new(|i| {
        let w = wv(i);
        Viz::Filter(WAVE[2].map(w.p[2]), WAVE[3].map(w.p[3]), mode_ix(w.mode))
    }));
    b.toggle("wv.filter_on", "Filter", fldb!(wv, wv_mut, filter_on))
        .tip("run the oscillators through the ladder filter");
    let modes = [FilterMode::Lp, FilterMode::Bp, FilterMode::Hp];
    b.choice(
        "wv.mode",
        "Mode",
        names(&modes),
        fldc!(wv, wv_mut, modes, mode),
    );
    b.toggle("wv.vintage", "Vintage", fldb!(wv, wv_mut, vintage))
        .tip("saturated, 2x oversampled ladder (else clean)");
    b.toggle("wv.pole4", "24 dB", fldb!(wv, wv_mut, pole4));
    for k in [2, 3, 4, 16, 25] {
        gen_knob!(b, "wv", wv, wv_mut, WAVE, k);
    }
    b.card(Tab::Main, "Amp envelope").viz = Some(Box::new(|i| {
        let w = wv(i);
        let m = |k: usize| WAVE[k].map(w.p[k]);
        Viz::Adsr(m(21), m(22), m(23), m(24))
    }));
    for k in 21..25 {
        gen_knob!(b, "wv", wv, wv_mut, WAVE, k);
    }
    b.card(Tab::Main, "Voice");
    b.knob(
        "wv.level",
        "Level",
        0.0,
        4.0,
        false,
        1.0,
        "",
        fld!(wv, wv_mut, level),
    );
    b.knob(
        "gen.glide",
        "Glide",
        0.0,
        1000.0,
        false,
        60.0,
        "ms",
        flds!(1000.0, pa, pa_mut, glide),
    )
    .tip("portamento time (with Legato, or 3C in the Tracker)");
    b.toggle("gen.legato", "Legato", fldb!(pa, pa_mut, legato))
        .tip("overlapping notes glide instead of retriggering (mono)");
    b.card(Tab::Modulation, "Filter envelope").viz = Some(Box::new(|i| {
        let w = wv(i);
        let m = |k: usize| WAVE[k].map(w.p[k]);
        Viz::Adsr(m(17), m(18), m(19), m(20))
    }));
    for k in [17, 18, 19, 20] {
        gen_knob!(b, "wv", wv, wv_mut, WAVE, k);
    }
}

/// a Grid knob slot `s` (macro or module parameter) with its default
fn gd_knob(b: &mut B, s: usize, def: &'static crate::gen::MacroDef, dflt: f32) {
    let bit = 1u64 << s;
    let gs: (Get, Set) = (
        Box::new(move |i| gd(i).p.get(s).copied().unwrap_or(0.0)),
        Box::new(move |i, v| {
            if let Some(x) = gd_mut(i).and_then(|g| g.p.get_mut(s)) {
                *x = v.clamp(0.0, 1.0);
            }
        }),
    );
    let lock: LockFns = (
        Box::new(move |i| gd(i).locks & bit != 0),
        Box::new(move |i| {
            if let Some(x) = gd_mut(i) {
                x.locks ^= bit;
            }
        }),
    );
    let p = b.mac(&format!("gd.{s}"), def, gs, lock, (s < 8).then_some(s), s);
    if let Ctl::Knob { def, .. } = &mut p.ctl {
        *def = dflt;
    }
}

fn grid(b: &mut B, g: &crate::grid::GridP) {
    use crate::grid::{spec, MACROS, MACRO_DEFS};
    b.card(Tab::Grid, "Grid");
    b.custom("gd.canvas", "Grid", Custom::GridCanvas)
        .tip("the patch: right-click to add modules, drag from an output to an input to cable");
    b.card(Tab::Main, "Macros");
    for (k, d) in MACRO_DEFS.iter().enumerate().take(MACROS) {
        gd_knob(b, k, d, 0.0);
    }
    for m in &g.modules {
        let sp = spec(m.kind);
        if sp.params.is_empty() {
            continue;
        }
        b.card(Tab::Main, &g.label(m.id));
        for (j, d) in sp.params.iter().enumerate() {
            let s = m.slots[j] as usize;
            if s < crate::gen::GEN_KNOBS {
                gd_knob(b, s, d, d.unmap(sp.defaults[j]));
            }
        }
    }
    b.card(Tab::Main, "Voice");
    b.knob(
        "gd.level",
        "Level",
        0.0,
        4.0,
        false,
        1.0,
        "",
        fld!(gd, gd_mut, level),
    );
    b.knob(
        "gen.glide",
        "Glide",
        0.0,
        1000.0,
        false,
        60.0,
        "ms",
        flds!(1000.0, pa, pa_mut, glide),
    )
    .tip("portamento time (with Legato, or 3C in the Tracker)");
    b.toggle("gen.legato", "Legato", fldb!(pa, pa_mut, legato))
        .tip("overlapping notes glide instead of retriggering (mono)");
}

/// the instrument's FX chain: one card per device on the FX tab
fn fx_chain(b: &mut B, inst: &Instrument, idx: usize) {
    b.card(Tab::Fx, "Chain").note = format!("{} device(s)", inst.fx.len());
    b.custom("fx.chain", "FX chain", Custom::FxChain)
        .tip("add effects, open the FX view (reorder, presets)");
    for (j, d) in inst.fx.iter().enumerate() {
        let card = b.card(Tab::Fx, &format!("{} {}", j + 1, d.title()));
        if d.kind == crate::fx::FxKind::Plugin {
            card.note = "plugin effect: parameters and editor in the FX view".into();
        }
        if d.kind == crate::fx::FxKind::Grid {
            card.note = "Grid FX: edit its patch in the FX view".into();
        }
        let gs: (Get, Set) = (
            Box::new(move |i| {
                if i.fx.get(j).is_some_and(|d| d.enabled) {
                    1.0
                } else {
                    0.0
                }
            }),
            Box::new(move |i, v| {
                if let Some(d) = i.fx.get_mut(j) {
                    d.enabled = v > 0.5;
                }
            }),
        );
        b.toggle(&format!("fx.{j}.on"), "On", gs)
            .tip("bypass / enable this device");
        for (k, info) in crate::fx::params(d.kind).iter().enumerate() {
            let norm = move |i: &Instrument| {
                i.fx.get(j)
                    .and_then(|d| d.params.get(k))
                    .copied()
                    .unwrap_or(0.0)
            };
            let key = format!("fx.{j}.{}", info.key);
            if info.choices.is_empty() {
                let gs: (Get, Set) = (
                    Box::new(norm),
                    Box::new(move |i, v| {
                        if let Some(x) = i.fx.get_mut(j).and_then(|d| d.params.get_mut(k)) {
                            *x = v.clamp(0.0, 1.0);
                        }
                    }),
                );
                let p = b.knob(
                    &key,
                    info.name,
                    0.0,
                    1.0,
                    false,
                    info.to_norm(info.def),
                    "",
                    gs,
                );
                p.fmt = Some(Box::new(move |v| info.format(v)));
                p.target = Some(Target::Fx(idx, j, k));
            } else {
                let gs: (Get, Set) = (
                    Box::new(move |i| info.to_real(norm(i))),
                    Box::new(move |i, v| {
                        if let Some(x) = i.fx.get_mut(j).and_then(|d| d.params.get_mut(k)) {
                            *x = info.to_norm(v.round());
                        }
                    }),
                );
                b.choice(
                    &key,
                    info.name,
                    info.choices.iter().map(|s| s.to_string()).collect(),
                    gs,
                );
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn inst(kind: InstrumentKind) -> Instrument {
        Instrument {
            name: "t".into(),
            volume: 0.8,
            transpose: 0,
            fx: vec![
                crate::fx::Device::new(crate::fx::FxKind::Delay),
                crate::fx::Device::new(crate::fx::FxKind::Filter),
            ],
            kind,
        }
    }

    fn gen(name: &str) -> Instrument {
        inst(InstrumentKind::Gen(crate::gen::preset(name).unwrap()))
    }

    /// one instrument of every type the editor knows
    pub(crate) fn all_types() -> Vec<(&'static str, Instrument)> {
        let mut tf = crate::voice::preset("Metal Demon").unwrap();
        tf.transform.res.keyed = false;
        let ladder = crate::synth::Patch {
            filter_model: crate::ladder::FilterModel::Vintage,
            ..Default::default()
        };
        vec![
            ("Synth", inst(InstrumentKind::Synth(Default::default()))),
            ("Moog", inst(InstrumentKind::Synth(ladder))),
            ("Voice", inst(InstrumentKind::Voice(Default::default()))),
            ("Voice", inst(InstrumentKind::Voice(tf))),
            ("Drum", gen("Psy Zap Kick")),
            ("Kit", gen("Psy Kit")),
            ("Chip FX", gen("Psy Laser")),
            ("Analog", gen("Psy Rolling Bass")),
            ("FM-4", gen("Metallic Bell")),
            ("Noise", gen("White Riser")),
            ("Wave", gen("Wave Supersaw")),
            ("Grid", gen("Grid Saw Bass")),
            (
                crate::poly::POLY_DISPLAY_NAME,
                gen(concat!(crate::poly_name!(), " Pluck")),
            ),
            (
                "Sample",
                inst(InstrumentKind::Sample {
                    file: "x.wav".into(),
                    rate: 48000,
                    data: vec![0.0, 0.5, -0.5, 0.0].into(),
                    base_note: 49,
                    lp: Default::default(),
                }),
            ),
            (
                "CLAP",
                inst(InstrumentKind::Plugin(crate::plugin::PluginPatch {
                    format: "clap".into(),
                    id: "x".into(),
                    name: "X".into(),
                    vendor: String::new(),
                    path: String::new(),
                    state: String::new(),
                    params: vec![],
                    macros: vec![],
                })),
            ),
        ]
    }

    /// every property the old sidebar editor (0.2.3.18) could edit, per type
    fn required(tag: &str) -> Vec<String> {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let synth_both = s(&[
            "osc1_oct",
            "pulse_width",
            "filter",
            "filter_model",
            "ladder_12db",
            "cutoff",
            "resonance",
            "env_amount",
            "amp.a",
            "amp.d",
            "amp.s",
            "amp.r",
            "fenv.d",
            "drive",
            "vel_cutoff",
            "start_delay",
            "glide",
            "legato",
            "accent",
            "accent_vel",
            "lfo.shape",
            "lfo.sync",
            "lfo.rate",
            "lfo.cutoff",
            "lfo.amp",
            "lfo.pitch",
        ]);
        let gen_filter = s(&[
            "gen.dice",
            "gen.release",
            "gen.filter_on",
            "filter_model",
            "ladder_12db",
            "filter",
            "cutoff",
            "resonance",
        ]);
        let mut r = s(&["volume", "transpose", "fx.chain"]);
        match tag {
            "Synth" | "Moog" => {
                r.extend(synth_both);
                r.extend(s(&["osc1", "unison", "detune_cents", "width", "metal"]));
            }
            "Voice" => {
                r.extend(synth_both);
                r.extend(s(&[
                    "vowels",
                    "words",
                    "throat",
                    "register",
                    "source",
                    "step",
                    "rate",
                    "morph_ms",
                    "babble",
                    "shift",
                    "sharp",
                    "breath",
                    "tension",
                    "jitter",
                    "lfo_vowel",
                    "lfo_shift",
                    "stutter_len",
                    "stutter_repeats",
                    "stutter_pitch",
                    "chop_rate",
                    "chop_duty",
                    "tf.blend",
                    "tf.order",
                    "tf.res.amount",
                    "tf.res.ratios",
                    "tf.res.keyed",
                    "tf.res.tune",
                    "tf.res.hz",
                    "tf.res.decay_ms",
                    "tf.res.damp",
                    "tf.res.spread",
                    "tf.dig.amount",
                    "tf.dig.drive_db",
                    "tf.dig.fold",
                    "tf.dig.bits",
                    "tf.dig.rate",
                    "tf.dig.alias",
                    "tf.dig.ring",
                    "tf.dig.ring_ratio",
                    "tf.dig.hp",
                    "tf.dig.lp",
                    "tf.sub.amount",
                    "tf.sub.mode",
                    "tf.sub.octave",
                    "tf.sub.drive",
                    "tf.sub.growl",
                    "tf.sub.xover",
                    "tf.sub.formant_drop",
                ]));
            }
            "Drum" | "Kit" => {
                r.extend(gen_filter);
                r.extend(s(&["drum.kind", "drum.model", "drum.level"]));
                r.extend((0..8).map(|k| format!("drum.m{k}")));
                r.push(
                    if tag == "Kit" {
                        "kit.piece"
                    } else {
                        "drum.follow_note"
                    }
                    .into(),
                );
            }
            "Chip FX" => {
                r.extend(gen_filter);
                r.extend(s(&[
                    "chipfx.category",
                    "chipfx.wave",
                    "chipfx.follow_note",
                    "chipfx.level",
                ]));
                r.extend((0..crate::chipfx::N).map(|k| format!("sfx.{k}")));
            }
            "Analog" => {
                r.extend(s(&[
                    "gen.dice",
                    "an.wave0",
                    "an.wave1",
                    "an.wave2",
                    "an.sync",
                    "an.vintage",
                    "an.pole4",
                    "an.mode",
                    "gen.glide",
                    "gen.legato",
                    "an.level",
                ]));
                r.extend((0..crate::gen2::AN).map(|k| format!("an.{k}")));
            }
            "FM-4" => {
                r.extend(s(&["gen.dice", "fm.algo", "fm.level"]));
                r.extend((0..crate::gen2::FN).map(|k| format!("fm.{k}")));
            }
            "Noise" => {
                r.extend(s(&[
                    "gen.dice",
                    "nz.color",
                    "nz.ladder",
                    "nz.mode",
                    "nz.hold",
                    "nz.key_track",
                    "nz.level",
                ]));
                r.extend((0..crate::gen2::NN).map(|k| format!("nz.{k}")));
            }
            "Wave" => {
                r.extend(s(&[
                    "gen.dice",
                    "wv.table0",
                    "wv.table1",
                    "wv.import0",
                    "wv.import1",
                    "wv.filter_on",
                    "wv.mode",
                    "wv.vintage",
                    "wv.pole4",
                    "gen.glide",
                    "gen.legato",
                    "wv.level",
                ]));
                r.extend((0..crate::gen_wave::WN).map(|k| format!("wv.{k}")));
            }
            "Grid" => {
                r.extend(s(&[
                    "gen.dice",
                    "gd.canvas",
                    "gd.level",
                    "gen.glide",
                    "gen.legato",
                ]));
                r.extend((0..crate::grid::MACROS).map(|k| format!("gd.{k}")));
                let Some(crate::gen::GenPatch {
                    engine: crate::gen::GenEngine::Grid(g),
                    ..
                }) = crate::gen::preset("Grid Saw Bass")
                else {
                    unreachable!()
                };
                r.extend(g.knobs().into_iter().map(|(k, _)| format!("gd.{k}")));
            }
            crate::poly::POLY_DISPLAY_NAME => {
                r.extend(s(&[
                    "gen.dice",
                    "ps.panel",
                    "ps.reset0",
                    "ps.reset1",
                    "ps.blend",
                    "ps.filter",
                    "ps.shaper",
                    "ps.mode",
                    "ps.polyphony",
                ]));
                for k in 0..crate::poly::MOD_SLOTS {
                    r.push(format!("ps.src{k}"));
                    r.push(format!("ps.dst{k}"));
                }
                r.extend((0..crate::poly::PN).map(|k| format!("ps.{k}")));
            }
            "Sample" => r.push("sample.info".into()),
            "CLAP" | "VST3" => r.extend(s(&["plugin.status", "plugin.macros", "plugin.params"])),
            _ => panic!("unknown type {tag}"),
        }
        r
    }

    #[test]
    fn every_old_editor_property_is_in_the_new_panel() {
        for (tag, i) in all_types() {
            assert_eq!(type_tag(&i.kind), tag);
            let l = build(&i, 0, 0);
            let keys: Vec<&str> = l.params.iter().map(|p| p.key.as_str()).collect();
            for k in required(tag) {
                assert!(
                    keys.contains(&k.as_str()),
                    "{tag}: '{k}' missing from the panel"
                );
            }
            // each FX device: power + every parameter
            for (j, d) in i.fx.iter().enumerate() {
                assert!(keys.contains(&format!("fx.{j}.on").as_str()));
                for info in crate::fx::params(d.kind) {
                    let k = format!("fx.{j}.{}", info.key);
                    assert!(keys.contains(&k.as_str()), "{tag}: {k}");
                }
            }
            // keys unique, every param on an existing card of its tab
            let mut seen = std::collections::HashSet::new();
            for p in &l.params {
                assert!(seen.insert(p.key.clone()), "{tag}: duplicate key {}", p.key);
                assert!(
                    l.cards.iter().any(|c| c.tab == p.tab && c.name == p.card),
                    "{tag}: {} on a missing card {}",
                    p.key,
                    p.card
                );
                assert!(!p.name.is_empty());
            }
            for c in &l.cards {
                assert!(
                    l.card_params(c).next().is_some(),
                    "{tag}: empty card {}",
                    c.name
                );
            }
            let per_tab: Vec<String> = Tab::ALL
                .iter()
                .map(|t| {
                    let n = l.params.iter().filter(|p| p.tab == *t).count();
                    format!("{} {n}", t.name())
                })
                .collect();
            println!(
                "{tag}: {} parameters, {} cards ({})",
                l.params.len(),
                l.cards.len(),
                per_tab.join(", ")
            );
        }
    }

    #[test]
    fn generator_knobs_offer_add_automation_lane() {
        use crate::automation::Target;
        // every knob of every generator type carries a Target::Gen that
        // names the same knob in crate::gen::gen_knobs
        for (tag, i) in all_types() {
            let InstrumentKind::Gen(g) = &i.kind else {
                continue;
            };
            let knobs = crate::gen::gen_knobs(g);
            let l = build(&i, 4, 2);
            let mut n = 0;
            for p in &l.params {
                if let Some(Target::Gen(idx, k)) = p.target {
                    assert_eq!(idx, 4);
                    let name = &knobs.iter().find(|x| x.0 == k).expect("knob listed").1;
                    assert!(
                        name.ends_with(p.name.as_str()),
                        "{tag}: {} vs {name}",
                        p.name
                    );
                    n += 1;
                }
            }
            assert!(n >= 8, "{tag}: only {n} automatable knobs");
        }
        // the Analog cutoff is knob 0
        let a = inst(InstrumentKind::Gen(crate::gen::GenPatch {
            engine: crate::gen::GenEngine::Analog(Default::default()),
            ..Default::default()
        }));
        let l = build(&a, 0, 0);
        let cut = l.params.iter().find(|p| p.key == "an.0").unwrap();
        assert_eq!(cut.target, Some(Target::Gen(0, 0)));
        assert_eq!(cut.name, "Cutoff");
    }

    #[test]
    fn kit_pieces_each_get_their_own_knobs() {
        let i = gen("Psy Kit");
        let GenEngine::Kit(k0) = crate::gen::preset("Psy Kit").unwrap().engine else {
            panic!()
        };
        for sel in 0..8 {
            let l = build(&i, 0, sel);
            let mut j = i.clone();
            (l.find("drum.m0").unwrap().set)(&mut j, 0.123);
            let InstrumentKind::Gen(g) = &j.kind else {
                panic!()
            };
            let GenEngine::Kit(k) = &g.engine else {
                panic!()
            };
            for (o, piece) in k.pieces.iter().enumerate() {
                if o == sel {
                    assert!((piece.macros[0] - 0.123).abs() < 1e-6);
                } else {
                    assert_eq!(piece, &k0.pieces[o]);
                }
            }
        }
    }

    /// every knob / choice / toggle writes the instrument and reads back
    #[test]
    fn every_parameter_round_trips_and_changes_the_instrument() {
        let mut n = 0;
        for (tag, i0) in all_types() {
            let l = build(&i0, 0, 0);
            for p in &l.params {
                let mut i = i0.clone();
                let cur = (p.get)(&i);
                let want = match &p.ctl {
                    Ctl::Custom(_) => continue,
                    Ctl::Toggle => 1.0 - cur.round(),
                    Ctl::Choice(o) => {
                        // "other" (a value not in the list) cannot be picked
                        let m = o.iter().filter(|x| x.as_str() != "other").count();
                        assert!(m >= 2, "{tag}: {} has one choice", p.key);
                        ((cur.round() as usize + 1) % m) as f32
                    }
                    Ctl::Knob { lo, hi, step, .. } => {
                        let a = if *step >= 1.0 {
                            ((lo + hi) / 2.0).round()
                        } else {
                            lo + (hi - lo) * 0.37
                        };
                        if (a - cur).abs() < 1e-3 {
                            if *step >= 1.0 {
                                lo + step
                            } else {
                                lo + (hi - lo) * 0.81
                            }
                        } else {
                            a
                        }
                    }
                };
                (p.set)(&mut i, want);
                assert_ne!(i, i0, "{tag}: {} did not change the instrument", p.key);
                n += 1;
                if p.key == "drum.kind" {
                    continue; // a new kind starts from its default drum
                }
                let back = (p.get)(&i);
                assert!(
                    (back - want).abs() <= 2e-3 * (1.0 + want.abs()),
                    "{tag}: {} wrote {want} read {back}",
                    p.key
                );
                if let Some((locked, toggle)) = &p.lock {
                    let a = locked(&i);
                    toggle(&mut i);
                    assert_ne!(a, locked(&i), "{tag}: lock of {}", p.key);
                }
                if let Ctl::Knob { lo, hi, def, .. } = p.ctl {
                    assert!(
                        def >= lo && def <= hi,
                        "{tag}: default of {} out of range",
                        p.key
                    );
                }
            }
        }
        assert!(n > 300, "only {n} parameters checked");
    }

    #[test]
    fn knob_mapping_round_trips_including_log_from_zero() {
        for (lo, hi, log) in [(0.0, 2.0, true), (20.0, 18000.0, true), (-1.0, 1.0, false)] {
            for k in 0..=10 {
                let n = k as f32 / 10.0;
                let v = from_norm(n, lo, hi, log);
                assert!((to_norm(v, lo, hi, log) - n).abs() < 1e-3, "{lo} {hi} {n}");
            }
        }
        assert_eq!(from_norm(0.0, 0.0, 2.0, true), 0.0);
        assert_eq!(fmt_unit(2400.0, "Hz"), "2.40 kHz");
        assert_eq!(fmt_unit(0.25, "s"), "250 ms");
    }
}
