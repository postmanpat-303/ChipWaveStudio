//! The metronome and the record count-in (Timeline live recording). A
//! short sine blip on every beat, higher on the bar. Real-time safe: plain
//! fields, no allocation, no locks. Off unless the UI turns it on, so
//! exports (their own engine) never hear it.
use super::Engine;

#[derive(Clone, Copy, Debug, Default)]
pub struct Metro {
    /// click on the beat while the timeline plays
    pub on: bool,
    /// timeline ticks per beat and per bar (from the song's signature)
    pub beat: i64,
    pub bar: i64,
    /// count-in: frames still to go before playback moves, and in all
    count_left: u32,
    count_all: u32,
    count_beat: u32,
    /// the click sounding now: frames left, phase, pitch
    click_left: u32,
    phase: f32,
    hz: f32,
}

const CLICK_S: f32 = 0.035;
const LEVEL: f32 = 0.28;

impl Metro {
    /// drop a count-in still to go
    pub fn cancel(&mut self) {
        self.count_left = 0;
    }

    /// a count-in is running
    pub(super) fn on_count(&self) -> bool {
        self.count_left > 0
    }

    fn click(&mut self, sr: f32, accent: bool) {
        self.click_left = (CLICK_S * sr) as u32;
        self.phase = 0.0;
        self.hz = if accent { 1760.0 } else { 1175.0 };
    }

    /// the next click sample (0 when none)
    fn sample(&mut self, sr: f32) -> f32 {
        if self.click_left == 0 {
            return 0.0;
        }
        let all = (CLICK_S * sr).max(1.0);
        let env = self.click_left as f32 / all;
        self.click_left -= 1;
        self.phase += self.hz / sr;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        (self.phase * std::f32::consts::TAU).sin() * env * env * LEVEL
    }
}

impl Engine {
    /// hold playback for `beats` beats of `secs_per_beat` and click them
    /// (call after `start`); 0 beats: none
    pub fn count_in(&mut self, beats: u32, secs_per_beat: f32) {
        let b = (secs_per_beat.max(0.05) * self.sr) as u32;
        self.metro.count_beat = b.max(1);
        self.metro.count_all = b * beats;
        self.metro.count_left = self.metro.count_all;
    }

    /// counting in: playback waits
    pub fn counting_in(&self) -> bool {
        self.playing && self.metro.count_left > 0
    }

    /// the count-in part of a callback: clicks only, the song waits.
    /// Returns how many frames it used.
    pub(super) fn metro_count(&mut self, out: &mut [f32]) -> usize {
        if !self.playing {
            self.metro.count_left = 0;
            return 0;
        }
        let n = (self.metro.count_left as usize).min(out.len() / 2);
        let sr = self.sr;
        let m = &mut self.metro;
        for f in 0..n {
            let el = m.count_all - m.count_left;
            if el % m.count_beat == 0 {
                m.click(sr, el == 0);
            }
            m.count_left -= 1;
            let v = m.sample(sr);
            out[2 * f] = v;
            out[2 * f + 1] = v;
        }
        n
    }

    /// clicks on the beats the timeline crossed in this callback (ticks
    /// `t0`..`t1` over `out`), mixed in
    pub(super) fn metro_mix(&mut self, t0: f64, t1: f64, out: &mut [f32]) {
        let frames = out.len() / 2;
        let sr = self.sr;
        let m = &mut self.metro;
        if frames == 0 {
            return;
        }
        let beat = m.beat.max(1) as f64;
        // the beat at or after t0 (a loop jump back: none this time)
        let mut next = if t1 > t0 {
            (t0 / beat).ceil() * beat
        } else {
            f64::INFINITY
        };
        for f in 0..frames {
            let t = t0 + (t1 - t0) * f as f64 / frames as f64;
            if t >= next {
                let k = (next / beat).round() as i64;
                let accent = m.bar > 0 && (k * m.beat) % m.bar == 0;
                m.click(sr, accent);
                next += beat;
            }
            if m.click_left > 0 {
                let v = m.sample(sr);
                out[2 * f] += v;
                out[2 * f + 1] += v;
            }
        }
    }
}
