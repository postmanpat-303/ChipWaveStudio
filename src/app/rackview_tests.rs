//! The Rack row (rack layout A, Del 23 Oct 2026): the side tabs, the
//! Delete / Ctrl+D / 0 keys on the last-clicked card, drops and reorder,
//! Browser › Instruments without presets, and the curated instrument faces
use super::rackview::{Pick, TAB_W};
use super::ui_tests::{click_at, drag, frame, key};
use super::*;
use crate::fx::FxKind;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn chain(h: &Harness<'static, App>) -> Vec<FxKind> {
    h.state()
        .with(|s| s.song.instruments[6].fx.iter().map(|d| d.kind).collect())
}

fn clips(h: &Harness<'static, App>) -> usize {
    h.state()
        .with(|s| s.song.timeline.as_ref().map_or(0, |t| t.clips.len()))
}

/// the demo on the Timeline (timeline mode), the Rack showing instrument 7
/// (Supersaw Pad: filter + chorus)
fn rack() -> Harness<'static, App> {
    let mut h = super::unified_tests::tl_demo();
    h.state_mut().instr = 7;
    h.state_mut().dev.follow_ch = h.state().ch;
    h.state_mut().set_lower(4);
    frames(&mut h, 3);
    assert!(h.state().devices_visible() && !h.state().dev.tree);
    h
}

/// an empty spot of a card's side tab (below its buttons): selects the card
fn tab_blank(c: egui::Rect) -> Pos2 {
    Pos2::new(c.left() + TAB_W / 2.0, c.bottom() - 12.0)
}

#[test]
fn the_side_tabs_bypass_remove_and_collapse() {
    let mut h = rack();
    let before = chain(&h);
    assert!(before.len() >= 2, "{before:?}");
    let c0 = h.state().dev.cards[0];
    // power, then collapse (the third button), on effect 1
    click_at(&mut h, Pos2::new(c0.left() + TAB_W / 2.0, c0.top() + 14.0));
    frames(&mut h, 2);
    assert!(!h.state().with(|s| s.song.instruments[6].fx[0].enabled));
    click_at(&mut h, Pos2::new(c0.left() + TAB_W / 2.0, c0.top() + 66.0));
    frames(&mut h, 2);
    assert!(h.state().with(|s| s.song.instruments[6].fx[0].collapsed));
    assert!(
        h.state().dev.cards[0].width() < 40.0 + TAB_W,
        "a slim strip"
    );
    // × removes effect 2; one undo puts it back
    let c1 = h.state().dev.cards[1];
    click_at(&mut h, Pos2::new(c1.left() + TAB_W / 2.0, c1.top() + 40.0));
    frames(&mut h, 2);
    assert_eq!(chain(&h), before[..1].to_vec());
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert_eq!(chain(&h), before);
    // the instrument's tab has only the collapse button (it stays)
    let i = h.state().dev.inst;
    click_at(&mut h, Pos2::new(i.left() + TAB_W / 2.0, i.top() + 14.0));
    frames(&mut h, 2);
    assert!(h.state().dev.inst_folded);
    assert_eq!(chain(&h), before);
    assert_eq!(h.state().with(|s| s.song.instruments.len()), {
        let s = fileio::demo_song();
        s.instruments.len()
    });
}

#[test]
fn delete_acts_on_the_last_clicked_panel() {
    let mut h = rack();
    let before = chain(&h);
    let n_clips = clips(&h);
    assert!(n_clips > 0);
    // a clip is selected on the Timeline, then a Rack card is clicked
    let id = h.state().tlv.cache.as_ref().unwrap().tl.clips[0].id;
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    let c0 = h.state().dev.cards[0];
    click_at(&mut h, tab_blank(c0));
    frames(&mut h, 2);
    assert!(h.state().dev.focus && h.state().dev.pick == Pick::Fx(0));
    key(&mut h, Key::Delete, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert_eq!(chain(&h), before[1..].to_vec(), "the card goes");
    assert_eq!(clips(&h), n_clips, "the clip stays");
    assert_eq!(h.state().dev.pick, Pick::Fx(0), "its neighbour is picked");
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(chain(&h), before, "Ctrl+Z puts it back");
    // Ctrl+D duplicates, 0 bypasses the picked card
    let p = tab_blank(h.state().dev.cards[1]);
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(h.state().dev.pick, Pick::Fx(1));
    key(&mut h, Key::D, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    let mut want = before.clone();
    want.insert(2, before[1]);
    assert_eq!(chain(&h), want);
    assert_eq!(h.state().dev.pick, Pick::Fx(2));
    key(&mut h, Key::Num0, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(!h.state().with(|s| s.song.instruments[6].fx[2].enabled));
    h.state_mut().do_undo(false);
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert_eq!(chain(&h), before);
    // Delete on the instrument card only says how to swap it
    let p = tab_blank(h.state().dev.inst);
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(h.state().dev.pick, Pick::Inst);
    let n_inst = h.state().with(|s| s.song.instruments.len());
    key(&mut h, Key::Delete, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert_eq!(chain(&h), before);
    assert_eq!(h.state().with(|s| s.song.instruments.len()), n_inst);
    assert!(
        h.state().status.contains("Replace instrument"),
        "{}",
        h.state().status
    );
    // a click on the Timeline gives it the keys back: Delete is a clip edit
    let lanes = h.state().tlv.lanes;
    let y = (h.state().tlv.tracks_bottom() + 10.0).min(lanes.bottom() - 4.0);
    click_at(&mut h, Pos2::new(lanes.center().x, y));
    frames(&mut h, 2);
    assert!(!h.state().dev.focus);
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    key(&mut h, Key::Delete, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert_eq!(chain(&h), before, "the Rack is left alone");
    assert_eq!(clips(&h), n_clips - 1, "the clip goes");
}

#[test]
fn browser_drops_land_on_the_rack_and_titles_reorder() {
    let mut h = rack();
    let before = chain(&h);
    let delay = crate::browser::builtin_items()
        .into_iter()
        .find(|i| i.key == "fx:Delay")
        .unwrap();
    // over the + after the last card: the end of the chain
    h.state_mut().br.drag = Some(delay.clone());
    let last = *h.state().dev.cards.last().unwrap();
    let p = Pos2::new(last.right() + 20.0, last.center().y);
    let at = h.state().br_drop_at(p);
    assert_eq!(at, Some(browserview::DropAt::Device(before.len())));
    h.state_mut().br.drag = None;
    let ctx = h.ctx.clone();
    h.state_mut().br_drop(&delay, at.unwrap(), &ctx);
    frames(&mut h, 2);
    let mut want = before.clone();
    want.push(FxKind::Delay);
    assert_eq!(chain(&h), want);
    // drag effect 1's title past effect 2: they swap
    let c0 = h.state().dev.cards[0];
    let c1 = h.state().dev.cards[1];
    drag(
        &mut h,
        Pos2::new(c0.left() + TAB_W + 50.0, c0.top() + 12.0),
        Pos2::new(c1.right() - 10.0, c1.center().y),
    );
    frames(&mut h, 2);
    want.swap(0, 1);
    assert_eq!(chain(&h), want);
}

#[test]
fn browser_instruments_lists_instruments_not_presets() {
    use crate::browser::Src;
    let tools: Vec<(&str, &str)> = super::trackpie::Builtin::ALL
        .iter()
        .map(|b| (b.label(), b.tip()))
        .collect();
    let mut items = crate::browser::tool_items(&tools);
    items.extend(crate::browser::builtin_items());
    let inst: Vec<usize> = (0..items.len())
        .filter(|&i| items[i].tab == crate::browser::Tab::Instruments)
        .collect();
    // presets, blanks and generator patches are filed under Instruments...
    assert!(inst.iter().any(|&i| !matches!(items[i].src, Src::Tool(_))));
    let mut kept = inst.clone();
    browserview::instruments_only(&items, &mut kept);
    assert!(!kept.is_empty());
    assert!(kept.iter().all(|&i| matches!(items[i].src, Src::Tool(_))));
    assert_eq!(
        kept.len(),
        inst.iter()
            .filter(|&&i| matches!(items[i].src, Src::Tool(_)))
            .count(),
        "every ChipWave instrument stays"
    );
}

#[test]
fn instrument_faces_are_curated_and_end_on_volume() {
    use super::instpanel::instcard::face_knobs;
    let song = fileio::demo_song();
    let mut seen = 0;
    for (idx, inst) in song.instruments.iter().enumerate() {
        if matches!(inst.kind, InstrumentKind::Rack(_)) || crate::drumkit::kit_of(inst).is_some() {
            continue;
        }
        let l = super::params::build(inst, idx, 0);
        let ks = face_knobs(&l, inst);
        if ks.is_empty() {
            continue;
        }
        seen += 1;
        let names: Vec<String> = ks.iter().map(|k| k.name(&l)).collect();
        assert!(names.len() <= 8, "{}: {names:?}", inst.name);
        assert_eq!(
            names.last().map(String::as_str),
            Some("Volume"),
            "{}",
            inst.name
        );
        assert!(
            names.iter().all(|n| !n.is_empty()),
            "{}: {names:?}",
            inst.name
        );
        let mut u = names.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), names.len(), "{}: no repeats {names:?}", inst.name);
    }
    assert!(seen >= 3, "faces checked: {seen}");
    // a Rack card draws the face knobs plus Volume, and Advanced ›
    let h = rack();
    assert!(h
        .query_all_by(|n| n.label().as_deref() == Some("Advanced ›"))
        .next()
        .is_some());
}
