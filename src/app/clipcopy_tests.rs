//! Timeline clip clipboard (Del, 25 Oct 2026): Ctrl+C / X / V on clips,
//! one or many across tracks, paste on the grid at the play marker or the
//! clicked spot, independent copies, one undo step each, and the
//! context-sensitive pies (clip: Copy / Cut, empty lane: Paste here).
use super::objpie::PieFor;
use super::objpie_tests::{pick, right_click};
use super::ui_tests::key;
use super::unified_tests::{at, clip, frames, notes_song, BAR};
use super::*;
use crate::timeline::{self as tl, Tick, PPQ};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

const CMD: egui::Modifiers = egui::Modifiers::COMMAND;

fn clips(h: &Harness<'static, App>) -> Vec<tl::Clip> {
    h.state()
        .with(|s| s.song.timeline.as_deref().unwrap().clips.clone())
}

fn select(h: &mut Harness<'static, App>, ids: &[u32]) {
    h.state_mut().pr.focus = false;
    h.state_mut().tlv.sels = ids.to_vec();
    h.state_mut().tlv.sel = ids.first().copied();
    frames(h, 1);
}

/// a second note clip: track 1, bar 2, 1 bar long
fn second(h: &mut Harness<'static, App>) -> u32 {
    let id = h.state().with(|s| {
        let id = tl::new_note_clip(&mut s.song, 1, BAR).unwrap();
        let t = s.song.timeline.as_deref_mut().unwrap();
        t.clips.iter_mut().find(|c| c.id == id).unwrap().len = BAR;
        id
    });
    h.state_mut().undo_reset();
    frames(h, 2);
    id
}

fn notes_of(h: &Harness<'static, App>, c: &tl::Clip) -> Vec<tl::Note> {
    let d = c.notes().unwrap();
    h.state()
        .with(|s| s.song.timeline.as_deref().unwrap().notes[d].notes.clone())
}

/// `notes_song` with at least 4 tracks
fn song4() -> (Harness<'static, App>, u32) {
    let (mut h, a) = notes_song();
    while h.state().with(|s| s.song.channels) < 4 {
        h.get_by_label("+ Add track").click();
        frames(&mut h, 3);
    }
    h.state_mut().undo_reset();
    (h, a)
}

#[test]
fn copy_and_paste_one_clip_at_the_play_marker_on_the_selected_track() {
    let (mut h, a) = song4();
    select(&mut h, &[a]);
    key(&mut h, Key::C, CMD);
    assert!(
        h.state().status.starts_with("Copied 1 clip "),
        "{}",
        h.state().status
    );
    let n = clips(&h).len();
    h.state_mut().ch = 2;
    h.state_mut().tlv.cue = 8 * BAR + 37;
    h.state_mut().tlv.head_drawn = None;
    key(&mut h, Key::V, CMD);
    let all = clips(&h);
    assert_eq!(all.len(), n + 1, "{}", h.state().status);
    let p = all
        .iter()
        .find(|c| c.id != a && c.track == 2)
        .expect("pasted");
    assert_eq!(p.start, 8 * BAR, "on the bar grid at the play marker");
    assert_eq!(p.len, clip(&h, a).len);
    assert_eq!(notes_of(&h, p), notes_of(&h, &clip(&h, a)));
    assert!(
        h.state().status.starts_with("Pasted 1 clip at bar 9"),
        "{}",
        h.state().status
    );
    assert_eq!(h.state().tlv.sels, vec![p.id], "the copy is selected");
    assert!(h.state().tlv.pasted.is_some(), "it glows");
    // one undo step
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(clips(&h).len(), n);
}

#[test]
fn many_clips_keep_their_track_and_time_offsets() {
    let (mut h, a) = song4();
    let b = second(&mut h);
    select(&mut h, &[a, b]);
    key(&mut h, Key::C, CMD);
    assert!(h.state().status.starts_with("Copied 2 clips"));
    let before: Vec<u32> = clips(&h).iter().map(|c| c.id).collect();
    h.state_mut().tl_paste(Some((1, 4 * BAR)), false);
    frames(&mut h, 2);
    let new: Vec<tl::Clip> = clips(&h)
        .into_iter()
        .filter(|c| !before.contains(&c.id))
        .collect();
    assert_eq!(new.len(), 2);
    let mut got: Vec<(u16, Tick)> = new.iter().map(|c| (c.track, c.start)).collect();
    got.sort_unstable();
    assert_eq!(got, vec![(1, 4 * BAR), (2, 5 * BAR)]);
    assert!(h.state().status.starts_with("Pasted 2 clips at bar 5"));
    // on the last track the group moves up so every clip has a track
    let before: Vec<u32> = clips(&h).iter().map(|c| c.id).collect();
    let last = h.state().with(|s| s.song.channels) - 1;
    h.state_mut().tl_paste(Some((last, 8 * BAR)), false);
    frames(&mut h, 2);
    let mut got: Vec<(u16, Tick)> = clips(&h)
        .into_iter()
        .filter(|c| !before.contains(&c.id))
        .map(|c| (c.track, c.start))
        .collect();
    got.sort_unstable();
    let l = last as u16;
    assert_eq!(got, vec![(l - 1, 8 * BAR), (l, 9 * BAR)]);
}

#[test]
fn cut_removes_them_and_one_undo_puts_them_back() {
    let (mut h, a) = song4();
    let b = second(&mut h);
    select(&mut h, &[a, b]);
    let n = clips(&h).len();
    key(&mut h, Key::X, CMD);
    assert!(
        h.state().status.starts_with("Cut 2 clips"),
        "{}",
        h.state().status
    );
    assert_eq!(clips(&h).len(), n - 2);
    key(&mut h, Key::Z, CMD);
    frames(&mut h, 2);
    let back = clips(&h);
    assert_eq!(back.len(), n, "one Ctrl+Z");
    assert!(back.iter().any(|c| c.id == a) && back.iter().any(|c| c.id == b));
    // the clipboard survives the undo: paste still works
    h.state_mut().tl_paste(Some((0, 6 * BAR)), false);
    frames(&mut h, 2);
    assert_eq!(clips(&h).len(), n + 2);
}

#[test]
fn duplicate_lands_right_after_the_selection() {
    let (mut h, a) = song4();
    let b = second(&mut h);
    select(&mut h, &[a, b]);
    key(&mut h, Key::D, CMD);
    let (ca, cb) = (clip(&h, a), clip(&h, b));
    let end = ca.end().max(cb.end());
    let span = end - ca.start.min(cb.start);
    let new: Vec<tl::Clip> = clips(&h)
        .into_iter()
        .filter(|c| c.id != a && c.id != b)
        .collect();
    assert_eq!(new.len(), 2);
    for (o, c) in [(&ca, 0u16), (&cb, 1)] {
        assert!(
            new.iter()
                .any(|n| n.track == c && n.start == o.start + span),
            "copy of track {c} at {} in {new:?}",
            o.start + span
        );
    }
    assert_eq!(span, 2 * BAR);
}

#[test]
fn paste_snaps_to_the_grid() {
    let (mut h, a) = song4();
    select(&mut h, &[a]);
    key(&mut h, Key::C, CMD);
    let grid = h.state().tlv.snap_grid();
    assert_eq!(grid, BAR, "Snap: bar");
    assert_eq!(
        h.state().tl_paste_spot(Some((1, 3 * BAR + PPQ + 5))),
        (1, 3 * BAR)
    );
    h.state_mut().tlv.snap = super::timeline::Snap::Beat;
    frames(&mut h, 2);
    assert_eq!(
        h.state().tl_paste_spot(Some((1, 3 * BAR + PPQ + 5))),
        (1, 3 * BAR + PPQ)
    );
    h.state_mut().tl_paste(Some((1, 3 * BAR + PPQ + 5)), false);
    frames(&mut h, 2);
    assert!(clips(&h)
        .iter()
        .any(|c| c.track == 1 && c.start == 3 * BAR + PPQ));
    assert!(h.state().status.contains("bar 4.2"), "{}", h.state().status);
}

#[test]
fn pasted_clips_are_independent_and_linked_paste_shares() {
    let (mut h, a) = song4();
    select(&mut h, &[a]);
    key(&mut h, Key::C, CMD);
    let orig = notes_of(&h, &clip(&h, a));
    h.state_mut().tl_paste(Some((1, 4 * BAR)), false);
    frames(&mut h, 2);
    let p = clips(&h)
        .into_iter()
        .find(|c| c.track == 1 && c.start == 4 * BAR)
        .unwrap();
    assert_ne!(p.notes(), clip(&h, a).notes(), "its own notes");
    // editing the copy leaves the original alone
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        t.notes[p.notes().unwrap()].make_mut().notes.clear();
    });
    assert_eq!(notes_of(&h, &clip(&h, a)), orig);
    // and the clipboard is a snapshot: editing the original after copying
    // does not change what pastes
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let d = t.clips.iter().find(|c| c.id == a).unwrap().notes().unwrap();
        t.notes[d].make_mut().notes.truncate(1);
    });
    h.state_mut().tl_paste(Some((2, 4 * BAR)), false);
    frames(&mut h, 2);
    let q = clips(&h)
        .into_iter()
        .find(|c| c.track == 2 && c.start == 4 * BAR)
        .unwrap();
    assert_eq!(notes_of(&h, &q), orig);
    // Ctrl+Shift+V / Linked copy: the same notes as the (live) original
    h.state_mut().tl_paste(Some((0, 8 * BAR)), true);
    frames(&mut h, 2);
    let l = clips(&h)
        .into_iter()
        .find(|c| c.track == 0 && c.start == 8 * BAR)
        .unwrap();
    assert_eq!(l.notes(), clip(&h, a).notes(), "linked");
    assert!(h.state().status.contains("linked"));
}

#[test]
fn nothing_copied_says_so() {
    let (mut h, _) = notes_song();
    h.state_mut().pr.focus = false;
    let n = clips(&h).len();
    key(&mut h, Key::V, CMD);
    assert!(
        h.state().status.starts_with("Nothing copied"),
        "{}",
        h.state().status
    );
    assert_eq!(clips(&h).len(), n);
    h.state_mut().tlv.sels.clear();
    key(&mut h, Key::C, CMD);
    assert!(h.state().status.starts_with("Select clips to copy"));
}

#[test]
fn clip_pie_copies_and_the_empty_lane_pie_pastes_here() {
    let (mut h, a) = song4();
    // empty lane, nothing copied: the pie greys Paste
    let p = at(&h, 1, 5 * BAR + PPQ);
    right_click(&mut h, p);
    let on = h.state().opie.as_ref().map(|x| x.1);
    assert!(
        matches!(on, Some(PieFor::Lane { track: 1, at }) if (5 * BAR..6 * BAR).contains(&at)),
        "{on:?}"
    );
    let first = h.state().opie.as_ref().unwrap().0.items[0].clone();
    assert_eq!(first.label, "Nothing copied");
    assert!(first.dim);
    h.state_mut().opie = None;
    frames(&mut h, 2);
    // the clip's pie: Copy
    let p = at(&h, 0, BAR / 2);
    right_click(&mut h, p);
    assert_eq!(h.state().opie.as_ref().map(|x| x.1), Some(PieFor::Clip(a)));
    let labels: Vec<String> = h
        .state()
        .opie
        .as_ref()
        .unwrap()
        .0
        .items
        .iter()
        .map(|x| x.label.clone())
        .collect();
    for want in ["Copy", "Cut", "Duplicate", "Delete", "Make unique"] {
        assert!(labels.iter().any(|x| x == want), "{want} in {labels:?}");
    }
    pick(&mut h, "Copy", None);
    assert!(
        h.state().status.starts_with("Copied 1 clip"),
        "{}",
        h.state().status
    );
    // the empty lane now offers Paste here, at the clicked spot on the grid
    let n = clips(&h).len();
    let p = at(&h, 1, 5 * BAR + PPQ);
    right_click(&mut h, p);
    assert_eq!(
        h.state().opie.as_ref().unwrap().0.items[0].label,
        "Paste here"
    );
    pick(&mut h, "Paste here", Some("Independent copy"));
    let all = clips(&h);
    assert_eq!(all.len(), n + 1, "{}", h.state().status);
    assert!(all.iter().any(|c| c.track == 1 && c.start == 5 * BAR));
    // the track pie: Paste at playhead (only while something is copied)
    let i = h.state().ring_info_for(2, false);
    assert!(i.paste);
    let slots = super::trackring::ring_slots(super::trackring::Level::Add, &i);
    assert!(slots.iter().any(|s| s.label == "Paste at playhead"));
}

/// what the window system sends: Ctrl+C / Ctrl+X arrive as Copy / Cut
/// events (no key event), Ctrl+V as Paste(system clipboard text), or
/// nothing at all when that clipboard holds no text
fn os_copy(h: &mut Harness<'static, App>) -> Option<String> {
    h.input_mut().events.push(egui::Event::Copy);
    super::ui_tests::frame(h);
    let text = h
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        });
    frames(h, 1);
    text
}

/// Del (25 Oct 2026): "the hot ctrl-c and ctrl-v do not function" — a
/// real run: click into the piano roll, then click a clip with the mouse,
/// and the window system's Copy / Paste events (not key presses) copy
/// and paste it; the copy fills the system clipboard so a Ctrl+V on an
/// empty one still arrives; a picture on it still pastes the clips
#[test]
fn clicked_clip_copies_and_pastes_with_the_window_systems_events() {
    use super::objpie_tests::press;
    let (mut h, a) = song4();
    h.state_mut().tlv.sels.clear();
    h.state_mut().tlv.sel = None;
    // the piano roll had the keyboard (clicked into last)
    h.state_mut().pr.open = true;
    h.state_mut().pr.focus = true;
    frames(&mut h, 2);
    let p = at(&h, 0, BAR / 2);
    press(&mut h, p, egui::PointerButton::Primary, true);
    press(&mut h, p, egui::PointerButton::Primary, false);
    frames(&mut h, 2);
    assert_eq!(h.state().tlv.sels, vec![a], "the click selects the clip");
    assert!(
        !h.state().pr_active(),
        "the Timeline has the keyboard again"
    );
    let text = os_copy(&mut h);
    assert!(
        h.state().status.starts_with("Copied 1 clip"),
        "{}",
        h.state().status
    );
    let text = text.expect("the copy fills the system clipboard");
    assert!(text.starts_with("ChipWave: 1 timeline clip"), "{text}");
    // Ctrl+V: Paste(that text)
    let n = clips(&h).len();
    h.state_mut().ch = 2;
    h.state_mut().tlv.cue = 4 * BAR;
    h.state_mut().tlv.head_drawn = None;
    h.input_mut().events.push(egui::Event::Paste(text));
    frames(&mut h, 2);
    let all = clips(&h);
    assert_eq!(all.len(), n + 1, "{}", h.state().status);
    assert!(all.iter().any(|c| c.track == 2 && c.start == 4 * BAR));
    // a picture on the system clipboard: PasteImage still pastes clips
    h.state_mut().tlv.cue = 6 * BAR;
    h.input_mut()
        .events
        .push(egui::Event::PasteImage(std::sync::Arc::new(
            egui::ColorImage::filled([1, 1], egui::Color32::BLACK),
        )));
    frames(&mut h, 2);
    let all = clips(&h);
    assert_eq!(all.len(), n + 2, "{}", h.state().status);
    assert!(all.iter().any(|c| c.track == 2 && c.start == 6 * BAR));
    // Ctrl+X: Cut
    let id = h.state().tlv.sels[0];
    h.input_mut().events.push(egui::Event::Cut);
    frames(&mut h, 2);
    assert!(clips(&h).iter().all(|c| c.id != id), "cut");
}

/// "there is no copy in the menu pie": a linked clip ("Take 1 ×3") has
/// Copy and Cut in its pie, and Copy then Ctrl+V pastes it
#[test]
fn a_linked_clips_pie_has_copy_and_cut() {
    let (mut h, a) = song4();
    select(&mut h, &[a]);
    for _ in 0..2 {
        h.state_mut().tl_apply(super::timeline::TlOp::Duplicate);
        frames(&mut h, 1);
    }
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        t.clips.iter_mut().find(|c| c.id == a).unwrap().name = Some("Take 1".into());
    });
    let d = clip(&h, a).notes().unwrap();
    let (x, y) = h
        .state()
        .with(|s| tl::linked(s.song.timeline.as_deref().unwrap(), d));
    assert_eq!(x + y, 3, "three clips share the notes");
    frames(&mut h, 2);
    let p = at(&h, 0, BAR / 2);
    super::objpie_tests::right_click(&mut h, p);
    assert_eq!(h.state().opie.as_ref().map(|x| x.1), Some(PieFor::Clip(a)));
    let labels: Vec<String> = h
        .state()
        .opie
        .as_ref()
        .unwrap()
        .0
        .items
        .iter()
        .map(|x| x.label.clone())
        .collect();
    for want in ["Copy", "Cut"] {
        assert!(labels.iter().any(|x| x == want), "{want} in {labels:?}");
    }
    pick(&mut h, "Copy", None);
    assert!(
        h.state().status.starts_with("Copied 1 clip"),
        "{}",
        h.state().status
    );
    let n = clips(&h).len();
    h.state_mut().ch = 3;
    h.state_mut().tlv.cue = 6 * BAR;
    h.state_mut().tlv.head_drawn = None;
    key(&mut h, Key::V, CMD);
    assert_eq!(clips(&h).len(), n + 1, "{}", h.state().status);
}
