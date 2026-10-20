//! The track pie menu (app/trackpie.rs): icon round-trip, the actions and
//! their undo, edge clamping, hit testing and the motion maths
use super::trackpie::*;
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline::TrackIcon;
use egui::Key;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn song<R>(h: &Harness<'static, App>, f: impl FnOnce(&Song) -> R) -> R {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    f(&g.song)
}

fn tl_demo() -> Song {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    s
}

fn harness_tl() -> Harness<'static, App> {
    let mut h = harness_with(tl_demo());
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    h
}

fn open(h: &mut Harness<'static, App>, tr: usize) {
    let ctx = h.ctx.clone();
    h.state_mut()
        .open_track_pie(tr, Pos2::new(600.0, 400.0), false, &ctx);
    frames(h, 2);
}

fn apply(h: &mut Harness<'static, App>, tr: usize, a: PieAct) {
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(tr, a, &ctx);
    frames(h, 2);
}

fn undo(h: &mut Harness<'static, App>) {
    h.state_mut().undo(false);
    frames(h, 2);
}

fn render_hash(s: &Song) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    for x in crate::engine::render_song(s, 11025) {
        x.to_bits().hash(&mut hs);
    }
    hs.finish()
}

#[test]
fn icon_round_trips_old_songs_read_auto_and_play_the_same() {
    let mut s = tl_demo();
    s.timeline.as_deref_mut().unwrap().tracks[1].icon = TrackIcon::Guitar;
    let dir = std::env::temp_dir().join(format!("cw-trackpie-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("icon.cwt");
    fileio::save_cwt(&s, &p).unwrap();
    let back = fileio::load_cwt(&p).unwrap();
    let tracks = &back.timeline.as_deref().unwrap().tracks;
    assert_eq!(tracks[1].icon, TrackIcon::Guitar);
    assert!(tracks
        .iter()
        .enumerate()
        .all(|(i, t)| i == 1 || t.icon == TrackIcon::Auto));
    // display only: the engine ignores it
    let mut plain = back.clone();
    plain.timeline.as_deref_mut().unwrap().tracks[1].icon = TrackIcon::Auto;
    assert_eq!(
        render_hash(&back),
        render_hash(&plain),
        "bit-identical with an icon set"
    );
    // Auto is not written, so songs without icons save exactly as before
    let t = toml::to_string(&crate::timeline::Track::default()).unwrap();
    assert!(!t.contains("icon"), "{t}");
    let t = toml::to_string(&tracks[1]).unwrap();
    assert!(t.contains("icon = \"guitar\""), "{t}");
    // an old track (no key) and an unknown icon from a newer version: Auto
    let old: crate::timeline::Track = toml::from_str("name = \"Bass\"").unwrap();
    assert_eq!(old.icon, TrackIcon::Auto);
    let newer: crate::timeline::Track =
        toml::from_str("name = \"X\"\nicon = \"theremin\"").unwrap();
    assert_eq!(newer.icon, TrackIcon::Auto);
    for i in TrackIcon::ALL {
        assert_eq!(TrackIcon::from_key(i.key()), i);
        assert!(super::glyphs::icon_glyph(i).is_some());
    }
    assert_eq!(TrackIcon::ALL.len(), 16);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn easing_and_open_progress() {
    assert_eq!(ease_out(0.0), 0.0);
    assert_eq!(ease_out(1.0), 1.0);
    assert!((ease_out(0.5) - 0.875).abs() < 1e-6, "cubic ease-out");
    assert_eq!(ease_in(0.5), 0.125);
    let mut last = -1.0;
    for k in 0..=100 {
        let v = ease_out(k as f32 / 100.0);
        assert!(v >= last, "monotone");
        last = v;
    }
    assert_eq!(ease_out(-1.0), 0.0);
    assert_eq!(ease_out(2.0), 1.0);
    // 160-200 ms open, staggered around the ring
    assert!((0.16..=0.2).contains(&OPEN_S));
    assert_eq!(CLOSE_S, 0.12);
    assert_eq!(HOVER_S, 0.08);
    assert_eq!(seg_progress(0.0, 0, false), 0.0);
    let mid = OPEN_S / 2.0;
    for i in 1..SEGS.len() {
        assert!(
            seg_progress(mid, i, false) < seg_progress(mid, i - 1, false),
            "stagger"
        );
    }
    let end = OPEN_S + (SEGS.len() - 1) as f64 * STAGGER_S;
    assert!(SEGS.len() as f64 * STAGGER_S < 0.12, "a few ms per segment");
    for i in 0..SEGS.len() {
        assert_eq!(seg_progress(end, i, false), 1.0);
    }
    assert!(!open_done(end - 0.01, false) && open_done(end, false));
    assert_eq!(scale_of(0.0), 0.6);
    assert_eq!(scale_of(1.0), 1.0);
    assert_eq!(disc_progress(OPEN_S, false), 1.0);
    assert_eq!(close_progress(0.0, false), 1.0);
    assert_eq!(close_progress(CLOSE_S, false), 0.0);
    // Reduce motion: no animation
    assert_eq!(seg_progress(0.0, 8, true), 1.0);
    assert!(open_done(0.0, true));
    assert_eq!(close_progress(0.0, true), 0.0);
}

#[test]
fn the_pie_stays_on_screen_and_hits_its_segments() {
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 900.0));
    let r = R_OUT + MARGIN;
    for p in [
        Pos2::new(0.0, 0.0),
        Pos2::new(1600.0, 900.0),
        Pos2::new(5.0, 880.0),
        Pos2::new(1590.0, 3.0),
    ] {
        let c = clamp_centre(p, screen);
        assert!(c.x - r >= -0.01 && c.x + r <= 1600.01, "{p:?} -> {c:?}");
        assert!(c.y - r >= -0.01 && c.y + r <= 900.01, "{p:?} -> {c:?}");
    }
    let mid = Pos2::new(800.0, 450.0);
    assert_eq!(clamp_centre(mid, screen), mid);
    let tiny = Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 200.0));
    assert_eq!(
        clamp_centre(Pos2::new(3.0, 3.0), tiny),
        Pos2::new(100.0, 100.0)
    );
    // hit testing: centre, every segment's middle, outside
    let c = Pos2::new(500.0, 500.0);
    assert_eq!(hit(c, c), Hit::Centre);
    for (i, seg) in SEGS.iter().enumerate() {
        let a = seg_angle(i);
        let p = c + egui::vec2(a.cos(), a.sin()) * (R_IN + R_OUT) / 2.0;
        assert_eq!(hit(c, p), Hit::Seg(i), "{seg:?}");
    }
    assert_eq!(SEGS[0], Seg::Rename, "Rename on top");
    assert_eq!(hit(c, c + egui::vec2(0.0, -(R_OUT + 20.0))), Hit::Outside);
    // submenus land on screen, outside the ring
    let info = TrackInfo {
        timeline: true,
        tracks: 4,
        ..Default::default()
    };
    for (i, seg) in SEGS.iter().enumerate().filter(|(_, s)| s.has_sub()) {
        let items = sub_items(*seg, &info);
        let (size, rects) = sub_layout(*seg, &items);
        for q in &rects {
            assert!(
                Rect::from_min_size(Pos2::ZERO, size).contains_rect(*q),
                "{seg:?}"
            );
        }
        for at in [Pos2::new(170.0, 170.0), Pos2::new(1430.0, 730.0), mid] {
            let cc = clamp_centre(at, screen);
            let sr = sub_rect(cc, i, size, screen);
            assert!(screen.contains_rect(sr), "{seg:?} at {at:?}: {sr:?}");
        }
    }
}

#[test]
fn submenu_entries_map_to_real_actions() {
    let mut info = TrackInfo {
        index: 0,
        tracks: 4,
        timeline: true,
        ..Default::default()
    };
    let icons = sub_items(Seg::Icon, &info);
    assert_eq!(icons.len(), 17, "Auto + 16 icons");
    assert!(icons[0].header && icons[0].on);
    assert_eq!(icons[7].act, Some(PieAct::Icon(TrackIcon::Guitar)));
    let org = sub_items(Seg::Organize, &info);
    assert_eq!(org[0].act, None, "the first track can't move up");
    assert_eq!(org[1].act, Some(PieAct::Move(MoveTo::Down)));
    let add = sub_items(Seg::Add, &info);
    let acts: Vec<_> = add.iter().map(|i| i.act).collect();
    assert!(acts.contains(&Some(PieAct::AddInstrument)));
    assert!(acts.contains(&Some(PieAct::AddAudio)));
    assert!(acts.contains(&Some(PieAct::Duplicate)));
    assert!(acts.contains(&None), "FX track: coming soon");
    assert!(
        acts.contains(&Some(PieAct::Automation)),
        "Automation lane ›"
    );
    info.full = true;
    assert!(sub_items(Seg::Add, &info)
        .iter()
        .all(|i| i.act.is_none() || i.act == Some(PieAct::Automation)));
    info.timeline = false;
    assert!(sub_items(Seg::Icon, &info).iter().all(|i| i.act.is_none()));
    assert!(sub_items(Seg::Colour, &info)
        .iter()
        .all(|i| i.act.is_none()));
    assert_eq!(Seg::Delete.act(), Some(PieAct::Delete));
    assert_eq!(Seg::SaveCpu.act(), None);
}

#[test]
fn keyboard_walks_the_ring_and_picks_an_icon_with_one_undo_step() {
    let mut h = harness_tl();
    open(&mut h, 1);
    assert!(h.state().pie_open());
    for _ in 0..3 {
        key(&mut h, Key::ArrowRight, egui::Modifiers::NONE);
    }
    assert_eq!(h.state().pie.as_ref().unwrap().key, Some(2), "Icon");
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    assert_eq!(h.state().pie.as_ref().unwrap().sub.map(|s| s.0), Some(2));
    // into the grid: Auto, then 7 to the right = Guitar
    for _ in 0..8 {
        key(&mut h, Key::ArrowRight, egui::Modifiers::NONE);
    }
    assert_eq!(h.state().pie.as_ref().unwrap().sub_key, Some(7));
    // Space would play behind an open pie; it must not
    key(&mut h, Key::Space, egui::Modifiers::NONE);
    assert!(!song_playing(&h));
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    frames(&mut h, 4);
    assert!(h.state().pie.is_none(), "closed after choosing");
    let icon =
        |h: &Harness<'static, App>| song(h, |s| s.timeline.as_deref().unwrap().tracks[1].icon);
    assert_eq!(icon(&h), TrackIcon::Guitar);
    undo(&mut h);
    assert_eq!(icon(&h), TrackIcon::Auto);
    // Esc closes a submenu first, then the pie
    open(&mut h, 1);
    let ctx = h.ctx.clone();
    h.state_mut().pie_choose(1, &ctx);
    assert!(h.state().pie.as_ref().unwrap().sub.is_some());
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().pie_open() && h.state().pie.as_ref().unwrap().sub.is_none());
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 3);
    assert!(h.state().pie.is_none());
}

fn song_playing(h: &Harness<'static, App>) -> bool {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    g.engine.playing
}

#[test]
fn delete_duplicate_move_colour_with_undo() {
    let mut h = harness_tl();
    let names = |h: &Harness<'static, App>| song(h, |s| s.channel_names.clone());
    let n0 = names(&h);
    let clips_on = |h: &Harness<'static, App>, tr: usize| {
        song(h, |s| {
            s.timeline
                .as_deref()
                .unwrap()
                .clips
                .iter()
                .filter(|c| c.track as usize == tr)
                .count()
        })
    };
    let c1 = clips_on(&h, 1);
    assert!(c1 > 0);
    // Delete
    apply(&mut h, 1, PieAct::Delete);
    assert_eq!(names(&h).len(), n0.len() - 1);
    assert!(!names(&h).contains(&n0[1]));
    assert_eq!(
        song(&h, |s| s.timeline.as_deref().unwrap().tracks.len()),
        n0.len(),
        "+ Song/Master"
    );
    undo(&mut h);
    assert_eq!(names(&h), n0);
    assert_eq!(clips_on(&h, 1), c1);
    // Duplicate: the copy below with its own clips and the same sound
    let before = song(&h, |s| s.clone());
    apply(&mut h, 1, PieAct::Duplicate);
    assert_eq!(names(&h).len(), n0.len() + 1);
    assert_eq!(names(&h)[2], format!("{} copy", n0[1].trim()));
    assert_eq!(clips_on(&h, 2), c1);
    let ids: std::collections::BTreeSet<u32> = song(&h, |s| {
        s.timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .map(|c| c.id)
            .collect()
    });
    assert_eq!(
        ids.len(),
        song(&h, |s| s.timeline.as_deref().unwrap().clips.len()),
        "unique ids"
    );
    undo(&mut h);
    assert_eq!(names(&h), n0);
    assert_eq!(
        render_hash(&song(&h, |s| s.clone())),
        render_hash(&before),
        "undo is exact"
    );
    // Organize: to the bottom and back
    apply(&mut h, 0, PieAct::Move(MoveTo::Bottom));
    assert_eq!(names(&h).last(), n0.first());
    undo(&mut h);
    assert_eq!(names(&h), n0);
    // Colour
    apply(&mut h, 2, PieAct::Colour(Some(0x123456)));
    assert_eq!(
        song(&h, |s| s.timeline.as_deref().unwrap().tracks[2].colour),
        Some(0x123456)
    );
    undo(&mut h);
    assert_eq!(
        song(&h, |s| s.timeline.as_deref().unwrap().tracks[2].colour),
        None
    );
    // Devices and Routing open their panels for the track
    apply(&mut h, 3, PieAct::Devices);
    assert!(h.state().devices_visible());
    assert_eq!(h.state().ch, 3);
    apply(&mut h, 2, PieAct::Routing);
    assert!(h.state().extra.inspector_open);
    assert_eq!(h.state().ch, 2);
}

#[test]
fn rename_inline_commits_with_enter_and_esc_cancels() {
    let mut h = harness_tl();
    apply(&mut h, 1, PieAct::Rename);
    assert!(h.state().track_rename.is_some());
    frames(&mut h, 2);
    h.input_mut().events.push(egui::Event::Text("Lead".into()));
    frame(&mut h);
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().track_rename.is_none());
    assert_eq!(song(&h, |s| s.channel_names[1].clone()), "Lead");
    assert_eq!(
        song(&h, |s| s.timeline.as_deref().unwrap().tracks[1]
            .name
            .clone()),
        "Lead"
    );
    undo(&mut h);
    assert_ne!(song(&h, |s| s.channel_names[1].clone()), "Lead");
    let was = song(&h, |s| s.channel_names[1].clone());
    apply(&mut h, 1, PieAct::Rename);
    frames(&mut h, 2);
    h.input_mut().events.push(egui::Event::Text("Nope".into()));
    frame(&mut h);
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().track_rename.is_none());
    assert_eq!(song(&h, |s| s.channel_names[1].clone()), was);
}

fn press(h: &mut Harness<'static, App>, p: Pos2, b: egui::PointerButton, down: bool) {
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: p,
        button: b,
        pressed: down,
        modifiers: egui::Modifiers::NONE,
    });
    frame(h);
}

#[test]
fn right_click_a_header_opens_it_and_drag_release_chooses() {
    let mut h = harness_tl();
    let lanes = h.state().tlv.lanes;
    let head = Pos2::new(lanes.left() - 150.0, lanes.top() + 70.0);
    // a click: opens and stays open
    press(&mut h, head, egui::PointerButton::Secondary, true);
    press(&mut h, head, egui::PointerButton::Secondary, false);
    frames(&mut h, 2);
    assert!(h.state().pie_open(), "right-click opens the pie");
    let tr = h.state().pie.as_ref().unwrap().track;
    let c = h.state().pie.as_ref().unwrap().centre;
    // the centre cancels
    press(&mut h, c, egui::PointerButton::Primary, true);
    press(&mut h, c, egui::PointerButton::Primary, false);
    frames(&mut h, 3);
    assert!(h.state().pie.is_none(), "the centre cancels");
    // press, drag onto Devices, release: chosen
    press(&mut h, head, egui::PointerButton::Secondary, true);
    frames(&mut h, 2);
    let c = h.state().pie.as_ref().unwrap().centre;
    let a = seg_angle(4);
    let on = c + egui::vec2(a.cos(), a.sin()) * (R_IN + R_OUT) / 2.0;
    h.input_mut().events.push(egui::Event::PointerMoved(on));
    frames(&mut h, 2);
    assert_eq!(
        h.state().pie.as_ref().unwrap().key,
        Some(4),
        "hover highlights"
    );
    press(&mut h, on, egui::PointerButton::Secondary, false);
    frames(&mut h, 3);
    assert!(h.state().pie.is_none());
    assert!(h.state().devices_visible());
    assert_eq!(h.state().ch, tr);
    // a click outside cancels; nothing behind reacts
    open(&mut h, 2);
    let n = song(&h, |s| s.channels);
    press(
        &mut h,
        Pos2::new(5.0, 5.0),
        egui::PointerButton::Primary,
        true,
    );
    press(
        &mut h,
        Pos2::new(5.0, 5.0),
        egui::PointerButton::Primary,
        false,
    );
    frames(&mut h, 3);
    assert!(h.state().pie.is_none());
    assert_eq!(song(&h, |s| s.channels), n);
}

#[test]
fn reduce_motion_closes_at_once_and_save_cpu_is_coming_soon() {
    let mut h = harness_tl();
    h.state_mut().extra.reduce_motion = true;
    open(&mut h, 1);
    let ctx = h.ctx.clone();
    h.state_mut().pie_choose(7, &ctx);
    assert!(h.state().pie_open(), "Save CPU does nothing yet");
    assert!(h.state().status.contains("coming soon"));
    h.state_mut().pie_close(&ctx);
    assert!(h.state().pie.is_none(), "no close animation");
}

#[test]
fn the_tracker_header_opens_it_too_and_shows_the_icon() {
    let mut s = tl_demo();
    s.timeline.as_deref_mut().unwrap().tracks[0].icon = TrackIcon::Brass;
    let mut h = harness_with(s);
    h.state_mut().view = View::Pattern;
    frames(&mut h, 3);
    let hdr = h.state().channel_headers(4);
    assert_eq!(hdr.glyphs[0], super::glyphs::Glyph::Brass);
    // a header's name area (left of its Play button)
    let (_, _, r) = *h
        .state()
        .hdr_hits
        .iter()
        .find(|(ch, op, _)| *ch == 1 && *op == super::pattern::HdrOp::Play)
        .unwrap();
    let p = Pos2::new(r.left() - 30.0, r.center().y);
    press(&mut h, p, egui::PointerButton::Secondary, true);
    press(&mut h, p, egui::PointerButton::Secondary, false);
    frames(&mut h, 2);
    assert!(h.state().pie_open());
    assert_eq!(h.state().pie.as_ref().unwrap().track, 1);
}
