//! Note-clip playback (M3b, docs/TIMELINE.md "Note clips"): a polyphonic
//! voice pool per track, fed by the plan's note spans at sample accuracy
//! on the row clock. Only runs when the timeline has note clips, so
//! tracker songs and block-only timelines render exactly as before.
use super::*;
use crate::timeline::{NoteSpan, Plan, ROW};

/// voices per track for note clips
pub const NOTE_VOICES: usize = 8;
/// plugin live-note tags for note voices (MIDI keys use 0..=255)
const PLUG_TAG: u16 = 0x100;

#[derive(Clone)]
pub(super) struct NoteVoice {
    instr: usize,
    freq: f32,
    vol: f32,
    synth: SynthVoice,
    pos: f64,
    pub(super) active: bool,
    /// still before its note-off
    down: bool,
    /// timeline tick of the note-off
    off_at: f64,
    age: u32,
    /// the note plays on a plugin instrument (no synth voice)
    plug: bool,
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
        if self.plug {
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

    fn new() -> Self {
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
}

impl NoteState {
    pub(super) fn new() -> Self {
        Self {
            v: vec![NoteVoice::new(); MAX_CHANNELS * NOTE_VOICES],
            cur: [0; MAX_CHANNELS],
            clock: 0.0,
            age: 0,
            any: false,
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
    }

    /// release every held note (synth voices fade with their envelope)
    pub(super) fn release_all(&mut self, plugins: &mut crate::plugin::Rack) {
        for (k, v) in self.v.iter_mut().enumerate() {
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
        for v in &mut self.v {
            v.active = false;
            v.down = false;
        }
        self.any = false;
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
        for v in &self.v {
            if v.active && v.down && v.instr < 256 {
                held[v.instr] = true;
            }
        }
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
    st.age = st.age.wrapping_add(1);
    let base = ch * NOTE_VOICES;
    let pool = &st.v[base..base + NOTE_VOICES];
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
    v.instr = idx;
    v.freq = note_freq(key) * 2f32.powf(n.tune / 12.0);
    v.vol = vol;
    v.synth = SynthVoice::new();
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
) {
    st.clock = t;
    if st.any {
        for (k, v) in st.v[..nch * NOTE_VOICES].iter_mut().enumerate() {
            if v.active && v.down && t >= v.off_at {
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

    /// timeline position in ticks while the timeline plays (recording)
    pub fn tl_ticks(&self) -> Option<f64> {
        (self.tl.on && self.playing).then(|| self.tl_clock())
    }

    /// note voices: release, and find the notes from timeline row `row`
    pub(super) fn notes_jump(&mut self, song: &Song, row: usize, strike: bool) {
        self.nv.release_all(&mut self.plugins);
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
