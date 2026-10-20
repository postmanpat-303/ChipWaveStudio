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
    crate::chanops::assign_track_instrument(&mut s, 0, slot);
    let id = tl::new_note_clip(&mut s, 0, 0).unwrap();
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
        let add = h.state().dk.pads.last().map(|p| p.1).unwrap();
        // + ADD PAD sits right after the last pad
        let pr = add.translate(egui::vec2(add.width() + 6.0, 0.0));
        let at = if h.state().dk.pads.len() % 8 == 0 {
            let first = h.state().dk.pads[0].1;
            egui::pos2(first.center().x, add.center().y + add.height() + 6.0)
        } else {
            pr.center()
        };
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
