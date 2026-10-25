//! The Generators panel's Write to timeline, end to end through the UI:
//! where the new clip goes (the track, a new track, the open clip), its
//! length and notes, the one undo step, the seed and the card's text.
use super::genpanel::{ChordDest, GenTab};
use super::pianoview::Target;
use super::trackpie::Builtin;
use super::ui_tests::{frame, harness_with, key};
use super::*;
use crate::timeline as tl;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

const BAR: tl::Tick = 4 * tl::PPQ;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// Lead (WaveForge), Bass (Analog), Drums (a 909 kit with its 1-bar
/// pattern) on the Timeline at 1366 x 768
fn song3() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.set_size(egui::vec2(1366.0, 768.0));
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
        a.undo_reset();
    }
    frames(&mut h, 3);
    h
}

fn empty_song() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.set_size(egui::vec2(1366.0, 768.0));
    h.state_mut().new_song();
    h.state_mut().view = View::Timeline;
    h.state_mut().undo_reset();
    frames(&mut h, 3);
    h
}

/// a note clip as the tests see it
#[derive(Debug, Clone, PartialEq)]
struct C {
    id: u32,
    track: usize,
    start: tl::Tick,
    len: tl::Tick,
    loop_len: Option<tl::Tick>,
    name: Option<String>,
    drum: bool,
    /// (data tick, tracker key) in data order
    notes: Vec<(tl::Tick, u8)>,
    chords: Vec<(tl::Tick, tl::Tick, Vec<u8>)>,
}

fn clips(h: &Harness<'static, App>) -> Vec<C> {
    h.state().with(|s| {
        let Some(t) = s.song.timeline.as_deref() else {
            return vec![];
        };
        t.clips
            .iter()
            .filter_map(|c| {
                let d = &t.notes[c.notes()?];
                Some(C {
                    id: c.id,
                    track: c.track as usize,
                    start: c.start,
                    len: c.len,
                    loop_len: c.loop_len,
                    name: c.name.clone(),
                    drum: d.drum.is_some(),
                    notes: d.notes.iter().map(|n| (n.start, n.key)).collect(),
                    chords: d.chords.iter().map(|m| (m.start, m.len, m.pcs())).collect(),
                })
            })
            .collect()
    })
}

fn counts(h: &Harness<'static, App>) -> (usize, usize, usize) {
    let n = clips(h).len();
    h.state()
        .with(|s| (s.song.channels, s.song.instruments.len(), n))
}

fn new_clip(before: &[C], after: &[C]) -> C {
    let mut n: Vec<_> = after
        .iter()
        .filter(|c| !before.iter().any(|b| b.id == c.id))
        .collect();
    assert_eq!(n.len(), 1, "one new clip: {after:#?}");
    n.remove(0).clone()
}

/// fixed, plain settings: Pop in C major, no colour / substitutions
fn plain(h: &mut Harness<'static, App>, bars: u32, seed: u64) {
    let o = &mut h.state_mut().cl.gen.opts;
    o.style = crate::chordgen::STYLES
        .iter()
        .position(|s| s.name == "Pop")
        .unwrap();
    o.root = 0;
    o.mode = crate::chordgen::Mode::Major;
    o.bars = bars;
    o.seed = seed;
    o.colour = 0.0;
    o.variation = 0.0;
    o.prog = None;
    frames(h, 2);
}

fn click_write(h: &mut Harness<'static, App>) {
    frames(h, 2);
    assert!(h.state().cl.gen.open, "the panel is open");
    h.get_by_label("Write to timeline").click();
    frames(h, 3);
}

fn undo(h: &mut Harness<'static, App>) {
    // Ctrl+Z, as typed
    key(h, egui::Key::Z, egui::Modifiers::COMMAND);
    frames(h, 3);
}

fn status(h: &Harness<'static, App>) -> String {
    h.state().status.clone()
}

/// every note is in the key (C major) and in the chord sounding at it
fn assert_notes_fit(c: &C) {
    assert!(!c.notes.is_empty(), "notes written: {c:?}");
    assert!(!c.chords.is_empty(), "chord markers written: {c:?}");
    let key = crate::chords::Key::new(0, crate::chords::Scale::Major);
    for &(t, k) in &c.notes {
        let pc = (k - 1) % 12;
        assert!(
            key.contains(pc),
            "note {k} at {t} is in C major: {:?}",
            c.chords
        );
        let m = c.chords.iter().find(|m| m.0 <= t && t < m.0 + m.1);
        let m = m.unwrap_or_else(|| panic!("a chord sounds at {t}"));
        assert!(
            m.2.contains(&pc),
            "note {k} at {t} is in its chord {:?}",
            m.2
        );
    }
    let end = c.notes.iter().map(|n| n.0).max().unwrap();
    assert!(
        end < c.loop_len.unwrap_or(c.len),
        "every note is inside the clip"
    );
}

/// Scenario 1: the track pie's Chords… on a melodic track with no clips, the play
/// marker inside bar 5: Write makes an 8-bar clip of notes there
#[test]
fn chords_on_a_melodic_track_write_at_the_play_marker() {
    let mut h = song3();
    h.state_mut().tlv.cue = 4 * BAR + tl::PPQ; // bar 5, beat 2
    h.state_mut().track_chords(0);
    plain(&mut h, 8, 7);
    assert_eq!(h.state().cl.gen.dest, ChordDest::Track);
    assert_eq!(h.state().cl.gen.at, 4 * BAR, "the play marker's bar");
    let before = clips(&h);
    let n0 = counts(&h);
    click_write(&mut h);
    let c = new_clip(&before, &clips(&h));
    assert_eq!((c.track, c.start, c.len), (0, 4 * BAR, 8 * BAR));
    assert_eq!(c.name.as_deref(), Some("Chords"));
    assert!(
        c.chords.first().is_some_and(|m| m.0 == 0),
        "the first chord at the clip start"
    );
    assert_notes_fit(&c);
    assert_eq!(counts(&h).0, n0.0, "no new track");
    assert!(!h.state().cl.gen.open, "the panel closes");
    let s = status(&h);
    assert!(
        s.contains("track 01") && s.contains("song bars 5–12") && s.contains("Ctrl+Z"),
        "{s}"
    );
    // the new clip is open in the piano roll: it is seen
    assert!(h.state().pr.open);
    assert!(
        matches!(h.state().pr_target(), Some(Target::Notes { clip: Some(id), .. }) if id == c.id)
    );
    // one undo takes it all back
    undo(&mut h);
    assert_eq!(counts(&h), n0);
    assert_eq!(clips(&h), before);
}

/// Scenario 2: no track selected (the Song / Master track): the timeline's
/// Chords… button, Write adds a new "Chords" track with a synth
#[test]
fn chords_with_no_track_selected_make_a_new_track() {
    let mut h = song3();
    h.state_mut().tl_select_track(3, false);
    frames(&mut h, 2);
    h.get_by_label("Chords…").click();
    frames(&mut h, 2);
    assert_eq!(h.state().cl.gen.track, None);
    assert_eq!(h.state().cl.gen.tab, GenTab::Chords);
    plain(&mut h, 4, 1);
    let before = clips(&h);
    let n0 = counts(&h);
    click_write(&mut h);
    let c = new_clip(&before, &clips(&h));
    assert_eq!(counts(&h).0, n0.0 + 1, "a new track");
    assert_eq!(
        (c.track, c.start, c.len),
        (3, 0, 4 * BAR),
        "after the last track, at bar 1"
    );
    assert_notes_fit(&c);
    assert_eq!(
        h.state().with(|s| s.song.channel_names[3].clone()),
        "Chords"
    );
    assert!(
        h.state().gen_sound(3).is_some(),
        "the new track plays a melodic sound"
    );
    let s = status(&h);
    assert!(
        s.contains("new track 04") && s.contains("song bars 1–4"),
        "{s}"
    );
    undo(&mut h);
    assert_eq!(counts(&h), n0, "one undo: track, instrument and clip");
    assert_eq!(clips(&h), before);
}

/// Scenario 3: a new empty song: the Chords… button still writes chords
#[test]
fn chords_in_an_empty_song() {
    let mut h = empty_song();
    assert_eq!(counts(&h), (0, 0, 0));
    h.get_by_label("Chords…").click();
    frames(&mut h, 2);
    plain(&mut h, 8, 3);
    click_write(&mut h);
    let c = new_clip(&[], &clips(&h));
    assert_eq!((c.track, c.start, c.len), (0, 0, 8 * BAR));
    assert_notes_fit(&c);
    assert_eq!(counts(&h).0, 1);
    assert!(h.state().gen_sound(0).is_some());
    undo(&mut h);
    assert_eq!(counts(&h), (0, 0, 0));
}

/// Scenario 4: Chords… on a drum track: the chords go on a new track below it
/// with a melodic synth, the drums stay as they were
#[test]
fn chords_on_a_drum_track_get_a_new_melodic_track() {
    let mut h = song3();
    let drums = clips(&h);
    assert!(drums.iter().any(|c| c.track == 2 && c.drum));
    h.state_mut().track_chords(2);
    plain(&mut h, 4, 2);
    let n0 = counts(&h);
    click_write(&mut h);
    let after = clips(&h);
    let c = new_clip(&drums, &after);
    assert_eq!(c.track, 3, "the new track sits below the drums");
    assert!(!c.drum);
    assert_notes_fit(&c);
    assert_eq!(counts(&h).0, n0.0 + 1);
    assert!(h.state().gen_sound(3).is_some(), "not the drum kit");
    for d in &drums {
        assert!(after.contains(d), "the drum clip is untouched");
    }
    undo(&mut h);
    assert_eq!(counts(&h), n0);
    assert_eq!(clips(&h), drums);
}

/// Scenario 5: the loop is on over bars 9–12: the clip starts at the loop start
/// whatever the play marker says
#[test]
fn chords_go_to_the_loop_start() {
    let mut h = song3();
    h.state_mut().with(|s| {
        let t = s.song.timeline.as_deref_mut().unwrap();
        t.loop_on = true;
        t.loop_range = Some((8 * BAR, 12 * BAR));
    });
    h.state_mut().tlv.cue = BAR + 7;
    h.state_mut().track_chords(1);
    plain(&mut h, 4, 5);
    let before = clips(&h);
    click_write(&mut h);
    let c = new_clip(&before, &clips(&h));
    assert_eq!((c.track, c.start, c.len), (1, 8 * BAR, 4 * BAR));
    assert_notes_fit(&c);
    assert!(status(&h).contains("song bars 9–12"), "{}", status(&h));
}

/// Scenario 6: the track already has a clip in those bars: a new track below it
/// takes the chords (a copy of the track's sound), the clip is untouched
#[test]
fn chords_over_a_busy_track_go_on_a_new_track() {
    let mut h = song3();
    h.state_mut().tlv.cue = 0;
    h.state_mut()
        .track_add_empty_pattern(0)
        .expect("a Lead pattern");
    frames(&mut h, 2);
    let before = clips(&h);
    let lead = before
        .iter()
        .find(|c| c.track == 0)
        .cloned()
        .expect("a Lead clip");
    h.state_mut().tlv.cue = 0;
    h.state_mut().track_chords(0);
    // the pie opened the Lead clip: Write goes to a new clip at bar 1
    h.state_mut().cl.gen.dest = ChordDest::Track;
    plain(&mut h, 4, 9);
    let n0 = counts(&h);
    click_write(&mut h);
    let after = clips(&h);
    let c = new_clip(&before, &after);
    assert_eq!((c.track, c.start), (1, 0), "a new track below Lead");
    assert_notes_fit(&c);
    assert_eq!(counts(&h).0, n0.0 + 1);
    assert!(after
        .iter()
        .any(|x| x.id == lead.id && x.track == 0 && x.notes == lead.notes));
    let (a, b) = (
        h.state().gen_sound(0).unwrap(),
        h.state().gen_sound(1).unwrap(),
    );
    assert_eq!(
        std::mem::discriminant(&a.kind),
        std::mem::discriminant(&b.kind),
        "the same kind of sound as Lead"
    );
    undo(&mut h);
    assert_eq!(counts(&h), n0);
}

/// Scenario 7: the roll's Generate on a clip: Write puts notes (not just markers)
/// into that clip, and a 1-bar clip grows to show all 8 bars
#[test]
fn chords_into_the_open_clip_write_notes_and_grow_it() {
    let mut h = song3();
    h.state_mut().tlv.cue = 0;
    h.state_mut()
        .track_add_empty_pattern(0)
        .expect("a Lead pattern");
    frames(&mut h, 2);
    let before = clips(&h);
    let lead = before.iter().find(|c| c.track == 0).cloned().unwrap();
    assert_eq!(lead.len, BAR);
    let id = lead.id;
    let data = h.state().with(|s| {
        let t = s.song.timeline.as_deref().unwrap();
        t.clips
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .notes()
            .unwrap() as _
    });
    h.state_mut().track_open_pattern(0, id);
    frames(&mut h, 2);
    assert_eq!(
        h.state().pr_target(),
        Some(Target::Notes {
            clip: Some(id),
            data
        })
    );
    h.state_mut().cl_open_gen_at(0);
    plain(&mut h, 8, 4);
    assert_eq!(h.state().cl.gen.dest, ChordDest::Clip);
    assert!(h.state().cl.gen.write, "notes by default, not markers only");
    let n0 = counts(&h);
    click_write(&mut h);
    let after = clips(&h);
    assert_eq!(counts(&h), n0, "no new clip or track");
    let c = after.iter().find(|c| c.id == id).unwrap();
    assert_eq!(c.len, 8 * BAR, "the clip grew to show the chords");
    assert_notes_fit(c);
    undo(&mut h);
    assert_eq!(clips(&h), before);
}

/// Scenario 8: a trimmed or looped clip grows to show what was written
#[test]
fn show_upto_grows_trimmed_and_looped_clips() {
    let base = tl::Clip {
        id: 1,
        len: BAR,
        kind: tl::ClipKind::Notes { data: 0 },
        ..tl::Clip::default()
    };
    // untrimmed: follows the data's new length
    let mut c = base.clone();
    super::chordlane::show_upto(&mut c, 8 * BAR, BAR, 8 * BAR);
    assert_eq!(c.len, 8 * BAR);
    // trimmed (shorter than its data): grows to the written end
    let mut c = base.clone();
    super::chordlane::show_upto(&mut c, 4 * BAR - 3, 2 * BAR, 4 * BAR);
    assert_eq!(c.len, 4 * BAR);
    // left-trimmed: the end is counted from the offset
    let mut c = tl::Clip {
        offset: BAR,
        ..base.clone()
    };
    super::chordlane::show_upto(&mut c, 4 * BAR, 8 * BAR, 8 * BAR);
    assert_eq!(c.len, 3 * BAR);
    // looped: the loop takes in the new chords (whole bars)
    let mut c = tl::Clip {
        loop_len: Some(BAR),
        len: 4 * BAR,
        ..base.clone()
    };
    super::chordlane::show_upto(&mut c, 2 * BAR + 5, BAR, 4 * BAR);
    assert_eq!((c.loop_len, c.len), (Some(3 * BAR), 4 * BAR));
    // already long enough: unchanged
    let mut c = tl::Clip {
        len: 16 * BAR,
        ..base
    };
    super::chordlane::show_upto(&mut c, 4 * BAR, 16 * BAR, 16 * BAR);
    assert_eq!(c.len, 16 * BAR);
}

/// Scenario 9: the same seed writes the same chords and notes; another seed (or
/// Reroll) gives another progression
#[test]
fn the_same_seed_writes_the_same_notes() {
    let run = |seed: u64| {
        let mut h = song3();
        h.state_mut().track_chords(0);
        plain(&mut h, 8, seed);
        click_write(&mut h);
        let c = new_clip(
            &[],
            &clips(&h)
                .into_iter()
                .filter(|c| !c.drum)
                .collect::<Vec<_>>(),
        );
        (c.chords, c.notes)
    };
    let a = run(11);
    assert_eq!(a, run(11));
    assert!((12..20).map(run).any(|b| b != a), "other seeds differ");
}

/// Scenario 10: the cards' text: the target is named, the song bars shown, no
/// "Bars bars", and everything fits at 1366 x 768
#[test]
fn card_text_names_the_target_and_bars() {
    let mut h = song3();
    h.state_mut().tlv.cue = 4 * BAR;
    h.state_mut().track_chords(0);
    plain(&mut h, 8, 1);
    for l in ["Write to timeline", "New track", "Song bars", "5–12"] {
        assert!(
            h.query_all_by_label_contains(l).next().is_some(),
            "chords card shows {l:?}"
        );
    }
    assert!(h.query_by_label_contains("Track 01 · ").is_some());
    let w = h.get_by_label("Write to timeline").rect();
    assert!(
        w.bottom() <= 768.0 && w.right() <= 1366.0,
        "Write is on screen: {w:?}"
    );
    for tab in [GenTab::Melody, GenTab::Bass] {
        h.state_mut().cl.gen.tab = tab;
        frames(&mut h, 3);
        assert!(h.query_by_label_contains("Bars bars").is_none());
        assert!(
            h.query_all_by(|n| n
                .label()
                .is_some_and(|l| l.starts_with("bars ") && l.len() < 14))
                .next()
                .is_none(),
            "no bare 'bars a–b' value after a Bars label"
        );
        assert!(
            h.query_by_label_contains("Song bars").is_some(),
            "{tab:?} without a clip"
        );
        let w = h.get_by_label("Write to timeline").rect();
        assert!(w.bottom() <= 768.0, "{tab:?} Write is on screen: {w:?}");
    }
}

/// Scenario 11: Melody and Bass from the track pie on a track with no clip: a new
/// track at the play marker's bar, one undo
#[test]
fn melody_and_bass_without_a_clip_make_a_new_track() {
    for (tab, part) in [(GenTab::Melody, "Melody"), (GenTab::Bass, "Bass")] {
        let mut h = song3();
        h.state_mut().tlv.cue = 2 * BAR + 1;
        match tab {
            GenTab::Melody => h.state_mut().track_melody(0),
            _ => h.state_mut().track_bass(0),
        }
        frames(&mut h, 2);
        assert_eq!(h.state().cl.gen.tab, tab);
        let before = clips(&h);
        let n0 = counts(&h);
        click_write(&mut h);
        let c = new_clip(&before, &clips(&h));
        assert_eq!(
            (c.track, c.start),
            (1, 2 * BAR),
            "{part}: below Lead at bar 3"
        );
        assert!(!c.notes.is_empty(), "{part}: notes");
        let end = c.notes.iter().map(|n| n.0).max().unwrap();
        assert!(end < c.len, "{part}: every note inside the clip");
        assert_eq!(h.state().with(|s| s.song.channel_names[1].clone()), part);
        assert!(status(&h).contains("song bar"), "{}", status(&h));
        undo(&mut h);
        assert_eq!(counts(&h), n0, "{part}: one undo");
        assert_eq!(clips(&h), before);
    }
}

/// Scenario 12: a drum track's Bass from the pie: a new Bass track with a synth
#[test]
fn bass_on_the_drum_track_makes_a_bass_track() {
    let mut h = song3();
    h.state_mut().track_bass(2);
    frames(&mut h, 2);
    let before = clips(&h);
    click_write(&mut h);
    let c = new_clip(&before, &clips(&h));
    assert_eq!(c.track, 3);
    assert!(!c.drum && !c.notes.is_empty());
    assert!(h.state().gen_sound(3).is_some());
}

/// Scenario 13: Preview plays on a melodic track: the one Write goes to, else the
/// song's first melodic track (not the selected drum kit), else nothing
#[test]
fn preview_plays_on_a_melodic_track() {
    let mut h = song3();
    h.state_mut().track_chords(1);
    assert_eq!(h.state().gen_preview_ch(), Some(1));
    h.state_mut().track_chords(2);
    frames(&mut h, 2);
    assert_eq!(h.state().gen_preview_ch(), Some(0), "the drums aren't used");
    h.get_by_label("Preview").click();
    frames(&mut h, 3);
    assert!(h.state().cl.gen.open, "Preview doesn't close the panel");
    let mut h = empty_song();
    h.get_by_label("Chords…").click();
    frames(&mut h, 2);
    assert_eq!(h.state().gen_preview_ch(), None);
}

/// Lead (WaveForge), a Drum Kit, a drum generator (Drum Synth) and a
/// synth patch "Trance Kick" (a Synth, but a kick), each with a 1-bar
/// pattern at bar 1 so the roll can open it
fn song_perc() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.set_size(egui::vec2(1366.0, 768.0));
    {
        let a = h.state_mut();
        a.new_song();
        for (k, n) in ["Lead", "Kit", "DrumGen", "Kick"].into_iter().enumerate() {
            a.track_insert(k, n);
        }
        a.view = View::Timeline;
        a.track_new_instrument(0, Builtin::WaveForge);
        a.track_set_pattern_instrument(1, Builtin::DrumKit, Some("909 Kit"));
        a.track_new_instrument(2, Builtin::DrumSynth);
        a.track_new_instrument(3, Builtin::Synth);
        let slot = a.track_instrument(3).expect("the Kick's synth") as usize;
        a.with(|s| {
            let i = &mut s.song.instruments[slot - 1];
            i.name = "Trance Kick".into();
            if let crate::model::InstrumentKind::Synth(p) = &mut i.kind {
                p.name = "Trance Kick".into();
            }
        });
        for k in [0, 2, 3] {
            a.tlv.cue = 0;
            a.track_add_empty_pattern(k).expect("a pattern");
        }
        a.undo_reset();
    }
    frames(&mut h, 3);
    h
}

const PITCHED: [&str; 3] = ["Chords…", "Melody…", "Bass…"];

fn ring_labels(h: &Harness<'static, App>, tr: usize) -> Vec<String> {
    let i = h.state().pie_info(tr);
    super::trackring::ring_slots(super::trackring::Level::Add, &i)
        .into_iter()
        .map(|s| s.label)
        .collect()
}

fn clip_on(h: &Harness<'static, App>, tr: usize) -> u32 {
    clips(h)
        .into_iter()
        .find(|c| c.track == tr)
        .map(|c| c.id)
        .expect("a clip on the track")
}

/// Scenario 14: the percussive check: drum engines, kits, drum names
/// (a synth "Trance Kick"); melodic synths, an "808 Bass" are pitched
#[test]
fn percussive_sounds_are_told_from_pitched_ones() {
    use super::trackpie::name_is_drum;
    for n in [
        "Trance Kick",
        "Snare 2",
        "Open Hats",
        "Hi-Hat",
        "Clap",
        "Low Tom",
        "Perc Loop",
        "Crash Cymbal",
        "Rimshot",
        "909 Kit",
        "808 kit",
        "Drums",
        "kick_01.wav",
    ] {
        assert!(name_is_drum(n), "{n} is a drum");
    }
    for n in [
        "Saw Lead",
        "808 Bass",
        "Tomato Pad",
        "Chat Pluck",
        "Supersaw",
        "Rolling Bass",
        "",
    ] {
        assert!(!name_is_drum(n), "{n} is pitched");
    }
    let h = song_perc();
    let a = h.state();
    assert!(!a.track_percussive(0), "WaveForge");
    for k in 1..4 {
        assert!(a.track_percussive(k), "track {k}");
        assert!(a.gen_sound(k).is_none(), "track {k} lends no sound");
    }
}

/// Scenario 15: the track pie offers Chords… / Melody… / Bass… on a
/// melodic synth only, not on a drum kit, a drum generator or a synth kick
#[test]
fn pitched_generators_hide_on_percussive_tracks_in_the_pie() {
    let h = song_perc();
    let lead = ring_labels(&h, 0);
    for p in PITCHED {
        assert!(lead.iter().any(|l| l == p), "{p} on the Lead: {lead:?}");
    }
    for k in 1..4 {
        let l = ring_labels(&h, k);
        for p in PITCHED {
            assert!(!l.iter().any(|x| x == p), "no {p} on track {k}: {l:?}");
        }
    }
}

/// Scenario 16: the Chords… button beside + Add track hides while a
/// percussive track is selected; it shows for a melodic one and for none
#[test]
fn chords_button_hides_for_a_percussive_track() {
    let mut h = song_perc();
    let shown = |h: &Harness<'static, App>| {
        h.query_all_by(|n| n.label().as_deref() == Some("Chords…"))
            .count()
            > 0
    };
    for (k, want) in [(0, true), (1, false), (2, false), (3, false)] {
        h.state_mut().tl_select_track(k, false);
        frames(&mut h, 3);
        assert_eq!(h.state().tl_cur_track(), k);
        assert_eq!(shown(&h), want, "track {k}");
    }
    // the Master / no track: Chords… stays, for a new Chords track
    let n = h.state().with(|s| s.song.channels);
    h.state_mut().tl_select_track(n, false);
    frames(&mut h, 3);
    if h.state().tl_cur_track() >= n {
        assert!(shown(&h), "no track: Chords… for a new track");
    }
}

/// Scenario 17: the piano roll shows the chord lane and Generate on a
/// melodic clip only (the synth kick's clip has neither)
#[test]
fn roll_hides_chords_and_generate_on_a_percussive_clip() {
    let mut h = song_perc();
    for (k, want) in [(0, true), (3, false)] {
        let id = clip_on(&h, k);
        h.state_mut().track_open_pattern(k, id);
        frames(&mut h, 4);
        assert!(
            matches!(h.state().pr_target(), Some(Target::Notes { clip: Some(c), .. }) if c == id),
            "track {k}'s clip in the roll"
        );
        let gen = h
            .query_all_by(|n| n.label().as_deref() == Some("Generate"))
            .count()
            > 0;
        assert_eq!(gen, want, "Generate on track {k}");
        let t = h.state().pr_target().unwrap();
        assert_eq!(h.state().target_percussive(t), !want);
    }
}

/// Scenario 18: Chords / Melody / Bass written from a percussive track
/// (reached through the panel itself) land on a new track with a pitched
/// sound, never a copy of the kick
#[test]
fn a_new_generator_track_never_copies_a_percussive_sound() {
    for tab in [GenTab::Chords, GenTab::Melody, GenTab::Bass] {
        let mut h = song_perc();
        let id = clip_on(&h, 3);
        h.state_mut().track_open_pattern(3, id);
        frames(&mut h, 2);
        h.state_mut().gen_open_for(Some(3), tab);
        plain(&mut h, 4, 3);
        let (n0, _, _) = counts(&h);
        let before = clips(&h);
        if tab == GenTab::Chords {
            click_write(&mut h);
        } else {
            h.state_mut().cl.gen.mel.new_track = true;
            h.state_mut().cl.gen.bass.new_track = true;
            click_write(&mut h);
        }
        let after = clips(&h);
        let c = new_clip(&before, &after);
        assert_eq!(counts(&h).0, n0 + 1, "{tab:?}: a new track");
        assert_ne!(c.track, 3, "{tab:?}: not on the kick");
        assert!(
            !h.state().track_percussive(c.track),
            "{tab:?}: a pitched sound"
        );
        assert!(h.state().gen_sound(c.track).is_some(), "{tab:?}");
        assert!(
            before.iter().all(|b| after.contains(b)),
            "{tab:?}: the old clips are untouched"
        );
    }
}
