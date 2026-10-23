//! The track pie's Add rings (app/trackring.rs): the slots of each level,
//! going in and back (keys and pointer), a new track with an empty 1-bar
//! pattern (one undo), the Browser paths and the edge clamp
use super::radial::slot_angle;
use super::trackpie::*;
use super::trackring::*;
use super::ui_tests::{frame, harness_with};
use super::*;
use egui::Key;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn harness_tl() -> Harness<'static, App> {
    let mut h = harness_with({
        let mut s = fileio::demo_song();
        crate::timeline::enter_timeline_mode(&mut s);
        s
    });
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.reduce_motion = true;
    frames(&mut h, 3);
    h
}

fn open(h: &mut Harness<'static, App>, tr: usize) {
    let ctx = h.ctx.clone();
    h.state_mut()
        .open_track_pie(tr, Pos2::new(600.0, 400.0), false, &ctx);
    frames(h, 2);
}

fn rkey(h: &mut Harness<'static, App>, k: Key) {
    let ctx = h.ctx.clone();
    h.state_mut().ring_key(k, &ctx);
    frames(h, 1);
}

fn level(h: &Harness<'static, App>) -> Option<Level> {
    h.state().ring_now().map(|r| r.1)
}

fn labels(s: &[Slot]) -> Vec<String> {
    s.iter().map(|s| s.label.clone()).collect()
}

/// the middle of slot `k` of the open ring, on screen
pub(super) fn ring_pos(h: &Harness<'static, App>, k: usize) -> Pos2 {
    let (_, _, a0, slots) = h.state().ring_now().expect("a ring");
    let c = h.state().pie.as_ref().unwrap().centre;
    let a = slot_angle(a0, slots.len(), k);
    c + egui::vec2(a.cos(), a.sin()) * ((R1 + R2) / 2.0)
}

fn info() -> TrackInfo {
    TrackInfo {
        tracks: 4,
        timeline: true,
        ..Default::default()
    }
}

#[test]
fn the_rings_hold_the_approved_entries() {
    let i = info();
    assert_eq!(
        labels(&ring_slots(Level::Add, &i)),
        [
            "Instrument",
            "Drum pattern",
            "Automation lane",
            "Browse…",
            "New pattern",
            "Mute",
            "Solo",
            "Duplicate",
            "Open in Rack",
        ]
    );
    let add = ring_slots(Level::Add, &i);
    assert_eq!(add[0].go, Some(Go::Open(Level::Instrument)));
    assert_eq!(add[1].go, Some(Go::Open(Level::DrumInstr)));
    assert_eq!(add[2].go, Some(Go::Open(Level::Automation)));
    assert_eq!(add[3].go, Some(Go::Act(PieAct::ChangeInstrument)));
    assert_eq!(add[4].go, Some(Go::Act(PieAct::NewPattern)));
    assert_eq!(add[5].go, Some(Go::Act(PieAct::Mute)));
    assert_eq!(add[8].go, Some(Go::Act(PieAct::Devices)));
    let muted = TrackInfo {
        mute: true,
        ..info()
    };
    let m = ring_slots(Level::Add, &muted);
    assert!(m[5].on && !m[6].on, "Mute lit when muted");
    assert!(
        add.iter().all(|s| !s.tip.is_empty()),
        "every slot has a hint"
    );
    let drum = ring_slots(Level::DrumInstr, &i);
    assert_eq!(
        labels(&drum),
        [
            "Back",
            "Drum Kit",
            "Drum Synth",
            "Grid",
            "Noise",
            "Chip FX",
            "WaveForge",
            "Synth",
            "Analog",
            "FM-4",
            "Wavetable",
            "Voice",
            "Browse…",
        ]
        .map(|s| if s == "WaveForge" {
            crate::poly::POLY_DISPLAY_NAME
        } else {
            s
        })
    );
    assert_eq!(drum[0].go, Some(Go::Back));
    assert_eq!(drum[1].go, Some(Go::Open(Level::Kits)));
    // the track pie's rings act on THAT track: nothing makes a new one
    assert_eq!(drum[4].go, Some(Go::Act(PieAct::DrumHere(Builtin::Noise))));
    assert_eq!(drum[12].go, Some(Go::Act(PieAct::ChangeInstrument)));
    let kits = ring_slots(Level::Kits, &i);
    assert_eq!(
        labels(&kits),
        [
            "Back",
            "909 Kit",
            "Psy Kit",
            "Techno Kit",
            "Acoustic-ish Kit",
            "Trap Kit",
            "Lo-fi Kit",
            "More kits…",
        ]
    );
    for k in RING_KITS {
        assert!(
            crate::drumpat::KIT_NAMES.iter().any(|(n, _)| *n == k),
            "{k} is a real kit"
        );
    }
    assert_eq!(kits[1].go, Some(Go::Act(PieAct::DrumKitHere("909 Kit"))));
    assert_eq!(kits[7].go, Some(Go::Act(PieAct::MoreKitsHere)));
    for l in [Level::Add, Level::Instrument, Level::DrumInstr, Level::Kits] {
        for s in ring_slots(l, &i) {
            assert!(
                !matches!(
                    s.go,
                    Some(Go::Act(
                        PieAct::DrumTrack(_)
                            | PieAct::DrumKitTrack(_)
                            | PieAct::DrumBrowse
                            | PieAct::MoreKits
                            | PieAct::NewTrackWith(_)
                            | PieAct::AddAudio
                            | PieAct::AddEmpty(_)
                            | PieAct::AddInstrument
                    ))
                ),
                "track pie {l:?} {} makes no new track",
                s.label
            );
        }
    }
    // the empty-area pie: new tracks
    let new = TrackInfo {
        new: true,
        ..info()
    };
    let add = ring_slots(Level::Add, &new);
    assert_eq!(
        labels(&add),
        [
            "Instrument track",
            "Drum pattern",
            "Audio track",
            "Empty track",
            "Browse…",
            "FX track",
        ]
    );
    assert_eq!(add[0].go, Some(Go::Open(Level::Instrument)));
    assert_eq!(add[5].go, None, "FX track: coming soon");
    let drum = ring_slots(Level::DrumInstr, &new);
    assert_eq!(drum[4].go, Some(Go::Act(PieAct::DrumTrack(Builtin::Noise))));
    assert_eq!(drum[12].go, Some(Go::Act(PieAct::DrumBrowse)));
    let kits = ring_slots(Level::Kits, &new);
    assert_eq!(kits[1].go, Some(Go::Act(PieAct::DrumKitTrack("909 Kit"))));
    assert_eq!(kits[7].go, Some(Go::Act(PieAct::MoreKits)));
    let ins = ring_slots(Level::Instrument, &new);
    assert_eq!(
        ins[1].go,
        Some(Go::Act(PieAct::NewTrackWith(Builtin::ALL[0])))
    );
    assert_eq!(ins.last().unwrap().go, Some(Go::Act(PieAct::AddInstrument)));
    let ins = ring_slots(Level::Instrument, &i);
    let all: Vec<&str> = Builtin::ALL.iter().map(|b| b.label()).collect();
    assert_eq!(labels(&ins[1..ins.len() - 1]), all);
    assert_eq!(
        ins.last().unwrap().go,
        Some(Go::Act(PieAct::ChangeInstrument))
    );
    // no room for a track: nothing that adds one stays on
    let full = TrackInfo {
        full: true,
        ..info()
    };
    assert_eq!(ring_slots(Level::Add, &full)[7].go, None, "no Duplicate");
    let full = TrackInfo { new: true, ..full };
    for l in [Level::Add, Level::Instrument, Level::DrumInstr, Level::Kits] {
        for s in ring_slots(l, &full) {
            assert!(
                matches!(s.go, None | Some(Go::Back) | Some(Go::Open(_))),
                "{l:?} {}",
                s.label
            );
        }
    }
    // tracker mode: no patterns, no lanes
    let tracker = TrackInfo {
        timeline: false,
        ..info()
    };
    let add = ring_slots(Level::Add, &tracker);
    assert_eq!(add[1].go, None);
    assert!(add[1].tip.contains("timeline mode"));
    assert_eq!(add[2].go, None);
    assert_eq!(add[4].go, None);
    // the 255 instrument slots used
    let ifull = TrackInfo {
        instr_full: true,
        ..info()
    };
    let k = ring_slots(Level::Kits, &ifull);
    assert!(k[1..k.len() - 1]
        .iter()
        .all(|s| s.go.is_none() && s.tip.contains("255")));
    let ifull = TrackInfo { new: true, ..ifull };
    assert!(ring_slots(Level::Kits, &ifull)[1..]
        .iter()
        .all(|s| s.go.is_none() && s.tip.contains("255")));
    // automation: the track's lanes as toggles
    let lanes = TrackInfo {
        lanes: vec![("Cutoff".into(), true), ("Pan".into(), false)],
        lanes_shown: true,
        ..info()
    };
    let au = ring_slots(Level::Automation, &lanes);
    assert_eq!(
        labels(&au),
        ["Back", "New lane…", "Hide lanes", "Cutoff", "Pan"]
    );
    assert!(au[3].on && !au[4].on);
    assert_eq!(au[4].go, Some(Go::Act(PieAct::AutoLane(1))));
}

/// Add opens level 1 with Drum pattern › on the Add segment; Enter goes
/// in with the likely next choice where the parent was and Back beside
/// it; Esc / Backspace step back one level, then to the plain pie
#[test]
fn rings_go_in_and_back() {
    let mut h = harness_tl();
    open(&mut h, 0);
    let ctx = h.ctx.clone();
    h.state_mut().pie_choose(add_seg(), &ctx);
    frames(&mut h, 1);
    assert_eq!(level(&h), Some(Level::Add));
    let r = h.state().pie.as_ref().unwrap().ring.clone().unwrap();
    assert_eq!(r.key, Some(1));
    let at = slot_angle(r.a0, 9, 1);
    let d = (at - seg_angle(add_seg())).rem_euclid(std::f32::consts::TAU);
    assert!(
        d < 1e-3 || d > std::f32::consts::TAU - 1e-3,
        "on the Add segment"
    );
    // Enter: Drum pattern ›
    rkey(&mut h, Key::Enter);
    assert_eq!(level(&h), Some(Level::DrumInstr));
    let (_, _, a0, slots) = h.state().ring_now().unwrap();
    let d = (slot_angle(a0, slots.len(), 1) - at).rem_euclid(std::f32::consts::TAU);
    assert!(
        d < 1e-3 || d > std::f32::consts::TAU - 1e-3,
        "Drum Kit › where Drum pattern › was"
    );
    assert_eq!(
        h.state().pie.as_ref().unwrap().ring.as_ref().unwrap().key,
        Some(1)
    );
    // Enter again: the kits
    rkey(&mut h, Key::Enter);
    assert_eq!(level(&h), Some(Level::Kits));
    // arrows walk the ring and wrap
    rkey(&mut h, Key::ArrowLeft);
    rkey(&mut h, Key::ArrowLeft);
    assert_eq!(
        h.state().pie.as_ref().unwrap().ring.as_ref().unwrap().key,
        Some(7),
        "More kits… is next to Back"
    );
    // Esc: back to Drum pattern ›, the way it was
    rkey(&mut h, Key::Escape);
    assert_eq!(level(&h), Some(Level::DrumInstr));
    assert_eq!(
        h.state().pie.as_ref().unwrap().ring.as_ref().unwrap().key,
        Some(1)
    );
    // ‹ Back (slot 0) by Enter
    rkey(&mut h, Key::ArrowLeft);
    rkey(&mut h, Key::Enter);
    assert_eq!(level(&h), Some(Level::Add));
    // Backspace at level 1: the plain pie, Add lit, still open
    rkey(&mut h, Key::Backspace);
    assert!(h
        .state()
        .pie
        .as_ref()
        .is_some_and(|p| p.ring.is_none() && p.key == Some(add_seg()) && p.closing.is_none()));
    // a disabled slot only says why (the empty-area pie's FX track)
    h.state_mut().pie = None;
    h.state_mut()
        .open_add_pie(Pos2::new(600.0, 400.0), false, &ctx);
    frames(&mut h, 2);
    assert_eq!(level(&h), Some(Level::Add));
    for _ in 0..4 {
        rkey(&mut h, Key::ArrowRight);
    }
    rkey(&mut h, Key::Enter);
    assert!(
        h.state().status.contains("coming soon"),
        "{}",
        h.state().status
    );
    assert_eq!(level(&h), Some(Level::Add));
    // Backspace at its first ring closes the empty-area pie
    rkey(&mut h, Key::Backspace);
    frames(&mut h, 3);
    assert!(h.state().pie.as_ref().is_none_or(|p| p.closing.is_some()));
}

fn track_clips(h: &Harness<'static, App>, tr: usize) -> Vec<(u32, i64, i64, usize)> {
    h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        t.clips
            .iter()
            .filter(|c| c.track as usize == tr)
            .map(|c| {
                let n = match c.kind {
                    crate::timeline::ClipKind::Notes { data } => t.notes[data as usize].notes.len(),
                    _ => 99,
                };
                (c.id, c.start, c.len, n)
            })
            .collect()
    })
}

fn counts(h: &Harness<'static, App>) -> (usize, usize, usize) {
    h.state().with(|s| {
        (
            s.song.channels,
            s.song.instruments.len(),
            s.song.timeline.as_deref().unwrap().clips.len(),
        )
    })
}

/// Drum pattern › Drum Kit › Psy Kit by the pointer on track 01: THAT
/// track plays the kit and its pattern opens in the step sequencer, no new
/// track; one undo takes it all away
#[test]
fn a_kit_from_the_track_pie_goes_on_that_track_one_undo() {
    let mut h = harness_tl();
    h.state_mut().undo_reset();
    let before = counts(&h);
    open(&mut h, 0);
    let c = h.state().pie.as_ref().unwrap().centre;
    let a = seg_angle(add_seg());
    // onto Add: the ring opens
    for r in [60.0, 90.0, 110.0] {
        let p = c + egui::vec2(a.cos(), a.sin()) * r;
        h.input_mut().events.push(egui::Event::PointerMoved(p));
        frames(&mut h, 3);
    }
    assert_eq!(level(&h), Some(Level::Add));
    for (k, want) in [(1, Level::DrumInstr), (1, Level::Kits)] {
        let p = ring_pos(&h, k);
        super::timeline_m2_tests::click_at(&mut h, p);
        frames(&mut h, 2);
        assert_eq!(level(&h), Some(want));
    }
    let p = ring_pos(&h, 2);
    super::timeline_m2_tests::click_at(&mut h, p);
    frames(&mut h, 4);
    assert!(h.state().pie.is_none(), "a pick closes the pie");
    let after = counts(&h);
    assert_eq!(after.0, before.0, "NO new track");
    assert_eq!(after.1, before.1 + 1, "one new instrument");
    let (slot, kit) = h.state().with(|s| {
        let slot = s.song.timeline.as_deref().unwrap().tracks[0].instr;
        let i = &s.song.instruments[slot as usize - 1];
        (
            slot,
            crate::drumkit::kit_of(i).is_some() && i.name.contains("Psy"),
        )
    });
    assert!(kit, "track 01 plays the Psy kit (slot {slot})");
    assert_eq!(h.state().tl_cur_track(), 0, "track 01 active");
    let clips = track_clips(&h, 0);
    assert!(!clips.is_empty());
    let open = h.state().dk.clip.expect("in the step sequencer");
    assert!(clips.iter().any(|c| c.0 == open), "a pattern on track 01");
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(counts(&h), before, "one undo");
}

/// a kit on a track with no pattern: an empty 1-bar pattern at the play
/// marker gets the kit's starter groove; still no new track
#[test]
fn a_kit_on_an_empty_track_writes_a_starter_groove() {
    let mut h = harness_tl();
    let (bar, cue) = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        let bar = t.sig_at(0).bar_ticks();
        (bar, bar * 2 + bar / 3)
    });
    assert!(h.state_mut().track_add(1));
    frames(&mut h, 2);
    h.state_mut().tlv.cue = cue;
    h.state_mut().undo_reset();
    let before = counts(&h);
    let ctx = h.ctx.clone();
    h.state_mut()
        .pie_apply(1, PieAct::DrumKitHere("909 Kit"), &ctx);
    frames(&mut h, 2);
    let after = counts(&h);
    assert_eq!(after.0, before.0, "NO new track");
    assert_eq!(after.2, before.2 + 1, "one new clip");
    let clips = track_clips(&h, 1);
    assert_eq!(clips.len(), 1);
    let (id, start, len, notes) = clips[0];
    assert_eq!((start, len), (bar * 2, bar), "1 bar at bar 3");
    assert!(notes > 0, "a starter groove");
    assert_eq!(h.state().dk.clip, Some(id), "in the step sequencer");
    assert_eq!(h.state().tl_cur_track(), 1);
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(counts(&h), before, "one undo");
}

/// the empty-area pie: Drum pattern › Drum Kit › 909 adds ONE track at
/// the end with the kit and an empty 1-bar pattern; Empty track adds one
#[test]
fn the_empty_area_pie_adds_tracks_at_the_end() {
    let mut h = harness_tl();
    h.state_mut().undo_reset();
    let before = counts(&h);
    h.state_mut().pie_apply_new(PieAct::DrumKitTrack("909 Kit"));
    frames(&mut h, 2);
    let after = counts(&h);
    assert_eq!(after.0, before.0 + 1, "one new track");
    let last = after.0 - 1;
    let clips = track_clips(&h, last);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].3, 0, "empty");
    assert_eq!(h.state().tl_cur_track(), last, "the new track active");
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(counts(&h), before, "one undo");
    h.state_mut().pie_apply_new(PieAct::AddEmpty(false));
    frames(&mut h, 2);
    assert_eq!(counts(&h).0, before.0 + 1);
    assert_eq!(h.state().tl_cur_track(), before.0);
    // the pie opens on its ring with no track segments
    let ctx = h.ctx.clone();
    h.state_mut()
        .open_add_pie(Pos2::new(600.0, 400.0), false, &ctx);
    frames(&mut h, 2);
    assert!(h.state().pie.as_ref().is_some_and(|p| p.empty));
    assert_eq!(
        labels(&h.state().ring_now().unwrap().3)[0],
        "Instrument track"
    );
}

/// Drum pattern › Noise: the same, open in the piano roll
#[test]
fn a_drum_instrument_from_the_rings_opens_the_piano_roll() {
    let mut h = harness_tl();
    h.state_mut().undo_reset();
    let before = counts(&h);
    let ctx = h.ctx.clone();
    h.state_mut()
        .pie_apply(2, PieAct::DrumTrack(Builtin::Noise), &ctx);
    frames(&mut h, 2);
    let clips = track_clips(&h, 3);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].3, 0, "empty");
    assert!(h.state().pr.open, "the piano roll");
    assert_eq!(counts(&h).0, before.0 + 1);
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(counts(&h), before);
    // tracker mode: says why, changes nothing
    let mut h = harness_with(fileio::demo_song());
    frames(&mut h, 2);
    let n = h.state().with(|s| s.song.channels);
    let ctx = h.ctx.clone();
    h.state_mut()
        .pie_apply(0, PieAct::DrumKitTrack("909 Kit"), &ctx);
    frames(&mut h, 2);
    assert_eq!(h.state().with(|s| s.song.channels), n);
    assert!(
        h.state().status.contains("timeline"),
        "{}",
        h.state().status
    );
}

/// More kits…: the Browser on Drum Kits; the pick goes on a new track
/// below with an empty 1-bar pattern, one undo. A plain Instrument
/// track… pick afterwards gets no pattern
#[test]
fn more_kits_goes_through_the_browser() {
    let mut h = harness_tl();
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(0, PieAct::MoreKits, &ctx);
    frames(&mut h, 2);
    {
        let a = h.state();
        assert!(a.extra.browser_open);
        assert_eq!(a.br.filter.tab, crate::browser::Tab::Instruments);
        assert_eq!(a.br.filter.category.as_deref(), Some("Drum Kit"));
        assert_eq!(a.br.new_track_at, Some(1));
        assert!(a.br.new_track_pattern);
    }
    h.state_mut().undo_reset();
    let before = counts(&h);
    let it = crate::browser::builtin_items()
        .into_iter()
        .find(|i| i.category == "Drum Kit" && matches!(i.src, crate::browser::Src::Gen(_)))
        .expect("a built-in kit");
    h.state_mut().br_load(&it, &ctx);
    frames(&mut h, 3);
    let after = counts(&h);
    assert_eq!((after.0, after.2), (before.0 + 1, before.2 + 1));
    let clips = track_clips(&h, 1);
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].3, 0, "empty");
    assert_eq!(h.state().dk.clip, Some(clips[0].0));
    assert!(!h.state().br.new_track_pattern);
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(counts(&h), before, "one undo");
    // Browse… then Instrument track…: that pick is a track only
    h.state_mut().pie_apply(0, PieAct::DrumBrowse, &ctx);
    h.state_mut().pie_apply(0, PieAct::AddInstrument, &ctx);
    assert!(!h.state().br.new_track_pattern);
    h.state_mut().br_load(&it, &ctx);
    frames(&mut h, 3);
    let now = counts(&h);
    assert_eq!((now.0, now.2), (before.0 + 1, before.2), "no pattern");
}

/// Automation lane ›: the track's lanes switch on and off from the ring
#[test]
fn automation_ring_toggles_lanes() {
    let mut h = harness_tl();
    let tr = 1;
    let i = h.state().pie_info(tr);
    assert!(!i.lanes.is_empty(), "the demo's track 02 has a lane");
    let on = i.lanes[0].1;
    let ctx = h.ctx.clone();
    h.state_mut().pie_apply(tr, PieAct::AutoLane(0), &ctx);
    frames(&mut h, 2);
    assert_eq!(h.state().pie_info(tr).lanes[0].1, !on);
    let shown = h.state().track_lanes_shown(tr);
    h.state_mut().pie_apply(tr, PieAct::AutoToggle, &ctx);
    frames(&mut h, 2);
    assert_eq!(h.state().track_lanes_shown(tr), !shown);
}

/// the pie's centre keeps the rings on a 1366x768 window wherever it opens
#[test]
fn rings_fit_at_1366x768() {
    let sc = Rect::from_min_size(Pos2::ZERO, egui::vec2(1366.0, 768.0));
    for x in [0.0, 5.0, 300.0, 683.0, 1100.0, 1360.0, 1366.0] {
        for y in [0.0, 5.0, 200.0, 384.0, 600.0, 760.0, 768.0] {
            let c = clamp_centre(Pos2::new(x, y), sc);
            let disc = Rect::from_center_size(c, egui::Vec2::splat(2.0 * (R2 + 4.0)));
            assert!(sc.contains_rect(disc), "({x},{y}) -> {c:?}");
        }
    }
    // in the middle it stays under the pointer
    assert_eq!(
        clamp_centre(Pos2::new(683.0, 384.0), sc),
        Pos2::new(683.0, 384.0)
    );
}
