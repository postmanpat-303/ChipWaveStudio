//! The eight-place menus: clicked target, cancel, multi-clip, scoped lanes.
use super::objpie::PieFor;
use super::objpie_tests::{pick, right_click};
use super::ui_tests::{frame, harness_with};
use super::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn song() -> (Harness<'static, App>, u32) {
    super::unified_tests::notes_song()
}

#[test]
fn right_click_track_b_while_a_is_selected_and_cancel_keeps_a() {
    let (mut h, _) = song();
    h.state_mut().ch = 0;
    frames(&mut h, 2);
    let (head, _) = h.state().tlv.track_rects(1).expect("track 2");
    // the name, not the mute button or the fader
    right_click(&mut h, egui::pos2(head.left() + 36.0, head.center().y));
    assert!(
        matches!(h.state().opie.as_ref().map(|p| p.1), Some(PieFor::Track(1))),
        "the menu is track 2, {:?}",
        h.state().opie.as_ref().map(|p| p.1)
    );
    assert_eq!(h.state().ch, 0, "still track 1 until a choice");
    h.key_press(egui::Key::Escape);
    frames(&mut h, 2);
    assert!(h.state().opie.is_none());
    assert_eq!(h.state().ch, 0);
}

#[test]
fn the_first_remove_click_does_not_delete() {
    let (mut h, id) = song();
    let n = h
        .state()
        .with(|s| s.song.timeline.as_deref().unwrap().clips.len());
    let p = super::unified_tests::at(&h, 0, crate::timeline::PPQ);
    right_click(&mut h, p);
    pick(&mut h, "Remove…", None);
    assert!(h.state().opie.is_some(), "the confirm ring stays open");
    assert_eq!(
        h.state()
            .with(|s| s.song.timeline.as_deref().unwrap().clips.len()),
        n
    );
    assert!(h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .any(|c| c.id == id)
    }));
    pick(&mut h, "Delete clip", None);
    assert!(h.state().opie.is_none());
    assert!(h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .all(|c| c.id != id)
    }));
}

#[test]
fn a_multi_selection_is_named_and_deleted_together() {
    let (mut h, a) = song();
    let b = h.state().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        let src = t.clips.iter().find(|c| c.id == a).unwrap().clone();
        crate::timeline::new_note_clip(&mut s.song, 0, src.end()).unwrap()
    });
    frames(&mut h, 2);
    h.state_mut().tlv.sels = vec![a, b];
    h.state_mut().tlv.sel = Some(b);
    let p = super::unified_tests::at(&h, 0, crate::timeline::PPQ);
    right_click(&mut h, p);
    let (pie, on) = h.state().opie.clone().unwrap();
    assert!(matches!(on, PieFor::Clip(id) if id == a));
    assert_eq!(pie.title, "2 clips");
    pick(&mut h, "Remove…", None);
    pick(&mut h, "Delete clip", None);
    let left = h.state().with(|s| {
        s.song
            .timeline
            .as_deref()
            .unwrap()
            .clips
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>()
    });
    assert!(!left.contains(&a) && !left.contains(&b), "{left:?}");
}

#[test]
fn nothing_copied_is_grey_and_a_sound_without_presets_offers_browse() {
    let (mut h, _) = song();
    h.state_mut().tlv.clipboard.clips.clear();
    let p = super::unified_tests::at(&h, 1, 4 * crate::timeline::PPQ);
    right_click(&mut h, p);
    let dim = h
        .state()
        .opie
        .as_ref()
        .unwrap()
        .0
        .items
        .iter()
        .find(|i| i.dim)
        .cloned();
    let dim = dim.expect("greyed paste");
    assert_eq!(dim.label, "Nothing copied");
    assert!(!dim.goes);
    // an empty track's sound wedge is Browse, not a dead Sounds ›
    h.state_mut().opie = None;
    let nch = h.state().with(|s| s.song.channels);
    h.state_mut().track_add(nch);
    frames(&mut h, 2);
    let tr = h.state().with(|s| s.song.channels) - 1;
    let ctx = h.ctx.clone();
    h.state_mut()
        .open_obj_pie(PieFor::Track(tr), egui::pos2(400.0, 300.0), &ctx);
    let labels: Vec<_> = h
        .state()
        .opie
        .as_ref()
        .unwrap()
        .0
        .items
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(
        labels.iter().any(|l| l.starts_with("Instrument")),
        "{labels:?}"
    );
    assert!(!labels.iter().any(|l| l.starts_with("Sounds")));
}

#[test]
fn an_effect_lane_page_names_only_that_effect() {
    let eq = super::compass::effect_param_names(crate::fx::FxKind::Eq);
    let rev = super::compass::effect_param_names(crate::fx::FxKind::Reverb);
    assert!(eq
        .iter()
        .any(|n| n.eq_ignore_ascii_case("mix") || n.contains("Low") || n.contains("low")));
    assert!(rev
        .iter()
        .all(|n| !eq.iter().any(|e| e == n) || n.eq_ignore_ascii_case("mix")));
    assert!(!eq.iter().any(|n| n == "Ceiling" || n == "Master"));
    let (mut h, _) = song();
    h.state_mut()
        .track_new_instrument(0, super::trackpie::Builtin::WaveForge);
    let idx = h.state().with(|s| s.song.instruments.len() - 1);
    h.state_mut().apply_fx_ops(
        idx,
        vec![super::fxview::FxOp::Insert(
            usize::MAX,
            Box::new(crate::fx::Device::new(crate::fx::FxKind::Reverb)),
        )],
    );
    let ctx = h.ctx.clone();
    h.state_mut()
        .open_obj_pie(PieFor::RackFx(idx, 0), egui::pos2(500.0, 400.0), &ctx);
    pick(&mut h, "Add lane", None);
    let names: Vec<_> = h
        .state()
        .opie
        .as_ref()
        .unwrap()
        .0
        .items
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(names.iter().any(|n| n == "Mix" || n == "Size"), "{names:?}");
    assert!(!names.iter().any(|n| n == "Cutoff" || n.contains("EQ")));
}

#[test]
fn compass_labels_fit_a_laptop_at_every_pie_size() {
    use super::piesize::{self, of_setting};
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1366.0, 768.0));
    for setting in [0u8, 1, 2] {
        piesize::set(of_setting(setting), screen);
        let k = piesize::k();
        let pt = super::radial::compass_label_pt();
        assert!(pt >= 11.0, "setting {setting} k {k} pt {pt}");
        let rout = super::radial::compass_rout();
        assert!(
            rout + 8.0 <= 384.0,
            "the ring fits 1366x768 vertically at setting {setting}: {rout}"
        );
        let r = (super::radial::compass_rin() + rout) / 2.0;
        let chord = 2.0 * r * (std::f32::consts::TAU / 16.0).sin() * 0.90;
        // "Drum pattern ›" at 11pt is about 78px; the chord stays above that
        assert!(chord >= 78.0, "chord {chord} at k {k}");
    }
}

#[test]
fn a_new_track_exports_audible_audio() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().new_song();
    frames(&mut h, 2);
    h.state_mut()
        .track_new_with_instrument(0, super::trackpie::Builtin::WaveForge);
    frames(&mut h, 2);
    h.state_mut().with(|s| {
        let id = crate::timeline::new_note_clip(&mut s.song, 0, 0).unwrap();
        let t = s.song.timeline.as_deref_mut().unwrap();
        let c = t.clips.iter().find(|c| c.id == id).unwrap();
        let data = c.notes().unwrap();
        t.notes[data].make_mut().notes.push(crate::timeline::Note {
            start: 0,
            len: crate::timeline::PPQ * 2,
            key: 60,
            vel: 100,
            ..Default::default()
        });
    });
    let song = h.state().with(|s| s.song.clone());
    // the title menu is Save, Save As… and Export…; Save uses the app's save path
    h.get_by_label("Untitled •").click();
    frames(&mut h, 2);
    assert!(h.query_by_label("Save").is_some());
    assert!(h.query_by_label("Save As…").is_some());
    assert!(h.query_by_label("Export…").is_some());
    let cwt = std::env::temp_dir().join(format!("chipwave-workflow-{}.cwt", std::process::id()));
    let _ = std::fs::remove_file(&cwt);
    h.state_mut().path = Some(cwt.clone());
    assert!(h.state_mut().save_and_wait(), "the song saves");
    let loaded = fileio::load_cwt(&cwt).expect("the saved song opens");
    assert!(
        loaded
            .timeline
            .as_ref()
            .is_some_and(|t| t.clips.iter().any(|c| c.notes().is_some())),
        "the saved song still has the note clip"
    );
    let _ = std::fs::remove_file(&cwt);
    let path = std::env::temp_dir().join("chipwave-workflow-export.wav");
    let opts = crate::export::Opts {
        format: crate::export::Format::Wav,
        scope: crate::export::Scope::Song,
        ..crate::export::Opts::default()
    };
    let pr = crate::export::Progress::default();
    let msg = crate::export::export(&song, &opts, &path, &pr).expect("export");
    assert!(msg.to_lowercase().contains("wav") || path.exists(), "{msg}");
    let mut r = hound::WavReader::open(&path).expect("wav");
    let spec = r.spec();
    assert_eq!(spec.channels, 2);
    let samples: Vec<f32> = r.samples::<i16>().map(|s| s.unwrap() as f32).collect();
    assert!(samples.len() > 1000, "{}", samples.len());
    let peak = samples.iter().fold(0.0f32, |a, x| a.max(x.abs()));
    assert!(peak > 100.0, "silent export, peak {peak}");
    let artifacts = std::path::Path::new("/opt/cursor/artifacts");
    if artifacts.is_dir() {
        let _ = std::fs::copy(&path, artifacts.join("workflow-export.wav"));
    }
}

#[test]
fn add_track_opens_the_choice_and_an_instrument_opens_the_rack() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().new_song();
    frames(&mut h, 3);
    h.get_by_label("+ Add track").click();
    frames(&mut h, 2);
    assert!(matches!(
        h.state().opie.as_ref().map(|p| p.1),
        Some(PieFor::NewTrack)
    ));
    let labels: Vec<_> = h
        .state()
        .opie
        .as_ref()
        .unwrap()
        .0
        .items
        .iter()
        .map(|i| i.key.clone())
        .collect();
    assert!(labels.contains(&"instruments".into()));
    assert!(labels.contains(&"drums".into()));
    assert!(labels.contains(&"audio".into()));
    assert!(labels.contains(&"empty".into()));
    assert!(!labels.iter().any(|k| k.contains("fx-track")));
    pick(&mut h, "Instrument ›", Some("WaveForge"));
    frames(&mut h, 2);
    assert_eq!(h.state().with(|s| s.song.channels), 1);
    assert!(h.state().extra.lower_dev, "the Rack opens on the new sound");
    assert!(h.state().with(|s| {
        s.song
            .instruments
            .first()
            .is_some_and(|i| i.name.to_lowercase().contains("wave") || !i.name.is_empty())
    }));
}
