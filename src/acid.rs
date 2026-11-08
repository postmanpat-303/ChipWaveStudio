// ChipWave Studio - GPL-3.0-or-later
//
// Glue around the GPL-3.0-only `openrb-303` voice. This file is ChipWave's:
// note mapping, the 3C slide and 3D accent, the knob panel, a level trim and
// resampling from the voice's 44100 Hz blocks. The voice arithmetic stays in
// third_party/openrb-303. See that crate's NOTICE. Linking the two means a
// ChipWave binary that includes Acid Bass is distributed under GPL-3.0 only
// (not legal advice). This file does not relicense any other ChipWave file.

//! Acid Bass: a monophonic bass instrument.
//!
//! The oscillator and filter are the OpenRB-338 bass-line voice, running at
//! 44100 Hz in blocks of 64. ChipWave resamples to the device rate. At
//! 44100 Hz each host sample is one voice sample. At any other rate the
//! voice still runs at 44100 Hz and the output is linearly interpolated, so
//! the pitch stays put and the tone is not claimed to match a 44100 render.
//! Tune at centre plays ChipWave C-2 as the voice's written C.
//!
//! Slide is the tracker's 3C (and Legato). The voice glides on its own curve,
//! one twelfth of the remaining distance per 64-sample block. 3Cxx's time
//! value is not that curve: 3C00 forces a fresh note, and any other 3C slides.
//! Accent is 3D. 3Dxx scales the Accent knob (40 is the whole knob, 00 is the
//! instrument's accent amount, which this preset sets to full).
//!
//! Not a hardware clone, and not affiliated with any instrument brand.

use openrb_303::{Bass, ACCENT, CUTOFF, DECAY, ENVMOD, RESONANCE, TUNE, WAVEFORM};

use crate::gen::{MacroDef, Ov};

/// Knobs on the patch, in `Target::Gen` order.
pub const N: usize = 8;
pub const K_TUNE: usize = 0;
pub const K_CUTOFF: usize = 1;
pub const K_RESO: usize = 2;
pub const K_ENV: usize = 3;
pub const K_DECAY: usize = 4;
pub const K_ACCENT: usize = 5;
pub const K_WAVE: usize = 6;
pub const K_MODEL: usize = 7;

const ENGINE_SR: f32 = 44_100.0;
/// Level trim in front of the mixer. The voice's own scale is large next to
/// ChipWave's other instruments. This is not part of the voice.
const TRIM: f32 = 1.0;
/// Semitone 0, tune at centre, sounds as C2. The pitch-table increment at
/// index 3072 is `110 Hz * 2^(3/12)` per sample, and the oscillator's phase
/// travels 2.0 per cycle, so the note is an octave below that increment.
/// `3072 + (tune + semis) * 64` stays inside the table for tune ±12 and
/// semis in -36..=35 (about C-1 to C5).
const ORIGIN_HZ: f64 = 65.40639132514965;
const SEMI_MIN: i32 = -36;
const SEMI_MAX: i32 = 35;

pub const KNOBS: [MacroDef; N] = [
    MacroDef {
        name: "Tune",
        lo: 0.0,
        hi: 127.0,
        exp: false,
        unit: "#",
        dice: (0.45, 0.55),
        tip: "fine tune, 64 is the centre",
    },
    MacroDef {
        name: "Cutoff",
        lo: 0.0,
        hi: 127.0,
        exp: false,
        unit: "#",
        dice: (0.05, 0.55),
        tip: "filter cutoff, 0 to 127. Automate this knob for a sweep",
    },
    MacroDef {
        name: "Resonance",
        lo: 0.0,
        hi: 127.0,
        exp: false,
        unit: "#",
        dice: (0.35, 0.9),
        tip: "filter resonance, 0 to 127",
    },
    MacroDef {
        name: "Env",
        lo: 0.0,
        hi: 127.0,
        exp: false,
        unit: "#",
        dice: (0.15, 0.85),
        tip: "how far the filter opens on a note, 0 to 127",
    },
    MacroDef {
        name: "Decay",
        lo: 0.0,
        hi: 127.0,
        exp: false,
        unit: "#",
        dice: (0.25, 0.9),
        tip: "how fast the filter closes, 0 to 127. An accented note uses a fixed decay",
    },
    MacroDef {
        name: "Accent",
        lo: 0.0,
        hi: 127.0,
        exp: false,
        unit: "#",
        dice: (0.3, 0.85),
        tip: "accent amount, 0 to 127. 3D in the Tracker accents a note; 3Dxx scales this knob",
    },
    MacroDef {
        name: "Wave",
        lo: 0.0,
        hi: 1.0,
        exp: false,
        unit: "",
        dice: (0.0, 1.0),
        tip: "below the centre is saw, from the centre up is square",
    },
    MacroDef {
        name: "Model",
        lo: 0.0,
        hi: 1.0,
        exp: false,
        unit: "",
        dice: (0.0, 1.0),
        tip: "below the centre is the 2.0 voice, from the centre up is Vintage",
    },
];

/// The upstream default bytes, as 0..1 knob positions.
pub fn defaults() -> [f32; N] {
    let n = |b: u8| b as f32 / 127.0;
    [
        n(0x40),
        n(0x14),
        n(0x5A),
        n(0x14),
        n(0x64),
        n(0x40),
        0.0,
        0.0,
    ]
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Patch {
    pub p: [f32; N],
    /// bit i locks knob i against Dice and Mutate
    pub locks: u64,
}

impl Default for Patch {
    fn default() -> Self {
        Self {
            p: defaults(),
            locks: 0,
        }
    }
}

impl Patch {
    pub fn fix(&mut self) {
        for x in &mut self.p {
            *x = if x.is_finite() {
                x.clamp(0.0, 1.0)
            } else {
                0.5
            };
        }
    }
    pub fn dice(&mut self, seed: u32) {
        let mut r = crate::gen::Rng::new(seed);
        for (i, def) in KNOBS.iter().enumerate() {
            if self.locks & (1 << i) == 0 {
                self.p[i] = def.dice.0 + (def.dice.1 - def.dice.0) * r.f();
            }
        }
    }
    pub fn mutate(&mut self, amount: f32, seed: u32) {
        let mut r = crate::gen::Rng::new(seed);
        for i in 0..N {
            if self.locks & (1 << i) == 0 {
                let x = r.f() * 2.0 - 1.0;
                self.p[i] = (self.p[i] + x * amount).clamp(0.0, 1.0);
            }
        }
    }
}

/// One voice plus the resampler. No heap, so it can live in the `Copy` generator state.
#[derive(Clone, Copy)]
pub struct Rt {
    voice: Bass,
    block: [f32; 64],
    /// next unread sample in `block`; 64 means the block is spent
    at: usize,
    prev: f32,
    cur: f32,
    /// fractional position between `prev` and `cur` for rates other than 44100
    phase: f64,
    primed: bool,
    last_semi: i32,
    held: bool,
    last_tune: u8,
    quiet: u32,
    pub finished: bool,
}

impl Default for Rt {
    fn default() -> Self {
        Self {
            voice: Bass::new(),
            block: [0.0; 64],
            at: 64,
            prev: 0.0,
            cur: 0.0,
            phase: 0.0,
            primed: false,
            last_semi: i32::MIN,
            held: false,
            last_tune: 255,
            quiet: 0,
            finished: false,
        }
    }
}

impl Rt {
    /// One host sample. `accent` is the tracker's 3D amount, 0 if this note is not accented.
    /// `gate` false is a note-off. A new pitch while the gate is still held slides.
    pub fn render(
        &mut self,
        p: &Patch,
        ov: Ov,
        freq: f32,
        sr: f32,
        gate: bool,
        accent: f32,
    ) -> f32 {
        let knobs = knobs(p, ov);
        self.apply(&knobs);
        let semi = semis(freq);
        if gate && freq.is_finite() && freq > 0.0 {
            let tune = byte(knobs[K_TUNE]);
            if !self.held || semi != self.last_semi {
                let slide = self.held;
                self.fire(semi, slide, accent);
                self.last_tune = tune;
            } else if tune != self.last_tune {
                let (note, up, down) = encode(semi);
                self.voice.retarget(note, up, down);
                self.last_tune = tune;
            }
            self.quiet = 0;
            self.finished = false;
        } else if self.held {
            self.voice.note_off();
            self.held = false;
        }
        let y = self.host(sr) * TRIM;
        let y = if y.is_finite() { y } else { 0.0 };
        if !gate {
            if y.abs() < 1.0e-5 {
                self.quiet = self.quiet.saturating_add(1);
            } else {
                self.quiet = 0;
            }
            // ~50 ms of silence ends the tail, so the channel can be reused
            let need = (sr.max(1.0) * 0.05) as u32;
            if self.quiet > need.max(64) {
                self.finished = true;
            }
        }
        y
    }

    fn fire(&mut self, semi: i32, slide: bool, accent: f32) {
        let (note, up, down) = encode(semi);
        let panel = self.voice.live[ACCENT];
        let accented = accent > 1.0e-4;
        if accented {
            let amt = ((panel as f32) * accent.clamp(0.0, 1.0)).round() as u8;
            self.voice.set_param(ACCENT as u8, amt);
        }
        // The voice glides when the *previous* note's slide flag is set, and
        // a 303 pattern puts that flag on the note you leave. ChipWave's 3C
        // is on the note you land on, so the flag is raised before this
        // note and cleared by it. The glide itself then runs until the pitch
        // arrives.
        if slide {
            self.voice.slide = true;
        }
        self.voice
            .note_on([note, 0, u8::from(accented), up, down, 0]);
        self.voice.set_param(ACCENT as u8, panel);
        self.held = true;
        self.last_semi = semi;
    }

    fn apply(&mut self, k: &[f32; N]) {
        let v = &mut self.voice;
        v.set_param(TUNE as u8, byte(k[K_TUNE]));
        v.set_param(CUTOFF as u8, byte(k[K_CUTOFF]));
        v.set_param(RESONANCE as u8, byte(k[K_RESO]));
        v.set_param(ENVMOD as u8, byte(k[K_ENV]));
        v.set_param(DECAY as u8, byte(k[K_DECAY]));
        v.set_param(ACCENT as u8, byte(k[K_ACCENT]));
        v.set_param(WAVEFORM as u8, u8::from(k[K_WAVE] >= 0.5));
        v.set_sound_model(k[K_MODEL] >= 0.5);
    }

    fn host(&mut self, sr: f32) -> f32 {
        let sr = if sr.is_finite() && sr >= 1.0 {
            sr
        } else {
            ENGINE_SR
        };
        if (sr - ENGINE_SR).abs() < 0.5 {
            return self.engine();
        }
        let step = (ENGINE_SR as f64 / sr as f64).clamp(0.0, 64.0);
        if !self.primed {
            self.cur = self.engine();
            self.prev = self.cur;
            self.primed = true;
            self.phase = 0.0;
        }
        self.phase += step;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.prev = self.cur;
            self.cur = self.engine();
        }
        let t = self.phase as f32;
        self.prev * (1.0 - t) + self.cur * t
    }

    fn engine(&mut self) -> f32 {
        if self.at >= 64 {
            self.voice.process(&mut self.block, openrb_303::tables());
            self.at = 0;
        }
        let y = self.block[self.at];
        self.at += 1;
        y
    }
}

fn knobs(p: &Patch, ov: Ov) -> [f32; N] {
    let mut k = p.p;
    for (i, slot) in k.iter_mut().enumerate() {
        if let Some(v) = ov.get(i) {
            if v.is_finite() {
                *slot = v.clamp(0.0, 1.0);
            }
        }
    }
    k
}

fn byte(v: f32) -> u8 {
    if !v.is_finite() {
        return 0;
    }
    (v.clamp(0.0, 1.0) * 127.0).round() as u8
}

/// Semitones above 110 Hz, clamped to the voice's table.
pub fn semis(freq: f32) -> i32 {
    if !freq.is_finite() || freq <= 0.0 {
        return 0;
    }
    let s = 12.0 * ((freq as f64) / ORIGIN_HZ).log2();
    if !s.is_finite() {
        return 0;
    }
    (s.round() as i32).clamp(SEMI_MIN, SEMI_MAX)
}

/// The factory instrument: Acid Bass, saw, 2.0 model, slide available via 3C.
pub fn preset() -> (&'static str, &'static str, crate::gen::GenPatch) {
    let mut g = crate::gen::GenPatch {
        engine: crate::gen::GenEngine::Acid(Patch::default()),
        ..Default::default()
    };
    // 3C01 reads this time to decide that the note slides. The voice then
    // glides on its own curve; the number is not the slide speed.
    g.base.glide = 0.06;
    g.base.legato = false;
    // 3D00 uses this, so an accent with no amount uses the whole Accent knob.
    g.base.accent = 1.0;
    g.base.gain = 1.0;
    g.filter_on = false;
    (
        "Acid Bass",
        "Monophonic bass. Slide with 3C, accent with 3D. The voice is from OpenRB-338.",
        g,
    )
}

/// `(note, octave up, octave down)` for [`Bass::note_on`].
fn encode(semis: i32) -> (u8, u8, u8) {
    let semis = semis.clamp(SEMI_MIN, SEMI_MAX);
    if semis >= 0 {
        (semis as u8, 0, 0)
    } else {
        let down = ((-semis) + 11) / 12;
        let note = semis + 12 * down;
        (note as u8, 0, down as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::Ov;

    fn patch(cut: f32, reso: f32, env: f32) -> Patch {
        let mut p = Patch::default();
        p.p[K_CUTOFF] = cut;
        p.p[K_RESO] = reso;
        p.p[K_ENV] = env;
        p.p[K_DECAY] = 0.7;
        p.p[K_ACCENT] = 100.0 / 127.0;
        p
    }

    fn run(p: &Patch, freq: f32, sr: f32, n: usize, accent: f32) -> Vec<f32> {
        let mut v = Rt::default();
        (0..n)
            .map(|_| v.render(p, Ov::NONE, freq, sr, true, accent))
            .collect()
    }

    /// Fundamental from the median rising-zero gap. The voice's saw crosses
    /// zero once per cycle once the mean is removed.
    fn hz(x: &[f32], sr: f32) -> f32 {
        if x.len() < 64 {
            return 0.0;
        }
        let mean = x.iter().sum::<f32>() / x.len() as f32;
        let mut last = 0usize;
        let mut gaps = Vec::new();
        for i in 1..x.len() {
            if x[i - 1] - mean <= 0.0 && x[i] - mean > 0.0 {
                if last > 0 {
                    gaps.push(i - last);
                }
                last = i;
            }
        }
        if gaps.len() < 2 {
            return 0.0;
        }
        gaps.sort_unstable();
        sr / gaps[gaps.len() / 2] as f32
    }

    #[test]
    fn native_rate_is_one_voice_sample_per_host_sample() {
        openrb_303::warm();
        let p = patch(0.8, 0.05, 0.1);
        let mut rt = Rt::default();
        let mut a = Vec::new();
        for _ in 0..64 {
            a.push(rt.render(&p, Ov::NONE, ORIGIN_HZ as f32, ENGINE_SR, true, 0.0));
        }
        let mut b = openrb_303::Bass::new();
        b.set_param(TUNE as u8, byte(p.p[K_TUNE]));
        b.set_param(CUTOFF as u8, byte(p.p[K_CUTOFF]));
        b.set_param(RESONANCE as u8, byte(p.p[K_RESO]));
        b.set_param(ENVMOD as u8, byte(p.p[K_ENV]));
        b.set_param(DECAY as u8, byte(p.p[K_DECAY]));
        b.set_param(ACCENT as u8, byte(p.p[K_ACCENT]));
        b.note_on([0, 0, 0, 0, 0, 0]);
        let mut block = [0.0; 64];
        b.process(&mut block, openrb_303::tables());
        for (i, (x, y)) in a.iter().zip(block).enumerate() {
            assert!(
                (x - y * TRIM).abs() < 1.0e-5,
                "sample {i}: {x} vs {}",
                y * TRIM
            );
        }
    }

    #[test]
    fn slide_moves_and_a_fresh_note_does_not() {
        openrb_303::warm();
        // cutoff wide open and resonance down, so the oscillator (not the
        // filter peak) is what the pitch measurement hears
        let p = patch(1.0, 0.0, 0.0);
        let sr = ENGINE_SR;
        let c3 = ORIGIN_HZ as f32;
        let c4 = (ORIGIN_HZ * 2.0) as f32;
        let mut slide = Rt::default();
        let mut head = Vec::new();
        for _ in 0..(sr * 0.25) as usize {
            head.push(slide.render(&p, Ov::NONE, c3, sr, true, 0.0));
        }
        let n_mid = (sr * 0.03) as usize;
        let mut mid = Vec::new();
        for _ in 0..n_mid {
            mid.push(slide.render(&p, Ov::NONE, c4, sr, true, 0.0));
        }
        let mut late = Vec::new();
        for _ in 0..(sr * 0.5) as usize {
            late.push(slide.render(&p, Ov::NONE, c4, sr, true, 0.0));
        }
        // a voice that stays on C2, and one that starts on C3, over the same span
        let mut stay = Rt::default();
        let mut jumped = Rt::default();
        for _ in 0..head.len() {
            let _ = stay.render(&p, Ov::NONE, c3, sr, true, 0.0);
            let _ = jumped.render(&p, Ov::NONE, c4, sr, true, 0.0);
        }
        let mut stayed = Vec::new();
        let mut hopped = Vec::new();
        for _ in 0..n_mid {
            stayed.push(stay.render(&p, Ov::NONE, c3, sr, true, 0.0));
            hopped.push(jumped.render(&p, Ov::NONE, c4, sr, true, 0.0));
        }
        let dist = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>();
        let f0 = hz(&head[head.len() / 2..], sr);
        let f2 = hz(&late[late.len() / 2..], sr);
        assert!((f0 - c3).abs() < 4.0, "start {f0}");
        assert!(dist(&mid, &stayed) > 0.5, "slide did not leave C2");
        assert!(dist(&mid, &hopped) > 0.5, "slide jumped instead of gliding");
        assert!((f2 - c4).abs() < 8.0, "arrived {f2}");
        let jump = run(&p, c4, sr, (sr * 0.08) as usize, 0.0);
        let fj = hz(&jump[jump.len() / 2..], sr);
        assert!((fj - c4).abs() < 6.0, "fresh note {fj}");
    }

    #[test]
    fn accent_is_louder_than_the_same_note() {
        openrb_303::warm();
        let p = patch(0.15, 0.75, 0.9);
        let n = (ENGINE_SR * 0.1) as usize;
        let plain = run(&p, 110.0, ENGINE_SR, n, 0.0);
        let acc = run(&p, 110.0, ENGINE_SR, n, 1.0);
        let peak = |x: &[f32]| x.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        assert!(
            peak(&acc) > peak(&plain) * 1.15,
            "{} vs {}",
            peak(&acc),
            peak(&plain)
        );
    }

    #[test]
    fn two_voices_do_not_hear_each_other() {
        openrb_303::warm();
        let p = patch(0.7, 0.2, 0.3);
        let mut a = Rt::default();
        let mut b = Rt::default();
        let mut solo = Rt::default();
        for _ in 0..2000 {
            let x = a.render(&p, Ov::NONE, 110.0, ENGINE_SR, true, 0.0);
            let _ = b.render(&p, Ov::NONE, 220.0, ENGINE_SR, true, 1.0);
            let s = solo.render(&p, Ov::NONE, 110.0, ENGINE_SR, true, 0.0);
            assert_eq!(x, s);
        }
    }

    #[test]
    fn extreme_knobs_and_other_rates_stay_finite() {
        openrb_303::warm();
        for model in [0.0_f32, 1.0] {
            for fill in [0.0_f32, 1.0] {
                let mut p = Patch::default();
                p.p.fill(fill);
                p.p[K_MODEL] = model;
                for &sr in &[11_025.0, 44_100.0, 48_000.0, 96_000.0] {
                    let y = run(&p, 55.0, sr, (sr * 0.05) as usize, 1.0);
                    assert!(y.iter().all(|v| v.is_finite()), "sr {sr} fill {fill}");
                    let peak = y.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
                    assert!(peak < 4.0, "level {peak} at {sr}");
                }
            }
        }
    }

    #[test]
    fn note_off_goes_quiet_and_a_cutoff_move_is_audible() {
        openrb_303::warm();
        let p = patch(0.4, 0.4, 0.5);
        let mut v = Rt::default();
        for _ in 0..(ENGINE_SR * 0.05) as usize {
            let _ = v.render(&p, Ov::NONE, 110.0, ENGINE_SR, true, 0.0);
        }
        let mut tail = Vec::new();
        for _ in 0..(ENGINE_SR * 0.4) as usize {
            tail.push(v.render(&p, Ov::NONE, 110.0, ENGINE_SR, false, 0.0));
        }
        let end = &tail[tail.len() - 1000..];
        let peak = end.iter().map(|x| x.abs()).fold(0.0_f32, f32::max);
        assert!(peak < 1.0e-3, "tail {peak}");
        assert!(v.finished);
        let open = run(&patch(0.95, 0.15, 0.2), 110.0, ENGINE_SR, 4000, 0.0);
        let shut = run(&patch(0.02, 0.15, 0.0), 110.0, ENGINE_SR, 4000, 0.0);
        let diff: f32 = open.iter().zip(&shut).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 1.0, "cutoff did nothing ({diff})");
    }

    #[test]
    fn c3_is_semitone_zero() {
        assert_eq!(semis(ORIGIN_HZ as f32), 0);
        assert_eq!(semis((ORIGIN_HZ * 2.0) as f32), 12);
        assert_eq!(semis((ORIGIN_HZ / 2.0) as f32), -12);
        assert_eq!(semis(110.0), 9);
        assert_eq!(encode(-12), (0, 0, 1));
    }

    fn open_filter(mut song: crate::model::Song) -> crate::model::Song {
        use crate::model::InstrumentKind;
        if let InstrumentKind::Gen(g) = &mut song.instruments[0].kind {
            if let crate::gen::GenEngine::Acid(a) = &mut g.engine {
                a.p[K_CUTOFF] = 1.0;
                a.p[K_RESO] = 0.0;
                a.p[K_ENV] = 0.0;
            }
        }
        song
    }

    fn song_with(notes: &[(usize, u8, u8, u8)], second: bool) -> crate::model::Song {
        use crate::model::{Cell, Instrument, InstrumentKind, Pattern};
        let mut song = crate::model::Song::empty();
        song.bpm = 125.0;
        song.speed = 1;
        song.patterns = vec![Pattern::new("acid", 8, 4)];
        song.mixer.limiter.enabled = false;
        let (_, _, g) = preset();
        let mk = |g| Instrument {
            name: "Acid Bass".into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Gen(g),
            arp: None,
        };
        song.instruments.push(mk(g.clone()));
        if second {
            song.instruments.push(mk(g));
        }
        for (row, note, fx, fxp) in notes {
            song.patterns[0].cells[*row][0] = Cell {
                note: *note,
                instr: 1,
                fx: *fx,
                fxp: *fxp,
                ..Default::default()
            };
        }
        song
    }

    fn left(x: &[f32]) -> Vec<f32> {
        x.iter().step_by(2).copied().collect()
    }

    #[test]
    fn tracker_slide_and_accent_and_a_saved_song_come_back() {
        use crate::automation::{Lane, Point, Shape, Target};
        let slide = open_filter(song_with(&[(0, 37, 0, 0), (2, 49, 0x3C, 0x01)], false));
        let jump = open_filter(song_with(&[(0, 37, 0, 0), (2, 49, 0, 0)], false));
        let sr = 44_100u32;
        let sl = left(&crate::engine::render_song(&slide, sr));
        let ju = left(&crate::engine::render_song(&jump, sr));
        assert!(sl.iter().all(|v| v.is_finite()));
        // row is a sixteenth at 125 bpm: 0.12 s. Row 2 starts at 0.24 s.
        let at = (sr as f32 * 0.24) as usize;
        let win = sr as usize / 50; // 20 ms, just after the new note
        let fs = hz(&sl[at + win..at + win * 3], sr as f32);
        let fj = hz(&ju[at + win..at + win * 3], sr as f32);
        assert!(fj > 240.0, "jump should already be C4 ({fj})");
        assert!(fs < fj - 15.0, "slide {fs} should lag the jump {fj}");
        let plain = song_with(&[(0, 37, 0, 0)], false);
        let accent = song_with(&[(0, 37, 0x3D, 0x40)], false);
        let peak = |s: &crate::model::Song| {
            left(&crate::engine::render_song(s, sr))
                .iter()
                .take((sr as f32 * 0.08) as usize)
                .map(|v| v.abs())
                .fold(0.0_f32, f32::max)
        };
        assert!(peak(&accent) > peak(&plain) * 1.1, "3D should be louder");
        // cutoff automation changes the render
        let mut swept = plain.clone();
        let mut lane = Lane::new(Target::Gen(0, K_CUTOFF), "Cutoff".into());
        lane.points.push(Point {
            pos: 0.0,
            value: 0.95,
            shape: Shape::Step,
            curve: 0.0,
        });
        swept.automation.push(lane);
        let a = left(&crate::engine::render_song(&plain, sr));
        let b = left(&crate::engine::render_song(&swept, sr));
        let diff: f32 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum();
        assert!(diff > 1.0, "cutoff lane did nothing ({diff})");
        // round trip, and only an Acid Bass song asks for reader 9
        let dir = std::env::temp_dir().join(format!("cw_acid_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("acid.cwt");
        crate::fileio::save_cwt(&swept, &path).unwrap();
        let back = crate::fileio::load_cwt(&path).unwrap();
        let again = left(&crate::engine::render_song(&back, sr));
        assert_eq!(b, again);
        let mut man = String::new();
        {
            use std::io::Read;
            let f = std::fs::File::open(&path).unwrap();
            let mut z = zip::ZipArchive::new(f).unwrap();
            z.by_name("manifest.toml")
                .unwrap()
                .read_to_string(&mut man)
                .unwrap();
        }
        assert!(man.contains("min_reader_version = 9"), "{man}");
        let old = dir.join("empty.cwt");
        crate::fileio::save_cwt(&crate::model::Song::empty(), &old).unwrap();
        let mut man = String::new();
        {
            use std::io::Read;
            let f = std::fs::File::open(&old).unwrap();
            let mut z = zip::ZipArchive::new(f).unwrap();
            z.by_name("manifest.toml")
                .unwrap()
                .read_to_string(&mut man)
                .unwrap();
        }
        assert!(man.contains("min_reader_version = 1"), "{man}");
        std::fs::remove_dir_all(&dir).ok();
        if let Ok(path) = std::env::var("ACID_DEMO_CWT") {
            let path = std::path::PathBuf::from(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            crate::fileio::save_cwt(&demo_tune(), &path).unwrap();
        }
    }

    /// A short tune: legato slides, 3D accents, and a cutoff lane. Not a
    /// bundled demo. Written only when ACID_DEMO_CWT is set.
    fn demo_tune() -> crate::model::Song {
        use crate::automation::{Lane, Point, Shape, Target};
        use crate::model::InstrumentKind;
        // C2, D2, F2, G2, A#2, C3, then a rest. One effect column, so the
        // slide is Legato and the accent is 3D on the marked notes.
        let notes = [
            (0usize, 25u8, 0u8, 0u8),
            (4, 27, 0x3D, 0x40),
            (8, 30, 0, 0),
            (12, 32, 0x3D, 0x20),
            (16, 34, 0, 0),
            (20, 37, 0x3D, 0x40),
            (24, 255, 0, 0),
        ];
        let mut song = song_with(&[], false);
        song.patterns = vec![crate::model::Pattern::new("acid", 32, 4)];
        for (row, note, fx, fxp) in notes {
            song.patterns[0].cells[row][0] = crate::model::Cell {
                note,
                instr: 1,
                fx,
                fxp,
                ..Default::default()
            };
        }
        song.title = "Acid Bass test".into();
        song.bpm = 120.0;
        if let InstrumentKind::Gen(g) = &mut song.instruments[0].kind {
            g.base.legato = true;
            if let crate::gen::GenEngine::Acid(a) = &mut g.engine {
                a.p[K_CUTOFF] = 0.12;
                a.p[K_RESO] = 0.75;
                a.p[K_ENV] = 0.55;
                a.p[K_DECAY] = 0.45;
                a.p[K_ACCENT] = 0.7;
            }
        }
        let mut lane = Lane::new(Target::Gen(0, K_CUTOFF), "Cutoff".into());
        lane.points.push(Point {
            pos: 0.0,
            value: 0.12,
            shape: Shape::Linear,
            curve: 0.0,
        });
        lane.points.push(Point {
            pos: 24.0,
            value: 0.82,
            shape: Shape::Linear,
            curve: 0.0,
        });
        song.automation.push(lane);
        song
    }

    #[test]
    fn two_channels_do_not_share_a_voice_and_other_rates_match_offline() {
        let mut both = song_with(&[(0, 37, 0, 0)], true);
        both.patterns[0].cells[0][1] = crate::model::Cell {
            note: 49,
            instr: 2,
            fx: 0x3D,
            fxp: 0x40,
            ..Default::default()
        };
        both.mixer.channels[1].volume = 0.0;
        let solo = song_with(&[(0, 37, 0, 0)], false);
        for sr in [11_025u32, 44_100, 48_000, 96_000] {
            let mix = crate::engine::render_song(&both, sr);
            let one = crate::engine::render_song(&solo, sr);
            assert!(mix.iter().all(|v| v.is_finite()), "{sr}");
            assert_eq!(mix.len(), one.len(), "{sr}");
            let err: f32 = mix.iter().zip(&one).map(|(a, b)| (a - b).abs()).sum();
            assert!(err < 1.0e-2, "crosstalk at {sr}: {err}");
            // offline twice is the same render
            assert_eq!(mix, crate::engine::render_song(&both, sr));
        }
    }
}
