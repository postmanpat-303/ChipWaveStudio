//! Browser › Drum Patterns and Files (docs/BROWSER.md)
use super::ui_tests::{drag, frame, harness_with};
use super::*;
use crate::browser::Tab;
use crate::drumpat::PATTERNS;
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

fn tl_harness() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    let (tl, _) = crate::timeline::from_order(&s);
    s.timeline = Some(Box::new(tl));
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.browser_open = true;
    h.state_mut().set_channels(4);
    frames(&mut h, 4);
    h
}

/// open the style folder of pattern `name` and return its row centre
fn pattern_row(h: &mut Harness<'static, App>, name: &str) -> (usize, Pos2) {
    let i = PATTERNS.iter().position(|p| p.name == name).unwrap();
    h.state_mut().br.filter.tab = Tab::Patterns;
    h.state_mut().br.pat.folder = Some(PATTERNS[i].style);
    frames(h, 3);
    let r = h
        .state()
        .br
        .pat
        .rows
        .iter()
        .find(|x| x.0 == i)
        .map(|x| x.1)
        .expect("row drawn");
    (i, r.center())
}

#[test]
fn styles_list_and_folders_show_the_patterns() {
    let mut h = tl_harness();
    h.state_mut().br.filter.tab = Tab::Patterns;
    frames(&mut h, 3);
    assert!(h.state().br.pat.rows.is_empty(), "the root lists styles");
    let (i, _) = pattern_row(&mut h, "Warehouse Four");
    let n = h.state().br.pat.rows.len();
    assert_eq!(n, crate::drumpat::in_style(PATTERNS[i].style).count());
    // search goes across every style
    h.state_mut().br.filter.query = "lo-fi kit".into();
    frames(&mut h, 3);
    let rows = h.state().br.pat.rows.clone();
    assert!(rows.len() >= 4);
    assert!(rows.iter().any(|r| PATTERNS[r.0].kit == "Lo-fi Kit"));
    assert!(rows.iter().all(|r| {
        let p = &PATTERNS[r.0];
        p.kit == "Lo-fi Kit" || crate::drumpat::STYLES[p.style].contains("Lo-fi")
    }));
}

#[test]
fn drop_below_the_last_track_makes_a_drum_track_with_the_kit_and_clip() {
    let mut h = tl_harness();
    let n0 = song(&mut h, |s| s.channels);
    let ni = song(&mut h, |s| s.instruments.len());
    let (i, p) = pattern_row(&mut h, "Full-on Drive");
    // over the lanes, below the last track, a bit into bar 3
    let lanes = h.state().tlv.lanes;
    let y = h.state().tlv.tracks_bottom() + 30.0;
    drag(&mut h, p, Pos2::new(lanes.left() + 200.0, y));
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1);
    assert_eq!(song(&mut h, |s| s.channel_names[n0].clone()), "Psy Kit");
    assert_eq!(song(&mut h, |s| s.instruments.len()), ni + 1);
    let pat = PATTERNS[i];
    let (start, len, notes, instr, kit) = song(&mut h, |s| {
        let t = s.timeline.as_deref().unwrap();
        let c = t
            .clips
            .iter()
            .find(|c| c.track as usize == n0)
            .expect("clip");
        let crate::timeline::ClipKind::Notes { data } = c.kind else {
            panic!()
        };
        let d = &t.notes[data as usize];
        (
            c.start,
            c.len,
            d.notes.len(),
            t.tracks[n0].instr,
            s.instruments.last().unwrap().name.clone(),
        )
    });
    assert_eq!(start % (4 * crate::timeline::PPQ), 0, "snapped to a bar");
    assert_eq!(len, pat.len());
    assert_eq!(notes, pat.hits());
    assert_eq!(instr as usize, ni + 1);
    assert_eq!(kit, "Psy Kit");
    // one undo step takes it all back
    super::ui_tests::key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channels), n0);
    assert_eq!(song(&mut h, |s| s.instruments.len()), ni);
}

#[test]
fn drop_on_a_drum_track_places_the_clip_and_reuses_its_kit() {
    let mut h = tl_harness();
    let n0 = song(&mut h, |s| s.channels);
    h.state_mut().pat_place(0, None, Some(0));
    frames(&mut h, 2);
    let ni = song(&mut h, |s| s.instruments.len());
    // a second pattern onto that track, at bar 5
    let at = 16 * crate::timeline::PPQ + 100;
    let j = PATTERNS
        .iter()
        .position(|p| p.kit != PATTERNS[0].kit)
        .unwrap();
    h.state_mut().pat_place(j, Some(n0), Some(at));
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1, "no new track");
    assert_eq!(
        song(&mut h, |s| s.instruments.len()),
        ni,
        "the track's kit is reused"
    );
    let starts: Vec<i64> = song(&mut h, |s| {
        s.timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .filter(|c| c.track as usize == n0)
            .map(|c| c.start)
            .collect()
    });
    assert_eq!(starts, vec![0, 16 * crate::timeline::PPQ]);
}

#[test]
fn a_tracker_song_asks_to_switch_first() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().extra.browser_open = true;
    frames(&mut h, 2);
    h.state_mut().pat_place(3, None, Some(0));
    frames(&mut h, 1);
    assert!(h.state().tlv.ask_switch.is_some());
    assert!(song(&mut h, |s| s.timeline.is_none()));
    h.state_mut().answer_switch(true);
    frames(&mut h, 2);
    let n = song(&mut h, |s| {
        s.timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .filter(|c| matches!(c.kind, crate::timeline::ClipKind::Notes { .. }))
            .count()
    });
    assert_eq!(n, 1);
}

#[test]
fn play_button_previews_and_esc_or_another_click_stops() {
    let mut h = tl_harness();
    let (i, p) = pattern_row(&mut h, "Classic House");
    h.state_mut().extra.browser_autoplay = false;
    // the step icon at the left of the row is the Play button
    let r = h.state().br.pat.rows.iter().find(|x| x.0 == i).unwrap().1;
    let ic = Pos2::new(r.left() + 20.0, p.y);
    super::ui_tests::click_at(&mut h, ic);
    frames(&mut h, 2);
    assert_eq!(h.state().br.pat.playing, Some(i));
    assert!(h.state().with(|s| s.engine.pattern_preview().is_some()));
    assert!(
        !h.state().with(|s| s.engine.playing),
        "the song isn't started"
    );
    super::ui_tests::click_at(&mut h, ic);
    frames(&mut h, 2);
    assert_eq!(h.state().br.pat.playing, None);
    assert!(h.state().with(|s| s.engine.pattern_preview().is_none()));
    // Esc (Browser has the keyboard) stops too
    h.state_mut().pat_toggle(i);
    h.state_mut().br.focus = true;
    let ctx = h.ctx.clone();
    h.state_mut()
        .br_key(egui::Key::Escape, Default::default(), &ctx);
    assert_eq!(h.state().br.pat.playing, None);
    // an instrument preview stops the pattern preview
    h.state_mut().pat_toggle(i);
    let it = crate::browser::builtin_items()
        .into_iter()
        .find(|x| x.name == "808 Boom")
        .unwrap();
    h.state_mut().br_preview(&it);
    assert_eq!(h.state().br.pat.playing, None);
}

#[test]
fn files_tab_navigates_folders_and_lists_loadable_files() {
    let d = std::env::temp_dir().join(format!("cw-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("Kicks")).unwrap();
    std::fs::write(d.join("loop.wav"), b"RIFF").unwrap();
    std::fs::write(d.join("notes.txt"), b"x").unwrap();
    let mut h = tl_harness();
    h.state_mut().br.filter.tab = Tab::Files;
    frames(&mut h, 2);
    h.state_mut().files_go(d.clone(), true);
    for _ in 0..100 {
        frame(&mut h);
        if !h.state().br.files.loading() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let names: Vec<String> = h
        .state()
        .br
        .files
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert_eq!(
        names,
        vec!["Kicks".to_string(), "loop.wav".to_string()],
        "folders first, only loadable files"
    );
    h.state_mut().files_go(d.join("Kicks"), true);
    frames(&mut h, 3);
    assert_eq!(h.state().br.files.back.last(), Some(&d));
    let _ = std::fs::remove_dir_all(&d);
}
