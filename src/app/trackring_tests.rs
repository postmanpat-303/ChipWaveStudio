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

fn info_as(class: TrackClass) -> TrackInfo {
    TrackInfo {
        class,
        kit: class == TrackClass::Drum,
        sounds: if class == TrackClass::Melodic {
            vec!["Saw Lead".into(), "Pluck".into()]
        } else {
            vec![]
        },
        sound: (class == TrackClass::Melodic).then(|| "Pluck".to_string()),
        ..info()
    }
}

fn wf(s: &str) -> &str {
    if s == "WaveForge" {
        crate::poly::POLY_DISPLAY_NAME
    } else {
        s
    }
}

const KIT_LABELS: [&str; 9] = [
    "Back",
    "909 Kit",
    "Psy Kit",
    "Techno Kit",
    "Acoustic-ish Kit",
    "Trap Kit",
    "Lo-fi Kit",
    "Drum Synth",
    "More kits…",
];

/// no drum in an Instrument ring, nothing melodic in a drum ring
fn assert_split(i: &TrackInfo) {
    let ins = ring_slots(Level::Instrument, i);
    for s in &ins {
        if let Some(Go::Act(PieAct::NewInstrument(b) | PieAct::NewTrackWith(b))) = s.go {
            assert!(!b.is_drum(), "Instrument › has no drum ({})", s.label);
        }
    }
    assert!(!labels(&ins)
        .iter()
        .any(|l| l.contains("Kit") || l == "Drum Synth"));
    let drum = ring_slots(Level::DrumInstr, i);
    assert_eq!(labels(&drum), KIT_LABELS);
    for s in &drum {
        if let Some(Go::Act(PieAct::DrumHere(b) | PieAct::DrumTrack(b))) = s.go {
            assert!(b.is_drum(), "drum ring has no melodic {}", s.label);
        }
    }
}

#[test]
fn drum_and_melodic_are_told_apart() {
    let drums: Vec<Builtin> = Builtin::ALL.into_iter().filter(|b| b.is_drum()).collect();
    assert_eq!(drums, [Builtin::DrumKit, Builtin::DrumSynth]);
    // `is_drum` agrees with what `make` builds (the pie's track context)
    for b in Builtin::ALL {
        let (_, k) = b.make().expect("a built-in");
        assert_eq!(is_drum_kind(&k), b.is_drum(), "{b:?}");
    }
    assert!(DRUM_INSTRS.iter().all(|b| b.is_drum()));
    assert!(melodic().all(|b| !b.is_drum()));
    assert_eq!(melodic().count() + drums.len(), Builtin::ALL.len());
    for k in RING_KITS {
        assert!(
            crate::drumpat::KIT_NAMES.iter().any(|(n, _)| *n == k),
            "{k} is a real kit"
        );
    }
}

/// the track pie follows the track: an empty one offers Instrument › and
/// Drum kit › first, a drum track its kits, a melodic one its sounds
#[test]
fn the_track_pie_follows_what_the_track_plays() {
    let empty = info_as(TrackClass::Empty);
    let add = ring_slots(Level::Add, &empty);
    assert_eq!(
        labels(&add),
        [
            "Instrument",
            "Drum kit",
            "Automation lane",
            "Browse…",
            "Mute",
            "Solo",
            "Duplicate",
            "Open in Rack",
        ]
    );
    assert_eq!(add[0].go, Some(Go::Open(Level::Instrument)));
    assert_eq!(add[1].go, Some(Go::Open(Level::DrumInstr)));
    assert_eq!(add[3].go, Some(Go::Act(PieAct::ChangeInstrument)));
    assert_split(&empty);

    let drum = info_as(TrackClass::Drum);
    let add = ring_slots(Level::Add, &drum);
    assert_eq!(
        labels(&add),
        [
            "Swap kit",
            "Drum pattern",
            "Dice kit",
            "Automation lane",
            "More kits…",
            "Mute",
            "Solo",
            "Duplicate",
            "Open in Rack",
        ]
    );
    assert_eq!(add[0].go, Some(Go::Open(Level::DrumInstr)));
    assert_eq!(add[1].go, Some(Go::Act(PieAct::NewPattern)));
    assert_eq!(add[2].go, Some(Go::Act(PieAct::DiceKit)));
    assert_eq!(add[4].go, Some(Go::Act(PieAct::MoreKitsHere)));
    assert!(
        !labels(&add)
            .iter()
            .any(|l| l == "Instrument" || l == "Sounds" || l == "Chords…"),
        "no melodic-only choices on a drum track"
    );
    // a Drum Synth track: no Dice kit (that is the Drum Kit's)
    let synth = TrackInfo {
        kit: false,
        ..drum.clone()
    };
    let d = ring_slots(Level::Add, &synth);
    assert_eq!(d[2].go, None);
    assert!(d[2].tip.contains("Drum Kit"));
    let ring = ring_slots(Level::DrumInstr, &drum);
    assert_eq!(ring[0].go, Some(Go::Back));
    assert_eq!(ring[1].go, Some(Go::Act(PieAct::DrumKitHere("909 Kit"))));
    assert_eq!(
        ring[7].go,
        Some(Go::Act(PieAct::DrumHere(Builtin::DrumSynth)))
    );
    assert_eq!(ring[8].go, Some(Go::Act(PieAct::MoreKitsHere)));
    assert_split(&drum);

    let mel = info_as(TrackClass::Melodic);
    let add = ring_slots(Level::Add, &mel);
    assert_eq!(
        labels(&add),
        [
            "Instrument",
            "Sounds",
            "New pattern",
            "Chords…",
            "Automation lane",
            "Browse…",
            "Mute",
            "Solo",
            "Duplicate",
            "Open in Rack",
        ]
    );
    assert_eq!(add[1].go, Some(Go::Open(Level::Sounds)));
    assert_eq!(add[2].go, Some(Go::Act(PieAct::NewPattern)));
    assert_eq!(add[3].go, Some(Go::Act(PieAct::Chords)));
    assert!(
        !labels(&add)
            .iter()
            .any(|l| l.contains("kit") || l.contains("Kit") || l.starts_with("Drum")),
        "no drum-only choices on a melodic track"
    );
    let snd = ring_slots(Level::Sounds, &mel);
    assert_eq!(labels(&snd), ["Back", "Saw Lead", "Pluck"]);
    assert!(snd[2].on && !snd[1].on, "the current preset ticked");
    assert_eq!(snd[1].go, Some(Go::Act(PieAct::Sound(0))));
    // no presets: Sounds › says why
    let bare = TrackInfo {
        sounds: vec![],
        ..mel.clone()
    };
    assert_eq!(ring_slots(Level::Add, &bare)[1].go, None);
    assert_split(&mel);
    let ins = ring_slots(Level::Instrument, &mel);
    let want: Vec<&str> = [
        "Back",
        "WaveForge",
        "StringWave",
        "Synth",
        "Analog",
        "FM-4",
        "Wavetable",
        "Grid",
        "Noise",
        "Chip FX",
        "Voice",
        "Browse…",
    ]
    .map(|s| {
        if s == "StringWave" {
            Builtin::StringWave.label()
        } else {
            wf(s)
        }
    })
    .to_vec();
    assert_eq!(labels(&ins), want);
    assert_eq!(
        ins[1].go,
        Some(Go::Act(PieAct::NewInstrument(Builtin::WaveForge)))
    );
    assert_eq!(
        ins.last().unwrap().go,
        Some(Go::Act(PieAct::ChangeInstrument))
    );

    // every context: hints everywhere, Back first on every inner ring,
    // nothing makes a new track
    for i in [&empty, &drum, &mel] {
        for l in [
            Level::Add,
            Level::Instrument,
            Level::DrumInstr,
            Level::Sounds,
        ] {
            let v = ring_slots(l, i);
            if l != Level::Add {
                assert_eq!(v[0].go, Some(Go::Back), "{l:?} keeps Back");
            }
            for s in v {
                assert!(!s.tip.is_empty(), "{} has a hint", s.label);
                assert!(
                    !matches!(
                        s.go,
                        Some(Go::Act(
                            PieAct::DrumTrack(_)
                                | PieAct::DrumKitTrack(_)
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
    }
}

#[test]
fn the_rings_hold_the_approved_entries() {
    let i = info_as(TrackClass::Melodic);
    let muted = TrackInfo {
        mute: true,
        ..i.clone()
    };
    let m = ring_slots(Level::Add, &muted);
    assert!(m[6].on && !m[7].on, "Mute lit when muted");
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
    assert_eq!(add[1].go, Some(Go::Open(Level::DrumInstr)));
    assert_eq!(add[5].go, None, "FX track: coming soon");
    // Drum pattern ›: straight to the kits, Drum Synth, More kits…
    assert_split(&new);
    let drum = ring_slots(Level::DrumInstr, &new);
    assert_eq!(drum[1].go, Some(Go::Act(PieAct::DrumKitTrack("909 Kit"))));
    assert_eq!(
        drum[7].go,
        Some(Go::Act(PieAct::DrumTrack(Builtin::DrumSynth)))
    );
    assert_eq!(drum[8].go, Some(Go::Act(PieAct::MoreKits)));
    let ins = ring_slots(Level::Instrument, &new);
    assert_eq!(
        ins[1].go,
        Some(Go::Act(PieAct::NewTrackWith(Builtin::WaveForge)))
    );
    assert_eq!(ins.last().unwrap().go, Some(Go::Act(PieAct::AddInstrument)));
    let all: Vec<&str> = melodic().map(|b| b.label()).collect();
    assert_eq!(labels(&ins[1..ins.len() - 1]), all);
    // no room for a track: nothing that adds one stays on
    let full = TrackInfo {
        full: true,
        ..i.clone()
    };
    assert_eq!(ring_slots(Level::Add, &full)[8].go, None, "no Duplicate");
    let full = TrackInfo { new: true, ..full };
    for l in [Level::Add, Level::Instrument, Level::DrumInstr] {
        for s in ring_slots(l, &full) {
            assert!(
                matches!(s.go, None | Some(Go::Back) | Some(Go::Open(_))),
                "{l:?} {}",
                s.label
            );
        }
    }
    // tracker mode: no patterns, no lanes
    for c in [TrackClass::Empty, TrackClass::Drum, TrackClass::Melodic] {
        let tracker = TrackInfo {
            timeline: false,
            ..info_as(c)
        };
        for s in ring_slots(Level::Add, &tracker) {
            if matches!(
                s.label.as_str(),
                "Drum kit"
                    | "Swap kit"
                    | "Drum pattern"
                    | "Dice kit"
                    | "New pattern"
                    | "Chords…"
                    | "Automation lane"
            ) {
                assert_eq!(s.go, None, "{c:?} {}", s.label);
                assert!(s.tip.contains("timeline mode"), "{}", s.tip);
            }
        }
    }
    // the 255 instrument slots used
    for new in [false, true] {
        let ifull = TrackInfo {
            instr_full: true,
            new,
            ..info_as(TrackClass::Drum)
        };
        let k = ring_slots(Level::DrumInstr, &ifull);
        assert!(k[1..k.len() - 1]
            .iter()
            .all(|s| s.go.is_none() && s.tip.contains("255")));
    }
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

/// a song with Lead (WaveForge), Bass (Analog), Drums (909 Kit) and an
/// empty track 04, as the shots' tracks-* views
fn harness_ctx() -> Harness<'static, App> {
    let mut h = harness_tl();
    {
        let a = h.state_mut();
        a.new_song();
        for (k, n) in ["Lead", "Bass", "Drums"].into_iter().enumerate() {
            a.track_insert(k, n);
        }
        a.view = View::Timeline;
        a.track_new_instrument(0, Builtin::WaveForge);
        a.track_new_instrument(1, Builtin::Analog);
        a.track_set_pattern_instrument(2, Builtin::DrumKit, Some("909 Kit"));
        assert!(a.track_add(3));
    }
    frames(&mut h, 3);
    h
}

/// the live App: each track's pie gets its context, and the actions work
#[test]
fn the_app_gives_each_track_its_context() {
    let mut h = harness_ctx();
    let a = h.state();
    assert_eq!(a.pie_info(0).class, TrackClass::Melodic);
    assert_eq!(a.pie_info(1).class, TrackClass::Melodic);
    let d = a.pie_info(2);
    assert_eq!((d.class, d.kit), (TrackClass::Drum, true));
    assert!(d.sounds.is_empty());
    assert_eq!(a.pie_info(3).class, TrackClass::Empty);
    let m = a.pie_info(0);
    assert!(!m.sounds.is_empty() && m.sounds.len() <= MAX_SOUNDS);
    assert_eq!(
        m.sound.as_deref(),
        Some("WaveForge Saw Lead"),
        "{:?}",
        m.sounds
    );
    // the ring opens with the track's own slot count, slot 1 on Add
    open(&mut h, 3);
    let ctx = h.ctx.clone();
    h.state_mut().pie_choose(add_seg(), &ctx);
    frames(&mut h, 1);
    let (_, _, a0, slots) = h.state().ring_now().unwrap();
    assert_eq!(labels(&slots)[..2], ["Instrument", "Drum kit"]);
    let d =
        (slot_angle(a0, slots.len(), 1) - seg_angle(add_seg())).rem_euclid(std::f32::consts::TAU);
    assert!(
        d < 1e-3 || d > std::f32::consts::TAU - 1e-3,
        "Drum kit › on Add"
    );
    h.state_mut().pie = None;
    // Sounds › on the melodic track: another preset, one undo
    let k = m
        .sounds
        .iter()
        .position(|n| Some(n) != m.sound.as_ref())
        .unwrap();
    h.state_mut().undo_reset();
    let slot = h.state().track_slot(0) as usize - 1;
    let before = h.state().with(|s| s.song.instruments[slot].clone());
    h.state_mut().pie_apply(0, PieAct::Sound(k as u8), &ctx);
    frames(&mut h, 2);
    let after = h.state().with(|s| s.song.instruments[slot].clone());
    assert_ne!(after.kind, before.kind, "the sound changed");
    assert_eq!(after.name, before.name, "the name stays");
    assert_eq!(
        h.state().pie_info(0).sound.as_deref(),
        Some(m.sounds[k].as_str())
    );
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(
        h.state().with(|s| s.song.instruments[slot].kind.clone()),
        before.kind
    );
    // Chords… on the melodic track: its pattern in the roll, the generator open
    h.state_mut().pie_apply(0, PieAct::Chords, &ctx);
    frames(&mut h, 2);
    assert!(h.state().pr.open, "the piano roll");
    assert!(h.state().cl.gen.open, "the chord generator");
    assert_eq!(h.state().tl_cur_track(), 0);
    // Dice kit on the drum track: the kit changes, one undo takes it back
    h.state_mut().cl.gen.open = false;
    h.state_mut().undo_reset();
    let ks = h.state().track_slot(2) as usize - 1;
    let k0 = h.state().with(|s| s.song.instruments[ks].kind.clone());
    h.state_mut().pie_apply(2, PieAct::DiceKit, &ctx);
    frames(&mut h, 4);
    let k1 = h.state().with(|s| s.song.instruments[ks].kind.clone());
    assert_ne!(k1, k0, "Dice kit rolled: {}", h.state().status);
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(h.state().with(|s| s.song.instruments[ks].kind.clone()), k0);
    // the empty track: Drum kit › 909 makes it a drum track
    h.state_mut()
        .pie_apply(3, PieAct::DrumKitHere("909 Kit"), &ctx);
    frames(&mut h, 2);
    assert_eq!(h.state().pie_info(3).class, TrackClass::Drum);
}

/// Add opens level 1 with its second slot (an empty track's Drum kit ›)
/// on the Add segment; Enter goes in with the likely next choice where
/// the parent was and Back beside it; Esc / Backspace step back one
/// level, then to the plain pie
#[test]
fn rings_go_in_and_back() {
    let mut h = harness_ctx();
    open(&mut h, 3);
    let ctx = h.ctx.clone();
    h.state_mut().pie_choose(add_seg(), &ctx);
    frames(&mut h, 1);
    assert_eq!(level(&h), Some(Level::Add));
    let r = h.state().pie.as_ref().unwrap().ring.clone().unwrap();
    assert_eq!(r.key, Some(1));
    let at = slot_angle(r.a0, 8, 1);
    let d = (at - seg_angle(add_seg())).rem_euclid(std::f32::consts::TAU);
    assert!(
        d < 1e-3 || d > std::f32::consts::TAU - 1e-3,
        "on the Add segment"
    );
    // Enter: Drum kit ›
    rkey(&mut h, Key::Enter);
    assert_eq!(level(&h), Some(Level::DrumInstr));
    let (_, _, a0, slots) = h.state().ring_now().unwrap();
    let d = (slot_angle(a0, slots.len(), 1) - at).rem_euclid(std::f32::consts::TAU);
    assert!(
        d < 1e-3 || d > std::f32::consts::TAU - 1e-3,
        "909 Kit where Drum kit › was"
    );
    assert_eq!(
        h.state().pie.as_ref().unwrap().ring.as_ref().unwrap().key,
        Some(1)
    );
    // arrows walk the ring and wrap
    rkey(&mut h, Key::ArrowLeft);
    rkey(&mut h, Key::ArrowLeft);
    assert_eq!(
        h.state().pie.as_ref().unwrap().ring.as_ref().unwrap().key,
        Some(8),
        "More kits… is next to Back"
    );
    // Esc: back to Add, the way it was
    rkey(&mut h, Key::Escape);
    assert_eq!(level(&h), Some(Level::Add));
    assert_eq!(
        h.state().pie.as_ref().unwrap().ring.as_ref().unwrap().key,
        Some(1)
    );
    // in again, then ‹ Back (slot 0) by Enter
    rkey(&mut h, Key::Enter);
    assert_eq!(level(&h), Some(Level::DrumInstr));
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

/// Drum kit › Psy Kit by the pointer on the empty track 04: THAT track
/// plays the kit and its pattern opens in the step sequencer, no new
/// track; one undo takes it all away
#[test]
fn a_kit_from_the_track_pie_goes_on_that_track_one_undo() {
    let mut h = harness_ctx();
    h.state_mut().undo_reset();
    let before = counts(&h);
    open(&mut h, 3);
    let c = h.state().pie.as_ref().unwrap().centre;
    let a = seg_angle(add_seg());
    // onto Add: the ring opens
    for r in [60.0, 90.0, 110.0] {
        let p = c + egui::vec2(a.cos(), a.sin()) * r;
        h.input_mut().events.push(egui::Event::PointerMoved(p));
        frames(&mut h, 3);
    }
    assert_eq!(level(&h), Some(Level::Add));
    let p = ring_pos(&h, 1);
    super::timeline_m2_tests::click_at(&mut h, p);
    frames(&mut h, 2);
    assert_eq!(level(&h), Some(Level::DrumInstr));
    let p = ring_pos(&h, 2);
    super::timeline_m2_tests::click_at(&mut h, p);
    frames(&mut h, 4);
    assert!(h.state().pie.is_none(), "a pick closes the pie");
    let after = counts(&h);
    assert_eq!(after.0, before.0, "NO new track");
    assert_eq!(after.1, before.1 + 1, "one new instrument");
    let (slot, kit) = h.state().with(|s| {
        let slot = s.song.timeline.as_deref().unwrap().tracks[3].instr;
        let i = &s.song.instruments[slot as usize - 1];
        (
            slot,
            crate::drumkit::kit_of(i).is_some() && i.name.contains("Psy"),
        )
    });
    assert!(kit, "track 04 plays the Psy kit (slot {slot})");
    assert_eq!(h.state().tl_cur_track(), 3, "track 04 active");
    let clips = track_clips(&h, 3);
    assert!(!clips.is_empty());
    let open = h.state().dk.clip.expect("in the step sequencer");
    assert!(clips.iter().any(|c| c.0 == open), "a pattern on track 04");
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

/// the empty-area pie: Drum pattern › 909 adds ONE track at
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

/// Drum pattern › Drum Synth: the same, open in the piano roll
#[test]
fn a_drum_instrument_from_the_rings_opens_the_piano_roll() {
    let mut h = harness_tl();
    h.state_mut().undo_reset();
    let before = counts(&h);
    let ctx = h.ctx.clone();
    h.state_mut()
        .pie_apply(2, PieAct::DrumTrack(Builtin::DrumSynth), &ctx);
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
    // More kits… then Instrument track…: that pick is a track only
    h.state_mut().pie_apply(0, PieAct::MoreKits, &ctx);
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
