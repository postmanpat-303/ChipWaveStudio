//! Stage 5 (docs/unified/PLAN.md): the pies on a Timeline clip, a launcher
//! clip and the Drum Kit step grid; clip colour and rename.
use super::objpie::PieFor;
use super::ui_tests::{click_at, exact, frame};
use super::unified_tests::{at, clip, frames, notes_song, BAR};
use super::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

pub(super) fn press(h: &mut Harness<'static, App>, p: Pos2, b: egui::PointerButton, down: bool) {
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: p,
        button: b,
        pressed: down,
        modifiers: egui::Modifiers::NONE,
    });
    frame(h);
}

pub(super) fn right_click(h: &mut Harness<'static, App>, p: Pos2) {
    press(h, p, egui::PointerButton::Secondary, true);
    press(h, p, egui::PointerButton::Secondary, false);
    frames(h, 2);
}

/// choose `label` (and the outer ring's `child`) in the open pie
pub(super) fn pick(h: &mut Harness<'static, App>, label: &str, child: Option<&str>) {
    let (pie, _) = h.state().opie.clone().expect("a pie is open");
    let i = pie
        .items
        .iter()
        .position(|x| x.label == label)
        .unwrap_or_else(|| panic!("no wedge {label}"));
    let w = pie.wedge_centre(i, (super::radial::R_IN + super::radial::R_MID) / 2.0);
    h.input_mut().events.push(egui::Event::PointerMoved(w));
    frames(h, 2);
    let p = match child {
        Some(c) => {
            let k = pie.items[i]
                .children
                .iter()
                .position(|x| x == c)
                .unwrap_or_else(|| panic!("no {c} in {label}"));
            let p = pie.child_centre(i, k);
            h.input_mut().events.push(egui::Event::PointerMoved(p));
            frames(h, 2);
            p
        }
        None => w,
    };
    click_at(h, p);
    frames(h, 3);
}

fn menu_open(h: &Harness<'static, App>) -> usize {
    h.query_all_by(|n| n.label().is_some_and(|l| l.contains("Make unique")))
        .count()
}

fn n_clips(h: &Harness<'static, App>) -> usize {
    h.state()
        .with(|s| s.song.timeline.as_deref().unwrap().clips.len())
}

#[test]
fn right_click_a_clip_opens_its_pie_colour_rename_duplicate() {
    let (mut h, a) = notes_song();
    let p = at(&h, 0, BAR / 2);
    right_click(&mut h, p);
    assert_eq!(h.state().opie.as_ref().map(|x| x.1), Some(PieFor::Clip(a)));
    assert!(menu_open(&h) == 0, "the pie, not the list menu");
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
    for want in [
        "Edit",
        "Duplicate",
        "Colour",
        "Rename",
        "To launcher",
        "Delete",
    ] {
        assert!(labels.iter().any(|x| x == want), "{want} in {labels:?}");
    }
    // Colour > Blue
    pick(&mut h, "Colour", Some("Blue"));
    assert!(h.state().opie.is_none(), "picking closes it");
    assert_eq!(clip(&h, a).colour, Some(0x5A86B4));
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(clip(&h, a).colour, None, "one undo step");
    // Rename: the window, typed, Enter
    let p = at(&h, 0, BAR / 2);
    right_click(&mut h, p);
    pick(&mut h, "Rename", None);
    assert!(h.state().clip_rename.is_some());
    h.state_mut().clip_rename.as_mut().unwrap().text = "Verse riff".into();
    frames(&mut h, 1);
    let r = exact(&h, "Rename");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert!(h.state().clip_rename.is_none());
    assert_eq!(clip(&h, a).name.as_deref(), Some("Verse riff"));
    // Duplicate > Independent copy: after it, its own notes, one undo
    let n0 = n_clips(&h);
    let p = at(&h, 0, BAR / 2);
    right_click(&mut h, p);
    pick(&mut h, "Duplicate", Some("Independent copy"));
    assert_eq!(n_clips(&h), n0 + 1, "{}", h.state().status);
    let b = h.state().tlv.sel.unwrap();
    assert_ne!(b, a);
    let cb = clip(&h, b);
    assert_eq!(cb.start, clip(&h, a).end(), "right after it");
    assert_ne!(cb.notes(), clip(&h, a).notes(), "independent notes");
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(n_clips(&h), n0, "one undo step");
    // Esc closes a pie without doing anything
    let p = at(&h, 0, BAR / 2);
    right_click(&mut h, p);
    assert!(h.state().opie.is_some());
    h.key_press(egui::Key::Escape);
    frames(&mut h, 2);
    assert!(h.state().opie.is_none());
    assert_eq!(n_clips(&h), n0);
}

#[test]
fn settings_list_keeps_the_list_menu() {
    let (mut h, _) = notes_song();
    h.state_mut().extra.clip_list_menu = true;
    let p = at(&h, 0, BAR / 2);
    right_click(&mut h, p);
    assert!(h.state().opie.is_none());
    assert!(menu_open(&h) > 0, "the list menu");
}

#[test]
fn a_launcher_clip_pie_places_it_after_the_last_clip() {
    let (mut h, a) = notes_song();
    let r = exact(&h, "Capture");
    click_at(&mut h, r.center());
    frames(&mut h, 3);
    let lid = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().launcher[0].clip.id);
    let sr = h
        .state()
        .ca
        .slot_rects
        .iter()
        .find(|x| x.0 == 0 && x.1 == 0)
        .map(|x| x.3)
        .unwrap();
    right_click(&mut h, sr.center() + egui::vec2(10.0, 0.0));
    assert_eq!(
        h.state().opie.as_ref().map(|x| x.1),
        Some(PieFor::Launch(lid))
    );
    // Rename and Colour reach the launcher clip too
    pick(&mut h, "Colour", Some("Green"));
    let l = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().launcher[0].clip.clone());
    assert_eq!(l.colour, Some(0x6E9A58));
    let n0 = n_clips(&h);
    right_click(&mut h, sr.center() + egui::vec2(10.0, 0.0));
    pick(&mut h, "Place in arrangement", Some("After the last clip"));
    assert_eq!(n_clips(&h), n0 + 1, "{}", h.state().status);
    let c = super::unified_tests::newest(&h);
    assert_eq!(
        c.start,
        clip(&h, a).end(),
        "after the arrangement's clip, on the bar"
    );
    assert_eq!(c.notes(), l.notes(), "a linked copy (D3)");
}
