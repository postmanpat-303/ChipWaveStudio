//! Importing an instrument makes it the active one (Del's report, 24 Oct
//! 2026: "importing instruments doesn't automatically change to the new
//! instrument"). File › Import instrument (.cwi), the Library's Import /
//! Add, a dropped .cwi or WAV, Import WAV… and the Browser's "Add as a new
//! instrument" all go through `adopt_new_instrument`: in the Timeline the
//! selected track plays it (an empty track) or a new track below does, it
//! is lit, it is `instr`, and the Rack shows its compact card.
use super::ui_tests::{frame, harness_with, key};
use super::*;
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
    frames(&mut h, 4);
    h
}

/// a .cwi on disk: a copy of the song's instrument `i`, renamed
fn cwi_file(h: &mut Harness<'static, App>, i: usize, name: &str) -> std::path::PathBuf {
    let mut inst = song(h, |s| s.instruments[i].clone());
    inst.name = name.into();
    let p = std::env::temp_dir().join(format!(
        "cw-import-{}-{name}.{}",
        std::process::id(),
        crate::instfile::EXT
    ));
    crate::instfile::save_cwi(&inst, &p).expect("save .cwi");
    p
}

/// the active instrument, the selected track's, and the Rack showing it
fn assert_active(h: &mut Harness<'static, App>, slot: u8, name: &str) {
    let ch = h.state().ch;
    assert_eq!(
        h.state().instr,
        slot,
        "the new instrument is the active one"
    );
    assert_eq!(
        h.state().track_instrument(ch),
        Some(slot),
        "the selected track plays it"
    );
    assert_eq!(h.state().tl_cur_track(), ch, "and that track is lit");
    assert!(h.state().extra.lower_dev, "the Rack is open");
    assert_eq!(
        song(h, |s| s.instruments[slot as usize - 1].name.clone()),
        name
    );
}

#[test]
fn an_imported_cwi_on_a_track_with_an_instrument_gets_a_new_track_and_is_active() {
    let mut h = tl_harness();
    h.state_mut().tl_select_track(1, false);
    frames(&mut h, 2);
    let old = h.state().track_slot(1);
    assert!(old > 0, "track 02 plays an instrument");
    let (n0, i0) = song(&mut h, |s| (s.channels, s.instruments.len()));
    let p = cwi_file(&mut h, 0, "Imported Lead");
    h.state_mut().import_cwi(&p);
    frames(&mut h, 3);
    let slot = (i0 + 1) as u8;
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1, "a new track");
    assert_eq!(h.state().ch, 2, "right below the selected track");
    assert_active(&mut h, slot, "Imported Lead");
    // the old track keeps its own instrument
    assert_eq!(h.state().track_slot(1), old);
    // more frames: nothing switches it back
    frames(&mut h, 6);
    assert_active(&mut h, slot, "Imported Lead");
    // one undo step takes the import and its track away
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(
        song(&mut h, |s| (s.channels, s.instruments.len())),
        (n0, i0)
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_imported_cwi_on_an_empty_track_is_that_tracks_instrument() {
    let mut h = tl_harness();
    let n0 = song(&mut h, |s| s.channels);
    assert!(h.state_mut().track_add(n0));
    frames(&mut h, 2);
    assert!(h.state().track_needs_instrument(n0), "an empty track");
    let i0 = song(&mut h, |s| s.instruments.len());
    let p = cwi_file(&mut h, 0, "Imported Pad");
    h.state_mut().import_cwi(&p);
    frames(&mut h, 3);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1, "no extra track");
    assert_eq!(h.state().ch, n0);
    assert_active(&mut h, (i0 + 1) as u8, "Imported Pad");
    assert!(!h.state().track_needs_instrument(n0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_browsers_add_as_a_new_instrument_is_active_too() {
    use super::browserview::Act;
    let mut h = tl_harness();
    h.state_mut().tl_select_track(0, false);
    frames(&mut h, 2);
    let (n0, i0) = song(&mut h, |s| (s.channels, s.instruments.len()));
    let items = song(&mut h, crate::browser::song_items);
    // a song instrument added as a new one (a copy): not a pick of the
    // existing slot
    let it = items[2].clone();
    let name = song(&mut h, |s| s.instruments[2].name.clone());
    h.state_mut().br_instrument(&it, Act::New);
    frames(&mut h, 3);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1);
    assert_eq!(h.state().ch, 1);
    assert_active(&mut h, (i0 + 1) as u8, &name);
}

#[test]
fn an_import_outside_the_timeline_selects_the_new_slot() {
    // a tracker song: the instrument column takes the current instrument
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().view = View::Pattern;
    frames(&mut h, 3);
    let (n0, i0) = song(&mut h, |s| (s.channels, s.instruments.len()));
    let p = cwi_file(&mut h, 0, "Tracker Lead");
    h.state_mut().import_cwi(&p);
    frames(&mut h, 3);
    assert_eq!(song(&mut h, |s| s.channels), n0, "no track is added");
    assert_eq!(h.state().instr, (i0 + 1) as u8);
    let _ = std::fs::remove_file(p);
}
