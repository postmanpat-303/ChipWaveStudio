//! Stage 3 (docs/unified/PLAN.md): one "track's instrument" rule (D6); a
//! header click shows the instrument's editor, a clip click the clip's;
//! a track without an instrument offers the chooser.
use super::ui_tests::{click_at, exact};
use super::unified_tests::{frames, notes_song};
use super::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn head_spot(h: &Harness<'static, App>, tr: usize) -> Pos2 {
    let (hr, _) = h.state().tlv.track_rects(tr).expect("track drawn");
    Pos2::new(hr.left() + 24.0, hr.top() + 10.0)
}

/// a song whose track 01 plays instrument 02 from its notes only (no
/// `Track::instr`), track 02 nothing
fn two_instrument_song() -> (Harness<'static, App>, u32) {
    let (mut h, id) = notes_song();
    h.state().with(|s| {
        let d = fileio::demo_song();
        s.song.instruments.push(d.instruments[0].clone());
        s.song.instruments.push(d.instruments[1].clone());
        let t = s.song.timeline.as_deref_mut().unwrap();
        for tr in &mut t.tracks {
            tr.instr = 0;
        }
        let data = t
            .clips
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .notes()
            .unwrap();
        for n in &mut t.notes[data].make_mut().notes {
            n.instr = 2;
        }
    });
    h.state_mut().instr = 1;
    frames(&mut h, 3);
    (h, id)
}

#[test]
fn every_path_agrees_on_the_tracks_instrument() {
    let (mut h, _) = two_instrument_song();
    let st = h.state();
    assert_eq!(st.track_instrument(0), Some(2), "the one its notes play");
    assert_eq!(st.track_slot(0), 2, "Browser / Devices / mixer");
    assert_eq!(st.track_instr(0), 2, "typing and auditions");
    assert_eq!(st.track_instrument(1), None, "nothing on track 02");
    // selecting the track picks it
    let p = head_spot(&h, 0);
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!((h.state().ch, h.state().instr), (0, 2));
    // an assigned instrument wins; a deleted one doesn't count
    h.state()
        .with(|s| s.song.timeline.as_deref_mut().unwrap().tracks[0].instr = 1);
    assert_eq!(h.state().track_instrument(0), Some(1));
    h.state()
        .with(|s| s.song.timeline.as_deref_mut().unwrap().tracks[0].instr = 9);
    assert_eq!(h.state().track_instrument(0), Some(2));
}

#[test]
fn a_header_shows_the_instrument_and_a_clip_shows_the_clip() {
    let (mut h, id) = two_instrument_song();
    h.state_mut().set_lower(3);
    frames(&mut h, 2);
    assert!(h.state().pr.open);
    let p = head_spot(&h, 0);
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert!(
        h.state().extra.lower_inst && !h.state().pr.open,
        "the instrument editor"
    );
    // a clip click: the piano roll on that clip
    let r = h.state().tlv.clip_rect(id).expect("clip drawn");
    click_at(&mut h, r.center() + egui::vec2(0.0, 6.0));
    frames(&mut h, 2);
    assert_eq!(h.state().tlv.sel, Some(id));
    assert!(
        h.state().pr.open && !h.state().extra.lower_inst,
        "the clip editor"
    );
    // the setting off: the lower panel stays as it is
    h.state_mut().extra.follow_select = false;
    let p = head_spot(&h, 0);
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert!(h.state().pr.open, "still the piano roll");
}

#[test]
fn a_track_without_an_instrument_offers_the_chooser() {
    let (mut h, _) = two_instrument_song();
    let p = head_spot(&h, 1);
    click_at(&mut h, p);
    frames(&mut h, 3);
    assert_eq!(h.state().ch, 1);
    assert!(h.state().chooser_wanted());
    let label = "Track 02 has no instrument · Choose an instrument:";
    assert!(h.query_by_label(label).is_some());
    let n0 = h.state().with(|s| s.song.instruments.len());
    let r = exact(&h, "Drum Kit");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let (n1, given) = h.state().with(|s| {
        (
            s.song.instruments.len(),
            s.song.timeline.as_deref().unwrap().tracks[1].instr,
        )
    });
    assert_eq!(n1, n0 + 1, "a new Drum Kit");
    assert_eq!(given as usize, n1, "track 02 plays it");
    assert!(!h.state().chooser_wanted());
    assert!(h.query_by_label(label).is_none());
    assert!(h.state().extra.lower_inst, "its editor shows");
}

#[test]
fn a_shared_instrument_says_so_and_a_track_gets_its_own_copy() {
    let (mut h, id) = two_instrument_song();
    // track 02 plays instrument 02 too (assigned)
    h.state()
        .with(|s| s.song.timeline.as_deref_mut().unwrap().tracks[1].instr = 2);
    frames(&mut h, 2);
    let p = head_spot(&h, 0);
    click_at(&mut h, p);
    frames(&mut h, 3);
    assert_eq!(h.state_mut().shared_with(), vec![1]);
    let label = "Also used by track 02: edits here change it too";
    assert!(h.query_by_label(label).is_some());
    let n0 = h.state().with(|s| s.song.instruments.len());
    let r = exact(&h, "Make a separate copy for this track");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let (n1, mine, theirs, notes) = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let d = t
            .clips
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .notes()
            .unwrap();
        (
            s.song.instruments.len(),
            t.tracks[0].instr,
            t.tracks[1].instr,
            t.notes[d].notes.iter().map(|n| n.instr).collect::<Vec<_>>(),
        )
    });
    assert_eq!(n1, n0 + 1);
    assert_eq!((mine as usize, theirs), (n1, 2));
    assert!(notes.iter().all(|&i| i as usize == n1), "{notes:?}");
    assert_eq!(h.state().instr as usize, n1, "its editor edits the copy");
    assert!(h.query_by_label(label).is_none());
    assert!(h.state_mut().shared_with().is_empty());
    // one undo step
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(h.state().with(|s| s.song.instruments.len()), n0);
}
