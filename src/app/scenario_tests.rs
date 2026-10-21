//! Stage 6 (docs/unified/PLAN.md): the three workflows end to end, as a
//! user does them (pies, the launcher grid, drags, keys), each ending in a
//! render and a save / load round trip.
//! A: Drum Kit -> pattern -> launcher -> drag -> Ctrl+D -> render
//! B: WaveForge -> notes in a new launcher clip -> drag -> render
//!    (recording into a launcher slot is not in this version)
//! C: chords -> clip -> two launcher versions -> place one -> render
use super::objpie_tests::{pick, right_click};
use super::ui_tests::{click_at, double_click, exact, frame, key};
use super::unified_tests::{at, clip, frames, newest, notes_song, BAR};
use super::*;
use crate::timeline as tl;
use egui_kittest::Harness;

fn drag_to(h: &mut Harness<'static, App>, from: Pos2, to: Pos2) {
    let ev = |h: &mut Harness<'static, App>, e: egui::Event| {
        h.input_mut().events.push(e);
        frame(h);
    };
    ev(h, egui::Event::PointerMoved(from));
    ev(
        h,
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    );
    for k in 1..=6 {
        ev(
            h,
            egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)),
        );
    }
    ev(
        h,
        egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    );
    frames(h, 2);
}

fn slot(h: &Harness<'static, App>, tr: usize, k: usize) -> Rect {
    h.state()
        .ca
        .slot_rects
        .iter()
        .find(|x| x.0 == tr && x.1 == k)
        .map(|x| x.3)
        .unwrap_or_else(|| panic!("slot {tr} {k} not drawn"))
}

fn song(h: &Harness<'static, App>) -> Song {
    h.state().with(|s| s.song.clone())
}

fn peak(s: &Song) -> f32 {
    crate::engine::render_song(s, 11025)
        .iter()
        .fold(0.0f32, |m, x| m.max(x.abs()))
}

fn links(h: &Harness<'static, App>, data: usize) -> usize {
    h.state().with(|s| {
        let (a, b) = tl::linked(s.song.timeline.as_deref().unwrap(), data);
        a + b
    })
}

/// save, load: the same clips, launcher, links, names, colours and sound
fn round_trip(s: &Song, name: &str) -> Song {
    let dir = std::env::temp_dir().join(format!("cw-scenario-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("song.cwt");
    fileio::save_cwt(s, &p).unwrap();
    let back = fileio::load_cwt(&p).unwrap();
    let (a, b) = (
        s.timeline.as_deref().unwrap(),
        back.timeline.as_deref().unwrap(),
    );
    assert!(
        back.timeline.as_deref().unwrap().same_content(a),
        "{name}: timeline"
    );
    assert_eq!(a.launcher.len(), b.launcher.len());
    for (x, y) in a.clips.iter().zip(&b.clips) {
        assert_eq!(
            (x.id, x.notes(), &x.name, x.colour),
            (y.id, y.notes(), &y.name, y.colour)
        );
    }
    let r = |s: &Song| crate::engine::render_song(s, 11025);
    assert!(r(s) == r(&back), "{name}: renders the same after loading");
    let _ = std::fs::remove_dir_all(&dir);
    back
}

#[test]
fn scenario_a_drum_kit_pattern_launcher_drag_duplicate_render() {
    let (mut h, _) = notes_song();
    // a Drum Kit on track 02 and an empty pattern clip
    let (slot_k, pid) = h.state().with(|s| {
        let k = s
            .song
            .add_instrument(crate::drumpat::drumkit_instrument("808 Kit").unwrap())
            .unwrap() as u8
            + 1;
        crate::chanops::assign_track_instrument(&mut s.song, 1, k);
        (k, tl::new_note_clip(&mut s.song, 1, 0).unwrap())
    });
    h.state_mut().instr = slot_k;
    h.state_mut().ch = 1;
    h.state_mut().tlv.sel = Some(pid);
    h.state_mut().tlv.sels = vec![pid];
    h.state_mut().set_lower(1);
    frames(&mut h, 4);
    assert_eq!(
        h.state().dk.clip,
        Some(pid),
        "the kit's step grid edits the pattern"
    );
    // the step grid's pie: Fill every beat on the kick lane
    let cell = h
        .state()
        .dk
        .cells
        .iter()
        .find(|c| c.0 == 0 && c.1 == 0)
        .unwrap()
        .2;
    right_click(&mut h, cell.center());
    pick(&mut h, "Fill", Some("Every beat"));
    let data = clip(&h, pid).notes().unwrap();
    let hits = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().notes[data].notes.len());
    assert!(hits >= 4, "{hits} hits");
    // the clip's pie: To launcher (a linked copy in a slot of track 02)
    let p = at(&h, 1, BAR / 4);
    right_click(&mut h, p);
    pick(&mut h, "To launcher", None);
    frames(&mut h, 3);
    assert!(
        h.state().ca.launch,
        "the grid shows the launcher: {}",
        h.state().status
    );
    let lid = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        tl::launch_at(t, 1, 0).map(|l| l.clip.id)
    });
    assert!(lid.is_some(), "{}", h.state().status);
    assert_eq!(links(&h, data), 2);
    // drag the slot onto track 02 at bar 5, then Ctrl+D
    let from = slot(&h, 1, 0).center() + egui::vec2(10.0, 0.0);
    let to = at(&h, 1, 4 * BAR);
    drag_to(&mut h, from, to);
    let c = newest(&h);
    assert_eq!((c.track, c.start, c.notes()), (1, 4 * BAR, Some(data)));
    key(&mut h, egui::Key::D, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    let d = newest(&h);
    assert_ne!(d.id, c.id);
    assert_eq!(
        (d.start, d.notes()),
        (c.end(), Some(data)),
        "Ctrl+D: right after, linked"
    );
    assert_eq!(links(&h, data), 4, "pattern, launcher, drop, duplicate");
    // render: the kit sounds
    let s = song(&h);
    assert!(peak(&s) > 0.01, "the pattern sounds");
    round_trip(&s, "a");
}

#[test]
fn scenario_b_waveforge_new_launcher_clip_drag_render() {
    let (mut h, _) = notes_song();
    // track 01 has no instrument: the lower panel offers the chooser
    h.state_mut().tl_select_track(0, false);
    h.state_mut().set_lower(1);
    frames(&mut h, 3);
    let r = exact(&h, crate::poly::POLY_DISPLAY_NAME);
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let wf = h
        .state()
        .track_instrument(0)
        .expect("WaveForge on track 01");
    // Capture, + Scene, double-click the empty slot: a new launcher clip
    let r = exact(&h, "Capture");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let r = exact(&h, "+ Scene");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let e = slot(&h, 0, 1);
    double_click(&mut h, e.center());
    frames(&mut h, 3);
    let lid = h.state().pr.launch.expect("open in the piano roll");
    // draw a note in the roll
    let key_n = 60u8;
    let p = Pos2::new(
        h.state().pr.x_of(0) + 5.0,
        h.state().pr.y_of(key_n) + h.state().pr.key_h / 2.0,
    );
    click_at(&mut h, p);
    frames(&mut h, 2);
    let (notes, data) = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let d = t.launch_clip(lid).unwrap().clip.notes().unwrap();
        (t.notes[d].notes.clone(), d)
    });
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].instr == wf || notes[0].instr == 0,
        "plays the track's WaveForge"
    );
    // drag it onto the arrangement: linked
    let from = slot(&h, 0, 1).center() + egui::vec2(10.0, 0.0);
    let to = at(&h, 0, 4 * BAR);
    drag_to(&mut h, from, to);
    let c = newest(&h);
    assert_eq!((c.track, c.start, c.notes()), (0, 4 * BAR, Some(data)));
    let s = song(&h);
    assert!(peak(&s) > 0.01, "WaveForge sounds");
    round_trip(&s, "b");
}

#[test]
fn scenario_c_chords_two_launcher_versions_place_one() {
    let (mut h, a) = notes_song();
    h.state().with(|s| {
        s.song
            .instruments
            .push(fileio::demo_song().instruments[0].clone());
        s.song.timeline.as_deref_mut().unwrap().tracks[0].instr = 1;
    });
    // chords into an empty clip (the chord lane's Write)
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let d = t.clips.iter().find(|c| c.id == a).unwrap().notes().unwrap();
        t.notes[d].make_mut().notes.clear();
    });
    h.state_mut().tlv.sel = Some(a);
    h.state_mut().tlv.sels = vec![a];
    h.state_mut().timeline_open_roll();
    frames(&mut h, 3);
    let t = h.state().pr.target.unwrap();
    let mark = |start| crate::chordgen::ChordMark {
        start,
        len: BAR,
        root: 0,
        q: crate::chordgen::Quality::Maj,
        bass: None,
    };
    // the chord lane's markers and their notes (Triads, as the Generate
    // dialog writes them)
    let marks = vec![mark(0), mark(BAR)];
    let notes = crate::chordgen::voice(&marks, crate::chordgen::Voicing::Triads, 4)
        .into_iter()
        .map(|(start, len, key)| tl::Note {
            start,
            len,
            key,
            vel: tl::roll::vel_u16(0.75),
            ..Default::default()
        })
        .collect();
    h.state_mut().cl_write(t, marks, notes, true);
    frames(&mut h, 2);
    let data = clip(&h, a).notes().unwrap();
    let at0 = h.state().with(|s| {
        s.song.timeline.as_deref().unwrap().notes[data]
            .notes
            .iter()
            .filter(|n| n.start == 0)
            .count()
    });
    assert!(
        at0 >= 3,
        "a chord at bar 1: {at0} notes ({:?})",
        h.state().cl.confirm.is_some()
    );
    // To launcher, then its pie's Variation in the launcher
    let p = at(&h, 0, BAR / 4);
    right_click(&mut h, p);
    pick(&mut h, "To launcher", None);
    frames(&mut h, 3);
    let l1 = h.state().with(|s| {
        tl::launch_at(s.song.timeline.as_deref().unwrap(), 0, 0)
            .unwrap()
            .clip
            .id
    });
    let r = slot(&h, 0, 0).center() + egui::vec2(10.0, 0.0);
    right_click(&mut h, r);
    pick(&mut h, "Variation", None);
    frames(&mut h, 2);
    let (l2, d2) = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let l = tl::launch_at(t, 0, 1).expect("the variation in scene 2");
        (l.clip.id, l.clip.notes().unwrap())
    });
    assert_ne!(l2, l1);
    assert_ne!(d2, data, "its own notes");
    // change the variation: up a whole tone
    h.state().with(|s| {
        for n in &mut s.song.timeline.as_deref_mut().unwrap().notes[d2]
            .make_mut()
            .notes
        {
            n.key += 2;
        }
    });
    frames(&mut h, 2);
    // place the variation after the arrangement's clip, from its pie
    let r = slot(&h, 0, 1).center() + egui::vec2(10.0, 0.0);
    right_click(&mut h, r);
    pick(&mut h, "Place", Some("After the last clip"));
    let c = newest(&h);
    assert_eq!(
        (c.track, c.start, c.notes()),
        (0, clip(&h, a).end(), Some(d2))
    );
    assert_eq!(links(&h, data), 2, "the original: arrangement + launcher");
    // Make unique on the placed one, then save / load keeps it apart
    let p = at(&h, 0, c.start + BAR / 4);
    right_click(&mut h, p);
    pick(&mut h, "Make unique", None);
    let c2 = clip(&h, c.id);
    assert_ne!(c2.notes(), Some(d2));
    let s = song(&h);
    assert!(peak(&s) > 0.01);
    let back = round_trip(&s, "c");
    let bt = back.timeline.as_deref().unwrap();
    assert_eq!(bt.launcher.len(), 2);
    assert_eq!(tl::linked(bt, data), (1, 1));
}

/// songs from older versions: load, save, load renders the same and the
/// file has no launcher (they keep opening in the old version)
#[test]
fn older_songs_round_trip_unchanged() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for f in ["chipwave-0.2.3.5-song.cwt", "chipwave-0.2.3.6-song.cwt"] {
        let s = fileio::load_cwt(&dir.join(f)).expect(f);
        assert!(!fileio::needs_reader_6(&s), "{f}");
        let mut t = s.clone();
        tl::enter_timeline_mode(&mut t);
        assert!(!fileio::needs_reader_6(&t), "{f} in timeline mode");
        let tmp = std::env::temp_dir().join(format!("cw-old-{}-{f}", std::process::id()));
        fileio::save_cwt(&s, &tmp).unwrap();
        let back = fileio::load_cwt(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        let r = |s: &Song| crate::engine::render_song(s, 11025);
        assert!(r(&s) == r(&back), "{f} renders the same after a save");
    }
    let _ = key;
}
