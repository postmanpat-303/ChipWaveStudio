//! Focus on the Timeline (20 Oct 2026): the playhead moves while the
//! timeline plays (every lower-panel tab, Clip Arranger on or off), chords
//! in the piano roll, the Tracker hidden by default, the save prompt.
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline as tl;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// a harness as Del runs it: the Tracker hidden (the default)
fn focus(song: Song) -> Harness<'static, App> {
    let mut h = harness_with(song);
    h.state_mut().extra.show_tracker = false;
    h.state_mut().extra.reduce_motion = true;
    frames(&mut h, 2);
    h
}

/// render the engine (no audio device in tests) for `rows` more rows
fn play_rows(h: &mut Harness<'static, App>, rows: u64) {
    h.state().with(|s| {
        let Shared { song, engine, .. } = s;
        let target = engine.rows_played + rows;
        let mut buf = vec![0.0f32; 128];
        let mut guard = 0;
        while engine.rows_played < target && engine.playing && guard < 200_000 {
            engine.render(song, &mut buf);
            guard += 1;
        }
    });
}

/// the demo in timeline mode with no automation at all (lanes used to be
/// the only thing that moved the playhead)
fn plain_demo() -> Song {
    let mut s = fileio::demo_song();
    tl::enter_timeline_mode(&mut s);
    s.automation.clear();
    s.modulation = Default::default();
    if let Some(t) = s.timeline.as_deref_mut() {
        for tr in &mut t.tracks {
            tr.auto.clear();
        }
        for c in &mut t.clips {
            c.auto.clear();
        }
    }
    s
}

/// File > New, one track, a note clip with a chord on it
fn new_song_with_notes() -> Harness<'static, App> {
    let mut h = focus(fileio::demo_song());
    h.state_mut().new_song();
    frames(&mut h, 3);
    h.get_by_label("+ Add track").click();
    frames(&mut h, 3);
    h.state().with(|s| {
        let id = tl::new_note_clip(&mut s.song, 0, 0).unwrap();
        let t = s.song.timeline.as_deref_mut().unwrap();
        let tl::ClipKind::Notes { data } = t.clips.iter().find(|c| c.id == id).unwrap().kind else {
            unreachable!()
        };
        let d = t.notes[data as usize].make_mut();
        for k in [49u8, 53, 56] {
            d.notes.push(tl::Note {
                start: 0,
                len: 8 * tl::PPQ,
                key: k,
                vel: 0xFFFF,
                ..Default::default()
            });
        }
        let Shared { song, engine, .. } = s;
        engine.ensure_plan(song);
    });
    frames(&mut h, 2);
    h
}

/// press Space, render `rows`, and return the playhead drawn before and after
fn heads_while_playing(h: &mut Harness<'static, App>) -> (Option<f64>, Option<f64>) {
    h.state().with(|s| s.engine.stop());
    frames(h, 2);
    key(h, Key::Space, egui::Modifiers::NONE);
    frames(h, 2);
    assert!(h.state().with(|s| s.engine.playing), "Space plays");
    play_rows(h, 6);
    frames(h, 2);
    let a = h.state().tlv.head_drawn;
    play_rows(h, 10);
    frames(h, 2);
    let b = h.state().tlv.head_drawn;
    h.state().with(|s| s.engine.stop());
    frames(h, 2);
    (a, b)
}

fn check_every_lower_tab(h: &mut Harness<'static, App>, what: &str) {
    use super::viewbar::VTab;
    for ca in [false, true] {
        h.state_mut().extra.clip_arranger = ca;
        for tab in [VTab::Piano, VTab::Devices, VTab::Inst, VTab::Auto] {
            h.state_mut().view = View::Timeline;
            h.state_mut().vtab_select(tab);
            frames(h, 3);
            assert_eq!(h.state().view, View::Timeline);
            assert_eq!(h.state().ca_active(), ca);
            let (a, b) = heads_while_playing(h);
            let (a, b) = (
                a.unwrap_or_else(|| panic!("{what} {tab:?} ca {ca}: no playhead")),
                b.unwrap(),
            );
            // 10 rows = 10 * ROW ticks further on
            let d = b - a;
            assert!(
                (d - 10.0 * tl::ROW as f64).abs() <= tl::ROW as f64,
                "{what} {tab:?} ca {ca}: playhead {a} -> {b}"
            );
        }
    }
}

#[test]
fn the_playhead_moves_while_the_timeline_plays_in_every_lower_tab() {
    let mut h = focus(plain_demo());
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    check_every_lower_tab(&mut h, "demo, no automation");
    let mut h = new_song_with_notes();
    check_every_lower_tab(&mut h, "new song");
    // the demo as shipped (with automation lanes) too
    let mut s = fileio::demo_song();
    tl::enter_timeline_mode(&mut s);
    let mut h = focus(s);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    check_every_lower_tab(&mut h, "demo");
}

#[test]
fn space_in_the_mixer_plays_the_timeline_not_a_block_loop() {
    let mut h = new_song_with_notes();
    h.state_mut().view = View::Mixer;
    frames(&mut h, 2);
    key(&mut h, Key::Space, egui::Modifiers::NONE);
    frames(&mut h, 2);
    let (playing, looping, tl_on) = h.state().with(|s| {
        (
            s.engine.playing,
            s.engine.loop_pattern,
            s.engine.tl_row().is_some(),
        )
    });
    assert!(playing && !looping && tl_on);
    play_rows(&mut h, 8);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 2);
    let a = h.state().tlv.head_drawn.expect("playhead");
    assert!(a >= 7.0 * tl::ROW as f64, "{a}");
    // Enter restarts from the play marker (no block restart)
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(h
        .state()
        .with(|s| s.engine.playing && !s.engine.loop_pattern));
}

// ---- chords

/// the roll at 20 px a row from tick 0, C-4 (49) in the middle
fn roll_view(h: &mut Harness<'static, App>) {
    frames(h, 2);
    let a = h.state_mut();
    a.pr.zoom = 20.0;
    a.pr.scroll_x = 0.0;
    a.pr.scroll_y = (120.0 - 52.0) * a.pr.key_h - a.pr.grid_rect.height() / 2.0;
    frames(h, 2);
}

/// screen point of (row, key) in the roll
fn roll_at(h: &Harness<'static, App>, row: usize, key: u8) -> Pos2 {
    let pr = &h.state().pr;
    Pos2::new(
        pr.x_of(tl::rows_to_ticks(row)) + 4.0,
        pr.y_of(key) + pr.key_h / 2.0,
    )
}

fn notes_of_clip(h: &Harness<'static, App>, id: u32) -> Vec<(tl::Tick, u8)> {
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else {
            panic!("not a note clip: {:?}", c.kind)
        };
        let mut v: Vec<_> = t.notes[data as usize]
            .notes
            .iter()
            .map(|n| (n.start, n.key))
            .collect();
        v.sort();
        v
    })
}

#[test]
fn double_click_makes_a_note_clip_and_clicks_on_one_time_build_a_chord() {
    use super::ui_tests::{click_at, double_click};
    let mut h = focus(fileio::demo_song());
    h.state_mut().new_song();
    frames(&mut h, 3);
    h.get_by_label("+ Add track").click();
    frames(&mut h, 3);
    // double-click empty space on the track: a note clip (not a block)
    let (_, row) = h.state().tlv.track_rects(0).expect("track 0");
    let p = Pos2::new(h.state().tlv.x_of(tl::PPQ * 2), row.center().y);
    double_click(&mut h, p);
    frames(&mut h, 2);
    let (id, kind) = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        assert_eq!(t.clips.len(), 1, "one new clip");
        (t.clips[0].id, t.clips[0].kind.clone())
    });
    assert!(matches!(kind, tl::ClipKind::Notes { .. }), "{kind:?}");
    assert!(
        matches!(h.state().pr_target(), Some(super::pianoview::Target::Notes { clip: Some(c), .. }) if c == id)
    );
    roll_view(&mut h);
    for k in [49u8, 53, 56] {
        let p = roll_at(&h, 0, k);
        click_at(&mut h, p);
        frames(&mut h, 1);
    }
    assert_eq!(
        notes_of_clip(&h, id),
        vec![(0, 49), (0, 53), (0, 56)],
        "a chord"
    );
}

/// the demo in timeline mode with channel `ch` of block 0 holding two
/// plain notes (rows 0 and 8), on the Timeline with that clip selected in
/// the piano roll
fn tracker_clip(ch: usize) -> (Harness<'static, App>, u32) {
    let mut s = fileio::demo_song();
    for r in s.patterns[0].cells.iter_mut() {
        r[ch] = Cell::default();
    }
    tl::roll::put_note(&mut s.patterns[0], ch, 0, 8, 49, 1, 0);
    tl::roll::put_note(&mut s.patterns[0], ch, 8, 8, 51, 1, 0);
    tl::enter_timeline_mode(&mut s);
    let mut h = focus(s);
    let id = h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .find(|c| c.block() == Some((0, ch)))
            .map(|c| c.id)
            .expect("a clip of block 0")
    });
    h.state_mut().view = View::Timeline;
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    h.state_mut().set_lower(3);
    h.state_mut().undo_reset();
    roll_view(&mut h);
    (h, id)
}

#[test]
fn a_chord_on_a_tracker_clip_turns_it_and_its_copies_into_one_note_clip() {
    let ch = 1;
    let (mut h, id) = tracker_clip(ch);
    let copies = h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .filter(|c| c.block() == Some((0, ch)))
            .count()
    });
    assert!(matches!(
        h.state().pr_target(),
        Some(super::pianoview::Target::Block { pattern: 0, col }) if col == ch
    ));
    // a note at another pitch on row 0: a chord, not a replacement
    let p = roll_at(&h, 0, 56);
    super::ui_tests::click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(
        notes_of_clip(&h, id),
        vec![(0, 49), (0, 56), (tl::rows_to_ticks(8), 51)]
    );
    // every copy plays the same note data now
    let datas: Vec<u32> = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        t.clips
            .iter()
            .filter_map(|c| match c.kind {
                tl::ClipKind::Notes { data } => Some(data),
                _ => None,
            })
            .collect()
    });
    assert_eq!(datas.len(), copies);
    assert!(datas.iter().all(|d| *d == datas[0]));
    assert!(
        h.state().status.contains("note clip"),
        "{}",
        h.state().status
    );
    // a third note on the same time: still adds
    let p = roll_at(&h, 0, 53);
    super::ui_tests::click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(notes_of_clip(&h, id).iter().filter(|n| n.0 == 0).count(), 3);
    // undo (twice: the third note, then the conversion with its note)
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    let back = h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .block()
    });
    assert_eq!(back, Some((0, ch)), "Ctrl+Z puts the Tracker clip back");
}

#[test]
fn block_col_to_notes_keeps_notes_and_refuses_lone_effects() {
    let mut s = fileio::demo_song();
    for r in s.patterns[0].cells.iter_mut() {
        r[0] = Cell::default();
    }
    tl::roll::put_note(&mut s.patterns[0], 0, 0, 4, 49, 2, 0x21);
    s.patterns[0].cells[0][0].fx = 0x0C;
    s.patterns[0].cells[0][0].fxp = 0x20;
    tl::enter_timeline_mode(&mut s);
    let mut a = s.clone();
    let d = tl::block_col_to_notes(&mut a, 0, 0).expect("converted");
    let t = a.timeline.as_deref().unwrap();
    let n = &t.notes[d].notes;
    assert_eq!(n.len(), 1);
    assert_eq!(
        (n[0].start, n[0].len, n[0].key, n[0].instr),
        (0, tl::rows_to_ticks(4), 49, 2)
    );
    assert_eq!(n[0].fx, Some((0x0C, 0x20)));
    assert!(t
        .clips
        .iter()
        .all(|c| c.block().is_none_or(|b| b != (0, 0))));
    // an effect on a row without a note: nothing changes
    let mut b = s.clone();
    b.patterns[0].cells[2][0].fx = 0x0A;
    b.patterns[0].cells[2][0].fxp = 0x04;
    let before = b.clone();
    assert!(tl::block_col_to_notes(&mut b, 0, 0).is_none());
    assert_eq!(
        serde_json::to_string(&b).unwrap(),
        serde_json::to_string(&before).unwrap()
    );
}

#[test]
fn midi_keys_held_together_step_in_as_a_chord() {
    let mut h = new_song_with_notes();
    let id = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().clips[0].id);
    {
        let a = h.state_mut();
        a.view = View::Timeline;
        a.tlv.sel = Some(id);
        a.tlv.sels = vec![id];
        a.set_lower(3);
        // the piano roll header's Step input toggle (off by default)
        a.pr.step_input = true;
    }
    frames(&mut h, 2);
    let now = std::time::Instant::now;
    // C-5 E-5 G-5 together, released, then D-5
    for (k, n) in [(72u8, 61u8), (76, 65), (79, 68)] {
        h.state_mut().midi_note_on(k, n, 0.8, now());
    }
    for k in [72u8, 76, 79] {
        h.state_mut().midi_note_off(k);
    }
    h.state_mut().midi_note_on(74, 63, 0.8, now());
    h.state_mut().midi_note_off(74);
    frames(&mut h, 2);
    let grid = h.state().pr_grid_now();
    let got = notes_of_clip(&h, id);
    for n in [(0, 61u8), (0, 65), (0, 68), (grid, 63)] {
        assert!(got.contains(&n), "{n:?} in {got:?}");
    }
    assert_eq!(got.len(), 3 + 4, "the clip's own chord + four keys");
}

// ---- the Tracker, hidden by default

fn has(h: &Harness<'static, App>, text: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text))
        .next()
        .is_some()
}

fn count(h: &Harness<'static, App>, text: &str) -> usize {
    h.query_all_by(|n| n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text))
        .count()
}

#[test]
fn the_tracker_tabs_are_hidden_by_default_and_the_tracker_stays_reachable() {
    assert!(!crate::settings::Extra::default().show_tracker);
    let old: crate::settings::Extra = toml::from_str("").unwrap();
    assert!(!old.show_tracker, "old settings files: hidden too");
    let mut h = focus(Song::clean());
    h.state_mut().load_demo(0);
    frames(&mut h, 3);
    assert_eq!(h.state().view, View::Timeline);
    assert!(
        h.state().with(|s| s.song.timeline.is_some()),
        "timeline mode"
    );
    for gone in [
        "TRACKER",
        "ORDER",
        "PIANO",
        "INST",
        "AUTO",
        "CHAINS",
        "TIMELINE + PIANO",
        "Order list",
        "Automation",
        "Play block",
    ] {
        assert!(!has(&h, gone), "{gone} is hidden");
    }
    for here in [
        "TIMELINE",
        "MIX",
        "FX",
        "SAMPLE",
        "Piano roll",
        "Nested Device Chains",
        "Instrument",
        "Play",
    ] {
        assert!(has(&h, here), "{here} is shown");
    }
    assert_eq!(count(&h, "Play"), 1, "one transport (Play / Stop toggle)");
    // View › Tracker stays: a specialised view, as are Ctrl+1 / Ctrl+2
    let views: Vec<View> = h.state().views_shown().iter().map(|v| v.0).collect();
    assert!(views.contains(&View::Pattern) && views.contains(&View::Timeline));
    key(&mut h, Key::Num1, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Pattern, "Ctrl+1: the Tracker");
    assert!(has(&h, "Play block"), "the Tracker's own controls");
    key(&mut h, Key::Num2, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Arranger, "Ctrl+2: the Order list");
    // the other shortcuts
    key(&mut h, Key::Num3, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Mixer);
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Timeline);
    h.state_mut().set_lower(3);
    frames(&mut h, 1);
    key(&mut h, Key::I, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(h.state().extra.lower_inst, "Ctrl+I");
    // again: back to the piano roll (not the hidden Order list)
    key(&mut h, Key::I, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(
        !h.state().extra.lower_inst && h.state().pr.open,
        "Ctrl+I again"
    );
    key(&mut h, Key::Num6, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert!(h.state().pr.open, "Ctrl+6");
    // Settings closes back to the Timeline (not the Tracker)
    key(&mut h, Key::Comma, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Settings);
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Timeline);
    // Settings › Show Tracker brings the tabs back
    h.state_mut().extra.show_tracker = true;
    frames(&mut h, 2);
    assert!(has(&h, "TRACKER") && has(&h, "ORDER") && has(&h, "Order list"));
    assert!(
        has(&h, "Automation"),
        "the Tracker's Automation tab is back"
    );
}

/// the lower panel's Automation tab is the Order list with its song lanes (a
/// Tracker view): hidden with the Tracker, like the Order list tab, and the
/// piano roll stands in when the panel was left on it (saved settings, a
/// song lane); the Timeline's automation is the lanes under each track
#[test]
fn the_automation_tab_hides_with_the_tracker_and_the_piano_roll_stands_in() {
    let mut h = focus(Song::clean());
    h.state_mut().load_demo(0);
    frames(&mut h, 3);
    assert_eq!(h.state().view, View::Timeline);
    assert!(!has(&h, "Automation") && !has(&h, "Order list"));
    // left on the Automation tab (an old settings file, or a song lane)
    {
        let a = h.state_mut();
        a.extra.lower_inst = false;
        a.extra.lower_dev = false;
        a.pr.open = false;
        a.show_auto = true;
        a.arr_focus = true;
    }
    frames(&mut h, 2);
    assert!(h.state().pr.open, "the piano roll stands in");
    assert!(
        !h.state().arr_focus,
        "the hidden Order list has no keyboard"
    );
    assert!(has(&h, "Piano roll") && !has(&h, "Insert block"));
    // asking for it (Ctrl+I back, the old tab number) lands on the piano roll
    h.state_mut().set_lower(1);
    h.state_mut().set_lower(2);
    frames(&mut h, 2);
    assert!(h.state().pr.open && !h.state().extra.lower_inst);
    // the View menu's "Automation lanes" checkbox is a Tracker one too
    assert!(!has(&h, "Automation lanes"));
    // the Tracker on: the tab is back and works as before
    h.state_mut().extra.show_tracker = true;
    frames(&mut h, 2);
    assert!(has(&h, "Automation") && has(&h, "Order list"));
    h.state_mut().set_lower(2);
    frames(&mut h, 2);
    let a = h.state();
    assert!(!a.pr.open && !a.extra.lower_inst && !a.extra.lower_dev && a.show_auto);
    assert!(has(&h, "Insert block"), "the Order list draws");
}

#[test]
fn with_the_tracker_hidden_every_demo_opens_in_timeline_mode_and_plays_identically() {
    fn render_hash(s: &Song) -> (usize, u64) {
        use std::hash::{Hash, Hasher};
        let a = crate::engine::render_song(s, 11025);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for x in &a {
            x.to_bits().hash(&mut h);
        }
        (a.len(), h.finish())
    }
    let mut h = focus(Song::clean());
    // even with "open tracker songs in timeline mode" off
    h.state_mut().extra.open_in_timeline = false;
    let i = 0;
    let want = render_hash(&fileio::demo_song_n(i));
    h.state_mut().load_demo(i);
    frames(&mut h, 2);
    assert_eq!(h.state().view, View::Timeline);
    assert!(!h.state().dirty, "opening is not an edit");
    let s = h.state().with(|s| s.song.clone());
    assert!(s.timeline.is_some(), "demo {i}: timeline mode");
    assert_eq!(render_hash(&s), want, "demo {i} plays the same");
}

#[test]
fn nothing_in_the_roll_follows_the_tracker_cursor_block() {
    // a timeline song, an empty track selected: no target (never block 0)
    let mut h = new_song_with_notes();
    h.state_mut().tlv.sel = None;
    h.state_mut().tlv.sels.clear();
    h.get_by_label("+ Add track").click();
    frames(&mut h, 3);
    h.state_mut().ch = 1;
    h.state_mut().tlv.sel = None;
    frames(&mut h, 2);
    assert_eq!(h.state().pr_target(), None);
    // stale tracker state does not matter
    h.state_mut().pattern = 0;
    frames(&mut h, 1);
    assert_eq!(h.state().pr_target(), None);
    // the Tracker view (View › Tracker) still edits its block
    h.state_mut().view = View::Pattern;
    frames(&mut h, 1);
    assert!(matches!(
        h.state().pr_target(),
        Some(super::pianoview::Target::Block { .. })
    ));
}

// ---- the save prompt

#[test]
fn the_save_prompt_has_a_title_one_line_and_three_equal_buttons() {
    use crate::prompt::Pending;
    let (t, l) = super::dialogs::prompt_text(&Pending::Quit, "My Song", None);
    assert_eq!(t, "Save changes to \u{201C}My Song\u{201D}?");
    assert!(!l.contains('\n'));
    let (t, _) = super::dialogs::prompt_text(
        &Pending::New,
        " ",
        Some(std::path::Path::new("/x/tune.cwt")),
    );
    assert_eq!(t, "Save changes to \u{201C}tune\u{201D}?");
    let (t, _) = super::dialogs::prompt_text(&Pending::Demo(0), "", None);
    assert_eq!(t, "Save changes to \u{201C}Untitled\u{201D}?");
    let mut h = focus(fileio::demo_song());
    let title = h.state().with(|s| s.song.title.clone());
    for p in [Pending::Quit, Pending::New, Pending::Demo(1)] {
        h.state_mut().dirty = true;
        h.state_mut().prompt = Some(p.clone());
        frames(&mut h, 2);
        assert!(
            has(&h, &format!("Save changes to \u{201C}{title}\u{201D}?")),
            "{p:?}"
        );
        let rects: Vec<Rect> = ["Save", "Don't save", "Cancel"]
            .iter()
            .map(|b| {
                h.query_all_by(|n| {
                    n.role() == egui::accesskit::Role::Button && n.label().as_deref() == Some(*b)
                })
                .next()
                .unwrap_or_else(|| panic!("{b}"))
                .rect()
            })
            .collect();
        // one row, left to right, same width, a clear gap
        for w in rects.windows(2) {
            assert!((w[0].width() - w[1].width()).abs() < 0.5, "{rects:?}");
            assert!((w[0].center().y - w[1].center().y).abs() < 0.5);
            assert!(w[1].left() - w[0].right() >= 8.0, "{rects:?}");
        }
        // Esc = Cancel: the prompt goes, the song stays
        key(&mut h, Key::Escape, egui::Modifiers::NONE);
        frames(&mut h, 2);
        assert!(h.state().prompt.is_none());
        assert!(h.state().dirty);
        assert_eq!(h.state().with(|s| s.song.title.clone()), title);
    }
}
