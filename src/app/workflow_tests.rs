//! Workflow / stability regressions (the 20 Oct 2026 brief): the Generate
//! dialog crash, invalid chord states, the track pie's submenus, split
//! preview, chord replace, piano-key audition and track instruments.
use super::timeline_m2_tests::{click_at, with_mods};
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline as tl;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

pub(super) fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn tl_setup() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    h.state_mut().enter_timeline();
    h.state_mut().undo_reset();
    h.state_mut().tlv.zoom = 40.0;
    h.state_mut().tlv.scroll = 0.0;
    frames(&mut h, 2);
    h
}

/// the demo in timeline mode with an empty 2-bar note clip on track 2,
/// selected and open in the piano roll
fn clip_setup() -> Harness<'static, App> {
    let mut h = tl_setup();
    let id = h
        .state()
        .with(|s| {
            let id = tl::new_note_clip(&mut s.song, 2, 0)?;
            let t = s.song.timeline.as_deref_mut()?;
            let c = t.clips.iter_mut().find(|c| c.id == id)?;
            c.len = 8 * tl::PPQ;
            let tl::ClipKind::Notes { data } = c.kind else {
                return None;
            };
            t.notes[data as usize].make_mut().len = 8 * tl::PPQ;
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(id)
        })
        .expect("a note clip");
    {
        let a = h.state_mut();
        a.tlv.sel = Some(id);
        a.tlv.sels = vec![id];
        a.set_lower(3);
        a.undo_reset();
    }
    frames(&mut h, 2);
    h.state_mut().pr.zoom = 20.0;
    frame(&mut h);
    h
}

fn note_data(h: &Harness<'static, App>) -> tl::NoteClipData {
    let id = h.state().tlv.sel.expect("a selected clip");
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else {
            panic!("not a note clip")
        };
        (*t.notes[data as usize]).clone()
    })
}

fn right_click(h: &mut Harness<'static, App>, p: Pos2) {
    with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    for pressed in [true, false] {
        with_mods(
            h,
            egui::Modifiers::NONE,
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Default::default(),
            },
        );
    }
    frame(h);
}

fn lane_at(h: &Harness<'static, App>, tick: tl::Tick) -> Pos2 {
    let r = h.state().cl.rect;
    Pos2::new(h.state().pr.x_of(tick) + 6.0, r.center().y)
}

// ------------------------------------------------------------ step 1: chord crash

/// Generate dialog: clicking a style with fewer progressions than the
/// shown one indexed the new style's list with the old index (panic at
/// chordlane.rs `cl_gen_window`)
#[test]
fn generate_dialog_style_switch_never_panics() {
    use crate::chordgen::STYLES;
    let blues = STYLES.iter().position(|s| s.name == "Blues").unwrap();
    assert_eq!(
        STYLES[blues].progs.len(),
        3,
        "the short list the crash needed"
    );
    let mut h = clip_setup();
    h.state_mut().cl_open_gen_at(0);
    frame(&mut h);
    assert!(h.state().cl.gen.open);
    // every style x every progression -> Blues (the shortest) and back
    for (from, st) in STYLES.iter().enumerate() {
        for prog in 0..st.progs.len() {
            h.state_mut().cl.gen.opts.style = from;
            h.state_mut().cl.gen.opts.prog = Some(prog);
            frame(&mut h);
            h.get_by_label("Blues").click();
            frame(&mut h);
            assert_eq!(h.state().cl.gen.opts.style, blues);
            h.state_mut().cl.gen.opts.style = blues;
            h.state_mut().cl.gen.opts.prog = Some(2);
            frame(&mut h);
            h.get_by_label(st.name).click();
            frame(&mut h);
            assert_eq!(h.state().cl.gen.opts.style, from);
        }
    }
    // a random progression (as the pie's style wedge opens it)
    for seed in 0..12 {
        h.state_mut().cl.gen.opts.style = 0;
        h.state_mut().cl.gen.opts.prog = None;
        h.state_mut().cl.gen.opts.seed = seed;
        frame(&mut h);
        h.get_by_label("Blues").click();
        frames(&mut h, 2);
    }
}

/// the whole right-click path: lane pie > More… > style buttons
#[test]
fn right_click_more_then_style_buttons_never_panics() {
    let mut h = clip_setup();
    h.state()
        .with(|s| s.song.info.key = Some(crate::chords::Key::new(4, crate::chords::Scale::Minor)));
    frame(&mut h);
    let p = lane_at(&h, 2 * 4 * tl::PPQ);
    right_click(&mut h, p);
    let w = {
        let (pie, _) = h.state().cl.pie.as_ref().expect("the lane pie opened");
        pie.wedge_centre(pie.items.len() - 1, 75.0)
    };
    with_mods(&mut h, egui::Modifiers::NONE, egui::Event::PointerMoved(w));
    click_at(&mut h, w);
    assert!(h.state().cl.gen.open, "More… opens the Generate dialog");
    for s in ["Blues", "Cinematic", "Blues", "Jazz", "Blues"] {
        h.get_by_label(s).click();
        frames(&mut h, 2);
    }
    assert!(h.state().cl.gen.open);
}

/// stale / invalid chord states do nothing (or say why) instead of panicking
#[test]
fn chord_lane_survives_invalid_and_stale_states() {
    let mut h = clip_setup();
    let t = h.state().pr.target.expect("a roll target");
    h.state_mut().cl_detect(t);
    h.state_mut()
        .cl_insert(t, vec![], true, Default::default(), 4);
    h.state_mut().cl.sel = Some(99);
    h.state_mut().cl_delete_sel();
    frames(&mut h, 2);
    assert!(note_data(&h).chords.is_empty());
    // a chord, its pie, then the chord goes away before each pick
    h.state_mut().cl_add_tonic(t, 0, 4 * tl::PPQ);
    frame(&mut h);
    for item in 0..9usize {
        let ctx = h.ctx.clone();
        h.state_mut()
            .cl_open_menu(&ctx, Pos2::new(600.0, 500.0), chordlane::PieOn::Mark(0), t);
        let m = h.state().cl_marks(t);
        h.state_mut().cl_write(t, vec![], vec![], true);
        frame(&mut h);
        let w = h
            .state()
            .cl
            .pie
            .as_ref()
            .map(|p| p.0.wedge_centre(item, 75.0));
        if let Some(w) = w {
            click_at(&mut h, w);
        }
        h.state_mut().cl_write(t, m, vec![], true);
        frame(&mut h);
    }
    let ctx = h.ctx.clone();
    h.state_mut()
        .cl_open_menu(&ctx, Pos2::new(600.0, 500.0), chordlane::PieOn::Mark(42), t);
    h.state_mut().cl.edit = Some(42);
    h.state_mut().instr = 0;
    if let Some(m) = h.state().cl_marks(t).first().cloned() {
        h.state_mut().cl_play(&m);
    }
    frames(&mut h, 3);
    assert!(h.state().cl.edit.is_none(), "a stale edit window closes");
    // the roll's clip deleted under an open lane pie, then Generate › Apply
    let ctx = h.ctx.clone();
    h.state_mut()
        .cl_open_menu(&ctx, Pos2::new(600.0, 500.0), chordlane::PieOn::Lane(0), t);
    if let pianoview::Target::Notes { clip: Some(id), .. } = t {
        h.state().with(|s| {
            if let Some(tl) = s.song.timeline.as_deref_mut() {
                tl.clips.retain(|c| c.id != id);
            }
        });
    }
    frames(&mut h, 3);
    h.state_mut().cl_open_gen_at(0);
    frames(&mut h, 2);
    if let Some(n) = h.query_by_label("Apply") {
        n.click();
    }
    frames(&mut h, 2);
}

/// a chord root out of 0..12 (a hand-edited file) draws and names safely
#[test]
fn out_of_range_chord_roots_do_not_overflow() {
    let mut h = clip_setup();
    let t = h.state().pr.target.unwrap();
    let m = crate::chordgen::ChordMark {
        start: 0,
        len: 4 * tl::PPQ,
        root: 250,
        q: crate::chordgen::Quality::Dom13,
        bass: Some(240),
    };
    let key = crate::chords::Key::new(0, crate::chords::Scale::Major);
    let _ = (m.numeral(key), m.name(key), m.pcs());
    let _ = crate::chordgen::suggestions(key, &m);
    h.state_mut().cl_write(t, vec![m], vec![], true);
    frames(&mut h, 3);
}

// ------------------------------------------------------------ step 1: track pie submenus

use super::trackpie::{hit, seg_angle, sub_items, sub_layout, sub_rect, PieAct, Seg, SEGS};

fn pie_harness(at: Pos2) -> Harness<'static, App> {
    let mut h = harness_with({
        let mut s = fileio::demo_song();
        crate::timeline::enter_timeline_mode(&mut s);
        s
    });
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    let ctx = h.ctx.clone();
    h.state_mut().open_track_pie(0, at, false, &ctx);
    frames(&mut h, 20);
    h
}

fn mv(h: &mut Harness<'static, App>, p: Pos2) {
    with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
}

fn open_sub(h: &Harness<'static, App>) -> Option<usize> {
    h.state().pie.as_ref().and_then(|p| p.sub.map(|s| s.0))
}

/// point at segment `seg`, return (start point, its panel)
fn onto(h: &mut Harness<'static, App>, seg: usize) -> (Pos2, Rect, Vec<Rect>) {
    let centre = h.state().pie.as_ref().unwrap().centre;
    let a = seg_angle(seg);
    let start = centre + egui::vec2(a.cos(), a.sin()) * 110.0;
    mv(h, start - egui::vec2(3.0, 3.0));
    mv(h, start);
    let info = h.state().pie_info(0);
    let items = sub_items(SEGS[seg], &info);
    let (size, rects) = sub_layout(SEGS[seg], &items);
    let sr = sub_rect(centre, seg, size, h.ctx.content_rect());
    let rects = rects
        .iter()
        .map(|q| q.translate(sr.min.to_vec2()))
        .collect();
    (start, sr, rects)
}

/// moving the pointer straight from a submenu segment to any part of its
/// panel keeps the panel open (it closed over Rename on the way to Colour)
#[test]
fn submenus_stay_open_on_the_way_to_them() {
    let subs: Vec<usize> = (0..SEGS.len()).filter(|&i| SEGS[i].has_sub()).collect();
    for at in [
        Pos2::new(600.0, 400.0),
        Pos2::new(1500.0, 400.0),
        Pos2::new(250.0, 300.0),
        Pos2::new(1550.0, 850.0),
        Pos2::new(850.0, 170.0),
    ] {
        for &seg in &subs {
            for which in 0..3 {
                let mut h = pie_harness(at);
                let centre = h.state().pie.as_ref().unwrap().centre;
                let (start, sr, rects) = onto(&mut h, seg);
                assert_eq!(open_sub(&h), Some(seg), "{:?} opens", SEGS[seg]);
                assert!(
                    (sr.center() - centre).length() > 0.0
                        && !sr.intersects(Rect::from_center_size(centre, egui::Vec2::splat(2.0)))
                );
                let target = match which {
                    0 => sr.center(),
                    1 => rects[0].center(),
                    _ => rects.last().unwrap().center(),
                };
                for k in 1..=24 {
                    let p = start + (target - start) * (k as f32 / 24.0);
                    mv(&mut h, p);
                    assert_eq!(
                        open_sub(&h),
                        Some(seg),
                        "{:?} pie at {at:?}, step {k}/24 to item {which}: closed over {:?}",
                        SEGS[seg],
                        hit(centre, p)
                    );
                }
            }
        }
    }
}

/// the Colour panel sits on the Colour side when there is room (it used to
/// flip to the far side of the pie)
#[test]
fn the_colour_panel_sits_beside_its_segment() {
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1700.0, 1000.0));
    let c = Pos2::new(600.0, 400.0);
    let (size, _) = sub_layout(
        Seg::Colour,
        &sub_items(
            Seg::Colour,
            &super::trackpie::TrackInfo {
                timeline: true,
                tracks: 4,
                ..Default::default()
            },
        ),
    );
    let i = SEGS.iter().position(|s| *s == Seg::Colour).unwrap();
    let r = sub_rect(c, i, size, screen);
    assert!(r.center().x > c.x, "right of the pie: {r:?}");
    assert!(screen.contains_rect(r));
    // clear of the ring
    let q = Pos2::new(
        c.x.clamp(r.left(), r.right()),
        c.y.clamp(r.top(), r.bottom()),
    );
    assert!((q - c).length() >= super::trackpie::R_OUT + 9.0, "{r:?}");
}

/// leaving the way to the panel for another segment switches at once;
/// resting on one on the way switches after a moment; a pick, Esc and a
/// click outside still close it
#[test]
fn submenu_switching_and_closing_stay_predictable() {
    let ci = SEGS.iter().position(|s| *s == Seg::Colour).unwrap();
    let ri = SEGS.iter().position(|s| *s == Seg::Routing).unwrap();
    let mut h = pie_harness(Pos2::new(600.0, 400.0));
    let centre = h.state().pie.as_ref().unwrap().centre;
    onto(&mut h, ci);
    assert_eq!(open_sub(&h), Some(ci));
    // straight to Routing (the other side): not on the way, so it closes
    let a = seg_angle(ri);
    mv(&mut h, centre + egui::vec2(a.cos(), a.sin()) * 100.0);
    assert_eq!(open_sub(&h), None, "Routing has no submenu");
    assert_eq!(h.state().pie.as_ref().unwrap().key, Some(ri));
    // back to Colour, then onto Icon (next to the panel's way) and rest
    let (_, sr, _) = onto(&mut h, ci);
    let ii = SEGS.iter().position(|s| *s == Seg::Icon).unwrap();
    let a = seg_angle(ii);
    let on_icon = centre + egui::vec2(a.cos(), a.sin()) * 140.0;
    mv(&mut h, on_icon);
    if super::trackpie::in_aim(
        centre + egui::vec2(seg_angle(ci).cos(), seg_angle(ci).sin()) * 110.0,
        sr,
        on_icon,
    ) {
        assert_eq!(open_sub(&h), Some(ci), "on the way: kept");
    }
    frames(&mut h, 40);
    assert_eq!(open_sub(&h), Some(ii), "resting on Icon opens its submenu");
    // a pick closes the pie and sets the colour (one undo step)
    let (_, _, rects) = onto(&mut h, ci);
    let swatch = rects[2].center();
    for k in 1..=10 {
        let from = centre + egui::vec2(seg_angle(ci).cos(), seg_angle(ci).sin()) * 110.0;
        mv(&mut h, from + (swatch - from) * (k as f32 / 10.0));
    }
    click_at(&mut h, swatch);
    frames(&mut h, 10);
    assert!(h.state().pie.is_none(), "a pick closes it");
    let col = h
        .state()
        .with(|s| s.song.timeline.as_ref().unwrap().tracks[0].colour);
    assert!(col.is_some(), "the colour is set at once");
    // Esc closes
    let ctx = h.ctx.clone();
    h.state_mut()
        .open_track_pie(0, Pos2::new(600.0, 400.0), false, &ctx);
    frames(&mut h, 5);
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 10);
    assert!(h.state().pie.is_none(), "Esc closes it");
    // a click outside the pie and its panel closes
    h.state_mut()
        .open_track_pie(0, Pos2::new(600.0, 400.0), false, &ctx);
    frames(&mut h, 5);
    onto(&mut h, ci);
    click_at(&mut h, Pos2::new(1500.0, 900.0));
    frames(&mut h, 10);
    assert!(h.state().pie.is_none(), "a click outside closes it");
}

// ------------------------------------------------------------ step 2: split preview

fn tl_now(h: &Harness<'static, App>) -> tl::Timeline {
    h.state()
        .with(|s| s.song.timeline.as_deref().cloned())
        .expect("timeline mode")
}

/// a clip on screen at least 2 bars long; the timeline at 40 px a beat,
/// snapping to beats
fn split_setup() -> (Harness<'static, App>, tl::Clip) {
    let mut h = tl_setup();
    h.state_mut().tlv.snap = super::timeline::Snap::Beat;
    h.state_mut().tlv.sel = None;
    h.state_mut().tlv.sels.clear();
    frames(&mut h, 2);
    let lanes = h.state().tlv.lanes;
    let cl = tl_now(&h)
        .clips
        .iter()
        .find(|c| {
            c.len >= 8 * tl::PPQ
                && h.state().tlv.clip_rect(c.id).is_some_and(|r| {
                    lanes.contains(r.left_center() + egui::vec2(4.0, 0.0))
                        && h.state().tlv.x_of(c.start + 8 * tl::PPQ) < lanes.right() - 20.0
                })
        })
        .cloned()
        .expect("a 2-bar clip on screen");
    (h, cl)
}

#[test]
fn s_shows_a_snapped_split_line_and_only_the_click_splits() {
    let (mut h, cl) = split_setup();
    let before = tl_now(&h);
    let r = h.state().tlv.clip_rect(cl.id).unwrap();
    // a beat and a bit into the clip: the line snaps to the beat
    let want = cl.start + 5 * tl::PPQ;
    let x = h.state().tlv.x_of(want) + 4.0;
    let p = Pos2::new(x, r.center().y);
    mv(&mut h, p);
    key(&mut h, Key::S, egui::Modifiers::NONE);
    assert!(h.state().tlv.split_arm, "S arms the split preview");
    mv(&mut h, p + egui::vec2(1.0, 0.0));
    assert_eq!(
        h.state().tlv.split_at,
        Some((cl.id, want)),
        "snapped to the beat"
    );
    assert_eq!(
        tl_now(&h).clips.len(),
        before.clips.len(),
        "nothing split yet"
    );
    // the preview follows the pointer
    let x2 = h.state().tlv.x_of(cl.start + 3 * tl::PPQ) + 2.0;
    mv(&mut h, Pos2::new(x2, r.center().y));
    assert_eq!(
        h.state().tlv.split_at,
        Some((cl.id, cl.start + 3 * tl::PPQ))
    );
    // Esc cancels: the song is unchanged
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    assert!(!h.state().tlv.split_arm);
    assert_eq!(tl_now(&h).clips, before.clips);
    // again, and the click commits exactly where the line was
    mv(&mut h, p);
    key(&mut h, Key::S, egui::Modifiers::NONE);
    mv(&mut h, p + egui::vec2(1.0, 0.0));
    let shown = h.state().tlv.split_at;
    click_at(&mut h, p + egui::vec2(1.0, 0.0));
    let after = tl_now(&h);
    assert!(!h.state().tlv.split_arm, "the click ends it");
    assert_eq!(after.clips.len(), before.clips.len() + 1);
    let left = after.clips.iter().find(|c| c.id == cl.id).unwrap();
    let right = after
        .clips
        .iter()
        .find(|c| !before.clips.iter().any(|b| b.id == c.id))
        .unwrap();
    assert_eq!(shown, Some((cl.id, want)));
    assert_eq!((left.start, left.end()), (cl.start, want));
    assert_eq!((right.start, right.end()), (want, cl.end()));
    assert_eq!(
        (right.kind.clone(), right.track, right.offset),
        (cl.kind.clone(), cl.track, cl.offset + (want - cl.start)),
        "same data, track and timing"
    );
    // other clips are untouched
    for b in before.clips.iter().filter(|b| b.id != cl.id) {
        assert!(after.clips.contains(b));
    }
    // one undo step back, redo again
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(tl_now(&h).clips, before.clips, "undo");
    h.state_mut().undo(true);
    frames(&mut h, 2);
    assert_eq!(tl_now(&h).clips.len(), before.clips.len() + 1, "redo");
}

#[test]
fn split_from_the_menu_waits_for_the_click_and_right_click_cancels() {
    let (mut h, cl) = split_setup();
    // the list menu (Settings › Clip menus › List; the pie has Split… too)
    h.state_mut().extra.clip_list_menu = true;
    let before = tl_now(&h);
    let r = h.state().tlv.clip_rect(cl.id).unwrap();
    let p = Pos2::new(
        h.state().tlv.x_of(cl.start + 4 * tl::PPQ) + 3.0,
        r.center().y,
    );
    click_at(&mut h, p);
    assert_eq!(h.state().tlv.sel, Some(cl.id));
    right_click(&mut h, p);
    frame(&mut h);
    h.query_by_label_contains("Split…")
        .expect("the clip menu offers Split…")
        .click();
    frames(&mut h, 3);
    assert!(h.state().tlv.split_arm, "the menu arms the preview");
    assert_eq!(
        tl_now(&h).clips,
        before.clips,
        "no split at the menu's spot"
    );
    // a click outside every clip does not split, the preview stays
    let lanes = h.state().tlv.lanes;
    click_at(&mut h, Pos2::new(lanes.right() - 5.0, lanes.bottom() - 5.0));
    assert!(h.state().tlv.split_arm);
    assert_eq!(tl_now(&h).clips, before.clips);
    // right-click cancels (and opens no menu)
    mv(&mut h, p);
    right_click(&mut h, p);
    assert!(!h.state().tlv.split_arm);
    assert_eq!(tl_now(&h).clips, before.clips);
}

/// a selected clip on another track is not cut by a split of this one
#[test]
fn split_cuts_the_clip_under_the_pointer_not_other_tracks_selection() {
    let (mut h, cl) = split_setup();
    let before = tl_now(&h);
    let at = cl.start + 4 * tl::PPQ;
    let other = before
        .clips
        .iter()
        .find(|c| c.track != cl.track && c.start < at && at < c.end())
        .cloned();
    if let Some(o) = &other {
        h.state_mut().tlv.sel = Some(o.id);
        h.state_mut().tlv.sels = vec![o.id];
    }
    let r = h.state().tlv.clip_rect(cl.id).unwrap();
    let p = Pos2::new(h.state().tlv.x_of(at) + 3.0, r.center().y);
    mv(&mut h, p);
    key(&mut h, Key::S, egui::Modifiers::NONE);
    mv(&mut h, p + egui::vec2(1.0, 0.0));
    click_at(&mut h, p + egui::vec2(1.0, 0.0));
    let after = tl_now(&h);
    assert_eq!(after.clips.len(), before.clips.len() + 1, "one clip cut");
    if let Some(o) = other {
        assert!(after.clips.contains(&o), "the other track's clip is whole");
    }
}

// ------------------------------------------------------- step 3: Add › Automation lane

fn track_auto(h: &Harness<'static, App>, tr: usize) -> Vec<(String, bool)> {
    h.state().with(|s| {
        s.song.timeline.as_deref().unwrap().tracks[tr]
            .auto
            .iter()
            .map(|l| (l.target.clone(), l.enabled))
            .collect()
    })
}

/// the pie's Add › Automation lane opens the track's lane menu; picking a
/// parameter makes a lane on that track (one undo step), the menu shows /
/// hides the lanes and switches them on and off, and it all saves
#[test]
fn add_automation_lane_from_the_track_pie() {
    let tr = 1;
    let mut h = pie_harness(Pos2::new(600.0, 400.0));
    // the entry is in Add › and acts in timeline mode
    let add = SEGS.iter().position(|s| *s == Seg::Add).unwrap();
    let _ = add;
    let info = h.state().pie_info(tr);
    use super::trackring::{ring_slots, Go, Level};
    assert!(ring_slots(Level::Add, &info)
        .iter()
        .any(|s| s.go == Some(Go::Open(Level::Automation))));
    assert!(ring_slots(Level::Automation, &info)
        .iter()
        .any(|s| s.go == Some(Go::Act(PieAct::Automation))));
    let before = track_auto(&h, tr);
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(tr, PieAct::Automation, &ctx);
    frames(&mut h, 3);
    assert!(h.state().track_auto.as_ref().is_some_and(|m| m.track == tr));
    assert!(h
        .query_by_label_contains(&format!("AUTOMATION · {:02}", tr + 1))
        .is_some());
    // pick track 2's instrument's resonance (the demo has its cutoff lane) in the picker submenu
    h.get_by_label("New lane: pick a parameter ›").click();
    frames(&mut h, 2);
    h.get_all_by(|n| n.label().is_some_and(|l| l == "02 Gallop Bass - Resonance"))
        .next()
        .expect("a cutoff parameter")
        .click();
    frames(&mut h, 3);
    let after = track_auto(&h, tr);
    assert_eq!(after.len(), before.len() + 1, "one new lane on track {tr}");
    assert_eq!(
        after.last().unwrap().0,
        crate::automation::Target::Resonance(1).id()
    );
    assert!(h.state().track_auto.is_none(), "picking closes the menu");
    assert!(h.state().track_lanes_shown(tr), "a new lane shows itself");
    // one undo step
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(track_auto(&h, tr), before);
    h.state_mut().undo(true);
    frames(&mut h, 2);
    assert_eq!(track_auto(&h, tr), after);
    // reopen: hide / show and switch the lane off
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(tr, PieAct::Automation, &ctx);
    frames(&mut h, 3);
    h.get_by_label("Hide this track's lanes").click();
    frames(&mut h, 2);
    assert!(!h.state().track_lanes_shown(tr));
    h.get_by_label("Show this track's lanes").click();
    frames(&mut h, 2);
    assert!(h.state().track_lanes_shown(tr));
    let label = h.state().track_lanes(tr).last().unwrap().1.clone();
    h.get_by_label(&label).click();
    frames(&mut h, 2);
    assert!(!track_auto(&h, tr).last().unwrap().1, "the lane is off");
    assert!(h.state().track_auto.is_some(), "toggles keep the menu open");
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().track_auto.is_none(), "Esc closes it");
    // saves and loads
    let p = std::env::temp_dir().join(format!("cw-wf-auto-{}.cwt", std::process::id()));
    let song = h.state().with(|s| s.song.clone());
    fileio::save_cwt(&song, &p).unwrap();
    let back = fileio::load_cwt(&p).unwrap();
    let _ = std::fs::remove_file(&p);
    let lanes: Vec<(String, bool)> = back.timeline.as_deref().unwrap().tracks[tr]
        .auto
        .iter()
        .map(|l| (l.target.clone(), l.enabled))
        .collect();
    assert_eq!(lanes, track_auto(&h, tr));
}

/// tracker mode: the entry says why and opens nothing
#[test]
fn automation_lane_menu_needs_timeline_mode() {
    let mut h = harness_with(fileio::demo_song());
    frames(&mut h, 2);
    h.state_mut().track_auto_open(0, Pos2::new(100.0, 100.0));
    frame(&mut h);
    assert!(h.state().track_auto.is_none());
    assert!(h.state().status.contains("timeline mode"));
}

// ------------------------------------------------------- step 4: chord replace

fn cmaj(start: tl::Tick) -> crate::chordgen::ChordMark {
    crate::chordgen::ChordMark {
        start,
        len: chordlane::BAR,
        root: 0,
        q: crate::chordgen::Quality::Maj,
        bass: None,
    }
}

fn put_notes(h: &mut Harness<'static, App>, notes: &[(tl::Tick, u8)]) {
    let t = h.state().pr.target.unwrap();
    let mut d = note_data(h);
    d.notes.extend(notes.iter().map(|&(start, key)| tl::Note {
        start,
        len: tl::PPQ / 2,
        key,
        vel: tl::roll::vel_u16(0.6),
        ..Default::default()
    }));
    h.state_mut().pr_write(t, pianoview::Source::Notes(d));
    h.state_mut().undo_reset();
    frame(h);
}

fn keys_at(d: &tl::NoteClipData) -> Vec<(tl::Tick, u8)> {
    let mut v: Vec<_> = d.notes.iter().map(|n| (n.start, n.key)).collect();
    v.sort();
    v
}

/// Generate › Apply with notes on: the chord notes replace the notes that
/// start inside the chord's span (in range, few: no question), the rest of
/// the clip stays, a second run adds nothing, one undo step
#[test]
fn chord_notes_replace_their_span_only_and_never_double() {
    let mut h = clip_setup();
    let t = h.state().pr.target.unwrap();
    let v = crate::chordgen::Voicing::Full;
    let voiced: Vec<u8> = crate::chordgen::voice(&[cmaj(0)], v, 4)
        .iter()
        .map(|x| x.2)
        .collect();
    let (lo, hi) = (*voiced.iter().min().unwrap(), *voiced.iter().max().unwrap());
    // two old notes in the first bar (inside the chord's range), one in bar 2
    put_notes(
        &mut h,
        &[(0, lo + 1), (tl::PPQ, hi - 1), (chordlane::BAR, 30)],
    );
    h.state_mut().cl_insert(t, vec![cmaj(0)], true, v, 4);
    frame(&mut h);
    assert!(
        h.state().cl.confirm.is_none(),
        "2 in-range notes: no question"
    );
    let once = note_data(&h);
    let mut want: Vec<(tl::Tick, u8)> = voiced.iter().map(|&k| (0, k)).collect();
    want.push((chordlane::BAR, 30));
    want.sort();
    assert_eq!(keys_at(&once), want, "span replaced, bar 2 kept");
    assert_eq!(once.chords.len(), 1);
    assert!(
        h.state().status.contains("replaced 2 note(s)"),
        "{}",
        h.state().status
    );
    // twice: the same
    h.state_mut().cl_insert(t, vec![cmaj(0)], true, v, 4);
    frame(&mut h);
    assert!(h.state().cl.confirm.is_none());
    assert_eq!(keys_at(&note_data(&h)), want, "no duplicates");
    // the second run changed nothing, so one undo step goes back to the start
    h.state_mut().undo(false);
    frame(&mut h);
    assert_eq!(
        keys_at(&note_data(&h)),
        vec![(0, lo + 1), (tl::PPQ, hi - 1), (chordlane::BAR, 30)]
    );
    // Add on top keeps the old notes, and still never doubles
    h.state_mut().cl.gen.on_top = true;
    for _ in 0..2 {
        h.state_mut().cl_insert(t, vec![cmaj(0)], true, v, 4);
        frame(&mut h);
    }
    let mut top: Vec<(tl::Tick, u8)> = voiced.iter().map(|&k| (0, k)).collect();
    top.extend([(0, lo + 1), (tl::PPQ, hi - 1), (chordlane::BAR, 30)]);
    top.sort();
    assert_eq!(keys_at(&note_data(&h)), top);
}

/// more than 8 notes, or notes outside the chord's pitch range, ask first;
/// Cancel changes nothing, Replace / Add on top do what they say
#[test]
fn chord_replace_asks_before_deleting_many_or_out_of_range_notes() {
    let mut h = clip_setup();
    let t = h.state().pr.target.unwrap();
    let v = crate::chordgen::Voicing::Full;
    let voiced: Vec<u8> = crate::chordgen::voice(&[cmaj(0)], v, 4)
        .iter()
        .map(|x| x.2)
        .collect();
    let lo = *voiced.iter().min().unwrap();
    // 10 in-range notes in the chord's bar
    let many: Vec<(tl::Tick, u8)> = (0..10).map(|i| (i * tl::PPQ / 4 + 1, lo + 1)).collect();
    put_notes(&mut h, &many);
    let before = note_data(&h);
    h.state_mut().cl_insert(t, vec![cmaj(0)], true, v, 4);
    frames(&mut h, 2);
    let c = h.state().cl.confirm.as_ref().expect("asks first");
    assert_eq!((c.removed, c.outside), (10, 0));
    assert!(
        c.scope.contains("track 03") && c.scope.contains("bar 1"),
        "{}",
        c.scope
    );
    assert!(h
        .query_by_label_contains("10 note(s) will be deleted")
        .is_some());
    assert!(h.query_by_label_contains("Target: ").is_some());
    assert_eq!(note_data(&h), before, "nothing yet");
    h.get_by_label("Cancel").click();
    frames(&mut h, 2);
    assert!(h.state().cl.confirm.is_none());
    assert_eq!(
        note_data(&h),
        before,
        "Cancel writes nothing (no markers either)"
    );
    // again, Replace
    h.state_mut().cl_insert(t, vec![cmaj(0)], true, v, 4);
    frames(&mut h, 2);
    h.get_by_label("Replace (delete 10)").click();
    frames(&mut h, 2);
    let mut want: Vec<(tl::Tick, u8)> = voiced.iter().map(|&k| (0, k)).collect();
    want.sort();
    assert_eq!(keys_at(&note_data(&h)), want);
    assert_eq!(note_data(&h).chords.len(), 1);
    h.state_mut().undo(false);
    frame(&mut h);
    assert_eq!(note_data(&h), before, "one undo step");
    // a melody note above the chord: asks even though it's only one
    put_notes(&mut h, &[]);
    let mut d = note_data(&h);
    d.notes.clear();
    h.state_mut().pr_write(t, pianoview::Source::Notes(d));
    put_notes(&mut h, &[(tl::PPQ, 100)]);
    h.state_mut().cl_insert(t, vec![cmaj(0)], true, v, 4);
    frames(&mut h, 2);
    let c = h
        .state()
        .cl
        .confirm
        .as_ref()
        .expect("asks for an out-of-range note");
    assert_eq!((c.removed, c.outside), (1, 1));
    assert!(h
        .query_by_label_contains("outside the new chords' pitch range")
        .is_some());
    h.get_by_label("Add on top").click();
    frames(&mut h, 2);
    let mut top = want.clone();
    top.push((tl::PPQ, 100));
    top.sort();
    assert_eq!(keys_at(&note_data(&h)), top, "Add on top keeps the melody");
}

/// the chord pie's "Write notes" replaces too: twice is the same as once
#[test]
fn write_notes_from_the_chord_pie_never_doubles() {
    let mut h = clip_setup();
    let t = h.state().pr.target.unwrap();
    h.state_mut().cl_write(t, vec![cmaj(0)], vec![], true);
    frame(&mut h);
    let mut counts = vec![];
    for _ in 0..2 {
        let ctx = h.ctx.clone();
        h.state_mut()
            .cl_open_menu(&ctx, Pos2::new(600.0, 500.0), chordlane::PieOn::Mark(0), t);
        frames(&mut h, 12);
        let w = h.state().cl.pie.as_ref().unwrap().0.wedge_centre(7, 75.0);
        click_at(&mut h, w);
        frames(&mut h, 2);
        assert!(h.state().cl.confirm.is_none());
        counts.push(note_data(&h).notes.len());
    }
    assert!(counts[0] >= 3, "the chord's notes: {counts:?}");
    assert_eq!(counts[0], counts[1], "no duplicates");
}

/// the Generate dialog names its target clip / bars and the scope
#[test]
fn generate_dialog_shows_target_and_scope() {
    let mut h = clip_setup();
    h.state_mut().cl_open_gen_at(chordlane::BAR);
    h.state_mut().cl.gen.write = true;
    frames(&mut h, 3);
    assert!(h.query_by_label_contains("track 03, bars 2–").is_some());
    assert!(h
        .query_by_label_contains("replaces the notes inside each chord's span only")
        .is_some());
    h.get_by_label("Add on top (keep the notes already there)")
        .click();
    frames(&mut h, 2);
    assert!(h.state().cl.gen.on_top);
    assert!(h
        .query_by_label_contains("notes added on top of the clip's notes")
        .is_some());
}

// ------------------------------------------------- step 5: piano-key audition

fn press_at(h: &mut Harness<'static, App>, p: Pos2, pressed: bool) {
    with_mods(
        h,
        egui::Modifiers::NONE,
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        },
    );
}

/// pressing a key on the roll's keyboard plays it on the track's
/// instrument (not the globally selected one), dragging plays the next
/// key, release stops it; no note and no undo step. Hover names the key.
#[test]
fn piano_keys_audition_on_the_tracks_instrument() {
    let mut h = clip_setup();
    let tr = 2;
    h.state().with(|s| {
        s.song.timeline.as_deref_mut().unwrap().tracks[tr].instr = 3;
    });
    h.state_mut().instr = 1;
    h.state_mut().undo_reset();
    frame(&mut h);
    let before = note_data(&h);
    let gr = h.state().pr.grid_rect;
    let kh = h.state().pr.key_h;
    let key_pos = |h: &Harness<'static, App>, k: u8| {
        Pos2::new(gr.left() - 12.0, h.state().pr.y_of(k) + kh / 2.0)
    };
    // the middle of the visible keys
    let mid = h.state().pr.key_at_pub(gr.center().y) as u8;
    let p = key_pos(&h, mid);
    mv(&mut h, p);
    frames(&mut h, 2);
    press_at(&mut h, p, true);
    frame(&mut h);
    assert_eq!(
        h.state().pr.heard,
        Some((mid, 3, tr as u8)),
        "the track's instrument"
    );
    assert_eq!(h.state().pr.key_down, Some(mid));
    // drag to the key above
    let q = key_pos(&h, mid + 1);
    mv(&mut h, q);
    frame(&mut h);
    assert_eq!(h.state().pr.heard, Some((mid + 1, 3, tr as u8)));
    assert_eq!(h.state().pr.key_down, Some(mid + 1));
    press_at(&mut h, q, false);
    frames(&mut h, 2);
    assert_eq!(h.state().pr.key_down, None, "release stops it");
    assert_eq!(note_data(&h), before, "no note written");
    h.state_mut().undo(false);
    frame(&mut h);
    assert_eq!(note_data(&h), before, "no undo step either");
    // the hover tooltip names the key and octave
    mv(&mut h, p + egui::vec2(1.0, 0.0));
    for _ in 0..30 {
        h.step();
        if h.query_all_by_label_contains(&format!("{} · octave", pianoview::key_label(mid)))
            .next()
            .is_some()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    assert!(h
        .query_all_by_label_contains(&format!(
            "{} · octave {}",
            pianoview::key_label(mid),
            (mid - 1) / 12
        ))
        .next()
        .is_some());
    // chord previews use the track's instrument too
    let m = cmaj(0);
    h.state_mut().cl_play(&m);
    assert_eq!(h.state().pr.heard.map(|x| x.1), Some(3));
}

#[test]
fn key_labels_count_like_the_tracker() {
    assert_eq!(pianoview::key_label(1), "C0");
    assert_eq!(pianoview::key_label(49), "C4");
    assert_eq!(pianoview::key_label(50), "C#4");
    assert_eq!(pianoview::key_label(120), "B9");
    assert_eq!(crate::model::note_name(50), "C#4");
}

// ------------------------------------------- step 6: new notes play the track's instrument

fn render_fp(s: &Song) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    for x in crate::engine::render_song(s, 11025) {
        x.to_bits().hash(&mut hs);
    }
    hs.finish()
}

/// the audit's repro: a track given instrument 2 drew notes that played
/// instrument 1. Notes drawn in the roll, written by the chord lane and
/// step-input now carry the track's instrument, so they sound like it;
/// notes already there keep theirs (their sound never changes)
#[test]
fn new_notes_play_the_tracks_instrument() {
    let mut h = clip_setup();
    let tr = 2;
    let pick = 2u8;
    h.state().with(|s| {
        s.song.timeline.as_deref_mut().unwrap().tracks[tr].instr = pick;
    });
    // an old note with no instrument of its own stays that way
    put_notes(&mut h, &[(3 * tl::PPQ, 40)]);
    h.state_mut().instr = 1;
    frame(&mut h);
    // draw a note with a click in the roll
    let gr = h.state().pr.grid_rect;
    let key = h.state().pr.key_at_pub(gr.center().y) as u8;
    let p = Pos2::new(
        h.state().pr.x_of(0) + 5.0,
        h.state().pr.y_of(key) + h.state().pr.key_h / 2.0,
    );
    click_at(&mut h, p);
    frame(&mut h);
    let d = note_data(&h);
    let drawn = d.notes.iter().find(|n| n.start == 0 && n.key == key);
    assert_eq!(drawn.map(|n| n.instr), Some(pick), "{:?}", d.notes);
    assert_eq!(
        d.notes.iter().find(|n| n.key == 40).map(|n| n.instr),
        Some(0),
        "old notes untouched"
    );
    // chord-lane notes
    let t = h.state().pr.target.unwrap();
    h.state_mut()
        .cl_insert(t, vec![cmaj(chordlane::BAR)], true, Default::default(), 4);
    frame(&mut h);
    let d = note_data(&h);
    let chord: Vec<u8> = d
        .notes
        .iter()
        .filter(|n| n.start == chordlane::BAR)
        .map(|n| n.instr)
        .collect();
    assert!(
        !chord.is_empty() && chord.iter().all(|&i| i == pick),
        "{chord:?}"
    );
    // and it is what you hear: the drawn note renders like an explicit
    // instrument-2 note, not like instrument 1
    let solo = |instr: u8| {
        let mut s = h.state().with(|s| s.song.clone());
        let id = h.state().tlv.sel.unwrap();
        let t = s.timeline.as_deref_mut().unwrap();
        t.clips.retain(|c| c.id == id);
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else {
            panic!()
        };
        let d = t.notes[data as usize].make_mut();
        d.notes.retain(|n| n.start == 0 && n.key == key);
        d.notes[0].instr = instr;
        s
    };
    let ours = render_fp(&solo(pick));
    let mut as_drawn = solo(pick);
    {
        let t = as_drawn.timeline.as_deref_mut().unwrap();
        let tl::ClipKind::Notes { data } = t.clips[0].kind else {
            panic!()
        };
        t.notes[data as usize].make_mut().notes[0] = drawn.unwrap().clone();
    }
    assert_eq!(render_fp(&as_drawn), ours);
    assert_ne!(render_fp(&solo(1)), ours, "instrument 1 sounds different");
}

/// a track whose instrument was deleted: new notes fall back to the
/// channel's sound (instr 0), the status says why, the header says
/// "missing", nothing panics
#[test]
fn a_missing_track_instrument_is_explained_not_a_panic() {
    let mut h = clip_setup();
    let tr = 2;
    h.state().with(|s| {
        s.song.timeline.as_deref_mut().unwrap().tracks[tr].instr = 200;
    });
    frames(&mut h, 2);
    let gr = h.state().pr.grid_rect;
    let key = h.state().pr.key_at_pub(gr.center().y) as u8;
    let p = Pos2::new(
        h.state().pr.x_of(0) + 5.0,
        h.state().pr.y_of(key) + h.state().pr.key_h / 2.0,
    );
    click_at(&mut h, p);
    frame(&mut h);
    let d = note_data(&h);
    assert_eq!(d.notes.first().map(|n| n.instr), Some(0));
    assert!(
        h.state().status.contains("is missing"),
        "{}",
        h.state().status
    );
    // auditions and typing use the selected instrument instead
    h.state_mut().instr = 1;
    assert_eq!(h.state().track_instr(tr), 1);
    let hdr = h.state().channel_headers(4);
    assert_eq!(hdr.instruments[tr], "C8 (missing instrument)");
}

// ------------------------------------- step 3b: Change instrument… / Drum pattern…

fn br_item(f: impl Fn(&crate::browser::Item) -> bool) -> crate::browser::Item {
    crate::browser::builtin_items()
        .into_iter()
        .find(|i| f(i))
        .expect("a built-in item")
}

/// Add › Change instrument… opens the Browser's Instruments (Favourites
/// first when there are some) and the next pick replaces this track's
/// instrument, one undo step; nothing else is aimed at the track after it
#[test]
fn change_instrument_from_the_track_pie() {
    let mut h = pie_harness(Pos2::new(600.0, 400.0));
    let tr = 1;
    use super::trackring::{ring_slots, Go, Level};
    let items = ring_slots(Level::Instrument, &h.state().pie_info(tr));
    assert!(items
        .iter()
        .any(|i| i.go == Some(Go::Act(PieAct::ChangeInstrument))));
    h.state_mut().extra.browser_favs = vec!["something".into()];
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(tr, PieAct::ChangeInstrument, &ctx);
    frames(&mut h, 2);
    {
        let a = h.state();
        assert!(a.extra.browser_open);
        assert_eq!(a.br.filter.tab, crate::browser::Tab::Instruments);
        assert_eq!(a.br.filter.loc, crate::browser::LocFilter::Favourites);
        assert_eq!(a.br.track_target.map(|t| t.0), Some(tr));
    }
    h.state_mut().undo_reset();
    let (n_inst, before) = h.state().with(|s| {
        (
            s.song.instruments.len(),
            s.song.timeline.as_deref().unwrap().tracks[tr].instr,
        )
    });
    let it = br_item(|i| matches!(i.src, crate::browser::Src::Gen(_)));
    h.state_mut().br_load(&it, &ctx);
    frames(&mut h, 2);
    let after = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().tracks[tr].instr);
    assert_eq!(
        after as usize,
        n_inst + 1,
        "the new instrument on track {tr}"
    );
    assert!(h.state().br.track_target.is_none());
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(
        h.state().with(|s| (
            s.song.instruments.len(),
            s.song.timeline.as_deref().unwrap().tracks[tr].instr
        )),
        (n_inst, before),
        "one undo step"
    );
    // opening the Browser some other way forgets the target
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(tr, PieAct::ChangeInstrument, &ctx);
    h.state_mut()
        .open_browser_tab(crate::browser::Tab::Instruments);
    assert!(h.state().br.track_target.is_none());
}

// ------------------------------------------- Del's crash path (20 Oct, evening)

fn dbl_at(h: &mut Harness<'static, App>, p: Pos2) {
    with_mods(h, egui::Modifiers::NONE, egui::Event::PointerMoved(p));
    let mut t = h.ctx.input(|i| i.time);
    for _ in 0..2 {
        for pressed in [true, false] {
            t += 0.05;
            h.input_mut().time = Some(t);
            with_mods(
                h,
                egui::Modifiers::NONE,
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            );
        }
    }
    h.input_mut().time = None;
    frames(h, 2);
}

/// a new song, + Add track, double-click the track: a new empty note clip
/// open in the piano roll
fn new_clip_setup() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().new_song();
    // the default install: the Tracker hidden, so a double-click on empty
    // space makes a note clip
    h.state_mut().extra.show_tracker = false;
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    h.get_by_label("+ Add track").click();
    frames(&mut h, 3);
    let (_, row) = h.state().tlv.track_rects(0).expect("the new track");
    let lanes = h.state().tlv.lanes;
    dbl_at(&mut h, Pos2::new(lanes.left() + 40.0, row.center().y));
    frames(&mut h, 3);
    assert!(h.state().pr.open, "the roll opens on the new clip");
    eprintln!("target: {:?}", h.state().pr_target());
    h
}

/// Del's steps: new track, double-click a clip, right-click the roll's
/// chord lane, Generate, click a style
#[test]
fn del_new_clip_chord_pie_generate_style_click() {
    use crate::chordgen::{PIE_STYLES, STYLES};
    let n = PIE_STYLES.len();
    // every way into the dialog from the pie: a style wedge (n of them),
    // More… (n + 1)
    for wedge in (0..n).chain([n + 1]) {
        let mut h = new_clip_setup();
        let lane = h.state().cl.rect;
        assert!(lane.height() > 0.0, "the chord lane shows");
        let p = Pos2::new(lane.left() + 60.0, lane.center().y);
        right_click(&mut h, p);
        frames(&mut h, 12);
        let w = h
            .state()
            .cl
            .pie
            .as_ref()
            .map(|p| p.0.wedge_centre(wedge, 75.0))
            .expect("the chord pie opens");
        click_at(&mut h, w);
        frames(&mut h, 3);
        assert!(h.state().cl.gen.open, "wedge {wedge} opens Generate");
        for st in STYLES.iter() {
            h.get_by_label(st.name).click();
            frames(&mut h, 2);
        }
        for st in STYLES.iter().rev() {
            h.get_by_label(st.name).click();
            frame(&mut h);
        }
    }
}

fn render_some(h: &mut Harness<'static, App>) {
    h.state().with(|s| {
        let crate::audio::Shared { song, engine, .. } = s;
        let mut buf = vec![0.0f32; 128];
        for _ in 0..400 {
            engine.render(song, &mut buf);
        }
    });
}

/// the pie's presets: point at a style, run over its progressions (each
/// plays), click one, and the new clip plays it
#[test]
fn del_new_clip_chord_pie_presets_hover_pick_and_play() {
    use crate::chordgen::{PIE_STYLES, STYLES};
    for (wi, &st) in PIE_STYLES.iter().enumerate() {
        for k in 0..STYLES[st].progs.len() {
            let mut h = new_clip_setup();
            let lane = h.state().cl.rect;
            right_click(&mut h, Pos2::new(lane.left() + 60.0, lane.center().y));
            frames(&mut h, 12);
            let w = h.state().cl.pie.as_ref().unwrap().0.wedge_centre(wi, 75.0);
            h.event(egui::Event::PointerMoved(w));
            frames(&mut h, 4);
            for kk in 0..STYLES[st].progs.len() {
                let c = h.state().cl.pie.as_ref().unwrap().0.child_centre(wi, kk);
                h.event(egui::Event::PointerMoved(c));
                frames(&mut h, 3);
                render_some(&mut h);
            }
            let c = h.state().cl.pie.as_ref().unwrap().0.child_centre(wi, k);
            click_at(&mut h, c);
            frames(&mut h, 4);
            render_some(&mut h);
            assert!(
                !h.state()
                    .cl_marks(h.state().pr_target().unwrap())
                    .is_empty(),
                "style {st} prog {k} wrote chords"
            );
            h.state_mut().play(1);
            frames(&mut h, 2);
            render_some(&mut h);
            frames(&mut h, 2);
        }
    }
}

/// fuzz the Generate dialog on a brand-new clip: random song keys and dialog
/// settings, random clicks on every control, Apply / Cancel / reopen,
/// undo / redo, playback. Any panic names its seed and step.
#[test]
fn generate_dialog_fuzz_on_a_new_clip() {
    use crate::chordgen::{Voicing, STYLES};
    use crate::chords::{Key, Scale};
    let seeds: u64 = std::env::var("CW_FUZZ_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let steps = 50;
    let mut fixed: Vec<&str> = vec![
        "Any",
        "Major",
        "Minor",
        "4",
        "8",
        "16",
        "32",
        "Slower",
        "Style",
        "Faster",
        " - ",
        " + ",
        "Another one",
        "Preview",
        "Apply",
        "Cancel",
    ];
    fixed.extend([
        "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
    ]);
    let mut wrote = 0;
    for seed in 0..seeds {
        let mut r = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut rnd = move |n: usize| {
            r ^= r << 13;
            r ^= r >> 7;
            r ^= r << 17;
            (r % n.max(1) as u64) as usize
        };
        let mut h = new_clip_setup();
        let key = match rnd(3) {
            0 => None,
            _ => Some(Key::new(rnd(12) as u8, Scale::ALL[rnd(Scale::ALL.len())])),
        };
        h.state().with(|s| s.song.info.key = key);
        let mut log = vec![format!("key {key:?}")];
        let mut had = false;
        let t = h.state().pr_target().unwrap();
        h.state_mut().cl_open_gen_at(0);
        frames(&mut h, 2);
        for step in 0..steps {
            let what = rnd(10);
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let a = h.state_mut();
                match what {
                    0 => {
                        let g = &mut a.cl.gen;
                        g.write = rnd(2) == 0;
                        g.on_top = rnd(3) == 0;
                        g.voicing = Voicing::ALL[rnd(Voicing::ALL.len())];
                        g.octave = 1 + rnd(7) as u8;
                        g.opts.colour = rnd(101) as f32 / 100.0;
                        g.opts.variation = rnd(101) as f32 / 100.0;
                        g.opts.walking_bass = rnd(2) == 0;
                        format!(
                            "set write {} top {} {:?} oct {} col {} var {} walk {}",
                            g.write,
                            g.on_top,
                            g.voicing,
                            g.octave,
                            g.opts.colour,
                            g.opts.variation,
                            g.opts.walking_bass
                        )
                    }
                    1 if !a.cl.gen.open => {
                        a.cl_open_gen_at(rnd(6) as i64 * 4 * tl::PPQ);
                        "reopen".into()
                    }
                    2 => {
                        a.undo(false);
                        "undo".into()
                    }
                    3 => {
                        a.undo(true);
                        "redo".into()
                    }
                    4 => {
                        a.play(1);
                        "play".into()
                    }
                    5..=7 => format!("style {}", STYLES[rnd(STYLES.len())].name),
                    8 if rnd(2) == 0 => "btn Apply".into(),
                    8 => "btn Generate".into(),
                    _ => format!("btn {}", fixed[rnd(fixed.len())]),
                }
            }));
            let Ok(act) = res else {
                panic!("seed {seed} step {step}: {log:?}");
            };
            log.push(act.clone());
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let label = act
                    .strip_prefix("style ")
                    .or_else(|| act.strip_prefix("btn "));
                if let Some(l) = label {
                    if let Some(n) = h.query_all_by_label(l).next() {
                        n.click();
                    }
                }
                frames(&mut h, 2);
                if act == "reopen" || act == "btn Generate" {
                    // a new window's first frame is a sizing pass
                    frames(&mut h, 2);
                }
                if let Some(name) = act.strip_prefix("style ") {
                    let g = &h.state().cl.gen;
                    // (an undo that removes the new clip hides the dialog)
                    let shown = h.query_all_by_label(name).next().is_some();
                    if g.open && shown {
                        assert_eq!(STYLES[g.opts.style].name, name, "the click lands");
                    }
                }
                render_some(&mut h);
                // a pending "Replace notes?" ask: answer it either way
                if h.state().cl.confirm.is_some() {
                    let l = if rnd(2) == 0 {
                        "Replace (delete"
                    } else {
                        "Cancel"
                    };
                    if let Some(n) = h.query_all_by_label_contains(l).last() {
                        n.click();
                    }
                    frames(&mut h, 2);
                }
            }));
            if res.is_err() {
                panic!("seed {seed} step {step}: {log:?}");
            }
            had |= !h.state().cl_marks(t).is_empty();
        }
        if had {
            wrote += 1;
        }
    }
    // the clicks land: most runs end with chords in the clip
    assert!(
        wrote * 3 >= seeds,
        "only {wrote} of {seeds} runs wrote chords"
    );
}

/// a chord on the new clip: every entry of its pie (Generate included,
/// then every style), each child, with undo between; nothing panics
#[test]
fn every_chord_pie_entry_on_a_new_clips_chord() {
    use crate::chordgen::STYLES;
    let mut h = new_clip_setup();
    let t = h.state().pr_target().unwrap();
    h.state_mut().cl_add_tonic(t, 0, 4 * tl::PPQ as tl::Tick);
    frames(&mut h, 2);
    assert_eq!(h.state().cl_marks(t).len(), 1);
    let lane = h.state().cl.rect;
    right_click(&mut h, Pos2::new(lane.left() + 20.0, lane.center().y));
    frames(&mut h, 6);
    let kids: Vec<usize> = {
        let (pie, on) = h.state().cl.pie.as_ref().expect("the chord's pie");
        assert!(matches!(on, chordlane::PieOn::Mark(0)));
        pie.items.iter().map(|i| i.children.len()).collect()
    };
    h.state_mut().cl.pie = None;
    for write in [false, true] {
        h.state_mut().cl.gen.write = write;
        for (item, &n) in kids.iter().enumerate() {
            for child in std::iter::once(None).chain((0..n).map(Some)) {
                h.state_mut().cl_mark_action(t, 0, item, child);
                frames(&mut h, 3);
                if h.state().cl.gen.open {
                    for st in STYLES.iter() {
                        h.get_by_label(st.name).click();
                        frames(&mut h, 2);
                    }
                    h.get_by_label("Apply").click();
                    frames(&mut h, 2);
                }
                h.state_mut().cl.edit = None;
                render_some(&mut h);
                frames(&mut h, 1);
                h.state_mut().undo(false);
                frames(&mut h, 2);
                if h.state().cl_marks(t).is_empty() {
                    h.state_mut().cl_add_tonic(t, 0, 4 * tl::PPQ as tl::Tick);
                    frames(&mut h, 2);
                }
            }
        }
    }
}

// ------------------------------------------- Settings scale slider (20 Oct)

/// dragging the Settings scale slider shows the pending scale and the
/// workspace size but rescales nothing until release; Reset to 100 % and
/// Ctrl+= / Ctrl+- / Ctrl+0 still work
#[test]
fn scale_slider_applies_on_release_only() {
    use egui::accesskit::Role;
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().view = View::Settings;
    h.state_mut().set_open = Some(1);
    frames(&mut h, 4);
    let ppp0 = h.ctx.pixels_per_point();
    assert!((h.state().zoom - 1.0).abs() < 0.001);
    assert!(h.state().max_zoom >= 1.5, "max {}", h.state().max_zoom);
    let reset = h.get_by_label("Reset to 100 %").rect();
    let slider = h
        .query_all_by(|n| n.role() == Role::Slider)
        .map(|n| n.rect())
        .filter(|r| (r.center().y - reset.center().y).abs() < 12.0 && r.right() <= reset.left())
        .max_by(|a, b| a.left().total_cmp(&b.left()))
        .expect("the scale slider");
    let y = slider.center().y;
    let from = Pos2::new(slider.left() + 4.0, y);
    // part way up (at the top end the Reset button scrolls out of the
    // harness window)
    let to = Pos2::new(slider.left() + slider.width() * 0.7, y);
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
    for k in 1..=8 {
        let p = from + (to - from) * (k as f32 / 8.0);
        ev(&mut h, egui::Event::PointerMoved(p));
        assert_eq!(h.ctx.pixels_per_point(), ppp0, "no rescale mid-drag");
        assert!((h.state().zoom - 1.0).abs() < 0.001);
    }
    let pending = h.state().zoom_drag.expect("a pending scale");
    assert!(pending > 1.0, "dragged up to {pending}");
    assert!(
        h.query_all_by_label_contains("at this scale")
            .next()
            .is_some(),
        "the pending scale and workspace size show"
    );
    let shown = format!("{:.0} %", pending * 100.0);
    assert!(
        h.query_all_by_label_contains(&shown).next().is_some(),
        "{shown}"
    );
    ev(
        &mut h,
        egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    );
    frames(&mut h, 3);
    assert!(h.state().zoom_drag.is_none());
    assert!(
        (h.state().zoom - pending).abs() < 0.001,
        "the release applies it"
    );
    assert!(h.ctx.pixels_per_point() > ppp0 + 0.1, "rescaled on release");
    // Reset to 100 %
    frames(&mut h, 2);
    let c = h.get_by_label("Reset to 100 %").rect().center();
    click_at(&mut h, Pos2::new(c.x, c.y));
    frames(&mut h, 3);
    assert!((h.state().zoom - 1.0).abs() < 0.001);
    assert!((h.ctx.pixels_per_point() - ppp0).abs() < 0.001);
    // the keyboard
    key(&mut h, egui::Key::Equals, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(
        (h.state().zoom - 1.25).abs() < 0.001,
        "Ctrl+= {}",
        h.state().zoom
    );
    key(&mut h, egui::Key::Minus, egui::Modifiers::COMMAND);
    key(&mut h, egui::Key::Minus, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(
        (h.state().zoom - 0.75).abs() < 0.001,
        "Ctrl+- {}",
        h.state().zoom
    );
    key(&mut h, egui::Key::Num0, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!((h.state().zoom - 1.0).abs() < 0.001, "Ctrl+0");
}

/// Add › lists the built-in instruments by name (WaveForge and Drum Kit
/// first) and Browse…; a pick makes the instrument and gives it to the
/// track, one undo step; they are disabled when the 255 slots are used
#[test]
fn add_a_built_in_instrument_from_the_track_pie() {
    use super::trackpie::Builtin;
    use super::trackring::{ring_slots, Go, Level};
    // every built-in has something to start from
    for b in Builtin::ALL {
        let (name, kind) = b.make().unwrap_or_else(|| panic!("{b:?}"));
        assert!(!name.is_empty());
        if b == Builtin::WaveForge {
            assert!(
                matches!(&kind, InstrumentKind::Gen(g) if matches!(g.engine, crate::gen::GenEngine::Poly(_)))
            );
        }
        if b == Builtin::StringWave {
            assert!(
                matches!(&kind, InstrumentKind::Gen(g) if matches!(g.engine, crate::gen::GenEngine::Strings(_)))
            );
        }
        if b == Builtin::DrumKit {
            assert!(
                matches!(&kind, InstrumentKind::Gen(g) if matches!(g.engine, crate::gen::GenEngine::Drums(_)))
            );
        }
    }
    let mut info = super::trackpie::TrackInfo {
        tracks: 4,
        timeline: true,
        ..Default::default()
    };
    let items = ring_slots(Level::Instrument, &info);
    let last = items.len() - 1;
    assert_eq!(items[0].label, "Back");
    assert_eq!(items[1].label, crate::poly::POLY_DISPLAY_NAME);
    assert_eq!(items[2].label, "Drum Kit");
    assert_eq!(items[last].label, "Browse…");
    assert_eq!(items[last].go, Some(Go::Act(PieAct::ChangeInstrument)));
    assert_eq!(
        items[1].go,
        Some(Go::Act(PieAct::NewInstrument(Builtin::WaveForge)))
    );
    info.instr_full = true;
    let items = ring_slots(Level::Instrument, &info);
    assert!(items[1..last]
        .iter()
        .all(|i| i.go.is_none() && i.tip.contains("255")));
    // with the pointer: Add › Instrument › Drum Kit on track 01 of the demo
    let mut h = pie_harness(Pos2::new(600.0, 400.0));
    h.state_mut().extra.reduce_motion = true;
    h.state_mut().undo_reset();
    let add = SEGS.iter().position(|s| *s == Seg::Add).unwrap();
    let centre = h.state().pie.as_ref().unwrap().centre;
    for r in [60.0, 90.0, 110.0] {
        mv(
            &mut h,
            centre + egui::vec2(seg_angle(add).cos(), seg_angle(add).sin()) * r,
        );
        frames(&mut h, 3);
    }
    let ring = |h: &Harness<'static, App>| h.state().ring_now().map(|r| r.1);
    assert_eq!(ring(&h), Some(Level::Add), "Add opens its ring");
    let p = super::trackring_tests::ring_pos(&h, 8);
    mv(&mut h, p);
    click_at(&mut h, p);
    frames(&mut h, 3);
    assert_eq!(ring(&h), Some(Level::Instrument));
    let target = super::trackring_tests::ring_pos(&h, 2);
    mv(&mut h, target);
    let before = h.state().with(|s| {
        (
            s.song.instruments.len(),
            s.song.timeline.as_deref().unwrap().tracks[0].instr,
        )
    });
    click_at(&mut h, target);
    frames(&mut h, 10);
    assert!(h.state().pie.is_none(), "a pick closes the pie");
    let (n, slot, kind, notes_ok) = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let slot = t.tracks[0].instr;
        let kind = s.song.instruments[slot as usize - 1].kind.clone();
        // the track's notes play it now
        let ok = t
            .clips
            .iter()
            .filter(|c| c.track == 0)
            .all(|c| match c.kind {
                tl::ClipKind::Notes { data } => {
                    t.notes[data as usize].notes.iter().all(|n| n.instr == slot)
                }
                _ => true,
            });
        (s.song.instruments.len(), slot, kind, ok)
    });
    assert_eq!(n, before.0 + 1, "one new instrument");
    assert_eq!(slot as usize, n, "assigned to the track");
    assert!(
        matches!(kind, InstrumentKind::Gen(ref g) if matches!(g.engine, crate::gen::GenEngine::Drums(_)))
    );
    assert!(notes_ok);
    assert!(
        h.state().status.contains("Track 01 now plays"),
        "{}",
        h.state().status
    );
    // one undo step back
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 3);
    let after = h.state().with(|s| {
        (
            s.song.instruments.len(),
            s.song.timeline.as_deref().unwrap().tracks[0].instr,
        )
    });
    assert_eq!(after, before);
}
