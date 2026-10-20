//! Musical typing: the computer keyboard as a little MIDI keyboard (Ctrl+K,
//! the toolbar's Typing button or the piano roll's). Off by default. While
//! on, A S D F G H J K L ; are the white keys from C, W E T Y U O P the
//! black ones, Z / X an octave down / up and C / V velocity down / up -
//! those keys play notes instead of their shortcuts. A typed note plays the
//! selected track's instrument and goes the MIDI way (`midi_note_on`), so
//! Record and Step input take it like a MIDI key.
use super::*;
use std::time::Instant;

/// synthetic key ids for typed notes (MIDI keys are 0..=127; the piano
/// roll's audition uses 230+)
const KEY_BASE: u8 = 180;

/// (key, semitones from the octave's C)
pub(crate) const NOTE_KEYS: [(Key, i32); 17] = [
    (Key::A, 0),
    (Key::W, 1),
    (Key::S, 2),
    (Key::E, 3),
    (Key::D, 4),
    (Key::F, 5),
    (Key::T, 6),
    (Key::G, 7),
    (Key::Y, 8),
    (Key::H, 9),
    (Key::U, 10),
    (Key::J, 11),
    (Key::K, 12),
    (Key::O, 13),
    (Key::L, 14),
    (Key::P, 15),
    (Key::Semicolon, 16),
];

pub(crate) struct Typing {
    pub(crate) on: bool,
    /// the octave A plays the C of (C4 = middle C)
    pub(crate) octave: i32,
    pub(crate) vel: f32,
    /// keys down: (key, id sent, note)
    held: Vec<(Key, u8, u8)>,
}

impl Default for Typing {
    fn default() -> Self {
        Self {
            on: false,
            octave: 4,
            vel: 0.8,
            held: vec![],
        }
    }
}

/// tracker note (1 = C-0 .. 120) for semitone `off` in octave `oct`
/// (MIDI key = 12 * (oct + 1) + off; tracker note = MIDI key - 11)
pub(crate) fn typed_note(oct: i32, off: i32) -> Option<u8> {
    let n = 12 * (oct + 1) + off - 11;
    (1..=120).contains(&n).then_some(n as u8)
}

fn is_typing_key(k: Key) -> bool {
    NOTE_KEYS.iter().any(|(x, _)| *x == k) || matches!(k, Key::Z | Key::X | Key::C | Key::V)
}

impl App {
    pub(super) fn typing_toggle(&mut self) {
        self.typing.on = !self.typing.on;
        if !self.typing.on {
            self.typing_release_all();
        }
        self.status = if self.typing.on {
            format!(
                "Musical typing on ({}): A-; white keys, W E T Y U O P black, Z/X octave, C/V velocity - Ctrl+K: off",
                self.typing_label()
            )
        } else {
            "Musical typing off".into()
        };
    }

    /// "C4 · 80" (the octave A plays, velocity 0-127)
    pub(crate) fn typing_label(&self) -> String {
        format!(
            "C{} · vel {}",
            self.typing.octave,
            (self.typing.vel * 127.0).round() as u8
        )
    }

    fn typing_release_all(&mut self) {
        let held: Vec<(Key, u8, u8)> = std::mem::take(&mut self.typing.held);
        for (_, id, _) in held {
            self.typing_off(id);
        }
    }

    fn typing_off(&mut self, id: u8) {
        self.with(|s| {
            let audio::Shared { song, engine, .. } = s;
            engine.live_event(song, crate::midi::LiveEv::Off { key: id });
        });
        self.midi_note_off(id);
    }

    /// the instrument the selected track plays
    fn typing_instr(&self) -> u8 {
        let ch = self.ch;
        let given = self.with(|s| {
            s.song
                .timeline
                .as_deref()
                .and_then(|t| t.tracks.get(ch))
                .map_or(0, |t| t.instr)
        });
        if given > 0 {
            given
        } else {
            self.instr.max(1)
        }
    }

    /// Ctrl+K, and (while on) the typing keys: taken out of `events`
    /// before any shortcut sees them
    pub(super) fn typing_keys(&mut self, events: Vec<egui::Event>) -> Vec<egui::Event> {
        let mut keep = Vec::with_capacity(events.len());
        let open = self.prompt.is_none() && self.fill.is_none() && self.rebinding.is_none();
        for ev in events {
            let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers: m,
                ..
            } = ev
            else {
                keep.push(ev);
                continue;
            };
            if key == Key::K && m.command && !m.shift && !m.alt {
                if pressed && !repeat && open {
                    self.typing_toggle();
                }
                continue;
            }
            if !self.typing.on || !open || m.command || m.alt || !is_typing_key(key) {
                keep.push(ev);
                continue;
            }
            if !pressed {
                if let Some(i) = self.typing.held.iter().position(|h| h.0 == key) {
                    let (_, id, _) = self.typing.held.remove(i);
                    self.typing_off(id);
                }
                continue;
            }
            if repeat {
                continue;
            }
            match key {
                Key::Z | Key::X => {
                    let d = if key == Key::Z { -1 } else { 1 };
                    self.typing.octave = (self.typing.octave + d).clamp(0, 8);
                    self.status = format!("Musical typing: {}", self.typing_label());
                }
                Key::C | Key::V => {
                    let d = if key == Key::C { -0.1 } else { 0.1 };
                    self.typing.vel = ((self.typing.vel + d) * 10.0).round() / 10.0;
                    self.typing.vel = self.typing.vel.clamp(0.1, 1.0);
                    self.status = format!("Musical typing: {}", self.typing_label());
                }
                _ => self.typing_on(key),
            }
        }
        keep
    }

    pub(super) fn typing_on(&mut self, key: Key) {
        let Some(k) = NOTE_KEYS.iter().position(|(x, _)| *x == key) else {
            return;
        };
        if self.typing.held.iter().any(|h| h.0 == key) {
            return;
        }
        let Some(mut note) = typed_note(self.typing.octave, NOTE_KEYS[k].1) else {
            return;
        };
        if let Some(sk) = self.scale_lock_key() {
            note = sk.snap(note);
        }
        let id = KEY_BASE + k as u8;
        let (ch, instr, vel) = (self.ch as u8, self.typing_instr(), self.typing.vel);
        self.with(|s| {
            let audio::Shared { song, engine, .. } = s;
            engine.live_event(
                song,
                crate::midi::LiveEv::On {
                    key: id,
                    note,
                    vel,
                    instr,
                    ch,
                },
            );
        });
        self.typing.held.push((key, id, note));
        self.midi_note_on(id, note, vel, Instant::now());
    }
}
