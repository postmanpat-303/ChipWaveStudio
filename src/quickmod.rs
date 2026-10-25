//! Quick-mod: the one "Movement" list of an instrument and the "Modulate
//! <param> with…" popover, mapped onto the data each engine already saves.
//!
//! Nothing here adds to a song file. A movement is either one of the
//! engine's own built-in routes (WaveForge's eight per-note slots,
//! StringWave's two LFOs and mod envelope, the ChipWave synth's LFO) or a
//! song-level modulator + route (`crate::modulation`), which already works
//! on every modulatable knob. [`EngineMod`] is the adapter per engine.
use crate::automation::Target;
use crate::gen::GenEngine;
use crate::model::{Instrument, InstrumentKind};
use crate::modulation::{LfoShape, ModKind, Modulation};

/// what the popover offers as a source
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    #[default]
    Lfo,
    Envelope,
    Random,
    Velocity,
    ModWheel,
}

impl Source {
    pub const ALL: [Source; 5] = [
        Source::Lfo,
        Source::Envelope,
        Source::Random,
        Source::Velocity,
        Source::ModWheel,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Source::Lfo => "LFO",
            Source::Envelope => "Envelope",
            Source::Random => "Random",
            Source::Velocity => "Velocity",
            Source::ModWheel => "Mod wheel",
        }
    }
    pub fn kind(self) -> ModKind {
        match self {
            Source::Lfo => ModKind::Lfo,
            Source::Envelope => ModKind::Envelope,
            Source::Random => ModKind::Random,
            Source::Velocity => ModKind::Velocity,
            Source::ModWheel => ModKind::ModWheel,
        }
    }
    /// shape and rate apply
    pub fn has_rate(self) -> bool {
        matches!(self, Source::Lfo | Source::Random)
    }
}

/// what the popover asks for
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub source: Source,
    pub shape: LfoShape,
    /// beats per cycle (0 = free, `rate` Hz)
    pub sync: f32,
    pub rate: f32,
    /// -1..1 of the knob's travel
    pub amount: f32,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            source: Source::Lfo,
            shape: LfoShape::Sine,
            sync: 1.0,
            rate: 1.0,
            amount: 0.25,
        }
    }
}

/// the synth LFO's three fixed destinations (ChipWave synth)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipDst {
    Cutoff,
    Amp,
    Pitch,
}

/// one of an engine's own routes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// WaveForge per-note slot
    Poly(u8),
    /// StringWave LFO 1 / 2
    StrLfo(u8),
    /// StringWave mod envelope
    StrEnv,
    /// ChipWave synth LFO onto one destination
    Chip(ChipDst),
}

/// a row of the Movement list
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowId {
    Native(Native),
    /// song-level route index
    Song(usize),
}

/// one movement as the list shows it
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: RowId,
    /// "LFO sine 1/4", "Filter env"
    pub source: String,
    /// "Cutoff"
    pub target: String,
    /// runs per voice (each note its own)
    pub per_note: bool,
    /// -1..1 (the list's bar, the knob ring)
    pub amount: f32,
    /// the amount in the engine's own words ("+25 %", "+1.0 oct")
    pub text: String,
    /// the song-level modulator index (its colour)
    pub modk: Option<usize>,
    /// the knobs it moves (rings)
    pub knobs: Vec<Target>,
}

/// what happened on Add
#[derive(Clone, Debug, PartialEq)]
pub struct Added {
    pub row: RowId,
    /// shown to the user: where it went when that was not the obvious place
    pub note: Option<String>,
}

/// The adapter: an engine's built-in routes as Movement rows.
pub trait EngineMod {
    /// shown in messages ("WaveForge has 8 per-note slots")
    fn engine(&self) -> &'static str;
    fn rows(&self, idx: usize) -> Vec<Row>;
    /// add natively: None = not something this engine does itself (use a
    /// song-level modulator), Some(Err(why)) = it would, but its slots
    /// are full
    fn add(&mut self, target: Target, spec: &Spec, bpm: f32) -> Option<Result<Native, String>>;
    fn remove(&mut self, id: Native);
    fn set_amount(&mut self, id: Native, a: f32);
}

fn pct(a: f32) -> String {
    format!("{:+.0} %", a * 100.0)
}

/// "1/4", "1 bar", "2.0 Hz"
pub fn rate_text(sync: f32, rate: f32) -> String {
    const S: [(f32, &str); 8] = [
        (0.25, "1/16"),
        (0.5, "1/8"),
        (1.0, "1/4"),
        (2.0, "1/2"),
        (4.0, "1 bar"),
        (8.0, "2 bars"),
        (16.0, "4 bars"),
        (32.0, "8 bars"),
    ];
    if sync > 0.0 {
        S.iter()
            .find(|x| (x.0 - sync).abs() < 1e-4)
            .map(|x| x.1.to_string())
            .unwrap_or_else(|| format!("{sync} beats"))
    } else {
        format!("{rate:.2} Hz")
    }
}

// ------------------------------------------------------------- WaveForge

impl EngineMod for crate::poly::PolyP {
    fn engine(&self) -> &'static str {
        crate::poly::POLY_DISPLAY_NAME
    }
    fn rows(&self, idx: usize) -> Vec<Row> {
        use crate::poly::*;
        (0..MOD_SLOTS)
            .filter(|&s| self.routes[s].src != ModSrc::Off)
            .map(|s| {
                let r = self.routes[s];
                let a = POLY[MOD_AMT + s].map(self.p[MOD_AMT + s]);
                let d = r.dst as usize;
                Row {
                    id: RowId::Native(Native::Poly(s as u8)),
                    source: r.src.name().to_string(),
                    target: knob_label(d),
                    per_note: true,
                    amount: a,
                    text: pct(a),
                    modk: None,
                    knobs: vec![Target::Gen(idx, d)],
                }
            })
            .collect()
    }
    fn add(&mut self, target: Target, spec: &Spec, _bpm: f32) -> Option<Result<Native, String>> {
        use crate::poly::*;
        let Target::Gen(_, d) = target else {
            return None;
        };
        let src = match spec.source {
            Source::Envelope => ModSrc::Feg,
            Source::Velocity => ModSrc::Vel,
            _ => return None,
        };
        if d >= MOD_AMT {
            return None;
        }
        let Some(s) = (0..MOD_SLOTS).find(|&s| self.routes[s].src == ModSrc::Off) else {
            return Some(Err(format!(
                "{}'s {MOD_SLOTS} per-note slots are full",
                self.engine()
            )));
        };
        self.route(s, src, d, spec.amount.clamp(-1.0, 1.0));
        Some(Ok(Native::Poly(s as u8)))
    }
    fn remove(&mut self, id: Native) {
        use crate::poly::*;
        if let Native::Poly(s) = id {
            let s = s as usize;
            if s < MOD_SLOTS {
                self.routes[s] = Route::default();
                self.p[MOD_AMT + s] = POLY[MOD_AMT + s].unmap(0.0);
            }
        }
    }
    fn set_amount(&mut self, id: Native, a: f32) {
        use crate::poly::*;
        if let Native::Poly(s) = id {
            let s = s as usize;
            if s < MOD_SLOTS {
                self.p[MOD_AMT + s] = POLY[MOD_AMT + s].unmap(a.clamp(-1.0, 1.0));
            }
        }
    }
}

// ------------------------------------------------------------ StringWave

/// the StringWave knob(s) a destination moves
fn str_knobs(d: crate::strings::Dest) -> Vec<usize> {
    use crate::strings::*;
    match d {
        Dest::Cutoff => vec![CUTOFF],
        Dest::Resonance => vec![RESO],
        Dest::Amp => vec![MASTER],
        Dest::Chorus => vec![CHO_MIX],
        Dest::Echo => vec![ECHO_MIX],
        Dest::Reverb => vec![REV_MIX],
        Dest::Tone => LAYER.iter().map(|b| b + L_TONE).collect(),
        Dest::Detune => LAYER.iter().map(|b| b + L_DET).collect(),
        Dest::Pan => LAYER.iter().map(|b| b + L_PAN).collect(),
        Dest::Undertones => vec![SUB_LVL, DRONE_LVL],
        Dest::LayerMix => LAYER.iter().map(|b| b + L_LVL).collect(),
        Dest::Pitch => vec![],
    }
}

/// the destination a StringWave knob is
fn str_dest(k: usize) -> Option<crate::strings::Dest> {
    crate::strings::Dest::ALL
        .into_iter()
        .filter(|d| *d != crate::strings::Dest::LayerMix)
        .find(|d| str_knobs(*d).contains(&k))
}

fn str_wave(spec: &Spec) -> crate::strings::LfoWave {
    use crate::strings::LfoWave;
    if spec.source == Source::Random {
        return LfoWave::SampleHold;
    }
    match spec.shape {
        LfoShape::Sine => LfoWave::Sine,
        LfoShape::Triangle => LfoWave::Triangle,
        LfoShape::SawUp | LfoShape::SawDown => LfoWave::Saw,
        LfoShape::Square => LfoWave::Square,
    }
}

impl EngineMod for crate::strings::StringsP {
    fn engine(&self) -> &'static str {
        crate::strings::STRINGS_DISPLAY_NAME
    }
    fn rows(&self, idx: usize) -> Vec<Row> {
        use crate::strings::*;
        let mut v = vec![];
        let knobs = |d: Dest| {
            str_knobs(d)
                .into_iter()
                .map(|k| Target::Gen(idx, k))
                .collect()
        };
        for n in 0..2 {
            let depth = STRINGS[LFO[n] + 1].map(self.p[LFO[n] + 1]);
            if self.lfo_on[n] && depth > 0.0 {
                let hz = STRINGS[LFO[n]].map(self.p[LFO[n]]);
                v.push(Row {
                    id: RowId::Native(Native::StrLfo(n as u8)),
                    source: format!(
                        "LFO {} {} {hz:.2} Hz",
                        n + 1,
                        self.lfo_wave[n].name().to_lowercase()
                    ),
                    target: self.lfo_dest[n].name().to_string(),
                    per_note: false,
                    amount: depth,
                    text: format!("{:.0} %", depth * 100.0),
                    modk: None,
                    knobs: knobs(self.lfo_dest[n]),
                });
            }
        }
        let amt = STRINGS[ME_AMT].map(self.p[ME_AMT]);
        if self.me_on && amt.abs() > 1e-4 {
            v.push(Row {
                id: RowId::Native(Native::StrEnv),
                source: "Mod env".into(),
                target: self.me_dest.name().to_string(),
                per_note: true,
                amount: amt,
                text: pct(amt),
                modk: None,
                knobs: knobs(self.me_dest),
            });
        }
        v
    }
    fn add(&mut self, target: Target, spec: &Spec, bpm: f32) -> Option<Result<Native, String>> {
        use crate::strings::*;
        let Target::Gen(_, k) = target else {
            return None;
        };
        let dest = str_dest(k)?;
        match spec.source {
            Source::Lfo | Source::Random => {
                let free = (0..2).find(|&n| {
                    !self.lfo_on[n] || STRINGS[LFO[n] + 1].map(self.p[LFO[n] + 1]) <= 0.0
                });
                let Some(n) = free else {
                    return Some(Err(format!("{}'s 2 LFOs are both in use", self.engine())));
                };
                let hz = if spec.sync > 0.0 {
                    bpm.max(1.0) / 60.0 / spec.sync
                } else {
                    spec.rate
                };
                self.lfo_on[n] = true;
                self.lfo_wave[n] = str_wave(spec);
                self.lfo_dest[n] = dest;
                self.set(&[
                    (LFO[n], hz.clamp(0.02, 20.0)),
                    (LFO[n] + 1, spec.amount.abs().clamp(0.0, 1.0)),
                ]);
                Some(Ok(Native::StrLfo(n as u8)))
            }
            Source::Envelope => {
                if self.me_on && STRINGS[ME_AMT].map(self.p[ME_AMT]).abs() > 1e-4 {
                    return Some(Err(format!(
                        "{}'s mod envelope is already in use",
                        self.engine()
                    )));
                }
                self.me_on = true;
                self.me_dest = dest;
                self.set(&[(ME_AMT, spec.amount.clamp(-1.0, 1.0))]);
                Some(Ok(Native::StrEnv))
            }
            _ => None,
        }
    }
    fn remove(&mut self, id: Native) {
        match id {
            Native::StrLfo(n) if (n as usize) < 2 => self.lfo_on[n as usize] = false,
            Native::StrEnv => self.me_on = false,
            _ => {}
        }
    }
    fn set_amount(&mut self, id: Native, a: f32) {
        use crate::strings::*;
        match id {
            Native::StrLfo(n) if (n as usize) < 2 => {
                self.set(&[(LFO[n as usize] + 1, a.abs().clamp(0.0, 1.0))]);
            }
            Native::StrEnv => {
                self.set(&[(ME_AMT, a.clamp(-1.0, 1.0))]);
            }
            _ => {}
        }
    }
}

// -------------------------------------------------------- ChipWave synth

/// the synth LFO's sync values (the LFO card's menu)
const CHIP_SYNC: [f32; 7] = [0.0, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0];

fn chip_shape(s: LfoShape) -> Option<crate::synth::Wave> {
    use crate::synth::Wave;
    Some(match s {
        LfoShape::Sine => Wave::Sine,
        LfoShape::Triangle => Wave::Triangle,
        LfoShape::Square => Wave::Square,
        LfoShape::SawDown => Wave::Saw,
        LfoShape::SawUp => return None,
    })
}

fn chip_wave_name(w: crate::synth::Wave) -> &'static str {
    use crate::synth::Wave;
    match w {
        Wave::Saw => "saw",
        Wave::Square => "square",
        Wave::Pulse => "pulse",
        Wave::Triangle => "triangle",
        Wave::Sine => "sine",
    }
}

fn chip_amount(l: &crate::synth::Lfo, d: ChipDst) -> f32 {
    match d {
        ChipDst::Cutoff => l.cutoff / 4.0,
        ChipDst::Amp => l.amp,
        ChipDst::Pitch => l.pitch / 12.0,
    }
}

impl EngineMod for crate::synth::Patch {
    fn engine(&self) -> &'static str {
        "ChipWave synth"
    }
    fn rows(&self, idx: usize) -> Vec<Row> {
        let l = &self.lfo;
        let src = format!(
            "Synth LFO {} {}",
            chip_wave_name(l.shape),
            rate_text(l.sync, l.rate)
        );
        let mut v = vec![];
        for (d, name, knobs, text) in [
            (
                ChipDst::Cutoff,
                "Cutoff",
                vec![Target::Cutoff(idx)],
                format!("{:+.1} oct", l.cutoff),
            ),
            (
                ChipDst::Amp,
                "Volume",
                vec![Target::Level(idx)],
                format!("{:.0} %", l.amp * 100.0),
            ),
            (
                ChipDst::Pitch,
                "Pitch",
                vec![],
                format!("{:.1} st", l.pitch),
            ),
        ] {
            let a = chip_amount(l, d);
            if a != 0.0 {
                v.push(Row {
                    id: RowId::Native(Native::Chip(d)),
                    source: src.clone(),
                    target: name.into(),
                    per_note: false,
                    amount: a,
                    text,
                    modk: None,
                    knobs,
                });
            }
        }
        v
    }
    fn add(&mut self, target: Target, spec: &Spec, _bpm: f32) -> Option<Result<Native, String>> {
        let d = match target {
            Target::Cutoff(_) => ChipDst::Cutoff,
            Target::Level(_) => ChipDst::Amp,
            _ => return None,
        };
        if spec.source != Source::Lfo {
            return None;
        }
        let shape = chip_shape(spec.shape)?;
        if !CHIP_SYNC.iter().any(|s| (s - spec.sync).abs() < 1e-4) {
            return None;
        }
        let l = &mut self.lfo;
        if chip_amount(l, d) != 0.0 {
            return Some(Err("the synth LFO already moves this knob".into()));
        }
        let same = l.shape == shape
            && (l.sync - spec.sync).abs() < 1e-4
            && (spec.sync > 0.0 || (l.rate - spec.rate).abs() < 1e-4);
        if l.active() && !same {
            return Some(Err(
                "the synth's one LFO is already in use at another shape or rate".into(),
            ));
        }
        l.shape = shape;
        l.sync = spec.sync;
        if spec.sync == 0.0 {
            l.rate = spec.rate.clamp(0.01, 30.0);
        }
        self.set_amount(Native::Chip(d), spec.amount);
        Some(Ok(Native::Chip(d)))
    }
    fn remove(&mut self, id: Native) {
        if let Native::Chip(d) = id {
            match d {
                ChipDst::Cutoff => self.lfo.cutoff = 0.0,
                ChipDst::Amp => self.lfo.amp = 0.0,
                ChipDst::Pitch => self.lfo.pitch = 0.0,
            }
        }
    }
    fn set_amount(&mut self, id: Native, a: f32) {
        if let Native::Chip(d) = id {
            let a = a.clamp(-1.0, 1.0);
            match d {
                ChipDst::Cutoff => self.lfo.cutoff = a * 4.0,
                ChipDst::Amp => self.lfo.amp = a.abs(),
                ChipDst::Pitch => self.lfo.pitch = a.abs() * 12.0,
            }
        }
    }
}

/// the adapter of an instrument with built-in routes
pub fn engine(kind: &InstrumentKind) -> Option<&dyn EngineMod> {
    match kind {
        InstrumentKind::Synth(p) => Some(p),
        InstrumentKind::Gen(g) => match &g.engine {
            GenEngine::Poly(x) => Some(x),
            GenEngine::Strings(x) => Some(x),
            _ => None,
        },
        _ => None,
    }
}

pub fn engine_mut(kind: &mut InstrumentKind) -> Option<&mut dyn EngineMod> {
    match kind {
        InstrumentKind::Synth(p) => Some(p),
        InstrumentKind::Gen(g) => match &mut g.engine {
            GenEngine::Poly(x) => Some(x),
            GenEngine::Strings(x) => Some(x),
            _ => None,
        },
        _ => None,
    }
}

// ------------------------------------------------------------ song-level

/// a song-level modulator's short description ("LFO sine 1/4")
pub fn song_source(m: &Modulation, k: usize) -> String {
    let md = &m.mods[k];
    let t = m.title(k);
    match md.kind {
        ModKind::Lfo => format!(
            "{t} {} {}",
            md.shape.name().to_lowercase(),
            rate_text(md.sync, md.rate)
        ),
        ModKind::Random => format!("{t} {}", rate_text(md.sync, md.rate)),
        _ => t,
    }
}

/// the song-level routes listed with instrument `idx`
pub fn song_rows(m: &Modulation, idx: usize) -> Vec<usize> {
    (0..m.routes.len())
        .filter(|&r| {
            let rt = &m.routes[r];
            m.find(rt.src).is_some_and(|k| m.mods[k].owner == idx)
                || crate::model::lane_instrument(&rt.target) == Some(idx)
        })
        .collect()
}

/// Every movement of instrument `idx`: the engine's own routes first, then
/// the song-level ones (`label` names a target id).
pub fn rows(
    inst: &Instrument,
    idx: usize,
    m: &Modulation,
    label: &dyn Fn(&str) -> String,
) -> Vec<Row> {
    let mut v = engine(&inst.kind).map(|e| e.rows(idx)).unwrap_or_default();
    for r in song_rows(m, idx) {
        let rt = &m.routes[r];
        let Some(k) = m.find(rt.src) else {
            continue;
        };
        let knobs = Target::parse(&rt.target).into_iter().collect();
        v.push(Row {
            id: RowId::Song(r),
            source: song_source(m, k),
            target: label(&rt.target),
            per_note: false,
            amount: rt.amount,
            text: pct(rt.amount),
            modk: Some(k),
            knobs,
        });
    }
    v
}

/// does this song-level modulator do what `spec` asks
fn song_fits(md: &crate::modulation::Modulator, spec: &Spec) -> bool {
    md.kind == spec.source.kind()
        && (!spec.source.has_rate()
            || ((md.sync - spec.sync).abs() < 1e-4
                && (spec.sync > 0.0 || (md.rate - spec.rate).abs() < 1e-4)
                && (spec.source != Source::Lfo || md.shape == spec.shape)))
}

/// set a song-level modulator to `spec`
pub fn song_apply(md: &mut crate::modulation::Modulator, spec: &Spec) {
    if spec.source.has_rate() {
        md.shape = spec.shape;
        md.sync = spec.sync;
        md.rate = spec.rate;
    }
}

/// Add a song-level route: a modulator of `idx` that already does this
/// (and is not on `target` yet) is shared, else a new one is made.
pub fn add_song(
    m: &mut Modulation,
    idx: usize,
    target: &str,
    spec: &Spec,
) -> Result<usize, String> {
    let k = match (0..m.mods.len()).find(|&k| {
        m.mods[k].owner == idx
            && song_fits(&m.mods[k], spec)
            && !m.routes_to(target).any(|(_, j)| j == k)
    }) {
        Some(k) => k,
        None => {
            let Some(k) = m.add(idx, spec.source.kind()) else {
                return Err(format!(
                    "slots full: a song holds {} modulators",
                    crate::modulation::MAX_MODS
                ));
            };
            song_apply(&mut m.mods[k], spec);
            k
        }
    };
    let id = m.mods[k].id;
    match m.connect(id, target, spec.amount) {
        Some(r) => {
            m.routes[r].amount = spec.amount.clamp(-1.0, 1.0);
            Ok(r)
        }
        None => {
            if !m.routes.iter().any(|r| r.src == id) {
                m.remove(k);
            }
            Err(format!(
                "slots full: a song holds {} modulation routes",
                crate::modulation::MAX_ROUTES
            ))
        }
    }
}

/// Add a movement onto `target` of instrument `idx`: the engine's own slot
/// when it has one for this, else a song-level modulator. Err = nowhere left.
pub fn add(
    inst: &mut Instrument,
    idx: usize,
    m: &mut Modulation,
    target: Target,
    spec: &Spec,
    bpm: f32,
) -> Result<Added, String> {
    let mut note = None;
    if let Some(e) = engine_mut(&mut inst.kind) {
        match e.add(target, spec, bpm) {
            Some(Ok(n)) => {
                return Ok(Added {
                    row: RowId::Native(n),
                    note: None,
                })
            }
            Some(Err(why)) => note = Some(why),
            None => {}
        }
    }
    if !crate::automation::modulatable(target) {
        return Err(note.unwrap_or_else(|| "this knob can't be modulated".into()));
    }
    match add_song(m, idx, &target.id(), spec) {
        Ok(r) => Ok(Added {
            row: RowId::Song(r),
            note: note.map(|n| {
                format!(
                    "{n}: added as an instrument-wide {} instead",
                    spec.source.name()
                )
            }),
        }),
        Err(e) => Err(match note {
            Some(n) => format!("{n}, and {e}"),
            None => e,
        }),
    }
}

/// remove a built-in row (song-level rows go through `Song::remove_route`,
/// which fixes the depth lanes)
pub fn remove_native(inst: &mut Instrument, n: Native) {
    if let Some(e) = engine_mut(&mut inst.kind) {
        e.remove(n);
    }
}

/// set a row's amount (-1..1)
pub fn set_amount(inst: &mut Instrument, m: &mut Modulation, row: RowId, a: f32) {
    match row {
        RowId::Native(n) => {
            if let Some(e) = engine_mut(&mut inst.kind) {
                e.set_amount(n, a);
            }
        }
        RowId::Song(r) => {
            if let Some(rt) = m.routes.get_mut(r) {
                rt.amount = a.clamp(-1.0, 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests;
