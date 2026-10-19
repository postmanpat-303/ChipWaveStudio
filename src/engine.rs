//! Tracker playback engine: rows/ticks, voices, a few effect commands.
//! Same code path for real-time audio and offline WAV export.
use crate::model::*;
use crate::synth::{Adsr, SynthVoice};

const SAMPLE_ENV: Adsr = Adsr {
    a: 0.0,
    d: 0.0,
    s: 1.0,
    r: 0.03,
};

#[derive(Clone)]
struct Chan {
    instr: usize, // index into song.instruments, usize::MAX = none
    note: u8,
    freq: f32,
    vol: f32,
    synth: SynthVoice,
    pos: f64, // sample playback position
    active: bool,
    slide: f32,
    vol_slide: f32,
    /// smoothed left/right gains (mixer fader, pan, mute; declick)
    gl: f32,
    gr: f32,
    /// previous voice fading out over ~5 ms after a retrigger (declick)
    tail: Tail,
    /// beat-repeat (effect 33 / Voice stutter), before the insert chain
    stut: crate::voice::Stutter,
    /// 34xy pitch step override for this note's stutter
    stut_semi: Option<f32>,
    /// next word a Voice note on this channel speaks (cycles the list)
    word_next: usize,
    /// a word was started on this row (3E then does not restart it)
    spoke: bool,
    /// pattern filter overrides (38-3B); cleared by a different instrument
    fm: crate::synth::FiltMod,
    /// Transform overrides 3F-42 (blend, resonance, digital, sub; < 0 =
    /// instrument); cleared by a different instrument
    tfo: crate::transform::Overrides,
    /// Generator macro overrides from 43xy (-1 = the instrument's value)
    gmo: [f32; 8],
    /// cutoff slide in octaves per tick (3A / 3B), this row only
    cut_slide: f32,
    /// note held (no note-off since it started): legato glide is possible
    gate: bool,
    /// glide target frequency (0 = none) and per-sample log step
    glide_to: f32,
    glide_k: f32,
}

#[derive(Clone)]
struct Tail {
    instr: usize,
    freq: f32,
    vol: f32,
    synth: SynthVoice,
    pos: f64,
    gain: f32,
}

/// 3F-42 value: 00 = the instrument's own setting (-1), 01-FF = 0..1
pub fn tf_amount(x: u8) -> f32 {
    if x == 0 {
        -1.0
    } else {
        (x - 1) as f32 / 254.0
    }
}

/// Effect 38xx: code 01..FF -> 20 Hz .. 20 kHz on a log scale.
pub fn cutoff_from_code(x: u8) -> f32 {
    20.0 * 1000f32.powf((x.max(1) - 1) as f32 / 254.0)
}

/// Render one voice sample; returns (sample, still_active).
/// sidechain envelope 0..1 at `t` seconds after the key note (1 = fully ducked)
fn duck_env(t: f32, release: f32) -> f32 {
    const ATTACK: f32 = 0.002;
    const HOLD: f32 = 0.012;
    if t < ATTACK {
        t / ATTACK
    } else if t < HOLD {
        1.0
    } else {
        let x = ((t - HOLD) / release.max(0.02)).min(1.0);
        (1.0 - x) * (1.0 - x)
    }
}

/// tracker note (C-0 = 1) -> MIDI key (C-0 = 12)
fn midi_key(n: u8) -> u8 {
    (n as i32 + 11).clamp(0, 127) as u8
}

#[allow(clippy::too_many_arguments)]
fn voice(
    song: &Song,
    instr: usize,
    freq: f32,
    synth: &mut SynthVoice,
    pos: &mut f64,
    sr: f32,
    ov: &AutoState,
    bpm: f32,
) -> ((f32, f32), bool) {
    let Some(inst) = song.instruments.get(instr) else {
        return ((0.0, 0.0), false);
    };
    let lvl = ov.level.get(instr).copied().unwrap_or(1.0);
    let ((l, r), alive) = voice_raw(song, inst, instr, freq, synth, pos, sr, ov, bpm);
    ((l * lvl, r * lvl), alive)
}

#[allow(clippy::too_many_arguments)]
fn voice_raw(
    _song: &Song,
    inst: &Instrument,
    instr: usize,
    freq: f32,
    synth: &mut SynthVoice,
    pos: &mut f64,
    sr: f32,
    ov: &AutoState,
    bpm: f32,
) -> ((f32, f32), bool) {
    match &inst.kind {
        InstrumentKind::Synth(p) => {
            let p = ov.patches.get(instr).and_then(|o| o.as_ref()).unwrap_or(p);
            let v = synth.render_st(p, freq, sr, bpm);
            (v, !synth.amp.done())
        }
        InstrumentKind::Voice(vp) => {
            let p = ov
                .patches
                .get(instr)
                .and_then(|o| o.as_ref())
                .unwrap_or(&vp.base);
            let v = synth.render_voice(p, vp, freq, sr, bpm);
            (v, !synth.amp.done())
        }
        InstrumentKind::Gen(gp) => {
            let p = ov
                .patches
                .get(instr)
                .and_then(|o| o.as_ref())
                .unwrap_or(&gp.base);
            let ko = ov.gen.get(instr).unwrap_or(&crate::gen::NO_KNOBS);
            let v = synth.render_gen_ko(p, gp, ko, freq, sr, bpm);
            (v, !synth.amp.done() && !synth.gen.done)
        }
        // plugin instruments render in the rack (Engine::render); the channel
        // voice only keeps the note state
        InstrumentKind::Plugin(_) => ((0.0, 0.0), true),
        InstrumentKind::Sample {
            data,
            rate,
            base_note,
            lp,
            ..
        } => {
            // `pos` is monotonic; the loop maps it into the data
            let Some(p) = crate::sample::read_pos(*pos, data.len(), lp) else {
                return ((0.0, 0.0), false);
            };
            let v = crate::sample::read(data, p, lp);
            let ft = 2f32.powf(lp.finetune as f32 / 1200.0);
            *pos += (freq * ft / note_freq(*base_note)) as f64 * *rate as f64 / sr as f64;
            let g = synth.amp.next(&SAMPLE_ENV, 1.0 / sr);
            ((v * g, v * g), !synth.amp.done())
        }
    }
}

impl Chan {
    /// Move the sounding voice into the tail slot so it fades out (no click).
    fn fade_to_tail(&mut self) {
        self.tail = Tail {
            instr: self.instr,
            freq: self.freq,
            vol: self.vol,
            synth: self.synth.clone(),
            pos: self.pos,
            gain: 1.0,
        };
        self.active = false;
    }

    fn new(sr: f32) -> Self {
        Self {
            stut: crate::voice::Stutter::new(sr as usize, (sr * 0.002) as usize),
            stut_semi: None,
            word_next: 0,
            spoke: false,
            fm: Default::default(),
            tfo: crate::transform::NO_OVERRIDES,
            gmo: [-1.0; 8],
            cut_slide: 0.0,
            gate: false,
            glide_to: 0.0,
            glide_k: 0.0,
            instr: usize::MAX,
            note: 0,
            freq: 0.0,
            vol: 1.0,
            synth: SynthVoice::new(),
            pos: 0.0,
            active: false,
            slide: 0.0,
            vol_slide: 0.0,
            gl: 1.0,
            gr: 1.0,
            tail: Tail {
                instr: usize::MAX,
                freq: 0.0,
                vol: 0.0,
                synth: SynthVoice::new(),
                pos: 0.0,
                gain: 0.0,
            },
        }
    }
}

mod audioclips;
mod notes;
#[cfg(test)]
pub use notes::NOTE_VOICES;

/// voices for live MIDI play (separate from the pattern channels)
pub const LIVE_VOICES: usize = 12;

#[derive(Clone)]
struct LiveVoice {
    key: u8,
    instr: usize,
    freq: f32,
    vol: f32,
    synth: SynthVoice,
    pos: f64,
    active: bool,
    /// key still held down
    down: bool,
    /// key released while the sustain pedal was down
    held_by_pedal: bool,
    age: u32,
}

/// Live MIDI input: a small polyphonic voice pool that plays through the
/// current instrument's FX chain and the cursor channel's fader, pan and
/// sends (not its mute, so jamming on a muted channel is still heard).
struct Live {
    rx: Option<rtrb::Consumer<crate::midi::LiveEv>>,
    v: Vec<LiveVoice>,
    chain: crate::fx::ChainState,
    /// instrument whose FX chain the bus uses (latest note), usize::MAX = none
    owner: usize,
    ch: usize,
    sustain: bool,
    /// pitch bend as a frequency ratio
    bend: f32,
    age: u32,
}

impl Live {
    fn new() -> Self {
        let v = LiveVoice {
            key: 0,
            instr: usize::MAX,
            freq: 0.0,
            vol: 0.0,
            synth: SynthVoice::new(),
            pos: 0.0,
            active: false,
            down: false,
            held_by_pedal: false,
            age: 0,
        };
        Self {
            rx: None,
            v: vec![v; LIVE_VOICES],
            chain: Default::default(),
            owner: usize::MAX,
            ch: 0,
            sustain: false,
            bend: 1.0,
            age: 0,
        }
    }
}

/// Automation overrides computed by the engine (never touches the song).
#[derive(Default)]
pub struct AutoState {
    /// per-instrument patch copies with automated params (None = not automated)
    patches: Vec<Option<crate::synth::Patch>>,
    /// per-instrument amp level multiplier
    level: Vec<f32>,
    /// smoothed current normalised value per lane (NaN = not yet set)
    vals: Vec<f32>,
    chan_vol: [Option<f32>; MAX_CHANNELS],
    chan_pan: [Option<f32>; MAX_CHANNELS],
    master: Option<f32>,
    countdown: u32,
    /// automated FX params: (instrument, device, param, normalised value).
    /// Capacity reserved up front; never grows past it in the audio path.
    fx: Vec<(usize, usize, usize, f32)>,
    /// automated return-bus params (bus, param, value)
    ret: Vec<(usize, usize, f32)>,
    /// automated plugin macros (instrument, macro 0-7, normalised value)
    macros: Vec<(usize, usize, f32)>,
    /// automated generator knobs per instrument (`< 0` = patch value);
    /// resized only when the instrument count changes
    gen: Vec<[f32; crate::gen::GEN_KNOBS]>,
    /// instruments with an automated generator knob in `gen`
    gen_on: Vec<bool>,
    send: [[Option<f32>; 2]; MAX_CHANNELS],
    /// parsed lane targets: (target string it was parsed from, result).
    /// Re-parsed only when a lane's target text changes.
    targets: Vec<(String, Option<crate::automation::Target>)>,
    /// this evaluation's driven values (target, normalised), lanes first in
    /// lane order, then modulated targets. Capacity reserved up front.
    pend: Vec<(crate::automation::Target, f32)>,
    /// route depths set by `mod.K.amount` lanes this evaluation (NaN = the
    /// route's own amount); MAX_ROUTES long, allocated once
    mod_amt: Vec<f32>,
}

const FX_OVER_CAP: usize = 64;
const PEND_CAP: usize = 512;

pub struct Engine {
    pub sr: f32,
    pub playing: bool,
    /// play only the current pattern (loop) instead of the order list
    pub loop_pattern: bool,
    pub order_pos: usize,
    pub row: usize,
    pub pattern: usize,
    tick: u32,
    samples_to_tick: f64,
    /// per-trigger noise seed (LCG)
    seed: u32,
    speed: u32,
    bpm: f32,
    /// seconds since the last note on the sidechain key channel
    duck_t: f32,
    chans: Vec<Chan>,
    pub song_ended: bool,
    pub peak: f32,
    /// master peaks (left, right) with decay, for meters
    pub peak_lr: [f32; 2],
    /// per-channel peak after fader, for mixer meters
    pub chan_peak: [f32; MAX_CHANNELS],
    /// rows played since start (tests / diagnostics)
    pub rows_played: u64,
    auto: AutoState,
    /// modulators (LFOs, envelopes, ...) evaluated with the automation;
    /// `mods.out` is the live value per modulator for the UI
    pub mods: crate::modulation::ModEngine,
    /// current song position in lines (for the UI playhead of lanes)
    pub song_pos: f32,
    /// per-channel insert chain (follows the channel's current instrument)
    chains: Vec<crate::fx::ChainState>,
    /// return buses A (delay) and B (reverb)
    rets: Vec<crate::fx::ChainState>,
    /// smoothed sends per channel
    sends: [[f32; 2]; MAX_CHANNELS],
    /// frames of 440 Hz test tone left (settings > audio)
    pub test_tone: u32,
    tone_ph: f32,
    live: Live,
    /// master true-peak limiter and loudness meter (src/master.rs)
    lim: crate::master::Limiter,
    meter: crate::master::Loudness,
    /// loudness / true peak / gain reduction for the UI
    pub loud: crate::master::Readout,
    /// plugin instrument instances (src/plugin/), one per instrument x channel
    pub plugins: crate::plugin::Rack,
    /// plugin effects in the FX chains (crate::plugin::fxrack)
    pub fx_plugins: crate::plugin::fxrack::FxRack,
    /// panic request (any thread sets it; the audio thread acts on it)
    pub panic_req: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// panic fade-out frames left / total
    panic_fade: u32,
    panic_len: u32,
    /// timeline-mode transport (docs/TIMELINE.md)
    tl: TlState,
    /// note-clip voices (engine/notes.rs)
    nv: notes::NoteState,
    /// audio-clip voices (timeline mode, M4)
    av: audioclips::AudioState,
}

/// Timeline-mode playback: the plan being played, the row position and the
/// per-track cursors. Row boundaries stay global (every channel steps on the
/// same row clock), so every effect command keeps its tracker meaning.
struct TlState {
    /// playing the song's timeline (not the order list / a block loop)
    on: bool,
    plan: Option<std::sync::Arc<crate::timeline::Plan>>,
    /// timeline row being played
    row: usize,
    /// per track: index of the current segment in the plan
    cur: [usize; MAX_CHANNELS],
    /// per track: segment the `limit` belongs to (usize::MAX = none)
    seg_of: [usize; MAX_CHANNELS],
    /// per track: rows of the segment's block that play (pattern break)
    limit: [usize; MAX_CHANNELS],
    /// per track with its own speed: effect steps done in this row
    steps: [u8; MAX_CHANNELS],
}

impl Default for TlState {
    fn default() -> Self {
        Self {
            on: false,
            plan: None,
            row: 0,
            cur: [0; MAX_CHANNELS],
            seg_of: [usize::MAX; MAX_CHANNELS],
            limit: [0; MAX_CHANNELS],
            steps: [0; MAX_CHANNELS],
        }
    }
}

impl Engine {
    pub fn new(sr: f32) -> Self {
        Self {
            sr,
            playing: false,
            loop_pattern: false,
            order_pos: 0,
            row: 0,
            pattern: 0,
            tick: 0,
            samples_to_tick: 0.0,
            seed: 0x2545_F491,
            speed: 6,
            bpm: 125.0,
            duck_t: 1e9,
            chans: (0..MAX_CHANNELS).map(|_| Chan::new(sr)).collect(),
            song_ended: false,
            peak: 0.0,
            peak_lr: [0.0; 2],
            chan_peak: [0.0; MAX_CHANNELS],
            rows_played: 0,
            auto: AutoState {
                fx: Vec::with_capacity(FX_OVER_CAP),
                ret: Vec::with_capacity(FX_OVER_CAP),
                macros: Vec::with_capacity(FX_OVER_CAP),
                pend: Vec::with_capacity(PEND_CAP),
                mod_amt: vec![f32::NAN; crate::modulation::MAX_ROUTES],
                ..Default::default()
            },
            mods: Default::default(),
            song_pos: 0.0,
            chains: (0..MAX_CHANNELS).map(|_| Default::default()).collect(),
            rets: (0..2).map(|_| Default::default()).collect(),
            sends: [[0.0; 2]; MAX_CHANNELS],
            test_tone: 0,
            tone_ph: 0.0,
            live: Live::new(),
            plugins: crate::plugin::Rack::new(),
            fx_plugins: crate::plugin::fxrack::FxRack::new(),
            panic_req: Default::default(),
            panic_fade: 0,
            panic_len: 1,
            lim: crate::master::Limiter::new(sr),
            meter: crate::master::Loudness::new(sr),
            loud: Default::default(),
            tl: TlState::default(),
            nv: notes::NoteState::new(),
            av: audioclips::AudioState::new(),
        }
    }

    /// Connect the live MIDI queue (drained at the start of every render).
    pub fn set_live_input(&mut self, rx: Option<rtrb::Consumer<crate::midi::LiveEv>>) {
        self.live.rx = rx;
    }

    /// Take the queue out (kept when the engine is replaced, e.g. device change).
    pub fn take_live_input(&mut self) -> Option<rtrb::Consumer<crate::midi::LiveEv>> {
        self.live.rx.take()
    }

    /// number of sounding live (MIDI) voices
    pub fn live_voices(&self) -> usize {
        self.live.v.iter().filter(|v| v.active).count()
    }

    /// Apply one live MIDI event (allocation-free).
    pub fn live_event(&mut self, song: &Song, ev: crate::midi::LiveEv) {
        use crate::midi::LiveEv;
        if let LiveEv::On {
            note, vel, instr, ..
        } = ev
        {
            self.mods
                .feed
                .note_on((instr as usize).wrapping_sub(1), note, vel);
        }
        if !self.plugins.is_empty() {
            match ev {
                LiveEv::On {
                    key,
                    note,
                    vel,
                    instr,
                    ch,
                } => {
                    let i = (instr as usize).wrapping_sub(1);
                    if let Some(inst) = song.instruments.get(i) {
                        if matches!(inst.kind, InstrumentKind::Plugin(_)) {
                            let n = (note as i32 + inst.transpose as i32).clamp(1, 127) as u8;
                            self.plugins.live_on(
                                i,
                                ch as usize,
                                key as u16,
                                midi_key(n),
                                inst.volume * vel,
                            );
                            return;
                        }
                    }
                }
                LiveEv::Off { key } => self.plugins.live_off(key as u16),
                LiveEv::AllOff => self.plugins.all_off(),
                _ => {}
            }
        }
        let lv = &mut self.live;
        match ev {
            LiveEv::On {
                key,
                note,
                vel,
                instr,
                ch,
            } => {
                let i = (instr as usize).wrapping_sub(1);
                let Some(inst) = song.instruments.get(i) else {
                    return;
                };
                lv.age = lv.age.wrapping_add(1);
                // same key again: retrigger its voice; else a free voice; else
                // steal the oldest released voice, else the oldest one
                let slot =
                    lv.v.iter()
                        .position(|v| v.active && v.key == key)
                        .or_else(|| lv.v.iter().position(|v| !v.active))
                        .or_else(|| {
                            let oldest = |only_released: bool| {
                                lv.v.iter()
                                    .enumerate()
                                    .filter(|(_, v)| !only_released || !v.down)
                                    .min_by_key(|(_, v)| v.age)
                                    .map(|(k, _)| k)
                            };
                            oldest(true).or_else(|| oldest(false))
                        })
                        .unwrap_or(0);
                let n = (note as i32 + inst.transpose as i32).clamp(1, 127) as u8;
                let v = &mut lv.v[slot];
                *v = LiveVoice {
                    key,
                    instr: i,
                    freq: note_freq(n),
                    vol: inst.volume * vel,
                    synth: {
                        let mut sv = SynthVoice::new();
                        if let InstrumentKind::Voice(vp) = &inst.kind {
                            if let Some(w) = vp.words.first() {
                                sv.vox.speak(w);
                            }
                        }
                        sv
                    },
                    pos: 0.0,
                    active: true,
                    down: true,
                    held_by_pedal: false,
                    age: lv.age,
                };
                lv.owner = i;
                lv.ch = ch as usize;
            }
            LiveEv::Off { key } => {
                for v in
                    lv.v.iter_mut()
                        .filter(|v| v.active && v.down && v.key == key)
                {
                    v.down = false;
                    if lv.sustain {
                        v.held_by_pedal = true;
                    } else {
                        v.synth.amp.release();
                        v.synth.fenv.release();
                    }
                }
            }
            LiveEv::Sustain(on) => {
                lv.sustain = on;
                if !on {
                    for v in lv.v.iter_mut().filter(|v| v.held_by_pedal) {
                        v.held_by_pedal = false;
                        v.synth.amp.release();
                        v.synth.fenv.release();
                    }
                }
            }
            LiveEv::Bend(semi) => lv.bend = 2f32.powf(semi.clamp(-24.0, 24.0) / 12.0),
            LiveEv::AllOff => {
                lv.sustain = false;
                lv.bend = 1.0;
                for v in lv.v.iter_mut() {
                    v.down = false;
                    v.held_by_pedal = false;
                    v.synth.amp.release();
                    v.synth.fenv.release();
                }
            }
        }
    }

    /// Position inside the current row, 0..1 (live recording quantise).
    pub fn row_phase(&self) -> f32 {
        let tl = self.tick_len().max(1.0);
        let tick_frac = (1.0 - self.samples_to_tick / tl).clamp(0.0, 1.0);
        ((self.tick as f32 + tick_frac as f32) / self.speed.max(1) as f32).clamp(0.0, 1.0)
    }

    /// length of one row in seconds at the running tempo
    pub fn row_secs(&self) -> f32 {
        (self.tick_len() * self.speed as f64 / self.sr as f64) as f32
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Instruments were removed/inserted in the song (delete, duplicate,
    /// undo): map every voice, fading tail and FX chain to the instrument's
    /// new index. Voices of a removed instrument stop.
    pub fn remap_instruments(&mut self, f: &dyn Fn(usize) -> Option<usize>) {
        drop(self.plugins.remap(f));
        drop(self.fx_plugins.remap(f));
        for c in &mut self.chans {
            if c.instr != usize::MAX {
                match f(c.instr) {
                    Some(i) => c.instr = i,
                    None => {
                        c.instr = usize::MAX;
                        c.active = false;
                    }
                }
            }
            if c.tail.instr != usize::MAX {
                match f(c.tail.instr) {
                    Some(i) => c.tail.instr = i,
                    None => {
                        c.tail.instr = usize::MAX;
                        c.tail.gain = 0.0;
                    }
                }
            }
        }
        for ch in &mut self.chains {
            ch.remap(f);
        }
        for v in &mut self.live.v {
            match f(v.instr) {
                Some(i) => v.instr = i,
                None => {
                    v.instr = usize::MAX;
                    v.active = false;
                }
            }
        }
        if self.live.owner != usize::MAX {
            self.live.owner = f(self.live.owner).unwrap_or(usize::MAX);
        }
        self.live.chain.remap(f);
        self.auto.fx.clear();
        self.auto.countdown = 0;
    }

    /// Change tempo/speed while playing: takes effect at the next tick
    /// (no restart, the current row position is kept).
    pub fn set_tempo(&mut self, bpm: f32, speed: u32) {
        let old = self.tick_len();
        self.bpm = bpm.clamp(20.0, 400.0);
        self.speed = speed.clamp(1, 32);
        if self.tick >= self.speed {
            self.tick = self.speed - 1;
        }
        // rescale the remaining time of the current tick
        let new = self.tick_len();
        if old > 0.0 {
            self.samples_to_tick = (self.samples_to_tick / old * new).min(new);
        }
    }

    pub fn start(
        &mut self,
        song: &Song,
        order_pos: usize,
        row: usize,
        loop_pattern: bool,
        pattern: usize,
    ) {
        self.playing = true;
        // audio clips still fading from the last play end now
        self.av.release_all();
        self.loop_pattern = loop_pattern;
        self.tl.on = !loop_pattern && song.timeline.is_some();
        if self.tl.on {
            self.ensure_plan(song);
        }
        self.order_pos = order_pos.min(song.order.len().saturating_sub(1));
        self.pattern = if loop_pattern {
            pattern
        } else {
            song.order.get(self.order_pos).copied().unwrap_or(0)
        };
        self.row = row;
        self.rows_played = 0;
        self.tick = 0;
        self.speed = song.speed.max(1);
        self.bpm = song.bpm;
        // the start row is processed now; the next tick is one full tick away
        // (was 0: the first tick fired on sample 0, so row 1 came a tick early)
        self.samples_to_tick = self.tick_len();
        self.song_ended = false;
        // integrated loudness and the true-peak hold measure from here
        self.meter.reset();
        // modulators restart with the song (LFO phases, envelopes, random
        // seeds): a render is the same every time
        self.mods.reset();
        // silent channels take their mixer gain at once (a muted or quiet
        // channel no longer blips during the 5 ms ramp from unity at start)
        let (tgt, _) = self.mix_targets(song);
        for (c, t) in self.chans.iter_mut().zip(tgt) {
            if !c.active && c.tail.gain == 0.0 {
                (c.gl, c.gr) = t;
            }
        }
        for c in &mut self.chans {
            if c.active {
                c.fade_to_tail();
            }
        }
        if self.tl.on {
            // timeline mode: `row` is the timeline row (order_pos unused)
            self.tl_jump(song, row, true);
        }
        self.process_row(song);
    }

    /// Timeline mode: (re)build the playback plan when the timeline changed
    /// since the last call. Called on the UI thread (start, seek, every
    /// frame), never by the audio thread, which only reads the plan.
    pub fn ensure_plan(&mut self, song: &Song) {
        match &song.timeline {
            Some(tl) => {
                let key = crate::timeline::plan_key(song, tl);
                if self.tl.plan.as_ref().is_none_or(|p| p.key != key) {
                    self.tl.plan = Some(std::sync::Arc::new(crate::timeline::build_plan(song, tl)));
                    // cursors are re-found against the new segments
                    let row = self.tl.row;
                    self.tl_cursors(row);
                    if let Some(p) = self.tl.plan.clone() {
                        let t = if self.playing {
                            self.nv.clock
                        } else {
                            (row as i64 * crate::timeline::ROW) as f64
                        };
                        self.nv.seek(&p, t);
                        // audio voices of unchanged clips play on
                        let live = self.playing && self.tl.on && self.av.live;
                        let at = if live { self.av.clock } else { t };
                        let sr = self.sr;
                        self.av.seek(&p, at, sr, live, true);
                    }
                }
            }
            None => {
                self.tl.plan = None;
                if self.tl.on {
                    self.tl.on = false;
                    self.playing = false;
                }
            }
        }
    }

    /// The timeline row being played (timeline mode only).
    pub fn tl_row(&self) -> Option<usize> {
        (self.tl.on && self.playing).then_some(self.tl.row)
    }

    /// put every track's cursor on `row` (binary search, no allocation)
    fn tl_cursors(&mut self, row: usize) {
        let Some(plan) = self.tl.plan.as_deref() else {
            return;
        };
        for ch in 0..MAX_CHANNELS {
            self.tl.cur[ch] = plan.seek(ch, row);
            self.tl.seg_of[ch] = usize::MAX;
        }
    }

    /// Jump to timeline row `to` with state chase: tempo, speed, instrument,
    /// volume and filter / Transform / macro overrides are replayed from the
    /// song start, so playing from anywhere sounds like playing through.
    /// Held notes of sustaining instruments are struck again at `to`;
    /// decaying ones (drums, plucks, samples) are not. Runs on the UI thread.
    fn tl_jump(&mut self, song: &Song, to: usize, chase: bool) {
        let Some(plan) = self.tl.plan.clone() else {
            self.playing = false;
            return;
        };
        let to = to.min(plan.end.saturating_sub(1));
        let nch = song.channels.min(MAX_CHANNELS);
        let mut held = [0u8; MAX_CHANNELS];
        if chase && to > 0 {
            self.speed = song.speed.max(1);
            self.bpm = song.bpm;
            self.tl_cursors(0);
            for row in 0..to {
                self.tl.row = row;
                self.tl_tempo(song, &plan, row);
                for (ch, h) in held.iter_mut().enumerate().take(nch) {
                    let cell = self.tl_cell(song, &plan, ch, row);
                    if cell.note == NOTE_OFF {
                        *h = 0;
                    } else if cell.note > 0 {
                        *h = cell.note;
                    }
                    self.chase_cell(song, ch, &cell);
                }
            }
        }
        self.tl.row = to;
        self.tl_cursors(to);
        for (ch, &note) in held.iter().enumerate().take(nch) {
            let sustains = song
                .instruments
                .get(self.chans[ch].instr)
                .and_then(|i| i.kind.patch())
                .is_some_and(|p| p.amp.s >= 0.05);
            if note > 0 && sustains {
                let cell = Cell {
                    note,
                    ..Default::default()
                };
                self.trigger(song, ch, &cell, None);
            }
        }
        self.notes_jump(song, to, chase);
    }

    /// state-only replay of one cell (chase): no sound, no plugin events
    fn chase_cell(&mut self, song: &Song, ch: usize, cell: &Cell) {
        let c = &mut self.chans[ch];
        if cell.instr > 0 && (cell.instr as usize) <= song.instruments.len() {
            if c.instr != cell.instr as usize - 1 {
                c.fm = Default::default();
                c.tfo = crate::transform::NO_OVERRIDES;
                c.gmo = [-1.0; 8];
            }
            c.instr = cell.instr as usize - 1;
            c.vol = song.instruments[c.instr].volume;
        }
        if cell.vol > 0 {
            c.vol = (cell.vol - 1) as f32 / 64.0;
        }
        match cell.fx {
            0x09 if cell.fxp > 0 => self.speed = cell.fxp as u32,
            0x0C => c.vol = (cell.fxp.min(0x40)) as f32 / 64.0,
            0x0F if cell.fxp > 0 && cell.fxp <= 0xF0 => self.bpm = cell.fxp as f32,
            0x38 => {
                c.fm.cut_oct = 0.0;
                c.fm.cut_abs = if cell.fxp == 0 {
                    0.0
                } else {
                    cutoff_from_code(cell.fxp)
                };
            }
            0x39 => {
                c.fm.res = if cell.fxp == 0 {
                    -1.0
                } else {
                    (cell.fxp - 1) as f32 / 254.0
                };
            }
            0x3F..=0x42 => c.tfo[(cell.fx - 0x3F) as usize] = tf_amount(cell.fxp),
            0x43 => match cell.fxp >> 4 {
                0 => c.gmo = [-1.0; 8],
                x @ 1..=8 => c.gmo[x as usize - 1] = (cell.fxp & 15) as f32 / 15.0,
                _ => {}
            },
            _ => {}
        }
        c.synth.vel = c.vol;
    }

    /// tempo-map changes at timeline row `row` (`0Fxx` in a cell still
    /// overrides until the map changes again)
    fn tl_tempo(&mut self, song: &Song, plan: &crate::timeline::Plan, row: usize) {
        if !plan.tempo_changes {
            return;
        }
        let Some(tl) = &song.timeline else { return };
        let t = crate::timeline::rows_to_ticks(row);
        let cur = tl.bpm_at(t, song.bpm);
        let prev = if row == 0 {
            song.bpm
        } else {
            tl.bpm_at(t - crate::timeline::ROW, song.bpm)
        };
        if cur.to_bits() != prev.to_bits() {
            self.bpm = cur.clamp(20.0, 400.0);
        }
    }

    /// the cell track `ch` plays at timeline row `row` (advances its cursor)
    fn tl_cell(
        &mut self,
        song: &Song,
        plan: &crate::timeline::Plan,
        ch: usize,
        row: usize,
    ) -> Cell {
        let Some(segs) = plan.tracks.get(ch) else {
            return Cell::default();
        };
        let mut k = self.tl.cur[ch];
        while k < segs.len() && segs[k].end <= row {
            k += 1;
        }
        self.tl.cur[ch] = k;
        match segs.get(k) {
            Some(s) if s.start <= row => {
                if self.tl.seg_of[ch] != k {
                    self.tl.seg_of[ch] = k;
                    self.tl.limit[ch] = crate::timeline::source_limit(song, s.pattern);
                }
                crate::timeline::plan_cell(song, s, row, self.tl.limit[ch])
            }
            _ => Cell::default(),
        }
    }

    /// Jump the running playback to `order_pos` / `row` (arranger ruler
    /// click). Keeps playing, the running tempo and speed; sounding notes fade
    /// out. A block loop keeps looping if the target is in the looped block,
    /// otherwise playback switches to the song order. Stopped: does nothing.
    pub fn seek(&mut self, song: &Song, order_pos: usize, row: usize) {
        if !self.playing || song.order.is_empty() {
            return;
        }
        if self.tl.on {
            // timeline mode: `row` is the timeline row; state is chased
            self.ensure_plan(song);
            self.tick = 0;
            self.song_ended = false;
            for c in &mut self.chans {
                if c.active {
                    c.fade_to_tail();
                }
            }
            self.tl_jump(song, row, true);
            self.samples_to_tick = self.tick_len();
            self.process_row(song);
            self.song_pos = self.calc_song_pos(song);
            return;
        }
        let op = order_pos.min(song.order.len() - 1);
        let pat = song.order[op];
        if self.loop_pattern && pat != self.pattern {
            self.loop_pattern = false;
        }
        self.order_pos = op;
        self.pattern = pat;
        let rows = song.patterns.get(pat).map(|p| p.rows).unwrap_or(ROWS);
        self.row = row.min(rows.saturating_sub(1));
        self.tick = 0;
        self.samples_to_tick = self.tick_len();
        self.song_ended = false;
        for c in &mut self.chans {
            if c.active {
                c.fade_to_tail();
            }
        }
        self.process_row(song);
        self.song_pos = self.calc_song_pos(song);
    }

    /// PANIC: stop playback and silence everything (voices, live notes,
    /// plugins, FX tails, limiter) after a 3 ms fade. Only sets a flag: the
    /// audio thread does the work at its next callback, without allocating.
    pub fn panic(&self) {
        self.panic_req
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// a panic fade is running or requested
    #[cfg(test)]
    pub fn panicking(&self) -> bool {
        self.panic_fade > 0 || self.panic_req.load(std::sync::atomic::Ordering::Acquire)
    }

    fn start_panic(&mut self) {
        self.playing = false;
        self.test_tone = 0;
        self.live.sustain = false;
        self.live.bend = 1.0;
        for v in &mut self.live.v {
            v.active = false;
        }
        self.nv.silence();
        self.av.silence();
        self.plugins.panic();
        self.fx_plugins.panic();
        self.panic_len = ((0.003 * self.sr) as u32).max(1);
        self.panic_fade = self.panic_len;
    }

    /// end of the panic fade: clear every voice, tail and filter state
    fn finish_panic(&mut self, song: &Song) {
        for c in &mut self.chans {
            c.active = false;
            c.synth.amp.release();
        }
        for v in &mut self.live.v {
            v.active = false;
        }
        let devs_of = |owner: usize| -> &[crate::fx::Device] {
            song.instruments
                .get(owner)
                .map(|x| x.fx.as_slice())
                .unwrap_or(&[])
        };
        for ch in self
            .chains
            .iter_mut()
            .chain(std::iter::once(&mut self.live.chain))
        {
            let devs = devs_of(ch.owner);
            for (st, d) in ch.states.iter_mut().zip(devs) {
                if st.kind == d.kind {
                    st.reset(d);
                }
            }
        }
        for (b, ch) in self.rets.iter_mut().enumerate() {
            if let (Some(st), Some(ret)) = (ch.states.first_mut(), song.mixer.returns.get(b)) {
                if st.kind == ret.device.kind {
                    st.reset(&ret.device);
                }
            }
        }
        self.lim.clear();
        self.plugins.panic();
        self.fx_plugins.panic();
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.nv.release_all(&mut self.plugins);
        self.av.release_all();
        self.plugins.all_off();
        for c in &mut self.chans {
            c.synth.amp.release();
        }
    }

    /// Preview a note on a channel (keyboard jamming in the editor).
    /// (instrument, read position in frames) of a sounding sample voice on `ch`
    pub fn sample_playhead(&self, song: &Song, ch: usize) -> Option<(usize, f64)> {
        let c = self.chans.get(ch)?;
        if !c.active {
            return None;
        }
        match &song.instruments.get(c.instr)?.kind {
            InstrumentKind::Sample { data, lp, .. } => {
                crate::sample::read_pos(c.pos, data.len(), lp).map(|p| (c.instr, p))
            }
            _ => None,
        }
    }

    /// the phoneme a Voice note on `ch` is speaking (tests)
    #[cfg(test)]
    pub fn chan_speech(&self, ch: usize) -> Option<crate::speech::Ph> {
        self.chans.get(ch)?.synth.vox.speech.current()
    }

    /// stop the voice on `ch` (with the usual declick fade)
    pub fn release(&mut self, ch: usize) {
        if let Some(c) = self.chans.get_mut(ch) {
            if c.active {
                c.fade_to_tail();
            }
        }
    }

    pub fn preview(&mut self, song: &Song, ch: usize, note: u8, instr: u8) {
        if ch >= self.chans.len() {
            return;
        }
        let cell = Cell {
            note,
            instr,
            ..Default::default()
        };
        self.trigger(song, ch, &cell, None);
    }

    fn tick_len(&self) -> f64 {
        // rows per beat = lines_per_beat; ticks per row = speed
        self.sr as f64 * 60.0 / (self.bpm as f64 * 4.0 * self.speed as f64)
    }

    /// output latency in frames added by the master limiter's lookahead
    #[cfg(test)]
    pub fn latency(&self) -> usize {
        self.lim.latency()
    }

    /// frames of `beats` at the running tempo
    fn beat_frames(&self, beats: f32) -> usize {
        (beats as f64 * 60.0 / self.bpm.max(1.0) as f64 * self.sr as f64) as usize
    }

    /// `glide`: pattern command 3C on this row: Some(None) = glide with the
    /// instrument's time, Some(Some(s)) = glide time in seconds, and
    /// Some(Some(0.0)) = force a retrigger (glide off for this note).
    fn trigger(&mut self, song: &Song, ch: usize, cell: &Cell, glide: Option<Option<f32>>) {
        if ch == song.mixer.duck_key && cell.note > 0 && cell.note != NOTE_OFF {
            self.duck_t = 0.0;
        }
        let new_note = cell.note > 0 && cell.note != NOTE_OFF;
        let ci = if cell.instr > 0 {
            cell.instr as usize - 1
        } else {
            self.chans[ch].instr
        };
        // Voice patch stutter: every note repeats its first slice
        let auto_stut = match song.instruments.get(ci).map(|i| &i.kind) {
            Some(InstrumentKind::Voice(vp)) if vp.stutter_len > 0.0 && vp.stutter_repeats > 0 => {
                Some((
                    self.beat_frames(vp.stutter_len),
                    vp.stutter_repeats,
                    vp.stutter_pitch,
                ))
            }
            _ => None,
        };
        let c = &mut self.chans[ch];
        if new_note && ci < song.instruments.len() {
            c.stut_semi = None;
            match auto_stut {
                Some((f, n, semi)) => c.stut.start(f, n, semi),
                None => c.stut.stop(),
            }
        }
        if cell.instr > 0 && (cell.instr as usize) <= song.instruments.len() {
            if c.instr != cell.instr as usize - 1 {
                // filter commands belong to the instrument they were set for
                c.fm = Default::default();
                c.tfo = crate::transform::NO_OVERRIDES;
                c.gmo = [-1.0; 8];
            }
            c.instr = cell.instr as usize - 1;
            c.vol = song.instruments[c.instr].volume;
        }
        let patch = song.instruments.get(c.instr).and_then(|i| i.kind.patch());
        // 303 glide: legato patch or 3C, onto a note that is still held
        let glide_s = match glide {
            Some(Some(t)) => t,
            Some(None) => patch.map(|p| p.glide).unwrap_or(0.06),
            None if patch.is_some_and(|p| p.legato) => patch.map(|p| p.glide).unwrap_or(0.0),
            None => 0.0,
        };
        let gliding = new_note && glide_s > 0.0 && c.active && c.gate && c.instr != usize::MAX;
        if cell.note == NOTE_OFF {
            c.gate = false;
            c.glide_to = 0.0;
            c.synth.amp.release();
            c.synth.fenv.release();
        } else if gliding {
            c.note = cell.note;
            let t = song
                .instruments
                .get(c.instr)
                .map(|i| i.transpose)
                .unwrap_or(0);
            c.glide_to = note_freq((cell.note as i32 + t as i32).clamp(1, 127) as u8);
            // exponential approach in log frequency, ~99 % there after glide_s
            c.glide_k = 1.0 - (-4.6 / (glide_s.max(0.001) * self.sr)).exp();
        } else if cell.note > 0 && c.instr != usize::MAX {
            c.gate = true;
            c.glide_to = 0.0;
            if c.active {
                c.fade_to_tail();
            }
            c.note = cell.note;
            let t = song
                .instruments
                .get(c.instr)
                .map(|i| i.transpose)
                .unwrap_or(0);
            c.freq = note_freq((cell.note as i32 + t as i32).clamp(1, 127) as u8);
            self.seed = self
                .seed
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            c.synth = SynthVoice::seeded(self.seed);
            c.pos = 0.0;
            c.active = true;
            // Voice words: each note speaks the next word, 3Exx picks one
            if let Some(InstrumentKind::Voice(vp)) = song.instruments.get(c.instr).map(|i| &i.kind)
            {
                if !vp.words.is_empty() {
                    let n = vp.words.len();
                    let w = if cell.fx == 0x3E {
                        cell.fxp as usize
                    } else {
                        c.word_next
                    } % n;
                    c.synth.vox.speak(&vp.words[w]);
                    c.word_next = (w + 1) % n;
                    c.spoke = true;
                }
            }
        }
        if cell.vol > 0 {
            c.vol = (cell.vol - 1) as f32 / 64.0;
        }
        c.synth.vel = c.vol;
        // velocity accent (a new note; 3D on the row can override it)
        if new_note && !gliding {
            self.mods.feed.note_on(c.instr, cell.note, c.vol);
            c.synth.accent = match patch {
                Some(p) if p.accent_vel > 0.0 && c.vol >= p.accent_vel - 1e-4 => p.accent,
                _ => 0.0,
            };
        }
        // plugin instruments: note on / off events (mono per channel)
        if !self.plugins.is_empty() {
            let (ci, cn, cv) = (c.instr, c.note, c.vol);
            if cell.note == NOTE_OFF {
                self.plugins.release_ch(ch);
            } else if new_note && !gliding {
                match song.instruments.get(ci) {
                    Some(inst) if matches!(inst.kind, InstrumentKind::Plugin(_)) => {
                        let n = (cn as i32 + inst.transpose as i32).clamp(1, 127) as u8;
                        self.plugins.note(ci, ch, midi_key(n), cv);
                    }
                    _ => self.plugins.release_ch(ch),
                }
            }
        }
    }

    fn process_row(&mut self, song: &Song) {
        if self.tl.on {
            self.process_tl_row(song);
            return;
        }
        let Some(pat) = song.patterns.get(self.pattern) else {
            self.playing = false;
            return;
        };
        let Some(cells) = pat.cells.get(self.row) else {
            return;
        };
        let mut brk = false;
        for ch in 0..song.channels.min(MAX_CHANNELS) {
            let cell = cells.get(ch).copied().unwrap_or_default();
            brk |= self.process_cell(song, ch, cell);
        }
        if brk {
            self.row = pat.rows;
        } // jump to next pattern at advance
    }

    /// Timeline mode: every track's cell at the current timeline row. A
    /// `0F00` pattern break only ends its clip's source (it never jumps).
    fn process_tl_row(&mut self, song: &Song) {
        let Some(plan) = self.tl.plan.clone() else {
            self.playing = false;
            return;
        };
        let row = self.tl.row;
        self.tl_tempo(song, &plan, row);
        let nch = song.channels.min(MAX_CHANNELS);
        let mut shown = None;
        for ch in 0..nch {
            let cell = self.tl_cell(song, &plan, ch, row);
            self.process_cell(song, ch, cell);
            self.tl.steps[ch] = 0;
            // the pattern editor follows the cursor track's clip (or the first)
            if let Some(s) = plan
                .tracks
                .get(ch)
                .and_then(|v| v.get(self.tl.cur[ch]))
                .filter(|s| s.start <= row)
            {
                if shown.is_none() || ch == self.live.ch {
                    shown = Some((s.pattern, s.src_row(row)));
                }
            }
        }
        if let Some((p, r)) = shown {
            self.pattern = p;
            self.row = r;
        }
    }

    /// One channel's cell on a row: trigger plus effect commands. Returns
    /// true for a `0F00` pattern break.
    fn process_cell(&mut self, song: &Song, ch: usize, cell: Cell) -> bool {
        let mut brk = false;
        {
            self.chans[ch].slide = 0.0;
            self.chans[ch].vol_slide = 0.0;
            self.chans[ch].cut_slide = 0.0;
            self.chans[ch].spoke = false;
            let glide = (cell.fx == 0x3C).then_some(match cell.fxp {
                0 => Some(0.0),
                1 => None,
                x => Some(x as f32 * 0.005),
            });
            self.trigger(song, ch, &cell, glide);
            let c = &mut self.chans[ch];
            match cell.fx {
                0x01 | 0x02 => {
                    c.slide = if cell.fx == 1 { 1.0 } else { -1.0 } * cell.fxp as f32;
                    c.glide_to = 0.0;
                }
                0x09 if cell.fxp > 0 => self.speed = cell.fxp as u32,
                0x0C => {
                    c.vol = (cell.fxp.min(0x40)) as f32 / 64.0;
                    c.synth.vel = c.vol;
                }
                // 08xx: this note's amp decay = xx * 10 ms (hats: short / long)
                0x08 if cell.fxp > 0 => c.synth.decay = cell.fxp as f32 * 0.01,
                0x0D => c.vol_slide = (cell.fxp >> 4) as f32 - (cell.fxp & 15) as f32,
                0x0F if cell.fxp == 0 => brk = true,
                0x0F if cell.fxp <= 0xF0 => self.bpm = cell.fxp as f32,
                0x30..=0x37 => self.voice_fx(song, ch, cell.fx, cell.fxp),
                // 3Exx: speak word xx (on a held note: say it again now)
                0x3E if !c.spoke && c.active => {
                    if let Some(InstrumentKind::Voice(vp)) =
                        song.instruments.get(c.instr).map(|i| &i.kind)
                    {
                        if !vp.words.is_empty() {
                            let n = vp.words.len();
                            let w = cell.fxp as usize % n;
                            c.synth.vox.speak(&vp.words[w]);
                            c.word_next = (w + 1) % n;
                        }
                    }
                }
                // 38xx cutoff: 00 = instrument, 01-FF = 20 Hz .. 20 kHz (log)
                0x38 => {
                    c.fm.cut_oct = 0.0;
                    c.fm.cut_abs = if cell.fxp == 0 {
                        0.0
                    } else {
                        cutoff_from_code(cell.fxp)
                    };
                }
                // 39xx resonance: 00 = instrument, 01-FF = 0 .. 100 %
                0x39 => {
                    c.fm.res = if cell.fxp == 0 {
                        -1.0
                    } else {
                        (cell.fxp - 1) as f32 / 254.0
                    };
                }
                // 3Axx / 3Bxx cutoff slide up / down: xx/96 octave per tick
                0x3A => c.cut_slide = cell.fxp as f32 / 96.0,
                0x3B => c.cut_slide = -(cell.fxp as f32) / 96.0,
                // 3Fxx Transform blend, 40xx resonance, 41xx digital, 42xx
                // sub: 00 = instrument, 01-FF = 0 .. 100 %
                0x3F..=0x42 => {
                    c.tfo[(cell.fx - 0x3F) as usize] = tf_amount(cell.fxp);
                }
                // 43xy Generator macro x (1-8) = y (0-F = 0 .. 100 %); 4300 =
                // back to the instrument's knobs
                0x43 => match cell.fxp >> 4 {
                    0 => c.gmo = [-1.0; 8],
                    x @ 1..=8 => c.gmo[x as usize - 1] = (cell.fxp & 15) as f32 / 15.0,
                    _ => {}
                },
                // 3Dxx accent: 00 = instrument amount, 01-40 = amount
                0x3D => {
                    c.synth.accent = if cell.fxp == 0 {
                        song.instruments
                            .get(c.instr)
                            .and_then(|i| i.kind.patch())
                            .map(|p| p.accent)
                            .unwrap_or(0.6)
                    } else {
                        cell.fxp.min(0x40) as f32 / 64.0
                    };
                }
                _ => {}
            }
        }
        brk
    }

    /// Voice / glitch effects 30-37 (docs/PATTERN_EDITOR.md). 33/34 work
    /// on any instrument; the others change the Voice generator of the note.
    fn voice_fx(&mut self, song: &Song, ch: usize, fx: u8, fxp: u8) {
        use crate::voice::{len_code_beats, Vowel};
        let (x, y) = (fxp >> 4, fxp & 15);
        match fx {
            0x33 => {
                let frames = self.beat_frames(len_code_beats(x));
                let c = &mut self.chans[ch];
                let semi = c.stut_semi.unwrap_or_else(|| {
                    match song.instruments.get(c.instr).map(|i| &i.kind) {
                        Some(InstrumentKind::Voice(vp)) => vp.stutter_pitch,
                        _ => 0.0,
                    }
                });
                if x == 0 || y == 0 {
                    c.stut.stop();
                } else {
                    c.stut.start(frames, y, semi);
                }
            }
            0x34 => {
                let c = &mut self.chans[ch];
                let semi = x as f32 - y as f32;
                c.stut_semi = Some(semi);
                c.stut.set_pitch(semi);
            }
            _ => {
                let c = &mut self.chans[ch];
                let v = &mut c.synth.vox;
                match fx {
                    0x30 => {
                        if let Some(vw) = Vowel::from_index(x) {
                            v.fx.vowel = Some((vw, y as f32 * 25.0));
                        }
                    }
                    0x31 => v.set_step(fxp as usize),
                    0x32 => v.fx.shift = Some(2f32.powf((fxp as f32 - 128.0) / 128.0)),
                    0x35 => v.fx.breath = Some(fxp.min(0x40) as f32 / 64.0),
                    0x36 => {
                        v.fx.babble = Some((len_code_beats(x.max(1)), y as f32 / 15.0));
                    }
                    0x37 => {
                        v.fx.chop = Some(if y == 0 || x == 0 {
                            (0.0, 1.0)
                        } else {
                            (len_code_beats(x), y as f32 / 16.0)
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    fn advance_row(&mut self, song: &Song) {
        if self.tl.on {
            self.rows_played += 1;
            let (end, lp) = match self.tl.plan.as_deref() {
                Some(p) => (p.end, p.loop_rows),
                None => (0, None),
            };
            let mut next = self.tl.row + 1;
            let mut jump = false;
            if let Some((a, b)) = lp {
                if next == b {
                    next = a;
                    jump = true;
                }
            }
            if next >= end {
                next = 0;
                jump = true;
                self.song_ended = true;
            }
            self.tl.row = next;
            if jump {
                self.tl_cursors(next);
                self.notes_jump(song, next, false);
            }
            self.process_row(song);
            return;
        }
        self.row += 1;
        self.rows_played += 1;
        let rows = song
            .patterns
            .get(self.pattern)
            .map(|p| p.rows)
            .unwrap_or(ROWS);
        if self.row >= rows {
            self.row = 0;
            if !self.loop_pattern {
                self.order_pos += 1;
                if self.order_pos >= song.order.len() {
                    self.order_pos = 0;
                    self.song_ended = true;
                }
                self.pattern = song.order.get(self.order_pos).copied().unwrap_or(0);
            }
        }
        self.process_row(song);
    }

    fn do_tick(&mut self, song: &Song) {
        if self.tl.on {
            self.do_tl_tick(song);
            return;
        }
        self.tick += 1;
        if self.tick >= self.speed {
            self.tick = 0;
            self.advance_row(song);
        } else {
            for c in &mut self.chans {
                if c.slide != 0.0 {
                    c.freq *= 2f32.powf(c.slide / 192.0);
                }
                if c.cut_slide != 0.0 {
                    c.fm.cut_oct = (c.fm.cut_oct + c.cut_slide).clamp(-10.0, 10.0);
                }
                if c.vol_slide != 0.0 {
                    c.vol = (c.vol + c.vol_slide / 64.0).clamp(0.0, 1.0);
                }
            }
        }
    }

    /// Timeline mode tick: tracks with their own speed (Track::speed) step
    /// their slides that many times per row, spread over the song's ticks.
    fn do_tl_tick(&mut self, song: &Song) {
        self.tick += 1;
        let ss = self.speed.max(1);
        let row_end = self.tick >= ss;
        let nch = song.channels.min(MAX_CHANNELS);
        for i in 0..nch {
            let own = self
                .tl
                .plan
                .as_deref()
                .and_then(|p| p.speeds.get(i).copied())
                .unwrap_or(0) as u32;
            let n = if own == 0 {
                u32::from(!row_end)
            } else {
                let total = own.saturating_sub(1);
                let due = if row_end || ss <= 1 {
                    total
                } else {
                    ((self.tick * total) as f32 / (ss - 1) as f32).round() as u32
                };
                let done = self.tl.steps[i] as u32;
                self.tl.steps[i] = due.min(255) as u8;
                due.saturating_sub(done)
            };
            let c = &mut self.chans[i];
            for _ in 0..n {
                if c.slide != 0.0 {
                    c.freq *= 2f32.powf(c.slide / 192.0);
                }
                if c.cut_slide != 0.0 {
                    c.fm.cut_oct = (c.fm.cut_oct + c.cut_slide).clamp(-10.0, 10.0);
                }
                if c.vol_slide != 0.0 {
                    c.vol = (c.vol + c.vol_slide / 64.0).clamp(0.0, 1.0);
                }
            }
        }
        if row_end {
            self.tick = 0;
            self.advance_row(song);
        }
    }

    /// mixer gains per channel (L, R) and the master gain, with automation
    fn mix_targets(&self, song: &Song) -> ([(f32, f32); MAX_CHANNELS], f32) {
        let nch = song.channels.min(MAX_CHANNELS);
        let mut tgt = [(0.0f32, 0.0f32); MAX_CHANNELS];
        for (i, t) in tgt.iter_mut().enumerate().take(nch) {
            let m = song.mixer.channels.get(i).copied().unwrap_or_default();
            let vol = self.auto.chan_vol[i].unwrap_or(m.volume);
            let pan = self.auto.chan_pan[i].unwrap_or(m.pan);
            let g = if song.mixer.audible(i) { vol } else { 0.0 };
            // constant power pan, normalised so centre = 1.0 per side
            let a = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
            *t = (
                g * a.cos() * std::f32::consts::SQRT_2,
                g * a.sin() * std::f32::consts::SQRT_2,
            );
        }
        (tgt, self.auto.master.unwrap_or(song.mixer.master))
    }

    /// Song position in lines (fractional, includes the tick phase).
    fn calc_song_pos(&self, song: &Song) -> f32 {
        if self.tl.on {
            let tl = self.tick_len().max(1.0);
            let tick_frac = (1.0 - self.samples_to_tick / tl).clamp(0.0, 1.0);
            return self.tl.row as f32 + (self.tick as f32 + tick_frac as f32) / self.speed as f32;
        }
        let idx = if self.loop_pattern {
            song.order.iter().position(|&b| b == self.pattern)
        } else {
            Some(self.order_pos)
        };
        let start: usize = idx
            .map(|i| {
                song.order[..i.min(song.order.len())]
                    .iter()
                    .map(|&b| song.patterns.get(b).map(|p| p.rows).unwrap_or(ROWS))
                    .sum()
            })
            .unwrap_or(0);
        let tl = self.tick_len().max(1.0);
        let tick_frac = (1.0 - self.samples_to_tick / tl).clamp(0.0, 1.0);
        start as f32 + self.row as f32 + (self.tick as f32 + tick_frac as f32) / self.speed as f32
    }

    /// Evaluate all lanes at the current position, then the modulation
    /// routes, and update the overrides.
    fn eval_auto(&mut self, song: &Song) {
        use crate::automation::{value_at, Target};
        let a = &mut self.auto;
        a.chan_vol = [None; MAX_CHANNELS];
        a.chan_pan = [None; MAX_CHANNELS];
        a.master = None;
        a.fx.clear();
        a.ret.clear();
        a.macros.clear();
        a.pend.clear();
        a.send = [[None; 2]; MAX_CHANNELS];
        let ni = song.instruments.len();
        if a.level.len() != ni {
            a.level.resize(ni, 1.0);
            a.patches.resize(ni, None);
            a.gen.resize(ni, crate::gen::NO_KNOBS);
            a.gen_on.resize(ni, false);
        }
        for (g, on) in a.gen.iter_mut().zip(a.gen_on.iter_mut()) {
            if *on {
                *g = crate::gen::NO_KNOBS;
                *on = false;
            }
        }
        a.level.iter_mut().for_each(|l| *l = 1.0);
        let lanes = self.playing && !song.automation.is_empty();
        let modm = &song.modulation;
        let modded = !modm.routes.is_empty() && !modm.mods.is_empty();
        if !lanes && !modded {
            a.patches.iter_mut().for_each(|p| *p = None);
            a.vals.clear();
            return;
        }
        if modded {
            a.mod_amt.iter_mut().for_each(|x| *x = f32::NAN);
        }
        if lanes {
            let pos = self.calc_song_pos(song);
            self.song_pos = pos;
            let a = &mut self.auto;
            if a.vals.len() != song.automation.len() {
                a.vals = vec![f32::NAN; song.automation.len()];
            }
            if a.targets.len() != song.automation.len() {
                a.targets
                    .resize(song.automation.len(), (String::new(), None));
                a.targets.iter_mut().for_each(|c| c.0.clear());
            }
            for (k, lane) in song.automation.iter().enumerate() {
                if !lane.enabled {
                    continue;
                }
                let cache = &mut a.targets[k];
                if cache.0 != lane.target {
                    cache.0.clone_from(&lane.target);
                    cache.1 = Target::parse(&lane.target);
                }
                let (Some(t), Some(n)) = (cache.1, value_at(&lane.points, pos)) else {
                    continue;
                };
                // smoothing: ~1.5 ms time constant at 32-frame steps
                let v = &mut a.vals[k];
                *v = if v.is_nan() { n } else { *v + (n - *v) * 0.5 };
                match t {
                    Target::Tempo => self.bpm = t.to_real(*v).clamp(20.0, 400.0),
                    Target::ModAmount(r) => {
                        if let Some(x) = a.mod_amt.get_mut(r) {
                            *x = t.to_real(*v);
                        }
                    }
                    _ if a.pend.len() < PEND_CAP => a.pend.push((t, *v)),
                    _ => {}
                }
            }
        } else {
            self.auto.vals.clear();
        }
        if modded {
            self.eval_mods(song);
        }
        let a = &mut self.auto;
        let mut used = [false; 256];
        for &(t, v) in a.pend.iter() {
            let real = t.to_real(v.clamp(0.0, 1.0));
            match t {
                Target::Cutoff(i)
                | Target::Resonance(i)
                | Target::Drive(i)
                | Target::Patch(i, _) => {
                    let Some(src) = song.instruments.get(i).and_then(|x| x.kind.patch()) else {
                        continue;
                    };
                    let slot = &mut a.patches[i];
                    if i < used.len() && !used[i] {
                        // refresh from the song (keeps live UI edits), no realloc after the first
                        match slot {
                            Some(p) => p.copy_sound_from(src),
                            None => *slot = Some(src.clone()),
                        }
                        used[i] = true;
                    }
                    let p = slot.as_mut().unwrap();
                    match t {
                        Target::Cutoff(_) => p.cutoff = real,
                        Target::Resonance(_) => p.resonance = real,
                        Target::Drive(_) => p.drive = real,
                        Target::Patch(_, k) => {
                            if let Some(pp) = crate::modulation::PATCH_PARAMS.get(k) {
                                (pp.set)(p, real);
                            }
                        }
                        _ => {}
                    }
                }
                Target::Level(i) => {
                    if let Some(l) = a.level.get_mut(i) {
                        *l = real;
                    }
                }
                Target::ChanVolume(c) if c < MAX_CHANNELS => a.chan_vol[c] = Some(real),
                Target::ChanPan(c) if c < MAX_CHANNELS => a.chan_pan[c] = Some(real),
                Target::Master => a.master = Some(real),
                Target::Fx(i, d, p) => {
                    if a.fx.len() < FX_OVER_CAP {
                        a.fx.push((i, d, p, real));
                    }
                }
                Target::Ret(b, p) => {
                    if a.ret.len() < FX_OVER_CAP {
                        a.ret.push((b, p, real));
                    }
                }
                Target::Send(c, b) if c < MAX_CHANNELS && b < 2 => a.send[c][b] = Some(real),
                Target::Macro(i, m) if a.macros.len() < FX_OVER_CAP => a.macros.push((i, m, real)),
                Target::Gen(i, k) if i < a.gen.len() && k < crate::gen::GEN_KNOBS => {
                    a.gen[i][k] = real;
                    a.gen_on[i] = true;
                }
                _ => {}
            }
        }
        for (i, p) in self.auto.patches.iter_mut().enumerate() {
            if i >= used.len() || !used[i] {
                *p = None;
            }
        }
    }

    /// Step the modulators and add every route's contribution to the
    /// pending values (on top of a lane's value, else the song's own value).
    fn eval_mods(&mut self, song: &Song) {
        use crate::modulation::{MAX_MODS, MAX_ROUTES};
        let Engine {
            auto: a,
            mods,
            fx_plugins,
            chans,
            live,
            nv,
            ..
        } = self;
        let held = &mut mods.feed.held;
        *held = [false; 256];
        for c in chans.iter().take(song.channels.min(MAX_CHANNELS)) {
            if c.active && c.gate && c.instr < 256 {
                held[c.instr] = true;
            }
        }
        for v in live.v.iter() {
            if v.active && (v.down || v.held_by_pedal) && v.instr < 256 {
                held[v.instr] = true;
            }
        }
        nv.mark_held(held);
        let m = &song.modulation;
        mods.step(m, song, 32.0 / self.sr, self.bpm);
        for (r, route) in m.routes.iter().enumerate().take(MAX_ROUTES) {
            let Some(si) = m.find(route.src).filter(|k| *k < MAX_MODS) else {
                continue;
            };
            let Some(t) = crate::automation::Target::parse(&route.target) else {
                continue;
            };
            if !crate::automation::modulatable(t) {
                continue;
            }
            let amt = match a.mod_amt[r] {
                x if x.is_nan() => route.amount,
                x => x,
            };
            let d = amt * mods.out[si];
            if let Some(e) = a.pend.iter_mut().find(|e| e.0 == t) {
                e.1 += d;
            } else if a.pend.len() < PEND_CAP {
                let base = match t {
                    crate::automation::Target::Fx(i, dv, p) => {
                        match song.instruments.get(i).and_then(|x| x.fx.get(dv)) {
                            Some(dev) if dev.plugin.is_some() => fx_plugins
                                .slots
                                .iter()
                                .position(|s| s.owner == i && s.uid == dev.uid)
                                .and_then(|si| fx_plugins.base_norm(si, p as u32)),
                            _ => crate::automation::song_norm(song, t),
                        }
                    }
                    _ => crate::automation::song_norm(song, t),
                };
                if let Some(b) = base {
                    a.pend.push((t, b + d));
                }
            }
        }
    }

    /// Bring the FX states in line with the song (structure + param targets).
    /// Buffers are only (re)allocated when a chain's structure changes.
    fn refresh_fx(&mut self, song: &Song) {
        let sr = self.sr;
        let a = &self.auto;
        for (i, ch) in self
            .chains
            .iter_mut()
            .enumerate()
            .take(song.channels.min(MAX_CHANNELS))
        {
            let c = &self.chans[i];
            let owner = if c.instr != usize::MAX {
                c.instr
            } else if c.tail.instr != usize::MAX {
                c.tail.instr
            } else {
                ch.owner
            };
            let devs: &[crate::fx::Device] = song
                .instruments
                .get(owner)
                .map(|x| x.fx.as_slice())
                .unwrap_or(&[]);
            ch.sync(devs, owner, sr);
            resolve_plugins(&mut self.fx_plugins, ch, devs, i, owner, &a.fx);
            let tfo = c.tfo;
            for (d, (st, dev)) in ch.states.iter_mut().zip(devs).enumerate() {
                // 40-42 set the mix of this channel's Resonator / Digital / Sub
                let cmd = match dev.kind {
                    crate::fx::FxKind::Resonator => tfo[1],
                    crate::fx::FxKind::Digital => tfo[2],
                    crate::fx::FxKind::Sub => tfo[3],
                    _ => -1.0,
                };
                st.set_targets(dev, |p| {
                    if p == 0 && cmd >= 0.0 {
                        return Some(cmd);
                    }
                    a.fx.iter()
                        .find(|o| o.0 == owner && o.1 == d && o.2 == p)
                        .map(|o| o.3)
                });
            }
        }
        let nch = song.channels.min(MAX_CHANNELS);
        self.fx_plugins.max_lat = self.fx_plugins.ch_lat[..nch]
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .min(crate::plugin::fxrack::PDC_MAX as u32 - 1);
        let lv = &mut self.live;
        if lv.owner != usize::MAX {
            let owner = lv.owner;
            let devs: &[crate::fx::Device] = song
                .instruments
                .get(owner)
                .map(|x| x.fx.as_slice())
                .unwrap_or(&[]);
            lv.chain.sync(devs, owner, sr);
            resolve_plugins(
                &mut self.fx_plugins,
                &mut lv.chain,
                devs,
                crate::plugin::fxrack::LIVE,
                owner,
                &a.fx,
            );
            for (d, (st, dev)) in lv.chain.states.iter_mut().zip(devs).enumerate() {
                st.set_targets(dev, |p| {
                    a.fx.iter()
                        .find(|o| o.0 == owner && o.1 == d && o.2 == p)
                        .map(|o| o.3)
                });
            }
        }
        for (b, ch) in self.rets.iter_mut().enumerate() {
            let Some(ret) = song.mixer.returns.get(b) else {
                ch.sync(&[], b, sr);
                continue;
            };
            let devs = std::slice::from_ref(&ret.device);
            ch.sync(devs, b, sr);
            if let Some(st) = ch.states.first_mut() {
                st.set_targets(&ret.device, |p| {
                    a.ret.iter().find(|o| o.0 == b && o.1 == p).map(|o| o.2)
                });
            }
        }
        // plugin parameters no longer automated or modulated: their own value
        if !self.fx_plugins.is_empty() {
            self.fx_plugins.release_untouched();
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn send_targets(&self, song: &Song) -> [[f32; 2]; MAX_CHANNELS] {
        let mut t = [[0.0; 2]; MAX_CHANNELS];
        for (i, tt) in t
            .iter_mut()
            .enumerate()
            .take(song.channels.min(MAX_CHANNELS))
        {
            let m = song.mixer.channels.get(i).copied().unwrap_or_default();
            for b in 0..2 {
                tt[b] = self.auto.send[i][b].unwrap_or(m.send[b]);
            }
        }
        t
    }

    /// Render interleaved stereo.
    pub fn render(&mut self, song: &Song, out: &mut [f32]) {
        crate::plugin::clap::AUDIO_THREAD.with(|a| a.set(true));
        self.render_inner(song, out);
        crate::plugin::clap::AUDIO_THREAD.with(|a| a.set(false));
    }

    fn render_inner(&mut self, song: &Song, out: &mut [f32]) {
        let frames_total = out.len() / 2;
        if self
            .panic_req
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            self.start_panic();
        }
        let plugs = !self.plugins.is_empty();
        let pdc = self.fx_plugins.pdc_active();
        // live MIDI events (lock-free queue, filled by the MIDI thread)
        if let Some(mut rx) = self.live.rx.take() {
            while let Ok(ev) = rx.pop() {
                self.live_event(song, ev);
            }
            self.live.rx = Some(rx);
        }
        let live_on = self.live.v.iter().any(|v| v.active);
        let nch = song.channels.clamp(1, MAX_CHANNELS);
        // note clips (timeline mode): the plan can't change during a callback
        let note_plan = self.tl.plan.clone().filter(|p| self.tl.on && p.has_notes());
        // audio clips (timeline mode, M4); voices fading out after a stop
        // still read their files through the plan
        let audio_plan = self
            .tl
            .plan
            .clone()
            .filter(|p| (self.tl.on || self.av.any) && p.has_audio());
        let live_ch = self.live.ch.min(nch - 1);
        let live_gain = {
            // fader + pan of the cursor channel, ignoring mute/solo
            let m = song
                .mixer
                .channels
                .get(live_ch)
                .copied()
                .unwrap_or_default();
            let a = (m.pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
            (
                m.volume * a.cos() * std::f32::consts::SQRT_2,
                m.volume * a.sin() * std::f32::consts::SQRT_2,
            )
        };
        let mut peak = self.peak * 0.97;
        let mut plr = [self.peak_lr[0] * 0.97, self.peak_lr[1] * 0.97];
        let mut cpk = self.chan_peak;
        for p in &mut cpk {
            *p *= 0.97;
        }
        // automation is evaluated on its own 32-frame grid (below), not per
        // callback, so the output does not depend on the buffer size
        // (export == realtime); the FX structure is synced right away
        self.refresh_fx(song);
        let (mut tgt, mut master) = self.mix_targets(song);
        let mut stgt = self.send_targets(song);
        let k = 1.0 - (-1.0 / (0.005 * self.sr)).exp();
        let ls = song.mixer.limiter;
        let (lim_on, lim_gain, lim_ceil, lim_rel) = (
            ls.enabled,
            crate::master::db_to_lin(ls.gain_db),
            crate::master::db_to_lin(ls.ceiling_db),
            ls.release_ms / 1000.0,
        );
        for (fi, frame) in out.chunks_mut(2).enumerate() {
            // automation is evaluated every 32 frames (sub-block), values smoothed
            if self.auto.countdown == 0 {
                self.auto.countdown = 32;
                self.eval_auto(song);
                self.refresh_fx(song);
                (tgt, master) = self.mix_targets(song);
                stgt = self.send_targets(song);
            }
            self.auto.countdown -= 1;
            if self.playing {
                // a tick lands on the first sample at or after its exact time
                // (f64 accumulator: no drift, at most 1 sample of rounding)
                if self.samples_to_tick <= 0.0 {
                    self.samples_to_tick += self.tick_len();
                    self.do_tick(song);
                }
                self.samples_to_tick -= 1.0;
                if let Some(p) = note_plan.as_deref() {
                    if self.playing && self.tl.on {
                        let t = self.tl_clock();
                        notes::note_events(
                            &mut self.nv,
                            &mut self.mods.feed,
                            p,
                            song,
                            &mut self.chans,
                            &mut self.plugins,
                            nch,
                            t,
                        );
                    }
                }
                if let Some(p) = audio_plan.as_deref() {
                    if self.playing && self.tl.on {
                        let t = self.tl_clock();
                        self.av.events(p, nch, t, self.sr);
                    }
                }
            }
            // plugin chunks end where the next tick (new events) starts
            let chunk = if plugs {
                let left = frames_total - fi;
                let until = if self.playing {
                    self.samples_to_tick.ceil().max(0.0) as usize + 1
                } else {
                    usize::MAX
                };
                left.min(until).min(crate::plugin::MAX_BLOCK)
            } else {
                0
            };
            let mut l = 0.0;
            let mut r = 0.0;
            let mut bus = [[0.0f32; 2]; 2];
            let ctx = crate::fx::Ctx {
                bpm: self.bpm,
                note: 0.0,
            };
            let mg = 0.6 * master;
            let sr = self.sr;
            let bpm = self.bpm;
            // sidechain pump: 2 ms dip, 10 ms hold, quadratic recovery
            self.duck_t += 1.0 / sr;
            let duck_env = duck_env(self.duck_t, song.mixer.duck_release);
            // 5 ms ramps for tails and mute changes
            let step = 1.0 / (0.005 * sr);
            // live MIDI voices -> live FX chain -> cursor channel (post fader)
            let (live_l, live_r) = if live_on || !self.live.chain.states.is_empty() {
                let lv = &mut self.live;
                let (mut s, mut s_r) = (0.0, 0.0);
                for v in lv.v.iter_mut().filter(|v| v.active) {
                    let ((x, xr), alive) = voice(
                        song,
                        v.instr,
                        v.freq * lv.bend,
                        &mut v.synth,
                        &mut v.pos,
                        sr,
                        &self.auto,
                        bpm,
                    );
                    v.active = alive;
                    s += x * v.vol;
                    s_r += xr * v.vol;
                }
                let (fl, fr) = if lv.chain.states.is_empty() {
                    (s, s_r)
                } else {
                    let note = lv.v.iter().find(|v| v.active).map(|v| v.freq * lv.bend);
                    let lctx = crate::fx::Ctx {
                        bpm,
                        note: note.unwrap_or(0.0),
                    };
                    lv.chain.process_with(s, s_r, &lctx, &mut self.fx_plugins)
                };
                (fl * live_gain.0, fr * live_gain.1)
            } else {
                (0.0, 0.0)
            };
            let mut nv_alive = false;
            let mut av_alive = false;
            for (i, (c, chain)) in self
                .chans
                .iter_mut()
                .zip(self.chains.iter_mut())
                .enumerate()
                .take(song.channels.min(MAX_CHANNELS))
            {
                let (mut s, mut s_r) = (0.0, 0.0);
                if c.glide_to > 0.0 {
                    let lf = c.freq.ln();
                    let lt = c.glide_to.ln();
                    let d = lt - lf;
                    if d.abs() < 1e-4 {
                        c.freq = c.glide_to;
                        c.glide_to = 0.0;
                    } else {
                        c.freq = (lf + d * c.glide_k).exp();
                    }
                }
                c.synth.fmod = c.fm;
                c.synth.tfo = c.tfo;
                c.synth.gmo = c.gmo;
                if c.active {
                    let ((v, vr), alive) = voice(
                        song,
                        c.instr,
                        c.freq,
                        &mut c.synth,
                        &mut c.pos,
                        sr,
                        &self.auto,
                        bpm,
                    );
                    c.active = alive;
                    s += v * c.vol;
                    s_r += vr * c.vol;
                }
                // note-clip voices of this track (before its insert chain)
                if self.nv.any {
                    let base = i * notes::NOTE_VOICES;
                    for v in self.nv.v[base..base + notes::NOTE_VOICES]
                        .iter_mut()
                        .filter(|v| v.active)
                    {
                        let (x, xr) = v.render(song, sr, &self.auto, bpm);
                        s += x;
                        s_r += xr;
                        nv_alive |= v.active;
                    }
                }
                // audio clips of this track (before its insert chain)
                if self.av.any {
                    if let Some(p) = audio_plan.as_deref() {
                        let t = self.av.clock;
                        let base = i * audioclips::AUDIO_VOICES;
                        for v in self.av.v[base..base + audioclips::AUDIO_VOICES]
                            .iter_mut()
                            .filter(|v| v.active)
                        {
                            let (x, xr) = v.render(p, t);
                            s += x;
                            s_r += xr;
                            av_alive |= v.active;
                        }
                    }
                }
                // plugin instruments on this channel (their own release tails)
                if plugs {
                    let rack = &mut self.plugins;
                    for k in 0..rack.ch_slots(i).len() {
                        let si = rack.ch_slots(i)[k] as usize;
                        let pi = rack.slots[si].instr;
                        let mine = pi == c.instr;
                        if rack.needs_chunk(si) {
                            if mine {
                                if let Some((n, _)) = rack.held(si) {
                                    let base = note_freq(n.saturating_sub(11));
                                    if base > 0.0 && c.freq > 0.0 {
                                        rack.set_tuning(si, 12.0 * (c.freq / base).log2());
                                    }
                                }
                            }
                            for m in 0..8 {
                                let v = if mine && c.gmo[m] >= 0.0 {
                                    c.gmo[m]
                                } else {
                                    self.auto
                                        .macros
                                        .iter()
                                        .find(|x| x.0 == pi && x.1 == m)
                                        .map(|x| x.2)
                                        .unwrap_or(-1.0)
                                };
                                rack.set_macro(si, m, v);
                            }
                            rack.chunk(si, chunk);
                        }
                        let (pl, pr) = rack.sample(si);
                        let lvl = self.auto.level.get(pi).copied().unwrap_or(1.0);
                        // volume column / 0C / 0D after the note: relative gain
                        let g = match rack.held(si) {
                            Some((_, v0)) if mine => (c.vol / v0).min(4.0),
                            _ => 1.0,
                        } * lvl;
                        s += pl * g;
                        s_r += pr * g;
                    }
                }
                if c.tail.gain > 0.0 {
                    let t = &mut c.tail;
                    let ((v, vr), alive) = voice(
                        song,
                        t.instr,
                        t.freq,
                        &mut t.synth,
                        &mut t.pos,
                        sr,
                        &self.auto,
                        bpm,
                    );
                    s += v * t.vol * t.gain;
                    s_r += vr * t.vol * t.gain;
                    t.gain = if alive { (t.gain - step).max(0.0) } else { 0.0 };
                }
                (s, s_r) = c.stut.process(s, s_r);
                let (tl, tr) = tgt[i];
                c.gl += (tl - c.gl) * k;
                c.gr += (tr - c.gr) * k;
                // insert chain: stereo voice in, stereo out
                let (fl, fr) = if chain.states.is_empty() {
                    (s, s_r)
                } else {
                    let cctx = crate::fx::Ctx {
                        bpm,
                        note: if c.active { c.freq } else { 0.0 },
                    };
                    chain.process_with(s, s_r, &cctx, &mut self.fx_plugins)
                };
                // plugin delay compensation: channels with less plugin
                // latency wait for the slowest chain
                let (fl, fr) = if pdc {
                    self.fx_plugins.compensate(i, fl, fr)
                } else {
                    (fl, fr)
                };
                let duck = song.mixer.channels.get(i).map(|m| m.duck).unwrap_or(0.0);
                let dg = 1.0 - duck.clamp(0.0, 1.0) * duck_env;
                let (mut sl, mut sr2) = (fl * c.gl * dg, fr * c.gr * dg);
                if i == live_ch {
                    sl += live_l;
                    sr2 += live_r;
                }
                let snd = &mut self.sends[i];
                for b in 0..2 {
                    snd[b] += (stgt[i][b] - snd[b]) * k;
                    bus[b][0] += sl * snd[b];
                    bus[b][1] += sr2 * snd[b];
                }
                cpk[i] = cpk[i].max((sl.abs().max(sr2.abs()) * 0.6).min(1.5));
                l += sl;
                r += sr2;
            }
            if self.nv.any {
                self.nv.any = nv_alive;
            }
            if self.av.any {
                self.av.any = av_alive && audio_plan.is_some();
            }
            if pdc {
                self.fx_plugins.pdc_advance();
            }
            // return buses (100 % wet devices) into the master
            for (b, ch) in self.rets.iter_mut().enumerate() {
                if ch.states.is_empty() {
                    continue;
                }
                let lvl = song.mixer.returns.get(b).map(|r| r.level).unwrap_or(0.0);
                let (rl, rr) = ch.process(bus[b][0], bus[b][1], &ctx);
                l += rl * lvl;
                r += rr * lvl;
            }
            if self.test_tone > 0 {
                self.test_tone -= 1;
                self.tone_ph = (self.tone_ph + 440.0 / self.sr).fract();
                let env = (self.test_tone as f32 / (0.02 * self.sr)).min(1.0);
                let v = (self.tone_ph * std::f32::consts::TAU).sin() * 0.25 * env / 0.6;
                l += v;
                r += v;
            }
            // master volume, then the true-peak limiter (or, bypassed, the
            // classic tanh soft clipper of 0.2.3.6 and earlier)
            let (l, r) = if lim_on {
                self.lim
                    .process(l * mg * lim_gain, r * mg * lim_gain, lim_ceil, lim_rel)
            } else {
                ((l * mg).tanh(), (r * mg).tanh())
            };
            let (l, r) = if self.panic_fade > 0 {
                // panic: 3 ms fade (no click), then everything is cleared
                self.panic_fade -= 1;
                let g = self.panic_fade as f32 / self.panic_len as f32;
                if self.panic_fade == 0 {
                    self.finish_panic(song);
                }
                (l * g, r * g)
            } else {
                (l, r)
            };
            self.meter.push(l, r, self.playing);
            peak = peak.max(l.abs()).max(r.abs());
            plr[0] = plr[0].max(l.abs());
            plr[1] = plr[1].max(r.abs());
            frame[0] = l;
            if frame.len() > 1 {
                frame[1] = r;
            }
        }
        self.peak = peak;
        self.peak_lr = plr;
        self.chan_peak = cpk;
        self.loud = crate::master::Readout {
            gain_red: if lim_on {
                crate::master::lin_to_db(self.lim.take_min_gain())
            } else {
                0.0
            },
            ..self.meter.out
        };
    }
}

/// Point the plugin devices of a chain at their running instances (or none:
/// passed through), send their automation, and note the chain's latency.
/// Engine thread; no allocation.
fn resolve_plugins(
    rack: &mut crate::plugin::fxrack::FxRack,
    ch: &mut crate::fx::ChainState,
    devs: &[crate::fx::Device],
    chain: usize,
    owner: usize,
    auto: &[(usize, usize, usize, f32)],
) {
    let mut lat = 0;
    for (d, (st, dev)) in ch.states.iter_mut().zip(devs).enumerate() {
        if st.kind != crate::fx::FxKind::Plugin {
            continue;
        }
        st.uid = dev.uid;
        st.plug = if rack.is_empty() {
            u16::MAX
        } else {
            rack.find(chain, owner, dev.uid)
        };
        if st.plug == u16::MAX {
            continue;
        }
        lat += rack.slots[st.plug as usize].latency;
        for o in auto.iter().filter(|o| o.0 == owner && o.1 == d) {
            rack.automate(st.plug, o.2 as u32, o.3);
        }
    }
    if chain < MAX_CHANNELS {
        rack.ch_lat[chain] = lat;
    }
}

/// Render the whole song once (order list) offline.
pub fn render_song(song: &Song, sr: u32) -> Vec<f32> {
    let mut e = Engine::new(sr as f32);
    crate::plugin::prepare(&mut e.plugins, song, sr as f32);
    crate::plugin::fxrack::prepare(&mut e.fx_plugins, song, sr as f32);
    e.start(song, 0, 0, false, 0);
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; 1024];
    let max = ((song.length_seconds() + 30.0) * sr as f64) as usize * 2;
    while !e.song_ended && out.len() < max {
        e.render(song, &mut buf);
        out.extend_from_slice(&buf);
    }
    // 3 s tail for releases and delay/reverb tails
    e.playing = false;
    for _ in 0..(sr as usize * 3 / 512) {
        e.render(song, &mut buf);
        out.extend_from_slice(&buf);
    }
    out
}

pub fn write_wav(path: &std::path::Path, data: &[f32], sr: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sr,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in data {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

/// Test-only allocator that counts allocations made on the current thread
/// while counting is switched on (proves `render` is allocation-free).
#[cfg(test)]
pub(crate) mod alloc_count {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static ON: Cell<bool> = const { Cell::new(false) };
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }
    pub struct Counting;
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let _ = ON.try_with(|on| {
                if on.get() {
                    COUNT.with(|c| c.set(c.get() + 1));
                }
            });
            // SAFETY: forwards to the system allocator with the same layout
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            // SAFETY: p was allocated by System with this layout
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let _ = ON.try_with(|on| {
                if on.get() {
                    COUNT.with(|c| c.set(c.get() + 1));
                }
            });
            // SAFETY: same contract as GlobalAlloc::realloc
            unsafe { System.realloc(p, l, n) }
        }
    }
    #[global_allocator]
    static A: Counting = Counting;

    /// run `f` and return how many allocations it made on this thread
    pub fn count(f: impl FnOnce()) -> usize {
        COUNT.with(|c| c.set(0));
        ON.with(|o| o.set(true));
        f();
        ON.with(|o| o.set(false));
        COUNT.with(|c| c.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a channel that switches between instruments with different FX chains
    /// mid-song reuses parked chains instead of allocating
    #[test]
    fn instrument_switch_does_not_allocate() {
        use crate::fx::{Device, FxKind};
        let mut song = crate::fileio::demo_song();
        song.automation.clear();
        song.instruments[0].fx = vec![Device::new(FxKind::Delay), Device::new(FxKind::Reverb)];
        song.instruments[1].fx = vec![Device::new(FxKind::Chorus)];
        let rows = song.patterns[0].rows;
        for r in 0..rows {
            let c = &mut song.patterns[0].cells[r][0];
            *c = Cell {
                note: 37 + (r % 12) as u8,
                instr: 1 + (r % 2) as u8,
                ..Default::default()
            };
        }
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, true, 0);
        let mut buf = vec![0.0f32; 512];
        for _ in 0..600 {
            e.render(&song, &mut buf);
        }
        let n = alloc_count::count(|| {
            for _ in 0..3000 {
                e.render(&song, &mut buf);
            }
        });
        assert_eq!(n, 0, "instrument switches allocated {n} times");
        assert!(buf.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn render_is_allocation_free_after_warmup() {
        let song = crate::fileio::demo_song();
        assert!(
            !song.automation.is_empty(),
            "demo should exercise automation"
        );
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 512];
        // warm-up: FX buffers and patch copies are built on first use
        for _ in 0..400 {
            e.render(&song, &mut buf);
        }
        let n = alloc_count::count(|| {
            for _ in 0..2000 {
                e.render(&song, &mut buf);
            }
        });
        assert_eq!(n, 0, "engine.render allocated {n} times");
    }

    fn rows_in(e: &mut Engine, song: &Song, secs: f32) -> u64 {
        let before = e.rows_played;
        let mut buf = vec![0.0f32; 512];
        for _ in 0..(secs * e.sr / 256.0) as usize {
            e.render(song, &mut buf);
        }
        e.rows_played - before
    }

    #[test]
    fn tempo_change_applies_while_playing() {
        let song = crate::fileio::demo_song();
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, true, 0);
        let r1 = rows_in(&mut e, &song, 2.0);
        e.set_tempo(song.bpm * 2.0, song.speed);
        let r2 = rows_in(&mut e, &song, 2.0);
        assert!(e.playing);
        assert!((r2 as f64 - 2.0 * r1 as f64).abs() <= 3.0, "{r1} {r2}");
    }

    #[test]
    fn automation_applies_in_render() {
        use crate::automation::{Lane, Point, Shape, Target};
        let mut song = crate::fileio::demo_song();
        let plain = {
            let mut s = song.clone();
            s.automation.clear();
            render_song(&s, 22050)
        };
        let auto = render_song(&song, 22050);
        let diff: f32 = plain.iter().zip(&auto).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 1.0, "demo lanes must change the sound ({diff})");
        // master volume lane at 0 => silence after the smoothing settles
        let mut l = Lane::new(Target::Master, "m".into());
        l.points.push(Point {
            pos: 0.0,
            value: 0.0,
            shape: Shape::Linear,
        });
        song.automation = vec![l];
        let out = render_song(&song, 22050);
        let peak = out[2000..44100].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak < 1e-3, "{peak}");
    }

    #[test]
    fn tempo_effect_0f_applies() {
        let mut song = crate::fileio::demo_song();
        song.bpm = 100.0;
        song.patterns[0].cells[0][0].fx = 0x0F;
        song.patterns[0].cells[0][0].fxp = 200;
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, true, 0);
        assert_eq!(e.bpm(), 200.0);
        // 200 BPM * 4 rows per beat = 13.33 rows/s
        let r = rows_in(&mut e, &song, 3.0);
        assert!((r as i64 - 40).abs() <= 2, "{r}");
    }

    #[test]
    fn fx_chain_and_sends_render_finite() {
        let mut song = crate::fileio::demo_song();
        song.mixer.channels[1].send = [0.5, 0.5];
        let a = render_song(&song, 22050);
        assert!(a.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        // without FX the render differs (the chains are really in the path)
        let mut dry = song.clone();
        dry.instruments.iter_mut().for_each(|i| i.fx.clear());
        dry.mixer.channels[1].send = [0.0; 2];
        let b = render_song(&dry, 22050);
        let diff: f32 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum();
        assert!(diff > 1.0, "{diff}");
        // all FX bypassed + no sends == dry
        let mut byp = song.clone();
        byp.instruments
            .iter_mut()
            .flat_map(|i| i.fx.iter_mut())
            .for_each(|d| d.enabled = false);
        byp.mixer.channels[1].send = [0.0; 2];
        let c = render_song(&byp, 22050);
        let n = b.len().min(c.len());
        let md = (0..n).map(|k| (b[k] - c[k]).abs()).fold(0.0f32, f32::max);
        assert!(md < 1e-3, "{md}");
    }

    #[test]
    fn sample_loop_playback_honoured() {
        use crate::sample::{LoopMode, SampleLoop};
        let mk = |mode| {
            let mut song = Song::empty();
            song.channels = 1;
            song.patterns = vec![Pattern::new("b", ROWS, 1)];
            song.instruments = vec![Instrument {
                name: "s".into(),
                volume: 1.0,
                transpose: 0,
                fx: vec![],
                kind: InstrumentKind::Sample {
                    file: String::new(),
                    rate: 8000,
                    data: (0..800).map(|i| ((i as f32) * 0.3).sin() * 0.8).collect(),
                    base_note: 48,
                    lp: SampleLoop {
                        mode,
                        start: 200,
                        end: 600,
                        finetune: 0,
                    },
                },
            }];
            song
        };
        let run = |song: &Song| {
            let mut e = Engine::new(8000.0);
            e.preview(song, 0, 48, 1);
            let mut buf = vec![0.0f32; 2 * 8000];
            e.render(song, &mut buf);
            (buf, e.sample_playhead(song, 0))
        };
        // without a loop the 0.1 s sample is over after 1 s
        let (b, ph) = run(&mk(LoopMode::Off));
        assert!(ph.is_none());
        assert!(b[2 * 4000..].iter().all(|x| x.abs() < 1e-6));
        for m in [LoopMode::Forward, LoopMode::PingPong] {
            let (b, ph) = run(&mk(m));
            let (_, p) = ph.expect("still sounding");
            assert!((200.0..=600.0).contains(&p), "{m:?} {p}");
            let tail = b[2 * 6000..].iter().fold(0.0f32, |a, x| a.max(x.abs()));
            assert!(tail > 0.1, "{m:?} keeps playing: {tail}");
            assert!(b.iter().all(|x| x.is_finite()));
        }
    }

    fn on(key: u8, vel: f32, instr: u8) -> crate::midi::LiveEv {
        crate::midi::LiveEv::On {
            key,
            note: crate::midi::tracker_note(key, 0).unwrap(),
            vel,
            instr,
            ch: 0,
        }
    }

    /// demo song with a sustaining synth in slot 01 and a quiet master
    /// (no limiter saturation)
    fn live_song() -> Song {
        let mut song = crate::fileio::demo_song();
        song.automation.clear();
        let p = crate::synth::Patch {
            amp: crate::synth::Adsr {
                a: 0.001,
                d: 0.1,
                s: 0.8,
                r: 0.05,
            },
            ..Default::default()
        };
        song.instruments[0].kind = InstrumentKind::Synth(p);
        song.instruments[0].fx.clear();
        song.instruments[0].volume = 1.0;
        song.mixer.master = 0.1;
        song
    }

    /// sidechain: a channel with `duck` dips right after each note on the key
    /// channel and recovers within the release time; duck 0 = untouched
    #[test]
    fn effect_08_sets_note_decay_and_a_new_hat_chokes_the_open_one() {
        // an open hat (long decay) on row 0, then a closed hit with 08 03 on row 4:
        // the open hat's ring stops at row 4 (same channel = choke group) and the
        // 08 note is gone within ~40 ms, while a plain note still rings
        let render = |fx: u8| {
            let mut song = live_song();
            song.bpm = 120.0;
            song.speed = 6;
            song.instruments[1].kind = InstrumentKind::Synth(crate::presets::hat(true));
            song.instruments[1].fx.clear();
            let pat = &mut song.patterns[0];
            for row in pat.cells.iter_mut() {
                row.iter_mut().for_each(|c| *c = Cell::default());
            }
            for (r, f) in [(0usize, 0u8), (4, fx)] {
                pat.cells[r][0] = Cell {
                    note: 61,
                    instr: 2,
                    fx: if f > 0 { 0x08 } else { 0 },
                    fxp: f,
                    ..Default::default()
                };
            }
            song.order = vec![0];
            song.mixer.channels[0].send = [0.0; 2];
            let mut e = Engine::new(48000.0);
            e.start(&song, 0, 0, false, 0);
            let mut out = vec![0.0f32; 48000 * 2];
            for c in out.chunks_mut(512) {
                e.render(&song, c);
            }
            out
        };
        // row 4 at 120 BPM = 0.5 s; window 60..110 ms after it
        let w = |o: &[f32]| rms(&o[(0.56 * 96000.0) as usize..(0.61 * 96000.0) as usize]);
        let plain = render(0);
        let short = render(3);
        assert!(
            w(&short) < 0.1 * w(&plain),
            "{} vs {}",
            w(&short),
            w(&plain)
        );
        // before row 4 both are the same open hat
        let pre = |o: &[f32]| rms(&o[(0.3 * 96000.0) as usize..(0.45 * 96000.0) as usize]);
        assert!((pre(&short) - pre(&plain)).abs() < 1e-6);
    }

    #[test]
    fn rows_trigger_sample_accurately_on_the_grid() {
        // 140 BPM, speed 6 at 44.1 kHz: a row is exactly 4725 samples and a
        // tick 787.5, so tick rounding must not drift or jitter the rows
        let mut song = live_song();
        song.bpm = 140.0;
        song.speed = 6;
        let p = crate::synth::Patch {
            amp: crate::synth::Adsr {
                a: 0.0,
                d: 0.01,
                s: 0.0,
                r: 0.001,
            },
            ..Default::default()
        };
        song.instruments[1].kind = InstrumentKind::Synth(p);
        song.instruments[1].fx.clear();
        let pat = &mut song.patterns[0];
        for row in pat.cells.iter_mut() {
            row.iter_mut().for_each(|c| *c = Cell::default());
        }
        let rows = [0usize, 1, 3, 5, 6, 13, 30, 63];
        for &r in &rows {
            pat.cells[r][0] = Cell {
                note: 49,
                instr: 2,
                ..Default::default()
            };
        }
        song.order = vec![0];
        song.mixer.channels[0].send = [0.0; 2];
        let sr = 44100.0;
        let mut e = Engine::new(sr);
        e.start(&song, 0, 0, false, 0);
        let n = 4725 * 64 + 2000;
        let mut out = vec![0.0f32; n * 2];
        for c in out.chunks_mut(512) {
            e.render(&song, c);
        }
        let mono: Vec<f32> = out.chunks(2).map(|f| f[0].abs().max(f[1].abs())).collect();
        let mut late = vec![];
        // the master limiter's lookahead delays everything by a fixed amount
        let lat = e.latency();
        for &r in &rows {
            let at = r * 4725 + lat;
            // silent just before the row, sounding a few samples after it
            let before = mono[at.saturating_sub(300)..at]
                .iter()
                .fold(0.0f32, |m, x| m.max(*x));
            let first = (at..at + 400).find(|&i| mono[i] > 1e-3).unwrap();
            assert!(before < 1e-3, "row {r}: sound {before} before the row");
            late.push(first - at);
        }
        // the same tiny offset (oscillator phase / gain ramp) on every row after
        // the first: no drift, no tick jitter
        assert!(late.iter().all(|&d| d <= 8), "{late:?}");
        let rest = &late[1..];
        let (lo, hi) = (rest.iter().min().unwrap(), rest.iter().max().unwrap());
        assert!(hi - lo <= 1, "row onsets jitter: {late:?}");
    }

    #[test]
    fn sidechain_ducking_pumps_after_key_notes() {
        let render = |duck: f32| {
            let mut song = live_song();
            song.bpm = 120.0;
            song.speed = 6;
            let p = crate::synth::Patch {
                amp: crate::synth::Adsr {
                    a: 0.0,
                    d: 0.1,
                    s: 1.0,
                    r: 0.05,
                },
                ..Default::default()
            };
            song.instruments[1].kind = InstrumentKind::Synth(p);
            song.instruments[1].fx.clear();
            let pat = &mut song.patterns[0];
            for row in pat.cells.iter_mut() {
                row.iter_mut().for_each(|c| *c = Cell::default());
            }
            // key channel 1: a note every beat (silent instrument 3 is fine:
            // only the trigger matters); channel 2: one long pad note
            for r in (0..64).step_by(4) {
                pat.cells[r][0] = Cell {
                    note: 49,
                    instr: 1,
                    ..Default::default()
                };
            }
            pat.cells[0][1] = Cell {
                note: 49,
                instr: 2,
                ..Default::default()
            };
            song.order = vec![0];
            song.mixer.channels[0].mute = true;
            song.mixer.channels[1].duck = duck;
            song.mixer.duck_key = 0;
            song.mixer.duck_release = 0.2;
            let mut e = Engine::new(48000.0);
            e.start(&song, 0, 0, false, 0);
            let mut out = vec![0.0f32; 48000 * 2];
            for c in out.chunks_mut(1024) {
                e.render(&song, c);
            }
            out
        };
        let dry = render(0.0);
        let wet = render(0.8);
        // beat 2 starts at 0.5 s; window 3..11 ms after it vs 400..480 ms after
        let w = |o: &[f32], a: f32, b: f32| rms(&o[(a * 96000.0) as usize..(b * 96000.0) as usize]);
        // deepest 4 ms window around the beat (the row may land a tick late)
        let dip = (0..20)
            .map(|k| {
                let a = 0.495 + k as f32 * 0.004;
                w(&wet, a, a + 0.004) / w(&dry, a, a + 0.004)
            })
            .fold(1.0f32, f32::min);
        let back = w(&wet, 0.9, 0.98) / w(&dry, 0.9, 0.98);
        assert!(dip < 0.3, "ducked to {dip}");
        assert!(back > 0.97, "recovered to {back}");
    }

    fn rms(b: &[f32]) -> f32 {
        (b.iter().map(|x| x * x).sum::<f32>() / b.len() as f32).sqrt()
    }

    /// polyphony, note-off, sustain pedal, pitch bend and voice stealing
    #[test]
    fn live_midi_voices() {
        use crate::midi::LiveEv;
        let song = live_song();
        let mut e = Engine::new(48000.0);
        let mut buf = vec![0.0f32; 1024];
        // a 3-note chord: three voices
        for k in [60, 64, 67] {
            e.live_event(&song, on(k, 0.8, 1));
        }
        assert_eq!(e.live_voices(), 3);
        e.render(&song, &mut buf);
        assert!(rms(&buf) > 0.001, "chord sounds");
        // the same key again retriggers, no extra voice
        e.live_event(&song, on(60, 0.8, 1));
        assert_eq!(e.live_voices(), 3);
        // note-off releases (voice ends after the release)
        for k in [60, 64, 67] {
            e.live_event(&song, LiveEv::Off { key: k });
        }
        for _ in 0..400 {
            e.render(&song, &mut buf);
        }
        assert_eq!(e.live_voices(), 0, "released voices end");
        // sustain pedal keeps a released note until the pedal goes up
        e.live_event(&song, LiveEv::Sustain(true));
        e.live_event(&song, on(62, 1.0, 1));
        e.live_event(&song, LiveEv::Off { key: 62 });
        for _ in 0..400 {
            e.render(&song, &mut buf);
        }
        assert_eq!(e.live_voices(), 1, "held by the pedal");
        e.live_event(&song, LiveEv::Sustain(false));
        for _ in 0..400 {
            e.render(&song, &mut buf);
        }
        assert_eq!(e.live_voices(), 0, "pedal up releases");
        // pitch bend scales the frequency
        e.live_event(&song, LiveEv::Bend(12.0));
        assert!((e.live.bend - 2.0).abs() < 1e-4);
        e.live_event(&song, LiveEv::AllOff);
        assert_eq!(e.live.bend, 1.0);
        // more keys than voices: the oldest is stolen, never more voices
        for k in 0..(LIVE_VOICES as u8 + 4) {
            e.live_event(&song, on(40 + k, 0.5, 1));
        }
        assert_eq!(e.live_voices(), LIVE_VOICES);
        assert!(!e.live.v.iter().any(|v| v.key == 40), "oldest stolen");
        // a bad instrument slot is ignored
        e.live_event(&song, LiveEv::AllOff);
        for _ in 0..400 {
            e.render(&song, &mut buf);
        }
        e.live_event(&song, on(60, 1.0, 200));
        assert_eq!(e.live_voices(), 0);
        assert!(buf.iter().all(|x| x.is_finite()));
    }

    /// velocity scales the level; the live bus follows the cursor channel's
    /// fader but not its mute
    #[test]
    fn live_velocity_and_channel_fader() {
        let mut song = live_song();
        let level = |song: &Song, vel: f32| {
            let mut e = Engine::new(48000.0);
            let mut buf = vec![0.0f32; 4096];
            e.live_event(song, on(60, vel, 1));
            e.render(song, &mut buf);
            rms(&buf)
        };
        let loud = level(&song, 1.0);
        let soft = level(&song, 0.25);
        assert!((soft / loud - 0.25).abs() < 0.05, "{soft} vs {loud}");
        song.mixer.channels[0].mute = true;
        assert!(
            (level(&song, 1.0) - loud).abs() < loud * 0.05,
            "mute ignored"
        );
        song.mixer.channels[0].volume = 0.0;
        assert!(level(&song, 1.0) < 1e-6, "fader applies");
    }

    /// MIDI events through the lock-free queue while the song plays: render
    /// stays allocation-free (after the live FX chain is built once)
    #[test]
    fn live_midi_through_queue_does_not_allocate() {
        use crate::fx::{Device, FxKind};
        use crate::midi::LiveEv;
        let mut song = crate::fileio::demo_song();
        song.automation.clear();
        song.instruments[0].fx = vec![Device::new(FxKind::Delay)];
        let (mut prod, cons) = rtrb::RingBuffer::new(256);
        let mut e = Engine::new(48000.0);
        e.set_live_input(Some(cons));
        e.start(&song, 0, 0, true, 0);
        let mut buf = vec![0.0f32; 512];
        // warm-up: one note builds the live FX chain
        prod.push(on(60, 1.0, 1)).unwrap();
        prod.push(LiveEv::Off { key: 60 }).unwrap();
        for _ in 0..200 {
            e.render(&song, &mut buf);
        }
        let n = alloc_count::count(|| {
            for k in 0..600u32 {
                let key = 48 + (k % 24) as u8;
                let _ = prod.push(on(key, 0.7, 1));
                let _ = prod.push(LiveEv::Bend((k % 5) as f32 * 0.1));
                let _ = prod.push(LiveEv::Sustain(k % 7 == 0));
                if k % 2 == 1 {
                    let _ = prod.push(LiveEv::Off { key });
                }
                e.render(&song, &mut buf);
            }
        });
        assert_eq!(n, 0, "live MIDI allocated {n} times");
        assert!(buf.iter().all(|x| x.is_finite()));
        // the queue survives an engine swap (device change)
        let rx = e.take_live_input();
        assert!(rx.is_some());
    }

    fn voice_song(preset: &str) -> Song {
        let mut song = live_song();
        song.bpm = 120.0;
        song.speed = 6;
        song.instruments[1].kind =
            InstrumentKind::Voice(crate::voice::preset(preset).expect("preset"));
        song.instruments[1].fx.clear();
        let pat = &mut song.patterns[0];
        for row in pat.cells.iter_mut() {
            row.iter_mut().for_each(|c| *c = Cell::default());
        }
        song.order = vec![0];
        song.mixer.channels[0].send = [0.0; 2];
        song.mixer.channels[0].duck = 0.0;
        song.mixer.master = 1.0;
        song
    }

    fn render_secs(song: &Song, secs: f32) -> Vec<f32> {
        let mut e = Engine::new(48000.0);
        e.start(song, 0, 0, false, 0);
        let mut out = vec![0.0f32; (secs * 48000.0) as usize * 2];
        for c in out.chunks_mut(512) {
            e.render(song, c);
        }
        out
    }

    #[test]
    fn voice_presets_and_effects_render_finite_and_allocation_free() {
        for (name, _, _) in crate::voice::presets() {
            let mut song = voice_song(name);
            // notes plus every voice effect 30-37
            let fx = [
                (0x30, 0x22),
                (0x31, 0x01),
                (0x32, 0x60),
                (0x33, 0x23),
                (0x34, 0x02),
                (0x35, 0x30),
                (0x36, 0x38),
                (0x37, 0x38),
            ];
            for (k, (f, p)) in fx.iter().enumerate() {
                let r = k * 8;
                song.patterns[0].cells[r][0] = Cell {
                    note: if k % 2 == 0 { 37 } else { 0 },
                    instr: 2,
                    vol: 0,
                    fx: *f,
                    fxp: *p,
                };
            }
            let out = render_secs(&song, 3.0);
            assert!(
                out.iter().all(|x| x.is_finite() && x.abs() <= 1.0),
                "{name}"
            );
            let rms = (out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32).sqrt();
            assert!(rms > 0.005, "{name} silent ({rms})");
            // allocation-free after warm-up
            let mut e = Engine::new(48000.0);
            e.start(&song, 0, 0, false, 0);
            let mut buf = vec![0.0f32; 512];
            for _ in 0..50 {
                e.render(&song, &mut buf);
            }
            let n = alloc_count::count(|| {
                for _ in 0..500 {
                    e.render(&song, &mut buf);
                }
            });
            assert_eq!(n, 0, "{name}: render allocated {n} times");
        }
    }

    #[test]
    fn effect_33_stutter_repeats_a_beat_slice_on_the_grid() {
        // 120 BPM at 48 kHz: 1/16 note = 6000 frames; 3302 = 2 repeats
        let mut song = voice_song("Choir Eee");
        if let InstrumentKind::Voice(v) = &mut song.instruments[1].kind {
            v.jitter = 0.0;
            v.breath = 0.0;
            v.base.lfo = Default::default();
            v.base.amp = crate::synth::Adsr {
                a: 0.05,
                d: 0.5,
                s: 0.5,
                r: 0.1,
            };
        }
        song.patterns[0].cells[0][0] = Cell {
            note: 37,
            instr: 2,
            vol: 0,
            fx: 0x33,
            fxp: 0x32,
        };
        let out = render_secs(&song, 0.6);
        let l: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        let diff = |a: usize, b: usize| {
            (200..5800)
                .map(|i| (l[a + i] - l[b + i]).abs())
                .fold(0.0f32, f32::max)
        };
        let peak = l[..18000].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.02, "audible ({peak})");
        // frames 6000.. and 12000.. replay frames 0..6000
        assert!(diff(6000, 0) < peak * 0.02, "repeat 1 {}", diff(6000, 0));
        assert!(diff(12000, 0) < peak * 0.02, "repeat 2 {}", diff(12000, 0));
        // then the live voice continues (no third repeat)
        assert!(diff(18000, 0) > peak * 0.1, "live again {}", diff(18000, 0));
    }

    #[test]
    fn voice_effects_30_31_32_set_the_note_state() {
        let mut song = voice_song("Glitch Babble");
        song.patterns[0].cells[0][0] = Cell {
            note: 37,
            instr: 2,
            vol: 0,
            fx: 0x30,
            fxp: 0x24,
        };
        song.patterns[0].cells[1][0] = Cell {
            fx: 0x32,
            fxp: 0x80,
            ..Default::default()
        };
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let fx = e.chans[0].synth.vox.fx;
        assert_eq!(fx.vowel, Some((crate::voice::Vowel::I, 100.0)));
        let mut buf = vec![0.0f32; 2 * 6200];
        e.render(&song, &mut buf);
        assert_eq!(e.chans[0].synth.vox.fx.shift, Some(1.0), "32 80 = x1.0");
        // a new note resets the per-note overrides
        e.preview(&song, 0, 40, 2);
        assert!(e.chans[0].synth.vox.fx.vowel.is_none());
    }

    #[test]
    fn voice_words_cycle_per_note_and_3e_picks_or_respeaks() {
        use crate::speech::Ph;
        let mut song = voice_song("Talk Ka-Ta"); // ka ta ka da
        let c = |note: u8, fx: u8, fxp: u8| Cell {
            note,
            instr: if note > 0 { 2 } else { 0 },
            vol: 0,
            fx,
            fxp,
        };
        song.patterns[0].cells[0][0] = c(37, 0, 0);
        song.patterns[0].cells[1][0] = c(37, 0, 0);
        song.patterns[0].cells[2][0] = c(0, 0x3E, 0x00);
        song.patterns[0].cells[3][0] = c(37, 0x3E, 0x03);
        song.patterns[0].cells[4][0] = c(37, 0, 0);
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let cur = |e: &Engine| e.chans[0].synth.vox.speech.current();
        assert_eq!(cur(&e), Some(Ph::K), "row 0: ka");
        let mut buf = vec![0.0f32; 2 * 6200];
        e.render(&song, &mut buf);
        assert_eq!(cur(&e), Some(Ph::T), "row 1: next word ta");
        let mut buf = vec![0.0f32; 2 * 6000];
        e.render(&song, &mut buf);
        assert_eq!(
            cur(&e),
            Some(Ph::K),
            "row 2: 3E00 says ka again on the held note"
        );
        e.render(&song, &mut buf);
        assert_eq!(cur(&e), Some(Ph::D), "row 3: 3E03 = da");
        e.render(&song, &mut buf);
        assert_eq!(cur(&e), Some(Ph::K), "row 4: cycles on to word 0");
        assert!(buf.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn throat_voices_render_fast_enough() {
        let mut song = voice_song("Demon Words");
        for ch in 0..8 {
            for r in (0..64).step_by(4) {
                song.patterns[0].cells[r][ch] = Cell {
                    note: 25 + (ch as u8) * 3,
                    instr: 2,
                    ..Default::default()
                };
            }
        }
        song.channels = 8;
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 1024];
        let t0 = std::time::Instant::now();
        let frames = 48000 * 4;
        for _ in 0..frames / 512 {
            e.render(&song, &mut buf);
        }
        let x = 4.0 / t0.elapsed().as_secs_f64();
        eprintln!("8 throat voices: {x:.1}x realtime");
        // a timing test on shared machines: only catch big regressions
        // (CI runners and a loaded box measured 2.6-3.5x; it used to need 3x)
        let min = if std::env::var_os("CI").is_some() {
            1.0
        } else {
            1.5
        };
        assert!(
            x > min,
            "8 throat voices at {x:.1}x realtime (need > {min}x)"
        );
        assert!(buf.iter().all(|v| v.is_finite()));
    }

    // ------------------------------------------- 0.2.3.11: Voice Transform

    const TF_PRESETS: [&str; 5] = [
        "Metal Demon",
        "Digital Ghost",
        "Sub Beast",
        "Cyber Choir",
        "Ritual Growl",
    ];

    /// one held note per bar of a Transform preset (optionally dry)
    fn tf_song(preset: &str, dry: bool) -> Song {
        let mut song = voice_song(preset);
        if let InstrumentKind::Voice(v) = &mut song.instruments[1].kind {
            if dry {
                v.transform.blend = 0.0;
            }
        }
        for r in [0usize, 16, 32, 48] {
            song.patterns[0].cells[r][0] = Cell {
                note: 25 + (r / 16) as u8 * 3,
                instr: 2,
                ..Default::default()
            };
        }
        song
    }

    fn rms_of(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    #[test]
    fn transform_presets_stay_near_the_dry_level() {
        let mut bad = vec![];
        for name in TF_PRESETS {
            let wet = render_secs(&tf_song(name, false), 4.0);
            let dry = render_secs(&tf_song(name, true), 4.0);
            let db = 20.0 * (rms_of(&wet) / rms_of(&dry)).log10();
            eprintln!("{name}: transformed {db:+.1} dB vs dry");
            // each stage alone at the preset's blend
            for st in 0..3 {
                let mut song = tf_song(name, false);
                if let InstrumentKind::Voice(v) = &mut song.instruments[1].kind {
                    let t = &mut v.transform;
                    if st != 0 {
                        t.res.amount = 0.0;
                    }
                    if st != 1 {
                        t.dig.amount = 0.0;
                    }
                    if st != 2 {
                        t.sub.amount = 0.0;
                    }
                }
                let one = render_secs(&song, 4.0);
                eprintln!(
                    "   stage {st}: {:+.1} dB",
                    20.0 * (rms_of(&one) / rms_of(&dry)).log10()
                );
            }
            assert!(wet.iter().all(|x| x.is_finite()));
            bad.push((name, db));
            assert_ne!(wet, dry, "{name}: the transform does something");
        }
        bad.retain(|(_, db)| !(-6.0..=6.0).contains(db));
        assert!(bad.is_empty(), "{bad:?}");
    }

    #[test]
    fn transform_blend_zero_is_bit_identical_to_no_transform() {
        // blend 0 = the 0.2.3.10 voice exactly, whatever the stage settings
        let a = render_secs(&tf_song("Metal Demon", true), 2.0);
        let mut song = tf_song("Metal Demon", true);
        if let InstrumentKind::Voice(v) = &mut song.instruments[1].kind {
            v.transform = crate::transform::Transform::default();
        }
        assert_eq!(a, render_secs(&song, 2.0));
    }

    #[test]
    fn transform_commands_3f_to_42_set_blend_and_amounts() {
        assert_eq!(tf_amount(0), -1.0);
        assert_eq!(tf_amount(1), 0.0);
        assert_eq!(tf_amount(0xFF), 1.0);
        // 3F01 on a Transform preset = dry; 3FFF on a dry patch = full
        let dry = render_secs(&tf_song("Sub Beast", true), 2.0);
        let mut song = tf_song("Sub Beast", false);
        song.patterns[0].cells[0][0].fx = 0x3F;
        song.patterns[0].cells[0][0].fxp = 0x01;
        let off = render_secs(&song, 2.0);
        let d = dry
            .iter()
            .zip(&off)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(d < 0.02, "3F01 sounds dry (max diff {d})");
        let mut song = tf_song("Sub Beast", true);
        song.patterns[0].cells[0][0].fx = 0x3F;
        song.patterns[0].cells[0][0].fxp = 0xFF;
        let full = render_secs(&song, 2.0);
        let wet = render_secs(&tf_song("Sub Beast", false), 2.0);
        let d2 = full
            .iter()
            .zip(&wet)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(d2 < 0.02, "3FFF = the preset's blend 1.0 (max diff {d2})");
        // 40-42 land on the channel, a new instrument clears them
        let mut song = tf_song("Metal Demon", false);
        song.patterns[0].cells[1][0] = Cell {
            fx: 0x40,
            fxp: 0x80,
            ..Default::default()
        };
        song.patterns[0].cells[2][0] = Cell {
            fx: 0x41,
            fxp: 0x01,
            ..Default::default()
        };
        song.patterns[0].cells[3][0] = Cell {
            fx: 0x42,
            fxp: 0xFF,
            ..Default::default()
        };
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 2 * 6000 * 4];
        e.render(&song, &mut buf);
        let o = e.chans[0].tfo;
        assert_eq!(o[0], -1.0);
        assert!((o[1] - 127.0 / 254.0).abs() < 1e-6);
        assert_eq!((o[2], o[3]), (0.0, 1.0));
        assert_eq!(e.chans[0].synth.tfo, o, "handed to the voice");
        assert!(buf.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn transform_sub_is_mono_below_120_hz_through_the_engine() {
        // the Voice output of every preset with a sub: side vs mid < 120 Hz
        for name in ["Metal Demon", "Sub Beast", "Ritual Growl", "Cyber Choir"] {
            let mut song = tf_song(name, false);
            song.instruments[1].fx.clear();
            let out = render_secs(&song, 3.0);
            let mut f = [[crate::synth::Svf::default(); 4]; 2];
            let g = (std::f32::consts::PI * 100.0 / 48000.0).tan();
            let (mut me, mut se) = (0.0f64, 0.0f64);
            for (i, fr) in out.chunks(2).enumerate() {
                let (mut m, mut s) = ((fr[0] + fr[1]) * 0.5, (fr[0] - fr[1]) * 0.5);
                let [fm, fs] = &mut f;
                for (a, b) in fm.iter_mut().zip(fs.iter_mut()) {
                    m = a.process_gk(m, g, std::f32::consts::SQRT_2).0;
                    s = b.process_gk(s, g, std::f32::consts::SQRT_2).0;
                }
                if i > 4800 {
                    me += (m * m) as f64;
                    se += (s * s) as f64;
                }
            }
            let db = 10.0 * (se / me.max(1e-12)).log10();
            eprintln!("{name}: side below 120 Hz {db:.1} dB re mid");
            assert!(me > 1e-3, "{name} has low end");
            assert!(db < -40.0, "{name}: side below 120 Hz at {db:.1} dB");
        }
    }

    #[test]
    fn transform_presets_true_peak_stays_under_the_limiter_ceiling() {
        // all five presets at once, hot, through the master limiter (-1 dBTP)
        let mut song = tf_song("Metal Demon", false);
        song.channels = 5;
        for (k, name) in TF_PRESETS.iter().enumerate() {
            let mut inst = song.instruments[1].clone();
            inst.kind = InstrumentKind::Voice(crate::voice::preset(name).unwrap());
            inst.volume = 1.0;
            song.instruments.push(inst);
            let slot = song.instruments.len() as u8;
            for r in (0..64).step_by(8) {
                song.patterns[0].cells[r][k] = Cell {
                    note: 13 + (k as u8) * 5 + (r as u8 / 8) % 3,
                    instr: slot,
                    ..Default::default()
                };
            }
            song.mixer.channels[k].volume = 2.0;
            song.mixer.channels[k].send = [0.0; 2];
        }
        song.mixer.master = 2.0;
        let out = render_secs(&song, 4.0);
        let tp = crate::master::true_peak(&out);
        let db = 20.0 * tp.log10();
        eprintln!("five Transform presets, hot: true peak {db:.2} dBTP");
        assert!(
            db <= -0.8,
            "true peak {db:.2} dBTP over the -1 dBTP ceiling"
        );
        assert!(rms_of(&out) > 0.05, "it is actually loud");
    }

    #[test]
    fn transform_voices_render_fast_enough() {
        // 8 Metal Demon voices vs the same voices dry: the transform's
        // cost relative to the voice (robust to a busy machine), and the
        // absolute speed
        let bench = |dry: bool| {
            let mut song = tf_song("Metal Demon", dry);
            for ch in 0..8 {
                for r in (0..64).step_by(8) {
                    song.patterns[0].cells[r][ch] = Cell {
                        note: 25 + (ch as u8) * 3,
                        instr: 2,
                        ..Default::default()
                    };
                }
            }
            song.channels = 8;
            let mut e = Engine::new(48000.0);
            e.start(&song, 0, 0, false, 0);
            let mut buf = vec![0.0f32; 1024];
            let t0 = std::time::Instant::now();
            for _ in 0..(48000 * 3) / 512 {
                e.render(&song, &mut buf);
            }
            3.0 / t0.elapsed().as_secs_f64()
        };
        let (wet, dry) = (bench(false), bench(true));
        eprintln!("8 Metal Demon voices: {wet:.1}x realtime (dry {dry:.1}x)");
        assert!(
            dry / wet < 3.0,
            "transform costs {:.1}x the voice",
            dry / wet
        );
        // shared CI runners (tests in parallel, unoptimised deps on Windows) only
        // need real time; a local run still asks for 2x
        let floor = if std::env::var_os("CI").is_some() {
            1.0
        } else {
            2.0
        };
        assert!(wet > floor, "8 transformed voices at {wet:.1}x realtime");
    }

    #[test]
    fn transform_fx_devices_follow_the_note_and_are_allocation_free() {
        use crate::fx::{Device, FxKind};
        let mut song = acid_song(crate::synth::Patch::default());
        song.instruments[0].fx = vec![
            Device::new(FxKind::Digital),
            Device::new(FxKind::Resonator),
            Device::new(FxKind::Sub),
        ];
        for r in (0..64).step_by(4) {
            song.patterns[0].cells[r][0] = Cell {
                note: 25 + (r % 12) as u8,
                instr: 1,
                fx: if r == 8 { 0x40 } else { 0 },
                fxp: 0xFF,
                ..Default::default()
            };
        }
        let out = render_secs(&song, 3.0);
        assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        assert!(rms_of(&out) > 0.01);
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 512];
        for _ in 0..50 {
            e.render(&song, &mut buf);
        }
        let n = alloc_count::count(|| {
            for _ in 0..500 {
                e.render(&song, &mut buf);
            }
        });
        assert_eq!(n, 0, "render allocated {n} times");
    }

    /// a held saw chord through `kind` on instrument 1, with an automation
    /// lane taking FX param `param` from 0 to 1 over the first 32 rows
    fn mod_fx_song(kind: crate::fx::FxKind, param: &str) -> Song {
        use crate::automation::{Lane, Point, Shape, Target};
        let mut song = acid_song(crate::synth::Patch {
            cutoff: 6000.0,
            amp: crate::synth::Adsr {
                a: 0.01,
                d: 0.5,
                s: 1.0,
                r: 0.2,
            },
            ..Default::default()
        });
        song.instruments[0].fx = vec![crate::fx::Device::new(kind)];
        song.patterns[0].cells[0][0] = note(37, 0, 0);
        let p = crate::fx::Device::param_index(kind, param).expect("param");
        let mut l = Lane::new(Target::Fx(0, 0, p), param.into());
        for (pos, value) in [(0.0, 0.0), (32.0, 1.0)] {
            l.points.push(Point {
                pos,
                value,
                shape: Shape::Linear,
            });
        }
        song.automation = vec![l];
        song
    }

    #[test]
    fn phaser_renders_follows_its_automation_lane_and_is_allocation_free() {
        use crate::fx::FxKind;
        for kind in [FxKind::Phaser, FxKind::FreqShift] {
            mod_fx_renders_and_automates(kind);
        }
    }

    fn mod_fx_renders_and_automates(kind: crate::fx::FxKind) {
        let song = mod_fx_song(kind, "mix");
        let out = render_secs(&song, 2.0);
        assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        // the lane takes the mix from 0 (dry) towards 1: the start of the
        // note matches a render without the phaser, later it doesn't
        let mut dry = song.clone();
        dry.instruments[0].fx.clear();
        dry.automation.clear();
        let d = render_secs(&dry, 2.0);
        let diff = |a: usize, b: usize| -> f32 {
            out[a..b]
                .iter()
                .zip(&d[a..b])
                .map(|(x, y)| (x - y).abs())
                .sum::<f32>()
                / (b - a) as f32
        };
        // 120 BPM speed 6: 32 rows = 4 s, so the mix is ~1 % at 0.05 s and
        // ~45 % at 1.8 s
        let (early, late) = (diff(4000, 8000), diff(150_000, 190_000));
        assert!(
            late > 0.01 && late > 8.0 * early,
            "{kind:?}: {early} -> {late}"
        );
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 512];
        for _ in 0..50 {
            e.render(&song, &mut buf);
        }
        let n = alloc_count::count(|| {
            for _ in 0..500 {
                e.render(&song, &mut buf);
            }
        });
        assert_eq!(n, 0, "render allocated {n} times");
    }

    // ------------------------------------------- generator knob automation

    /// one long note on a generator instrument 1 (filter off)
    fn gen_engine_song(engine: crate::gen::GenEngine) -> Song {
        let mut song = acid_song(saw_patch());
        song.instruments[0].kind = InstrumentKind::Gen(crate::gen::GenPatch {
            engine,
            ..Default::default()
        });
        song.patterns[0].cells[0][0] = note(25, 0, 0); // C-2
        song
    }

    fn analog() -> crate::gen::GenEngine {
        let mut a = crate::gen2::AnalogP::default();
        a.p[1] = 0.0; // no resonance: brightness follows the cutoff only
        crate::gen::GenEngine::Analog(a)
    }

    #[test]
    fn a_lane_sweeps_the_analog_cutoff_and_is_allocation_free() {
        use crate::automation::{Lane, Point, Shape, Target};
        let mut song = gen_engine_song(analog());
        let t = Target::parse("instrument.1.gen.1").expect("gen target");
        assert_eq!(t, Target::Gen(0, 0));
        assert_eq!(t.id(), "instrument.1.gen.1");
        let mut l = Lane::new(t, "Cutoff".into());
        for (pos, value) in [(0.0, 0.0), (16.0, 1.0)] {
            l.points.push(Point {
                pos,
                value,
                shape: Shape::Linear,
            });
        }
        song.automation = vec![l];
        let out = left(&render_secs(&song, 2.4));
        assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        // 16 rows = 2 s: closed at the start, wide open at the end
        let early = bright(&out[2400..12000]);
        let late = bright(&out[100_000..112_000]);
        assert!(late > 4.0 * early, "{early} -> {late}");
        // the 43xy macro still wins over the lane (pattern beats automation)
        let mut m = song.clone();
        m.patterns[0].cells[0][0] = note(25, 0x43, 0x10); // macro 1 = 0
        let mo = left(&render_secs(&m, 2.4));
        let ml = bright(&mo[100_000..112_000]);
        assert!(ml < late / 4.0, "43xy {ml} vs lane {late}");
        // allocation-free with the lane running
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 512];
        for _ in 0..50 {
            e.render(&song, &mut buf);
        }
        let n = alloc_count::count(|| {
            for _ in 0..500 {
                e.render(&song, &mut buf);
            }
        });
        assert_eq!(n, 0, "render allocated {n} times");
    }

    #[test]
    fn every_generator_knob_is_an_automation_target() {
        use crate::automation::Target;
        use crate::gen::{gen_knob_norm, gen_knobs, presets, set_gen_knob_norm, GEN_KNOBS};
        for (name, _, mut g) in presets() {
            let knobs = gen_knobs(&g);
            assert!(!knobs.is_empty(), "{name}");
            for (k, label) in &knobs {
                assert!(*k < GEN_KNOBS && !label.is_empty());
                let t = Target::Gen(3, *k);
                assert_eq!(Target::parse(&t.id()), Some(t));
                assert!(set_gen_knob_norm(&mut g, *k, 0.25));
                let v = gen_knob_norm(&g, *k).unwrap();
                assert!((v - 0.25).abs() < 0.02, "{name} {label} {v}");
            }
        }
        // ids out of range don't parse (and older songs never contain them)
        assert_eq!(Target::parse("instrument.1.gen.0"), None);
        assert_eq!(Target::parse("instrument.1.gen.65"), None);
    }

    #[test]
    fn a_lane_moves_a_drum_kit_piece_macro() {
        use crate::automation::{Lane, Point, Target};
        let mut song = gen_song("808 Kit", &[49]); // C-4 = Kick
        let render = |s: &Song| left(&render_secs(s, 0.4));
        let plain = render(&song);
        let mut l = Lane::new(Target::Gen(1, 0), "Kick: Tune".into());
        l.points.push(Point {
            pos: 0.0,
            value: 1.0,
            shape: Default::default(),
        });
        song.automation = vec![l];
        let moved = render(&song);
        let d: f32 = plain.iter().zip(&moved).map(|(a, b)| (a - b).abs()).sum();
        assert!(d > 1.0, "kick macro 1 lane changed nothing ({d})");
    }

    #[test]
    fn filter_commands_reach_generator_instruments() {
        // Analog (filter off): 38xx/39xx move the ladder cutoff / resonance
        let b = |engine: crate::gen::GenEngine, fx: u8, fxp: u8| {
            let mut song = gen_engine_song(engine);
            song.patterns[0].cells[0][0] = note(25, fx, fxp);
            bright(&left(&render_secs(&song, 0.3))[4800..14000])
        };
        let plain = b(analog(), 0, 0);
        let open = b(analog(), 0x38, 0xE0);
        let shut = b(analog(), 0x38, 0x30);
        assert!(
            open > 2.0 * plain && plain > 1.5 * shut,
            "{shut} {plain} {open}"
        );
        assert_eq!(b(analog(), 0x38, 0x00), plain, "3800 = instrument cutoff");
        // 3Bxx slides the ladder down from the knob
        let slid = b(analog(), 0x3B, 0x40);
        assert!(slid < plain * 0.8, "3B: {slid} vs {plain}");
        // FM-4 (no filter of its own, Filter off): 38xx switches the
        // instrument filter on
        let fm = || crate::gen::GenEngine::Fm(crate::gen2::FmP::default());
        let fplain = b(fm(), 0, 0);
        let fshut = b(fm(), 0x38, 0x30);
        assert!(fplain > 2.0 * fshut, "FM: {fplain} vs {fshut}");
        assert_eq!(b(fm(), 0x38, 0x00), fplain);
    }

    // ------------------------------------------------ 0.2.3.12: generators

    /// a generator on instrument 2, hits every 4 rows on channel 0
    fn gen_song(preset: &str, notes: &[u8]) -> Song {
        let mut song = voice_song("Demon Ahh");
        song.instruments[1].kind = InstrumentKind::Gen(crate::gen::preset(preset).expect("preset"));
        for (k, r) in (0..64).step_by(4).enumerate() {
            song.patterns[0].cells[r][0] = Cell {
                note: notes[k % notes.len()],
                instr: 2,
                ..Default::default()
            };
        }
        song
    }

    #[test]
    fn generators_render_through_the_engine_allocation_free() {
        // C-4 = 49: the kit's kick, snare, clap, hats, toms, percs
        let kit = gen_song("Psy Kit", &[49, 51, 52, 55, 59, 53, 56, 50, 60, 58]);
        for (name, song) in [
            ("Psy Kit", kit),
            ("Psy Zap Kick", gen_song("Psy Zap Kick", &[49, 45])),
            ("Comb Hat", gen_song("Comb Hat", &[49])),
            ("Cymbal", gen_song("Cymbal", &[49])),
            ("Explosion", gen_song("Explosion", &[49, 37])),
            ("Psy Laser", gen_song("Psy Laser", &[49, 61])),
            (
                "Psy Rolling Bass",
                gen_song("Psy Rolling Bass", &[17, 19, 17]),
            ),
            ("Dark Pad", gen_song("Dark Pad", &[37])),
            ("E-Piano", gen_song("E-Piano", &[49, 53])),
            ("Growl", gen_song("Growl", &[25])),
            ("White Riser", gen_song("White Riser", &[49])),
            ("Vinyl Crackle", gen_song("Vinyl Crackle", &[49])),
        ] {
            let mut song = song;
            // and through the ladder (vintage) with pattern filter + macro commands
            if let InstrumentKind::Gen(g) = &mut song.instruments[1].kind {
                g.filter_on = name == "Psy Kit";
                g.base.filter_model = crate::ladder::FilterModel::Vintage;
            }
            song.patterns[0].cells[8][0].fx = 0x43;
            song.patterns[0].cells[8][0].fxp = 0x2F;
            song.patterns[0].cells[16][0].fx = 0x38;
            song.patterns[0].cells[16][0].fxp = 0x60;
            let out = render_secs(&song, 3.0);
            assert!(
                out.iter().all(|x| x.is_finite() && x.abs() <= 1.0),
                "{name}"
            );
            assert!(rms_of(&out) > 0.005, "{name} silent");
            let mut e = Engine::new(48000.0);
            e.start(&song, 0, 0, false, 0);
            let mut buf = vec![0.0f32; 512];
            for _ in 0..50 {
                e.render(&song, &mut buf);
            }
            let n = alloc_count::count(|| {
                for _ in 0..500 {
                    e.render(&song, &mut buf);
                }
            });
            assert_eq!(n, 0, "{name}: render allocated {n} times");
        }
    }

    #[test]
    fn command_43_sets_generator_macros_per_channel() {
        let mut song = gen_song("Psy Zap Kick", &[49]);
        song.patterns[0].cells[0][0].fx = 0x43;
        song.patterns[0].cells[0][0].fxp = 0x40; // macro 4 (decay) = 0
        let short = render_secs(&song, 0.4);
        let long = render_secs(&gen_song("Psy Zap Kick", &[49]), 0.4);
        // 120 BPM speed 6: row 4 at 0.5 s, so 0.25-0.4 s is the first kick's tail
        let tail = |o: &[f32]| rms_of(&o[24000..38400]);
        assert!(
            tail(&short) < tail(&long) * 0.2,
            "{} vs {}",
            tail(&short),
            tail(&long)
        );
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 1024];
        e.render(&song, &mut buf);
        assert_eq!(e.chans[0].gmo[3], 0.0);
        assert_eq!(e.chans[0].synth.gmo, e.chans[0].gmo, "handed to the voice");
        // 4300 clears
        song.patterns[0].cells[4][0].fx = 0x43;
        song.patterns[0].cells[4][0].fxp = 0x00;
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 2 * 30000];
        e.render(&song, &mut buf);
        assert_eq!(e.chans[0].gmo, [-1.0; 8]);
    }

    #[test]
    fn ladder_synth_patches_render_and_old_patches_are_unchanged() {
        // the default (SVF) patch is bit-identical to a patch without the new fields
        let p = crate::synth::Patch::default();
        assert_eq!(p.filter_model, crate::ladder::FilterModel::Svf);
        for m in [
            crate::ladder::FilterModel::Ladder,
            crate::ladder::FilterModel::Vintage,
        ] {
            let mut lp = crate::synth::Patch {
                filter_model: m,
                cutoff: 600.0,
                resonance: 0.9,
                env_amount: 0.6,
                ..Default::default()
            };
            for slope12 in [false, true] {
                lp.ladder_12db = slope12;
                let mut song = acid_song(lp.clone());
                for r in (0..64).step_by(4) {
                    song.patterns[0].cells[r][0] = Cell {
                        note: 25 + (r % 7) as u8,
                        instr: 1,
                        ..Default::default()
                    };
                }
                let out = render_secs(&song, 2.0);
                assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{m:?}");
                assert!(rms_of(&out) > 0.01, "{m:?} silent");
                let mut e = Engine::new(48000.0);
                e.start(&song, 0, 0, false, 0);
                let mut buf = vec![0.0f32; 512];
                e.render(&song, &mut buf);
                let n = alloc_count::count(|| {
                    for _ in 0..300 {
                        e.render(&song, &mut buf);
                    }
                });
                assert_eq!(n, 0, "{m:?} allocated");
            }
        }
    }

    // ------------------------------------------------ 0.2.3.7: acid commands

    /// one synth on channel 0, everything else silent
    fn acid_song(p: crate::synth::Patch) -> Song {
        let mut song = live_song();
        song.bpm = 120.0;
        song.speed = 6;
        song.instruments[0].kind = InstrumentKind::Synth(p);
        for row in song.patterns[0].cells.iter_mut() {
            row.iter_mut().for_each(|c| *c = Cell::default());
        }
        song.order = vec![0];
        for m in song.mixer.channels.iter_mut() {
            m.send = [0.0; 2];
            m.duck = 0.0;
        }
        song.mixer.master = 0.5;
        song
    }

    fn saw_patch() -> crate::synth::Patch {
        crate::synth::Patch {
            cutoff: 400.0,
            resonance: 0.1,
            amp: crate::synth::Adsr {
                a: 0.001,
                d: 0.5,
                s: 1.0,
                r: 0.05,
            },
            ..Default::default()
        }
    }

    fn note(n: u8, fx: u8, fxp: u8) -> Cell {
        Cell {
            note: n,
            instr: 1,
            vol: 0,
            fx,
            fxp,
        }
    }

    fn left(out: &[f32]) -> Vec<f32> {
        out.chunks(2).map(|f| f[0]).collect()
    }

    /// derivative energy / energy: higher = brighter
    fn bright(x: &[f32]) -> f32 {
        let e: f32 = x.iter().map(|v| v * v).sum::<f32>().max(1e-12);
        x.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>() / e
    }

    /// frequency from positive zero crossings
    fn freq_of(x: &[f32]) -> f32 {
        let c: Vec<usize> = (1..x.len())
            .filter(|&i| x[i - 1] <= 0.0 && x[i] > 0.0)
            .collect();
        if c.len() < 2 {
            return 0.0;
        }
        48000.0 * (c.len() - 1) as f32 / (c[c.len() - 1] - c[0]) as f32
    }

    // 120 BPM, speed 6: one row = 6000 frames, one tick = 1000 frames

    #[test]
    fn effect_38_39_set_cutoff_and_resonance_per_row() {
        let b = |fx: u8, fxp: u8| {
            let mut song = acid_song(saw_patch());
            song.patterns[0].cells[0][0] = note(37, fx, fxp);
            bright(&left(&render_secs(&song, 0.12))[1000..5000])
        };
        let plain = b(0, 0);
        let open = b(0x38, 0xD0);
        let shut = b(0x38, 0x30);
        assert!(
            open > 3.0 * plain && plain > 1.5 * shut,
            "{shut} {plain} {open}"
        );
        assert_eq!(b(0x38, 0x00), plain, "3800 = instrument cutoff");
        assert!((cutoff_from_code(1) - 20.0).abs() < 0.01);
        assert!((cutoff_from_code(0xFF) - 20000.0).abs() < 1.0);
        // resonance: the harmonic at the cutoff rings louder
        let res = |fxp: u8| {
            let mut song = acid_song(crate::synth::Patch {
                cutoff: 990.0,
                ..saw_patch()
            });
            song.patterns[0].cells[0][0] = note(34, 0x39, fxp); // A-2 110 Hz
            let x = left(&render_secs(&song, 0.15));
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in x[1000..5800].iter().enumerate() {
                let w = std::f64::consts::TAU * 990.0 * i as f64 / 48000.0;
                re += *v as f64 * w.cos();
                im += *v as f64 * w.sin();
            }
            (re * re + im * im).sqrt()
        };
        assert!(
            res(0xF0) > 3.0 * res(0x01),
            "{} vs {}",
            res(0xF0),
            res(0x01)
        );
    }

    #[test]
    fn effect_3a_3b_slide_the_cutoff_per_tick_and_a_new_instrument_resets() {
        let mut song = acid_song(saw_patch());
        song.patterns[0].cells[0][0] = note(37, 0x3A, 0x30); // +0.5 oct per tick
        let x = left(&render_secs(&song, 0.25));
        let early = bright(&x[200..900]);
        let late = bright(&x[5100..5900]); // 5 ticks later: +2.5 octaves
        assert!(late > 3.0 * early, "{early} -> {late}");
        // the slide holds after the row, and 3B brings it down again
        song.patterns[0].cells[1][0] = Cell {
            fx: 0x3B,
            fxp: 0x30,
            ..Default::default()
        };
        let y = left(&render_secs(&song, 0.25));
        assert!(bright(&y[11100..11900]) < 0.6 * bright(&y[5100..5900]));
        // a note with a different instrument clears the overrides
        let mut e = Engine::new(48000.0);
        let mut s2 = acid_song(saw_patch());
        s2.instruments[1] = s2.instruments[0].clone();
        s2.patterns[0].cells[0][0] = note(37, 0x38, 0xF0);
        s2.patterns[0].cells[1][0] = Cell {
            instr: 2,
            ..note(37, 0, 0)
        };
        e.start(&s2, 0, 0, false, 0);
        assert!(e.chans[0].fm.cut_abs > 0.0);
        let mut buf = vec![0.0f32; 6000 * 2 + 200];
        e.render(&s2, &mut buf);
        assert_eq!(e.chans[0].fm, crate::synth::FiltMod::default());
    }

    fn sine_patch() -> crate::synth::Patch {
        crate::synth::Patch {
            osc1: crate::synth::Wave::Sine,
            cutoff: 18000.0,
            ..saw_patch()
        }
    }

    #[test]
    fn legato_glides_between_held_notes_without_retrigger() {
        // C-3 (130.8 Hz) then C-4 (261.6 Hz) one row later, glide 100 ms
        let render = |legato: bool, fx: u8, fxp: u8| {
            let mut song = acid_song(crate::synth::Patch {
                legato,
                glide: 0.1,
                ..sine_patch()
            });
            song.patterns[0].cells[0][0] = note(37, 0, 0);
            song.patterns[0].cells[1][0] = Cell {
                instr: 0,
                ..note(49, fx, fxp)
            };
            left(&render_secs(&song, 0.4))
        };
        let g = render(true, 0, 0);
        let start = 6000;
        let f_mid = freq_of(&g[start + 600..start + 1800]);
        let f_end = freq_of(&g[start + 7000..start + 9000]);
        assert!(f_mid > 145.0 && f_mid < 250.0, "mid glide {f_mid}");
        assert!((f_end - 261.6).abs() < 3.0, "end {f_end}");
        // no retrigger: the level stays (no 5 ms declick dip / attack)
        let dip = rms(&g[start..start + 300]) / rms(&g[start - 600..start - 300]);
        assert!(dip > 0.9, "level ratio {dip}");
        // without legato it jumps at once
        let j = render(false, 0, 0);
        assert!((freq_of(&j[start + 600..start + 1800]) - 261.6).abs() < 5.0);
        // 3C01 glides on a non-legato patch; 3C00 forces the jump on a legato one
        let c = render(false, 0x3C, 0x01);
        let f = freq_of(&c[start + 600..start + 1800]);
        assert!(f > 145.0 && f < 250.0, "3C01 {f}");
        let c = render(true, 0x3C, 0x00);
        assert!((freq_of(&c[start + 600..start + 1800]) - 261.6).abs() < 5.0);
        // 3Cxx sets the time: 3C02 = 10 ms, done long before 3C40 = 320 ms
        let fast = render(false, 0x3C, 0x02);
        assert!((freq_of(&fast[start + 1200..start + 2400]) - 261.6).abs() < 4.0);
        let slow = render(false, 0x3C, 0x40);
        assert!(freq_of(&slow[start + 1200..start + 2400]) < 200.0);
    }

    #[test]
    fn glide_needs_a_held_note_and_slides_01_02_still_work() {
        let mut song = acid_song(crate::synth::Patch {
            legato: true,
            ..sine_patch()
        });
        song.patterns[0].cells[0][0] = note(37, 0, 0);
        song.patterns[0].cells[1][0] = Cell {
            note: NOTE_OFF,
            ..Default::default()
        };
        song.patterns[0].cells[2][0] = note(49, 0, 0);
        let x = left(&render_secs(&song, 0.4));
        let f = freq_of(&x[12600..13800]);
        assert!((f - 261.6).abs() < 5.0, "after note-off: no glide ({f})");
        // 01 slides up (1/192 octave per tick per unit)
        let mut song = acid_song(sine_patch());
        song.patterns[0].cells[0][0] = note(37, 0x01, 0x20);
        let x = left(&render_secs(&song, 0.15));
        let (a, b) = (freq_of(&x[100..900]), freq_of(&x[5100..5900]));
        let want = 130.8 * 2f32.powf(5.0 * 32.0 / 192.0);
        assert!(
            (a - 130.8).abs() < 4.0 && (b - want).abs() < 8.0,
            "{a} {b} {want}"
        );
    }

    #[test]
    fn accent_is_louder_brighter_and_shorter() {
        let p = crate::synth::Patch {
            env_amount: 0.4,
            amp: crate::synth::Adsr {
                a: 0.001,
                d: 0.2,
                s: 0.0,
                r: 0.05,
            },
            ..saw_patch()
        };
        let r = |p: &crate::synth::Patch, c: Cell| {
            let mut song = acid_song(p.clone());
            song.mixer.limiter.enabled = false;
            song.mixer.master = 0.2;
            song.patterns[0].cells[0][0] = c;
            left(&render_secs(&song, 0.3))
        };
        let plain = r(&p, note(37, 0, 0));
        let acc = r(&p, note(37, 0x3D, 0x00));
        let pk = |x: &[f32]| x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let db = 20.0 * (pk(&acc[..4000]) / pk(&plain[..4000])).log10();
        assert!(db > 2.0, "accent +{db:.1} dB");
        assert!(bright(&acc[200..2000]) > 1.3 * bright(&plain[200..2000]));
        // shorter: relative tail level 150 ms in is lower
        let tail = |x: &[f32]| rms(&x[7000..8000]) / pk(&x[..4000]);
        assert!(
            tail(&acc) < 0.8 * tail(&plain),
            "{} {}",
            tail(&acc),
            tail(&plain)
        );
        // velocity threshold: vol 40 accents, vol 20 does not
        let pv = crate::synth::Patch {
            accent_vel: 0.75,
            ..p.clone()
        };
        let hi = r(
            &pv,
            Cell {
                vol: 0x41,
                ..note(37, 0, 0)
            },
        );
        let full = r(
            &p,
            Cell {
                vol: 0x41,
                ..note(37, 0, 0)
            },
        );
        assert!(pk(&hi[..4000]) > 1.25 * pk(&full[..4000]));
        let lo = r(
            &pv,
            Cell {
                vol: 0x21,
                ..note(37, 0, 0)
            },
        );
        let lo_plain = r(
            &p,
            Cell {
                vol: 0x21,
                ..note(37, 0, 0)
            },
        );
        assert_eq!(lo, lo_plain);
    }

    #[test]
    fn acid_commands_and_limiter_are_allocation_free() {
        let mut song = acid_song(crate::synth::Patch {
            legato: true,
            accent_vel: 0.5,
            ..saw_patch()
        });
        song.mixer.limiter.gain_db = 12.0;
        let cmds = [
            (0x38, 0x60),
            (0x39, 0xC0),
            (0x3A, 0x10),
            (0x3B, 0x08),
            (0x3C, 0x01),
            (0x3D, 0x30),
        ];
        for (k, (f, p)) in cmds.iter().enumerate() {
            song.patterns[0].cells[k * 2][0] = note(37 + k as u8, *f, *p);
        }
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 1024];
        e.render(&song, &mut buf);
        let n = alloc_count::count(|| {
            for _ in 0..200 {
                e.render(&song, &mut buf);
            }
        });
        assert_eq!(n, 0);
        assert!(buf.iter().all(|x| x.is_finite()));
        assert!(e.loud.true_peak <= -1.0 + 1e-3, "{}", e.loud.true_peak);
        assert!(e.loud.gain_red < 0.0 || e.loud.momentary.is_finite());
    }

    #[test]
    fn master_limiter_holds_the_ceiling_on_the_demo_and_bypass_is_the_old_clipper() {
        let mut song = crate::fileio::demo_song();
        song.order.truncate(2);
        song.mixer.limiter.gain_db = 12.0;
        let out = render_song(&song, 44100);
        let tp = crate::master::lin_to_db(crate::master::true_peak(&out));
        assert!(tp <= -1.0 + 1e-3 && tp > -3.0, "{tp}");
        // bypass: exactly tanh(0.6 * master * x) as in 0.2.3.6
        song.mixer.limiter.enabled = false;
        song.mixer.limiter.gain_db = 0.0;
        let out = render_song(&song, 44100);
        assert!(out.iter().all(|x| x.abs() < 1.0));
    }

    /// widen a song to n channels (as the Channels box does)
    fn widen(song: &mut Song, n: usize) {
        song.channels = n;
        song.channel_names.resize(n, "Ch".into());
        for p in &mut song.patterns {
            for r in &mut p.cells {
                r.resize(n, Cell::default());
            }
        }
    }

    #[test]
    fn thirty_two_channels_all_play_and_mix_independently() {
        assert_eq!(MAX_CHANNELS, 32);
        let mut song = acid_song(saw_patch());
        widen(&mut song, MAX_CHANNELS);
        song.patterns[0].cells[0][31] = note(37, 0, 0);
        let a = rms(&render_secs(&song, 0.2));
        assert!(a > 1e-3, "{a}");
        song.mixer.channels[31].mute = true;
        let b = rms(&render_secs(&song, 0.2));
        assert!(b < a * 0.01, "mute ch 32: {b} vs {a}");
        song.mixer.channels[31].mute = false;
        song.patterns[0].cells[0][31] = Cell::default();
        song.patterns[0].cells[0][19] = note(37, 0, 0);
        song.mixer.channels[19].pan = -1.0;
        let out = render_secs(&song, 0.2);
        let (l, r): (Vec<f32>, Vec<f32>) = out.chunks(2).map(|f| (f[0], f[1])).unzip();
        assert!(rms(&r) < rms(&l) * 0.1, "pan ch 20 left");
        // solo on ch 30 silences ch 20
        song.mixer.channels[29].solo = true;
        assert!(rms(&render_secs(&song, 0.2)) < 1e-4);
    }

    #[test]
    fn thirty_two_busy_channels_render_fast_and_allocation_free() {
        let mut song = crate::fileio::demo_song();
        let n0 = song.channels;
        widen(&mut song, MAX_CHANNELS);
        // the demo's tracks repeated over all 32 channels
        for p in &mut song.patterns {
            for r in &mut p.cells {
                for c in n0..MAX_CHANNELS {
                    r[c] = r[c % n0];
                }
            }
        }
        song.order.truncate(4);
        let secs = 4.0;
        let t = std::time::Instant::now();
        let out = render_secs(&song, secs);
        let el = t.elapsed().as_secs_f32();
        eprintln!(
            "32 channels: {secs} s rendered in {el:.2} s ({:.1}x realtime)",
            secs / el
        );
        assert!(out.iter().all(|x| x.is_finite()));
        if !cfg!(debug_assertions) {
            assert!(el < secs * 0.5, "{el} s for {secs} s of 32 channels");
        }
        // (an insert chain's state is built the first time its instrument
        // plays on a channel, so warm up one pass first, then replay)
        let mut e = Engine::new(48000.0);
        let mut buf = vec![0.0f32; 1024];
        e.start(&song, 0, 0, false, 0);
        for _ in 0..(secs * 48000.0 / 512.0) as usize {
            e.render(&song, &mut buf);
        }
        e.start(&song, 0, 0, false, 0);
        let n = alloc_count::count(|| {
            for _ in 0..300 {
                e.render(&song, &mut buf);
            }
        });
        assert_eq!(n, 0, "allocations with 32 channels");
    }

    // ------------------------------------------- 0.2.3.16: PANIC

    fn tail_song() -> Song {
        // the demo with long delay + reverb returns turned up on every channel
        let mut s = crate::fileio::demo_song();
        for m in s.mixer.channels.iter_mut() {
            m.send = [0.8, 0.8];
        }
        for r in s.mixer.returns.iter_mut() {
            r.level = 1.0;
        }
        s
    }

    fn max_abs(b: &[f32]) -> f32 {
        b.iter().fold(0.0f32, |m, x| m.max(x.abs()))
    }

    #[test]
    fn panic_silences_playback_and_fx_tails_within_5_ms() {
        let song = tail_song();
        assert!(!song.mixer.returns.is_empty());
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 1024];
        let mut loud = 0.0f32;
        for _ in 0..200 {
            e.render(&song, &mut buf);
            loud = loud.max(max_abs(&buf));
        }
        assert!(loud > 0.05, "the song plays: {loud}");
        e.panic();
        assert!(e.panicking());
        // 5 ms = 240 frames: the 3 ms fade is over, then hard silence
        let mut first = vec![0.0f32; 480];
        e.render(&song, &mut first);
        assert!(!e.playing);
        assert!(!e.panicking());
        assert!(
            max_abs(&first[300..]) < 1e-6,
            "after the fade: {}",
            max_abs(&first[300..])
        );
        // the fade itself has no click: each step is small
        let steps = first[..300]
            .chunks(2)
            .zip(first[2..302].chunks(2))
            .map(|(a, b)| (a[0] - b[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(steps < 0.1, "fade step {steps}");
        // and no delay / reverb tail comes back afterwards
        for _ in 0..100 {
            e.render(&song, &mut buf);
            assert!(max_abs(&buf) < 1e-6, "tail after panic: {}", max_abs(&buf));
        }
    }

    #[test]
    fn panic_works_while_stopped_and_without_allocating() {
        let song = tail_song();
        let mut e = Engine::new(48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 1024];
        for _ in 0..200 {
            e.render(&song, &mut buf);
        }
        // stopped: the release and the reverb / delay tails still ring
        e.stop();
        e.render(&song, &mut buf);
        assert!(max_abs(&buf) > 1e-3, "tail rings after stop");
        let n = alloc_count::count(|| {
            e.panic();
            e.render(&song, &mut buf);
            e.render(&song, &mut buf);
        });
        assert_eq!(n, 0, "panic allocated {n} times");
        assert!(max_abs(&buf) < 1e-6);
        // playing again afterwards works
        e.start(&song, 0, 0, false, 0);
        let mut loud = 0.0f32;
        for _ in 0..100 {
            e.render(&song, &mut buf);
            loud = loud.max(max_abs(&buf));
        }
        assert!(loud > 0.05, "plays again after panic: {loud}");
    }

    #[test]
    fn panic_silences_plugin_voices() {
        let song = crate::plugin::tests::plugin_song();
        let mut e = Engine::new(48000.0);
        crate::plugin::prepare(&mut e.plugins, &song, 48000.0);
        e.start(&song, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 1024];
        for _ in 0..10 {
            e.render(&song, &mut buf);
        }
        assert!(max_abs(&buf) > 0.01);
        e.panic();
        e.render(&song, &mut buf);
        assert!(max_abs(&buf[600..]) < 1e-6, "{}", max_abs(&buf[600..]));
        e.render(&song, &mut buf);
        assert!(max_abs(&buf) < 1e-6);
    }
}
