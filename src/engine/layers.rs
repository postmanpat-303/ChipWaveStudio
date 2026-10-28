//! Rack layers in the engine (docs/racks/PLAN.md Stage 4c): a note on a
//! rack starts a voice on every layer that takes its key. Each track has
//! its own layer voices (8 per layer) and one effect chain per layer (the
//! layer slot's own effects); the layers are added up, each at its level
//! and pan, before the track's chain (the rack's effects).
//!
//! Nothing here is allocated until a song has a rack, so every other song
//! takes exactly the old code paths. While playing, nothing allocates.
use super::notes::{NoteVoice, NOTE_VOICES};
use super::*;
use crate::model::MAX_LAYERS;

/// voices per layer per track
pub(super) const LAYER_VOICES: usize = NOTE_VOICES;

/// one track's layers
pub(super) struct Bank {
    /// LAYER_VOICES per layer, layer `k` at `k * LAYER_VOICES`
    pub(super) v: Vec<NoteVoice>,
    pub(super) chains: Vec<crate::fx::ChainState>,
    /// per layer: (left, right) gain after its chain
    pub(super) gains: [(f32, f32); MAX_LAYERS],
    /// the rack whose layers the chains follow (usize::MAX = none)
    pub(super) rack: usize,
}

impl Bank {
    fn new() -> Self {
        Self {
            v: vec![NoteVoice::new(); MAX_LAYERS * LAYER_VOICES],
            chains: (0..MAX_LAYERS).map(|_| Default::default()).collect(),
            gains: [(0.0, 0.0); MAX_LAYERS],
            rack: usize::MAX,
        }
    }
}

#[derive(Default)]
pub(super) struct Layers {
    /// MAX_CHANNELS banks once a song has a rack, else empty
    pub(super) banks: Vec<Bank>,
    /// some layer voice is sounding
    pub(super) any: bool,
    age: u32,
    /// drum pad Round Robin hit numbers per key
    hits: crate::drumkit::padrr::HitCount,
}

/// the song has a rack instrument
pub(super) fn has_rack(song: &Song) -> bool {
    song.instruments
        .iter()
        .any(|i| matches!(i.kind, InstrumentKind::Rack(_)))
}

impl Layers {
    /// the banks, made once (start of play / effect refresh), never while
    /// notes play
    pub(super) fn ensure(&mut self, song: &Song) {
        if self.banks.is_empty() && has_rack(song) {
            self.banks = (0..MAX_CHANNELS).map(|_| Bank::new()).collect();
        }
    }

    /// Start note `key` of rack `rack` on track `ch` on each layer that
    /// takes the key. `gain` is the note's level on the rack (its volume
    /// times the velocity or the tracker volume), `vel` the velocity the
    /// layer's synth sees (`set_vel`: given to it, as the tracker does),
    /// `off_at` the note-off (tick; INFINITY = until the
    /// tracker ends it). Plugin instruments and racks never play as layers.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn on(
        &mut self,
        song: &Song,
        ch: usize,
        rack: usize,
        key: u8,
        tune: f32,
        gain: f32,
        vel: f32,
        set_vel: bool,
        off_at: f64,
    ) {
        let Some(InstrumentKind::Rack(r)) = song.instruments.get(rack).map(|i| &i.kind) else {
            return;
        };
        let rt = song.instruments[rack].transpose as i32;
        let Some(b) = self.banks.get_mut(ch) else {
            return;
        };
        for (k, l) in r.layers.iter().enumerate().take(MAX_LAYERS) {
            if !l.takes(key) {
                continue;
            }
            let Some(li) = song.instruments.get(l.slot) else {
                continue;
            };
            if matches!(li.kind, InstrumentKind::Plugin(_) | InstrumentKind::Rack(_)) {
                continue;
            }
            self.age = self.age.wrapping_add(1);
            let base = k * LAYER_VOICES;
            let pool = &b.v[base..base + LAYER_VOICES];
            let slot = pool
                .iter()
                .position(|v| !v.active)
                .or_else(|| {
                    let oldest = |only_released: bool| {
                        pool.iter()
                            .enumerate()
                            .filter(|(_, v)| !only_released || !v.down)
                            .min_by_key(|(_, v)| v.age)
                            .map(|(k, _)| k)
                    };
                    oldest(true).or_else(|| oldest(false))
                })
                .unwrap_or(0)
                + base;
            let n =
                (key as i32 + rt + l.transpose as i32 + li.transpose as i32).clamp(1, 127) as u8;
            let freq = note_freq(n) * 2f32.powf(tune / 12.0);
            let mut vol = gain * li.volume;
            let mut synth = SynthVoice::new();
            if let Some(kit) = crate::drumkit::kit_of(li) {
                synth.gen.hit = self.hits.next(n);
                vol = gain * li.volume * crate::drumkit::hit(kit, freq, vel).0 / vel.max(1e-6);
            }
            if notes::poly_patch(li).is_some() {
                // velocity goes to the synth (Vel > amp), as on a track
                vol = gain * li.volume / vel.max(1e-6);
                synth = notes::poly_voice(vel, 0.0, self.age.wrapping_mul(0x9E37_79B9));
            } else if set_vel {
                synth.vel = vel;
            }
            if let InstrumentKind::Voice(vp) = &li.kind {
                if let Some(w) = vp.words.first() {
                    synth.vox.speak(w);
                }
            }
            let v = &mut b.v[slot];
            v.sw_stolen();
            if crate::strings::rack::patch(song, l.slot).is_some() {
                v.sw_start(l.slot, midi_key(n), vel);
            }
            v.instr = l.slot;
            v.freq = freq;
            v.vol = vol;
            v.synth = synth;
            v.pos = 0.0;
            v.active = true;
            v.down = true;
            v.off_at = off_at;
            v.age = self.age;
            self.any = true;
        }
    }

    fn release(v: &mut NoteVoice) {
        v.down = false;
        v.synth.amp.release();
        v.synth.fenv.release();
    }

    /// the tracker ended the note on track `ch` (note-off or a new note)
    pub(super) fn off_tracker(&mut self, ch: usize) {
        if let Some(b) = self.banks.get_mut(ch) {
            for v in b.v.iter_mut() {
                if v.active && v.down && v.off_at == f64::INFINITY {
                    Self::release(v);
                }
            }
        }
    }

    /// note-offs due at tick `t` (launcher tracks: launch tick `lc`)
    pub(super) fn events(&mut self, nch: usize, t: f64, lc: f64, lmask: u64) {
        if !self.any {
            return;
        }
        for (ch, b) in self.banks.iter_mut().enumerate().take(nch) {
            let now = if lmask >> ch.min(63) & 1 == 1 { lc } else { t };
            for v in b.v.iter_mut() {
                if v.active && v.down && now >= v.off_at {
                    Self::release(v);
                }
            }
        }
    }

    /// release the held layer notes of the tracks in `mask` that end by
    /// `now` (tracker cells through an Arp: the launch clock)
    pub(super) fn release_due(&mut self, mask: u64, now: f64) {
        if !self.any {
            return;
        }
        for (ch, b) in self.banks.iter_mut().enumerate() {
            if mask >> ch.min(63) & 1 == 0 {
                continue;
            }
            for v in b.v.iter_mut() {
                if v.active && v.down && now >= v.off_at {
                    Self::release(v);
                }
            }
        }
    }

    /// release the held layer notes of the tracks in `mask`
    pub(super) fn release_where(&mut self, mask: u64) {
        for (ch, b) in self.banks.iter_mut().enumerate() {
            if mask >> ch.min(63) & 1 == 0 {
                continue;
            }
            for v in b.v.iter_mut().filter(|v| v.active && v.down) {
                Self::release(v);
            }
        }
    }

    pub(super) fn silence(&mut self) {
        for b in &mut self.banks {
            for v in &mut b.v {
                v.active = false;
                v.down = false;
            }
        }
        self.any = false;
    }

    /// layer chains and gains follow rack `owner` on track `ch` (effect
    /// refresh; buffers change only when a chain's structure does)
    pub(super) fn sync(&mut self, song: &Song, ch: usize, owner: usize, sr: f32, a: &AutoState) {
        let Some(b) = self.banks.get_mut(ch) else {
            return;
        };
        let layers: &[crate::model::Layer] = match song.instruments.get(owner).map(|i| &i.kind) {
            Some(InstrumentKind::Rack(r)) => &r.layers,
            _ => &[],
        };
        b.rack = if layers.is_empty() { usize::MAX } else { owner };
        for k in 0..MAX_LAYERS {
            let (devs, slot, g) = match layers.get(k) {
                Some(l) => (
                    song.instruments
                        .get(l.slot)
                        .map(|x| x.fx.as_slice())
                        .unwrap_or(&[]),
                    l.slot,
                    l.gains(),
                ),
                None => (&[][..], usize::MAX, (0.0, 0.0)),
            };
            b.gains[k] = g;
            let c = &mut b.chains[k];
            c.sync(devs, slot, sr);
            for (d, (st, dev)) in c.states.iter_mut().zip(devs).enumerate() {
                st.set_targets(dev, |p| {
                    a.fx.iter()
                        .find(|o| o.0 == slot && o.1 == d && o.2 == p)
                        .map(|o| o.3)
                });
            }
        }
    }

    /// track `ch`'s layers, summed after their chains and gains. Sets
    /// `alive` when a voice still sounds.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render(
        &mut self,
        ch: usize,
        song: &Song,
        sr: f32,
        auto: &AutoState,
        bpm: f32,
        fxp: &mut crate::plugin::fxrack::FxRack,
        sw: &mut crate::strings::rack::Rack,
        sw_chunk: usize,
        alive: &mut bool,
    ) -> (f32, f32) {
        let Some(b) = self.banks.get_mut(ch) else {
            return (0.0, 0.0);
        };
        // StringWave layers sound at the rack's volume (as layer voices do)
        let rack_vol = song.instruments.get(b.rack).map_or(1.0, |x| x.volume);
        let (mut l, mut r) = (0.0, 0.0);
        for k in 0..MAX_LAYERS {
            let base = k * LAYER_VOICES;
            let (mut s, mut sr2) = (0.0, 0.0);
            let mut on = false;
            let mut note = 0.0;
            for (j, v) in b.v[base..base + LAYER_VOICES]
                .iter_mut()
                .enumerate()
                .filter(|(_, v)| v.active)
            {
                if v.sw_busy() {
                    notes::sw_pull(
                        v,
                        sw,
                        song,
                        ch,
                        k as u8,
                        crate::strings::rack::LAYER + (base + j) as u16,
                    );
                }
                let (x, xr) = v.render(song, sr, auto, bpm);
                s += x;
                sr2 += xr;
                on = true;
                note = v.freq;
                *alive |= v.active;
            }
            if !sw.is_empty() {
                let (x, xr, sounding) =
                    sw_sample(sw, song, auto, ch, k as u8, None, sw_chunk, rack_vol);
                s += x;
                sr2 += xr;
                on |= sounding;
            }
            let c = &mut b.chains[k];
            if !on && c.states.is_empty() {
                continue;
            }
            let (fl, fr) = if c.states.is_empty() {
                (s, sr2)
            } else {
                c.process_with(s, sr2, &crate::fx::Ctx { bpm, note }, fxp)
            };
            let (gl, gr) = b.gains[k];
            l += fl * gl;
            r += fr * gr;
        }
        (l, r)
    }
}
