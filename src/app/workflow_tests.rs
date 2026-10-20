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
