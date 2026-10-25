//! Del: "the track can be a bit bigger by default. scalable and moveable up
//! and down". Taller default rows, each track's own height (drag a header's
//! bottom edge, double-click resets, Alt / Ctrl+wheel resize every track),
//! and moving a track keeps everything that is the track's with it.
use super::timeline::{DEFAULT_TRACK_H, MAX_TRACK_H, MIN_TRACK_H};
use super::trackpie::{MoveTo, PieAct};
use super::ui_tests::{drag, frame, harness_with};
use super::*;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn tl_demo(timeline_mode: bool) -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    if timeline_mode {
        crate::timeline::enter_timeline_mode(&mut s);
    }
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    h
}

fn own_h(h: &Harness<'static, App>, tr: usize) -> f32 {
    h.state()
        .with(|s| s.song.timeline.as_ref().unwrap().tracks[tr].height)
}

/// screen point on track `tr`'s header bottom edge (the resize strip)
fn edge(h: &Harness<'static, App>, tr: usize) -> Pos2 {
    let (hr, _) = h.state().tlv.track_rects(tr).unwrap();
    Pos2::new(hr.left() + 60.0, hr.bottom() - 1.0)
}

fn wheel(h: &mut Harness<'static, App>, at: Pos2, m: egui::Modifiers, dy: f32) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    frame(h);
    h.input_mut().events.push(egui::Event::ModifiersChanged(m));
    h.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, dy),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: m,
    });
    frame(h);
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(Default::default()));
    frames(h, 2);
}

#[test]
fn rows_are_taller_by_default() {
    let h = tl_demo(true);
    const { assert!(DEFAULT_TRACK_H >= 46.0 * 1.25 && DEFAULT_TRACK_H <= 46.0 * 1.5) };
    let lay = &h.state().tlv.layout;
    assert_eq!(lay.h(0), DEFAULT_TRACK_H);
    assert_eq!(lay.top[1] - lay.top[0], DEFAULT_TRACK_H);
    let (hr, row) = h.state().tlv.track_rects(1).unwrap();
    assert_eq!(hr.height(), DEFAULT_TRACK_H);
    assert_eq!(row.height(), DEFAULT_TRACK_H);
}

#[test]
fn dragging_a_header_edge_sets_that_track_height_undo_and_double_click_reset() {
    let mut h = tl_demo(true);
    let top2 = h.state().tlv.layout.top[2];
    let at = edge(&h, 1);
    drag(&mut h, at, at + egui::vec2(0.0, 30.0));
    frames(&mut h, 2);
    let want = DEFAULT_TRACK_H + 30.0;
    assert_eq!(own_h(&h, 1), want, "{}", h.state().status);
    assert_eq!(h.state().tlv.layout.h(1), want);
    assert_eq!(
        h.state().tlv.layout.top[2],
        top2 + 30.0,
        "the rows below move down"
    );
    assert_eq!(own_h(&h, 0), 0.0, "the others keep the default");
    assert!(h.state().tlv.resize.is_none());
    // limits
    let at = edge(&h, 1);
    drag(&mut h, at, at - egui::vec2(0.0, 300.0));
    frames(&mut h, 2);
    assert_eq!(own_h(&h, 1), MIN_TRACK_H);
    // one undo step per drag
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(own_h(&h, 1), want);
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(own_h(&h, 1), 0.0);
    assert_eq!(h.state().tlv.layout.h(1), DEFAULT_TRACK_H);
    // double-click the edge: back to the default
    h.state_mut().set_track_height(3, 120.0);
    frames(&mut h, 2);
    assert_eq!(h.state().tlv.layout.h(3), 120.0);
    let at = edge(&h, 3);
    super::ui_tests::double_click(&mut h, at);
    frames(&mut h, 2);
    assert_eq!(own_h(&h, 3), 0.0);
    assert_eq!(h.state().tlv.layout.h(3), DEFAULT_TRACK_H);
}

#[test]
fn alt_or_ctrl_wheel_over_the_headers_resizes_every_track_and_is_saved() {
    let mut h = tl_demo(true);
    h.state_mut().set_track_height(2, 100.0);
    frames(&mut h, 2);
    let heads = h.state().tlv.heads;
    let z0 = h.state().tlv.zoom;
    wheel(&mut h, heads.center(), egui::Modifiers::COMMAND, 1.0);
    let d = h.state().tlv.track_h;
    assert!(d > DEFAULT_TRACK_H, "taller: {d}");
    assert_eq!(h.state().extra.track_h, d, "kept in the settings");
    assert_eq!(h.state().tlv.layout.h(0), d);
    assert!(own_h(&h, 2) > 100.0, "own heights scale along");
    assert_eq!(h.state().tlv.zoom, z0, "no time zoom over the headers");
    // Alt+wheel anywhere: shorter, within the limits
    let lanes = h.state().tlv.lanes;
    for _ in 0..30 {
        wheel(&mut h, lanes.center(), egui::Modifiers::ALT, -1.0);
    }
    assert_eq!(h.state().tlv.track_h, MIN_TRACK_H);
    assert!(own_h(&h, 2) >= MIN_TRACK_H);
    for _ in 0..40 {
        wheel(&mut h, lanes.center(), egui::Modifiers::ALT, 1.0);
    }
    assert_eq!(h.state().tlv.track_h, MAX_TRACK_H);
    // Ctrl+wheel over the lanes still zooms time, heights stay
    wheel(&mut h, lanes.center(), egui::Modifiers::COMMAND, 1.0);
    assert_eq!(h.state().tlv.track_h, MAX_TRACK_H);
    assert_ne!(h.state().tlv.zoom, z0);
}

#[test]
fn moving_a_track_takes_its_height_colour_mix_name_and_clips_and_undo_restores() {
    let mut h = tl_demo(true);
    h.state_mut().set_track_height(1, 90.0);
    let snap = |h: &Harness<'static, App>, tr: usize| {
        h.state().with(|s| {
            let t = s.song.timeline.as_ref().unwrap();
            let mut clips: Vec<_> = t
                .clips
                .iter()
                .filter(|c| c.track as usize == tr)
                .map(|c| {
                    (
                        c.start,
                        c.len, // a block clip's column is the channel's: it moves with it
                        format!("{:?}", c.kind)
                            .split(", col")
                            .next()
                            .unwrap()
                            .to_string(),
                    )
                })
                .collect();
            clips.sort_by_key(|c| c.0);
            let m = &s.song.mixer.channels[tr];
            (
                t.tracks[tr].height,
                t.tracks[tr].colour,
                s.song.channel_names[tr].clone(),
                (m.mute, m.volume.to_bits()),
                format!("{clips:?}"),
            )
        })
    };
    h.state_mut().with(|s| {
        s.song.mixer.channels[1].mute = true;
        s.song.mixer.channels[1].volume = 0.6;
        s.song.timeline.as_deref_mut().unwrap().tracks[1].colour = Some(0x123456);
    });
    frames(&mut h, 2);
    let t1 = snap(&h, 1);
    let t2 = snap(&h, 2);
    let before = h.state().with(|s| s.song.clone());
    // the track menu's Move down
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(1, PieAct::Move(MoveTo::Down), &ctx);
    frames(&mut h, 2);
    assert_eq!(snap(&h, 2), t1, "track 02 is now 03, with all of its own");
    assert_eq!(snap(&h, 1), t2);
    assert_eq!(h.state().tlv.layout.h(2), 90.0);
    assert_eq!(h.state().tlv.layout.h(1), DEFAULT_TRACK_H);
    // header drag back up (to the top)
    let (hr, _) = h.state().tlv.track_rects(2).unwrap();
    let (h0, _) = h.state().tlv.track_rects(0).unwrap();
    drag(
        &mut h,
        Pos2::new(hr.left() + 30.0, hr.center().y),
        Pos2::new(hr.left() + 30.0, h0.top() + 4.0),
    );
    frames(&mut h, 2);
    assert_eq!(snap(&h, 0), t1, "dragged to the top");
    // undo both moves: the song is as it was
    h.state_mut().undo(false);
    frames(&mut h, 2);
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(snap(&h, 1), t1);
    let after = h.state().with(|s| s.song.clone());
    assert_eq!(
        serde_json::to_string(&after).unwrap(),
        serde_json::to_string(&before).unwrap()
    );
}

#[test]
fn tracker_mode_heights_are_for_the_session_and_follow_a_move() {
    let mut h = tl_demo(false);
    h.state_mut().set_track_height(0, 80.0);
    frames(&mut h, 2);
    assert!(
        h.state().with(|s| s.song.timeline.is_none()),
        "the song is untouched"
    );
    assert_eq!(h.state().tlv.layout.h(0), 80.0);
    h.state_mut().track_move(0, 2);
    frames(&mut h, 2);
    assert_eq!(h.state().tlv.layout.h(2), 80.0);
    assert_eq!(h.state().tlv.layout.h(0), DEFAULT_TRACK_H);
}

#[test]
fn own_heights_are_saved_with_the_song_and_old_songs_save_as_before() {
    let dir = std::env::temp_dir().join(format!("cw-tracksize-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    // no own heights: the file has no height field (old songs are unchanged)
    let p0 = dir.join("plain.cwt");
    fileio::save_cwt(&s, &p0).unwrap();
    let raw = std::fs::read(&p0).unwrap();
    assert!(
        !String::from_utf8_lossy(&raw).contains("\"height\""),
        "no height key without own heights"
    );
    s.timeline.as_deref_mut().unwrap().tracks[3].height = 110.0;
    let p = dir.join("tall.cwt");
    fileio::save_cwt(&s, &p).unwrap();
    let back = fileio::load_cwt(&p).unwrap();
    let t = back.timeline.as_ref().unwrap();
    assert_eq!(t.tracks[3].height, 110.0);
    assert_eq!(t.tracks[2].height, 0.0);
    // and the view uses it
    let mut h = harness_with(back);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    assert_eq!(h.state().tlv.layout.h(3), 110.0);
    // the default height is a setting
    let e: crate::settings::Extra = toml::from_str("").unwrap_or_default();
    assert_eq!(e.track_h, 0.0, "old settings files: the built-in default");
    let _ = std::fs::remove_dir_all(&dir);
}
