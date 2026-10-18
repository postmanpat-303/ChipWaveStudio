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
    send: [[Option<f32>; 2]; MAX_CHANNELS],
    /// parsed lane targets: (target string it was parsed from, result).
    /// Re-parsed only when a lane's target text changes.
    targets: Vec<(String, Option<crate::automation::Target>)>,
}

const FX_OVER_CAP: usize = 64;

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
                ..Default::default()
            },
            song_pos: 0.0,
            chains: (0..MAX_CHANNELS).map(|_| Default::default()).collect(),
            rets: (0..2).map(|_| Default::default()).collect(),
            sends: [[0.0; 2]; MAX_CHANNELS],
            test_tone: 0,
            tone_ph: 0.0,
            live: Live::new(),
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
                    synth: SynthVoice::new(),
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
        self.loop_pattern = loop_pattern;
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
        for c in &mut self.chans {
            if c.active {
                c.fade_to_tail();
            }
        }
        self.process_row(song);
    }

    pub fn stop(&mut self) {
        self.playing = false;
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
        self.trigger(song, ch, &cell);
    }

    fn tick_len(&self) -> f64 {
        // rows per beat = lines_per_beat; ticks per row = speed
        self.sr as f64 * 60.0 / (self.bpm as f64 * 4.0 * self.speed as f64)
    }

    /// frames of `beats` at the running tempo
    fn beat_frames(&self, beats: f32) -> usize {
        (beats as f64 * 60.0 / self.bpm.max(1.0) as f64 * self.sr as f64) as usize
    }

    fn trigger(&mut self, song: &Song, ch: usize, cell: &Cell) {
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
            c.instr = cell.instr as usize - 1;
            c.vol = song.instruments[c.instr].volume;
        }
        if cell.note == NOTE_OFF {
            c.synth.amp.release();
            c.synth.fenv.release();
        } else if cell.note > 0 && c.instr != usize::MAX {
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
        }
        if cell.vol > 0 {
            c.vol = (cell.vol - 1) as f32 / 64.0;
        }
        c.synth.vel = c.vol;
    }

    fn process_row(&mut self, song: &Song) {
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
            self.chans[ch].slide = 0.0;
            self.chans[ch].vol_slide = 0.0;
            self.trigger(song, ch, &cell);
            let c = &mut self.chans[ch];
            match cell.fx {
                0x01 => c.slide = cell.fxp as f32,
                0x02 => c.slide = -(cell.fxp as f32),
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
                _ => {}
            }
        }
        if brk {
            self.row = pat.rows;
        } // jump to next pattern at advance
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
        self.tick += 1;
        if self.tick >= self.speed {
            self.tick = 0;
            self.advance_row(song);
        } else {
            for c in &mut self.chans {
                if c.slide != 0.0 {
                    c.freq *= 2f32.powf(c.slide / 192.0);
                }
                if c.vol_slide != 0.0 {
                    c.vol = (c.vol + c.vol_slide / 64.0).clamp(0.0, 1.0);
                }
            }
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

    /// Evaluate all lanes at the current position and update the overrides.
    fn eval_auto(&mut self, song: &Song) {
        use crate::automation::{value_at, Target};
        let a = &mut self.auto;
        a.chan_vol = [None; MAX_CHANNELS];
        a.chan_pan = [None; MAX_CHANNELS];
        a.master = None;
        a.fx.clear();
        a.ret.clear();
        a.send = [[None; 2]; MAX_CHANNELS];
        let ni = song.instruments.len();
        if a.level.len() != ni {
            a.level.resize(ni, 1.0);
            a.patches.resize(ni, None);
        }
        a.level.iter_mut().for_each(|l| *l = 1.0);
        if !self.playing || song.automation.is_empty() {
            a.patches.iter_mut().for_each(|p| *p = None);
            a.vals.clear();
            return;
        }
        let pos = self.calc_song_pos(song);
        self.song_pos = pos;
        let a = &mut self.auto;
        if a.vals.len() != song.automation.len() {
            a.vals = vec![f32::NAN; song.automation.len()];
        }
        let mut used = [false; 256];
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
            let real = t.to_real(*v);
            match t {
                Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) => {
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
                        _ => p.drive = real,
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
                Target::Tempo => self.bpm = real.clamp(20.0, 400.0),
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
                _ => {}
            }
        }
        for (i, p) in self.auto.patches.iter_mut().enumerate() {
            if i >= used.len() || !used[i] {
                *p = None;
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
            for (d, (st, dev)) in ch.states.iter_mut().zip(devs).enumerate() {
                st.set_targets(dev, |p| {
                    a.fx.iter()
                        .find(|o| o.0 == owner && o.1 == d && o.2 == p)
                        .map(|o| o.3)
                });
            }
        }
        let lv = &mut self.live;
        if lv.owner != usize::MAX {
            let owner = lv.owner;
            let devs: &[crate::fx::Device] = song
                .instruments
                .get(owner)
                .map(|x| x.fx.as_slice())
                .unwrap_or(&[]);
            lv.chain.sync(devs, owner, sr);
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
        // live MIDI events (lock-free queue, filled by the MIDI thread)
        if let Some(mut rx) = self.live.rx.take() {
            while let Ok(ev) = rx.pop() {
                self.live_event(song, ev);
            }
            self.live.rx = Some(rx);
        }
        let live_on = self.live.v.iter().any(|v| v.active);
        let nch = song.channels.clamp(1, MAX_CHANNELS);
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
        self.eval_auto(song);
        self.refresh_fx(song);
        let (mut tgt, mut master) = self.mix_targets(song);
        let mut stgt = self.send_targets(song);
        let k = 1.0 - (-1.0 / (0.005 * self.sr)).exp();
        for frame in out.chunks_mut(2) {
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
            }
            let mut l = 0.0;
            let mut r = 0.0;
            let mut bus = [[0.0f32; 2]; 2];
            let ctx = crate::fx::Ctx { bpm: self.bpm };
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
                    lv.chain.process(s, s_r, &ctx)
                };
                (fl * live_gain.0, fr * live_gain.1)
            } else {
                (0.0, 0.0)
            };
            for (i, (c, chain)) in self
                .chans
                .iter_mut()
                .zip(self.chains.iter_mut())
                .enumerate()
                .take(song.channels.min(MAX_CHANNELS))
            {
                let (mut s, mut s_r) = (0.0, 0.0);
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
                    chain.process(s, s_r, &ctx)
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
            // master volume, then soft master limiter
            let l = (l * 0.6 * master).tanh();
            let r = (r * 0.6 * master).tanh();
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
    }
}

/// Render the whole song once (order list) offline.
pub fn render_song(song: &Song, sr: u32) -> Vec<f32> {
    let mut e = Engine::new(sr as f32);
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
        for &r in &rows {
            let at = r * 4725;
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
}
