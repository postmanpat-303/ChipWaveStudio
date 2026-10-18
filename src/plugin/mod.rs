//! Plugin instruments (0.2.3.16): CLAP hosting (src/plugin/clap.rs), the
//! out-of-process scanner and cache (scan.rs), editor windows (gui.rs) and
//! the engine-side rack that runs the instances.
//!
//! One plugin instance runs per (instrument, channel) pair that the song
//! uses, so each instance's stereo output goes through that channel's insert
//! FX chain, fader, sends and mixer exactly like a built-in instrument.
//! Instances are created on the UI thread (or the export thread) and inserted
//! into the engine under the audio lock; the audio thread only feeds events
//! and pulls audio (no allocation, no locks).
pub mod clap;
pub mod gui;
pub mod scan;

use crate::model::{InstrumentKind, Song, MAX_CHANNELS};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// largest block a plugin is asked to render (chunks never exceed it)
pub const MAX_BLOCK: usize = 1024;
/// instruments x channels index size
const MAX_INSTR: usize = 256;
/// plugin instruments sounding on one channel at once
const PER_CH: usize = 8;
pub const NO_PARAM: u32 = u32::MAX;

/// A plugin instrument in the song (`InstrumentKind::Plugin`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginPatch {
    /// "clap"
    pub format: String,
    /// plugin id (CLAP: reverse-DNS id)
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    /// where it was found last; on another machine the scan cache finds it by id
    #[serde(default)]
    pub path: String,
    /// plugin state (CLAP state extension), base64
    #[serde(default)]
    pub state: String,
    /// parameter values for plugins without a state extension (id, value)
    #[serde(default)]
    pub params: Vec<(u32, f64)>,
    /// parameters on macros 1-8 (43xy, automation "instr.N.macro.K"); NO_PARAM = none
    #[serde(default)]
    pub macros: Vec<u32>,
}

impl PluginPatch {
    pub fn macro_id(&self, k: usize) -> u32 {
        self.macros.get(k).copied().unwrap_or(NO_PARAM)
    }
    fn key(&self) -> String {
        format!("{}|{}|{}", self.format, self.id, self.path)
    }
    pub fn state_bytes(&self) -> Option<Vec<u8>> {
        if self.state.is_empty() {
            return None;
        }
        base64::engine::general_purpose::STANDARD
            .decode(self.state.as_bytes())
            .ok()
    }
    pub fn set_state_bytes(&mut self, b: &[u8]) {
        self.state = base64::engine::general_purpose::STANDARD.encode(b);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamInfo {
    pub id: u32,
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub stepped: bool,
    pub readonly: bool,
}

/// A running plugin instance (CLAP today). Audio-thread methods: note_on,
/// note_off, tuning, param, process. Everything else: UI / main thread.
pub trait Hosted: Send {
    fn note_on(&mut self, key: u8, vel: f32);
    fn note_off(&mut self, key: u8);
    /// per-note pitch offset in semitones (glide, slides)
    fn tuning(&mut self, key: u8, semis: f32);
    fn param(&mut self, id: u32, value: f64);
    /// panic: silence every voice now (all notes / sound off, plugin reset)
    fn panic(&mut self);
    /// render `n` frames (<= MAX_BLOCK) with the queued events at frame 0
    fn process(&mut self, n: usize, l: &mut [f32], r: &mut [f32]);
    fn params(&self) -> Vec<ParamInfo>;
    fn value(&self, id: u32) -> Option<f64>;
    fn value_text(&self, id: u32, value: f64) -> Option<String>;
    fn save_state(&mut self) -> Option<Vec<u8>>;
    fn load_state(&mut self, data: &[u8]) -> bool;
    /// parameter change from the UI (applied even while not processing)
    fn set_param_main(&mut self, id: u32, value: f64);
    fn has_editor(&self) -> bool;
    fn open_editor(&mut self, title: &str) -> Result<(), String>;
    fn close_editor(&mut self);
    fn editor_open(&self) -> bool;
    /// main-thread housekeeping: callbacks, restart, editor resize / close
    fn poll_main(&mut self);
    /// the plugin changed its own state (editor edits) since the last call
    fn take_dirty(&self) -> bool;
    fn set_sample_rate(&mut self, sr: f32);
}

/// Create an instance for `p` at `sr`, with its saved state restored.
pub fn instantiate(p: &PluginPatch, sr: f32) -> Result<Box<dyn Hosted>, String> {
    let path = scan::resolve(p).ok_or_else(|| {
        format!(
            "plugin \"{}\" ({}) is not installed on this computer",
            p.name, p.id
        )
    })?;
    let mut inst: Box<dyn Hosted> = match p.format.as_str() {
        "clap" => Box::new(clap::ClapInst::new(&path, &p.id, sr)?),
        f => return Err(format!("plugin format {f} is not supported yet")),
    };
    if let Some(st) = p.state_bytes() {
        if !inst.load_state(&st) {
            eprintln!("plugin {}: saved state was not accepted", p.name);
        }
    }
    for &(id, v) in &p.params {
        inst.set_param_main(id, v);
    }
    Ok(inst)
}

struct Macro {
    id: u32,
    min: f64,
    max: f64,
}

pub struct Slot {
    pub instr: usize,
    pub ch: usize,
    pub inst: Box<dyn Hosted>,
    key: String,
    l: Box<[f32]>,
    r: Box<[f32]>,
    pos: usize,
    len: usize,
    /// the key that is held down (tracker notes are mono per channel)
    note: Option<u8>,
    vel: f32,
    tune: f32,
    macros: [Macro; 8],
    sent: [f32; 8],
}

impl Slot {
    fn new(instr: usize, ch: usize, p: &PluginPatch, inst: Box<dyn Hosted>) -> Slot {
        let infos = inst.params();
        let m = |k: usize| {
            let id = p.macro_id(k);
            infos
                .iter()
                .find(|i| i.id == id)
                .map(|i| Macro {
                    id,
                    min: i.min,
                    max: i.max,
                })
                .unwrap_or(Macro {
                    id: NO_PARAM,
                    min: 0.0,
                    max: 1.0,
                })
        };
        Slot {
            instr,
            ch,
            inst,
            key: p.key(),
            l: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            r: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            pos: 0,
            len: 0,
            note: None,
            vel: 1.0,
            tune: 0.0,
            macros: std::array::from_fn(m),
            sent: [-1.0; 8],
        }
    }
}

/// Engine-side set of plugin instances.
pub struct Rack {
    pub slots: Vec<Slot>,
    /// [instr * MAX_CHANNELS + ch] -> slot index + 1 (0 = none)
    idx: Vec<u16>,
    by_ch: [[u16; PER_CH]; MAX_CHANNELS],
    by_ch_n: [u8; MAX_CHANNELS],
    /// instruments that failed to load: (instrument, patch key, message)
    pub errors: Vec<(usize, String, String)>,
    /// live MIDI keys routed to plugins: (key, slot, sent note)
    live: [(u8, u16, u8); 16],
}

impl Default for Rack {
    fn default() -> Self {
        Self::new()
    }
}

impl Rack {
    pub fn new() -> Rack {
        Rack {
            slots: Vec::with_capacity(64),
            idx: vec![0; MAX_INSTR * MAX_CHANNELS],
            by_ch: [[0; PER_CH]; MAX_CHANNELS],
            by_ch_n: [0; MAX_CHANNELS],
            errors: vec![],
            live: [(0, u16::MAX, 0); 16],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    fn rebuild(&mut self) {
        self.idx.iter_mut().for_each(|x| *x = 0);
        self.by_ch_n = [0; MAX_CHANNELS];
        for (i, s) in self.slots.iter().enumerate() {
            if s.instr < MAX_INSTR && s.ch < MAX_CHANNELS {
                self.idx[s.instr * MAX_CHANNELS + s.ch] = i as u16 + 1;
                let n = self.by_ch_n[s.ch] as usize;
                if n < PER_CH {
                    self.by_ch[s.ch][n] = i as u16;
                    self.by_ch_n[s.ch] += 1;
                }
            }
        }
        self.live = [(0, u16::MAX, 0); 16];
    }

    pub fn slot(&self, instr: usize, ch: usize) -> Option<usize> {
        if instr >= MAX_INSTR || ch >= MAX_CHANNELS {
            return None;
        }
        match self.idx[instr * MAX_CHANNELS + ch] {
            0 => None,
            n => Some(n as usize - 1),
        }
    }

    /// the first instance of an instrument (state, editor, parameter list)
    pub fn primary(&self, instr: usize) -> Option<usize> {
        self.slots.iter().position(|s| s.instr == instr)
    }

    pub fn error(&self, instr: usize) -> Option<&str> {
        self.errors
            .iter()
            .find(|e| e.0 == instr)
            .map(|e| e.2.as_str())
    }

    // ---------------------------------------------------------- audio thread

    /// panic: every instance stops all sound now (audio thread)
    pub fn panic(&mut self) {
        for s in &mut self.slots {
            s.note = None;
            s.inst.panic();
            s.pos = s.len;
        }
        for l in self.live.iter_mut() {
            l.1 = u16::MAX;
        }
    }

    /// a tracker note on channel `ch`: releases other plugin notes on the
    /// channel, then plays `key` on (instr, ch) if it has an instance
    pub fn note(&mut self, instr: usize, ch: usize, key: u8, vel: f32) {
        self.release_ch(ch);
        if let Some(i) = self.slot(instr, ch) {
            let s = &mut self.slots[i];
            s.inst.note_on(key, vel);
            s.note = Some(key);
            s.vel = vel.max(1e-3);
            s.tune = 0.0;
        }
    }

    /// note off for whatever plugin note channel `ch` is holding
    pub fn release_ch(&mut self, ch: usize) {
        if ch >= MAX_CHANNELS {
            return;
        }
        for k in 0..self.by_ch_n[ch] as usize {
            let s = &mut self.slots[self.by_ch[ch][k] as usize];
            if let Some(n) = s.note.take() {
                s.inst.note_off(n);
            }
        }
    }

    pub fn all_off(&mut self) {
        for s in &mut self.slots {
            if let Some(n) = s.note.take() {
                s.inst.note_off(n);
            }
        }
        for l in self.live.iter_mut() {
            if l.1 != u16::MAX {
                if let Some(s) = self.slots.get_mut(l.1 as usize) {
                    s.inst.note_off(l.2);
                }
                l.1 = u16::MAX;
            }
        }
    }

    pub fn live_on(&mut self, instr: usize, ch: usize, key: u8, note: u8, vel: f32) -> bool {
        let Some(i) = self.slot(instr, ch) else {
            return false;
        };
        self.slots[i].inst.note_on(note, vel);
        if let Some(f) = self.live.iter_mut().find(|l| l.1 == u16::MAX) {
            *f = (key, i as u16, note);
        }
        true
    }

    pub fn live_off(&mut self, key: u8) {
        for l in self
            .live
            .iter_mut()
            .filter(|l| l.1 != u16::MAX && l.0 == key)
        {
            if let Some(s) = self.slots.get_mut(l.1 as usize) {
                s.inst.note_off(l.2);
            }
            l.1 = u16::MAX;
        }
    }

    pub fn ch_slots(&self, ch: usize) -> &[u16] {
        if ch >= MAX_CHANNELS {
            return &[];
        }
        &self.by_ch[ch][..self.by_ch_n[ch] as usize]
    }

    /// the held note of a slot and its velocity
    pub fn held(&self, si: usize) -> Option<(u8, f32)> {
        let s = &self.slots[si];
        s.note.map(|n| (n, s.vel))
    }

    /// the slot's buffer is used up: the next sample renders a new chunk
    pub fn needs_chunk(&self, si: usize) -> bool {
        let s = &self.slots[si];
        s.pos >= s.len
    }

    /// pitch offset of the held note (sent when it changes)
    pub fn set_tuning(&mut self, si: usize, semis: f32) {
        let s = &mut self.slots[si];
        if let Some(n) = s.note {
            if (semis - s.tune).abs() > 0.002 {
                s.tune = semis;
                s.inst.tuning(n, semis);
            }
        }
    }

    /// macro k (0..1, < 0 = leave the parameter alone)
    pub fn set_macro(&mut self, si: usize, k: usize, v: f32) {
        let s = &mut self.slots[si];
        let m = &s.macros[k];
        if v < 0.0 || m.id == NO_PARAM || (v - s.sent[k]).abs() < 1e-4 {
            return;
        }
        s.sent[k] = v;
        let val = m.min + (m.max - m.min) * v as f64;
        let id = m.id;
        s.inst.param(id, val);
    }

    /// render the next chunk of `n` frames
    pub fn chunk(&mut self, si: usize, n: usize) {
        let s = &mut self.slots[si];
        let n = n.clamp(1, MAX_BLOCK);
        s.inst.process(n, &mut s.l, &mut s.r);
        s.pos = 0;
        s.len = n;
    }

    /// next output sample of a slot (call `chunk` first when `needs_chunk`)
    pub fn sample(&mut self, si: usize) -> (f32, f32) {
        let s = &mut self.slots[si];
        if s.pos >= s.len {
            return (0.0, 0.0);
        }
        let v = (s.l[s.pos], s.r[s.pos]);
        s.pos += 1;
        v
    }

    // ---------------------------------------------------------- main thread

    /// (instrument, channel) pairs the song needs instances for, plus `extra`
    /// (the cursor channel's instrument, for jamming)
    pub fn needed(song: &Song, extra: Option<(usize, usize)>) -> Vec<(usize, usize)> {
        let is_plug = |i: usize| {
            matches!(
                song.instruments.get(i).map(|x| &x.kind),
                Some(InstrumentKind::Plugin(_))
            )
        };
        if !(0..song.instruments.len()).any(is_plug) {
            return vec![];
        }
        let nch = song.channels.min(MAX_CHANNELS);
        let mut used = vec![false; MAX_INSTR * MAX_CHANNELS];
        for p in &song.patterns {
            for row in &p.cells {
                for (c, cell) in row.iter().enumerate().take(nch) {
                    if cell.instr > 0 && is_plug(cell.instr as usize - 1) {
                        used[(cell.instr as usize - 1).min(MAX_INSTR - 1) * MAX_CHANNELS + c] =
                            true;
                    }
                }
            }
        }
        if let Some((i, c)) = extra {
            if is_plug(i) && c < nch && i < MAX_INSTR {
                used[i * MAX_CHANNELS + c] = true;
            }
        }
        let mut v = vec![];
        for i in 0..song.instruments.len().min(MAX_INSTR) {
            for c in 0..nch {
                if used[i * MAX_CHANNELS + c] {
                    v.push((i, c));
                }
            }
        }
        v
    }

    /// What to create and what to drop to match `song`.
    pub fn plan(&self, song: &Song, extra: Option<(usize, usize)>) -> Plan {
        let need = Rack::needed(song, extra);
        let patch = |i: usize| match song.instruments.get(i).map(|x| &x.kind) {
            Some(InstrumentKind::Plugin(p)) => Some(p),
            _ => None,
        };
        let mut add = vec![];
        for &(i, c) in &need {
            let Some(p) = patch(i) else { continue };
            let have = self
                .slot(i, c)
                .is_some_and(|s| self.slots[s].key == p.key());
            let failed = self.errors.iter().any(|e| e.0 == i && e.1 == p.key());
            if !have && !failed {
                add.push((i, c, p.clone()));
            }
        }
        let drop = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                !need.contains(&(s.instr, s.ch)) || patch(s.instr).is_none_or(|p| p.key() != s.key)
            })
            .map(|(k, _)| k)
            .collect();
        Plan { add, drop }
    }

    /// Take out the slots in `drop` (drop the instances outside the lock).
    pub fn take(&mut self, drop: &[usize]) -> Vec<Slot> {
        let mut out = vec![];
        let mut k = 0;
        while k < self.slots.len() {
            if drop.contains(&k) {
                // keep indices of later entries stable while scanning
                out.push(k);
            }
            k += 1;
        }
        let mut taken = vec![];
        for &k in out.iter().rev() {
            let mut s = self.slots.remove(k);
            if let Some(n) = s.note.take() {
                s.inst.note_off(n);
            }
            taken.push(s);
        }
        self.rebuild();
        taken
    }

    pub fn insert(&mut self, instr: usize, ch: usize, p: &PluginPatch, inst: Box<dyn Hosted>) {
        if let Some(old) = self.slot(instr, ch) {
            self.slots.remove(old);
        }
        // a sibling's current state wins over the song's saved one
        self.slots.push(Slot::new(instr, ch, p, inst));
        self.errors.retain(|e| e.0 != instr);
        self.rebuild();
    }

    pub fn fail(&mut self, instr: usize, p: &PluginPatch, msg: String) {
        self.errors.retain(|e| e.0 != instr);
        self.errors.push((instr, p.key(), msg));
    }

    /// forget load errors (Reload button) so the next sync tries again
    pub fn retry(&mut self, instr: usize) {
        self.errors.retain(|e| e.0 != instr);
    }

    /// instruments were renumbered (see Engine::remap_instruments)
    pub fn remap(&mut self, f: &dyn Fn(usize) -> Option<usize>) -> Vec<Slot> {
        let mut gone = vec![];
        let mut k = 0;
        while k < self.slots.len() {
            match f(self.slots[k].instr) {
                Some(n) => {
                    self.slots[k].instr = n;
                    k += 1;
                }
                None => gone.push(self.slots.remove(k)),
            }
        }
        self.errors = std::mem::take(&mut self.errors)
            .into_iter()
            .filter_map(|(i, key, m)| f(i).map(|n| (n, key, m)))
            .collect();
        self.rebuild();
        gone
    }

    /// macro parameter ranges changed (macro assignment edited)
    pub fn refresh_macros(&mut self, song: &Song) {
        for s in &mut self.slots {
            if let Some(InstrumentKind::Plugin(p)) = song.instruments.get(s.instr).map(|i| &i.kind)
            {
                let infos = s.inst.params();
                for k in 0..8 {
                    let id = p.macro_id(k);
                    s.macros[k] = infos
                        .iter()
                        .find(|i| i.id == id)
                        .map(|i| Macro {
                            id,
                            min: i.min,
                            max: i.max,
                        })
                        .unwrap_or(Macro {
                            id: NO_PARAM,
                            min: 0.0,
                            max: 1.0,
                        });
                    s.sent[k] = -1.0;
                }
            }
        }
    }

    /// UI parameter edit: every instance of the instrument
    pub fn set_param(&mut self, instr: usize, id: u32, v: f64) {
        for s in self.slots.iter_mut().filter(|s| s.instr == instr) {
            s.inst.set_param_main(id, v);
        }
    }

    /// main-thread housekeeping for every instance; returns instruments whose
    /// plugin changed its own state (to be saved into the song)
    pub fn poll_main(&mut self) -> Vec<usize> {
        let mut dirty = vec![];
        for s in &mut self.slots {
            s.inst.poll_main();
            if s.inst.take_dirty() && !dirty.contains(&s.instr) {
                dirty.push(s.instr);
            }
        }
        dirty
    }

    /// Save the primary instance's state of `instr` into the song and copy
    /// it to the other instances (channels) of the same instrument.
    pub fn capture(&mut self, song: &mut Song, instr: usize) -> bool {
        let Some(pi) = self.primary(instr) else {
            return false;
        };
        let Some(InstrumentKind::Plugin(p)) = song.instruments.get_mut(instr).map(|i| &mut i.kind)
        else {
            return false;
        };
        let st = self.slots[pi].inst.save_state();
        match &st {
            Some(b) => {
                p.set_state_bytes(b);
                p.params.clear();
            }
            None => {
                // no state extension: remember every parameter value
                let inst = &self.slots[pi].inst;
                p.params = inst
                    .params()
                    .iter()
                    .filter(|i| !i.readonly)
                    .filter_map(|i| inst.value(i.id).map(|v| (i.id, v)))
                    .collect();
            }
        }
        let key = p.key();
        let params = p.params.clone();
        for (k, s) in self.slots.iter_mut().enumerate() {
            if k == pi || s.instr != instr {
                continue;
            }
            s.key = key.clone();
            match &st {
                Some(b) => {
                    s.inst.load_state(b);
                }
                None => {
                    for &(id, v) in &params {
                        s.inst.set_param_main(id, v);
                    }
                }
            }
        }
        true
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        for s in &mut self.slots {
            s.inst.set_sample_rate(sr);
        }
    }
}

pub struct Plan {
    pub add: Vec<(usize, usize, PluginPatch)>,
    pub drop: Vec<usize>,
}

/// Bring the rack of `shared`'s engine in line with its song: instances are
/// created without holding the audio lock, then inserted under it.
pub fn sync(shared: &crate::audio::SharedRef, extra: Option<(usize, usize)>) {
    let (plan, sr) = {
        let s = crate::audio::lock(shared);
        (s.engine.plugins.plan(&s.song, extra), s.engine.sr)
    };
    if plan.add.is_empty() && plan.drop.is_empty() {
        return;
    }
    let old = {
        let mut s = crate::audio::lock(shared);
        s.engine.plugins.take(&plan.drop)
    };
    drop(old);
    // a second channel of an instrument starts from the first one's state
    for (i, c, mut p) in plan.add {
        {
            let mut s = crate::audio::lock(shared);
            let crate::audio::Shared { song, engine, .. } = &mut *s;
            if engine.plugins.primary(i).is_some() {
                engine.plugins.capture(song, i);
                if let Some(InstrumentKind::Plugin(q)) = song.instruments.get(i).map(|x| &x.kind) {
                    p = q.clone();
                }
            }
        }
        let r = instantiate(&p, sr);
        let mut s = crate::audio::lock(shared);
        // the song may have changed while loading
        let still = matches!(
            s.song.instruments.get(i).map(|x| &x.kind),
            Some(InstrumentKind::Plugin(q)) if q.key() == p.key()
        );
        if !still {
            continue;
        }
        match r {
            Ok(inst) => s.engine.plugins.insert(i, c, &p, inst),
            Err(e) => s.engine.plugins.fail(i, &p, e),
        }
    }
}

/// Offline: create every instance the song needs on this thread (export,
/// --render, tests).
pub fn prepare(rack: &mut Rack, song: &Song, sr: f32) {
    let plan = rack.plan(song, None);
    drop(rack.take(&plan.drop));
    for (i, c, p) in plan.add {
        match instantiate(&p, sr) {
            Ok(inst) => rack.insert(i, c, &p, inst),
            Err(e) => rack.fail(i, &p, e),
        }
    }
}

/// Warnings for plugin instruments that can't be found here (song load).
pub fn missing_warnings(song: &Song) -> Vec<String> {
    song.instruments
        .iter()
        .enumerate()
        .filter_map(|(i, inst)| match &inst.kind {
            InstrumentKind::Plugin(p) if scan::resolve(p).is_none() => Some(format!(
                "Instrument {:02} \"{}\": plugin {} ({}, {}) is not installed here. It stays silent; its settings are kept in the song.",
                i + 1,
                inst.name,
                p.name,
                p.format.to_uppercase(),
                p.id
            )),
            _ => None,
        })
        .collect()
}

/// current value of macro `k` of plugin instrument `instr`, normalised 0..1
pub fn macro_norm(s: &crate::audio::Shared, instr: usize, k: usize) -> Option<f32> {
    let Some(InstrumentKind::Plugin(p)) = s.song.instruments.get(instr).map(|x| &x.kind) else {
        return None;
    };
    let id = p.macro_id(k);
    let si = s.engine.plugins.primary(instr)?;
    let inst = &s.engine.plugins.slots[si].inst;
    let info = inst.params().into_iter().find(|x| x.id == id)?;
    let v = inst.value(id)?;
    Some(((v - info.min) / (info.max - info.min).max(1e-9)).clamp(0.0, 1.0) as f32)
}

/// set macro `k` from a normalised value (MIDI learn)
pub fn set_macro_norm(s: &mut crate::audio::Shared, instr: usize, k: usize, n: f32) -> bool {
    let Some(InstrumentKind::Plugin(p)) = s.song.instruments.get(instr).map(|x| &x.kind) else {
        return false;
    };
    let id = p.macro_id(k);
    let Some(si) = s.engine.plugins.primary(instr) else {
        return false;
    };
    let Some(info) = s.engine.plugins.slots[si]
        .inst
        .params()
        .into_iter()
        .find(|x| x.id == id)
    else {
        return false;
    };
    let v = info.min + (info.max - info.min) * n.clamp(0.0, 1.0) as f64;
    s.engine.plugins.set_param(instr, id, v);
    true
}

#[cfg(test)]
pub mod tests;
