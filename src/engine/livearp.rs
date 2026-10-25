//! The Arp on live keys (src/arp.rs): the keys held on an instrument with
//! an Arp go to its stepper instead of sounding; the stepper's notes play
//! as live notes of their own (ids `OUT_BASE` + slot). It steps on the song
//! position while the song plays and on a free clock at the song tempo
//! while stopped; a jump of the clock lets its notes go (no stuck notes).
use super::*;
use crate::arp::{Held, Hits, Mods, Stepper};
use crate::midi::LiveEv;

/// live note ids of the Arp's notes: OUT_BASE + slot (MIDI keys use
/// 0..=127, the computer keyboard 180 and up)
pub(super) const OUT_BASE: u8 = 128;
const OUTS: usize = 32;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Dom {
    Free,
    Timeline,
    Rows,
}

pub(super) struct LiveArp {
    st: Stepper,
    /// the instrument slot (1-based) and mixer channel of the held keys
    instr: u8,
    ch: u8,
    /// per output slot: the tick its note ends (NaN = free)
    off: [f64; OUTS],
    next: usize,
    dom: Dom,
    clock: f64,
    /// the free clock (ticks) while stopped
    free: f64,
    hits: Hits,
    /// keys held or notes sounding (nothing runs otherwise)
    pub(super) active: bool,
    /// the notes played (tests): tracker note
    #[cfg(test)]
    log: Vec<u8>,
}

impl LiveArp {
    pub(super) fn new() -> Self {
        Self {
            st: Stepper::default(),
            instr: 0,
            ch: 0,
            off: [f64::NAN; OUTS],
            next: 0,
            dom: Dom::Free,
            clock: 0.0,
            free: 0.0,
            hits: Hits::default(),
            active: false,
            #[cfg(test)]
            log: vec![],
        }
    }

    /// forget everything (panic: the voices are silenced anyway)
    pub(super) fn reset(&mut self) {
        self.st.clear();
        self.off = [f64::NAN; OUTS];
        self.active = false;
    }
}

impl Engine {
    fn larp_clock(&self) -> (Dom, f64) {
        if !self.playing {
            (Dom::Free, self.larp.free)
        } else if self.tl.on {
            (Dom::Timeline, self.tl_clock())
        } else {
            (Dom::Rows, self.launch_clock())
        }
    }

    /// let every Arp note go
    fn larp_flush(&mut self, song: &Song) {
        for s in 0..OUTS {
            if !self.larp.off[s].is_nan() {
                self.larp.off[s] = f64::NAN;
                self.live_event_raw(
                    song,
                    LiveEv::Off {
                        key: OUT_BASE + s as u8,
                    },
                );
            }
        }
    }

    /// One live event: true when the Arp takes it (a key on an instrument
    /// with an Arp).
    pub(super) fn larp_take(&mut self, song: &Song, ev: LiveEv) -> bool {
        match ev {
            LiveEv::On {
                key,
                note,
                vel,
                instr,
                ch,
            } => {
                let i = (instr as usize).wrapping_sub(1);
                let Some(a) = song.instruments.get(i).and_then(|x| x.arp.as_ref()) else {
                    return false;
                };
                let latch = a.latch;
                if self.larp.active && (self.larp.instr != instr || self.larp.ch != ch) {
                    // another instrument: its own chord
                    self.larp_flush(song);
                    self.larp.st.clear();
                }
                let (dom, mut clock) = self.larp_clock();
                if !self.larp.active || self.larp.st.is_empty() {
                    if dom == Dom::Free && !self.larp.active {
                        // stopped: the first key starts the free clock
                        self.larp.free = 0.0;
                        clock = 0.0;
                    }
                    self.larp.dom = dom;
                    self.larp.clock = clock;
                }
                if self.larp.dom != dom {
                    clock = self.larp.clock;
                }
                self.larp.instr = instr;
                self.larp.ch = ch;
                self.larp.st.idx = i;
                self.larp.st.add(
                    latch,
                    Held {
                        key: note,
                        vel,
                        instr,
                        start: clock,
                        end: f64::INFINITY,
                        id: key,
                        ..Default::default()
                    },
                );
                self.larp.active = true;
                true
            }
            LiveEv::Off { key } => {
                if self.larp.active {
                    let latch = song
                        .instruments
                        .get(self.larp.st.idx)
                        .and_then(|x| x.arp.as_ref())
                        .is_some_and(|a| a.latch);
                    self.larp.st.release(key, latch);
                }
                false
            }
            LiveEv::AllOff => {
                if self.larp.active {
                    self.larp_flush(song);
                    self.larp.reset();
                }
                false
            }
            _ => false,
        }
    }

    /// once per sample while the Arp is active: its notes due
    pub(super) fn larp_tick(&mut self, song: &Song) {
        let i = self.larp.st.idx;
        let inst = song.instruments.get(i);
        let Some(a) = inst.and_then(|x| x.arp.as_ref()) else {
            // switched off: its notes go
            self.larp_flush(song);
            self.larp.reset();
            return;
        };
        let kit = inst.is_some_and(|x| crate::drumkit::kit_of(x).is_some());
        let (dom, clock) = self.larp_clock();
        if dom == Dom::Free {
            self.larp.free += self.bpm.max(1.0) as f64 * crate::timeline::PPQ as f64
                / 60.0
                / self.sr.max(1.0) as f64;
        }
        let jump = dom != self.larp.dom
            || clock < self.larp.clock - 1e-6
            || clock > self.larp.clock + 4.0 * crate::timeline::PPQ as f64;
        if jump {
            self.larp_flush(song);
            self.larp.st.rebase(clock);
            self.larp.dom = dom;
        }
        self.larp.clock = clock;
        for s in 0..OUTS {
            if self.larp.off[s] <= clock {
                self.larp.off[s] = f64::NAN;
                self.live_event_raw(
                    song,
                    LiveEv::Off {
                        key: OUT_BASE + s as u8,
                    },
                );
            }
        }
        let lv = &mut self.larp;
        lv.st.advance(a, &Mods::default(), kit, clock, &mut lv.hits);
        for k in 0..self.larp.hits.n {
            let h = self.larp.hits.h[k];
            let s = self.larp.next;
            self.larp.next = (s + 1) % OUTS;
            let key = OUT_BASE + s as u8;
            if !self.larp.off[s].is_nan() {
                self.live_event_raw(song, LiveEv::Off { key });
            }
            self.larp.off[s] = h.end;
            #[cfg(test)]
            self.larp.log.push(h.key);
            let ch = self.larp.ch;
            self.live_event_raw(
                song,
                LiveEv::On {
                    key,
                    note: h.key,
                    vel: h.vel,
                    instr: h.instr,
                    ch,
                },
            );
        }
        self.larp.active = !self.larp.st.is_empty() || self.larp.off.iter().any(|x| !x.is_nan());
    }

    /// the Arp holds live keys (the UI's readout; tests)
    pub fn live_arp_holding(&self) -> bool {
        self.larp.active && !self.larp.st.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;
    /// 120 BPM at 48 kHz: a 1/16 step is 6 000 frames
    const STEP: usize = 6000;

    fn song() -> Song {
        let mut song = crate::fileio::demo_song();
        song.automation.clear();
        song.bpm = 120.0;
        song.instruments[0].kind = InstrumentKind::Synth(crate::synth::Patch {
            amp: crate::synth::Adsr {
                a: 0.001,
                d: 0.1,
                s: 0.8,
                r: 0.01,
            },
            ..Default::default()
        });
        song.instruments[0].arp = Some(Default::default());
        song
    }

    fn on(key: u8) -> LiveEv {
        LiveEv::On {
            key,
            note: crate::midi::tracker_note(key, 0).unwrap(),
            vel: 0.8,
            instr: 1,
            ch: 0,
        }
    }

    fn run(e: &mut Engine, s: &Song, frames: usize) {
        let mut b = vec![0.0f32; 256];
        for _ in 0..frames.div_ceil(128) {
            e.render(s, &mut b);
        }
    }

    fn down(e: &Engine) -> usize {
        e.live.v.iter().filter(|v| v.active && v.down).count()
    }

    fn n(key: u8) -> u8 {
        crate::midi::tracker_note(key, 0).unwrap()
    }

    #[test]
    fn live_keys_play_through_the_arp_and_let_go() {
        let s = song();
        let mut e = Engine::new(SR);
        e.bpm = 120.0;
        for k in [60, 64, 67] {
            e.live_event(&s, on(k));
        }
        run(&mut e, &s, 256);
        assert_eq!(down(&e), 1, "one note, not the chord");
        run(&mut e, &s, 4 * STEP - 512);
        assert_eq!(e.larp.log, [n(60), n(64), n(67), n(60)]);
        assert!(e.live_arp_holding());
        for k in [60, 64, 67] {
            e.live_event(&s, LiveEv::Off { key: k });
        }
        let before = e.larp.log.len();
        run(&mut e, &s, 3 * STEP);
        assert_eq!(e.larp.log.len(), before, "keys up: no more notes");
        assert_eq!(down(&e), 0, "and nothing hangs on");
        assert!(!e.larp.active);
    }

    #[test]
    fn live_latch_holds_until_a_new_chord_or_all_off() {
        let mut s = song();
        s.instruments[0].arp.as_mut().unwrap().latch = true;
        let mut e = Engine::new(SR);
        e.bpm = 120.0;
        e.live_event(&s, on(60));
        e.live_event(&s, on(64));
        run(&mut e, &s, 256);
        e.live_event(&s, LiveEv::Off { key: 60 });
        e.live_event(&s, LiveEv::Off { key: 64 });
        run(&mut e, &s, 4 * STEP);
        assert_eq!(e.larp.log[..4], [n(60), n(64), n(60), n(64)]);
        // a new key after all were up: a new chord
        e.live_event(&s, on(72));
        e.larp.log.clear();
        run(&mut e, &s, 3 * STEP);
        assert!(!e.larp.log.is_empty() && e.larp.log.iter().all(|&k| k == n(72)));
        e.live_event(&s, LiveEv::AllOff);
        e.larp.log.clear();
        run(&mut e, &s, 2 * STEP);
        assert!(e.larp.log.is_empty());
        assert_eq!(down(&e), 0);
    }

    #[test]
    fn live_arp_follows_the_song_and_survives_stop_and_off() {
        let mut s = song();
        let mut e = Engine::new(SR);
        e.start(&s, 0, 0, false, 0);
        run(&mut e, &s, 1000);
        e.live_event(&s, on(60));
        e.live_event(&s, on(67));
        run(&mut e, &s, 3 * STEP);
        assert!(e.larp.log.len() >= 2);
        e.stop();
        run(&mut e, &s, 256);
        assert!(down(&e) <= 1, "the stop let the song-clock notes go");
        let k = e.larp.log.len();
        run(&mut e, &s, 2 * STEP);
        assert!(e.larp.log.len() > k, "stopped: the keys still play");
        // the Arp switched off: its notes go at once
        s.instruments[0].arp = None;
        run(&mut e, &s, 256);
        assert!(!e.larp.active);
        assert_eq!(down(&e), 0);
        // and keys then play plainly
        e.live_event(&s, on(62));
        run(&mut e, &s, 256);
        assert_eq!(down(&e), 1);
    }
}
