//! MIDI keyboard input (docs/MIDI.md).
//!
//! The OS MIDI thread (midir callback) parses each message, applies the
//! channel filter, octave and velocity curve, and pushes small `Copy` events
//! into two lock-free rtrb queues: one read by the audio engine (live play,
//! no locks or allocation on that side) and one read by the UI (recording,
//! MIDI learn, the activity light). Settings that the MIDI thread needs are
//! atomics in `Config`, written by the UI every frame.
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicU32, AtomicU8, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

/// A parsed channel message (channel 0..=15).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Msg {
    NoteOn {
        ch: u8,
        key: u8,
        vel: u8,
    },
    NoteOff {
        ch: u8,
        key: u8,
    },
    Cc {
        ch: u8,
        cc: u8,
        val: u8,
    },
    /// -8192..=8191, 0 = centre
    Bend {
        ch: u8,
        value: i16,
    },
}

/// Parse one MIDI message. Note-on with velocity 0 is a note-off; system,
/// clock, aftertouch and program change messages are ignored.
pub fn parse(b: &[u8]) -> Option<Msg> {
    let (&status, data) = b.split_first()?;
    let ch = status & 0x0F;
    let d = |i: usize| data.get(i).copied().filter(|v| *v < 0x80);
    match status & 0xF0 {
        0x90 => {
            let (key, vel) = (d(0)?, d(1)?);
            Some(if vel == 0 {
                Msg::NoteOff { ch, key }
            } else {
                Msg::NoteOn { ch, key, vel }
            })
        }
        0x80 => Some(Msg::NoteOff { ch, key: d(0)? }),
        0xB0 => Some(Msg::Cc {
            ch,
            cc: d(0)?,
            val: d(1)?,
        }),
        0xE0 => {
            let v = (d(0)? as i16) | ((d(1)? as i16) << 7);
            Some(Msg::Bend {
                ch,
                value: v - 8192,
            })
        }
        _ => None,
    }
}

/// Velocity curve (0..127 -> gain 0..1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Curve {
    #[default]
    Linear,
    /// light touch already loud (good for stiff keys)
    Soft,
    /// needs a firm touch to get loud
    Hard,
    /// every note at full volume
    Fixed,
}

impl Curve {
    pub const ALL: [Curve; 4] = [Curve::Linear, Curve::Soft, Curve::Hard, Curve::Fixed];
    pub fn name(self) -> &'static str {
        match self {
            Curve::Linear => "Linear",
            Curve::Soft => "Soft (light touch)",
            Curve::Hard => "Hard (firm touch)",
            Curve::Fixed => "Fixed (full)",
        }
    }
    pub fn apply(self, vel: u8) -> f32 {
        let x = vel.min(127) as f32 / 127.0;
        match self {
            Curve::Linear => x,
            Curve::Soft => x.sqrt(),
            Curve::Hard => x * x,
            Curve::Fixed => 1.0,
        }
    }
    fn to_u8(self) -> u8 {
        self as u8
    }
    fn from_u8(v: u8) -> Curve {
        Curve::ALL.get(v as usize).copied().unwrap_or_default()
    }
}

/// MIDI key -> tracker note (1 = C-0, 49 = C-4). Key 60 (middle C) is C-4
/// when `octave` is 0; `octave` shifts by whole octaves. None outside the
/// tracker range.
pub fn tracker_note(key: u8, octave: i32) -> Option<u8> {
    let n = key as i32 - 11 + octave * 12;
    (1..=120).contains(&n).then_some(n as u8)
}

/// gain 0..1 -> tracker volume column value (1..=65 = volume 00..40)
pub fn vol_column(g: f32) -> u8 {
    ((g.clamp(0.0, 1.0) * 64.0).round() as u8).max(1) + 1
}

/// Event for the audio engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiveEv {
    /// play `note` (tracker numbering) with instrument slot `instr` (1-based)
    /// at gain `vel`, through mixer channel `ch`
    On {
        key: u8,
        note: u8,
        vel: f32,
        instr: u8,
        ch: u8,
    },
    Off {
        key: u8,
    },
    Sustain(bool),
    /// pitch bend in semitones
    Bend(f32),
    /// release everything (device change, panic)
    AllOff,
}

/// Event for the UI thread (recording, learn, activity).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiEv {
    On {
        key: u8,
        note: u8,
        vel: f32,
        at: std::time::Instant,
    },
    Off {
        key: u8,
    },
    Cc {
        cc: u8,
        val: u8,
    },
}

/// Settings read by the MIDI thread (written by the UI).
pub struct Config {
    /// 0 = all channels (omni), 1..=16 = that channel only
    pub channel: AtomicU8,
    pub velocity: AtomicBool,
    curve: AtomicU8,
    pub octave: AtomicI8,
    /// current instrument slot (1-based) and cursor channel
    pub instr: AtomicU8,
    pub mon_ch: AtomicU8,
    /// pitch bend range in semitones
    pub bend_range: AtomicU8,
    /// messages received (activity light)
    pub activity: AtomicU32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            channel: AtomicU8::new(0),
            velocity: AtomicBool::new(true),
            curve: AtomicU8::new(0),
            octave: AtomicI8::new(0),
            instr: AtomicU8::new(1),
            mon_ch: AtomicU8::new(0),
            bend_range: AtomicU8::new(2),
            activity: AtomicU32::new(0),
        }
    }
}

impl Config {
    pub fn set_curve(&self, c: Curve) {
        self.curve.store(c.to_u8(), Relaxed);
    }
    pub fn curve(&self) -> Curve {
        Curve::from_u8(self.curve.load(Relaxed))
    }
    /// copy the persisted settings in
    pub fn apply(&self, s: &MidiSettings) {
        self.channel.store(s.channel.min(16), Relaxed);
        self.velocity.store(s.velocity, Relaxed);
        self.set_curve(s.curve);
        self.octave.store(s.octave.clamp(-4, 4), Relaxed);
        self.bend_range.store(s.bend_range.clamp(1, 24), Relaxed);
    }

    /// Map a raw message to engine and UI events (pure; unit tested).
    pub fn translate(&self, m: Msg) -> (Option<LiveEv>, Option<UiEv>) {
        let filt = self.channel.load(Relaxed);
        let ch = match m {
            Msg::NoteOn { ch, .. }
            | Msg::NoteOff { ch, .. }
            | Msg::Cc { ch, .. }
            | Msg::Bend { ch, .. } => ch,
        };
        if filt != 0 && ch + 1 != filt {
            return (None, None);
        }
        let oct = self.octave.load(Relaxed) as i32;
        match m {
            Msg::NoteOn { key, vel, .. } => {
                let Some(note) = tracker_note(key, oct) else {
                    return (None, None);
                };
                let g = if self.velocity.load(Relaxed) {
                    self.curve().apply(vel)
                } else {
                    1.0
                };
                (
                    Some(LiveEv::On {
                        key,
                        note,
                        vel: g,
                        instr: self.instr.load(Relaxed),
                        ch: self.mon_ch.load(Relaxed),
                    }),
                    Some(UiEv::On {
                        key,
                        note,
                        vel: g,
                        at: std::time::Instant::now(),
                    }),
                )
            }
            Msg::NoteOff { key, .. } => (Some(LiveEv::Off { key }), Some(UiEv::Off { key })),
            Msg::Cc { cc: 64, val, .. } => (Some(LiveEv::Sustain(val >= 64)), None),
            // all sound off / all notes off
            Msg::Cc { cc: 120 | 123, .. } => (Some(LiveEv::AllOff), None),
            Msg::Cc { cc, val, .. } => (None, Some(UiEv::Cc { cc, val })),
            Msg::Bend { value, .. } => {
                let r = self.bend_range.load(Relaxed) as f32;
                (Some(LiveEv::Bend(value as f32 / 8192.0 * r)), None)
            }
        }
    }
}

/// Persisted MIDI settings (settings.toml, [midi]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiSettings {
    /// input port name, "" = none, "*" = all devices
    pub device: String,
    /// connect to the last device at start and when it is plugged in
    pub auto_connect: bool,
    /// 0 = all channels, 1..=16
    pub channel: u8,
    pub velocity: bool,
    pub curve: Curve,
    /// MIDI octave shift (independent of the computer-keyboard octave)
    pub octave: i8,
    /// chords go into the cursor channel and the channels to its right
    pub chord_spread: bool,
    /// while the song plays, record at the playing row (quantised) instead
    /// of at the cursor
    pub live_record: bool,
    pub bend_range: u8,
    /// MIDI learn: CC number -> automation target id (e.g. "instrument.1.cutoff")
    pub learn: Vec<(u8, String)>,
}

impl Default for MidiSettings {
    fn default() -> Self {
        Self {
            device: String::new(),
            auto_connect: true,
            channel: 0,
            velocity: true,
            curve: Curve::Linear,
            octave: 0,
            chord_spread: false,
            live_record: true,
            bend_range: 2,
            learn: vec![],
        }
    }
}

/// queue sizes (events); a full queue drops events rather than block
const QUEUE: usize = 1024;

struct Sinks {
    audio: rtrb::Producer<LiveEv>,
    ui: rtrb::Producer<UiEv>,
    ctx: Option<eframe::egui::Context>,
}

/// Owns the device connections and the queues.
pub struct Hub {
    pub cfg: Arc<Config>,
    sinks: Arc<Mutex<Sinks>>,
    pub ui_rx: rtrb::Consumer<UiEv>,
    conns: Vec<(String, midir::MidiInputConnection<()>)>,
    /// selection the connections were made for
    wanted: String,
    /// ports seen at the last scan
    pub ports: Vec<String>,
    last_scan: Option<std::time::Instant>,
    pub error: Option<String>,
}

fn lock(s: &Mutex<Sinks>) -> std::sync::MutexGuard<'_, Sinks> {
    s.lock().unwrap_or_else(|p| p.into_inner())
}

/// the body of the OS MIDI callback (also used to inject test events)
fn deliver(cfg: &Config, sinks: &Mutex<Sinks>, bytes: &[u8]) {
    let Some(m) = parse(bytes) else {
        return;
    };
    cfg.activity.fetch_add(1, Relaxed);
    let (a, u) = cfg.translate(m);
    let mut s = lock(sinks);
    if let Some(a) = a {
        let _ = s.audio.push(a);
    }
    if let Some(u) = u {
        let _ = s.ui.push(u);
    }
    if let Some(c) = &s.ctx {
        c.request_repaint();
    }
}

/// Names of the MIDI input ports.
pub fn list_ports() -> Result<Vec<String>, String> {
    let mi = midir::MidiInput::new("ChipWave scan").map_err(|e| e.to_string())?;
    Ok(mi
        .ports()
        .iter()
        .map(|p| mi.port_name(p).unwrap_or_else(|_| "?".into()))
        .collect())
}

impl Hub {
    /// The hub and the engine's end of the audio queue.
    pub fn new() -> (Hub, rtrb::Consumer<LiveEv>) {
        let (ap, ac) = rtrb::RingBuffer::new(QUEUE);
        let (up, uc) = rtrb::RingBuffer::new(QUEUE);
        (
            Hub {
                cfg: Arc::new(Config::default()),
                sinks: Arc::new(Mutex::new(Sinks {
                    audio: ap,
                    ui: up,
                    ctx: None,
                })),
                ui_rx: uc,
                conns: vec![],
                wanted: String::new(),
                ports: vec![],
                last_scan: None,
                error: None,
            },
            ac,
        )
    }

    pub fn set_ctx(&self, ctx: &eframe::egui::Context) {
        let mut s = lock(&self.sinks);
        if s.ctx.is_none() {
            s.ctx = Some(ctx.clone());
        }
    }

    /// Feed raw MIDI bytes as if they came from a device (tests, self-test).
    pub fn inject(&self, bytes: &[u8]) {
        deliver(&self.cfg, &self.sinks, bytes);
    }

    /// names of the connected ports
    pub fn connected(&self) -> Vec<&str> {
        self.conns.iter().map(|(n, _)| n.as_str()).collect()
    }

    fn release_all(&self) {
        let _ = lock(&self.sinks).audio.push(LiveEv::AllOff);
    }

    /// Connect to `sel` ("" = none, "*" = all ports, else a port name).
    pub fn connect(&mut self, sel: &str) {
        self.conns.clear();
        self.release_all();
        self.wanted = sel.to_string();
        self.error = None;
        self.rescan_now();
    }

    /// Re-read the port list and (dis)connect so the connections match the
    /// selection: hot-plug. Cheap enough to call every couple of seconds.
    pub fn rescan_now(&mut self) {
        self.last_scan = Some(std::time::Instant::now());
        match list_ports() {
            Ok(p) => self.ports = p,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        }
        // drop connections whose port went away (unplugged)
        let before = self.conns.len();
        let ports = self.ports.clone();
        self.conns.retain(|(n, _)| ports.contains(n));
        if self.conns.len() != before {
            self.release_all();
        }
        if self.wanted.is_empty() {
            self.conns.clear();
            return;
        }
        let want: Vec<String> = if self.wanted == "*" {
            ports
        } else {
            ports.into_iter().filter(|p| *p == self.wanted).collect()
        };
        for name in want {
            if self.conns.iter().any(|(n, _)| *n == name) {
                continue;
            }
            let mi = match midir::MidiInput::new("ChipWave") {
                Ok(m) => m,
                Err(e) => {
                    self.error = Some(e.to_string());
                    return;
                }
            };
            let Some(port) = mi
                .ports()
                .into_iter()
                .find(|p| mi.port_name(p).ok().as_deref() == Some(name.as_str()))
            else {
                continue;
            };
            let (cfg, sinks) = (self.cfg.clone(), self.sinks.clone());
            match mi.connect(
                &port,
                "chipwave-in",
                move |_t, bytes, _| deliver(&cfg, &sinks, bytes),
                (),
            ) {
                Ok(c) => self.conns.push((name, c)),
                Err(e) => self.error = Some(format!("{name}: {e}")),
            }
        }
    }

    /// call every frame: rescans every 2 s while a device is selected
    pub fn poll(&mut self) {
        if self.wanted.is_empty() {
            return;
        }
        if self
            .last_scan
            .is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(2))
        {
            self.rescan_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_messages() {
        assert_eq!(
            parse(&[0x90, 60, 100]),
            Some(Msg::NoteOn {
                ch: 0,
                key: 60,
                vel: 100
            })
        );
        assert_eq!(parse(&[0x93, 60, 0]), Some(Msg::NoteOff { ch: 3, key: 60 }));
        assert_eq!(
            parse(&[0x8F, 61, 40]),
            Some(Msg::NoteOff { ch: 15, key: 61 })
        );
        assert_eq!(
            parse(&[0xB0, 64, 127]),
            Some(Msg::Cc {
                ch: 0,
                cc: 64,
                val: 127
            })
        );
        assert_eq!(
            parse(&[0xE1, 0x00, 0x40]),
            Some(Msg::Bend { ch: 1, value: 0 })
        );
        assert_eq!(
            parse(&[0xE0, 0x7F, 0x7F]),
            Some(Msg::Bend { ch: 0, value: 8191 })
        );
        assert_eq!(
            parse(&[0xE0, 0, 0]),
            Some(Msg::Bend {
                ch: 0,
                value: -8192
            })
        );
        // clock, sysex, program change, truncated / bad data bytes
        assert_eq!(parse(&[0xF8]), None);
        assert_eq!(parse(&[0xF0, 1, 2, 0xF7]), None);
        assert_eq!(parse(&[0xC0, 5]), None);
        assert_eq!(parse(&[0x90, 60]), None);
        assert_eq!(parse(&[0x90, 0x80, 1]), None);
        assert_eq!(parse(&[]), None);
    }

    #[test]
    fn notes_curves_and_volume_column() {
        assert_eq!(tracker_note(60, 0), Some(49)); // middle C = C-4
        assert_eq!(crate::model::note_name(49), "C-4");
        assert_eq!(tracker_note(60, 1), Some(61));
        assert_eq!(tracker_note(60, -1), Some(37));
        assert_eq!(tracker_note(11, 0), None);
        assert_eq!(tracker_note(127, 2), None);
        assert_eq!(Curve::Linear.apply(127), 1.0);
        assert_eq!(Curve::Fixed.apply(1), 1.0);
        assert!(Curve::Soft.apply(32) > Curve::Linear.apply(32));
        assert!(Curve::Hard.apply(32) < Curve::Linear.apply(32));
        assert_eq!(vol_column(1.0), 65); // volume 40
        assert_eq!(vol_column(0.5), 33); // volume 20
        assert_eq!(vol_column(0.0), 2); // never "empty"
    }

    #[test]
    fn translate_filters_and_maps() {
        let c = Config::default();
        c.instr.store(3, Relaxed);
        c.mon_ch.store(2, Relaxed);
        let (a, u) = c.translate(Msg::NoteOn {
            ch: 4,
            key: 72,
            vel: 127,
        });
        assert_eq!(
            a,
            Some(LiveEv::On {
                key: 72,
                note: 61,
                vel: 1.0,
                instr: 3,
                ch: 2
            })
        );
        assert!(matches!(u, Some(UiEv::On { note: 61, .. })));
        // channel filter
        c.channel.store(1, Relaxed);
        assert_eq!(
            c.translate(Msg::NoteOn {
                ch: 4,
                key: 72,
                vel: 127
            }),
            (None, None)
        );
        assert!(c
            .translate(Msg::NoteOn {
                ch: 0,
                key: 72,
                vel: 127
            })
            .0
            .is_some());
        // velocity off = full
        c.velocity.store(false, Relaxed);
        let (a, _) = c.translate(Msg::NoteOn {
            ch: 0,
            key: 72,
            vel: 10,
        });
        assert!(matches!(a, Some(LiveEv::On { vel, .. }) if vel == 1.0));
        // octave shift
        c.octave.store(-1, Relaxed);
        let (a, _) = c.translate(Msg::NoteOn {
            ch: 0,
            key: 60,
            vel: 10,
        });
        assert!(matches!(a, Some(LiveEv::On { note: 37, .. })));
        // sustain, bend (+-2 semitones), mod wheel goes to the UI only
        assert_eq!(
            c.translate(Msg::Cc {
                ch: 0,
                cc: 64,
                val: 127
            })
            .0,
            Some(LiveEv::Sustain(true))
        );
        assert_eq!(
            c.translate(Msg::Cc {
                ch: 0,
                cc: 64,
                val: 0
            })
            .0,
            Some(LiveEv::Sustain(false))
        );
        assert_eq!(
            c.translate(Msg::Bend {
                ch: 0,
                value: 8192 / 2
            })
            .0,
            Some(LiveEv::Bend(1.0))
        );
        assert_eq!(
            c.translate(Msg::Cc {
                ch: 0,
                cc: 1,
                val: 9
            }),
            (None, Some(UiEv::Cc { cc: 1, val: 9 }))
        );
        assert_eq!(
            c.translate(Msg::Cc {
                ch: 0,
                cc: 123,
                val: 0
            })
            .0,
            Some(LiveEv::AllOff)
        );
    }

    #[test]
    fn injected_bytes_reach_both_queues() {
        let (mut hub, mut audio) = Hub::new();
        hub.inject(&[0x90, 60, 64]);
        hub.inject(&[0x80, 60, 0]);
        hub.inject(&[0xFE]); // active sensing: ignored
        assert!(matches!(audio.pop(), Ok(LiveEv::On { note: 49, .. })));
        assert_eq!(audio.pop(), Ok(LiveEv::Off { key: 60 }));
        assert!(audio.pop().is_err());
        assert!(matches!(hub.ui_rx.pop(), Ok(UiEv::On { key: 60, .. })));
        assert_eq!(hub.ui_rx.pop(), Ok(UiEv::Off { key: 60 }));
        assert_eq!(hub.cfg.activity.load(Relaxed), 2);
        // a full queue drops instead of blocking
        for _ in 0..QUEUE + 10 {
            hub.inject(&[0x90, 60, 64]);
        }
        let _ = &mut hub;
    }
}
