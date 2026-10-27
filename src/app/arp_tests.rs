//! The Arp (0.2.16): the Rack's "+ ARP" strip, the card with its readout
//! and 16-step grid at 1366x768, one undo step per grid edit, and "Bake
//! Arp to notes"
use super::arpcard::{COL_W, LABEL_W};
use super::ui_tests::{click_at, frame, harness_with, key};
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
        let tl::ClipKind::Notes { data } = c.kind else {
            unreachable!()
        };
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
        let tl::ClipKind::Notes { data } = c.kind else {
            unreachable!()
        };
        t.notes[data as usize].notes.len()
    };
    assert!(
        arp(&h, slot as usize).is_none(),
        "the Arp is off after a bake"
    );
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

/// the MP3 demo's song (CHIPWAVE_ARP_DEMO=<out.cwt>, run with --ignored):
/// the demo's first 8 bars, its pads replaced by a held Am-F-C-G clip that
/// the Breakdown Pluck plays through the Arp (Gated trance, a ratchet, an
/// octave step)
#[test]
#[ignore]
fn write_the_arp_demo_song() {
    let Some(out) = std::env::var_os("CHIPWAVE_ARP_DEMO") else {
        return;
    };
    let mut song = fileio::demo_song();
    let (t, _) = crate::timeline::from_order(&song);
    song.timeline = Some(Box::new(t));
    let bar = 4 * tl::PPQ;
    let pads: Vec<usize> = {
        let t = song.timeline.as_deref_mut().unwrap();
        t.clips.retain(|c| c.start < 8 * bar);
        for c in &mut t.clips {
            c.len = c.len.min(8 * bar - c.start);
        }
        let pads: Vec<usize> = (0..t.tracks.len())
            .filter(|&i| t.tracks[i].name.contains("Pad"))
            .collect();
        t.clips.retain(|c| !pads.contains(&(c.track as usize)));
        // CHIPWAVE_ARP_SOLO: the Arp alone
        if std::env::var_os("CHIPWAVE_ARP_SOLO").is_some() {
            t.clips.clear();
        }
        pads
    };
    let tr = pads[0];
    // a pluck, so each step is heard
    let slot = 1 + song
        .instruments
        .iter()
        .position(|i| i.name.contains("Pluck"))
        .expect("the demo's pluck") as u8;
    let id = tl::new_note_clip(&mut song, tr, 0).unwrap();
    {
        let t = song.timeline.as_deref_mut().unwrap();
        let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
        c.len = 8 * bar;
        let tl::ClipKind::Notes { data } = c.kind else {
            unreachable!()
        };
        let d = t.notes[data as usize].make_mut();
        d.len = 8 * bar;
        for (k, chord) in [[57u8, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]]
            .iter()
            .enumerate()
        {
            for key in chord {
                d.notes.push(tl::Note {
                    start: k as i64 * 2 * bar,
                    len: 2 * bar,
                    key: *key,
                    vel: 0xD000,
                    instr: slot,
                    ..Default::default()
                });
            }
        }
    }
    let mut a = crate::arp::presets(false)
        .into_iter()
        .find(|p| p.0 == "Gated trance")
        .map(|p| p.1)
        .unwrap();
    a.steps.resize(32, crate::arp::Step::default());
    a.steps[6].ratchet = 3;
    a.steps[14].transpose = 12;
    song.instruments[slot as usize - 1].arp = Some(a);
    fileio::save_cwt(&song, std::path::Path::new(&out)).unwrap();
}

#[test]
fn the_more_fold_out_fits_a_laptop_and_its_edits_land() {
    let mut h = devices();
    let strip = h.state().dev.arp_card.center();
    click_at(&mut h, strip);
    frames(&mut h, 3);
    assert!(arp(&h, 7).is_some());
    h.get_by_label("More ▸").click();
    frames(&mut h, 3);
    let a = arp(&h, 7).unwrap();
    assert!(a.show_more, "More opens the fold-out");
    let card = h.state().dev.arp_card;
    let more = h.state().dev.arp_more;
    assert!(more.width() > 150.0, "{more:?}");
    assert!(card.right() <= 1366.5 && card.bottom() <= 768.5, "{card:?}");
    assert!(card.contains_rect(more), "{more:?} in {card:?}");
    // the grid keeps all 8 rows (Chance the last) inside the card
    let rows = h.state().dev.arp_rows;
    for r in &rows[1..] {
        assert!(r.height() >= 12.0, "{rows:?}");
        assert!(r.bottom() <= card.bottom() + 0.5, "{r:?} {card:?}");
        assert!(r.right() <= more.left(), "{r:?} {more:?}");
    }
    // the fold-out's lines are all there
    for l in [
        "Bounce",
        "Retrigger",
        "Free-run",
        "set a song key",
        "25%",
        "Fill",
        "1/96",
    ] {
        assert!(h.query_by_label(l).is_some(), "{l}");
    }
    // Euclid 5/8 Fill: five hits over eight steps, the pattern 8 long
    h.get_by_label("Fill").click();
    frames(&mut h, 3);
    let a = arp(&h, 7).unwrap();
    assert_eq!(a.len, 8);
    let hits = (0..8)
        .filter(|&k| a.step(k).kind == crate::arp::StepKind::Play)
        .count();
    assert_eq!(hits, 5);
    // one undo step
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert_eq!(arp(&h, 7).unwrap().len, 16);
    // Bounce and the 1/96 fast chord
    h.get_by_label("Bounce").click();
    frames(&mut h, 2);
    h.get_by_label("1/96").click();
    frames(&mut h, 2);
    let a = arp(&h, 7).unwrap();
    assert_eq!(a.oct_mode, crate::arp::OctMode::Bounce);
    assert_eq!((a.div, a.feel), (64, crate::arp::Feel::Triplet));
    // a Chance cell: 100 -> 75
    let rows = h.state().dev.arp_rows;
    let at = Pos2::new(
        rows[7].left() + LABEL_W + 2.0 * COL_W + COL_W / 2.0,
        rows[7].center().y,
    );
    click_at(&mut h, at);
    frames(&mut h, 2);
    assert!((arp(&h, 7).unwrap().step(2).chance - 0.75).abs() < 1e-3);
}

/// the Arp's Rate knob: right-click > "Add automation lane" adds a lane
/// on Target::Arp (rate), and the lane plays the rate
#[test]
fn the_rate_knob_offers_an_automation_lane() {
    let mut h = devices();
    let strip = h.state().dev.arp_card.center();
    click_at(&mut h, strip);
    frames(&mut h, 3);
    assert!(arp(&h, 7).is_some());
    let n0 = h.state().with(|s| s.song.automation.len());
    let card = h.state().dev.arp_card;
    let at = h
        .query_all_by_label("Rate")
        .map(|n| n.rect().center())
        .find(|p| card.contains(*p))
        .expect("the Arp's Rate knob");
    super::workflow_tests::right_click(&mut h, at);
    frames(&mut h, 2);
    h.get_by_label("Add automation lane").click();
    frames(&mut h, 3);
    let ids: Vec<String> = h
        .state()
        .with(|s| s.song.automation.iter().map(|l| l.target.clone()).collect());
    assert_eq!(ids.len(), n0 + 1, "{ids:?}");
    let want = crate::automation::Target::Arp(6, 0).id();
    assert!(ids.contains(&want), "{want} not in {ids:?}");
}

/// the arp2 MP3 demo's song (CHIPWAVE_ARP2_DEMO=<out.cwt>, --ignored):
/// Am-F-C-G (two bars a chord) five times over the demo's first 8 bars of
/// drums and bass, a new named preset each time: Trance Gate 16ths, Chip
/// Fast Chord, Synthwave Octaves, Euclid 5/8 Pluck, Probability Sparkle
#[test]
#[ignore]
fn write_the_arp2_demo_song() {
    let Some(out) = std::env::var_os("CHIPWAVE_ARP2_DEMO") else {
        return;
    };
    let presets = [
        "Trance Gate 16ths",
        "Chip Fast Chord",
        "Synthwave Octaves",
        "Euclid 5/8 Pluck",
        "Probability Sparkle",
    ];
    let mut song = fileio::demo_song();
    let (t, _) = crate::timeline::from_order(&song);
    song.timeline = Some(Box::new(t));
    let bar = 4 * tl::PPQ;
    let sec = 8 * bar;
    let tr = {
        let t = song.timeline.as_deref_mut().unwrap();
        t.clips.retain(|c| c.start < sec);
        for c in &mut t.clips {
            c.len = c.len.min(sec - c.start);
        }
        t.end = 0;
        t.markers.clear();
        let pads: Vec<usize> = (0..t.tracks.len())
            .filter(|&i| t.tracks[i].name.contains("Pad"))
            .collect();
        t.clips.retain(|c| !pads.contains(&(c.track as usize)));
        // the 8 bars again under each preset
        let base = t.clips.clone();
        for k in 1..presets.len() as i64 {
            for c in &base {
                let mut c = c.clone();
                c.id = t.new_id();
                c.start += k * sec;
                t.clips.push(c);
            }
        }
        pads[0]
    };
    let pluck = song
        .instruments
        .iter()
        .position(|i| i.name.contains("Pluck"))
        .expect("the demo's pluck");
    // one copy of the pluck per preset (the Arp lives on the instrument)
    let mut slots = vec![];
    for (k, name) in presets.iter().enumerate() {
        let mut i = song.instruments[pluck].clone();
        i.name = format!("Arp {}", k + 1);
        i.arp = Some(crate::arp::preset(name).unwrap_or_else(|| panic!("{name}")));
        song.instruments.push(i);
        slots.push(song.instruments.len() as u8);
    }
    let id = tl::new_note_clip(&mut song, tr, 0).unwrap();
    let t = song.timeline.as_deref_mut().unwrap();
    let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
    let total = sec * presets.len() as i64;
    c.len = total;
    let tl::ClipKind::Notes { data } = c.kind else {
        unreachable!()
    };
    let d = t.notes[data as usize].make_mut();
    d.len = total;
    for (p, &slot) in slots.iter().enumerate() {
        for (k, chord) in [[57u8, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]]
            .iter()
            .enumerate()
        {
            for key in chord {
                d.notes.push(tl::Note {
                    start: p as i64 * sec + k as i64 * 2 * bar,
                    len: 2 * bar,
                    key: *key,
                    vel: 0xD000,
                    instr: slot,
                    ..Default::default()
                });
            }
        }
    }
    song.info.key = crate::chords::parse_key("A minor");
    fileio::save_cwt(&song, std::path::Path::new(&out)).unwrap();
}

/// Del: "it didn't expand to 32 arps when selected". Length 32 widens the
/// grid to all 32 steps at once (it used to keep 16 columns with a small
/// "1-16 ›" page switch); the card still fits 1366x768 with 10 pt text,
/// step 32's cells take clicks, and Length 16 narrows it again
#[test]
fn length_32_shows_all_32_steps_on_a_laptop() {
    let mut h = devices();
    let strip = h.state().dev.arp_card;
    click_at(&mut h, strip.center());
    frames(&mut h, 3);
    let w16 = h.state().dev.arp_card.width();
    let card = h.state().dev.arp_card;
    let b32 = h
        .query_all_by_label("32")
        .map(|n| n.rect())
        .find(|r| card.contains(r.center()))
        .expect("the Length 32 button");
    click_at(&mut h, b32.center());
    frames(&mut h, 3);
    let a = arp(&h, 7).unwrap();
    assert_eq!(a.len, 32);
    assert_eq!(a.steps.len(), crate::arp::MAX_STEPS);
    let card = h.state().dev.arp_card;
    assert!(
        card.width() >= w16 + 16.0 * COL_W - 0.5,
        "{w16} -> {card:?}"
    );
    assert!(card.right() <= 1366.5 && card.bottom() <= 768.5, "{card:?}");
    assert!(crate::theme::ts::TINY >= 10.0);
    let rows = h.state().dev.arp_rows;
    let x32 = rows[3].left() + LABEL_W + 31.0 * COL_W + COL_W / 2.0;
    assert!(x32 < card.right(), "step 32 at {x32} past {card:?}");
    // step 32's accent cell: one click, one undo step
    assert!(!a.step(31).accent);
    click_at(&mut h, Pos2::new(x32, rows[3].center().y));
    frames(&mut h, 3);
    assert!(arp(&h, 7).unwrap().step(31).accent, "step 32 accented");
    h.state_mut().do_undo(false);
    frames(&mut h, 2);
    assert!(!arp(&h, 7).unwrap().step(31).accent);
    // back to 16: the card narrows to 16 columns
    let card = h.state().dev.arp_card;
    let b16 = h
        .query_all_by_label("16")
        .map(|n| n.rect())
        .find(|r| card.contains(r.center()) && r.top() < card.top() + 30.0)
        .expect("the Length 16 button");
    click_at(&mut h, b16.center());
    frames(&mut h, 3);
    assert_eq!(arp(&h, 7).unwrap().len, 16);
    assert!((h.state().dev.arp_card.width() - w16).abs() < 0.5);
}
