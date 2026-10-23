//! Plugin effects (CLAP FX) in the FX chains (Unreleased, after 0.2.4.0).
//!
//! A `FxKind::Plugin` device holds a `PluginPatch` (`Device::plugin`). The
//! engine keeps one running instance per (chain, owner instrument, device
//! uid) in this rack: chains are the per-channel insert chains (index =
//! channel) and the live-jam chain (`LIVE`), exactly where the built-in
//! devices run. Instances are created on the UI / export thread and moved in
//! under the audio lock; the audio thread only queues parameter events and
//! processes audio (no allocation, no locks, no library calls besides the
//! plugin's own `process`).
//!
//! The chains run sample by sample, a plugin wants blocks: each slot buffers
//! `FX_BLOCK` samples, so a plugin device adds `FX_BLOCK` samples plus the
//! latency the plugin reports. Plugin delay compensation delays the other
//! channels so every channel reaches the mixer aligned (also in export).
use super::{instantiate, Hosted, PluginPatch};
use crate::model::{Song, MAX_CHANNELS};

/// samples per plugin call in an FX chain (= the latency it adds on top of
/// the plugin's own)
pub const FX_BLOCK: usize = 64;
/// chain index of the live-jam chain (0..MAX_CHANNELS = channel chains)
pub const LIVE: usize = MAX_CHANNELS;
/// longest compensated delay per channel (samples, about 0.68 s at 48 kHz)
pub const PDC_MAX: usize = 32768;

pub struct FxSlot {
    pub chain: usize,
    pub owner: usize,
    pub uid: u32,
    pub inst: Box<dyn Hosted>,
    key: String,
    inl: Box<[f32]>,
    inr: Box<[f32]>,
    outl: Box<[f32]>,
    outr: Box<[f32]>,
    dryl: Box<[f32]>,
    dryr: Box<[f32]>,
    pos: usize,
    /// automatable parameters (id, min, max), sorted by id
    ranges: Vec<(u32, f64, f64)>,
    /// last automation value sent per `ranges` entry (< 0 = none)
    sent: Vec<f32>,
    /// the parameter's own value per `ranges` entry, normalised (what it
    /// returns to when automation / modulation stops driving it)
    base: Vec<f32>,
    /// `ranges` entries driven since the last release pass
    touched: Vec<bool>,
    /// FX_BLOCK + the plugin's reported latency
    pub latency: u32,
    /// on/off crossfade (1 = plugin, 0 = dry)
    gain: f32,
}

impl FxSlot {
    fn new(chain: usize, owner: usize, uid: u32, p: &PluginPatch, inst: Box<dyn Hosted>) -> Self {
        let mut ranges: Vec<(u32, f64, f64)> = inst
            .params()
            .iter()
            .filter(|i| !i.readonly)
            .map(|i| (i.id, i.min, i.max))
            .collect();
        ranges.sort_by_key(|r| r.0);
        let base = ranges
            .iter()
            .map(|&(id, lo, hi)| norm_of(inst.value(id), lo, hi))
            .collect();
        let z = || vec![0.0f32; FX_BLOCK].into_boxed_slice();
        FxSlot {
            chain,
            owner,
            uid,
            latency: FX_BLOCK as u32 + inst.latency(),
            inst,
            key: key(p, uid),
            inl: z(),
            inr: z(),
            outl: z(),
            outr: z(),
            dryl: z(),
            dryr: z(),
            pos: 0,
            sent: vec![-1.0; ranges.len()],
            touched: vec![false; ranges.len()],
            base,
            ranges,
            gain: 1.0,
        }
    }
}

/// a plugin value over its range, 0..1 (0.5 when unknown)
fn norm_of(v: Option<f64>, lo: f64, hi: f64) -> f32 {
    match v {
        Some(v) if hi > lo => ((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32,
        _ => 0.5,
    }
}

fn key(p: &PluginPatch, uid: u32) -> String {
    format!("{}#{uid}", p.key())
}

/// a plugin device that could not be loaded: (owner, uid, key, message)
#[derive(Clone, Debug)]
pub struct FxError {
    pub owner: usize,
    pub uid: u32,
    key: String,
    pub msg: String,
}

/// Engine-side set of plugin effect instances.
pub struct FxRack {
    pub slots: Vec<FxSlot>,
    pub errors: Vec<FxError>,
    /// plugin delay compensation: per channel delay lines (allocated on the
    /// main thread with the first slot) and the chain latencies
    pdc: Vec<[Box<[f32]>; 2]>,
    pdc_w: usize,
    pub ch_lat: [u32; MAX_CHANNELS],
    pub max_lat: u32,
}

impl Default for FxRack {
    fn default() -> Self {
        Self::new()
    }
}

impl FxRack {
    pub fn new() -> FxRack {
        FxRack {
            slots: Vec::with_capacity(64),
            errors: vec![],
            pdc: vec![],
            pdc_w: 0,
            ch_lat: [0; MAX_CHANNELS],
            max_lat: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    // ---------------------------------------------------------- audio thread

    /// slot of (chain, owner, uid); `u16::MAX` = none
    pub fn find(&self, chain: usize, owner: usize, uid: u32) -> u16 {
        self.slots
            .iter()
            .position(|s| s.chain == chain && s.owner == owner && s.uid == uid)
            .map(|i| i as u16)
            .unwrap_or(u16::MAX)
    }

    /// automation: parameter `id` to a normalised value (sent when it changes)
    pub fn automate(&mut self, si: u16, id: u32, n: f32) {
        let Some(s) = self.slots.get_mut(si as usize) else {
            return;
        };
        let Ok(k) = s.ranges.binary_search_by_key(&id, |r| r.0) else {
            return;
        };
        let n = n.clamp(0.0, 1.0);
        s.touched[k] = true;
        if (n - s.sent[k]).abs() < 1e-5 {
            return;
        }
        s.sent[k] = n;
        let (_, lo, hi) = s.ranges[k];
        s.inst.param(id, lo + (hi - lo) * n as f64);
    }

    /// the parameter's own (undriven) value of slot `si` in its units
    pub fn base_value(&self, si: usize, id: u32) -> Option<f64> {
        let s = self.slots.get(si)?;
        let k = s.ranges.binary_search_by_key(&id, |r| r.0).ok()?;
        let (_, lo, hi) = s.ranges[k];
        Some(lo + (hi - lo) * s.base[k] as f64)
    }

    /// the parameter's own (undriven) value of slot `si`, normalised
    pub fn base_norm(&self, si: usize, id: u32) -> Option<f32> {
        let s = self.slots.get(si)?;
        let k = s.ranges.binary_search_by_key(&id, |r| r.0).ok()?;
        Some(s.base[k])
    }

    /// Parameters automation / modulation drove before but not since the
    /// last call go back to their own value. Call after each automation
    /// pass. No allocation.
    pub fn release_untouched(&mut self) {
        for s in &mut self.slots {
            for k in 0..s.ranges.len() {
                if s.touched[k] {
                    s.touched[k] = false;
                } else if s.sent[k] >= 0.0 {
                    s.sent[k] = -1.0;
                    let (id, lo, hi) = s.ranges[k];
                    s.inst.param(id, lo + (hi - lo) * s.base[k] as f64);
                }
            }
        }
    }

    /// one sample through slot `si`; `on` = device enabled (bypass crossfades
    /// to the equally delayed dry signal)
    pub fn process(&mut self, si: u16, l: f32, r: f32, on: bool, step: f32) -> (f32, f32) {
        let Some(s) = self.slots.get_mut(si as usize) else {
            return (l, r);
        };
        let p = s.pos;
        s.inl[p] = l;
        s.inr[p] = r;
        let (wl, wr, dl, dr) = (s.outl[p], s.outr[p], s.dryl[p], s.dryr[p]);
        s.gain = if on {
            (s.gain + step).min(1.0)
        } else {
            (s.gain - step).max(0.0)
        };
        s.pos += 1;
        if s.pos == FX_BLOCK {
            s.pos = 0;
            s.dryl.copy_from_slice(&s.inl);
            s.dryr.copy_from_slice(&s.inr);
            s.inst
                .process_fx(FX_BLOCK, &s.inl, &s.inr, &mut s.outl, &mut s.outr);
        }
        let g = s.gain;
        (dl + (wl - dl) * g, dr + (wr - dr) * g)
    }

    /// panic: drop tails and buffered audio
    pub fn panic(&mut self) {
        for s in &mut self.slots {
            s.inst.panic();
            for b in [
                &mut s.inl,
                &mut s.inr,
                &mut s.outl,
                &mut s.outr,
                &mut s.dryl,
                &mut s.dryr,
            ] {
                b.fill(0.0);
            }
        }
        for p in &mut self.pdc {
            p[0].fill(0.0);
            p[1].fill(0.0);
        }
    }

    /// delay compensation for channel `ch`: delays it by `max_lat - ch_lat`
    /// (call once per channel per sample, then `pdc_advance`)
    pub fn compensate(&mut self, ch: usize, l: f32, r: f32) -> (f32, f32) {
        let d = self
            .max_lat
            .saturating_sub(self.ch_lat.get(ch).copied().unwrap_or(0)) as usize;
        let Some(line) = self.pdc.get_mut(ch) else {
            return (l, r);
        };
        let w = self.pdc_w;
        line[0][w] = l;
        line[1][w] = r;
        if d == 0 {
            return (l, r);
        }
        let ri = (w + PDC_MAX - d.min(PDC_MAX - 1)) % PDC_MAX;
        (line[0][ri], line[1][ri])
    }

    pub fn pdc_advance(&mut self) {
        if !self.pdc.is_empty() {
            self.pdc_w = (self.pdc_w + 1) % PDC_MAX;
        }
    }

    pub fn pdc_active(&self) -> bool {
        self.max_lat > 0 && !self.pdc.is_empty()
    }

    // ---------------------------------------------------------- main thread

    /// the instance the UI talks to for a device (state, editor, parameters)
    pub fn primary(&self, owner: usize, uid: u32) -> Option<usize> {
        // prefer a channel chain, then the live chain
        self.slots
            .iter()
            .position(|s| s.owner == owner && s.uid == uid && s.chain != LIVE)
            .or_else(|| {
                self.slots
                    .iter()
                    .position(|s| s.owner == owner && s.uid == uid)
            })
    }

    pub fn error(&self, owner: usize, uid: u32) -> Option<&str> {
        self.errors
            .iter()
            .find(|e| e.owner == owner && e.uid == uid)
            .map(|e| e.msg.as_str())
    }

    /// (chain, owner, uid, patch) for every instance the song needs; `extra`
    /// = (instrument, channel) under the cursor (jamming: its channel chain
    /// and the live chain)
    pub fn needed(
        song: &Song,
        extra: Option<(usize, usize)>,
    ) -> Vec<(usize, usize, u32, PluginPatch)> {
        let ni = song.instruments.len();
        let has = |i: usize| {
            song.instruments.get(i).is_some_and(|x| {
                x.fx.iter()
                    .any(|d| d.kind == crate::fx::FxKind::Plugin && d.plugin.is_some())
            })
        };
        if !(0..ni).any(has) {
            return vec![];
        }
        let nch = song.channels.min(MAX_CHANNELS);
        let mut used = vec![false; ni * (MAX_CHANNELS + 1)];
        let mut mark = |i: usize, c: usize| {
            if i < ni && c <= MAX_CHANNELS && has(i) {
                used[i * (MAX_CHANNELS + 1) + c] = true;
            }
        };
        // timeline mode: a track can play any block column, so every channel
        let timeline = song.timeline.is_some();
        for p in &song.patterns {
            for row in &p.cells {
                for (c, cell) in row.iter().enumerate().take(nch) {
                    if cell.instr > 0 {
                        let i = cell.instr as usize - 1;
                        if timeline {
                            for t in 0..nch {
                                mark(i, t);
                            }
                        } else {
                            mark(i, c);
                        }
                    }
                }
            }
        }
        if let Some((i, c)) = extra {
            if c < nch {
                mark(i, c);
            }
            mark(i, LIVE);
        }
        let mut v = vec![];
        for (i, inst) in song.instruments.iter().enumerate() {
            for c in 0..=MAX_CHANNELS {
                if !used[i * (MAX_CHANNELS + 1) + c] {
                    continue;
                }
                for d in &inst.fx {
                    if let (crate::fx::FxKind::Plugin, Some(p)) = (d.kind, &d.plugin) {
                        v.push((c, i, d.uid, (**p).clone()));
                    }
                }
            }
        }
        v
    }

    /// what to create and which slots to drop to match `song`
    pub fn plan(&self, song: &Song, extra: Option<(usize, usize)>) -> FxPlan {
        let need = FxRack::needed(song, extra);
        let mut add = vec![];
        for (c, i, uid, p) in &need {
            let k = key(p, *uid);
            let have = self
                .slots
                .iter()
                .any(|s| s.chain == *c && s.owner == *i && s.uid == *uid && s.key == k);
            let failed = self
                .errors
                .iter()
                .any(|e| e.owner == *i && e.uid == *uid && e.key == k);
            if !have && !failed {
                add.push((*c, *i, *uid, p.clone()));
            }
        }
        let drop = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                !need.iter().any(|(c, i, uid, p)| {
                    s.chain == *c && s.owner == *i && s.uid == *uid && s.key == key(p, *uid)
                })
            })
            .map(|(k, _)| k)
            .collect();
        FxPlan { add, drop }
    }

    /// take out the slots in `drop` (dropped outside the lock)
    pub fn take(&mut self, drop: &[usize]) -> Vec<FxSlot> {
        let mut taken = vec![];
        for k in (0..self.slots.len()).rev() {
            if drop.contains(&k) {
                taken.push(self.slots.remove(k));
            }
        }
        taken
    }

    /// add an instance (main thread, under the audio lock)
    pub fn insert(
        &mut self,
        chain: usize,
        owner: usize,
        uid: u32,
        p: &PluginPatch,
        inst: Box<dyn Hosted>,
    ) {
        if self.pdc.is_empty() {
            let z = || vec![0.0f32; PDC_MAX].into_boxed_slice();
            self.pdc = (0..MAX_CHANNELS).map(|_| [z(), z()]).collect();
        }
        self.slots
            .retain(|s| !(s.chain == chain && s.owner == owner && s.uid == uid));
        self.slots.push(FxSlot::new(chain, owner, uid, p, inst));
        self.errors.retain(|e| !(e.owner == owner && e.uid == uid));
    }

    pub fn fail(&mut self, owner: usize, uid: u32, p: &PluginPatch, msg: String) {
        self.errors.retain(|e| !(e.owner == owner && e.uid == uid));
        self.errors.push(FxError {
            owner,
            uid,
            key: key(p, uid),
            msg,
        });
    }

    /// forget load errors (Reload / a new scan) so the next sync tries again
    pub fn retry(&mut self) {
        self.errors.clear();
    }

    /// instruments were renumbered (see Engine::remap_instruments)
    pub fn remap(&mut self, f: &dyn Fn(usize) -> Option<usize>) -> Vec<FxSlot> {
        let mut gone = vec![];
        let mut k = 0;
        while k < self.slots.len() {
            match f(self.slots[k].owner) {
                Some(n) => {
                    self.slots[k].owner = n;
                    k += 1;
                }
                None => gone.push(self.slots.remove(k)),
            }
        }
        self.errors = std::mem::take(&mut self.errors)
            .into_iter()
            .filter_map(|mut e| {
                f(e.owner).map(|n| {
                    e.owner = n;
                    e
                })
            })
            .collect();
        gone
    }

    /// UI parameter edit: every instance of the device
    pub fn set_param(&mut self, owner: usize, uid: u32, id: u32, v: f64) {
        for s in self
            .slots
            .iter_mut()
            .filter(|s| s.owner == owner && s.uid == uid)
        {
            s.inst.set_param_main(id, v);
            if let Ok(k) = s.ranges.binary_search_by_key(&id, |r| r.0) {
                let (_, lo, hi) = s.ranges[k];
                s.base[k] = norm_of(Some(v), lo, hi);
            }
        }
    }

    /// main-thread housekeeping; returns devices (owner, uid) whose plugin
    /// changed its own state (editor edits)
    pub fn poll_main(&mut self) -> Vec<(usize, u32)> {
        let mut dirty = vec![];
        for s in &mut self.slots {
            s.inst.poll_main();
            if s.inst.take_dirty() {
                // editor edits: the new own values of the undriven parameters
                for k in 0..s.ranges.len() {
                    if s.sent[k] < 0.0 {
                        let (id, lo, hi) = s.ranges[k];
                        s.base[k] = norm_of(s.inst.value(id), lo, hi);
                    }
                }
                if !dirty.contains(&(s.owner, s.uid)) {
                    dirty.push((s.owner, s.uid));
                }
            }
        }
        dirty
    }

    /// Save the primary instance's state of device `uid` of instrument
    /// `owner` into the song and copy it to the device's other instances.
    pub fn capture(&mut self, song: &mut Song, owner: usize, uid: u32) -> bool {
        let Some(pi) = self.primary(owner, uid) else {
            return false;
        };
        let Some(dev) = song
            .instruments
            .get_mut(owner)
            .and_then(|i| i.fx.iter_mut().find(|d| d.uid == uid && d.plugin.is_some()))
        else {
            return false;
        };
        let p = dev.plugin.as_mut().unwrap();
        let st = self.slots[pi].inst.save_state();
        match &st {
            Some(b) => {
                p.set_state_bytes(b);
                p.params.clear();
            }
            None => {
                let inst = &self.slots[pi].inst;
                p.params = inst
                    .params()
                    .iter()
                    .filter(|i| !i.readonly)
                    .filter_map(|i| inst.value(i.id).map(|v| (i.id, v)))
                    .collect();
            }
        }
        let k = key(p, uid);
        let params = p.params.clone();
        for (j, s) in self.slots.iter_mut().enumerate() {
            if j == pi || s.owner != owner || s.uid != uid {
                continue;
            }
            s.key = k.clone();
            // unchanged instances are left alone (no cut tails on autosave)
            super::copy_state(st.as_deref(), &params, s.inst.as_mut());
        }
        true
    }

    /// capture every device that has an instance (before save / export)
    pub fn capture_all(&mut self, song: &mut Song) {
        let mut seen: Vec<(usize, u32)> = vec![];
        for s in &self.slots {
            if !seen.contains(&(s.owner, s.uid)) {
                seen.push((s.owner, s.uid));
            }
        }
        for (o, u) in seen {
            self.capture(song, o, u);
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        for s in &mut self.slots {
            s.inst.set_sample_rate(sr);
            s.latency = FX_BLOCK as u32 + s.inst.latency();
        }
    }
}

pub struct FxPlan {
    pub add: Vec<(usize, usize, u32, PluginPatch)>,
    pub drop: Vec<usize>,
}

/// Bring the effect rack of `shared`'s engine in line with its song: instances
/// are created without the audio lock and moved in under it. A device's
/// second instance (another channel) starts from the first one's state.
pub fn sync(shared: &crate::audio::SharedRef, extra: Option<(usize, usize)>) {
    let (plan, sr) = {
        let s = crate::audio::lock(shared);
        (s.engine.fx_plugins.plan(&s.song, extra), s.engine.sr)
    };
    if plan.add.is_empty() && plan.drop.is_empty() {
        return;
    }
    let old = {
        let mut s = crate::audio::lock(shared);
        s.engine.fx_plugins.take(&plan.drop)
    };
    drop(old);
    for (c, i, uid, mut p) in plan.add {
        {
            let mut s = crate::audio::lock(shared);
            let crate::audio::Shared { song, engine, .. } = &mut *s;
            if engine.fx_plugins.primary(i, uid).is_some() {
                engine.fx_plugins.capture(song, i, uid);
                if let Some(q) = device_patch(song, i, uid) {
                    p = q.clone();
                }
            }
        }
        let r = instantiate(&p, sr);
        let mut s = crate::audio::lock(shared);
        // the song may have changed while loading
        let still = device_patch(&s.song, i, uid).is_some_and(|q| q.key() == p.key());
        if !still {
            continue;
        }
        match r {
            Ok(inst) => s.engine.fx_plugins.insert(c, i, uid, &p, inst),
            Err(e) => s.engine.fx_plugins.fail(i, uid, &p, e),
        }
    }
}

/// the plugin patch of device `uid` of instrument `owner`
pub fn device_patch(song: &Song, owner: usize, uid: u32) -> Option<&PluginPatch> {
    song.instruments
        .get(owner)?
        .fx
        .iter()
        .find(|d| d.uid == uid)
        .and_then(|d| d.plugin.as_deref())
}

/// Offline: create every effect instance the song needs (export, --render,
/// tests).
pub fn prepare(rack: &mut FxRack, song: &Song, sr: f32) {
    let plan = rack.plan(song, None);
    drop(rack.take(&plan.drop));
    for (c, i, uid, p) in plan.add {
        match instantiate(&p, sr) {
            Ok(inst) => rack.insert(c, i, uid, &p, inst),
            Err(e) => rack.fail(i, uid, &p, e),
        }
    }
}

/// Warnings for plugin effects that can't be found here (song load).
pub fn missing_warnings(song: &Song) -> Vec<String> {
    let mut v = vec![];
    for (i, inst) in song.instruments.iter().enumerate() {
        for (d, dev) in inst.fx.iter().enumerate() {
            if let Some(p) = &dev.plugin {
                if super::scan::resolve(p).is_none() {
                    v.push(format!(
                        "Instrument {:02} \"{}\", FX {}: plugin effect {} ({}, {}) is not installed here. It is bypassed; its settings are kept in the song.",
                        i + 1,
                        inst.name,
                        d + 1,
                        p.name,
                        p.format.to_uppercase(),
                        p.id
                    ));
                }
            }
        }
    }
    v
}

/// does the song use plugin effects at all
pub fn song_has_fx(song: &Song) -> bool {
    song.instruments
        .iter()
        .any(|i| i.fx.iter().any(|d| d.plugin.is_some()))
}
