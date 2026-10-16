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
    muted: bool,
}

impl Chan {
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
            muted: false,
        }
    }
}

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
        }
    }

    pub fn set_mutes(&mut self, mutes: &[bool]) {
        for (c, m) in self.chans.iter_mut().zip(mutes) {
            c.muted = *m;
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
        self.tick = 0;
        self.samples_to_tick = 0.0;
        self.speed = song.speed.max(1);
        self.bpm = song.bpm;
        self.song_ended = false;
        for c in &mut self.chans {
            c.active = false;
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
            c.note = cell.note;
            c.freq = note_freq(cell.note);
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

    /// Render interleaved stereo.
    pub fn render(&mut self, song: &Song, out: &mut [f32]) {
        let mut peak = self.peak * 0.97;
        for frame in out.chunks_mut(2) {
            if self.playing {
                self.samples_to_tick -= 1.0;
                if self.samples_to_tick <= 0.0 {
                    self.samples_to_tick += self.tick_len();
                    self.do_tick(song);
                }
            }
            let mut l = 0.0;
            let mut r = 0.0;
            for (i, c) in self
                .chans
                .iter_mut()
                .enumerate()
                .take(song.channels.min(MAX_CHANNELS))
            {
                if !c.active || c.instr >= song.instruments.len() {
                    continue;
                }
                let inst = &song.instruments[c.instr];
                let s = match &inst.kind {
                    InstrumentKind::Synth(p) => {
                        let v = c.synth.render(p, c.freq, self.sr);
                        if c.synth.amp.done() {
                            c.active = false;
                        }
                        v
                    }
                    InstrumentKind::Sample {
                        data,
                        rate,
                        base_note,
                        ..
                    } => {
                        let idx = c.pos as usize;
                        if idx + 1 >= data.len() {
                            c.active = false;
                            0.0
                        } else {
                            let fr = (c.pos - idx as f64) as f32;
                            let v = data[idx] * (1.0 - fr) + data[idx + 1] * fr;
                            let ratio = (c.freq / note_freq(*base_note)) as f64 * *rate as f64
                                / self.sr as f64;
                            c.pos += ratio;
                            let g = c.synth.amp.next(&SAMPLE_ENV, 1.0 / self.sr);
                            if c.synth.amp.done() {
                                c.active = false;
                            }
                            v * g
                        }
                    }
                };
                if c.muted {
                    continue;
                }
                let s = s * c.vol;
                // simple fixed pan: alternate channels slightly
                let pan = if song.channels > 1 {
                    (i % 2) as f32 * 0.3 - 0.15
                } else {
                    0.0
                };
                l += s * (1.0 - pan);
                r += s * (1.0 + pan);
            }
            // soft master limiter
            let l = (l * 0.6).tanh();
            let r = (r * 0.6).tanh();
            peak = peak.max(l.abs()).max(r.abs());
            frame[0] = l;
            if frame.len() > 1 {
                frame[1] = r;
            }
        }
        self.peak = peak;
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
    // 1.5 s tail for releases
    e.playing = false;
    for _ in 0..(sr as usize * 3 / 2 / 512) {
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
