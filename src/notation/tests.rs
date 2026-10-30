//! Sheet music export: values and bars add up, ties, chords, rests, drums,
//! keys, signatures, clefs, tracker and timeline songs, the demos, and (when
//! LilyPond is installed) real PDFs.
use super::*;
use crate::model::{Cell, Pattern};
use crate::timeline::{enter_timeline_mode, new_note_clip, ClipKind, Note};

/// a tracker-mode song: `ch` channels, one 16-row block, instrument 1
fn song(ch: usize) -> Song {
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.timeline = None;
    s.bpm = 120.0;
    s.channels = ch;
    s.channel_names = (0..ch).map(|i| format!("T{}", i + 1)).collect();
    s.patterns = vec![Pattern::new("A", 16, ch)];
    s.order = vec![0];
    s.mixer.channels.resize(ch, Default::default());
    s.info.key = None;
    s.info.artist.clear();
    s
}

fn put(s: &mut Song, pat: usize, row: usize, ch: usize, note: u8, instr: u8) {
    s.patterns[pat].cells[row][ch] = Cell {
        note,
        instr,
        ..Default::default()
    };
}

/// a pitched instrument slot of the demo (1-based) with no transpose
fn pitched(s: &mut Song) -> u8 {
    let i = s
        .instruments
        .iter()
        .position(|i| drum_of_inst(i).is_none() && !matches!(i.kind, InstrumentKind::Gen(_)))
        .unwrap_or(0);
    s.instruments[i].transpose = 0;
    s.instruments[i].name = "Lead".into();
    i as u8 + 1
}

fn drum_of_inst(i: &crate::model::Instrument) -> Option<Drum> {
    drum_by_name(&i.name)
}

fn opts(s: &Song) -> Opts {
    let mut o = default_opts(s, "");
    o.key = KeyChoice::None;
    o
}

fn items_of(sc: &Score, part: usize, staff: usize) -> &Vec<Vec<Item>> {
    &sc.parts[part].staves[staff].bars
}

/// every staff's bars add up to the bar length, ties pair up, values are
/// writable, drums are never tied
fn check_score(sc: &Score) {
    for p in &sc.parts {
        for st in &p.staves {
            assert_eq!(
                st.bars.len(),
                sc.bars.len(),
                "{}: one entry per bar",
                p.name
            );
            let flat: Vec<&Item> = st.bars.iter().flatten().collect();
            for (b, items) in sc.bars.iter().zip(&st.bars) {
                let sum: Tick = items.iter().map(|i| i.dur).sum();
                assert_eq!(sum, b.len, "{}: bar at {} adds up", p.name, b.start);
                let mut pos = 0;
                for it in items {
                    if !it.bar_rest {
                        assert!(
                            VALUES.iter().any(|v| v.0 == it.dur),
                            "{}: {} is a written value",
                            p.name,
                            it.dur
                        );
                        let (_, ty, _) = value_name(it.dur);
                        assert!(!ty.is_empty());
                    }
                    pos += it.dur;
                    assert!(pos <= b.len);
                }
            }
            for w in flat.windows(2) {
                assert_eq!(
                    w[0].tie, w[1].tied,
                    "{}: tie start and stop pair up",
                    p.name
                );
                if w[0].tie {
                    assert_eq!(w[0].content, w[1].content, "a tie joins the same notes");
                }
            }
            if let Some(f) = flat.first() {
                assert!(!f.tied);
            }
            if let Some(l) = flat.last() {
                assert!(!l.tie);
            }
            if p.drums {
                assert!(
                    flat.iter().all(|i| !i.tie && !i.tied),
                    "drums are never tied"
                );
            }
        }
    }
}

/// a tiny XML well-formedness check: tags nest and close
fn xml_ok(x: &str) {
    let mut stack: Vec<String> = vec![];
    let mut rest = x;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let j = rest.find('>').expect("tag closes");
        let tag = &rest[..j];
        rest = &rest[j + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop().as_deref(), Some(name.trim()), "tags nest");
        } else if !tag.ends_with('/') {
            stack.push(tag.split_whitespace().next().unwrap().to_string());
        }
    }
    assert!(stack.is_empty(), "every tag closes: {stack:?}");
    assert!(
        !x.contains("&")
            || x.matches('&').count()
                == x.matches("&amp;").count()
                    + x.matches("&lt;").count()
                    + x.matches("&gt;").count()
                    + x.matches("&quot;").count()
    );
}

/// braces and chord brackets balance in a .ly file
fn ly_ok(l: &str) {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut prev = ' ';
    for c in l.chars() {
        if c == '"' && prev != '\\' {
            in_str = !in_str;
        }
        if !in_str {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0);
        }
        prev = c;
    }
    assert_eq!(depth, 0, "braces balance");
    assert!(l.contains("\\version"));
}

#[test]
fn split_values_add_up_and_stay_in_the_bar() {
    for bar in [
        4 * PPQ,
        3 * PPQ,
        2 * PPQ,
        3 * PPQ / 2 * 2,
        7 * PPQ / 2,
        5 * PPQ,
        6 * PPQ,
    ] {
        let mut p = 0;
        while p < bar {
            let mut len = MIN_VALUE;
            while p + len <= bar {
                for dots in [false, true] {
                    let v = split(p, len, bar, dots);
                    assert_eq!(v.iter().sum::<Tick>(), len, "bar {bar} pos {p} len {len}");
                    let mut at = p;
                    for d in v {
                        assert!(VALUES.iter().any(|x| x.0 == d), "{d} is a value");
                        if !dots {
                            assert!(
                                VALUES.iter().any(|x| x.0 == d && x.1 == d),
                                "rests undotted"
                            );
                        }
                        at += d;
                        assert!(at <= bar);
                    }
                }
                len += MIN_VALUE;
            }
            p += MIN_VALUE;
        }
    }
    // a whole bar of 4/4 is one whole note; a beat and a half on the beat is dotted
    assert_eq!(split(0, 4 * PPQ, 4 * PPQ, true), vec![4 * PPQ]);
    assert_eq!(split(0, 3 * PPQ / 2, 4 * PPQ, true), vec![3 * PPQ / 2]);
    // a half note may start on beat 2; an 8th-late quarter is two tied 8ths
    assert_eq!(split(PPQ, 2 * PPQ, 4 * PPQ, true), vec![2 * PPQ]);
    assert_eq!(split(PPQ / 2, PPQ, 4 * PPQ, true), vec![PPQ / 2, PPQ / 2]);
    // rests: an 8th then a half, not a dotted 8th
    assert_eq!(
        split(3 * PPQ / 2, 5 * PPQ / 2, 4 * PPQ, false),
        vec![PPQ / 2, 2 * PPQ]
    );
}

#[test]
fn tracker_notes_rests_and_note_offs() {
    let mut s = song(1);
    let i = pitched(&mut s);
    put(&mut s, 0, 0, 0, 49, i); // C-4 = middle C
    put(&mut s, 0, 6, 0, 255, 0); // off after a dotted quarter
    put(&mut s, 0, 8, 0, 51, 0); // D-4, instrument kept, to the end
    let sc = build(&s, &opts(&s));
    check_score(&sc);
    assert_eq!(sc.bars.len(), 1);
    let b = &items_of(&sc, 0, 0)[0];
    let durs: Vec<Tick> = b.iter().map(|i| i.dur).collect();
    assert_eq!(durs, vec![3 * PPQ / 2, PPQ / 2, 2 * PPQ]);
    assert_eq!(b[0].content, Content::Notes(vec![60]));
    assert_eq!(b[1].content, Content::Rest);
    assert_eq!(b[2].content, Content::Notes(vec![62]));
    let ly = to_ly(&sc);
    ly_ok(&ly);
    assert!(ly.contains("c'4. r8 d'2 |"), "{ly}");
    let x = to_musicxml(&sc);
    xml_ok(&x);
    assert!(x.contains("<step>C</step><octave>4</octave></pitch><duration>12</duration>"));
    assert!(x.contains("<rest/><duration>4</duration>"));
}

#[test]
fn ties_across_the_barline() {
    let mut s = song(1);
    let i = pitched(&mut s);
    s.patterns = vec![Pattern::new("A", 32, 1)];
    put(&mut s, 0, 12, 0, 49, i); // beat 4 of bar 1 ...
    put(&mut s, 0, 20, 0, 255, 0); // ... to beat 2 of bar 2
    let sc = build(&s, &opts(&s));
    check_score(&sc);
    assert_eq!(sc.bars.len(), 2);
    let b1 = &items_of(&sc, 0, 0)[0];
    let b2 = &items_of(&sc, 0, 0)[1];
    assert!(b1.last().unwrap().tie && b1.last().unwrap().dur == PPQ);
    assert!(b2[0].tied && b2[0].dur == PPQ && !b2[0].tie);
    let ly = to_ly(&sc);
    assert!(ly.contains("c'4~ | % 1"), "{ly}");
    let x = to_musicxml(&sc);
    xml_ok(&x);
    assert!(x.contains("<tie type=\"start\"/>") && x.contains("<tied type=\"stop\"/>"));
}

#[test]
fn chords_from_tracks_that_share_an_instrument() {
    let mut s = song(3);
    let i = pitched(&mut s);
    for (ch, k) in [(0, 49), (1, 53), (2, 56)] {
        put(&mut s, 0, 0, ch, k, i); // C E G
        put(&mut s, 0, 4, ch, 255, 0);
    }
    let o = opts(&s);
    assert_eq!(o.parts.len(), 1, "three tracks, one instrument: one part");
    assert_eq!(o.parts[0].name, "Lead");
    let sc = build(&s, &o);
    check_score(&sc);
    assert_eq!(
        items_of(&sc, 0, 0)[0][0].content,
        Content::Notes(vec![60, 64, 67])
    );
    assert!(to_ly(&sc).contains("<c' e' g'>4"));
    let x = to_musicxml(&sc);
    xml_ok(&x);
    assert_eq!(x.matches("<chord/>").count(), 2);
    // unmerged: three parts
    let mut o2 = o.clone();
    o2.parts = reparts(&s, false, &o.parts);
    assert_eq!(o2.parts.len(), 3);
    check_score(&build(&s, &o2));
}

#[test]
fn drums_get_a_drum_staff_short_hits_and_no_ties() {
    let mut s = song(2);
    let lead = pitched(&mut s);
    let k = s.instruments.len();
    let mut kick = s.instruments[lead as usize - 1].clone();
    kick.name = "Kick".into();
    let mut hat = kick.clone();
    hat.name = "Closed Hat".into();
    s.instruments.push(kick);
    s.instruments.push(hat);
    let (ki, hi) = (k as u8 + 1, k as u8 + 2);
    for r in [0, 4, 8, 12] {
        put(&mut s, 0, r, 0, 49, ki);
    }
    for r in [2, 6, 10, 14] {
        put(&mut s, 0, r, 1, 49, hi);
    }
    let mut o = opts(&s);
    o.merge = false;
    o.parts = reparts(&s, false, &o.parts);
    assert!(o.parts.iter().all(|p| p.drums));
    let sc = build(&s, &o);
    check_score(&sc);
    assert!(sc
        .parts
        .iter()
        .all(|p| p.staves[0].clef == Clef::Percussion));
    let ly = to_ly(&sc);
    ly_ok(&ly);
    assert!(ly.contains("\\new DrumStaff"));
    assert!(
        ly.contains("bd4 bd4 bd4 bd4"),
        "kicks last at most to the end of the beat: {ly}"
    );
    assert!(ly.contains("r8 hh8 r8 hh8"), "{ly}");
    let x = to_musicxml(&sc);
    xml_ok(&x);
    assert!(x.contains("<sign>percussion</sign>"));
    assert!(x.contains(
        "<unpitched><display-step>F</display-step><display-octave>4</display-octave></unpitched>"
    ));
    assert!(x.contains("<notehead>x</notehead>"));
    assert!(x.contains("<midi-unpitched>37</midi-unpitched>"));
    // Include drums off: no drum parts
    o.drums = false;
    assert!(build(&s, &o).parts.is_empty());
}

#[test]
fn kit_pads_name_the_drums() {
    // the Oceans 11 demo plays a Drum Kit
    let s = crate::fileio::demo_song_n(4);
    let o = default_opts(&s, "");
    let p = o.parts.iter().find(|p| p.drums).expect("a drum part");
    let sc = build(&s, &o);
    check_score(&sc);
    let pi = o
        .parts
        .iter()
        .filter(|q| q.on)
        .position(|q| q == p)
        .unwrap();
    let used: Vec<Drum> = sc.parts[pi].staves[0]
        .bars
        .iter()
        .flatten()
        .filter_map(|i| match &i.content {
            Content::Drums(d) => Some(d.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(
        used.contains(&Drum::Kick) || used.contains(&Drum::Snare),
        "{used:?}"
    );
}

#[test]
fn keys_spell_with_the_signature() {
    let d = KeySig::of(Key::new(2, Scale::Major));
    assert_eq!(
        d,
        KeySig {
            fifths: 2,
            minor: false
        }
    );
    assert_eq!(d.spell(66), ('F', 1, 4), "F# in D major");
    assert_eq!(d.home(), ('D', 0));
    let bb = KeySig::of(Key::new(10, Scale::Major));
    assert_eq!(bb.fifths, -2);
    assert_eq!(bb.spell(70), ('B', -1, 4));
    assert_eq!(bb.spell(63), ('E', -1, 4));
    let dm = KeySig::of(Key::new(2, Scale::Minor));
    assert_eq!(
        dm,
        KeySig {
            fifths: -1,
            minor: true
        }
    );
    assert_eq!(dm.home(), ('D', 0));
    let fsm = KeySig::of(Key::new(6, Scale::Minor));
    assert_eq!(fsm.fifths, 3);
    assert_eq!(fsm.home(), ('F', 1));
    // E Phrygian: no sharps or flats
    assert_eq!(KeySig::of(Key::new(4, Scale::Phrygian)).fifths, 0);
    // F# major: E# for F
    let fs = KeySig::of(Key::new(6, Scale::Major));
    assert_eq!(fs.fifths, 6);
    assert_eq!(fs.spell(65), ('E', 1, 4));
    // B# / Cb octaves
    let cs = KeySig {
        fifths: 7,
        minor: false,
    };
    assert_eq!(cs.spell(60), ('B', 1, 3));
    let cb = KeySig {
        fifths: -7,
        minor: false,
    };
    assert_eq!(cb.spell(59), ('C', -1, 4));
    // out-of-key notes: sharps in sharp keys, flats in flat keys
    assert_eq!(d.spell(63), ('D', 1, 4));
    assert_eq!(bb.spell(66), ('G', -1, 4));
    // in the .ly file
    let mut s = song(1);
    let i = pitched(&mut s);
    put(&mut s, 0, 0, 0, 49 + 6, i); // F#4
    let mut o = opts(&s);
    o.key = KeyChoice::Key(Key::new(2, Scale::Major));
    let sc = build(&s, &o);
    let ly = to_ly(&sc);
    assert!(
        ly.contains("\\key d \\major") && ly.contains("fis'1"),
        "{ly}"
    );
    assert!(to_musicxml(&sc).contains("<fifths>2</fifths><mode>major</mode>"));
}

#[test]
fn clefs_follow_the_range() {
    assert_eq!(clefs(&[60, 64, 72]), vec![Clef::Treble]);
    assert_eq!(clefs(&[40, 43, 48]), vec![Clef::Bass]);
    assert_eq!(clefs(&[28, 31, 35]), vec![Clef::Bass8vb]);
    assert_eq!(clefs(&[84, 88, 91]), vec![Clef::Treble8va]);
    assert_eq!(clefs(&[50, 55, 57]), vec![Clef::Treble8vb]);
    assert_eq!(clefs(&[36, 48, 72, 79]), vec![Clef::Treble, Clef::Bass]);
    let mut s = song(1);
    let i = pitched(&mut s);
    put(&mut s, 0, 0, 0, 25, i); // C-2 = MIDI 36
    put(&mut s, 0, 8, 0, 85, i); // C-7 = MIDI 96
    let sc = build(&s, &opts(&s));
    check_score(&sc);
    assert_eq!(sc.parts[0].staves.len(), 2, "a grand staff");
    let ly = to_ly(&sc);
    assert!(ly.contains("\\new PianoStaff"));
    let x = to_musicxml(&sc);
    xml_ok(&x);
    assert!(x.contains("<staves>2</staves>") && x.contains("<backup>"));
    // transposing clefs in both files
    let mut s = song(1);
    let i = pitched(&mut s);
    put(&mut s, 0, 0, 0, 85, i);
    put(&mut s, 0, 8, 0, 89, i);
    let sc = build(&s, &opts(&s));
    assert_eq!(sc.parts[0].staves[0].clef, Clef::Treble8va);
    assert!(to_ly(&sc).contains("\\clef \"treble^8\""));
    assert!(to_musicxml(&sc).contains("<clef-octave-change>1</clef-octave-change>"));
}

#[test]
fn time_signatures_and_tempo_changes() {
    let mut s = song(1);
    let i = pitched(&mut s);
    s.patterns = vec![Pattern::new("A", 48, 1)];
    put(&mut s, 0, 0, 0, 49, i);
    enter_timeline_mode(&mut s);
    {
        let t = s.timeline.as_deref_mut().unwrap();
        t.signature = vec![
            SigPoint {
                tick: 0,
                num: 3,
                den: 4,
            },
            SigPoint {
                tick: 6 * PPQ,
                num: 6,
                den: 8,
            },
        ];
        t.tempo = vec![
            tl::TempoPoint {
                tick: 0,
                bpm: 120.0,
                ramp: false,
            },
            tl::TempoPoint {
                tick: 3 * PPQ,
                bpm: 140.0,
                ramp: false,
            },
        ];
    }
    let sc = build(&s, &opts(&s));
    check_score(&sc);
    assert_eq!(sc.bars[0].len, 3 * PPQ);
    assert_eq!((sc.bars[2].num, sc.bars[2].den), (6, 8));
    assert_eq!(sc.bars[0].tempo, Some((120.0, "")));
    assert_eq!(sc.bars[1].tempo, Some((140.0, "")));
    let ly = to_ly(&sc);
    ly_ok(&ly);
    assert!(ly.contains("\\time 3/4") && ly.contains("\\time 6/8"));
    assert!(ly.contains("\\tempo 4 = 120") && ly.contains("\\tempo 4 = 140"));
    let x = to_musicxml(&sc);
    xml_ok(&x);
    assert!(x.contains("<beats>6</beats><beat-type>8</beat-type>"));
    assert!(x.contains("<sound tempo=\"140\"/>"));
}

#[test]
fn quantise_grid_snaps_notes() {
    let mut s = song(1);
    let i = pitched(&mut s);
    put(&mut s, 0, 1, 0, 49, i); // a 16th late
    put(&mut s, 0, 5, 0, 255, 0);
    let mut o = opts(&s);
    o.grid = PPQ / 2;
    let sc = build(&s, &o);
    check_score(&sc);
    let b = &items_of(&sc, 0, 0)[0];
    assert!(matches!(b[0].content, Content::Notes(_)) || matches!(b[1].content, Content::Notes(_)));
    for it in b {
        assert_eq!(it.dur % (PPQ / 2), 0, "on the 8th grid");
    }
}

#[test]
fn timeline_mode_writes_the_same_score_as_tracker_mode() {
    for n in 0..crate::fileio::DEMOS.len() {
        let s = crate::fileio::demo_song_n(n);
        let mut t = s.clone();
        enter_timeline_mode(&mut t);
        let (a, b) = (
            build(&s, &default_opts(&s, "")),
            build(&t, &default_opts(&t, "")),
        );
        assert_eq!(to_ly(&a), to_ly(&b), "demo {n}");
    }
}

#[test]
fn note_clips_are_written() {
    let mut s = song(1);
    let i = pitched(&mut s);
    enter_timeline_mode(&mut s);
    let id = new_note_clip(&mut s, 0, 0).unwrap();
    let t = s.timeline.as_deref_mut().unwrap();
    let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
    c.len = 4 * PPQ;
    let ClipKind::Notes { data } = c.kind else {
        unreachable!()
    };
    let note = |start, len, key| Note {
        start,
        len,
        key,
        instr: i,
        vel: 100,
        ..Default::default()
    };
    let d = t.notes[data as usize].make_mut();
    d.len = 4 * PPQ;
    d.notes = vec![note(0, PPQ, 49), note(0, PPQ, 53), note(PPQ, 3 * PPQ, 56)];
    let sc = build(&s, &opts(&s));
    check_score(&sc);
    let ly = to_ly(&sc);
    assert!(ly.contains("<c' e'>4 g'2."), "{ly}");
}

#[test]
fn every_demo_exports_valid_files() {
    for n in 0..crate::fileio::DEMOS.len() {
        let s = crate::fileio::demo_song_n(n);
        for grid in [PPQ / 8, PPQ / 4, PPQ] {
            let mut o = default_opts(&s, "");
            o.grid = grid;
            let sc = build(&s, &o);
            check_score(&sc);
            assert!(!sc.parts.is_empty(), "demo {n} has parts");
            let ly = to_ly(&sc);
            ly_ok(&ly);
            let x = to_musicxml(&sc);
            xml_ok(&x);
            assert_eq!(
                x.matches("<measure ").count(),
                sc.bars.len() * sc.parts.len(),
                "demo {n}: every part has every bar"
            );
        }
    }
    // Tallest Morning: D major, 126 BPM, its title
    let s = crate::fileio::demo_song_n(5);
    let o = default_opts(&s, "");
    assert!(o.title.starts_with("Tallest Morning"), "{}", o.title);
    let sc = build(&s, &o);
    assert_eq!(
        sc.key,
        KeySig {
            fifths: 2,
            minor: false
        }
    );
    assert_eq!(sc.bars[0].tempo.map(|t| t.0.round()), Some(126.0));
    assert!(to_ly(&sc).contains("title = \"Tallest Morning"));
}

#[test]
fn composer_is_the_song_artist_or_the_setting_never_made_up() {
    let mut s = song(1);
    pitched(&mut s);
    assert_eq!(default_opts(&s, "").composer, "");
    assert_eq!(default_opts(&s, "  Me ").composer, "Me");
    s.info.artist = "Song Artist".into();
    assert_eq!(default_opts(&s, "Me").composer, "Song Artist");
    let sc = build(&s, &default_opts(&s, ""));
    assert!(to_ly(&sc).contains("composer = \"Song Artist\""));
    // no composer: no composer line
    s.info.artist.clear();
    assert!(!to_ly(&build(&s, &default_opts(&s, ""))).contains("composer"));
}

#[test]
fn bar_range_and_text_escaping() {
    let s = crate::fileio::demo_song_n(5);
    let mut o = default_opts(&s, "");
    o.bars = (3, 6);
    o.title = "A \"quoted\" <title> & more".into();
    let sc = build(&s, &o);
    check_score(&sc);
    assert_eq!(sc.bars.len(), 4);
    assert!(
        sc.bars[0].tempo.is_some(),
        "the first written bar has the tempo"
    );
    let ly = to_ly(&sc);
    ly_ok(&ly);
    assert!(ly.contains("title = \"A \\\"quoted\\\" <title> & more\""));
    let x = to_musicxml(&sc);
    xml_ok(&x);
    assert!(x.contains("A &quot;quoted&quot; &lt;title&gt; &amp; more"));
    assert!(bar_count(&s) >= 6);
}

/// LilyPond for the PDF test: CW_LILYPOND, else the copy on the build box
fn test_lilypond() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("CW_LILYPOND") {
        return lily::find(&p.to_string_lossy());
    }
    let p = std::path::Path::new("/workspace/notation-poc/lilypond-2.26.0");
    p.exists()
        .then(|| lily::find(&p.to_string_lossy()))
        .flatten()
}

#[test]
fn demos_render_to_pdf_with_lilypond_when_installed() {
    let Some(exe) = test_lilypond() else {
        eprintln!("LilyPond not found: PDF render test skipped");
        return;
    };
    let dir = std::env::temp_dir().join(format!("cw-notation-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for n in 0..crate::fileio::DEMOS.len() {
        let s = crate::fileio::demo_song_n(n);
        let sc = build(&s, &default_opts(&s, ""));
        let ly = dir.join(format!("demo{n}.ly"));
        std::fs::write(&ly, to_ly(&sc)).unwrap();
        let (pdf, log) = lily::run(&exe, &ly, &dir.join(format!("demo{n}")))
            .unwrap_or_else(|e| panic!("demo {n}: {e}"));
        let bytes = std::fs::read(&pdf).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "demo {n}: a PDF");
        let bad = lily::problems(&log);
        assert!(bad.is_empty(), "demo {n}: LilyPond reported {bad:?}");
        assert!(!log.contains("warning: barcheck"), "demo {n}: {log}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
