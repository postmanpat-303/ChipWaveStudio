//! File > New song is a clean project: no tracks, no instruments, only
//! Song / Master; nothing breaks with zero tracks (docs/TIMELINE.md)
use super::ui_tests::{drag, frame, harness_with, key};
use super::*;
use egui::Key;
use egui_kittest::Harness;

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

fn clean() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().new_song();
    frames(&mut h, 3);
    h
}

#[test]
fn new_song_has_no_tracks_or_instruments_only_song_master() {
    let mut h = clean();
    assert_eq!(h.state().view, View::Timeline);
    let (ch, ni, np, tracks, bpm) = song(&mut h, |s| {
        let tl = s.timeline.as_ref().expect("timeline mode");
        (
            s.channels,
            s.instruments.len(),
            s.patterns
                .iter()
                .flat_map(|p| p.cells.iter().flatten())
                .filter(|c| c.note > 0)
                .count(),
            tl.tracks.iter().map(|t| t.kind).collect::<Vec<_>>(),
            s.bpm,
        )
    });
    assert_eq!((ch, ni, np), (0, 0, 0));
    assert_eq!(tracks, vec![crate::timeline::TrackKind::Master]);
    assert_eq!(bpm, Song::empty().bpm, "default tempo");
    assert!(h.state().status.contains("Browser"), "{}", h.state().status);
    assert!(!h.state().dirty);
    // Ctrl+N goes the same way (after the demo is replaced, nothing to save)
    key(&mut h, Key::N, egui::Modifiers::COMMAND);
    frames(&mut h, 3);
    assert_eq!(song(&mut h, |s| (s.channels, s.instruments.len())), (0, 0));
}

#[test]
fn every_view_panel_and_key_survives_zero_tracks() {
    let mut h = clean();
    // the Inspector, the lower panels and every bottom tab
    key(&mut h, Key::I, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().extra.inspector_open);
    for (tab, _, _) in super::viewbar::VTABS {
        h.state_mut().vtab_select(tab);
        frames(&mut h, 3);
        for k in [
            Key::ArrowDown,
            Key::ArrowRight,
            Key::Tab,
            Key::Z,
            Key::Delete,
            Key::Space,
            Key::Space,
        ] {
            key(&mut h, k, egui::Modifiers::NONE);
        }
        frames(&mut h, 2);
    }
    for (v, _, _) in super::topbar::VIEWS {
        h.state_mut().view = v;
        frames(&mut h, 3);
    }
    h.state_mut().view = View::Timeline;
    h.state_mut().set_lower(1);
    frames(&mut h, 3);
    h.state_mut().set_lower(0);
    frames(&mut h, 3);
    // export renders (silence) without a panic
    let s = song(&mut h, |s| s.clone());
    let _ = crate::engine::render_song(&s, 8000);
    // tracker copy / back to tracker mode gets one empty channel
    let t = crate::timeline::tracker_copy(&s).unwrap();
    assert_eq!(t.channels, 1);
    assert!(t.timeline.is_none());
    h.state_mut().leave_timeline();
    frames(&mut h, 2);
    assert_eq!(
        song(&mut h, |s| (s.channels, s.timeline.is_none())),
        (1, true)
    );
    h.state_mut().view = View::Pattern;
    frames(&mut h, 3);
    key(&mut h, Key::Z, egui::Modifiers::NONE);
    frames(&mut h, 2);
}

#[test]
fn browser_drop_makes_the_first_track_and_undo_takes_it_away() {
    let mut h = clean();
    h.state_mut().extra.browser_open = true;
    frames(&mut h, 3);
    h.state_mut().br.filter.tab = crate::browser::Tab::Instruments;
    h.state_mut().br.filter.query = "FM-4".into();
    frames(&mut h, 3);
    let (_, r) = h.state().br.row_rects.first().cloned().expect("FM-4");
    let lanes = h.state().tlv.lanes;
    let y = h.state().tlv.tracks_bottom() + 40.0;
    drag(&mut h, r.center(), Pos2::new(lanes.left() + 200.0, y));
    frames(&mut h, 3);
    let (ch, ni, kinds) = song(&mut h, |s| {
        (
            s.channels,
            s.instruments.len(),
            s.timeline
                .as_ref()
                .unwrap()
                .tracks
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!((ch, ni), (1, 1), "{}", h.state().status);
    assert_eq!(kinds.len(), 2);
    assert_eq!(
        kinds[1],
        crate::timeline::TrackKind::Master,
        "Song / Master stays last"
    );
    // the Inspector shows the new track
    key(&mut h, Key::I, egui::Modifiers::NONE);
    frames(&mut h, 3);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 3);
    assert_eq!(song(&mut h, |s| (s.channels, s.instruments.len())), (0, 0));
}

#[test]
fn add_track_on_a_clean_song() {
    use egui_kittest::kittest::Queryable;
    let mut h = clean();
    h.get_by_label("+ Add track").click();
    frames(&mut h, 3);
    let kinds = song(&mut h, |s| {
        s.timeline
            .as_ref()
            .unwrap()
            .tracks
            .iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>()
    });
    assert_eq!(song(&mut h, |s| s.channels), 1, "{}", h.state().status);
    assert_eq!(kinds.len(), 2);
    assert_eq!(kinds[1], crate::timeline::TrackKind::Master);
    assert!(
        h.state().status.contains("track 01"),
        "{}",
        h.state().status
    );
    h.state_mut().set_channels(3);
    frames(&mut h, 2);
    let kinds = song(&mut h, |s| {
        s.timeline
            .as_ref()
            .unwrap()
            .tracks
            .iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>()
    });
    assert_eq!(
        (kinds.len(), kinds[3]),
        (4, crate::timeline::TrackKind::Master)
    );
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
}

// ---- the default layout: Timeline on top, piano roll below (Ctrl+6)

#[test]
fn new_song_opens_the_timeline_with_the_piano_roll_below() {
    let mut h = clean();
    assert!(h.state().is_default_layout());
    assert!(h.state().pr.open && !h.state().extra.lower_inst);
    assert_eq!(h.state().extra.lower_h, 0.0, "default height (about 40 %)");
    // empty song: an empty piano roll with a hint, no target
    assert_eq!(h.state().pr_target(), None);
    assert!(h.state().pr.target.is_none());
    // the Inspector opens alongside
    key(&mut h, Key::I, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h.state().extra.inspector_open && h.state().is_default_layout());
}

#[test]
fn ctrl_6_view_bar_and_view_menu_bring_the_default_layout_back() {
    use egui_kittest::kittest::Queryable;
    let mut h = clean();
    key(&mut h, Key::Num3, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Mixer);
    key(&mut h, Key::Num6, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(h.state().is_default_layout(), "Ctrl+6");
    // the instrument editor, then View › Reset to default layout (the view
    // bar's TIMELINE + PIANO button is gone)
    h.state_mut().show_instrument_panel();
    frames(&mut h, 2);
    assert!(!h.state().is_default_layout());
    assert!(h
        .query_all_by(|n| n.label().as_deref() == Some("TIMELINE + PIANO"))
        .next()
        .is_none());
    key(&mut h, Key::Num6, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(h.state().is_default_layout(), "Ctrl+6 again");
    // a resized lower panel goes back to the default height
    h.state_mut().extra.lower_h = 600.0;
    h.state_mut().view = View::Fx;
    h.state_mut().reset_layout();
    assert!(h.state().is_default_layout() && h.state().extra.lower_h == 0.0);
}

#[test]
fn the_piano_roll_follows_the_selected_clip_or_the_tracks_first_clip() {
    use crate::app::pianoview::Target;
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().default_layout();
    frames(&mut h, 3);
    let clips = song(&mut h, |s| s.timeline.as_ref().unwrap().clips.clone());
    // nothing selected: track 1's first clip
    h.state_mut().ch = 1;
    frames(&mut h, 2);
    let first = clips
        .iter()
        .filter(|c| c.track == 1)
        .min_by_key(|c| c.start)
        .unwrap();
    let (p, col) = first.block().unwrap();
    assert_eq!(
        h.state().pr_target(),
        Some(Target::Block { pattern: p, col })
    );
    // a selected clip on another track wins
    let other = clips
        .iter()
        .filter(|c| c.track == 3)
        .max_by_key(|c| c.start)
        .unwrap();
    h.state_mut().tlv.sel = Some(other.id);
    frames(&mut h, 2);
    let (p, col) = other.block().unwrap();
    assert_eq!(h.state().pr.target, Some(Target::Block { pattern: p, col }));
}

#[test]
fn opening_a_timeline_song_shows_the_default_layout() {
    let dir = std::env::temp_dir().join(format!("cw-layout-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let p = dir.join("tl.cwt");
    fileio::save_cwt(&s, &p).unwrap();
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().view = View::Mixer;
    h.state_mut().open_path(p);
    frames(&mut h, 2);
    assert!(h.state().is_default_layout());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn startup_without_a_file_is_the_clean_song_not_a_demo() {
    // the song run_with() starts from: clean (timeline) or empty (tracker
    // setting); only --screenshot still starts from the Lumen Drift demo
    let s = super::startup_song(false, true);
    let tl = s.timeline.as_ref().expect("timeline mode");
    assert_eq!((s.channels, s.instruments.len()), (0, 0));
    assert_eq!(
        tl.tracks.iter().map(|t| t.kind).collect::<Vec<_>>(),
        vec![crate::timeline::TrackKind::Master]
    );
    assert_eq!(s.title, Song::clean().title);
    assert_ne!(s.title, fileio::demo_song().title);
    let e = super::startup_song(false, false);
    assert_eq!(e.instruments.len(), 0, "tracker setting: empty, no demo");
    assert_eq!(
        super::startup_song(true, true).title,
        fileio::demo_song().title,
        "screenshots load the demo explicitly"
    );
    // the startup step: File > New's layout, nothing to undo or save,
    // the audio line kept
    let mut h = harness_with(super::startup_song(false, true));
    h.state_mut().status = "Audio: test @ 48000 Hz.".into();
    h.state_mut().start_clean();
    frames(&mut h, 3);
    assert_eq!(h.state().view, View::Timeline);
    assert!(h.state().pr.open, "Timeline + piano roll layout");
    assert!(!h.state().dirty);
    assert!(!h.state().hist.can_undo());
    assert!(
        h.state().status.starts_with("Audio: test"),
        "{}",
        h.state().status
    );
    assert!(h.state().status.contains("Browser"), "{}", h.state().status);
    assert_eq!(song(&mut h, |s| (s.channels, s.instruments.len())), (0, 0));
}

/// Del (22 Oct 2026): the lower panel jumped between heights when going
/// from the Piano roll to Nested Device Chains; the tabs share one height,
/// and a dragged height survives File > New (only Ctrl+6 resets it)
#[test]
fn lower_panel_tabs_share_one_height_and_a_dragged_height_is_kept() {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().default_layout();
    frames(&mut h, 3);
    let top = |h: &Harness<'static, App>| h.state().ip.split.expect("splitter").top();
    let y0 = top(&h);
    for m in [4u8, 1, 3, 4, 3] {
        h.state_mut().set_lower(m);
        frames(&mut h, 2);
        assert_eq!(top(&h), y0, "automatic height, tab {m}");
    }
    h.state_mut().extra.lower_h = 333.0;
    frames(&mut h, 2);
    let y1 = top(&h);
    assert_ne!(y1, y0);
    for m in [4u8, 3, 1, 4] {
        h.state_mut().set_lower(m);
        frames(&mut h, 2);
        assert_eq!(top(&h), y1, "dragged height, tab {m}");
    }
    h.state_mut().new_song();
    frames(&mut h, 3);
    assert_eq!(h.state().extra.lower_h, 333.0, "File > New keeps it");
    assert_eq!(top(&h), y1);
    h.state_mut().reset_layout();
    assert_eq!(h.state().extra.lower_h, 0.0, "Ctrl+6 resets it");
}
