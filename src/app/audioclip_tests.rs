//! Audio clips in the Timeline view (M4): dropping files on a track,
//! dragging the fade handles, the clip menu edits, tracker-mode drops.
use super::timeline_m2_tests::drag_m;
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline as tl;
use egui_kittest::Harness;

/// the demo in timeline mode on the Timeline view, 40 px a beat
fn setup(mode: bool) -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
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

fn tone_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("chipwave_m4ui_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    let data: Vec<f32> = (0..44100).map(|i| (i as f32 * 0.05).sin() * 0.4).collect();
    std::fs::write(&p, crate::audiofile::wav_f32(44100, 1, &data)).unwrap();
    p
}

/// a point in the lanes on `track`, at tick `t`
fn lane_point(h: &Harness<'static, App>, track: usize, t: tl::Tick) -> Pos2 {
    let tv = &h.state().tlv;
    let th = if tv.track_h > 0.0 { tv.track_h } else { 46.0 };
    Pos2::new(tv.x_of(t) + 3.0, tv.lanes.top() + (track as f32 + 0.5) * th)
}

#[derive(Debug)]
struct TestFile(std::path::PathBuf);

impl egui::DroppedFile for TestFile {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn drop_file(h: &mut Harness<'static, App>, at: Pos2, p: std::path::PathBuf) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    frame(h);
    h.input_mut()
        .dropped_files
        .push(std::sync::Arc::new(TestFile(p)));
    frame(h);
    // decoding runs on a worker thread
    for _ in 0..200 {
        if h.state().tlv.imports.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        frame(h);
    }
    frame(h);
}

fn audio_clips(h: &Harness<'static, App>) -> Vec<tl::Clip> {
    h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .map(|t| {
                t.clips
                    .iter()
                    .filter(|c| c.audio().is_some())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    })
}

#[test]
fn dropping_a_file_on_a_track_makes_an_audio_clip_there_and_undo_removes_it() {
    let mut h = setup(true);
    let f = tone_file("tone drop.wav");
    let at = lane_point(&h, 2, 4 * tl::PPQ);
    drop_file(&mut h, at, f);
    let v = audio_clips(&h);
    assert_eq!(v.len(), 1, "{}", h.state().status);
    let c = &v[0];
    assert_eq!(c.track, 2);
    assert_eq!(c.start % tl::ROW, 0, "on the grid");
    assert!(
        (c.start - 4 * tl::PPQ).abs() <= tl::PPQ,
        "near the drop: {}",
        c.start
    );
    assert!(
        h.state().status.contains("tone drop.wav"),
        "{}",
        h.state().status
    );
    let a = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().audio[0].clone());
    assert!(a.embed && a.bytes.is_some() && !a.missing());
    assert_eq!(h.state().tlv.sels, vec![c.id], "the new clip is selected");
    // one undo step takes it back
    frame(&mut h);
    h.state_mut().undo(false);
    frame(&mut h);
    assert!(audio_clips(&h).is_empty());
}

#[test]
fn the_fade_handles_drag_fades_in_and_out() {
    let mut h = setup(true);
    let at = lane_point(&h, 1, 8 * tl::PPQ);
    drop_file(&mut h, at, tone_file("fade.wav"));
    let c = audio_clips(&h)[0].clone();
    let r = h.state().tlv.clip_rect(c.id).expect("drawn");
    let hh = 12.0;
    // fade-out handle: top right corner, dragged two beats to the left
    let from = Pos2::new(r.right() - 3.0, r.top() + hh * 0.5);
    let to = from - egui::vec2(80.0, 0.0);
    drag_m(&mut h, from, to, egui::Modifiers::NONE);
    let c2 = audio_clips(&h)[0].clone();
    assert!(
        (c2.fade_out - 2 * tl::PPQ).abs() < tl::PPQ / 4,
        "fade out {} (status {})",
        c2.fade_out,
        h.state().status
    );
    assert_eq!((c2.start, c2.len), (c.start, c.len), "the clip stays put");
    // fade-in handle: top left, one beat to the right
    let from = Pos2::new(r.left() + 3.0, r.top() + hh * 0.5);
    drag_m(
        &mut h,
        from,
        from + egui::vec2(40.0, 0.0),
        egui::Modifiers::NONE,
    );
    let c3 = audio_clips(&h)[0].clone();
    assert!(
        (c3.fade_in - tl::PPQ).abs() < tl::PPQ / 4,
        "fade in {}",
        c3.fade_in
    );
    // the body still moves the clip
    let mid = Pos2::new(r.center().x, r.bottom() - 6.0);
    drag_m(
        &mut h,
        mid,
        mid + egui::vec2(160.0, 0.0),
        egui::Modifiers::NONE,
    );
    let c4 = audio_clips(&h)[0].clone();
    assert!(c4.start > c.start, "moved");
    assert_eq!((c4.fade_in, c4.fade_out), (c3.fade_in, c3.fade_out));
}

#[test]
fn clip_menu_edits_gain_shape_and_embedding() {
    let mut h = setup(true);
    let f = tone_file("menu.wav");
    let at = lane_point(&h, 0, 0);
    drop_file(&mut h, at, f.clone());
    let id = audio_clips(&h)[0].id;
    h.state_mut().tlv.sels = vec![id];
    h.state_mut()
        .audio_op(timeline::audioclip::AudioOp::Gain(0.5));
    h.state_mut()
        .audio_op(timeline::audioclip::AudioOp::Shape(tl::FadeShape::SCurve));
    let c = audio_clips(&h)[0].clone();
    assert_eq!((c.gain, c.fade_shape), (0.5, tl::FadeShape::SCurve));
    // reference the file, then store it again
    h.state_mut()
        .audio_op(timeline::audioclip::AudioOp::Embed(0, false));
    let a = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().audio[0].clone());
    assert!(!a.embed && a.bytes.is_none(), "{}", h.state().status);
    std::fs::remove_file(&f).unwrap();
    h.state_mut()
        .audio_op(timeline::audioclip::AudioOp::Embed(0, true));
    let a = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().audio[0].clone());
    assert!(!a.embed, "the file is gone: it can't be stored");
    assert!(
        h.state().status.contains("Can't read"),
        "{}",
        h.state().status
    );
    // the clip menu draws for an audio clip
    let r = h.state().tlv.clip_rect(id).unwrap();
    let p = Pos2::new(r.center().x, r.bottom() - 5.0);
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    frame(&mut h);
    use egui_kittest::kittest::Queryable;
    let _ = h.get_by_label("Store the file in the song");
}

#[test]
fn a_drop_in_tracker_mode_asks_to_switch_then_places_the_clip() {
    let mut h = setup(false);
    let at = lane_point(&h, 3, 0);
    drop_file(&mut h, at, tone_file("tracker.wav"));
    assert!(h.state().tlv.ask_switch.is_some(), "asks to switch");
    assert!(h.state().with(|s| s.song.timeline.is_none()));
    h.state_mut().answer_switch(true);
    frame(&mut h);
    let v = audio_clips(&h);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].track, 3);
}

#[test]
fn a_file_that_is_not_audio_reports_and_places_nothing() {
    let mut h = setup(true);
    let dir = std::env::temp_dir().join(format!("chipwave_m4ui_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("broken.mp3");
    std::fs::write(&p, b"this is not an mp3 file at all, just text").unwrap();
    let at = lane_point(&h, 0, 0);
    drop_file(&mut h, at, p);
    assert!(audio_clips(&h).is_empty());
    assert!(
        h.state().status.starts_with("Can't import broken.mp3"),
        "{}",
        h.state().status
    );
}
