//! Stage 4b (docs/unified/PLAN.md): the Clip Arranger's own launcher clips.
//! Capture turns the arrangement's scenes into launcher clips linked to it;
//! ▶ launches one per track on the boundary; an empty slot's double-click
//! makes a new clip and opens it in the piano roll; a launcher clip drags
//! onto the timeline as a linked copy.
use super::ui_tests::{click_at, double_click, frame};
use super::unified_tests::{at, frames, notes_song, BAR};
use super::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn launcher(h: &Harness<'static, App>) -> Vec<crate::timeline::LaunchClip> {
    h.state()
        .with(|s| s.song.timeline.as_deref().unwrap().launcher.clone())
}

fn slot(h: &Harness<'static, App>, tr: usize, b: usize) -> (Rect, Rect) {
    h.state()
        .ca
        .slot_rects
        .iter()
        .find(|x| x.0 == tr && x.1 == b)
        .map(|x| (x.2, x.3))
        .unwrap_or_else(|| panic!("slot {tr} {b} not drawn"))
}

fn render(h: &mut Harness<'static, App>, blocks: usize) -> f32 {
    h.state().with(|s| {
        let Shared { song, engine, .. } = s;
        let mut buf = vec![0.0f32; 256];
        let mut pk = 0f32;
        for _ in 0..blocks {
            engine.render(song, &mut buf);
            pk = buf.iter().fold(pk, |m, x| m.max(x.abs()));
        }
        pk
    })
}

/// Capture, then the launcher's own clips play and edit
#[test]
fn capture_makes_linked_launcher_clips_that_launch_and_edit() {
    let (mut h, a) = notes_song();
    // an instrument for the notes (a new song has none)
    h.state().with(|s| {
        s.song
            .instruments
            .push(fileio::demo_song().instruments[0].clone());
        let t = s.song.timeline.as_deref_mut().unwrap();
        for d in &mut t.notes {
            for n in &mut d.make_mut().notes {
                n.instr = 1;
            }
        }
    });
    frames(&mut h, 2);
    assert!(!h.state().ca.launch);
    let r = super::ui_tests::exact(&h, "Capture");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    assert!(
        h.state().ca.launch,
        "the grid shows the launcher now: {}",
        h.state().status
    );
    let l = launcher(&h);
    assert_eq!(l.len(), 1, "{l:?}");
    let lid = l[0].clip.id;
    assert_ne!(lid, a, "a clip of its own");
    assert_eq!(
        l[0].clip.notes(),
        super::unified_tests::clip(&h, a).notes(),
        "linked to the arrangement's notes"
    );
    assert!(
        h.state().status.starts_with("Captured 1 clip"),
        "{}",
        h.state().status
    );
    assert_eq!(h.state().ca.secs.as_ref().unwrap()[0].clips[0], Some(lid));
    // the old Capture button is gone; + Scene is there
    assert!(h.query_by_label("Capture").is_none());
    // ▶ from a stop: only that track plays, its clip looping
    h.state_mut().extra.launch_q = 3; // next row
    frames(&mut h, 2);
    let (strip, _) = slot(&h, 0, 0);
    click_at(&mut h, strip.center());
    frames(&mut h, 1);
    render(&mut h, 4);
    assert_eq!(h.state().with(|s| s.engine.launched(0)), Some(lid));
    let peak = render(&mut h, 400);
    assert!(peak > 0.001, "the launched clip sounds ({peak})");
    frames(&mut h, 2);
    assert!(
        h.state().ca.line.contains("1 launcher clip playing"),
        "{}",
        h.state().ca.line
    );
    // its ▶ is a ■ now: a click stops it on the boundary, again plays it
    let (strip, _) = slot(&h, 0, 0);
    click_at(&mut h, strip.center());
    frames(&mut h, 1);
    render(&mut h, 400);
    assert_eq!(
        h.state().with(|s| s.engine.launched(0)),
        None,
        "■ stopped it"
    );
    frames(&mut h, 2);
    click_at(&mut h, strip.center());
    frames(&mut h, 1);
    render(&mut h, 40);
    assert_eq!(
        h.state().with(|s| s.engine.launched(0)),
        Some(lid),
        "▶ again"
    );
    frames(&mut h, 2);
    // Stop all
    h.get_by_label("Stop all").click();
    frames(&mut h, 2);
    render(&mut h, 2000);
    assert_eq!(h.state().with(|s| s.engine.launched(0)), None);
    // + Scene, then double-click its empty slot on track 02: a new clip
    h.get_by_label("+ Scene").click();
    frames(&mut h, 3);
    assert_eq!(h.state().ca.secs.as_ref().unwrap().len(), 2);
    let (_, whole) = slot(&h, 1, 1);
    double_click(&mut h, whole.center());
    frames(&mut h, 3);
    let l = launcher(&h);
    assert_eq!(l.len(), 2, "{l:?}");
    let new = l.iter().find(|c| c.scene == 1).unwrap();
    assert_eq!((new.clip.track, new.clip.len), (1, BAR));
    assert_eq!(h.state().pr.launch, Some(new.clip.id));
    let data = new.clip.notes().unwrap();
    assert_eq!(
        h.state().pr_target(),
        Some(super::pianoview::Target::Notes { data, clip: None }),
        "the piano roll edits it"
    );
    // Delete it from its right-click menu; Ctrl+Z brings it back
    h.state_mut().ca_delete(new.clip.id);
    frames(&mut h, 2);
    assert_eq!(launcher(&h).len(), 1);
    assert_eq!(h.state().pr.launch, None);
}

/// A launcher clip drags onto the timeline as a linked copy
#[test]
fn a_launcher_clip_drags_onto_the_timeline_as_a_linked_copy() {
    let (mut h, a) = notes_song();
    let r = super::ui_tests::exact(&h, "Capture");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let lid = launcher(&h)[0].clip.id;
    let (_, whole) = slot(&h, 0, 0);
    let to = at(&h, 1, 3 * BAR);
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
        .drop_plan(to, super::dropplan::DropSrc::Clip(lid), false)
        .expect("a launcher clip is a drop source");
    assert_eq!((plan.track, plan.start), (1, 3 * BAR));
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
    let c = super::unified_tests::newest(&h);
    assert!(c.id != a && c.id != lid);
    assert_eq!((c.track, c.start), (1, 3 * BAR));
    assert_eq!(c.notes(), launcher(&h)[0].clip.notes(), "linked");
    // ×3 now: the arrangement's, the launcher's and the copy
    let d = c.notes().unwrap();
    let n = h.state().with(|s| {
        let (x, y) = crate::timeline::linked(s.song.timeline.as_deref().unwrap(), d);
        x + y
    });
    assert_eq!(n, 3);
}

/// The piano roll names linked clips and makes one unique
#[test]
fn the_piano_roll_shows_linked_clips_and_makes_a_launcher_clip_unique() {
    let (mut h, a) = notes_song();
    let r = super::ui_tests::exact(&h, "Capture");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let lid = launcher(&h)[0].clip.id;
    let (_, whole) = slot(&h, 0, 0);
    double_click(&mut h, whole.center());
    frames(&mut h, 4);
    assert_eq!(h.state().pr.launch, Some(lid));
    let r = super::ui_tests::exact(&h, "Linked ×2");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let mine = launcher(&h)[0].clip.notes();
    let arr = super::unified_tests::clip(&h, a).notes();
    assert_ne!(mine, arr, "its own notes now");
    assert!(h.query_by_label("Linked ×2").is_none());
    assert!(h.state().status.starts_with("Made unique"));
    // one undo step brings the link back
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(launcher(&h)[0].clip.notes(), arr);
}
