//! Automation lanes in the Timeline view (M5): unfolding a track, adding a
//! lane, double-clicking / dragging / drawing / bending points, clip lanes winning
//! inside their clip, undo, and tracker mode asking first.
use super::timeline::autolanes::{AutoOp, LaneRef, AUTO_H};
use super::timeline_m2_tests::{click_at, drag_m};
use super::ui_tests::{double_click, frame, harness_with, key};
use super::*;
use crate::automation::Target;
use crate::timeline as tl;
use egui_kittest::Harness;

fn setup(mode: bool) -> Harness<'static, App> {
    let mut song = fileio::demo_song();
    // design A migrates song lanes onto tracks on enter — clear them so these
    // UI tests start with empty track folds (playback bit-exact tests cover migrate)
    song.automation.clear();
    let mut h = harness_with(song);
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    if mode {
        h.state_mut().enter_timeline();
    }
    h.state_mut().undo_reset();
    h.state_mut().tlv.zoom = 40.0;
    h.state_mut().tlv.scroll = 0.0;
    frame(&mut h);
    frame(&mut h);
    h
}

fn timeline(h: &Harness<'static, App>) -> tl::Timeline {
    h.state()
        .with(|s| s.song.timeline.as_deref().cloned())
        .expect("timeline mode")
}

/// centre of track `tr`'s "A" (automation) button in its header
fn a_button(h: &Harness<'static, App>, tr: usize) -> Pos2 {
    let tv = &h.state().tlv;
    let y = tv.heads.top() + tv.layout.top[tr] - 0.0 + 1.0 + 4.0 + 8.5;
    Pos2::new(tv.heads.right() - 108.0 - 17.0 - 3.0 + 8.5, y)
}

/// a point in lane row `k` of track `tr` at tick `t` and value `v` (0..1)
fn lane_pt(h: &Harness<'static, App>, tr: usize, k: usize, t: tl::Tick, v: f32) -> Pos2 {
    let tv = &h.state().tlv;
    let top = tv.lanes.top() + tv.layout.lane_top(tr, k);
    let inner_top = top + 2.0;
    let inner_h = AUTO_H - 4.0;
    Pos2::new(tv.x_of(t), inner_top + inner_h - 3.0 - v * (inner_h - 6.0))
}

/// unfold track 1 and give it a volume lane
fn with_lane(h: &mut Harness<'static, App>) {
    let p = a_button(h, 1);
    click_at(h, p);
    assert_eq!(timeline(h).view.auto_open, 0b10, "track 2 unfolds");
    h.state_mut()
        .auto_op(AutoOp::AddLane(1, Target::ChanVolume(1)));
    frame(h);
    frame(h);
}

#[test]
fn the_a_button_unfolds_a_track_and_a_lane_can_be_added_and_undone() {
    let mut h = setup(true);
    let flat = h.state().tlv.layout.total();
    with_lane(&mut h);
    let t = timeline(&h);
    assert_eq!(t.tracks[1].auto.len(), 1);
    let l = &t.tracks[1].auto[0];
    assert_eq!(l.target, "mixer.ch.2.volume");
    assert!(l.label.contains("Volume"), "{}", l.label);
    assert_eq!(l.points.len(), 1, "starts flat at the fader");
    let lay = &h.state().tlv.layout;
    assert_eq!(lay.lanes[1], vec!["mixer.ch.2.volume".to_string()]);
    assert!(lay.total() > flat + AUTO_H, "one lane row and the strip");
    // tracks under it moved down
    assert!(lay.top[2] - lay.top[1] > AUTO_H);
    frame(&mut h);
    h.state_mut().undo(false);
    frame(&mut h);
    assert!(timeline(&h).tracks[1].auto.is_empty(), "undo removes it");
    // folding again
    let p = a_button(&h, 1);
    click_at(&mut h, p);
    assert_eq!(timeline(&h).view.auto_open, 0);
}

#[test]
fn clicking_adds_a_point_dragging_moves_it_and_clips_stay_put() {
    let mut h = setup(true);
    with_lane(&mut h);
    let clips = timeline(&h).clips.clone();
    let p = lane_pt(&h, 1, 0, 8 * tl::PPQ, 0.9);
    double_click(&mut h, p);
    let l = timeline(&h).tracks[1].auto[0].clone();
    assert_eq!(l.points.len(), 2, "{}", h.state().status);
    let q = l.points.iter().find(|p| p.pos > 0.0).unwrap();
    assert_eq!(q.pos, 32.0, "8 beats = row 32, on the row grid");
    assert!((q.value - 0.9).abs() < 0.04, "{}", q.value);
    // drag it later and lower
    let from = lane_pt(&h, 1, 0, 8 * tl::PPQ, q.value);
    let to = lane_pt(&h, 1, 0, 12 * tl::PPQ, 0.3);
    drag_m(&mut h, from, to, egui::Modifiers::NONE);
    let l = timeline(&h).tracks[1].auto[0].clone();
    assert_eq!(l.points.len(), 2);
    assert_eq!(l.points[1].pos, 48.0);
    assert!(
        (l.points[1].value - 0.3).abs() < 0.04,
        "{}",
        l.points[1].value
    );
    // nothing happened to the clips (the lane row is not a clip row)
    assert_eq!(timeline(&h).clips, clips);
    assert!(h.state().tlv.auto_drag.is_none());
    // double-click deletes the point
    let p = lane_pt(&h, 1, 0, 12 * tl::PPQ, 0.3);
    double_click(&mut h, p);
    let n = timeline(&h).tracks[1].auto[0].points.len();
    assert_eq!(n, 1, "{}", h.state().status);
    // the engine plays the lane
    let k = h.state().with(|s| {
        s.engine.ensure_plan(&s.song);
        let t = s.song.timeline.as_deref().unwrap();
        tl::build_auto(&s.song, t).layers.len()
    });
    assert_eq!(k, 1);
}

#[test]
fn ctrl_drag_draws_and_alt_drag_bends_a_segment() {
    let mut h = setup(true);
    with_lane(&mut h);
    let a = lane_pt(&h, 1, 0, 4 * tl::PPQ, 0.2);
    let b = lane_pt(&h, 1, 0, 12 * tl::PPQ, 0.8);
    drag_m(&mut h, a, b, egui::Modifiers::COMMAND);
    let l = timeline(&h).tracks[1].auto[0].clone();
    assert!(l.points.len() >= 6, "drawn: {:?}", l.points);
    assert!(
        l.points.windows(2).all(|w| w[0].pos <= w[1].pos),
        "in order"
    );
    // bend the segment between the last two points
    let n = l.points.len();
    let (p0, p1) = (l.points[n - 2], l.points[n - 1]);
    let mid_t = ((p0.pos + p1.pos) / 2.0 * tl::ROW as f32) as tl::Tick;
    let mid = lane_pt(&h, 1, 0, mid_t, (p0.value + p1.value) / 2.0);
    // the segment must be wide enough to grab between its points
    if (p1.pos - p0.pos) * tl::ROW as f32 * 40.0 / tl::PPQ as f32
        > 2.0 * crate::automation::POINT_HIT + 4.0
    {
        drag_m(
            &mut h,
            mid,
            mid - egui::vec2(0.0, 30.0),
            egui::Modifiers::ALT,
        );
        let l = timeline(&h).tracks[1].auto[0].clone();
        assert!(l.points[n - 2].curve != 0.0, "{:?}", l.points[n - 2]);
    }
    // a long segment to bend for sure
    h.state_mut()
        .auto_op(AutoOp::Clear(LaneRef::Track(1, Target::ChanVolume(1).id())));
    h.state_mut().auto_op(AutoOp::Add(
        LaneRef::Track(1, Target::ChanVolume(1).id()),
        64.0,
        1.0,
        None,
    ));
    frame(&mut h);
    let v0 = timeline(&h).tracks[1].auto[0].points[0].value;
    let mid = lane_pt(&h, 1, 0, 8 * tl::PPQ, (v0 + 1.0) / 2.0);
    drag_m(
        &mut h,
        mid,
        mid - egui::vec2(0.0, 30.0),
        egui::Modifiers::ALT,
    );
    let c = timeline(&h).tracks[1].auto[0].points[0].curve;
    let rising = v0 < 1.0;
    assert!(if rising { c > 0.2 } else { c < -0.2 }, "{c}");
}

#[test]
fn a_clip_lane_wins_inside_its_clip_and_edits_go_there() {
    let mut h = setup(true);
    with_lane(&mut h);
    let t = timeline(&h);
    let cl = t
        .clips
        .iter()
        .filter(|c| c.track == 1 && c.end() > 2 * tl::PPQ)
        .min_by_key(|c| c.start)
        .cloned()
        .unwrap();
    let tg = Target::ChanVolume(1).id();
    h.state_mut()
        .auto_op(AutoOp::AddClipLane(cl.id, tg.clone()));
    frame(&mut h);
    frame(&mut h);
    let c2 = timeline(&h)
        .clips
        .iter()
        .find(|c| c.id == cl.id)
        .unwrap()
        .clone();
    assert_eq!(c2.auto.len(), 1, "{}", h.state().status);
    // a double-click inside the clip edits the clip's lane, not the track's
    // (half way through its first pass: a looped clip wraps at loop_len)
    let at = (cl.start + cl.loop_len.unwrap_or(cl.len).min(cl.len) / 2) / tl::ROW * tl::ROW;
    let p = lane_pt(&h, 1, 0, at, 0.75);
    double_click(&mut h, p);
    let t = timeline(&h);
    let c3 = t.clips.iter().find(|c| c.id == cl.id).unwrap();
    assert_eq!(
        c3.auto[0].points.len(),
        2,
        "{} {:?} {:?} {:?}",
        h.state().status,
        t.tracks[1].auto,
        c3,
        h.state().tlv.layout
    );
    assert_eq!(
        t.tracks[1].auto[0].points.len(),
        1,
        "the track lane is untouched"
    );
    // the plan: inside the clip the clip lane plays
    let v = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let p = tl::build_auto(&s.song, t);
        p.value(0, at as f64 + 1.0)
    });
    assert!((v.unwrap() - 0.75).abs() < 0.05, "{v:?}");
    // moving the clip keeps its lane
    h.state_mut().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        tl::move_clips(t, &[cl.id], 4 * tl::PPQ, 0, 8);
    });
    frame(&mut h);
    assert_eq!(
        timeline(&h)
            .clips
            .iter()
            .find(|c| c.id == cl.id)
            .unwrap()
            .auto,
        c3.auto
    );
}

#[test]
fn in_tracker_mode_the_a_button_asks_to_switch() {
    let mut h = setup(false);
    let p = a_button(&h, 1);
    click_at(&mut h, p);
    assert!(h.state().tlv.ask_switch.is_some());
    assert!(h.state().with(|s| s.song.timeline.is_none()));
}

#[test]
fn add_lane_from_a_knob_goes_on_the_track_and_keeps_the_instrument_panel() {
    let mut h = setup(true);
    h.state_mut().extra.lower_inst = true;
    // mixer volume for channel 1 (= track 1)
    h.state_mut().add_lane(Target::ChanVolume(1));
    frame(&mut h);
    let t = timeline(&h);
    assert_eq!(t.tracks[1].auto.len(), 1, "{}", h.state().status);
    assert_eq!(t.tracks[1].auto[0].target, "mixer.ch.2.volume");
    assert!(t.view.auto_open & 0b10 != 0, "track unfolds");
    assert!(h.state().extra.lower_inst, "instrument editor stays open");
    assert!(
        h.state().with(|s| {
            !s.song
                .automation
                .iter()
                .any(|l| l.target == "mixer.ch.2.volume")
        }),
        "volume went on the track, not a song lane"
    );
    // second call focuses, does not duplicate
    h.state_mut().add_lane(Target::ChanVolume(1));
    assert_eq!(timeline(&h).tracks[1].auto.len(), 1);
}

#[test]
fn lane_colour_follows_the_parameter_not_the_index() {
    let mut h = setup(true);
    h.state_mut()
        .auto_op(AutoOp::AddLane(1, Target::ChanVolume(1)));
    h.state_mut()
        .auto_op(AutoOp::AddLane(1, Target::ChanPan(1)));
    frame(&mut h);
    let t = timeline(&h);
    assert_eq!(t.tracks[1].auto.len(), 2);
    let v = crate::automation::Target::ChanVolume(1).family().colour();
    let p = crate::automation::Target::ChanPan(1).family().colour();
    assert_ne!(v, p, "volume and pan have different colours");
    assert_eq!(v, 0x6FD07A);
    assert_eq!(p, 0xE0A060);
}

#[test]
fn follow_last_touched_opens_the_track_lane() {
    let mut h = setup(true);
    h.state_mut().tlv.follow_touch = true;
    h.state_mut().touch_param(Target::ChanVolume(1));
    frame(&mut h);
    let t = timeline(&h);
    assert!(t.view.auto_open & 0b10 != 0);
    assert_eq!(t.tracks[1].auto.len(), 1);
    assert_eq!(h.state().tlv.last_touch, Some(Target::ChanVolume(1)));
}

#[test]
fn lane_led_is_some_when_a_track_lane_exists() {
    let mut h = setup(true);
    assert!(h.state().lane_led(Target::ChanVolume(1)).is_none());
    h.state_mut()
        .auto_op(AutoOp::AddLane(1, Target::ChanVolume(1)));
    let led = h.state().lane_led(Target::ChanVolume(1));
    assert!(led.is_some(), "LED after adding a lane");
}
