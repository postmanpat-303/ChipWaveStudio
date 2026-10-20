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

use super::trackpie::{hit, seg_angle, sub_items, sub_layout, sub_rect, Seg, SEGS};

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
