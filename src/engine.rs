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
fn voice(
    song: &Song,
    instr: usize,
    freq: f32,
    synth: &mut SynthVoice,
    pos: &mut f64,
    sr: f32,
    ov: &AutoState,
) -> (f32, bool) {
    let Some(inst) = song.instruments.get(instr) else {
        return (0.0, false);
    };
    let lvl = ov.level.get(instr).copied().unwrap_or(1.0);
    let (v, alive) = voice_raw(song, inst, instr, freq, synth, pos, sr, ov);
    (v * lvl, alive)
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
) -> (f32, bool) {
    match &inst.kind {
        InstrumentKind::Synth(p) => {
            let p = ov.patches.get(instr).and_then(|o| o.as_ref()).unwrap_or(p);
            let v = synth.render(p, freq, sr);
            (v, !synth.amp.done())
        }
        InstrumentKind::Sample {
            data,
            rate,
            base_note,
            ..
        } => {
            let idx = *pos as usize;
            if idx + 1 >= data.len() {
                return (0.0, false);
            }
            let fr = (*pos - idx as f64) as f32;
            let v = data[idx] * (1.0 - fr) + data[idx + 1] * fr;
            *pos += (freq / note_freq(*base_note)) as f64 * *rate as f64 / sr as f64;
            let g = synth.amp.next(&SAMPLE_ENV, 1.0 / sr);
            (v * g, !synth.amp.done())
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

    fn new() -> Self {
        Self {
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
    speed: u32,
    bpm: f32,
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
            speed: 6,
            bpm: 125.0,
            chans: (0..MAX_CHANNELS).map(|_| Chan::new()).collect(),
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
        }
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
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
        self.samples_to_tick = 0.0;
        self.speed = song.speed.max(1);
        self.bpm = song.bpm;
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

    fn trigger(&mut self, song: &Song, ch: usize, cell: &Cell) {
        let c = &mut self.chans[ch];
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
            c.synth = SynthVoice::new();
            c.pos = 0.0;
            c.active = true;
        }
        if cell.vol > 0 {
            c.vol = (cell.vol - 1) as f32 / 64.0;
        }
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
                0x0C => c.vol = (cell.fxp.min(0x40)) as f32 / 64.0,
                0x0D => c.vol_slide = (cell.fxp >> 4) as f32 - (cell.fxp & 15) as f32,
                0x0F if cell.fxp == 0 => brk = true,
                0x0F if cell.fxp <= 0xF0 => self.bpm = cell.fxp as f32,
                _ => {}
            }
        }
        if brk {
            self.row = pat.rows;
        } // jump to next pattern at advance
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
        for (k, lane) in song.automation.iter().enumerate() {
            if !lane.enabled {
                continue;
            }
            let (Some(t), Some(n)) = (Target::parse(&lane.target), value_at(&lane.points, pos))
            else {
                continue;
            };
            // smoothing: ~1.5 ms time constant at 32-frame steps
            let v = &mut a.vals[k];
            *v = if v.is_nan() { n } else { *v + (n - *v) * 0.5 };
            let real = t.to_real(*v);
            match t {
                Target::Cutoff(i) | Target::Resonance(i) | Target::Drive(i) => {
                    let Some(InstrumentKind::Synth(src)) = song.instruments.get(i).map(|x| &x.kind)
                    else {
                        continue;
                    };
                    let slot = &mut a.patches[i];
                    if i < used.len() && !used[i] {
                        // refresh from the song (keeps live UI edits), no realloc after the first
                        match slot {
                            Some(p) => p.clone_from(src),
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
                self.samples_to_tick -= 1.0;
                if self.samples_to_tick <= 0.0 {
                    self.samples_to_tick += self.tick_len();
                    self.do_tick(song);
                }
            }
            let mut l = 0.0;
            let mut r = 0.0;
            let mut bus = [[0.0f32; 2]; 2];
            let ctx = crate::fx::Ctx { bpm: self.bpm };
            let sr = self.sr;
            // 5 ms ramps for tails and mute changes
            let step = 1.0 / (0.005 * sr);
            for (i, (c, chain)) in self
                .chans
                .iter_mut()
                .zip(self.chains.iter_mut())
                .enumerate()
                .take(song.channels.min(MAX_CHANNELS))
            {
                let mut s = 0.0;
                if c.active {
                    let (v, alive) = voice(
                        song,
                        c.instr,
                        c.freq,
                        &mut c.synth,
                        &mut c.pos,
                        sr,
                        &self.auto,
                    );
                    c.active = alive;
                    s += v * c.vol;
                }
                if c.tail.gain > 0.0 {
                    let t = &mut c.tail;
                    let (v, alive) = voice(
                        song,
                        t.instr,
                        t.freq,
                        &mut t.synth,
                        &mut t.pos,
                        sr,
                        &self.auto,
                    );
                    s += v * t.vol * t.gain;
                    t.gain = if alive { (t.gain - step).max(0.0) } else { 0.0 };
                }
                let (tl, tr) = tgt[i];
                c.gl += (tl - c.gl) * k;
                c.gr += (tr - c.gr) * k;
                // insert chain: mono voice in, stereo out
                let (fl, fr) = if chain.states.is_empty() {
                    (s, s)
                } else {
                    chain.process(s, s, &ctx)
                };
                let (sl, sr2) = (fl * c.gl, fr * c.gr);
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
