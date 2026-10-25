//! The Arp (0.2.16): the Rack's "+ ARP" strip, the card with its readout
//! and 16-step grid at 1366x768, one undo step per grid edit, and "Bake
//! Arp to notes"
use super::arpcard::{COL_W, LABEL_W};
use super::ui_tests::{click_at, frame, harness_with, key};
use super::*;
use crate::timeline as tl;
use egui::Key;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn arp(h: &Harness<'static, App>, slot: usize) -> Option<crate::arp::Arp> {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    g.song.instruments[slot - 1].arp.clone()
}

/// the demo at 1366x768, Devices showing instrument 7 (a synth)
fn devices() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.set_size(egui::vec2(1366.0, 768.0));
    h.state_mut().dev.tree = false;
    key(&mut h, Key::Num7, egui::Modifiers::COMMAND);
    h.state_mut().instr = 7;
    h.state_mut().dev.follow_ch = h.state().ch;
    frames(&mut h, 3);
    h
}

#[test]
fn plus_arp_opens_the_card_that_fits_a_laptop_and_grid_edits_undo() {
    let mut h = devices();
    assert!(arp(&h, 7).is_none());
    let strip = h.state().dev.arp_card;
    assert!(strip.width() > 0.0 && strip.width() < 40.0, "{strip:?}");
    click_at(&mut h, strip.center());
    frames(&mut h, 3);
    let a = arp(&h, 7).expect("the Arp switched on");
    // the readout says it in plain words
    let words = a.describe(false);
    assert!(words.len() > 10, "{words}");
    // the card and all 16 steps of the grid fit on 1366x768
    let card = h.state().dev.arp_card;
    assert!(card.width() > 400.0, "{card:?}");
    assert!(card.right() <= 1366.5 && card.bottom() <= 768.5, "{card:?}");
    let rows = h.state().dev.arp_rows;
    for r in &rows[1..] {
        assert!(r.height() >= 12.0, "{rows:?}");
        assert!(r.bottom() <= card.bottom() + 0.5, "{r:?} {card:?}");
    }
    let last = rows[3].left() + LABEL_W + 15.0 * COL_W + COL_W / 2.0;
    assert!(last <= card.right(), "step 16 at {last} past {card:?}");
    // a click on step 3's accent toggles it: one undo step
    let before = a.step(2).accent;
    let at = Pos2::new(
        rows[3].left() + LABEL_W + 2.0 * COL_W + COL_W / 2.0,
        rows[3].center().y,
    );
    click_at(&mut h, at);
    frames(&mut h, 3);
    assert_eq!(arp(&h, 7).unwrap().step(2).accent, !before);
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert_eq!(arp(&h, 7).unwrap().step(2).accent, before);
    // undo again: the Arp is off, its strip back
    h.state_mut().do_undo(false);
    frames(&mut h, 3);
    assert!(arp(&h, 7).is_none());
    assert!(h.state().dev.arp_card.width() < 40.0);
}

#[test]
fn bake_arp_writes_the_steps_as_notes_and_switches_the_arp_off() {
    // a timeline song with a new note clip on track 1: a held C-E-G chord
    // over a bar, playing instrument 7 (a synth) through an Arp
    let mut song = fileio::demo_song();
    let (t, _) = crate::timeline::from_order(&song);
    song.timeline = Some(Box::new(t));
    let slot = 7u8;
    let id = tl::new_note_clip(&mut song, 0, 0).expect("a note clip");
    {
        let t = song.timeline.as_deref_mut().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else { unreachable!() };
        let d = t.notes[data as usize].make_mut();
        for key in [60u8, 64, 67] {
            d.notes.push(tl::Note {
                start: 0,
                len: 4 * tl::PPQ,
                key,
                vel: 0xC000,
                instr: slot,
                ..Default::default()
            });
        }
    }
    song.instruments[slot as usize - 1].arp = Some(crate::arp::Arp::default());
    let n0 = 3;
    let mut h = harness_with(song);
    frames(&mut h, 2);
    let s = h.state().shared.clone();
    assert_eq!(tl::arp_slots(&audio::lock(&s).song, id), vec![slot]);
    h.state_mut().tlv.sels = vec![id];
    h.state_mut().tl_apply(super::timeline::TlOp::BakeArp);
    frames(&mut h, 2);
    let notes = |h: &Harness<'static, App>| {
        let s = h.state().shared.clone();
        let g = audio::lock(&s);
        let t = g.song.timeline.as_deref().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let tl::ClipKind::Notes { data } = c.kind else { unreachable!() };
        t.notes[data as usize].notes.len()
    };
    assert!(arp(&h, slot as usize).is_none(), "the Arp is off after a bake");
    assert!(notes(&h) > 0);
    // a bar of 1/16 steps over the chord: 16 notes, not the 3 held
    assert_eq!(notes(&h), 16, "the clip now holds the Arp's steps");
    assert!(h.state().status.contains("Baked"), "{}", h.state().status);
    // one undo step brings back the held notes and the Arp
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert_eq!(notes(&h), n0);
    assert!(arp(&h, slot as usize).is_some());
}
