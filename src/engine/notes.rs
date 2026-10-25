//! Note-clip playback (M3b, docs/TIMELINE.md "Note clips"): a polyphonic
//! voice pool per track, fed by the plan's note spans at sample accuracy
//! on the row clock. Only runs when the timeline has note clips, so
//! tracker songs and block-only timelines render exactly as before.
use super::*;
use crate::timeline::{NoteSpan, Plan, Tick, ROW};

/// voices per track for note clips
pub const NOTE_VOICES: usize = 8;
/// plugin live-note tags for note voices (MIDI keys use 0..=255)
const PLUG_TAG: u16 = 0x100;

#[derive(Clone)]
pub(super) struct NoteVoice {
    pub(super) instr: usize,
    pub(super) freq: f32,
    pub(super) vol: f32,
    pub(super) synth: SynthVoice,
    pub(super) pos: f64,
    pub(super) active: bool,
    /// still before its note-off
    pub(super) down: bool,
    /// timeline tick of the note-off
    pub(super) off_at: f64,
    pub(super) age: u32,
    /// the note plays on a plugin instrument (no synth voice)
    plug: bool,
    /// the note plays on StringWave: the voice only tracks the note, the
    /// rack slot sounds it ([`sw_pull`])
    pub(super) sw: bool,
    /// StringWave: the note-on is still to be sent (MIDI key, velocity)
    sw_new: Option<(u8, f32)>,
    /// StringWave: the MIDI key sent and not yet released
    sw_sent: Option<u8>,
    /// StringWave: a stolen note to release first (instrument, MIDI key)
    sw_kill: Option<(usize, u8)>,
}

impl NoteVoice {
    /// one stereo sample (plugin notes sound through the plugin rack)
    pub(super) fn render(
        &mut self,
        song: &Song,
        sr: f32,
        auto: &AutoState,
        bpm: f32,
    ) -> (f32, f32) {
        if self.plug || self.sw {
            return (0.0, 0.0);
        }
        let ((x, xr), alive) = voice(
            song,
            self.instr,
            self.freq,
            &mut self.synth,
            &mut self.pos,
            sr,
            auto,
            bpm,
        );
        self.active = alive;
        (x * self.vol, xr * self.vol)
    }

    pub(super) fn new() -> Self {
        Self {
            instr: usize::MAX,
            freq: 0.0,
            vol: 0.0,
            synth: SynthVoice::new(),
            pos: 0.0,
            active: false,
            down: false,
            off_at: 0.0,
            age: 0,
            plug: false,
            sw: false,
            sw_new: None,
            sw_sent: None,
            sw_kill: None,
        }
    }

    /// Start StringWave note `key` (MIDI) at `vel` on this voice for
    /// instrument `instr`; a note it still sounds is released first.
    pub(super) fn sw_start(&mut self, instr: usize, key: u8, vel: f32) {
        self.sw_stolen();
        self.instr = instr;
        self.sw = true;
        self.sw_new = Some((key, vel));
    }

    /// StringWave note events to send ([`sw_pull`])
    pub(super) fn sw_busy(&self) -> bool {
        self.sw || self.sw_kill.is_some()
    }

    /// the voice is re-used: a StringWave note it sounds is to be released
    pub(super) fn sw_stolen(&mut self) {
        if self.sw && self.active {
            if let Some(k) = self.sw_sent.take() {
                self.sw_kill = Some((self.instr, k));
            }
        }
        self.sw_new = None;
        self.sw = false;
    }
}

/// Send a StringWave voice's pending note events to its rack slot
/// (instr, `ch`, `layer`) with note tag `tag`; the voice is free once its
/// note is released (the slot plays the tail).
pub(super) fn sw_pull(
    v: &mut NoteVoice,
    rack: &mut crate::strings::rack::Rack,
    song: &Song,
    ch: usize,
    layer: u8,
    tag: u16,
) {
    if let Some((i, key)) = v.sw_kill.take() {
        if let Some(si) = rack.find(i, ch, layer) {
            rack.slots[si].note_off(key, tag);
        }
    }
    if !v.sw {
        return;
    }
    if let Some((key, vel)) = v.sw_new.take() {
        if let Some(si) = rack.ensure(song, v.instr, ch, layer) {
            rack.slots[si].note_on(key, vel, tag);
            v.sw_sent = Some(key);
        }
    }
    if let Some(key) = v.sw_sent {
        if !v.down {
            if let Some(si) = rack.find(v.instr, ch, layer) {
                rack.slots[si].note_off(key, tag);
            }
            v.sw_sent = None;
        }
    }
    if v.sw_sent.is_none() {
        v.active = false;
    }
}

/// a launcher clip playing on a track (Stage 4b): it loops from `origin`
/// on the launch clock (rows played since the start, never jumps back)
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct LPlay {
    /// launcher clip id
    pub(super) id: u32,
    /// its index in `Plan::launch` (checked against `id` each use)
    idx: usize,
    pub(super) origin: Tick,
    pass: i64,
    cur: usize,
}

impl LPlay {
    pub(super) fn new(id: u32, origin: Tick) -> Self {
        Self {
            id,
            idx: usize::MAX,
            origin,
            pass: -1,
            cur: 0,
        }
    }
}

pub(super) struct NoteState {
    /// NOTE_VOICES per track, preallocated (no allocation while playing)
    pub(super) v: Vec<NoteVoice>,
    /// per track: the next span to start
    cur: [usize; MAX_CHANNELS],
    /// timeline position (ticks) at the last sample
    pub(super) clock: f64,
    age: u32,
    /// some voice is active (rendering is skipped otherwise)
    pub(super) any: bool,
    /// per track: the last note's frequency (poly synth glide)
    last_f: [f32; MAX_CHANNELS],
    /// tracks playing a launcher clip instead of the arrangement (bit per
    /// track; 0 = none, and then nothing below runs)
    pub(super) lmask: u64,
    pub(super) lp: [LPlay; MAX_CHANNELS],
    /// rack layers (empty until a song has a rack)
    pub(super) layers: super::layers::Layers,
}

impl NoteState {
    pub(super) fn new() -> Self {
        Self {
            v: vec![NoteVoice::new(); MAX_CHANNELS * NOTE_VOICES],
            cur: [0; MAX_CHANNELS],
            clock: 0.0,
            age: 0,
            any: false,
            last_f: [0.0; MAX_CHANNELS],
            lmask: 0,
            lp: [LPlay::default(); MAX_CHANNELS],
            layers: Default::default(),
        }
    }

    /// put the cursors on the first notes at or after tick `t`
    pub(super) fn seek(&mut self, plan: &Plan, t: f64) {
        for ch in 0..MAX_CHANNELS {
            self.cur[ch] = plan
                .notes
                .get(ch)
                .map_or(0, |v| v.partition_point(|n| (n.start as f64) < t));
        }
        self.clock = t;
        self.last_f = [0.0; MAX_CHANNELS];
    }

    /// release every held note (synth voices fade with their envelope)
    pub(super) fn release_all(&mut self, plugins: &mut crate::plugin::Rack) {
        self.release_where(plugins, u64::MAX);
    }

    /// release the held notes of the tracks in `mask` (bit per track)
    pub(super) fn release_where(&mut self, plugins: &mut crate::plugin::Rack, mask: u64) {
        self.layers.release_where(mask);
        for (k, v) in self.v.iter_mut().enumerate() {
            if mask >> (k / NOTE_VOICES).min(63) & 1 == 0 {
                continue;
            }
            if v.active && v.down {
                v.down = false;
                if v.plug {
                    plugins.live_off(PLUG_TAG + k as u16);
                    v.active = false;
                } else {
                    v.synth.amp.release();
                    v.synth.fenv.release();
                }
            }
        }
    }

    pub(super) fn silence(&mut self) {
        self.layers.silence();
        for v in &mut self.v {
            v.active = false;
            v.down = false;
        }
        self.any = false;
    }

    /// active voices of instrument `idx` (the poly synth's Voices readout)
    pub(super) fn voices_of(&self, idx: usize) -> usize {
        self.v.iter().filter(|v| v.active && v.instr == idx).count()
            + self
                .layers
                .banks
                .iter()
                .flat_map(|b| &b.v)
                .filter(|v| v.active && v.instr == idx)
                .count()
    }

    /// the number of sounding note voices on `track` (tests)
    #[cfg(test)]
    pub(super) fn sounding(&self, track: usize) -> usize {
        self.v[track * NOTE_VOICES..(track + 1) * NOTE_VOICES]
            .iter()
            .filter(|v| v.active && v.down)
            .count()
    }
}

impl NoteState {
    /// mark the instruments with a held note (modulator envelopes)
    pub(super) fn mark_held(&self, held: &mut [bool; 256]) {
        for v in self
            .v
            .iter()
            .chain(self.layers.banks.iter().flat_map(|b| &b.v))
        {
            if v.active && v.down && v.instr < 256 {
                held[v.instr] = true;
            }
        }
    }
}

/// the poly synth patch of an instrument (None for every other kind)
pub(super) fn poly_patch(inst: &crate::model::Instrument) -> Option<&crate::poly::PolyP> {
    match &inst.kind {
        InstrumentKind::Gen(g) => match &g.engine {
            crate::gen::GenEngine::Poly(x) => Some(x),
            _ => None,
        },
        _ => None,
    }
}

/// Poly synth voice choice in a pool: `(active, held, age, instrument)` per
/// voice. Mono / Legato: `Reuse(k)` the newest voice of the instrument
/// (`retrigger` false = Legato glide). Poly: at `polyphony` voices of the
/// instrument, steal its oldest (released first); else `Free`.
pub(super) enum PolyPick {
    Reuse(usize, bool),
    Steal(usize),
    Free,
}

pub(super) fn poly_pick(
    pp: &crate::poly::PolyP,
    instr: usize,
    pool: impl Iterator<Item = (bool, bool, u32, usize)> + Clone,
) -> PolyPick {
    use crate::poly::VoiceMode;
    let mine = pool
        .enumerate()
        .filter(move |(_, (a, _, _, i))| *a && *i == instr);
    if pp.mode != VoiceMode::Poly {
        if let Some((k, (_, held, _, _))) = mine.clone().max_by_key(|(_, v)| v.2) {
            return PolyPick::Reuse(k, !(pp.mode == VoiceMode::Legato && held));
        }
        return PolyPick::Free;
    }
    if mine.clone().count() >= pp.polyphony.max(1) as usize {
        let oldest = |only_released: bool| {
            mine.clone()
                .filter(|(_, v)| !only_released || !v.1)
                .min_by_key(|(_, v)| v.2)
                .map(|(k, _)| k)
        };
        if let Some(k) = oldest(true).or_else(|| oldest(false)) {
            return PolyPick::Steal(k);
        }
    }
    PolyPick::Free
}

/// a fresh poly synth voice: velocity to the synth (the patch's Vel > amp
/// and Vel > cut use it), a glide from `from` Hz (0 = none), its own seed
pub(super) fn poly_voice(vel: f32, from: f32, seed: u32) -> SynthVoice {
    let mut sv = SynthVoice::seeded(seed);
    sv.vel = vel;
    sv.gen.ps.glide_from = from;
    sv
}

/// re-use a poly synth voice for a new note (Mono / Legato)
pub(super) fn poly_reuse(sv: &mut SynthVoice, vel: f32, retrigger: bool) {
    sv.gen.ps.glide_from = sv.gen.ps.sounding();
    sv.vel = vel;
    if retrigger {
        sv.gen.ps.retrigger = true;
        // the gate the poly synth reads (its own envelopes do the rest)
        sv.amp = crate::synth::Env::new();
        sv.fenv = crate::synth::Env::new();
    }
}

/// Start note `n` on track `ch`: a free voice, else the oldest released
/// one, else the oldest (voice stealing).
#[allow(clippy::too_many_arguments)]
pub(super) fn note_on(
    st: &mut NoteState,
    feed: &mut crate::modulation::NoteFeed,
    song: &Song,
    chans: &mut [Chan],
    plugins: &mut crate::plugin::Rack,
    ch: usize,
    n: &NoteSpan,
) {
    let c = &mut chans[ch];
    let idx = if n.instr > 0 {
        n.instr as usize - 1
    } else if c.instr != usize::MAX {
        c.instr
    } else {
        0
    };
    let Some(inst) = song.instruments.get(idx) else {
        return;
    };
    feed.note_on(idx, n.key, n.vel);
    // a track with only note clips: its insert chain follows their instrument
    if c.instr == usize::MAX {
        c.instr = idx;
    }
    // a rack is silent itself: its layers play the note
    if matches!(inst.kind, InstrumentKind::Rack(_)) {
        let gain = inst.volume * n.vel;
        st.layers.on(
            song,
            ch,
            idx,
            n.key,
            n.tune,
            gain,
            n.vel,
            false,
            n.end as f64,
        );
        return;
    }
    st.age = st.age.wrapping_add(1);
    let base = ch * NOTE_VOICES;
    let poly = poly_patch(inst);
    let mut steal = None;
    let mut from = 0.0;
    if let Some(pp) = poly {
        let key = (n.key as i32 + inst.transpose as i32).clamp(1, 127) as u8;
        let freq = note_freq(key) * 2f32.powf(n.tune / 12.0);
        from = std::mem::replace(&mut st.last_f[ch], freq);
        let pool = st.v[base..base + NOTE_VOICES]
            .iter()
            .map(|v| (v.active && !v.plug, v.down, v.age, v.instr));
        match poly_pick(pp, idx, pool) {
            PolyPick::Reuse(k, retrigger) => {
                let v = &mut st.v[base + k];
                poly_reuse(&mut v.synth, n.vel, retrigger);
                v.freq = freq;
                v.vol = inst.volume;
                v.down = true;
                v.off_at = n.end as f64;
                v.age = st.age;
                return;
            }
            PolyPick::Steal(k) => steal = Some(k),
            PolyPick::Free => {}
        }
    }
    let pool = &st.v[base..base + NOTE_VOICES];
    let slot = steal
        .or_else(|| pool.iter().position(|v| !v.active))
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
    if st.v[slot].active && st.v[slot].plug {
        plugins.live_off(PLUG_TAG + slot as u16);
    }
    let key = (n.key as i32 + inst.transpose as i32).clamp(1, 127) as u8;
    let mut vol = inst.volume * n.vel;
    // Drum Kit: the pad's velocity depth, and its choke group cuts the
    // group's other voices on this track
    if let Some(k) = crate::drumkit::kit_of(inst) {
        let f = note_freq(key) * 2f32.powf(n.tune / 12.0);
        let (g, grp) = crate::drumkit::hit(k, f, n.vel);
        vol = inst.volume * g;
        if grp > 0 {
            for v in st.v[base..base + NOTE_VOICES].iter_mut() {
                if v.active
                    && !v.plug
                    && v.instr == idx
                    && crate::drumkit::choke_of(k, v.freq) == grp
                {
                    v.down = false;
                    v.synth.amp.release();
                    v.synth.fenv.release();
                }
            }
        }
    }
    let plug = matches!(inst.kind, InstrumentKind::Plugin(_));
    if plug && !plugins.live_on(idx, ch, PLUG_TAG + slot as u16, midi_key(key), vol) {
        return;
    }
    let v = &mut st.v[slot];
    v.sw_stolen();
    if crate::strings::rack::patch(song, idx).is_some() {
        v.sw_start(idx, midi_key(key), n.vel);
    }
    v.instr = idx;
    v.freq = note_freq(key) * 2f32.powf(n.tune / 12.0);
    v.vol = vol;
    v.synth = SynthVoice::new();
    v.synth.gen.hit = n.hit;
    if poly.is_some() {
        // velocity goes to the synth (Vel > amp), not the voice volume
        v.vol = inst.volume;
        v.synth = poly_voice(n.vel, from, st.age.wrapping_mul(0x9E37_79B9));
    }
    if let InstrumentKind::Voice(vp) = &inst.kind {
        if let Some(w) = vp.words.first() {
            v.synth.vox.speak(w);
        }
    }
    v.pos = 0.0;
    v.active = true;
    v.down = true;
    v.off_at = n.end as f64;
    v.age = st.age;
    v.plug = plug;
    st.any = true;
}

/// Note-ons and note-offs due at timeline tick `t` (once per sample).
#[allow(clippy::too_many_arguments)]
pub(super) fn note_events(
    st: &mut NoteState,
    feed: &mut crate::modulation::NoteFeed,
    plan: &Plan,
    song: &Song,
    chans: &mut [Chan],
    plugins: &mut crate::plugin::Rack,
    nch: usize,
    t: f64,
    lc: f64,
) {
    st.clock = t;
    let lmask = st.lmask;
    st.layers.events(nch, t, lc, lmask);
    if st.any {
        for (k, v) in st.v[..nch * NOTE_VOICES].iter_mut().enumerate() {
            // launcher notes end on the launch clock
            let now = if lmask != 0 && lmask >> (k / NOTE_VOICES) & 1 == 1 {
                lc
            } else {
                t
            };
            if v.active && v.down && now >= v.off_at {
                v.down = false;
                if v.plug {
                    plugins.live_off(PLUG_TAG + k as u16);
                    v.active = false;
                } else {
                    v.synth.amp.release();
                    v.synth.fenv.release();
                }
            }
        }
    }
    for ch in 0..nch {
        if lmask != 0 && lmask >> ch & 1 == 1 {
            launch_events(st, feed, plan, song, chans, plugins, ch, lc);
            continue;
        }
        let Some(spans) = plan.notes.get(ch) else {
            continue;
        };
        while let Some(n) = spans.get(st.cur[ch]) {
            if n.start as f64 > t {
                break;
            }
            st.cur[ch] += 1;
            if (n.end as f64) > t {
                note_on(st, feed, song, chans, plugins, ch, n);
            }
        }
    }
}

/// Note-ons of the launcher clip playing on track `ch` at launch-clock
/// tick `lc`: the clip loops from its origin, pass after pass.
#[allow(clippy::too_many_arguments)]
fn launch_events(
    st: &mut NoteState,
    feed: &mut crate::modulation::NoteFeed,
    plan: &Plan,
    song: &Song,
    chans: &mut [Chan],
    plugins: &mut crate::plugin::Rack,
    ch: usize,
    lc: f64,
) {
    let mut lp = st.lp[ch];
    let idx = match plan.launch.get(lp.idx) {
        Some(x) if x.id == lp.id => lp.idx,
        _ => match plan.launch.iter().position(|x| x.id == lp.id) {
            Some(i) => i,
            None => return,
        },
    };
    lp.idx = idx;
    let ln = &plan.launch[idx];
    let rel = lc - lp.origin as f64;
    if ln.len <= 0 || rel < 0.0 {
        st.lp[ch] = lp;
        return;
    }
    let pass = (rel / ln.len as f64).floor() as i64;
    if pass != lp.pass {
        lp.pass = pass;
        lp.cur = 0;
    }
    let base = lp.origin + pass * ln.len;
    let local = lc - base as f64;
    while let Some(n) = ln.notes.get(lp.cur) {
        if n.start as f64 > local {
            break;
        }
        lp.cur += 1;
        if (n.end as f64) > local {
            // the hit number keeps counting across the cycle's repeats
            let per = ln.notes.iter().filter(|x| x.key == n.key).count() as u32;
            let m = NoteSpan {
                start: base + n.start,
                end: base + n.end,
                hit: n.hit.wrapping_add((pass as u32).wrapping_mul(per)),
                ..*n
            };
            note_on(st, feed, song, chans, plugins, ch, &m);
        }
    }
    st.lp[ch] = lp;
}

/// Seek / start at tick `t`: notes that started before `t` and are still
/// held are struck again when their instrument sustains (as the tracker
/// chase does for held notes); decaying ones (drums, plucks) are not.
#[allow(clippy::too_many_arguments)]
pub(super) fn restrike(
    st: &mut NoteState,
    feed: &mut crate::modulation::NoteFeed,
    plan: &Plan,
    song: &Song,
    chans: &mut [Chan],
    plugins: &mut crate::plugin::Rack,
    nch: usize,
    t: f64,
) {
    for ch in 0..nch {
        let Some(spans) = plan.notes.get(ch) else {
            continue;
        };
        let upto = st.cur[ch].min(spans.len());
        for n in &spans[..upto] {
            if (n.start as f64) < t && (n.end as f64) > t {
                let idx = if n.instr > 0 {
                    n.instr as usize - 1
                } else if chans[ch].instr != usize::MAX {
                    chans[ch].instr
                } else {
                    0
                };
                let sustains = song.instruments.get(idx).is_some_and(|i| {
                    matches!(i.kind, InstrumentKind::Plugin(_))
                        || crate::strings::rack::patch(song, idx).is_some()
                        || i.kind.patch().is_some_and(|p| p.amp.s >= 0.05)
                });
                if sustains {
                    note_on(st, feed, song, chans, plugins, ch, n);
                }
            }
        }
    }
}

impl Engine {
    /// the timeline position in ticks, inside the current row (row clock)
    pub(super) fn tl_clock(&self) -> f64 {
        let tl = self.tick_len().max(1.0);
        let frac = (1.0 - self.samples_to_tick / tl).clamp(0.0, 1.0);
        let ph = ((self.tick as f64 + frac) / self.speed.max(1) as f64).clamp(0.0, 1.0);
        (self.tl.row as f64 + ph) * ROW as f64
    }

    /// the launch clock (Stage 4b): `tl_clock` on the rows played since the
    /// start instead of the timeline row, so it never jumps back when the
    /// arrangement loops
    pub(super) fn launch_clock(&self) -> f64 {
        self.tl_clock() + (self.rows_played as f64 - self.tl.row as f64) * ROW as f64
    }

    /// timeline position in ticks while the timeline plays (recording)
    pub fn tl_ticks(&self) -> Option<f64> {
        (self.tl.on && self.playing).then(|| self.tl_clock())
    }

    /// note voices: release, and find the notes from timeline row `row`
    pub(super) fn notes_jump(&mut self, song: &Song, row: usize, strike: bool) {
        // launcher clips play on through a jump of the arrangement
        let keep = self.nv.lmask;
        self.nv.release_where(&mut self.plugins, !keep);
        let Some(plan) = self.tl.plan.clone() else {
            return;
        };
        let t = (row as i64 * ROW) as f64;
        self.nv.seek(&plan, t);
        if plan.has_audio() || self.av.any {
            let (sr, playing) = (self.sr, self.playing && self.tl.on);
            self.av.seek(&plan, t, sr, playing, false);
        }
        if strike && plan.has_notes() {
            let nch = song.channels.min(MAX_CHANNELS);
            restrike(
                &mut self.nv,
                &mut self.mods.feed,
                &plan,
                song,
                &mut self.chans,
                &mut self.plugins,
                nch,
                t,
            );
        }
    }

    /// sounding audio-clip voices on `track` (tests)
    #[cfg(test)]
    pub fn audio_voices(&self, track: usize) -> usize {
        self.av.sounding(track)
    }

    /// sounding note-clip voices on `track` (tests)
    #[cfg(test)]
    pub fn note_voices(&self, track: usize) -> usize {
        self.nv.sounding(track)
    }
}
