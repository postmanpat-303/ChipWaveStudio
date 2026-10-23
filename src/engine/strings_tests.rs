//! StringWave in the engine: tracker notes, note clips, rack layers and
//! live MIDI play its own engine; automation is heard while a note holds;
//! the output doesn't depend on the buffer size; songs without StringWave
//! never make an engine; nothing allocates once the engines exist.
use super::racks_tests::{clip_song, diff, one_note_song, render};
use super::*;
use crate::model::{Layer, RackP};

fn strings_kind() -> InstrumentKind {
    InstrumentKind::Gen(crate::gen::GenPatch {
        engine: crate::gen::GenEngine::Strings(crate::strings::StringsP::default()),
        ..Default::default()
    })
}

/// slot 1 = StringWave (Velvet Ensemble), one note on row 0, a note-off on
/// row 8 (1 s)
pub(crate) fn sw_song() -> Song {
    let mut s = one_note_song();
    s.instruments[0].kind = strings_kind();
    s.instruments[0].name = "StringWave".into();
    s.patterns[0].cells[8][0] = Cell {
        note: NOTE_OFF,
        ..Default::default()
    };
    s
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

/// stereo frames a..b seconds
fn sec(v: &[f32], a: f32, b: f32) -> &[f32] {
    &v[(a * 48000.0) as usize * 2..(b * 48000.0) as usize * 2]
}

#[test]
fn a_tracker_note_plays_and_its_note_off_releases_it() {
    let s = sw_song();
    let out = render(&s, 5.0);
    assert!(out.iter().all(|x| x.is_finite()));
    let held = rms(sec(&out, 0.4, 0.9));
    assert!(held > 1e-3, "the note sounds ({held})");
    // the same note held on: the note-off is what makes it fade
    let mut on = s.clone();
    on.patterns[0].cells[8][0] = Cell::default();
    let kept = render(&on, 5.0);
    let (a, b) = (rms(sec(&out, 4.0, 5.0)), rms(sec(&kept, 4.0, 5.0)));
    assert!(a < b * 0.5, "released {a} vs held {b}");
}

#[test]
fn tracker_pitch_commands_reach_the_engine() {
    // 01xx on row 2 slides the held note up: the same until then, then not
    let plain = render(&sw_song(), 1.0);
    let mut up = sw_song();
    up.patterns[0].cells[2][0] = Cell {
        fx: 0x01,
        fxp: 0x40,
        ..Default::default()
    };
    let slid = render(&up, 1.0);
    assert!(sec(&plain, 0.0, 0.2) == sec(&slid, 0.0, 0.2));
    let d = diff(sec(&plain, 0.5, 1.0), sec(&slid, 0.5, 1.0));
    assert!(d > 1e-3, "the slide is heard ({d})");
    // 0Cxx after the note: its volume
    let mut quiet = sw_song();
    quiet.patterns[0].cells[2][0] = Cell {
        fx: 0x0C,
        fxp: 0x10,
        ..Default::default()
    };
    let q = render(&quiet, 1.0);
    let r = rms(sec(&q, 0.6, 0.9)) / rms(sec(&plain, 0.6, 0.9));
    assert!(r < 0.5, "0C turns it down ({r})");
}

#[test]
fn note_clips_play_and_end() {
    let s = clip_song(sw_song(), 1, &[37, 41, 44]);
    let out = render(&s, 6.0);
    assert!(rms(sec(&out, 0.3, 1.2)) > 1e-3, "clip notes sound");
    assert!(
        rms(sec(&out, 5.0, 6.0)) < rms(sec(&out, 0.3, 1.2)) * 0.3,
        "and end"
    );
}

#[test]
fn a_rack_layer_plays_stringwave() {
    let mut s = sw_song();
    let sw = s.instruments[0].clone();
    s.instruments[0].kind = InstrumentKind::Rack(RackP {
        layers: vec![Layer::new(1)],
    });
    s.instruments[0].name = "Rack".into();
    s.instruments.truncate(1);
    s.instruments.push(sw);
    let out = render(&s, 1.0);
    assert!(rms(sec(&out, 0.4, 0.9)) > 1e-3, "the layer sounds");
    let mut e = Engine::new(48000.0);
    e.start(&s, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 4096];
    e.render(&s, &mut buf);
    assert!(e.strings.find(1, 0, 0).is_some(), "a layer slot (layer 0)");
    assert!(e.strings.find(1, 0, crate::strings::rack::DIRECT).is_none());
}

#[test]
fn live_notes_play_while_stopped_and_stop_on_all_off() {
    use crate::midi::LiveEv;
    let s = sw_song();
    let mut e = Engine::new(48000.0);
    let mut buf = vec![0.0f32; 48000];
    e.live_event(
        &s,
        LiveEv::On {
            key: 60,
            note: 49,
            vel: 0.9,
            instr: 1,
            ch: 0,
        },
    );
    e.render(&s, &mut buf);
    assert!(rms(&buf[24000..]) > 1e-3, "a live note sounds");
    assert_eq!(e.inst_voices(0), 1);
    e.live_event(&s, LiveEv::Off { key: 60 });
    for _ in 0..8 {
        e.render(&s, &mut buf);
    }
    assert!(rms(&buf) < 1e-3, "released ({})", rms(&buf));
    assert_eq!(e.inst_voices(0), 0);
}

#[test]
fn automation_is_heard_while_a_note_holds() {
    use crate::automation::{Lane, Point, Target};
    let mut s = sw_song();
    s.patterns[0].cells[8][0] = Cell::default();
    let plain = render(&s, 2.0);
    let mut l = Lane::new(Target::Gen(0, crate::strings::CUTOFF), "Cutoff".into());
    for (pos, value) in [(0.0, 0.9), (4.0, 0.05)] {
        l.points.push(Point {
            pos,
            value,
            shape: Default::default(),
            curve: 0.0,
        });
    }
    s.automation = vec![l];
    let auto = render(&s, 2.0);
    // the start matches closely, the end is darker
    let d = diff(sec(&plain, 1.5, 2.0), sec(&auto, 1.5, 2.0));
    assert!(d > 1e-3, "the lane changes the held note ({d})");
}

#[test]
fn the_output_does_not_depend_on_the_buffer_size() {
    let s = clip_song(sw_song(), 1, &[37, 44]);
    let run = |n: usize| {
        let mut e = Engine::new(48000.0);
        e.start(&s, 0, 0, false, 0);
        let mut out = vec![0.0f32; 96000 * 2];
        for c in out.chunks_mut(n * 2) {
            e.render(&s, c);
        }
        out
    };
    let (a, b) = (run(512), run(37));
    assert!(a == b, "512 vs 37 frames differ");
}

#[test]
fn songs_without_stringwave_never_make_an_engine() {
    let s = one_note_song();
    let mut e = Engine::new(48000.0);
    crate::strings::rack::prepare(&mut e.strings, &s, 48000.0);
    e.start(&s, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 8192];
    e.render(&s, &mut buf);
    assert!(e.strings.is_empty());
    assert!(!crate::fileio::needs_reader_8(&s));
}

#[test]
fn a_playing_stringwave_does_not_allocate() {
    let s = clip_song(sw_song(), 1, &[37, 41, 44, 49, 37, 41]);
    let mut e = Engine::new(48000.0);
    crate::strings::rack::prepare(&mut e.strings, &s, 48000.0);
    assert!(!e.strings.is_empty(), "prepare made the engine");
    e.start(&s, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 512];
    for _ in 0..40 {
        e.render(&s, &mut buf);
    }
    let n = alloc_count::count(|| {
        for _ in 0..200 {
            e.render(&s, &mut buf);
        }
    });
    assert_eq!(n, 0, "no allocation while StringWave plays");
}

#[test]
fn stringwave_songs_round_trip_and_need_reader_8() {
    let s = sw_song();
    assert!(crate::fileio::needs_reader_8(&s));
    let dir = std::env::temp_dir().join(format!("cw-sw-song-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("sw.cwt");
    crate::fileio::save_cwt(&s, &f).unwrap();
    let text = std::fs::read_to_string(&f).unwrap_or_default();
    let back = crate::fileio::load_cwt(&f).unwrap();
    assert_eq!(back.instruments, s.instruments);
    assert_eq!(render(&back, 0.5), render(&s, 0.5));
    drop(text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleting_the_instrument_stops_its_engine() {
    let s = sw_song();
    let mut e = Engine::new(48000.0);
    e.start(&s, 0, 0, false, 0);
    let mut buf = vec![0.0f32; 8192];
    e.render(&s, &mut buf);
    assert!(!e.strings.is_empty());
    e.remap_instruments(&|i| if i == 0 { None } else { Some(i - 1) });
    assert!(e.strings.is_empty());
}

#[test]
fn live_wheel_expression_and_pedal() {
    use crate::midi::LiveEv;
    let song = sw_song();
    let rms = |exprv: Option<u8>, pedal: bool| {
        let mut e = Engine::new(48000.0);
        crate::strings::rack::prepare(&mut e.strings, &song, 48000.0);
        if let Some(v) = exprv {
            e.live_event(&song, LiveEv::Cc { cc: 11, val: v });
        }
        e.live_event(&song, LiveEv::Cc { cc: 1, val: 127 });
        if pedal {
            e.live_event(&song, LiveEv::Sustain(true));
        }
        e.live_event(
            &song,
            LiveEv::On {
                key: 60,
                note: 49,
                vel: 0.8,
                instr: 1,
                ch: 0,
            },
        );
        let mut buf = vec![0f32; 2 * 24000];
        e.render(&song, &mut buf);
        e.live_event(&song, LiveEv::Off { key: 60 });
        let mut tail = vec![0f32; 2 * 48000];
        e.render(&song, &mut tail);
        let r = |b: &[f32]| (b.iter().map(|x| x * x).sum::<f32>() / b.len() as f32).sqrt();
        (r(&buf), r(&tail[tail.len() - 9600..]))
    };
    let (full, rel) = rms(None, false);
    let (soft, _) = rms(Some(40), false);
    let (_, held) = rms(None, true);
    assert!(full > 1e-3, "{full}");
    assert!(soft < full * 0.5, "expression {soft} vs {full}");
    // the pedal holds the note after its key-up
    assert!(held > rel * 4.0 && held > 1e-4, "pedal {held} vs {rel}");
}
