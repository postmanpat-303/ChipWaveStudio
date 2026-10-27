//! One list of every knob in a song.
//!
//! A *knob* is a continuous value that automation lanes, MIDI learn,
//! controller maps and modulation routes can drive. Each has a stable text
//! id (`instrument.1.cutoff`, saved in songs and settings) and a
//! [`Target`] (the same knob as a small enum). The knobs themselves are
//! still described where they live — `modulation::PATCH_PARAMS`,
//! `fx::params`, `gen::gen_knobs`, the mixer — this module only reads those
//! tables, so a knob added there shows up here (and in every feature that
//! asks) without another list to edit.
//!
//! See docs/ARCHITECTURE.md ("A new knob").
use crate::automation::Target;
use crate::fx::FxKind;
use crate::model::{InstrumentKind, Song};

/// whose knob it is
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// an instrument slot (0-based)
    Instrument(usize),
    /// a mixer channel (0-based)
    Channel(usize),
    /// a return bus (0 = A, 1 = B)
    Return(usize),
    /// master volume, tempo, modulation depths
    Song,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Knob {
    pub target: Target,
    pub owner: Owner,
    /// the device it sits on, when there is one ("Delay")
    pub device: Option<String>,
    /// the parameter's own name ("Filter cutoff", "Time", "Decay")
    pub name: String,
    /// what the knob's 0..1 maps to (`Target::to_real`): real units for the
    /// filter, patch, mixer, tempo and depth knobs; 0..1 for FX, generator,
    /// send and macro knobs, which are stored normalised (their own units
    /// are in `fx::ParamInfo` / `gen::MacroDef`)
    pub lo: f32,
    pub hi: f32,
    pub log: bool,
    /// a stepped choice (a dropdown in the panel), still automatable
    pub choice: bool,
    /// the value lives in a plugin (its range and name come from the plugin)
    pub plugin: bool,
}

fn ret_kind(r: usize) -> FxKind {
    if r == 0 {
        FxKind::Delay
    } else {
        FxKind::Reverb
    }
}

/// does this instrument have a sound patch, and so the filter knobs (cutoff,
/// resonance, drive) and every `PATCH_PARAMS` row: Synth, Voice and Gen do
/// (the engine applies those lanes to any `InstrumentKind::patch()`)
fn has_filter(k: &InstrumentKind) -> bool {
    k.patch().is_some()
}

fn has_patch(k: &InstrumentKind, _key: &str) -> bool {
    k.patch().is_some()
}

/// The knob `t` in this song, or None if the song has no such knob (an
/// instrument, device, channel or route that isn't there).
pub fn get(song: &Song, t: Target) -> Option<Knob> {
    let (lo, hi, log) = t.bounds();
    let mut k = Knob {
        target: t,
        owner: Owner::Song,
        device: None,
        name: t.param_name().to_string(),
        lo,
        hi,
        log,
        choice: false,
        plugin: false,
    };
    let inst = |i: usize| song.instruments.get(i);
    match t {
        Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) => {
            if !has_filter(&inst(i)?.kind) {
                return None;
            }
            k.owner = Owner::Instrument(i);
        }
        Target::Level(i) => {
            inst(i)?;
            k.owner = Owner::Instrument(i);
        }
        Target::Patch(i, p) => {
            let pp = crate::modulation::PATCH_PARAMS.get(p)?;
            if !has_patch(&inst(i)?.kind, pp.key) {
                return None;
            }
            k.owner = Owner::Instrument(i);
        }
        Target::Gen(i, g) => {
            let InstrumentKind::Gen(gp) = &inst(i)?.kind else {
                return None;
            };
            k.name = crate::gen::gen_knobs(gp).into_iter().find(|x| x.0 == g)?.1;
            k.owner = Owner::Instrument(i);
        }
        Target::Macro(i, m) => {
            if !matches!(inst(i)?.kind, InstrumentKind::Plugin(_)) || m >= 8 {
                return None;
            }
            k.owner = Owner::Instrument(i);
            k.name = format!("Macro {}", m + 1);
            k.plugin = true;
        }
        Target::Fx(i, d, p) => {
            let dev = inst(i)?.fx.get(d)?;
            k.owner = Owner::Instrument(i);
            k.device = Some(dev.title());
            if dev.plugin.is_some() || dev.kind == FxKind::Plugin {
                k.plugin = true;
                k.name = format!("Param {p}");
            } else {
                let info = crate::fx::params(dev.kind).get(p)?;
                k.name = info.name.to_string();
                k.choice = !info.choices.is_empty();
            }
        }
        Target::Arp(i, p) => {
            inst(i)?.arp.as_ref()?;
            k.owner = Owner::Instrument(i);
            k.device = Some("Arp".into());
            k.name = crate::arp::Param::ALL.get(p)?.name().to_string();
        }
        Target::Ret(r, p) => {
            song.mixer.returns.get(r)?;
            let info = crate::fx::params(ret_kind(r)).get(p)?;
            k.owner = Owner::Return(r);
            k.device = Some(ret_kind(r).name().to_string());
            k.name = info.name.to_string();
            k.choice = !info.choices.is_empty();
        }
        Target::ChanVolume(c) | Target::ChanPan(c) => {
            song.mixer.channels.get(c)?;
            k.owner = Owner::Channel(c);
        }
        Target::Send(c, r) => {
            song.mixer.channels.get(c)?.send.get(r)?;
            k.owner = Owner::Channel(c);
            k.name = if r == 0 { "Send A" } else { "Send B" }.into();
        }
        Target::ModAmount(r) => {
            song.modulation.routes.get(r)?;
        }
        Target::Master | Target::Tempo => {}
    }
    Some(k)
}

/// Every knob target in the song, in menu order: per instrument (level,
/// filter, patch, generator knobs, plugin macros, FX parameters), then
/// the mixer channels, the returns, master, tempo and modulation depths.
/// Parameters inside plugin effects are not listed (the plugin knows them).
pub fn targets(song: &Song) -> Vec<Target> {
    let mut v = Vec::new();
    for (i, inst) in song.instruments.iter().enumerate() {
        v.push(Target::Level(i));
        if has_filter(&inst.kind) {
            v.extend([Target::Cutoff(i), Target::Resonance(i), Target::Drive(i)]);
        }
        for (p, pp) in crate::modulation::PATCH_PARAMS.iter().enumerate() {
            if has_patch(&inst.kind, pp.key) {
                v.push(Target::Patch(i, p));
            }
        }
        if let InstrumentKind::Gen(g) = &inst.kind {
            v.extend(
                crate::gen::gen_knobs(g)
                    .into_iter()
                    .map(|(k, _)| Target::Gen(i, k)),
            );
        }
        if matches!(inst.kind, InstrumentKind::Plugin(_)) {
            v.extend((0..8).map(|m| Target::Macro(i, m)));
        }
        for (d, dev) in inst.fx.iter().enumerate() {
            if dev.plugin.is_some() || dev.kind == FxKind::Plugin {
                continue;
            }
            v.extend((0..crate::fx::params(dev.kind).len()).map(|p| Target::Fx(i, d, p)));
        }
    }
    for c in 0..song.mixer.channels.len() {
        v.extend([
            Target::ChanVolume(c),
            Target::ChanPan(c),
            Target::Send(c, 0),
            Target::Send(c, 1),
        ]);
    }
    for r in 0..song.mixer.returns.len().min(2) {
        v.extend((0..crate::fx::params(ret_kind(r)).len()).map(|p| Target::Ret(r, p)));
    }
    v.extend([Target::Master, Target::Tempo]);
    v.extend((0..song.modulation.routes.len()).map(Target::ModAmount));
    v
}

/// [`targets`] with names and ranges
pub fn all(song: &Song) -> Vec<Knob> {
    targets(song)
        .into_iter()
        .filter_map(|t| get(song, t))
        .collect()
}

/// A knob id (as saved in songs, settings and controller maps) to the knob
/// it names in this song; also accepts named FX parameters
/// (`instrument.1.fx.2.time`). None if the id is unknown or the song has
/// no such knob. Plugin effect parameters (`instrument.N.fx.D.pK` on a
/// plugin) are accepted as long as the device is there.
pub fn resolve(song: &Song, id: &str) -> Option<Target> {
    let t = Target::parse_in(id, song)?;
    get(song, t).map(|_| t)
}

/// the parameter name of a knob ("Time", "Decay", "Filter cutoff"), None if
/// the song has no such knob
pub fn name(song: &Song, t: Target) -> Option<String> {
    get(song, t).map(|k| k.name)
}

/// Set a knob from a 0..1 value, as a dragged knob, a learned MIDI CC or a
/// controller map would. Returns false if the song has no such knob or the
/// value lives in a plugin (plugin macros: `plugin::set_macro_norm`;
/// plugin FX parameters are only written into the device's saved list).
/// Tempo is rounded to whole BPM; the caller tells a playing engine.
pub fn set_norm(song: &mut Song, t: Target, n: f32) -> bool {
    let n = n.clamp(0.0, 1.0);
    let real = t.to_real(n);
    match t {
        Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) => {
            let Some(x) = song.instruments.get_mut(i) else {
                return false;
            };
            if !has_filter(&x.kind) {
                return false;
            }
            let Some(p) = x.kind.patch_mut() else {
                return false;
            };
            match t {
                Target::Cutoff(_) => p.cutoff = real,
                Target::Resonance(_) => p.resonance = real,
                _ => p.drive = real,
            }
        }
        // a learned CC sets the instrument volume (0..1)
        Target::Level(i) => match song.instruments.get_mut(i) {
            Some(x) => x.volume = n,
            None => return false,
        },
        Target::ChanVolume(c) => match song.mixer.channels.get_mut(c) {
            Some(m) => m.volume = real,
            None => return false,
        },
        Target::ChanPan(c) => match song.mixer.channels.get_mut(c) {
            Some(m) => m.pan = real,
            None => return false,
        },
        Target::Send(c, b) => match song
            .mixer
            .channels
            .get_mut(c)
            .and_then(|m| m.send.get_mut(b))
        {
            Some(v) => *v = n,
            None => return false,
        },
        Target::Macro(..) => return false,
        Target::Gen(i, k) => match song.instruments.get_mut(i).map(|x| &mut x.kind) {
            Some(InstrumentKind::Gen(g)) => return crate::gen::set_gen_knob_norm(g, k, n),
            _ => return false,
        },
        Target::Master => song.mixer.master = real,
        Target::Tempo => song.bpm = real.round(),
        Target::Fx(i, d, p) => {
            match song
                .instruments
                .get_mut(i)
                .and_then(|x| x.fx.get_mut(d))
                .and_then(|dv| dv.params.get_mut(p))
            {
                Some(v) => *v = n,
                None => return false,
            }
        }
        Target::Ret(b, p) => {
            match song
                .mixer
                .returns
                .get_mut(b)
                .and_then(|r| r.device.params.get_mut(p))
            {
                Some(v) => *v = n,
                None => return false,
            }
        }
        Target::Patch(i, k) => {
            let (Some(x), Some(pp)) = (
                song.instruments.get_mut(i),
                crate::modulation::PATCH_PARAMS.get(k),
            ) else {
                return false;
            };
            if !has_patch(&x.kind, pp.key) {
                return false;
            }
            let Some(p) = x.kind.patch_mut() else {
                return false;
            };
            (pp.set)(p, real);
        }
        Target::ModAmount(k) => match song.modulation.routes.get_mut(k) {
            Some(r) => r.amount = real,
            None => return false,
        },
        Target::Arp(i, k) => match (
            song.instruments.get_mut(i).and_then(|x| x.arp.as_mut()),
            crate::arp::Param::ALL.get(k),
        ) {
            (Some(a), Some(p)) => p.set(a, n),
            _ => return false,
        },
    }
    true
}

#[cfg(test)]
mod tests;
