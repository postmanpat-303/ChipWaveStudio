//! Audio-clip playback (M4, docs/TIMELINE.md "Audio clips"): a small voice
//! pool per track, fed by the plan's audio spans at sample accuracy on the
//! row clock. A file plays at its own speed: the voice steps through it at
//! `file rate / output rate` frames per sample (4-point Hermite), so export
//! at any rate sounds the same and tempo changes don't stretch it. Voices
//! hold only `Copy` data and read the decoded file through the plan, so
//! the audio thread never allocates or frees. Runs only when the timeline
//! has audio clips: every other song renders exactly as before.
use crate::model::MAX_CHANNELS;
use crate::timeline::{AudioSpan, Plan};

/// voices per track (overlapping clips, crossfades, release tails)
pub const AUDIO_VOICES: usize = 4;
/// de-click ramp when a clip is cut (seek, stop, loop wrap): ~1.5 ms at 44.1 kHz
const RAMP_SAMPLES: f32 = 64.0;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AudioVoice {
    pub(super) active: bool,
    span: AudioSpan,
    /// source frame (before the loop wrap)
    pos: f64,
    /// source frames per output sample
    step: f64,
    /// de-click gain 0..=1
    ramp: f32,
    /// fading out (seek, stop)
    rel: bool,
}

impl AudioVoice {
    /// one stereo sample at timeline tick `t`
    #[inline]
    pub(super) fn render(&mut self, plan: &Plan, t: f64) -> (f32, f32) {
        let Some(Some(pcm)) = plan.pcm.get(self.span.asset as usize) else {
            self.active = false;
            return (0.0, 0.0);
        };
        let p = if self.span.loop_len > 0.0 {
            self.pos.rem_euclid(self.span.loop_len)
        } else {
            self.pos
        };
        let (l, r) = pcm.read(p);
        let g = self.span.env(t) * self.ramp;
        self.pos += self.step;
        let d = 1.0 / RAMP_SAMPLES;
        if self.rel {
            self.ramp -= d;
            if self.ramp <= 0.0 {
                self.active = false;
            }
        } else if self.ramp < 1.0 {
            self.ramp = (self.ramp + d).min(1.0);
        }
        (l * g, r * g)
    }
}

pub(super) struct AudioState {
    /// AUDIO_VOICES per track, preallocated
    pub(super) v: Vec<AudioVoice>,
    /// per track: the next span to start
    cur: [usize; MAX_CHANNELS],
    /// timeline position (ticks) at the last sample
    pub(super) clock: f64,
    /// some voice is active (rendering is skipped otherwise)
    pub(super) any: bool,
    /// `Engine::start` is setting up (plan rebuilds don't start voices)
    pub(super) starting: bool,
}

impl AudioState {
    pub(super) fn new() -> Self {
        Self {
            v: vec![AudioVoice::default(); MAX_CHANNELS * AUDIO_VOICES],
            cur: [0; MAX_CHANNELS],
            clock: 0.0,
            any: false,
            starting: false,
        }
    }

    /// fade every voice out (stop, seek)
    pub(super) fn release_all(&mut self) {
        for v in self.v.iter_mut().filter(|v| v.active) {
            v.rel = true;
        }
    }

    pub(super) fn silence(&mut self) {
        for v in &mut self.v {
            v.active = false;
        }
        self.any = false;
    }

    /// Start span `s` on track `ch` at tick `t` (`ramp`: fade in, for a
    /// start inside the clip). A free voice, else the quietest fading one,
    /// else the one that started first.
    fn start(&mut self, plan: &Plan, ch: usize, s: &AudioSpan, t: f64, sr: f32, ramp: bool) {
        let base = ch * AUDIO_VOICES;
        let pool = &self.v[base..base + AUDIO_VOICES];
        let k = pool
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                pool.iter()
                    .enumerate()
                    .filter(|(_, v)| v.rel)
                    .min_by(|a, b| a.1.ramp.total_cmp(&b.1.ramp))
                    .map(|(k, _)| k)
            })
            .or_else(|| {
                pool.iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.span.start)
                    .map(|(k, _)| k)
            })
            .unwrap_or(0);
        self.v[base + k] = AudioVoice {
            active: true,
            span: *s,
            pos: plan.audio_frame(s, t),
            step: s.rate / sr.max(1.0) as f64,
            ramp: if ramp { 0.0 } else { 1.0 },
            rel: false,
        };
        self.any = true;
    }

    /// Put the cursors at tick `t`. `keep`: the plan was rebuilt while
    /// playing, so voices whose clip is unchanged play on (gain and fades
    /// follow the edit); otherwise every voice fades out. Clips under the
    /// playhead start mid-way with a short fade-in.
    pub(super) fn seek(&mut self, plan: &Plan, t: f64, sr: f32, playing: bool, keep: bool) {
        self.clock = t;
        for ch in 0..MAX_CHANNELS {
            let spans = plan.audio.get(ch).map_or(&[][..], |v| v.as_slice());
            self.cur[ch] = spans.partition_point(|s| (s.start as f64) < t);
            let base = ch * AUDIO_VOICES;
            for v in self.v[base..base + AUDIO_VOICES]
                .iter_mut()
                .filter(|v| v.active && !v.rel)
            {
                let m = keep
                    .then(|| {
                        spans.iter().find(|s| {
                            s.start == v.span.start
                                && s.asset == v.span.asset
                                && s.off == v.span.off
                                && s.loop_len == v.span.loop_len
                                && (s.end as f64) > t
                        })
                    })
                    .flatten();
                match m {
                    Some(s) => v.span = *s,
                    None => v.rel = true,
                }
            }
            if !playing {
                continue;
            }
            for s in &spans[..self.cur[ch]] {
                if (s.end as f64) <= t {
                    continue;
                }
                let base = ch * AUDIO_VOICES;
                let on = self.v[base..base + AUDIO_VOICES].iter().any(|v| {
                    v.active && !v.rel && v.span.start == s.start && v.span.asset == s.asset
                });
                if !on {
                    self.start(plan, ch, s, t, sr, true);
                }
            }
        }
    }

    /// Clip starts and ends due at timeline tick `t` (once per sample).
    pub(super) fn events(&mut self, plan: &Plan, nch: usize, t: f64, sr: f32) {
        self.clock = t;
        if self.any {
            for v in self.v[..nch * AUDIO_VOICES].iter_mut() {
                // clips end exactly at their end (as cut: a split plays
                // seamlessly; fades are the clip's own)
                if v.active && t >= v.span.end as f64 {
                    v.active = false;
                }
            }
        }
        for ch in 0..nch {
            let Some(spans) = plan.audio.get(ch) else {
                continue;
            };
            while let Some(s) = spans.get(self.cur[ch]) {
                if s.start as f64 > t {
                    break;
                }
                self.cur[ch] += 1;
                if (s.end as f64) > t {
                    let s = *s;
                    self.start(plan, ch, &s, t, sr, false);
                }
            }
        }
    }

    /// sounding audio voices on `track` (tests)
    #[cfg(test)]
    pub(super) fn sounding(&self, track: usize) -> usize {
        self.v[track * AUDIO_VOICES..(track + 1) * AUDIO_VOICES]
            .iter()
            .filter(|v| v.active && !v.rel)
            .count()
    }
}
