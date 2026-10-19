//! Default is timeline mode: tracker-mode songs and demos open in timeline
//! mode (Settings › Open tracker songs in timeline mode, on by default),
//! play bit-identically, and opening is not an edit (docs/TIMELINE.md)
use super::ui_tests::{frame, harness_with};
use super::*;
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

/// a hash of the rendered samples (bit patterns), with the length
fn render_hash(s: &Song) -> (usize, u64) {
    use std::hash::{Hash, Hasher};
    let a = crate::engine::render_song(s, 11025);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for x in &a {
        x.to_bits().hash(&mut h);
    }
    (a.len(), h.finish())
}

#[test]
fn defaults_are_timeline_mode() {
    let e = crate::settings::Extra::default();
    assert!(e.new_in_timeline, "Settings › New songs: timeline");
    assert!(
        e.open_in_timeline,
        "Settings › Open tracker songs in timeline mode"
    );
    // an old settings file without the key keeps the default (on)
    let old: crate::settings::Extra = toml::from_str("").unwrap();
    assert!(old.open_in_timeline && old.new_in_timeline);
}

#[test]
fn every_demo_opens_in_timeline_mode_unedited_and_plays_identically() {
    // the tracker-mode originals, rendered in parallel
    let n = fileio::DEMOS.len();
    let want: Vec<(usize, u64)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..n)
            .map(|i| sc.spawn(move || render_hash(&fileio::demo_song_n(i))))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut h = harness_with(Song::clean());
    h.state_mut().extra.open_in_timeline = true;
    let mut loaded = vec![];
    for i in 0..n {
        assert!(
            fileio::demo_song_n(i).timeline.is_none(),
            "demo {i} is tracker mode"
        );
        h.state_mut().load_demo(i);
        frames(&mut h, 2);
        let st = h.state();
        assert!(
            song(&h, |s| s.timeline.is_some()),
            "demo {i}: timeline mode"
        );
        assert!(!st.dirty, "demo {i}: opening is not an edit");
        assert!(!st.hist.can_undo(), "demo {i}: no conversion step to undo");
        assert!(
            st.status.contains(super::file::OPENED_IN_TIMELINE),
            "{}",
            st.status
        );
        assert_eq!(st.view, View::Timeline, "default layout");
        assert!(st.tlv.ask_switch.is_none(), "no dialog");
        loaded.push(song(&h, |s| s.clone()));
    }
    let got: Vec<(usize, u64)> = std::thread::scope(|sc| {
        let hs: Vec<_> = loaded
            .iter()
            .map(|s| sc.spawn(move || render_hash(s)))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for i in 0..n {
        assert!(want[i].0 > 11025, "{}", fileio::DEMOS[i].0);
        assert_eq!(got[i], want[i], "{}: render hash", fileio::DEMOS[i].0);
    }
}

#[test]
fn a_tracker_cwt_opens_in_timeline_mode_and_the_file_is_untouched() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/chipwave-0.2.3.6-song.cwt");
    let dir = std::env::temp_dir().join(format!("cw-opentl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("old.cwt");
    std::fs::copy(&src, &p).unwrap();
    let before = std::fs::read(&p).unwrap();
    let mut h = harness_with(Song::clean());
    h.state_mut().extra.open_in_timeline = true;
    h.state_mut().open_path(p.clone());
    frames(&mut h, 2);
    assert!(song(&h, |s| s.timeline.is_some()));
    assert!(!h.state().dirty);
    assert!(!h.state().hist.can_undo());
    assert_eq!(h.state().path.as_deref(), Some(p.as_path()));
    assert!(
        h.state().status.contains("Opened in timeline mode"),
        "{}",
        h.state().status
    );
    assert_eq!(
        std::fs::read(&p).unwrap(),
        before,
        "nothing written on open"
    );
    // the tracker view still works on the converted song
    h.state_mut().view = View::Pattern;
    frames(&mut h, 3);
    assert_eq!(h.state().view, View::Pattern);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn setting_off_keeps_tracker_mode_and_the_dialog() {
    let mut h = harness_with(Song::clean());
    h.state_mut().extra.open_in_timeline = false;
    h.state_mut().load_demo(0);
    frames(&mut h, 2);
    assert!(song(&h, |s| s.timeline.is_none()), "stays in tracker mode");
    assert!(
        !h.state().status.contains("timeline mode"),
        "{}",
        h.state().status
    );
    assert!(!h.state().dirty);
    // as before: the timeline asks before the first timeline edit
    h.state_mut().tlv.ask_switch = Some(None);
    h.state_mut().answer_switch(true);
    assert!(song(&h, |s| s.timeline.is_some()));
    assert!(h.state().hist.can_undo() || h.state().dirty);
}

#[test]
fn convert_on_open_leaves_timeline_songs_and_off_alone() {
    let mut s = fileio::demo_song();
    assert!(!super::file::convert_on_open(&mut s, false));
    assert!(s.timeline.is_none());
    assert!(super::file::convert_on_open(&mut s, true));
    let t = s.clone();
    assert!(
        !super::file::convert_on_open(&mut s, true),
        "already timeline"
    );
    let clips = |s: &Song| s.timeline.as_ref().map(|t| (t.tracks.len(), t.clips.len()));
    assert_eq!(clips(&s), clips(&t));
    assert_eq!(s.automation.len(), t.automation.len());
}
