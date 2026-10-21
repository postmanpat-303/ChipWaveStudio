//! The unified workflow (docs/unified/PLAN.md): Stage 2, the Timeline stays
//! on top. The sample editor, Open in Tracker and the effects open under
//! it; the lower panel leaves at least three track rows (1366x768) and the
//! current track scrolls into view; the panel header names what it edits.
use super::ui_tests::{click_at, exact, frame, harness_with, loop_instrument};
use super::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

pub(super) fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// `song` in timeline mode on the Timeline, the Tracker hidden (the default)
pub(super) fn tl_song(song: Song) -> Harness<'static, App> {
    let mut s = song;
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().extra.show_tracker = false;
    h.state_mut().default_layout();
    frames(&mut h, 3);
    h
}

pub(super) fn tl_demo() -> Harness<'static, App> {
    tl_song(fileio::demo_song())
}

pub(super) fn laptop(h: &mut Harness<'static, App>) {
    h.set_size(egui::vec2(1366.0, 768.0));
    frames(h, 3);
}

fn lanes_rows(h: &Harness<'static, App>) -> f32 {
    h.state().tlv.lanes.height() / h.state().tlv.track_h_pts()
}

#[test]
fn open_in_sample_editor_keeps_the_timeline_on_top() {
    let mut s = fileio::demo_song();
    s.instruments.push(loop_instrument());
    let slot = s.instruments.len() as u8;
    let mut h = tl_song(s);
    h.state_mut().instr = slot;
    h.state_mut().show_instrument_panel();
    frames(&mut h, 3);
    let r = exact(&h, "Open in Sample editor");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    assert_eq!(h.state().view, View::Timeline, "the timeline stays");
    assert!(h.state().ip.sample && h.state().sample_shown());
    assert!(h.state().tlv.lanes.height() > 0.0, "the timeline is drawn");
    exact(&h, "Sample editor");
    // the sample editor's own tools work there (Transients slices it)
    let r = exact(&h, "Transients");
    click_at(&mut h, r.center());
    assert_eq!(h.state().smp.slices.len(), 8, "{:?}", h.state().smp.slices);
    // Ctrl+Z over the sample editor uses its history, elsewhere the song's
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(Pos2::new(700.0, 20.0)));
    frames(&mut h, 2);
    assert!(!h.state().sample_undo_here(), "pointer on the menu bar");
    let r = exact(&h, "Sample editor");
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(r.center()));
    frames(&mut h, 2);
    assert!(
        h.state().sample_undo_here(),
        "pointer over the sample editor"
    );
    // back to the knobs, and Full view on request
    let r = exact(&h, "Back to the knobs");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert!(!h.state().ip.sample && !h.state().sample_shown());
    h.state_mut().ip.sample = true;
    frames(&mut h, 2);
    let r = exact(&h, "Full view  Ctrl+5");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Sample);
}

#[test]
fn without_the_tracker_tracker_clips_open_in_the_piano_roll() {
    let mut h = tl_demo();
    assert!(!h.state().tracker_on());
    h.state_mut().set_lower(1);
    h.state_mut().timeline_open(2, 1, 0, usize::MAX);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Timeline, "the timeline stays");
    assert!(h.state().pr.open, "the piano roll shows the clip");
    assert_eq!((h.state().pattern, h.state().ch), (2, 1));
    // with the Tracker on, Open in Tracker still opens the Tracker
    h.state_mut().extra.show_tracker = true;
    h.state_mut().timeline_open(2, 1, 0, usize::MAX);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Pattern);
}

#[test]
fn the_lower_panel_leaves_three_track_rows_on_a_laptop() {
    let mut h = tl_demo();
    laptop(&mut h);
    for m in [3u8, 1, 4] {
        h.state_mut().set_lower(m);
        // even when dragged as high as it goes
        h.state_mut().extra.lower_h = 5000.0;
        frames(&mut h, 3);
        let rows = lanes_rows(&h);
        assert!(rows >= 2.95, "lower tab {m}: {rows:.2} track rows visible");
    }
}

#[test]
fn opening_the_lower_panel_scrolls_the_current_track_into_view() {
    let mut h = tl_demo();
    laptop(&mut h);
    h.state_mut().extra.full_height = true;
    frames(&mut h, 2);
    let last = h.state().with(|s| s.song.channels) - 1;
    h.state_mut().ch = last;
    h.state_mut().tlv.scroll_y = 0.0;
    frames(&mut h, 2);
    h.state_mut().set_lower(1);
    frames(&mut h, 4);
    let (_, row) = h.state().tlv.track_rects(last).expect("track row");
    let lanes = h.state().tlv.lanes;
    assert!(
        row.top() >= lanes.top() - 0.5 && row.bottom() <= lanes.bottom() + 0.5,
        "track {last} row {row:?} inside the lanes {lanes:?}"
    );
}

#[test]
fn the_lower_panel_header_names_the_track_and_its_instrument() {
    let mut h = tl_demo();
    let slot = h.state().track_slot(0);
    assert!(slot > 0);
    h.state_mut().ch = 0;
    h.state_mut().instr = slot;
    h.state_mut().set_lower(1);
    frames(&mut h, 2);
    let name = h.state().with(|s| s.song.channel_names[0].clone());
    let txt = h.state().lower_breadcrumb().expect("breadcrumb");
    assert!(
        txt.starts_with(&format!("Track 01 {name}  ›  {slot:02X} ")),
        "{txt}"
    );
    exact(&h, &txt);
    // another instrument than the track's: only the instrument
    let other = if slot == 1 { 2 } else { 1 };
    h.state_mut().instr = other;
    frames(&mut h, 2);
    let txt = h.state().lower_breadcrumb().expect("breadcrumb");
    assert!(txt.starts_with(&format!("{other:02X} ")), "{txt}");
}

// ---- Stage 4a: one drop system (preview, snap, overlap prompt) ----

/// a new song with two tracks, the Clip Arranger on, a take of 8 notes at
/// bar 1 on track 01; returns the harness and the clip id
pub(super) fn notes_song() -> (Harness<'static, App>, u32) {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().extra.show_tracker = false;
    h.state_mut().extra.reduce_motion = true;
    h.state_mut().extra.clip_arranger = true;
    frames(&mut h, 2);
    h.state_mut().new_song();
    frames(&mut h, 3);
    for _ in 0..2 {
        h.get_by_label("+ Add track").click();
        frames(&mut h, 3);
    }
    h.state_mut().view = View::Timeline;
    h.state_mut().tlv.snap = super::timeline::Snap::Bar;
    let id = h.state().with(|s| {
        let id = crate::timeline::new_note_clip(&mut s.song, 0, 0).unwrap();
        let t = s.song.timeline.as_deref_mut().unwrap();
        let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
        c.len = 2 * 4 * crate::timeline::PPQ;
        let data = c.notes().unwrap();
        t.notes[data].make_mut().notes = (0..8)
            .map(|k| crate::timeline::Note {
                start: k * crate::timeline::PPQ,
                len: crate::timeline::PPQ / 2,
                key: 49 + k as u8,
                ..Default::default()
            })
            .collect();
        id
    });
    frames(&mut h, 4);
    assert!(h.state().ca.secs.is_some(), "note clips: section scenes");
    (h, id)
}

pub(super) fn clip(h: &Harness<'static, App>, id: u32) -> crate::timeline::Clip {
    h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .unwrap()
    })
}

/// the clip a drop placed (it is selected)
pub(super) fn newest(h: &Harness<'static, App>) -> crate::timeline::Clip {
    let id = h.state().tlv.sel.expect("the placed clip is selected");
    clip(h, id)
}

fn n_clips(h: &Harness<'static, App>) -> usize {
    h.state()
        .with(|s| s.song.timeline.as_deref().unwrap().clips.len())
}

/// the screen point of `tick` on track `tr` (as last drawn)
pub(super) fn at(h: &Harness<'static, App>, tr: usize, tick: crate::timeline::Tick) -> Pos2 {
    let (_, row) = h.state().tlv.track_rects(tr).unwrap();
    Pos2::new(h.state().tlv.x_of(tick) + 6.0, row.center().y)
}

pub(super) const BAR: crate::timeline::Tick = 4 * crate::timeline::PPQ;

#[test]
fn dragging_a_note_slot_makes_a_linked_copy_on_the_bar_with_a_preview() {
    let (mut h, a) = notes_song();
    let whole = h
        .state()
        .ca
        .slot_rects
        .iter()
        .find(|x| x.0 == 0 && x.1 == 0)
        .map(|x| x.3)
        .expect("slot of track 01, bar 1");
    let to = at(&h, 1, 3 * BAR);
    // hold it over the target: the preview card says what will happen
    let ev = |h: &mut Harness<'static, App>, e: egui::Event| {
        h.input_mut().events.push(e);
        frame(h);
    };
    let from = whole.center() + egui::vec2(10.0, 0.0);
    ev(&mut h, egui::Event::PointerMoved(from));
    ev(
        &mut h,
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    );
    for k in 1..=6 {
        let p = from + (to - from) * (k as f32 / 6.0);
        ev(&mut h, egui::Event::PointerMoved(p));
    }
    let plan = h
        .state()
        .drop_plan(to, super::dropplan::DropSrc::Clip(a), false)
        .unwrap();
    assert_eq!(
        (plan.track, plan.start, plan.new_track),
        (1, 3 * BAR, false)
    );
    assert!(plan.overlaps.is_empty() && plan.bad.is_none());
    assert_eq!(plan.snap, "Bar");
    ev(
        &mut h,
        egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    );
    frames(&mut h, 2);
    assert!(
        h.state().drop_ask.is_none(),
        "nothing in the way: no question"
    );
    let c = newest(&h);
    assert_ne!(c.id, a);
    assert_eq!((c.track, c.start, c.len), (1, 3 * BAR, 2 * BAR));
    assert_eq!(c.notes(), clip(&h, a).notes(), "linked: the same notes");
    assert!(
        h.state().status.starts_with("Placed on track 02 at bar 4"),
        "{}",
        h.state().status
    );
    assert_eq!(h.state().ch, 1, "the target track is current");
}

#[test]
fn alt_drops_an_independent_copy_and_the_master_row_makes_a_track() {
    use super::dropplan::DropSrc;
    let (mut h, a) = notes_song();
    // Alt: its own notes (equal, not shared)
    let plan = h
        .state()
        .drop_plan(at(&h, 1, BAR), DropSrc::Clip(a), false)
        .unwrap();
    h.state_mut()
        .drop_release(DropSrc::Clip(a), Some(plan), true);
    frames(&mut h, 2);
    let c = newest(&h);
    assert_eq!((c.track, c.start), (1, BAR));
    assert_ne!(c.notes(), clip(&h, a).notes(), "an independent copy");
    let orig = clip(&h, a).notes().unwrap();
    let same = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        t.notes[c.notes().unwrap()].notes == t.notes[orig].notes
    });
    assert!(same, "with the same notes in it");
    assert!(
        h.state().status.contains("independent"),
        "{}",
        h.state().status
    );
    // the Song / Master row: a new track, with the source track's instrument
    let nch = h.state().with(|s| s.song.channels);
    let (_, row) = h.state().tlv.track_rects(nch).expect("the Master row");
    let p = Pos2::new(h.state().tlv.x_of(BAR) + 6.0, row.center().y);
    let plan = h.state().drop_plan(p, DropSrc::Clip(a), false).unwrap();
    assert!(plan.new_track && plan.bad.is_none(), "{plan:?}");
    assert_eq!(plan.track, nch);
    let before = n_clips(&h);
    let slot0 = h.state().track_slot(0);
    h.state_mut()
        .drop_release(DropSrc::Clip(a), Some(plan), false);
    frames(&mut h, 2);
    assert_eq!(h.state().with(|s| s.song.channels), nch + 1);
    let c = newest(&h);
    assert_eq!(c.track as usize, nch, "on the new track");
    assert_eq!(
        h.state().track_slot(nch),
        slot0,
        "the source track's instrument"
    );
    assert_eq!(n_clips(&h), before + 1);
    // one Ctrl+Z: the clip and the track go together
    super::ui_tests::key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(h.state().with(|s| s.song.channels), nch);
    assert_eq!(n_clips(&h), before);
}

#[test]
fn a_drop_over_a_clip_asks_and_each_answer_does_what_it_says() {
    use super::dropplan::DropSrc;
    let (mut h, a) = notes_song();
    let ask = |h: &mut Harness<'static, App>| {
        let plan = h
            .state()
            .drop_plan(at(h, 0, BAR), DropSrc::Clip(a), false)
            .unwrap();
        assert_eq!(plan.overlaps, vec![a]);
        assert!(plan.layer_ok, "note clips can play together");
        h.state_mut()
            .drop_release(DropSrc::Clip(a), Some(plan), false);
        frames(h, 2);
        assert!(h.state().drop_ask.is_some());
    };
    let before = n_clips(&h);
    // Cancel (and Esc): nothing changes
    ask(&mut h);
    assert!(super::ui_tests::exact(&h, "Place the clip over others?").width() > 0.0);
    h.get_by_label("Cancel").click();
    frames(&mut h, 2);
    assert!(h.state().drop_ask.is_none());
    assert_eq!(n_clips(&h), before);
    assert_eq!(h.state().status, "Not placed (cancelled)");
    ask(&mut h);
    super::ui_tests::key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().drop_ask.is_none());
    assert_eq!(n_clips(&h), before);
    // Place after it: the first free bar after the clip (bar 3)
    ask(&mut h);
    h.get_by_label("Place after it (bar 3)").click();
    frames(&mut h, 2);
    let c = newest(&h);
    assert_eq!((c.track, c.start), (0, 2 * BAR));
    assert_eq!(clip(&h, a).len, 2 * BAR, "the clip in the way is untouched");
    super::ui_tests::key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(n_clips(&h), before, "one undo step");
    // Keep both: layered over the original
    ask(&mut h);
    h.get_by_label("Keep both").click();
    frames(&mut h, 2);
    let c = newest(&h);
    assert_eq!((c.track, c.start), (0, BAR));
    assert_eq!(clip(&h, a).len, 2 * BAR);
    super::ui_tests::key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(n_clips(&h), before);
    // Replace: the original keeps only its first bar
    ask(&mut h);
    h.get_by_label("Replace").click();
    frames(&mut h, 2);
    let c = newest(&h);
    assert_eq!((c.track, c.start, c.len), (0, BAR, 2 * BAR));
    assert_eq!(clip(&h, a).len, BAR, "split at the new clip's start");
    super::ui_tests::key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!((n_clips(&h), clip(&h, a).len), (before, 2 * BAR));
}

#[test]
fn moving_a_clip_over_another_shows_red_and_ctrl_d_says_what_it_covers() {
    let (mut h, a) = notes_song();
    // a second take at bar 3 (right after the first)
    let b = h
        .state()
        .with(|s| crate::timeline::new_note_clip(&mut s.song, 0, 2 * BAR).unwrap());
    frames(&mut h, 3);
    let r = h.state().tlv.clip_rect(a).expect("clip drawn");
    let from = r.center();
    let to = from + egui::vec2(h.state().tlv.x_of(BAR) - h.state().tlv.x_of(0), 0.0);
    let ev = |h: &mut Harness<'static, App>, e: egui::Event| {
        h.input_mut().events.push(e);
        frame(h);
    };
    ev(&mut h, egui::Event::PointerMoved(from));
    ev(
        &mut h,
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    );
    for k in 1..=6 {
        ev(
            &mut h,
            egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)),
        );
    }
    assert_eq!(clip(&h, a).start, BAR, "it moves live, one bar on");
    assert_eq!(h.state().tlv.move_over, vec![b], "the clip it now covers");
    assert!(
        h.state().status.contains("now over"),
        "{}",
        h.state().status
    );
    assert!(h.state().drop_ask.is_none(), "moves never ask");
    ev(
        &mut h,
        egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    );
    frames(&mut h, 2);
    assert!(
        h.state().tlv.move_over.is_empty(),
        "the red goes with the drag"
    );
    assert_eq!(clip(&h, a).start, BAR);
    assert_eq!(clip(&h, b).start, 2 * BAR, "the other clip is untouched");
    // Ctrl+D right after it lands on the clip at bar 3 too: the status says so
    super::ui_tests::key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(clip(&h, a).start, 0);
    h.state_mut().tlv.sels = vec![a];
    h.state_mut().tlv.sel = Some(a);
    h.state_mut().tl_apply(super::timeline::TlOp::Duplicate);
    frames(&mut h, 2);
    assert!(
        h.state()
            .status
            .contains("now over a clip at bar 3 on track 01: both play"),
        "{}",
        h.state().status
    );
}

#[test]
fn a_browser_drum_pattern_drop_previews_snaps_and_asks_over_a_clip() {
    use super::dropplan::DropSrc;
    let (mut h, a) = notes_song();
    h.state_mut().tlv.snap = super::timeline::Snap::Beat;
    frames(&mut h, 2);
    let it = crate::browser::builtin_items()
        .into_iter()
        .find(|i| matches!(i.src, crate::browser::Src::Pattern(_)))
        .expect("a drum pattern in the Browser");
    let crate::browser::Src::Pattern(pi) = it.src else {
        unreachable!()
    };
    let p = crate::drumpat::PATTERNS[pi];
    // over track 01 one bar and one beat in: snapped to the beat, over the take
    let to = at(&h, 0, BAR + crate::timeline::PPQ);
    let plan = h
        .state()
        .drop_plan(to, DropSrc::Pattern(pi as u16), false)
        .unwrap();
    assert_eq!(
        (plan.track, plan.start, plan.len),
        (0, BAR + crate::timeline::PPQ, p.len())
    );
    assert_eq!(plan.overlaps, vec![a]);
    assert_eq!(plan.snap, "Beat");
    // the real drag (picked up in the Browser with the button held), held
    // over the track, then let go
    let ev = |h: &mut Harness<'static, App>, e: egui::Event| {
        h.input_mut().events.push(e);
        frame(h);
    };
    let press = |pressed| egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    ev(&mut h, egui::Event::PointerMoved(to));
    ev(&mut h, press(true));
    h.state_mut().br.drag = Some(it.clone());
    ev(&mut h, egui::Event::PointerMoved(to + egui::vec2(1.0, 0.0)));
    assert!(h.state().br.drag.is_some(), "still dragging");
    ev(&mut h, egui::Event::PointerMoved(to));
    ev(&mut h, press(false));
    frames(&mut h, 2);
    assert!(h.state().br.drag.is_none());
    let ask = h.state().drop_ask.clone().unwrap_or_else(|| {
        panic!(
            "it asks over the take; status {:?} central {:?} to {to:?}",
            h.state().status,
            h.state().br.central
        )
    });
    assert_eq!(ask.src, DropSrc::Pattern(pi as u16));
    let before = n_clips(&h);
    h.get_by_label("Keep both").click();
    frames(&mut h, 2);
    let c = newest(&h);
    assert_ne!(c.id, a);
    assert_eq!(
        (c.track, c.start, c.len),
        (0, BAR + crate::timeline::PPQ, p.len()),
        "exactly where the preview said"
    );
    assert_eq!(clip(&h, a).len, 2 * BAR, "the take stays");
    assert_eq!(n_clips(&h), before + 1);
    let name = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        t.notes[c.notes().unwrap()].name.clone()
    });
    assert_eq!(name, p.name);
    assert!(
        h.state().status.contains("play together"),
        "{}",
        h.state().status
    );
    super::ui_tests::key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(n_clips(&h), before, "one undo step");
    // below the tracks: a new drum track, no question
    let nch = h.state().with(|s| s.song.channels);
    let below = Pos2::new(
        h.state().tlv.x_of(BAR) + 6.0,
        h.state().tlv.tracks_bottom() + 30.0,
    );
    let plan = h
        .state()
        .drop_plan(below, DropSrc::Pattern(pi as u16), false)
        .unwrap();
    assert!(plan.new_track, "{plan:?}");
    h.state_mut()
        .drop_release(DropSrc::Pattern(pi as u16), Some(plan), false);
    frames(&mut h, 2);
    assert_eq!(h.state().with(|s| s.song.channels), nch + 1);
    assert_eq!(newest(&h).track as usize, nch);
    assert!(h.state().drop_ask.is_none());
}
