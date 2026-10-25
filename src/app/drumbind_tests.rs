//! A drum kit added after another instrument plays the KIT, by every way
//! of adding one (Del, 25 Oct 2026: "when i add a drum kit it plays the
//! stringwave sound"). The kit's steps and starter grooves were written
//! without an instrument (0), which the engine plays as the channel's
//! last instrument: slot 01, the first track's StringWave.
use super::trackpie::{Builtin, PieAct};
use super::ui_tests::{click_at, frame, harness_with};
use super::*;
use crate::timeline as tl;
use egui_kittest::{kittest::Queryable, Harness};

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// a new timeline song: track 01 plays a new `first` (slot 01) with a
/// note clip of it, track 02 is empty
fn song_after(first: Builtin) -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.set_size(egui::vec2(1366.0, 768.0));
    {
        let a = h.state_mut();
        a.new_song();
        a.view = View::Timeline;
        a.extra.reduce_motion = true;
        assert!(a.track_add(0) && a.track_add(1));
        a.track_new_instrument(0, first);
        assert_eq!(a.track_slot(0), 1);
        a.with(|s| {
            let id = tl::new_note_clip(&mut s.song, 0, 0).unwrap();
            let t = s.song.timeline.as_deref_mut().unwrap();
            let c = t.clips.iter().find(|c| c.id == id).unwrap();
            let data = c.notes().unwrap();
            t.notes[data].make_mut().notes.push(tl::Note {
                key: 49,
                len: 4 * tl::ROW,
                instr: 1,
                ..Default::default()
            });
        });
    }
    frames(&mut h, 3);
    h
}

fn kit_item() -> crate::browser::Item {
    crate::browser::builtin_items()
        .into_iter()
        .find(|i| i.category == "Drum Kit" && matches!(i.src, crate::browser::Src::Gen(_)))
        .expect("a built-in kit")
}

/// click two steps of the open step sequencer (kick lane: steps 1 and 5)
fn click_steps(h: &mut Harness<'static, App>) {
    frames(h, 3);
    for step in [0, 4] {
        let r = h
            .state()
            .dk
            .cells
            .iter()
            .find(|c| c.0 == 0 && c.1 == step)
            .map(|c| c.2)
            .expect("the step grid is drawn");
        click_at(h, r.center());
        frames(h, 2);
    }
}

/// every note on track `tr` and the instrument it plays (an unset one
/// plays the channel's last: slot 01 on a fresh channel)
fn played(h: &Harness<'static, App>, tr: usize) -> Vec<u8> {
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        t.clips
            .iter()
            .filter(|c| c.track as usize == tr)
            .filter_map(|c| c.notes())
            .flat_map(|d| t.notes[d].notes.iter().map(|n| n.instr.max(1)))
            .collect()
    })
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

/// the kit track alone, rendered: it sounds, and silencing the kit
/// instrument silences it (so the kit, not slot 01, is what plays)
fn kit_is_heard(h: &Harness<'static, App>, tr: usize, kit: u8) {
    let mut s = h.state().with(|s| s.song.clone());
    s.timeline
        .as_deref_mut()
        .unwrap()
        .clips
        .retain(|c| c.track as usize == tr);
    let a = crate::engine::render_song(&s, 11025);
    assert!(rms(&a) > 1e-4, "the kit track sounds");
    s.instruments[kit as usize - 1].volume = 0.0;
    let b = crate::engine::render_song(&s, 11025);
    assert!(rms(&b) < rms(&a) * 0.05, "it is the kit that sounds");
}

/// the kit on track `tr`: the track plays a Drum Kit, the track is
/// named after it, every note plays it and the render is the kit's
fn check(h: &Harness<'static, App>, tr: usize, how: &str) {
    let slot = h.state().track_slot(tr);
    let (is_kit, iname, tname) = h.state().with(|s| {
        let i = &s.song.instruments[slot as usize - 1];
        (
            crate::drumkit::kit_of(i).is_some(),
            i.name.clone(),
            s.song.channel_names[tr].clone(),
        )
    });
    assert!(slot > 1 && is_kit, "{how}: track {tr} plays a kit");
    // (ChipWave Instruments › Drum Kit names its new track by the type)
    if tname != "Drum Kit" {
        assert_eq!(tname, iname, "{how}: the header's name and subtitle agree");
    }
    let p = played(h, tr);
    assert!(!p.is_empty(), "{how}: the pattern has notes");
    assert!(
        p.iter().all(|&k| k == slot),
        "{how}: every note plays the kit {slot}, not {p:?}"
    );
    kit_is_heard(h, tr, slot);
}

fn every_path(first: Builtin) {
    // the track pie on track 01: Drum pattern › Drum kit › 909 (a new
    // track 02 with an empty pattern in the step sequencer)
    let mut h = song_after(first);
    let ctx = h.ctx.clone();
    h.state_mut()
        .pie_apply(0, PieAct::DrumKitTrack("909 Kit"), &ctx);
    click_steps(&mut h);
    check(&h, 1, "track pie, new track");
    // the empty-area pie: New track › Drum pattern › 909 (at the end)
    let mut h = song_after(first);
    h.state_mut().pie_apply_new(PieAct::DrumKitTrack("909 Kit"));
    click_steps(&mut h);
    check(&h, 2, "empty-area pie");
    // the empty track 02's pie: Drum kit › 909 here (its starter groove)
    let mut h = song_after(first);
    let ctx = h.ctx.clone();
    h.state_mut()
        .pie_apply(1, PieAct::DrumKitHere("909 Kit"), &ctx);
    frames(&mut h, 3);
    check(&h, 1, "track pie, this track");
    // More kits… › a Browser kit (a new track 02 with an empty pattern)
    let mut h = song_after(first);
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(0, PieAct::MoreKits, &ctx);
    h.state_mut().br_load(&kit_item(), &ctx);
    click_steps(&mut h);
    check(&h, 1, "More kits…");
    // More kits here › a Browser kit onto track 02, then a drawn clip
    let mut h = song_after(first);
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(1, PieAct::MoreKitsHere, &ctx);
    h.state_mut().br_load(&kit_item(), &ctx);
    frames(&mut h, 3);
    let id = h
        .state()
        .with(|s| tl::new_note_clip(&mut s.song, 1, 0))
        .unwrap();
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    h.state_mut().ch = 1;
    h.state_mut().instr = h.state().track_slot(1);
    h.state_mut().show_instrument_editor();
    click_steps(&mut h);
    check(&h, 1, "More kits here");
    // the Browser's kit dropped below the tracks (a new kit track)
    let mut h = song_after(first);
    let n = h.state().with(|s| s.song.channels);
    h.state_mut().track_new_with_instrument(n, Builtin::DrumKit);
    frames(&mut h, 2);
    let id = h
        .state()
        .with(|s| tl::new_note_clip(&mut s.song, n, 0))
        .unwrap();
    h.state_mut().tlv.sel = Some(id);
    h.state_mut().tlv.sels = vec![id];
    h.state_mut().show_instrument_editor();
    click_steps(&mut h);
    check(&h, n, "a Drum Kit track");
}

#[test]
fn a_kit_after_stringwave_plays_the_kit() {
    every_path(Builtin::StringWave);
}

#[test]
fn a_kit_after_waveforge_plays_the_kit() {
    every_path(Builtin::WaveForge);
}

/// Del's song: Kits… › 909 on the new "Psy Kit" track's empty pattern.
/// The groove plays the kit and the track's name follows the kit's
#[test]
fn kits_on_a_new_kit_track_renames_it_and_plays_the_groove() {
    let mut h = song_after(Builtin::StringWave);
    let ctx = h.ctx.clone();
    h.state_mut()
        .pie_apply(0, PieAct::DrumKitTrack("Psy Kit"), &ctx);
    frames(&mut h, 3);
    assert_eq!(
        h.state().with(|s| s.song.channel_names[1].clone()),
        "Psy Kit"
    );
    h.get_by_label("Kits…").click();
    frames(&mut h, 3);
    h.get_by_label("909 Kit").click();
    frames(&mut h, 3);
    check(&h, 1, "Kits… › 909");
    assert_eq!(
        h.state().with(|s| s.song.channel_names[1].clone()),
        "909 Kit"
    );
}
