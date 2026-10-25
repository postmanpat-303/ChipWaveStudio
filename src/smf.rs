//! Standard MIDI File (.mid) import (docs/MIDI.md): the notes of every track
//! and channel, with their velocities and their key pressure (poly
//! aftertouch, or channel aftertouch on every held note) as each note's
//! Pressure curve. Times are in beats, so the notes follow the song's
//! tempo; tempo changes, controllers and program changes are not read.
use crate::timeline::{self as tl, Note, Tick};

/// what an import gives: the notes (sorted by start) and a length in
/// ticks rounded up to whole bars of 4/4
#[derive(Debug, Default, PartialEq)]
pub struct MidiFile {
    pub notes: Vec<Note>,
    pub len: Tick,
    /// the first track name in the file, if any
    pub name: Option<String>,
}

fn vlq(b: &[u8], i: &mut usize) -> Result<u32, String> {
    let mut v: u32 = 0;
    for _ in 0..4 {
        let x = *b.get(*i).ok_or("the file ends inside a number")?;
        *i += 1;
        v = (v << 7) | (x & 0x7F) as u32;
        if x & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err("a number is too long".into())
}

/// a note still held: index into the notes, its MIDI channel and key
struct Held {
    idx: usize,
    ch: u8,
    key: u8,
}

/// parse a .mid file
pub fn parse(b: &[u8]) -> Result<MidiFile, String> {
    if b.len() < 14 || &b[0..4] != b"MThd" {
        return Err("not a MIDI file (no MThd header)".into());
    }
    let hlen = u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as usize;
    let ntrk = u16::from_be_bytes([b[10], b[11]]) as usize;
    let div = u16::from_be_bytes([b[12], b[13]]);
    if div & 0x8000 != 0 || div == 0 {
        return Err("SMPTE-timed MIDI files are not supported (beats only)".into());
    }
    let to_tick = |t: u64| (t * tl::PPQ as u64 / div as u64) as Tick;
    let mut out = MidiFile::default();
    let mut pos = 8 + hlen;
    let mut end_all: Tick = 0;
    for _ in 0..ntrk {
        if pos + 8 > b.len() {
            break;
        }
        let tlen = u32::from_be_bytes([b[pos + 4], b[pos + 5], b[pos + 6], b[pos + 7]]) as usize;
        let body = &b[pos + 8..(pos + 8 + tlen).min(b.len())];
        let is_track = &b[pos..pos + 4] == b"MTrk";
        pos += 8 + tlen;
        if !is_track {
            continue;
        }
        let mut i = 0;
        let mut t: u64 = 0;
        let mut run: u8 = 0;
        let mut held: Vec<Held> = vec![];
        let close = |out: &mut MidiFile, h: Held, at: Tick| {
            let n = &mut out.notes[h.idx];
            n.len = (at - n.start).max(1);
            if let Some(p) = n.expr.iter_mut().find(|e| e.kind == tl::ExprKind::Pressure) {
                p.points.retain(|q| q.0 < n.len);
            }
            n.expr.retain(|e| !e.points.is_empty());
        };
        while i < body.len() {
            t += vlq(body, &mut i)? as u64;
            let at = to_tick(t);
            let mut st = *body.get(i).ok_or("the file ends inside an event")?;
            if st & 0x80 != 0 {
                i += 1;
            } else if run == 0 {
                return Err("a running status with no status before it".into());
            } else {
                st = run;
            }
            match st {
                0xFF => {
                    let ty = *body.get(i).ok_or("short meta event")?;
                    i += 1;
                    let l = vlq(body, &mut i)? as usize;
                    let d = body.get(i..i + l).ok_or("short meta event")?;
                    i += l;
                    if ty == 0x03 && out.name.is_none() && !d.is_empty() {
                        out.name = Some(String::from_utf8_lossy(d).trim().to_string())
                            .filter(|s| !s.is_empty());
                    }
                    if ty == 0x2F {
                        break;
                    }
                }
                0xF0 | 0xF7 => {
                    let l = vlq(body, &mut i)? as usize;
                    i += l;
                }
                0x80..=0xEF => {
                    run = st;
                    let ch = st & 0x0F;
                    let n_data = if matches!(st & 0xF0, 0xC0 | 0xD0) {
                        1
                    } else {
                        2
                    };
                    let d = body.get(i..i + n_data).ok_or("short event")?;
                    i += n_data;
                    let (d0, d1) = (d[0] & 0x7F, d.get(1).map_or(0, |x| x & 0x7F));
                    match st & 0xF0 {
                        0x90 if d1 > 0 => {
                            // a retrigger of a held key ends the old note
                            if let Some(k) = held.iter().position(|h| h.ch == ch && h.key == d0) {
                                close(&mut out, held.remove(k), at);
                            }
                            let Some(key) = crate::midi::tracker_note(d0, 0) else {
                                continue;
                            };
                            out.notes.push(Note {
                                start: at,
                                len: 1,
                                key,
                                vel: tl::vel_from_7(d1),
                                ..Default::default()
                            });
                            held.push(Held {
                                idx: out.notes.len() - 1,
                                ch,
                                key: d0,
                            });
                        }
                        0x80 | 0x90 => {
                            if let Some(k) = held.iter().position(|h| h.ch == ch && h.key == d0) {
                                close(&mut out, held.remove(k), at);
                            }
                        }
                        0xA0 => {
                            for h in held.iter().filter(|h| h.ch == ch && h.key == d0) {
                                let n = &mut out.notes[h.idx];
                                let rel = at - n.start;
                                n.press_push(rel, d1 as f32 / 127.0);
                            }
                        }
                        0xD0 => {
                            for h in held.iter().filter(|h| h.ch == ch) {
                                let n = &mut out.notes[h.idx];
                                let rel = at - n.start;
                                n.press_push(rel, d0 as f32 / 127.0);
                            }
                        }
                        _ => {}
                    }
                    end_all = end_all.max(at);
                }
                _ => return Err(format!("unknown event {st:#04x}")),
            }
        }
        // notes still held at the end of the track end there
        let tend = to_tick(t);
        end_all = end_all.max(tend);
        for h in held.drain(..) {
            close(&mut out, h, tend);
        }
    }
    if out.notes.is_empty() {
        return Err("no notes in the file".into());
    }
    out.notes.sort_by_key(|n| (n.start, n.key));
    let last = out.notes.iter().map(|n| n.start + n.len).max().unwrap_or(0);
    let bar = 4 * tl::PPQ;
    out.len = (last.max(end_all).max(1) + bar - 1) / bar * bar;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(div: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
        let mut b = b"MThd".to_vec();
        b.extend(6u32.to_be_bytes());
        b.extend(1u16.to_be_bytes());
        b.extend((tracks.len() as u16).to_be_bytes());
        b.extend(div.to_be_bytes());
        for t in tracks {
            b.extend(b"MTrk");
            b.extend((t.len() as u32).to_be_bytes());
            b.extend(t);
        }
        b
    }

    #[test]
    fn notes_velocity_and_pressure_come_in() {
        // 96 ticks a beat: C4 for 1 beat with key pressure, then E4 + G4
        // (running status, note-on 0 as an off) with channel pressure
        let t1 = vec![
            0x00, 0xFF, 0x03, 0x04, b'L', b'e', b'a', b'd', // name
            0x00, 0x90, 60, 100, // C4 on
            0x30, 0xA0, 60, 64, // key pressure half way
            0x30, 0x80, 60, 0, // off after a beat
            0x00, 0x90, 64, 127, // E4
            0x00, 67, 80, // G4, running status
            0x18, 0xD0, 127, // channel pressure: both
            0x81, 0x00, 0x90, 64, 0, // E4 off (vlq 128)
            0x00, 67, 0, // G4 off
            0x00, 0xFF, 0x2F, 0x00,
        ];
        let f = parse(&file(96, &[t1])).unwrap();
        assert_eq!(f.name.as_deref(), Some("Lead"));
        let b = tl::PPQ;
        let got: Vec<(Tick, Tick, u8)> = f.notes.iter().map(|n| (n.start, n.len, n.key)).collect();
        let tr = |k| crate::midi::tracker_note(k, 0).unwrap();
        assert_eq!(
            got,
            vec![
                (0, b, tr(60)),
                (b, b * (0x18 + 128) / 96, tr(64)),
                (b, b * (0x18 + 128) / 96, tr(67)),
            ]
        );
        assert_eq!(f.notes[0].vel, tl::vel_from_7(100));
        assert_eq!(f.notes[0].pressure().unwrap(), &[(b / 2, 64.0 / 127.0)]);
        for n in &f.notes[1..] {
            assert_eq!(n.pressure().unwrap(), &[(b / 4, 1.0)]);
        }
        assert_eq!(f.len, 4 * b, "one bar");
    }

    #[test]
    fn files_without_pressure_have_no_lanes_and_bad_files_say_why() {
        let t = vec![
            0x00, 0x90, 60, 100, 0x60, 0x80, 60, 0, 0x00, 0xFF, 0x2F, 0x00,
        ];
        let f = parse(&file(96, &[t])).unwrap();
        assert!(f.notes[0].expr.is_empty());
        assert!(parse(b"RIFF....").is_err());
        assert!(parse(&file(0xE728, &[])).is_err(), "SMPTE");
        let t = vec![0x00, 0xFF, 0x2F, 0x00];
        assert!(parse(&file(96, &[t])).is_err(), "no notes");
        // a hanging note ends at the end of its track
        let t = vec![0x00, 0x90, 60, 100, 0x60, 0xFF, 0x2F, 0x00];
        let f = parse(&file(96, &[t])).unwrap();
        assert_eq!(f.notes[0].len, tl::PPQ);
        // truncated
        assert!(parse(&file(96, &[vec![0x00, 0x90, 60]])).is_err());
    }
}
