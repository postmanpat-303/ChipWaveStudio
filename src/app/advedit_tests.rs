//! Tracker: Advanced Edit panel and the coloured channel headers
//! (docs/ADVANCED_EDIT.md). The operations themselves: src/advedit/tests.rs.
use super::advedit::AdvOp;
use super::pattern::HdrOp;
use super::ui_tests::{click_at, frame, harness_with, key};
use super::*;
use crate::advedit::{Field, Mask, Section};
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

fn ctrl_shift() -> egui::Modifiers {
    egui::Modifiers {
        ctrl: true,
        command: true,
        shift: true,
        ..Default::default()
    }
}

fn ctrl() -> egui::Modifiers {
    egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    }
}

/// the demo song in the tracker on its busiest (block, channel 1/2 pair)
fn tracker() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().view = View::Pattern;
    let b = song(&h, |s| {
        (0..s.patterns.len())
            .filter(|&i| s.patterns[i].derived.is_none())
            .max_by_key(|&i| {
                s.patterns[i]
                    .cells
                    .iter()
                    .filter(|r| r[0].note > 0 && r[0].note < 255)
                    .count()
            })
            .unwrap()
    });
    h.state_mut().pattern = b;
    frames(&mut h, 3);
    h
}

fn notes(h: &Harness<'static, App>, b: usize, ch: usize) -> Vec<u8> {
    song(h, |s| {
        s.patterns[b].cells.iter().map(|r| r[ch].note).collect()
    })
}

#[test]
fn ctrl_shift_a_toggles_the_docked_panel_and_ctrl_a_still_selects() {
    let mut h = tracker();
    assert!(!h.state().extra.adv_edit_open);
    key(&mut h, Key::A, ctrl_shift());
    frames(&mut h, 2);
    assert!(h.state().extra.adv_edit_open);
    assert!(h.state().sel.is_none(), "Ctrl+Shift+A doesn't select");
    // the panel's buttons are there (and work from a click)
    h.state_mut().sel = Some(((0, 0), (15, 0)));
    h.state_mut().adv.section = Section::Selection;
    frames(&mut h, 2);
    let b = h.state().pattern;
    let before = notes(&h, b, 0);
    h.get_by_label("Flip").click();
    frames(&mut h, 3);
    let after = notes(&h, b, 0);
    let mut want = before[..16].to_vec();
    want.reverse();
    assert_eq!(&after[..16], &want[..]);
    assert_eq!(&after[16..], &before[16..]);
    key(&mut h, Key::A, ctrl_shift());
    frames(&mut h, 2);
    assert!(!h.state().extra.adv_edit_open);
    key(&mut h, Key::A, ctrl());
    frames(&mut h, 1);
    assert!(
        h.state().sel.is_some(),
        "Ctrl+A selects the channel as before"
    );
}

#[test]
fn each_operation_is_one_undo_step() {
    let mut h = tracker();
    let orig = song(&h, |s| s.patterns.clone());
    h.state_mut().adv.section = Section::TrackPattern;
    h.state_mut().ch = 0;
    let ctx = h.ctx.clone();
    for op in [
        AdvOp::Flip,
        AdvOp::Shrink,
        AdvOp::Expand,
        AdvOp::Transpose(12),
        AdvOp::Mirror,
        AdvOp::Fade(0x40, 0),
        AdvOp::Clear,
    ] {
        let before = song(&h, |s| s.patterns.clone());
        h.state_mut().adv_op(op, Some(&ctx));
        frames(&mut h, 2);
        assert_ne!(
            song(&h, |s| s.patterns.clone()),
            before,
            "{op:?} changed something"
        );
        key(&mut h, Key::Z, ctrl());
        frames(&mut h, 2);
        assert_eq!(
            song(&h, |s| s.patterns.clone()),
            before,
            "{op:?}: one Ctrl+Z undoes it"
        );
    }
    assert_eq!(song(&h, |s| s.patterns.clone()), orig);
}

#[test]
fn copy_mix_paste_and_replace_follow_the_section_and_mask() {
    let mut h = tracker();
    let ctx = h.ctx.clone();
    // copy channel 1's notes only, mix-paste them over channel 2
    h.state_mut().adv.section = Section::TrackPattern;
    h.state_mut().adv.mask = Mask::only(Field::Note);
    h.state_mut().ch = 0;
    h.state_mut().adv_op(AdvOp::Copy, Some(&ctx));
    let b = h.state().pattern;
    let src = notes(&h, b, 0);
    let dst = notes(&h, b, 1);
    h.state_mut().ch = 1;
    h.state_mut().row = 0;
    h.state_mut().adv_op(AdvOp::MixPaste, Some(&ctx));
    frames(&mut h, 2);
    let got = notes(&h, b, 1);
    for r in 0..got.len() {
        // a mix-paste keeps every existing note and fills the empty ones
        let want = if dst[r] != 0 { dst[r] } else { src[r] };
        assert_eq!(got[r], want, "row {r}");
    }
    // Column in song: the cursor's column; replace instrument everywhere on channel 1
    let (from, n_from) = song(&h, |s| {
        let i = s.patterns[b].cells.iter().find(|r| r[0].instr > 0).unwrap()[0].instr;
        (
            i,
            s.patterns
                .iter()
                .flat_map(|p| p.cells.iter())
                .filter(|r| r[0].instr == i)
                .count(),
        )
    });
    let to = if from == 1 { 2 } else { 1 };
    h.state_mut().adv.section = Section::TrackSong;
    h.state_mut().adv.mask = Mask::default();
    h.state_mut().ch = 0;
    h.state_mut().adv.rep_from = from;
    h.state_mut().adv.rep_to = to;
    h.state_mut().adv_op(AdvOp::Replace, Some(&ctx));
    let left = song(&h, |s| {
        s.patterns
            .iter()
            .filter(|p| p.derived.is_none())
            .flat_map(|p| p.cells.iter())
            .filter(|r| r[0].instr == from)
            .count()
    });
    assert!(n_from > 0);
    assert_eq!(left, 0);
    assert!(
        h.state().status.contains("Instrument"),
        "{}",
        h.state().status
    );
    // an empty mask refuses with a message
    h.state_mut().adv.mask = Mask([false; 5]);
    let before = song(&h, |s| s.patterns.clone());
    h.state_mut().adv_op(AdvOp::Flip, Some(&ctx));
    assert_eq!(song(&h, |s| s.patterns.clone()), before);
    assert!(h.state().status.contains("mask"));
}

#[test]
fn headers_show_track_colour_instrument_and_their_buttons_work() {
    let mut s = fileio::demo_song();
    let (mut tl, _) = crate::timeline::from_order(&s);
    tl.tracks[2].colour = Some(0x123456);
    s.timeline = Some(Box::new(tl));
    let mut h = harness_with(s);
    h.state_mut().view = View::Pattern;
    frames(&mut h, 3);
    let hdr = h.state().channel_headers(4);
    assert_eq!(
        hdr.colours[2],
        hex(0x123456),
        "the timeline track's own colour"
    );
    assert_eq!(hdr.colours[1], super::timeline::track_colour(1));
    assert!(
        hdr.instruments.iter().all(|n| !n.is_empty()),
        "{:?}",
        hdr.instruments
    );
    let hit = |h: &Harness<'static, App>, ch, op| {
        h.state()
            .hdr_hits
            .iter()
            .find(|x| x.0 == ch && x.1 == op)
            .map(|x| x.2.center())
            .unwrap()
    };
    // M and S toggle mute / solo
    let m = hit(&h, 1, HdrOp::Mute);
    click_at(&mut h, m);
    frames(&mut h, 2);
    assert!(song(&h, |s| s.mixer.channels[1].mute));
    let sp = hit(&h, 1, HdrOp::Solo);
    click_at(&mut h, sp);
    frames(&mut h, 2);
    assert!(song(&h, |s| s.mixer.channels[1].solo));
    // Play auditions the channel's instrument (a held preview note)
    let p = hit(&h, 0, HdrOp::Play);
    click_at(&mut h, p);
    frames(&mut h, 1);
    assert!(!h.state().chords.offs.is_empty(), "audition note held");
    assert!(
        h.state().status.starts_with("Audition"),
        "{}",
        h.state().status
    );
    assert_eq!(h.state().ch, 0);
}

#[test]
fn headers_stay_compact_at_32_channels_and_names_truncate() {
    let mut h = tracker();
    h.state_mut().set_channels(32);
    h.state_mut()
        .with(|s| s.song.channel_names[3] = "A very long channel name indeed".into());
    frames(&mut h, 3);
    let hits = &h.state().hdr_hits;
    assert!(!hits.is_empty());
    // the buttons of one header sit inside one channel's width (no overlap)
    let w = |ch| {
        let xs: Vec<f32> = hits
            .iter()
            .filter(|x| x.0 == ch)
            .map(|x| x.2.left())
            .collect();
        xs.iter().cloned().fold(f32::MAX, f32::min)
    };
    let r3: Vec<_> = hits.iter().filter(|x| x.0 == 3).collect();
    let r4 = w(4);
    assert!(
        r3.iter().all(|x| x.2.right() < r4),
        "header 4's buttons don't overlap header 5's"
    );
}
