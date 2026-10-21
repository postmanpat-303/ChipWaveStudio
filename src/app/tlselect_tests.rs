//! Del: "when i select a track it should also make the timeline selection
//! and vice versa". One track selection (`ch` + its instrument) drives the
//! headers, the lane tint, the lower panel and live input; the timeline's
//! clip selection and the headers follow each other.
use super::ui_tests::{click_at, drag, frame, harness_with, key};
use super::*;
use egui::Key;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn tl_demo() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    frames(&mut h, 3);
    h
}

/// visible clips of track `tr`: (id, screen rect)
fn clips_on(h: &Harness<'static, App>, tr: usize) -> Vec<(u32, Rect)> {
    let st = h.state();
    let c = st.tlv.cache.as_ref().unwrap();
    c.tl.clips
        .iter()
        .filter(|x| x.track as usize == tr)
        .filter_map(|x| {
            let r = st.tlv.clip_rect(x.id)?;
            (r.width() > 12.0 && st.tlv.lanes.contains(r.center())).then_some((x.id, r))
        })
        .collect()
}

fn track_of(h: &Harness<'static, App>, id: u32) -> usize {
    let c = h.state().tlv.cache.as_ref().unwrap();
    c.tl.clips.iter().find(|x| x.id == id).unwrap().track as usize
}

/// a spot on the header's name (left of M / S / the fader)
fn head_spot(h: &Harness<'static, App>, tr: usize) -> Pos2 {
    let (hr, _) = h.state().tlv.track_rects(tr).expect("track drawn");
    Pos2::new(hr.left() + 24.0, hr.top() + 10.0)
}

fn click_mod(h: &mut Harness<'static, App>, p: Pos2, modifiers: egui::Modifiers) {
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(modifiers));
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    frame(h);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        });
        frame(h);
    }
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(Default::default()));
    frames(h, 2);
}

#[test]
fn a_track_header_selects_its_lane_and_keeps_only_its_clips() {
    let mut h = tl_demo();
    let (a, _) = clips_on(&h, 0)[0];
    let (b, _) = clips_on(&h, 3)[0];
    h.state_mut().tlv.sels = vec![a, b];
    h.state_mut().tlv.sel = Some(a);
    let p = head_spot(&h, 3);
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(h.state().ch, 3);
    assert_eq!(h.state().tl_cur_track(), 3, "header and lane lit");
    assert_eq!(h.state().instr, h.state().track_slot(3));
    assert_eq!(h.state().tlv.sels, vec![b], "track 4's clip stays selected");
    assert_eq!(h.state().tlv.sel, Some(b));
    // Shift / Ctrl+click on a header: the clip selection is left alone
    h.state_mut().tlv.sels = vec![a, b];
    let p = head_spot(&h, 5);
    click_mod(&mut h, p, egui::Modifiers::SHIFT);
    assert_eq!(h.state().ch, 5);
    assert_eq!(h.state().tlv.sels, vec![a, b]);
}

#[test]
fn a_clip_click_selects_its_track_and_the_lower_panel_follows() {
    let mut h = tl_demo();
    // Devices in the lower panel
    key(&mut h, Key::Num7, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(h.state().ch, 0);
    let (id, r) = clips_on(&h, 4)[0];
    h.state_mut().dirty = false;
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert!(!h.state().dirty, "selecting is not an edit");
    assert_eq!(h.state().tlv.sels, vec![id]);
    assert_eq!(h.state().ch, 4, "the headers follow the clip");
    assert_eq!(h.state().tl_cur_track(), 4);
    let slot = h.state().track_slot(4);
    assert_eq!(h.state().instr, slot);
    assert_eq!(h.state().dev.follow_ch, 4, "Devices shows track 5");
    assert!(h
        .state()
        .devices_target_label()
        .contains(&format!("{slot:02X}")));
    // the piano roll edits that clip
    assert!(matches!(
        h.state().pr_target(),
        Some(super::pianoview::Target::Block { .. } | super::pianoview::Target::Notes { .. })
    ));
}

#[test]
fn an_empty_lane_click_selects_that_track() {
    let mut h = tl_demo();
    let (_, row) = h.state().tlv.track_rects(6).unwrap();
    // past the song's end: nothing there
    let end = h.state().tlv.cache.as_ref().unwrap().map.end();
    let x = (h.state().tlv.x_of(end) + 30.0).min(h.state().tlv.lanes.right() - 10.0);
    let p = Pos2::new(x, row.top() + 10.0);
    assert!(h.state().tlv.lanes.contains(p));
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(h.state().ch, 6);
    assert!(h.state().tlv.sels.is_empty());
}

#[test]
fn a_rubber_band_selects_the_topmost_track_it_caught() {
    let mut h = tl_demo();
    let (_, r2) = clips_on(&h, 2)[0];
    let (_, r5) = h.state().tlv.track_rects(5).unwrap();
    // from the gap left of track 3's first clip (or its left edge) down to track 6
    let from = Pos2::new(h.state().tlv.lanes.left() + 2.0, r2.top() + 2.0);
    let to = Pos2::new(r2.right() + 4.0, r5.bottom() - 4.0);
    // start on empty space: the lanes' left edge before the first clip
    assert!(h.state().tlv.lanes.contains(from));
    drag(&mut h, from, to);
    frames(&mut h, 2);
    let sels = h.state().tlv.sels.clone();
    assert!(!sels.is_empty(), "the band caught clips");
    let top = sels.iter().map(|&id| track_of(&h, id)).min().unwrap();
    assert!(top >= 2);
    assert_eq!(h.state().ch, top, "the headers show the topmost track");
}

#[test]
fn the_song_master_header_is_selectable_and_keeps_the_instrument_track() {
    let mut h = tl_demo();
    h.state_mut().add_lane(crate::automation::Target::Master);
    frames(&mut h, 3);
    let m = h
        .state()
        .with(|s| {
            s.song.timeline.as_deref().and_then(|t| {
                t.tracks
                    .iter()
                    .position(|x| x.kind == crate::timeline::TrackKind::Master)
            })
        })
        .expect("a Song / Master track");
    // scrolled into view (it sits after the eight channel tracks)
    h.state_mut().tlv.reveal = Some((m, 0));
    frames(&mut h, 3);
    let (_, row) = h.state().tlv.track_rects(m - 1).unwrap();
    let p = Pos2::new(row.right() - 20.0, row.top() + 10.0);
    click_at(&mut h, p);
    let tr = h.state().ch;
    assert_eq!(tr, m - 1, "a lane click on track 8");
    let p = head_spot(&h, m);
    click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(h.state().tl_cur_track(), m, "Song / Master lit");
    assert_eq!(h.state().ch, tr, "live input stays on the last track");
    assert!(
        h.state().status.contains("Song / Master"),
        "{}",
        h.state().status
    );
    // the Tracker's channel keys (or anything else) moving `ch`: the
    // headers follow it again
    h.state_mut().ch = 4;
    frames(&mut h, 2);
    assert_eq!(h.state().tl_cur_track(), 4);
}
