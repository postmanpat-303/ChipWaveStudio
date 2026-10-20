//! Stabilisation (docs/TIMELINE.md, Del's brief of 20 Oct 2026): regression
//! tests for the inconsistencies found by the audit. Opening another song
//! forgets the old selection; the clipboard only edits the Tracker where it
//! shows; the keys go where the user clicked; the unsaved mark means the
//! song really changed.
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

fn song<R>(h: &Harness<'static, App>, f: impl FnOnce(&Song) -> R) -> R {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    f(&g.song)
}

fn clips(h: &Harness<'static, App>) -> usize {
    song(h, |s| s.timeline.as_ref().map_or(0, |t| t.clips.len()))
}

/// demo `i` opened the normal way (timeline mode, not dirty) on the
/// Timeline view, with the keyboard on the Timeline (as after a click there)
fn opened_demo(i: usize) -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().load_demo(i);
    h.state_mut().view = View::Timeline;
    h.state_mut().pr.focus = false;
    frames(&mut h, 4);
    assert!(
        song(&h, |s| s.timeline.is_some()),
        "demo opens in timeline mode"
    );
    assert!(!h.state().dirty, "opening is not an edit");
    h
}

#[test]
fn opening_another_song_forgets_the_old_selection() {
    let mut h = opened_demo(0);
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    assert!(!h.state().tlv.sels.is_empty(), "Ctrl+A selected clips");
    h.state_mut().pr.sel = vec![0, 1];
    h.state_mut().load_demo(1);
    h.state_mut().view = View::Timeline;
    h.state_mut().pr.focus = false;
    frames(&mut h, 4);
    assert!(h.state().tlv.sels.is_empty() && h.state().tlv.sel.is_none());
    assert!(h.state().pr.sel.is_empty());
    // so Delete right after opening removes nothing
    let n = clips(&h);
    key(&mut h, Key::Delete, egui::Modifiers::NONE);
    assert_eq!(
        clips(&h),
        n,
        "Delete removed clips of the newly opened song"
    );
    assert!(!h.state().dirty);
}

/// the hidden Tracker cursor on a cell with a note
fn cursor_on_a_note(h: &mut Harness<'static, App>) {
    let (p, r, c) = song(h, |s| {
        for (pi, p) in s.patterns.iter().enumerate() {
            for (ri, row) in p.cells.iter().enumerate() {
                for (ci, cell) in row.iter().enumerate() {
                    if cell.note > 0 {
                        return (pi, ri, ci);
                    }
                }
            }
        }
        (0, 0, 0)
    });
    h.state_mut().pattern = p;
    h.state_mut().row = r;
    h.state_mut().ch = c;
    h.state_mut().edit = true;
}

fn cells(s: &Song) -> Vec<Vec<Vec<Cell>>> {
    s.patterns.iter().map(|p| p.cells.clone()).collect()
}

#[test]
fn cut_copy_paste_outside_the_tracker_leave_the_blocks_alone() {
    let mut h = opened_demo(0);
    cursor_on_a_note(&mut h);
    let before = song(&h, cells);
    for v in [View::Timeline, View::Mixer, View::Fx] {
        h.state_mut().view = v;
        frames(&mut h, 2);
        for e in [
            egui::Event::Cut,
            egui::Event::Copy,
            egui::Event::Paste("C-4 01 .. 0000".into()),
        ] {
            let d = format!("{e:?}");
            h.input_mut().events.push(e);
            frames(&mut h, 2);
            eprintln!(
                "{v:?} {d}: {} '{}'",
                song(&h, |s| s.channels),
                h.state().status
            );
        }
        assert_eq!(
            song(&h, cells),
            before,
            "{v:?}: a hidden Tracker block was edited"
        );
        assert_eq!(h.state().status, super::keys::CLIPBOARD_ELSEWHERE);
    }
    assert!(!h.state().dirty);
    // in the Tracker they still work
    h.state_mut().view = View::Pattern;
    frames(&mut h, 2);
    h.input_mut().events.push(egui::Event::Cut);
    frames(&mut h, 2);
    assert_ne!(song(&h, cells), before, "Ctrl+X in the Tracker cuts");
}

#[test]
fn clipboard_keys_on_a_song_with_no_tracks_do_not_crash() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().new_song();
    frames(&mut h, 3);
    assert_eq!(song(&h, |s| s.channels), 0);
    // straight into the Tracker's clipboard code with no tracks (this
    // panicked: the selection was a rectangle of zero channels)
    h.state_mut().edit = true;
    let ctx = h.ctx.clone();
    h.state_mut().copy(false, &ctx);
    assert!(
        h.state().status.starts_with("Nothing to copy"),
        "{}",
        h.state().status
    );
    h.state_mut().paste(Some("C-4 01 .. 0000".into()));
    assert!(
        h.state().status.starts_with("Nothing to paste"),
        "{}",
        h.state().status
    );
    // and through the keys in every view (opening the Tracker view on an
    // empty song gives it a track first)
    for v in [View::Timeline, View::Pattern, View::Arranger] {
        h.state_mut().view = v;
        h.state_mut().edit = true;
        frames(&mut h, 2);
        for e in [
            egui::Event::Copy,
            egui::Event::Cut,
            egui::Event::Paste("C-4 01 .. 0000".into()),
        ] {
            h.input_mut().events.push(e);
            frames(&mut h, 2);
        }
    }
}

#[test]
fn after_opening_a_song_the_timeline_has_the_keyboard() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().load_demo(0);
    frames(&mut h, 4);
    assert_eq!(h.state().view, View::Timeline);
    assert!(h.state().is_default_layout(), "Timeline + piano roll");
    assert!(!h.state().pr_active(), "the roll waits for a click");
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    assert_eq!(
        h.state().tlv.sels.len(),
        clips(&h),
        "Ctrl+A selects every clip"
    );
    assert!(h.state().pr.sel.is_empty(), "and no roll notes");
    // Ctrl+6 and File > New keep it on the Timeline too
    key(&mut h, Key::Num6, egui::Modifiers::COMMAND);
    assert!(!h.state().pr_active());
    // opening a clip in the roll gives the roll the keys
    let id = song(&h, |s| s.timeline.as_ref().unwrap().clips[0].id);
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    h.state_mut().timeline_open_roll();
    frames(&mut h, 2);
    assert!(h.state().pr_active(), "an opened clip takes the keys");
}

#[test]
fn selecting_and_refused_edits_do_not_mark_the_song_unsaved() {
    let mut h = opened_demo(0);
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    assert!(!h.state().tlv.sels.is_empty());
    assert!(!h.state().dirty, "Ctrl+A is not an edit");
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    // Split with nothing under the mouse or the play marker (test-combined:
    // with workflow's split preview, S arms the amber line and Esc cancels it)
    h.state_mut().tlv.cue = 10_000 * tl::PPQ;
    key(&mut h, Key::S, egui::Modifiers::NONE);
    assert!(
        h.state().status.contains("Nothing to split") || h.state().tlv.split_arm,
        "{}",
        h.state().status
    );
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    assert!(!h.state().tlv.split_arm, "Esc cancels the split");
    // Ctrl+L / Ctrl+J with nothing selected
    key(&mut h, Key::L, egui::Modifiers::COMMAND);
    key(&mut h, Key::J, egui::Modifiers::COMMAND);
    assert!(!h.state().dirty, "refused edits are not edits");
    assert!(!h.state().hist.can_undo());
    // a real edit still is
    let id = song(&h, |s| s.timeline.as_ref().unwrap().clips[0].id);
    h.state_mut().tlv.sels = vec![id];
    h.state_mut().tlv.sel = Some(id);
    key(&mut h, Key::M, egui::Modifiers::NONE);
    assert!(h.state().dirty, "M mutes: unsaved");
}

#[test]
fn a_drag_that_snaps_back_is_not_an_edit_and_a_real_move_is() {
    let mut h = harness_with(fileio::demo_song());
    key(&mut h, Key::T, egui::Modifiers::COMMAND);
    h.state_mut().enter_timeline();
    h.state_mut().undo_reset();
    h.state_mut().pr.focus = false;
    h.state_mut().tlv.zoom = 40.0;
    h.state_mut().tlv.scroll = 0.0;
    h.state_mut().tlv.snap = super::timeline::Snap::Bar;
    frames(&mut h, 2);
    h.state_mut().dirty = false;
    let id = {
        let c = h.state().tlv.cache.as_ref().unwrap();
        c.tl.clips
            .iter()
            .find(|c| c.track == 0 && c.len >= 8 * tl::PPQ)
            .unwrap()
            .id
    };
    let from = h.state().tlv.clip_rect(id).unwrap().center();
    // 12 px at 40 px a beat snaps back to the same bar
    super::timeline_m2_tests::drag_m(
        &mut h,
        from,
        from + egui::vec2(12.0, 0.0),
        egui::Modifiers::NONE,
    );
    frames(&mut h, 3);
    assert!(!h.state().dirty, "a drag that moved nothing");
    assert!(!h.state().hist.can_undo());
    // a bar further is an edit
    let from = h.state().tlv.clip_rect(id).unwrap().center();
    super::timeline_m2_tests::drag_m(
        &mut h,
        from,
        from + egui::vec2(160.0, 0.0),
        egui::Modifiers::NONE,
    );
    frames(&mut h, 3);
    assert!(h.state().dirty, "moved a bar: unsaved");
    assert!(h.state().hist.can_undo());
}

#[test]
fn undo_back_to_the_opened_or_saved_song_is_not_unsaved() {
    let mut h = opened_demo(0);
    let id = song(&h, |s| s.timeline.as_ref().unwrap().clips[0].id);
    h.state_mut().tlv.sels = vec![id];
    h.state_mut().tlv.sel = Some(id);
    key(&mut h, Key::M, egui::Modifiers::NONE);
    assert!(h.state().dirty);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert!(!song(&h, |s| s.timeline.as_ref().unwrap().clips[0].muted));
    assert!(!h.state().dirty, "undone back to the opened song");
    key(&mut h, Key::Y, egui::Modifiers::COMMAND);
    assert!(h.state().dirty, "redo: unsaved again");
    // save here: undo past it is unsaved, redo back to it is clean
    let dir = std::env::temp_dir().join(format!("cw-stabilise-undo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("u.cwt");
    assert!(h.state_mut().start_save(p.clone(), true));
    assert!(!h.state().dirty);
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert!(h.state().dirty, "undone past the save");
    key(&mut h, Key::Y, egui::Modifiers::COMMAND);
    assert!(!h.state().dirty, "redone back to the saved song");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn track_lanes_stay_on_their_track_no_unpin_that_springs_back() {
    use super::timeline::autolanes::AutoOp;
    use crate::automation::Target;
    let mut h = opened_demo(0);
    h.state_mut()
        .auto_op(AutoOp::AddLane(1, Target::ChanVolume(1)));
    frames(&mut h, 3);
    let tgt = Target::ChanVolume(1).id();
    let on_track = |h: &Harness<'static, App>| {
        song(h, |s| {
            s.timeline.as_ref().unwrap().tracks[1]
                .auto
                .iter()
                .any(|l| l.target == tgt)
        })
    };
    assert!(on_track(&h));
    // a song lane left over in timeline mode is pinned onto its track
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let l = t.tracks[1].auto.pop().unwrap();
        s.song.automation.push(l);
    });
    frames(&mut h, 3);
    assert!(on_track(&h) && song(&h, |s| s.automation.is_empty()));
}

#[test]
fn escape_toggles_edit_mode_only_in_the_tracker_views() {
    let mut h = opened_demo(0);
    for v in [View::Timeline, View::Mixer, View::Fx, View::Sample] {
        h.state_mut().view = v;
        frames(&mut h, 2);
        let e0 = h.state().edit;
        key(&mut h, Key::Escape, egui::Modifiers::NONE);
        assert_eq!(h.state().edit, e0, "Esc changed Edit mode in {v:?}");
    }
    h.state_mut().view = View::Pattern;
    frames(&mut h, 2);
    let e0 = h.state().edit;
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    assert_ne!(h.state().edit, e0, "Esc is Edit mode in the Tracker");
}

#[test]
fn block_length_keys_only_act_where_the_tracker_shows() {
    let mut h = opened_demo(0);
    let p = h.state().pattern;
    let rows = |h: &Harness<'static, App>| song(h, |s| s.patterns[p].rows);
    let r0 = rows(&h);
    let both = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    key(&mut h, Key::PageDown, both);
    key(&mut h, Key::PageUp, both);
    assert_eq!(rows(&h), r0, "the Timeline resized a hidden block");
    assert!(!h.state().dirty);
    h.state_mut().view = View::Pattern;
    frames(&mut h, 2);
    key(&mut h, Key::PageDown, both);
    assert_eq!(rows(&h), r0 * 2, "the Tracker doubles it");
}

/// File > New with one track and a note clip of 4 notes (keys 49, 51, 53,
/// 55), open in the piano roll
fn roll_with_four_notes() -> (Harness<'static, App>, u32) {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().new_song();
    frames(&mut h, 2);
    assert!(h.state_mut().track_add(0));
    frames(&mut h, 2);
    let id = h.state().with(|s| {
        let id = tl::new_note_clip(&mut s.song, 0, 0).unwrap();
        let t = s.song.timeline.as_deref_mut().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        if let tl::ClipKind::Notes { data } = c.kind {
            let d = t.notes[data as usize].make_mut();
            for (n, k) in [49u8, 51, 53, 55].iter().enumerate() {
                d.notes.push(tl::Note {
                    start: n as tl::Tick * tl::PPQ,
                    len: tl::PPQ / 2,
                    key: *k,
                    vel: 0xC000,
                    rel_vel: 0,
                    instr: 0,
                    fx: None,
                    tune: 0.0,
                    chance: 0,
                    expr: vec![],
                });
            }
        }
        let Shared { song, engine, .. } = s;
        engine.ensure_plan(song);
        id
    });
    frames(&mut h, 2);
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    h.state_mut().timeline_open_roll();
    frames(&mut h, 3);
    (h, id)
}

#[test]
fn undo_does_not_move_the_piano_roll_selection_onto_other_notes() {
    let (mut h, _) = roll_with_four_notes();
    // delete the 2nd note (key 51) as one step, then select key 53 (index 1)
    h.state_mut().snapshot();
    h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        t.notes[0].make_mut().notes.remove(1);
    });
    frames(&mut h, 2);
    h.state_mut().pr.sel = vec![1];
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(
        song(&h, |s| s.timeline.as_ref().unwrap().notes[0].notes.len()),
        4
    );
    assert!(
        h.state().pr.sel.is_empty(),
        "the selection would now be key 51"
    );
    // so an arrow key after undo moves nothing
    let keys = |h: &Harness<'static, App>| {
        song(h, |s| {
            s.timeline.as_ref().unwrap().notes[0]
                .notes
                .iter()
                .map(|n| n.key)
                .collect::<Vec<_>>()
        })
    };
    let k0 = keys(&h);
    key(&mut h, Key::ArrowUp, egui::Modifiers::NONE);
    assert_eq!(keys(&h), k0);
}

#[test]
fn tracker_mode_delete_or_mute_with_nothing_selected_does_not_ask_to_switch() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().extra.open_in_timeline = false;
    h.state_mut().load_demo(0);
    h.state_mut().view = View::Timeline;
    h.state_mut().pr.focus = false;
    frames(&mut h, 4);
    assert!(song(&h, |s| s.timeline.is_none()), "tracker mode");
    for (k, m) in [
        (Key::Delete, egui::Modifiers::NONE),
        (Key::M, egui::Modifiers::NONE),
        (Key::D, egui::Modifiers::COMMAND),
        (Key::J, egui::Modifiers::COMMAND),
    ] {
        key(&mut h, k, m);
        assert!(
            h.state().tlv.ask_switch.is_none(),
            "{k:?} asked to switch modes"
        );
        assert!(
            h.state().status.starts_with("Select a clip first"),
            "{}",
            h.state().status
        );
    }
    assert!(song(&h, |s| s.timeline.is_none()) && !h.state().dirty);
}

fn orphan_in(dir_name: &str) -> crate::recovery::Orphan {
    let dir = std::env::temp_dir().join(format!("cw-stab-{dir_name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("autosave.cwt");
    fileio::save_cwt(&fileio::demo_song(), &file).unwrap();
    crate::recovery::Orphan {
        dir,
        file,
        info: Default::default(),
        source: "autosave",
        modified: std::time::SystemTime::now(),
        crashed: true,
    }
}

fn press(h: &mut Harness<'static, App>, label: &str) {
    let c = h.get_by_label(label).rect().center();
    super::ui_tests::click_at(h, c);
    frames(h, 2);
}

#[test]
fn recovery_discard_asks_before_deleting_the_copy() {
    let mut h = harness_with(fileio::demo_song());
    let o = orphan_in("discard");
    let dir = o.dir.clone();
    h.state_mut().orphans.push(o);
    frames(&mut h, 3);
    press(&mut h, "Discard");
    assert!(dir.exists(), "one click must not delete the copy");
    assert_eq!(h.state().orphans.len(), 1);
    press(&mut h, "Keep");
    assert!(dir.exists());
    press(&mut h, "Discard");
    press(&mut h, "Delete it");
    assert!(!dir.exists());
    assert!(h.state().orphans.is_empty());
}

#[test]
fn recovering_a_copy_opens_it_unsaved_and_not_clean_after_undo() {
    let mut h = harness_with(fileio::demo_song());
    let o = orphan_in("recover");
    let dir = o.dir.clone();
    h.state_mut().orphans.push(o);
    frames(&mut h, 3);
    press(&mut h, "Recover");
    assert!(h.state().orphans.is_empty() && !dir.exists());
    assert!(h.state().dirty && h.state().path.is_none());
    assert!(song(&h, |s| s.title.ends_with("(recovered)")));
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert!(h.state().dirty, "a recovered song is on disk nowhere");
}
