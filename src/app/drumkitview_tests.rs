//! UI tests of the Drum Kit editor (click steps, play the pattern loop)
use super::super::ui_tests::{click_at, frame, harness_with};
use super::*;
use egui_kittest::{kittest::Queryable, Harness};

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// a timeline song with a Drum Kit on track 1 and an empty pattern clip
fn kit_harness() -> (Harness<'static, App>, u8, u32) {
    let mut s = crate::fileio::demo_song();
    let (t, _) = crate::timeline::from_order(&s);
    s.timeline = Some(Box::new(t));
    let slot = s
        .add_instrument(crate::drumpat::drumkit_instrument("808 Kit").unwrap())
        .unwrap() as u8
        + 1;
    // the kit on a new, empty last track
    let tr = s.channels;
    assert!(crate::chanops::insert_channel(&mut s, tr, "Drums"));
    crate::chanops::assign_track_instrument(&mut s, tr, slot);
    let id = tl::new_note_clip(&mut s, tr, 0).unwrap();
    let mut h = harness_with(s);
    h.state_mut().instr = slot;
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().extra.lower_h = 760.0;
    frames(&mut h, 4);
    (h, slot, id)
}

fn notes(h: &Harness<'static, App>, id: u32) -> Vec<Note> {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    let t = g.song.timeline.as_deref().unwrap();
    let c = t.clips.iter().find(|c| c.id == id).unwrap();
    let tl::ClipKind::Notes { data } = c.kind else {
        panic!()
    };
    t.notes[data as usize].notes.clone()
}

#[test]
fn the_editor_opens_on_a_drum_kit_and_a_step_click_adds_and_removes_a_hit() {
    let (mut h, slot, id) = kit_harness();
    assert_eq!(h.state().dk.clip, Some(id), "edits the selected clip");
    assert_eq!(h.state().dk.pads.len(), 16, "16 pads by default");
    // the snare lane (pad 1), step 5
    let (p, k, r) = *h
        .state()
        .dk
        .cells
        .iter()
        .find(|c| c.0 == 1 && c.1 == 4)
        .expect("cell drawn");
    let key = h.with_kit(slot, |k| k.pads[p].key);
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    let n = notes(&h, id);
    assert_eq!(n.len(), 1, "{n:?}");
    assert_eq!(n[0].key, key);
    assert_eq!(dk::step_of(n[0].start), k);
    assert_eq!(n[0].instr, 0, "the track's own kit");
    let drum = {
        let s = h.state().shared.clone();
        let g = audio::lock(&s);
        let t = g.song.timeline.as_deref().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else {
            panic!()
        };
        t.notes[data as usize].drum.clone()
    };
    assert!(
        drum.is_some(),
        "a step edit marks the clip as a Drum Kit pattern"
    );
    // undo takes it back; a second click removes it
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert!(notes(&h, id).is_empty());
}

#[test]
fn play_loops_the_song_over_the_pattern() {
    let (mut h, _, id) = kit_harness();
    let r = h
        .query_all_by(|n| n.label().as_deref() == Some("Play loop"))
        .next()
        .map(|n| n.rect())
        .expect("Play loop button");
    let c = egui::pos2(r.center().x, r.center().y);
    click_at(&mut h, c);
    frames(&mut h, 2);
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    let t = g.song.timeline.as_deref().unwrap();
    let clip = t.clips.iter().find(|c| c.id == id).unwrap();
    assert!(t.loop_on);
    assert_eq!(t.loop_range, Some((clip.start, clip.start + clip.len)));
    assert!(g.engine.playing, "playing");
}

#[test]
fn add_pad_stops_at_32() {
    let (mut h, slot, _) = kit_harness();
    for _ in 0..20 {
        let at = h
            .state()
            .dk
            .ctl
            .iter()
            .find(|c| c.0 == "add_pad")
            .map(|c| c.1.center());
        let Some(at) = at else { break };
        click_at(&mut h, at);
        frames(&mut h, 2);
    }
    assert_eq!(h.with_kit(slot, |k| k.pads.len()), dk::MAX_PADS);
}

trait KitAccess {
    fn with_kit<R>(&self, slot: u8, f: impl FnOnce(&DrumKitP) -> R) -> R;
}
impl KitAccess for Harness<'static, App> {
    fn with_kit<R>(&self, slot: u8, f: impl FnOnce(&DrumKitP) -> R) -> R {
        let s = self.state().shared.clone();
        let g = audio::lock(&s);
        f(dk::kit_of(&g.song.instruments[slot as usize - 1]).unwrap())
    }
}

fn clip_count(h: &Harness<'static, App>) -> usize {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    g.song.timeline.as_deref().unwrap().clips.len()
}

fn ctl(h: &Harness<'static, App>, name: &str) -> egui::Rect {
    h.state()
        .dk
        .ctl
        .iter()
        .find(|c| c.0 == name)
        .unwrap_or_else(|| panic!("{name} not drawn"))
        .1
}

/// Generate › Add to track: one clip, on the kit's track, at the marker's
/// bar; one undo removes it; a second Add doesn't stack on the same spot
#[test]
fn generate_add_to_track_places_one_clip_at_the_marker_bar() {
    let (mut h, slot, first) = kit_harness();
    let bar = {
        let s = h.state().shared.clone();
        let g = audio::lock(&s);
        g.song.timeline.as_deref().unwrap().sig_at(0).bar_ticks()
    };
    // the marker a bit into bar 5, away from the first clip (bar 1)
    h.state_mut().tlv.cue = 4 * bar + ROW * 3;
    h.state_mut().dk.tab = 2;
    frames(&mut h, 2);
    let at = ctl(&h, "regen").center();
    click_at(&mut h, at);
    frames(&mut h, 2);
    assert!(h.state().dk.gen.result.is_some());
    let n0 = clip_count(&h);
    let at = ctl(&h, "gen_add").center();
    click_at(&mut h, at);
    frames(&mut h, 3);
    assert_eq!(clip_count(&h), n0 + 1, "exactly one clip");
    let id = h.state().tlv.sel.unwrap();
    assert_ne!(id, first);
    let (track, start, len, ndata) = {
        let s = h.state().shared.clone();
        let g = audio::lock(&s);
        let t = g.song.timeline.as_deref().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else {
            panic!()
        };
        (c.track, c.start, c.len, t.notes[data as usize].notes.len())
    };
    assert_eq!(
        track as usize,
        h.state().dk_track(slot).unwrap(),
        "the kit's track"
    );
    assert_eq!(start, 4 * bar, "at the marker, snapped to bar 5");
    assert!(len > 0 && ndata > 0);
    assert!(h.state().status.contains("Added"), "{}", h.state().status);
    assert_eq!(
        h.state().tlv.flash.map(|f| f.0),
        Some(id),
        "the new clip flashes"
    );
    assert_eq!(h.with_kit(slot, |k| k.pads.len()), 16);
    // one undo removes it
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(clip_count(&h), n0, "one undo step");
    // twice: the second goes after the first, not on top of it
    h.state_mut().dk.tab = 2;
    frames(&mut h, 2);
    if h.state().dk.gen.result.is_none() {
        let at = ctl(&h, "regen").center();
        click_at(&mut h, at);
        frames(&mut h, 2);
    }
    for _ in 0..2 {
        let at = ctl(&h, "gen_add").center();
        click_at(&mut h, at);
        frames(&mut h, 3);
    }
    assert_eq!(clip_count(&h), n0 + 2);
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    let t = g.song.timeline.as_deref().unwrap();
    let mut on: Vec<(Tick, Tick)> = t
        .clips
        .iter()
        .filter(|c| c.track == track)
        .map(|c| (c.start, c.end()))
        .collect();
    on.sort();
    for w in on.windows(2) {
        assert!(w[0].1 <= w[1].0, "clips overlap: {on:?}");
    }
    assert!(on.iter().any(|c| c.0 == 4 * bar));
}

/// the pattern area's Add to track places a linked copy the same way
#[test]
fn pattern_add_to_track_places_a_linked_copy() {
    let (mut h, _, first) = kit_harness();
    let bar = {
        let s = h.state().shared.clone();
        let g = audio::lock(&s);
        g.song.timeline.as_deref().unwrap().sig_at(0).bar_ticks()
    };
    // the marker a bit into the second bar after the first clip
    let end = {
        let s = h.state().shared.clone();
        let g = audio::lock(&s);
        let t = g.song.timeline.as_deref().unwrap();
        t.clips.iter().find(|c| c.id == first).unwrap().end()
    };
    let want = (end + bar - 1) / bar * bar + bar;
    h.state_mut().tlv.cue = want + ROW;
    frames(&mut h, 2);
    let n0 = clip_count(&h);
    let at = ctl(&h, "add").center();
    click_at(&mut h, at);
    frames(&mut h, 3);
    assert_eq!(clip_count(&h), n0 + 1);
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    let t = g.song.timeline.as_deref().unwrap();
    let id = h.state().tlv.sel.unwrap();
    let a = t.clips.iter().find(|c| c.id == first).unwrap();
    let b = t.clips.iter().find(|c| c.id == id).unwrap();
    assert_eq!(b.start, want);
    assert_eq!(a.kind, b.kind, "linked: same note data");
}

/// laptop windows (scale 1.0, the default panel height): the compact layout
/// shows pads, the step grid, Add to track and the tabs without scrolling,
/// and Generate's Add to track and Regenerate too
#[test]
fn laptop_windows_show_the_key_controls() {
    for (w, hh) in [(1366.0, 768.0), (1280.0, 720.0)] {
        let (mut h, _, _) = kit_harness();
        h.state_mut().extra.lower_h = 0.0;
        h.set_size(egui::vec2(w, hh));
        h.state_mut().dk.tab = 0;
        frames(&mut h, 4);
        let st = h.state();
        assert!(st.dk.compact, "{w}x{hh}: compact");
        let view = st.dk.view.unwrap();
        let inside = |r: egui::Rect, what: &str| {
            assert!(
                view.expand(0.5).contains_rect(r),
                "{w}x{hh}: {what} {r:?} outside {view:?}"
            );
        };
        assert_eq!(st.dk.pads.len(), 16, "{w}x{hh}: all 16 pads drawn");
        for (_, r) in &st.dk.pads {
            inside(*r, "pad");
        }
        assert!(
            st.dk.cells.len() >= 8 * 16,
            "{w}x{hh}: 8 lanes x 16 steps, got {} cells in {:?}",
            st.dk.cells.len(),
            st.dk.view
        );
        for (_, _, r) in &st.dk.cells {
            inside(*r, "step cell");
        }
        inside(ctl(&h, "add"), "Add to track");
        inside(ctl(&h, "tabs"), "tab bar");
        // Generate
        h.state_mut().dk.tab = 2;
        frames(&mut h, 2);
        let view = h.state().dk.view.unwrap();
        for k in ["regen", "gen_add", "tabs"] {
            let r = ctl(&h, k);
            assert!(
                view.expand(0.5).contains_rect(r),
                "{w}x{hh}: {k} {r:?} outside {view:?}"
            );
        }
    }
}

/// a tall panel keeps the full layout (cards, 8 pads a row, Sound below)
#[test]
fn a_tall_panel_uses_the_full_layout() {
    let (mut h, _, _) = kit_harness();
    h.set_size(egui::vec2(1800.0, 1500.0));
    h.state_mut().extra.lower_h = 1150.0;
    frames(&mut h, 4);
    let st = h.state();
    assert!(!st.dk.compact, "view {:?}", st.dk.view);
    assert_eq!(st.dk.pads.len(), 16);
    let (a, b) = (st.dk.pads[0].1, st.dk.pads[7].1);
    assert!((a.top() - b.top()).abs() < 0.5, "8 pads a row");
    assert!(st.dk.cells.len() >= 8 * 16);
    assert!(st.dk.ctl.iter().any(|c| c.0 == "add"));
}

/// Stage 5: right-click the step grid: the pattern's pie (Fill, Clear,
/// Copy pattern), each one undo step
#[test]
fn the_step_grid_pie_fills_clears_and_duplicates() {
    let (mut h, slot, id) = kit_harness();
    let (p, _, r) = *h
        .state()
        .dk
        .cells
        .iter()
        .find(|c| c.0 == 0 && c.1 == 2)
        .expect("cell drawn");
    let key = h.with_kit(slot, |k| k.pads[p].key);
    super::super::objpie_tests::right_click(&mut h, r.center());
    assert!(
        matches!(
            h.state().opie.as_ref().map(|x| x.1),
            Some(super::super::objpie::PieFor::Drum { clip, pad: 0, .. }) if clip == id
        ),
        "the pattern's pie on lane 1"
    );
    assert!(notes(&h, id).is_empty(), "a right-click toggles nothing");
    super::super::objpie_tests::pick(&mut h, "Fill", Some("Every beat"));
    let n = notes(&h, id);
    assert!(n.len() >= 4, "{n:?}");
    assert!(n
        .iter()
        .all(|x| x.key == key && dk::step_of(x.start).is_multiple_of(4)));
    // Clear > This lane
    super::super::objpie_tests::right_click(&mut h, r.center());
    super::super::objpie_tests::pick(&mut h, "Clear", Some("This lane"));
    assert!(notes(&h, id).is_empty());
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert!(!notes(&h, id).is_empty(), "undo brings the fill back");
    // Copy pattern: a new clip of its own
    let n0 = clip_count(&h);
    super::super::objpie_tests::right_click(&mut h, r.center());
    super::super::objpie_tests::pick(&mut h, "Copy pattern", None);
    assert_eq!(clip_count(&h), n0 + 1, "{}", h.state().status);
}

/// Del's steps (23 Oct): a Drum Kit on a new track (the pie's Add ›
/// Drum Kit), then a clip drawn by hand on the timeline (no drum metadata),
/// at a laptop size (the compact layout); returns (harness, slot, clip id)
fn manual_clip_harness() -> (Harness<'static, App>, u8, u32) {
    let mut s = crate::fileio::demo_song();
    let (t, _) = crate::timeline::from_order(&s);
    s.timeline = Some(Box::new(t));
    let mut h = harness_with(s);
    h.set_size(egui::vec2(1366.0, 768.0));
    frames(&mut h, 2);
    let n = h.state().with(|s| s.song.channels);
    h.state_mut()
        .track_new_with_instrument(n, crate::app::trackpie::Builtin::DrumKit);
    frames(&mut h, 2);
    let tr = h.state().ch;
    let slot = h.state().instr;
    let id = h
        .state()
        .with(|s| tl::new_note_clip(&mut s.song, tr, 0))
        .unwrap();
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().show_instrument_editor();
    frames(&mut h, 4);
    assert_eq!(
        h.state().dk.clip,
        Some(id),
        "the editor edits the drawn clip"
    );
    assert!(h.state().dk.compact);
    (h, slot, id)
}

fn inst_name(h: &Harness<'static, App>, slot: u8) -> String {
    h.state()
        .with(|s| s.song.instruments[slot as usize - 1].name.clone())
}

fn pick_kit(h: &mut Harness<'static, App>, kit: &str) {
    h.get_by_label("Kits…").click();
    frames(h, 3);
    h.get_by_label(kit).click();
    frames(h, 3);
}

/// the pads of `kit` carry `name`'s sounds and names, key by key
fn has_kit(kit: &DrumKitP, name: &str) -> bool {
    let p = dk::preset(name).unwrap();
    let GenEngine::Drums(src) = p.engine else {
        panic!()
    };
    kit.pads.iter().all(|pad| {
        src.pads
            .iter()
            .find(|x| x.key == pad.key)
            .is_none_or(|x| x.drum == pad.drum && x.name == pad.name)
    })
}

/// the bug: Kits… on an empty, hand-drawn clip changed the pads' sounds
/// only (the header kept "Psy Kit", nothing to hear): now the kit's name,
/// sounds and starter groove land, one undo takes it all back
#[test]
fn kits_on_an_empty_manual_clip_loads_the_kit_and_its_groove() {
    let (mut h, slot, id) = manual_clip_harness();
    let name0 = inst_name(&h, slot);
    let kit0 = h.with_kit(slot, |k| k.clone());
    assert!(notes(&h, id).is_empty());
    pick_kit(&mut h, "Chip Kit");
    assert_eq!(inst_name(&h, slot), "Chip Kit", "the header names the kit");
    assert!(
        h.with_kit(slot, |k| has_kit(k, "Chip Kit")),
        "the Chip Kit sounds"
    );
    let g = starter_groove("Chip Kit").unwrap();
    let n = notes(&h, id);
    assert_eq!(n.len(), g.hits(), "the starter groove {}", g.name);
    assert!(n.iter().all(|x| x.instr == 0), "on the track's own kit");
    assert!(
        h.state().status.contains("Chip Kit"),
        "{}",
        h.state().status
    );
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(inst_name(&h, slot), name0);
    assert_eq!(
        h.with_kit(slot, |k| k.clone()),
        kit0,
        "one undo: the sounds"
    );
    assert!(notes(&h, id).is_empty(), "one undo: the groove");
}

/// with notes, Kits… swaps the sounds and the name; the notes stay
#[test]
fn kits_on_a_manual_clip_with_notes_keeps_the_notes() {
    let (mut h, slot, id) = manual_clip_harness();
    let (p, _, r) = *h
        .state()
        .dk
        .cells
        .iter()
        .find(|c| c.0 == 0 && c.1 == 0)
        .expect("cell drawn");
    let _ = p;
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    let n0 = notes(&h, id);
    assert_eq!(n0.len(), 1);
    pick_kit(&mut h, "909 Kit");
    assert_eq!(inst_name(&h, slot), "909 Kit");
    assert!(h.with_kit(slot, |k| has_kit(k, "909 Kit")));
    assert_eq!(notes(&h, id), n0, "the pattern stays");
    assert!(
        h.state().status.contains("notes stay"),
        "{}",
        h.state().status
    );
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_ne!(inst_name(&h, slot), "909 Kit");
    assert_eq!(notes(&h, id), n0);
}

/// Dice kit on an empty pattern: another kit and its groove, one undo;
/// with notes it re-rolls the pads and the notes stay
#[test]
fn dice_kit_on_a_manual_clip() {
    let (mut h, slot, id) = manual_clip_harness();
    let name0 = inst_name(&h, slot);
    h.get_by_label("Dice kit").click();
    frames(&mut h, 3);
    let name1 = inst_name(&h, slot);
    assert_ne!(name1, name0, "a new kit");
    assert!(h.with_kit(slot, |k| has_kit(k, &name1)));
    let g = starter_groove(&name1).unwrap();
    assert_eq!(notes(&h, id).len(), g.hits(), "{name1}'s groove {}", g.name);
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(inst_name(&h, slot), name0);
    assert!(notes(&h, id).is_empty(), "one undo");
    // now with the groove back: Dice re-rolls the pads, notes stay
    h.state_mut().undo(true);
    frames(&mut h, 2);
    let n0 = notes(&h, id);
    assert!(!n0.is_empty());
    let kit0 = h.with_kit(slot, |k| k.clone());
    h.get_by_label("Dice kit").click();
    frames(&mut h, 3);
    assert_eq!(notes(&h, id), n0, "the pattern stays");
    assert_eq!(inst_name(&h, slot), name1, "the same kit");
    let k1 = h.with_kit(slot, |k| k.clone());
    assert!(
        kit0.pads
            .iter()
            .zip(&k1.pads)
            .any(|(a, b)| a.drum != b.drum),
        "the pads re-rolled"
    );
    assert!(
        h.state().status.contains("re-rolled"),
        "{}",
        h.state().status
    );
}

/// every Kits… entry has a starter groove of its own
#[test]
fn every_kit_has_a_starter_groove() {
    for (n, _, _) in dk::presets() {
        let g = starter_groove(&n).unwrap_or_else(|| panic!("{n}"));
        assert_eq!(g.kit, n);
        assert!(g.hits() > 0);
    }
}

/// the Round Robin tab: off by default, On adds the settings, + Sound adds
/// sounds that the Next 8 hits row then rotates through; undo steps back
#[test]
fn the_round_robin_tab_turns_on_adds_sounds_and_undoes() {
    let (mut h, slot, _) = kit_harness();
    let sel = h.state().dk.sel;
    assert!(
        h.with_kit(slot, |k| k.pads[sel].rr.is_none()),
        "off by default"
    );
    h.state_mut().dk.tab = 3;
    frames(&mut h, 2);
    h.get_by_label("On").click();
    frames(&mut h, 2);
    assert!(h.with_kit(slot, |k| k.pads[sel].rr.is_some()));
    for _ in 0..2 {
        h.get_by_label("+ Sound").click();
        frames(&mut h, 2);
    }
    assert_eq!(
        h.with_kit(slot, |k| k.pads[sel].rr.as_ref().unwrap().sounds()),
        3
    );
    assert!(h.query_by_label("Next 8 hits").is_some());
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(
        h.with_kit(slot, |k| k.pads[sel].rr.as_ref().map(|r| r.sounds())),
        Some(2),
        "one undo step per edit"
    );
    // off again: nothing left behind
    h.get_by_label("On").click();
    frames(&mut h, 2);
    assert!(h.with_kit(slot, |k| k.pads[sel].rr.is_none()));
}
