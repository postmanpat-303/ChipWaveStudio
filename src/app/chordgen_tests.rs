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

/// 1. the track pie's Chords… on a melodic track with no clips, the play
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

/// 2. no track selected (the Song / Master track): the timeline's
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

/// 3. a new empty song: the Chords… button still writes chords
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

/// 4. Chords… on a drum track: the chords go on a new track below it
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

/// 5. the loop is on over bars 9–12: the clip starts at the loop start
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

/// 6. the track already has a clip in those bars: a new track below it
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

/// 7. the roll's Generate on a clip: Write puts notes (not just markers)
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

/// 8. a trimmed or looped clip grows to show what was written
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

/// 9. the same seed writes the same chords and notes; another seed (or
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

/// 10. the cards' text: the target is named, the song bars shown, no
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

/// 11. Melody and Bass from the track pie on a track with no clip: a new
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

/// 12. a drum track's Bass from the pie: a new Bass track with a synth
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

/// 13. Preview plays on a melodic track: the one Write goes to, else the
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
