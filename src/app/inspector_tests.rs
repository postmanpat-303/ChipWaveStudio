//! The left Inspector and the bottom view tab bar (docs/INSPECTOR.md)
use super::ui_tests::{click_at, frame, harness_with, key};
use super::*;
use egui::Key;
use egui_kittest::{kittest::Queryable, Harness};

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn song<R>(h: &mut Harness<'static, App>, f: impl FnOnce(&Song) -> R) -> R {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    f(&g.song)
}

fn tl_harness() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    let (tl, _) = crate::timeline::from_order(&s);
    s.timeline = Some(Box::new(tl));
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 4);
    h
}

#[test]
fn i_toggles_the_inspector_but_types_a_note_in_the_pattern_editor() {
    let mut h = tl_harness();
    assert!(!h.state().extra.inspector_open);
    key(&mut h, Key::I, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().extra.inspector_open, "I opens it in the Timeline");
    let r = h.state().insp.rect.expect("drawn");
    assert!(r.left() < 5.0 && r.width() > 200.0, "{r:?}");
    key(&mut h, Key::I, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(!h.state().extra.inspector_open);
    // the pattern editor: I is a note key, Alt+I is the Inspector
    h.state_mut().view = View::Pattern;
    h.state_mut().edit = true;
    frames(&mut h, 2);
    key(&mut h, Key::I, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(!h.state().extra.inspector_open, "I typed a note");
    key(&mut h, Key::I, egui::Modifiers::ALT);
    frames(&mut h, 2);
    assert!(h.state().extra.inspector_open, "Alt+I");
}

#[test]
fn track_name_commits_on_click_away_and_esc_cancels_and_undo_restores() {
    let mut h = tl_harness();
    h.state_mut().extra.inspector_open = true;
    h.state_mut().ch = 1;
    frames(&mut h, 3);
    let name0 = song(&mut h, |s| s.channel_names[1].clone());
    let field = h.state().insp.name_rect.expect("name field");
    // Esc cancels
    click_at(&mut h, field.center());
    h.input_mut().events.push(egui::Event::Text("XYZ".into()));
    frames(&mut h, 2);
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 3);
    assert_eq!(
        song(&mut h, |s| s.channel_names[1].clone()),
        name0,
        "Esc keeps the old name"
    );
    // click-away commits
    click_at(&mut h, field.center());
    key(&mut h, Key::End, egui::Modifiers::NONE);
    h.input_mut().events.push(egui::Event::Text(" 2".into()));
    frames(&mut h, 2);
    click_at(&mut h, Pos2::new(900.0, 980.0));
    frames(&mut h, 3);
    let new = song(&mut h, |s| s.channel_names[1].clone());
    assert_eq!(new, format!("{name0} 2"));
    // and it is one undo step
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 3);
    assert_eq!(song(&mut h, |s| s.channel_names[1].clone()), name0);
}

#[test]
fn clip_section_mutes_the_selected_clip_and_undo_brings_it_back() {
    let mut h = tl_harness();
    h.state_mut().extra.inspector_open = true;
    let id = song(&mut h, |s| s.timeline.as_ref().unwrap().clips[0].id);
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    frames(&mut h, 3);
    assert_eq!(h.state().insp.focus, super::inspector::Focus::Clip);
    h.get_by_label("Muted").click();
    frames(&mut h, 3);
    let muted = |h: &mut Harness<'static, App>| {
        song(h, |s| {
            s.timeline
                .as_ref()
                .unwrap()
                .clips
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .muted
        })
    };
    assert!(muted(&mut h));
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 3);
    assert!(!muted(&mut h));
    // clicking a track header shows the track again
    h.state_mut().ch = 3;
    frames(&mut h, 2);
    assert_eq!(h.state().insp.focus, super::inspector::Focus::Track);
}

#[test]
fn view_tab_bar_switches_views_and_the_old_ways_still_work() {
    let mut h = tl_harness();
    assert!(h.state().extra.view_bar);
    h.get_by_label("MIX").click();
    frames(&mut h, 2);
    assert!(h.state().view == View::Mixer);
    h.get_by_label("TRACKER").click();
    frames(&mut h, 2);
    assert!(h.state().view == View::Pattern);
    h.get_by_label("INST").click();
    frames(&mut h, 2);
    assert!(h.state().extra.lower_inst && h.state().lower_visible());
    assert_eq!(h.state().vtab_current(), Some(super::viewbar::VTab::Inst));
    h.get_by_label("ORDER").click();
    frames(&mut h, 2);
    assert!(h.state().view == View::Arranger);
    // the shortcuts and the toolbar keep working
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(h.state().view == View::Timeline);
    assert_eq!(
        h.state()
            .vtab_current()
            .map(|t| t != super::viewbar::VTab::Arranger),
        Some(true)
    );
    key(&mut h, Key::Num3, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(h.state().vtab_current(), Some(super::viewbar::VTab::Mix));
}
