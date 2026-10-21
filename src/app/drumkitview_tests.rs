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
/// Duplicate as variation), each one undo step
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
    // Duplicate as variation: a new clip of its own
    let n0 = clip_count(&h);
    super::super::objpie_tests::right_click(&mut h, r.center());
    super::super::objpie_tests::pick(&mut h, "Duplicate as variation", None);
    assert_eq!(clip_count(&h), n0 + 1, "{}", h.state().status);
}
