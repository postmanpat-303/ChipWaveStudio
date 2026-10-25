//! Drum-pattern preview (Browser › Drum Patterns, docs/BROWSER.md): a small
//! step player that runs next to the song. While the transport plays it
//! waits for the next bar and then stays locked to the song position (song
//! tempo, seeks and loops followed); while stopped it loops on its own clock
//! at the song tempo. It plays its own kit (not a song instrument), mixed in
//! before the master, so the song is never changed. Engine thread: no
//! allocation once it is set up (the UI builds it).
use super::*;
use crate::timeline::{Tick, ROW};

const VOICES: usize = 16;

struct PVoice {
    synth: SynthVoice,
    freq: f32,
    vol: f32,
    key: u8,
    active: bool,
    age: u32,
}

pub struct PatPreview {
    patch: crate::gen::GenPatch,
    /// (tick in the pattern, kit note, gain), sorted
    ev: Vec<(f64, u8, f32)>,
    len: f64,
    /// bar length in ticks (the song's signature when it started)
    bar: f64,
    /// output gain 0..1.5
    pub vol: f32,
    /// absolute tick of step 0
    start: f64,
    /// transport state this alignment is for (None = not aligned yet)
    armed: Option<bool>,
    started: bool,
    clock: f64,
    prev_rel: f64,
    next: usize,
    voices: Vec<PVoice>,
    age: u32,
    seed: u32,
    /// drum pad Round Robin hit numbers per key
    hits: crate::drumkit::padrr::HitCount,
}

impl PatPreview {
    /// `ev`: (tick, kit note, velocity 1..=127); `len` and `bar` in ticks
    pub fn new(
        patch: crate::gen::GenPatch,
        ev: &[(Tick, u8, u8)],
        len: Tick,
        bar: Tick,
        vol: f32,
    ) -> Self {
        let mut e: Vec<(f64, u8, f32)> = ev
            .iter()
            .map(|(t, k, v)| (*t as f64, *k, *v as f32 / 127.0))
            .collect();
        e.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self {
            patch,
            ev: e,
            len: len.max(ROW) as f64,
            bar: bar.max(ROW) as f64,
            vol,
            start: 0.0,
            armed: None,
            started: false,
            clock: 0.0,
            prev_rel: -1.0,
            next: 0,
            voices: (0..VOICES)
                .map(|_| PVoice {
                    synth: SynthVoice::new(),
                    freq: 0.0,
                    vol: 0.0,
                    key: 0,
                    active: false,
                    age: 0,
                })
                .collect(),
            age: 0,
            seed: 0x51F1_5EED,
            hits: Default::default(),
        }
    }

    /// position in the pattern 0..1, or None while waiting for the bar
    pub fn progress(&self) -> Option<f32> {
        if !self.started {
            return None;
        }
        Some(((self.clock - self.start).rem_euclid(self.len) / self.len) as f32)
    }

    /// waiting for the next bar of the playing song
    pub fn waiting(&self) -> bool {
        !self.started
    }

    /// once per callback: follow the transport (`song_pos` in ticks when playing)
    fn sync(&mut self, playing: bool, song_pos: f64) {
        if self.armed != Some(playing) {
            self.armed = Some(playing);
            self.started = false;
            self.prev_rel = -1.0;
            self.next = 0;
            if playing {
                // quantised to the next bar (now, when exactly on a bar)
                let b = (song_pos / self.bar - 1e-6).ceil().max(0.0);
                self.start = b * self.bar;
                self.clock = song_pos;
            } else {
                self.start = 0.0;
                self.clock = 0.0;
            }
            return;
        }
        if playing {
            if (song_pos - self.clock).abs() > ROW as f64 {
                // seek / loop / block jump: pick up where the song now is
                self.clock = song_pos;
                if self.started {
                    let rel = (self.clock - self.start).rem_euclid(self.len);
                    self.next = self.ev.partition_point(|e| e.0 <= rel);
                    self.prev_rel = rel;
                }
            } else {
                self.clock = song_pos;
            }
        }
    }

    fn fire(&mut self, k: usize) {
        let (_, key, mut g) = self.ev[k];
        if let crate::gen::GenEngine::Drums(kit) = &self.patch.engine {
            // a Drum Kit: its own choke groups and velocity depth
            let grp;
            (g, grp) = crate::drumkit::hit(kit, note_freq(key), g);
            if grp > 0 {
                for v in self.voices.iter_mut().filter(|v| v.active) {
                    if crate::drumkit::choke_of(kit, v.freq) == grp {
                        v.synth.amp.release();
                    }
                }
            }
        } else if matches!(key % 12, 6 | 8) {
            // a closed hat chokes the open hat (as on one kit channel)
            for v in self
                .voices
                .iter_mut()
                .filter(|v| v.active && v.key % 12 == 10)
            {
                v.synth.amp.release();
            }
        }
        self.age = self.age.wrapping_add(1);
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let slot = self
            .voices
            .iter()
            .position(|v| !v.active)
            .unwrap_or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });
        let v = &mut self.voices[slot];
        v.synth = SynthVoice::seeded(self.seed);
        v.synth.gen.hit = self.hits.next(key);
        v.freq = note_freq(key);
        v.vol = 0.85 * g;
        v.key = key;
        v.active = true;
        v.age = self.age;
    }

    /// one stereo sample; `rate` = ticks per sample
    fn sample(&mut self, rate: f64, sr: f32, bpm: f32) -> (f32, f32) {
        self.clock += rate;
        if !self.started && self.clock >= self.start {
            self.started = true;
            self.prev_rel = -1.0;
            self.next = 0;
        }
        if self.started && !self.ev.is_empty() {
            let rel = (self.clock - self.start).rem_euclid(self.len);
            if rel < self.prev_rel {
                while self.next < self.ev.len() {
                    self.fire(self.next);
                    self.next += 1;
                }
                self.next = 0;
            }
            while self.next < self.ev.len() && self.ev[self.next].0 <= rel {
                self.fire(self.next);
                self.next += 1;
            }
            self.prev_rel = rel;
        }
        let (mut l, mut r) = (0.0, 0.0);
        let p = &self.patch;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            let (x, y) = v.synth.render_gen(&p.base, p, v.freq, sr, bpm);
            v.active = !v.synth.amp.done() && !v.synth.gen.done;
            l += x * v.vol;
            r += y * v.vol;
        }
        (l * self.vol, r * self.vol)
    }
}

impl Engine {
    /// set (Some) or stop (None) the drum-pattern preview
    pub fn set_pattern_preview(&mut self, p: Option<Box<PatPreview>>) -> Option<Box<PatPreview>> {
        std::mem::replace(&mut self.patprev, p)
    }

    pub fn pattern_preview(&self) -> Option<&PatPreview> {
        self.patprev.as_deref()
    }

    pub fn pattern_preview_mut(&mut self) -> Option<&mut PatPreview> {
        self.patprev.as_deref_mut()
    }

    /// callback start: align the preview with the transport
    pub(super) fn pp_begin(&mut self, song: &Song) -> f64 {
        if self.patprev.is_none() {
            return 0.0;
        }
        let playing = self.playing;
        let pos = if playing {
            self.calc_song_pos(song) as f64 * ROW as f64
        } else {
            0.0
        };
        let bpm = if playing {
            self.bpm
        } else {
            song.bpm.clamp(20.0, 400.0)
        };
        if let Some(pp) = self.patprev.as_deref_mut() {
            pp.sync(playing, pos);
        }
        // ticks per sample at that tempo
        bpm as f64 * crate::timeline::PPQ as f64 / 60.0 / self.sr as f64
    }

    pub(super) fn pp_sample(&mut self, rate: f64) -> (f32, f32) {
        let sr = self.sr;
        let bpm = self.bpm;
        match self.patprev.as_deref_mut() {
            Some(pp) => pp.sample(rate, sr, bpm),
            None => (0.0, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kick_only() -> (crate::gen::GenPatch, Vec<(Tick, u8, u8)>) {
        (
            crate::drumpat::kit_patch("909 Kit").unwrap(),
            vec![(0, 49, 127)],
        )
    }

    /// first frame louder than `th` in `out` (stereo)
    fn onset(out: &[f32], th: f32) -> Option<usize> {
        out.chunks(2).position(|f| f[0].abs() > th)
    }

    #[test]
    fn stopped_preview_loops_at_the_song_tempo() {
        let mut song = Song::empty();
        song.bpm = 120.0;
        let sr = 48_000.0;
        let mut e = Engine::new(sr);
        let (patch, ev) = kick_only();
        // one bar of 4/4 = 16 rows; a kick at 0 -> one hit per bar (2 s at 120)
        e.set_pattern_preview(Some(Box::new(PatPreview::new(
            patch,
            &ev,
            16 * ROW,
            16 * ROW,
            1.0,
        ))));
        let mut out = vec![0.0f32; (sr as usize) * 5 / 2 * 2];
        e.render(&song, &mut out);
        let lat = e.latency();
        let a = onset(&out, 1e-3).expect("first kick");
        assert!(a <= lat + 8, "starts at once: {a}");
        // the second kick, one bar (2 s) later
        let b0 = (sr as usize) * 2 - 200;
        let b = onset(&out[b0 * 2..], 0.05)
            .map(|x| x + b0)
            .expect("second kick");
        assert!(
            (b as i64 - (96_000 + lat) as i64).abs() < 400,
            "second kick at {b}"
        );
        assert!(e.pattern_preview().unwrap().progress().is_some());
        e.set_pattern_preview(None);
        let mut tail = vec![0.0f32; 4800];
        e.render(&song, &mut tail);
    }

    #[test]
    fn playing_preview_waits_for_the_next_bar_and_locks_to_the_song() {
        let mut song = Song::empty();
        song.bpm = 125.0;
        song.speed = 6;
        let sr = 48_000.0;
        let mut e = Engine::new(sr);
        e.start(&song, 0, 3, true, 0); // the song is 3 rows into the bar
        let (patch, ev) = kick_only();
        e.set_pattern_preview(Some(Box::new(PatPreview::new(
            patch,
            &ev,
            16 * ROW,
            16 * ROW,
            1.0,
        ))));
        let mut out = vec![0.0f32; 48_000 * 2 * 2];
        e.render(&song, &mut out);
        let row = sr as f64 * 15.0 / 125.0;
        let expect = (13.0 * row) as usize + e.latency();
        let a = onset(&out, 1e-3).expect("kick");
        assert!(
            (a as i64 - expect as i64).abs() <= 4,
            "first kick at {a}, the next bar is at {expect}"
        );
    }

    #[test]
    fn a_seek_follows_the_song_position() {
        let mut song = Song::empty();
        song.bpm = 140.0;
        let mut e = Engine::new(48_000.0);
        e.start(&song, 0, 0, true, 0);
        let (patch, ev) = kick_only();
        e.set_pattern_preview(Some(Box::new(PatPreview::new(
            patch,
            &ev,
            16 * ROW,
            16 * ROW,
            1.0,
        ))));
        let mut out = vec![0.0f32; 9600];
        e.render(&song, &mut out);
        let p0 = e.pattern_preview().unwrap().progress().unwrap();
        assert!(p0 < 0.1);
        // jump to row 8 (half a bar): the preview is half way too
        e.seek(&song, 0, 8);
        e.render(&song, &mut out);
        let p1 = e.pattern_preview().unwrap().progress().unwrap();
        assert!((p1 - 0.5).abs() < 0.08, "progress {p1}");
    }
}
